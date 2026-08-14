use std::fmt::Write;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use colored::Colorize;
use ignore::Error;
use log::{debug, warn};
#[cfg(not(target_family = "wasm"))]
use rayon::prelude::*;
use ruff_linter::message::create_panic_diagnostic;
use ruff_python_ast::{SourceType, TomlSourceType};
use rustc_hash::FxHashMap;

use ruff_db::diagnostic::Diagnostic;
use ruff_db::panic::catch_unwind;
use ruff_linter::package::PackageRoot;
use ruff_linter::registry::Rule;
use ruff_linter::settings::types::UnsafeFixes;
use ruff_linter::settings::{LinterSettings, flags};
use ruff_linter::{IOError, Violation, fs, warn_user_once};
use ruff_source_file::SourceFileBuilder;
use ruff_text_size::TextRange;
use ruff_workspace::resolver::{
    PyprojectConfig, ResolvedFile, Resolver, match_exclusion, project_files_in_path,
};

use crate::args::ConfigArguments;
use crate::cache::{Cache, PackageCacheMap, PackageCaches};
use crate::diagnostics::Diagnostics;

/// Run the linter over a collection of files.
pub(crate) fn check(
    files: &[PathBuf],
    pyproject_config: &PyprojectConfig,
    config_arguments: &ConfigArguments,
    cache: flags::Cache,
    noqa: flags::Noqa,
    fix_mode: flags::FixMode,
    unsafe_fixes: UnsafeFixes,
) -> Result<Diagnostics> {
    // Collect all the Python files to check.
    let start = Instant::now();
    let (mut paths, resolver) = project_files_in_path(files, pyproject_config, config_arguments)?;
    debug!("Identified files to lint in: {:?}", start.elapsed());

    // Filter out paths for file types not supported for linting
    paths.retain(|path| {
        if let Ok(ResolvedFile::Root(path) | ResolvedFile::Nested(path)) = path {
            matches!(
                SourceType::from(path),
                SourceType::Python(_)
                    | SourceType::Toml(TomlSourceType::Pyproject | TomlSourceType::Ruff)
            )
        } else {
            true
        }
    });

    if paths.is_empty() {
        warn_user_once!("No Python files found under the given path(s)");
        return Ok(Diagnostics::default());
    }

    let linted_python_paths = linted_python_paths(&paths, &resolver);

    // Discover the package root for each Python file.
    let package_roots = resolver.package_roots(
        &paths
            .iter()
            .flatten()
            .map(ResolvedFile::path)
            .collect::<Vec<_>>(),
    );

    // Load the caches.
    let caches = if cache.is_enabled() {
        Some(PackageCacheMap::init(&package_roots, &resolver))
    } else {
        None
    };

    let start = Instant::now();
    let diagnostics_per_file = paths.par_iter().filter_map(|resolved_file| {
        let result = match resolved_file {
            Ok(resolved_file) => {
                let path = resolved_file.path();
                let package = path
                    .parent()
                    .and_then(|parent| package_roots.get(parent))
                    .and_then(|package| *package);

                let settings = resolver.resolve(path);

                if (settings.file_resolver.force_exclude || !resolved_file.is_root())
                    && match_exclusion(
                        resolved_file.path(),
                        resolved_file.file_name(),
                        &settings.linter.exclude,
                    )
                {
                    return None;
                }

                let cache_root = package
                    .map(PackageRoot::path)
                    .unwrap_or_else(|| path.parent().unwrap_or(path));
                let cache = caches.get(cache_root);

                lint_path(
                    path,
                    package,
                    &settings.linter,
                    cache,
                    noqa,
                    fix_mode,
                    unsafe_fixes,
                )
                .map_err(|e| {
                    (Some(path.to_path_buf()), {
                        let mut error = e.to_string();
                        for cause in e.chain() {
                            write!(&mut error, "\n  Cause: {cause}").unwrap();
                        }
                        error
                    })
                })
            }
            Err(e) => Err((
                if let Error::WithPath { path, .. } = e {
                    Some(path.clone())
                } else {
                    None
                },
                e.io_error()
                    .map_or_else(|| e.to_string(), io::Error::to_string),
            )),
        };

        Some(result.unwrap_or_else(|(path, message)| {
            if let Some(path) = &path {
                let settings = resolver.resolve(path);
                if settings.linter.rules.enabled(Rule::IOError) {
                    let dummy =
                        SourceFileBuilder::new(path.to_string_lossy().as_ref(), "").finish();

                    Diagnostics::new(
                        vec![IOError { message }.into_diagnostic(TextRange::default(), &dummy)],
                        FxHashMap::default(),
                    )
                } else {
                    warn!(
                        "{}{}{} {message}",
                        "Failed to lint ".bold(),
                        fs::relativize_path(path).bold(),
                        ":".bold()
                    );
                    Diagnostics::default()
                }
            } else {
                warn!("{} {message}", "Encountered error:".bold());
                Diagnostics::default()
            }
        }))
    });

    // Aggregate the diagnostics of all checked files and count the checked files.
    // This can't be a regular for loop because we use `par_iter`.
    let (mut all_diagnostics, checked_files) = diagnostics_per_file
        .fold(
            || (Diagnostics::default(), 0u64),
            |(all_diagnostics, checked_files), file_diagnostics| {
                (all_diagnostics + file_diagnostics, checked_files + 1)
            },
        )
        .reduce(
            || (Diagnostics::default(), 0u64),
            |a, b| (a.0 + b.0, a.1 + b.1),
        );

    all_diagnostics += crate::commands::odoo::check_duplicate_inherited_model_extensions(
        &linted_python_paths,
        &resolver,
        noqa,
    )?;

    all_diagnostics
        .inner
        .sort_by(Diagnostic::ruff_start_ordering);

    // Store the caches.
    caches.persist()?;

    let duration = start.elapsed();
    debug!("Checked {checked_files:?} files in: {duration:?}");

    Ok(all_diagnostics)
}

fn linted_python_paths(
    paths: &[Result<ResolvedFile, Error>],
    resolver: &Resolver<'_>,
) -> Vec<PathBuf> {
    paths
        .iter()
        .filter_map(|resolved_file| {
            let resolved_file = resolved_file.as_ref().ok()?;
            let path = resolved_file.path();
            if !matches!(SourceType::from(path), SourceType::Python(_)) {
                return None;
            }
            let settings = resolver.resolve(path);
            if (settings.file_resolver.force_exclude || !resolved_file.is_root())
                && match_exclusion(path, resolved_file.file_name(), &settings.linter.exclude)
            {
                return None;
            }
            Some(path.to_path_buf())
        })
        .collect()
}

/// Wraps [`lint_path`](crate::diagnostics::lint_path) in a [`catch_unwind`](std::panic::catch_unwind) and emits
/// a diagnostic if the linting the file panics.
fn lint_path(
    path: &Path,
    package: Option<PackageRoot<'_>>,
    settings: &LinterSettings,
    cache: Option<&Cache>,
    noqa: flags::Noqa,
    fix_mode: flags::FixMode,
    unsafe_fixes: UnsafeFixes,
) -> Result<Diagnostics> {
    let result = catch_unwind(|| {
        crate::diagnostics::lint_path(path, package, settings, cache, noqa, fix_mode, unsafe_fixes)
    });

    match result {
        Ok(inner) => inner,
        Err(error) => {
            let diagnostic = create_panic_diagnostic(&error, Some(path));
            Ok(Diagnostics::new(vec![diagnostic], FxHashMap::default()))
        }
    }
}

#[cfg(test)]
#[cfg(unix)]
mod test {
    use std::fs;
    use std::os::unix::fs::OpenOptionsExt;

    use anyhow::Result;
    use rustc_hash::FxHashMap;
    use tempfile::TempDir;

    use ruff_db::diagnostic::{DiagnosticFormat, DisplayDiagnosticConfig, DisplayDiagnostics};
    use ruff_linter::UnresolvedRuleSelector;
    use ruff_linter::message::EmitterContext;
    use ruff_linter::registry::Rule;
    use ruff_linter::settings::types::{
        CompiledPerFileIgnoreList, FilePattern, FilePatternSet, PerFileIgnore, PreviewMode,
        UnsafeFixes,
    };
    use ruff_linter::settings::{LinterSettings, flags};
    use ruff_workspace::Settings;
    use ruff_workspace::resolver::{PyprojectConfig, PyprojectDiscoveryStrategy};

    use crate::args::ConfigArguments;

    use super::check;

    /// We check that regular python files, pyproject.toml and jupyter notebooks all handle io
    /// errors gracefully
    #[test]
    fn unreadable_files() -> Result<()> {
        let path = "E902.py";
        let rule_code = Rule::IOError;

        // Create inaccessible files
        let tempdir = TempDir::new()?;
        let pyproject_toml = tempdir.path().join("pyproject.toml");
        let python_file = tempdir.path().join("code.py");
        let notebook = tempdir.path().join("notebook.ipynb");
        for file in [&pyproject_toml, &python_file, &notebook] {
            fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o000)
                .open(file)?;
        }

        // Configure
        let snapshot = format!("{}_{}", rule_code.noqa_code(), path);
        // invalid pyproject.toml is not active by default
        let settings = Settings {
            linter: LinterSettings::for_rules(vec![rule_code, Rule::InvalidPyprojectToml]),
            ..Settings::default()
        };
        let pyproject_config =
            PyprojectConfig::new(PyprojectDiscoveryStrategy::Fixed, settings, None);

        // Run
        let diagnostics = check(
            &[tempdir.path().to_path_buf()],
            &pyproject_config,
            &ConfigArguments::default(),
            flags::Cache::Disabled,
            flags::Noqa::Disabled,
            flags::FixMode::Generate,
            UnsafeFixes::Enabled,
        )
        .unwrap();

        let config = DisplayDiagnosticConfig::new("ruff")
            .format(DiagnosticFormat::Concise)
            .hide_severity(true);
        let messages = DisplayDiagnostics::new(
            &EmitterContext::new(&FxHashMap::default()),
            &config,
            &diagnostics.inner,
        )
        .to_string();

        insta::with_settings!({
            omit_expression => true,
            filters => vec![
                // The tempdir is always different (and platform dependent)
                (tempdir.path().to_str().unwrap(), "/home/ferris/project"),
            ]
        }, {
            insta::assert_snapshot!(snapshot, messages);
        });
        Ok(())
    }

    #[test]
    fn duplicate_inherited_model_extension_scans_module_from_changed_file() -> Result<()> {
        let tempdir = TempDir::new()?;
        let module = tempdir.path().join("my_module");
        let models = module.join("models");
        fs::create_dir(tempdir.path().join(".git"))?;
        fs::create_dir_all(&models)?;
        fs::write(module.join("__manifest__.py"), "{}\n")?;

        let changed = models.join("partner_a.py");
        fs::write(
            &changed,
            r#"
from odoo import models


class PartnerA(models.Model):
    _inherit = "res.partner"
"#,
        )?;
        fs::write(
            models.join("partner_b.py"),
            r#"
from odoo import models


class PartnerB(models.Model):
    _inherit = "res.partner"
"#,
        )?;
        fs::write(
            models.join("delegated.py"),
            r#"
from odoo import models


class DelegatedPartner(models.Model):
    _name = "delegated.partner"
    _inherit = "res.partner"
"#,
        )?;
        fs::write(
            models.join("suppressed_noqa_a.py"),
            r#"
from odoo import models


class SuppressedNoqaA(models.Model):
    _inherit = "res.users"  # noqa: ODOO063
"#,
        )?;
        fs::write(
            models.join("suppressed_noqa_b.py"),
            r#"
from odoo import models


class SuppressedNoqaB(models.Model):
    _inherit = "res.users"
"#,
        )?;
        fs::write(
            models.join("suppressed_multiline_noqa_a.py"),
            r#"
from odoo import models


class SuppressedMultilineNoqaA(models.Model):
    _inherit = (
        "res.groups"  # noqa: ODOO063
    )
"#,
        )?;
        fs::write(
            models.join("suppressed_multiline_noqa_b.py"),
            r#"
from odoo import models


class SuppressedMultilineNoqaB(models.Model):
    _inherit = "res.groups"
"#,
        )?;
        fs::write(
            models.join("suppressed_pylint_name_a.py"),
            r#"
from odoo import models


class SuppressedPylintNameA(models.Model):
    _inherit = "res.company"  # pylint: disable=consider-merging-classes-inherited
"#,
        )?;
        fs::write(
            models.join("suppressed_pylint_name_b.py"),
            r#"
from odoo import models


class SuppressedPylintNameB(models.Model):
    _inherit = "res.company"
"#,
        )?;
        fs::write(
            models.join("suppressed_pylint_code_a.py"),
            r#"
from odoo import models


class SuppressedPylintCodeA(models.Model):
    _inherit = "res.country"  # pylint: disable=R8180
"#,
        )?;
        fs::write(
            models.join("suppressed_pylint_code_b.py"),
            r#"
from odoo import models


class SuppressedPylintCodeB(models.Model):
    _inherit = "res.country"
"#,
        )?;
        fs::write(
            models.join("ignored_by_per_file.py"),
            r#"
from odoo import models


class IgnoredByPerFile(models.Model):
    _inherit = "res.currency"
"#,
        )?;
        fs::write(
            models.join("per_file_pair.py"),
            r#"
from odoo import models


class PerFilePair(models.Model):
    _inherit = "res.currency"
"#,
        )?;

        let mut linter = LinterSettings::for_rule(Rule::ConsiderMergingClassesInherited);
        linter.per_file_ignores = CompiledPerFileIgnoreList::resolve(
            vec![PerFileIgnore::new(
                "ignored_by_per_file.py".to_string(),
                vec![UnresolvedRuleSelector::cli("ODOO063")],
                None,
            )],
            PreviewMode::Disabled,
        )?;
        let settings = Settings {
            linter,
            ..Settings::default()
        };
        let pyproject_config =
            PyprojectConfig::new(PyprojectDiscoveryStrategy::Fixed, settings, None);

        let diagnostics = check(
            &[changed],
            &pyproject_config,
            &ConfigArguments::default(),
            flags::Cache::Disabled,
            flags::Noqa::Enabled,
            flags::FixMode::Generate,
            UnsafeFixes::Enabled,
        )?;

        let config = DisplayDiagnosticConfig::new("ruff")
            .format(DiagnosticFormat::Concise)
            .hide_severity(true);
        let messages = DisplayDiagnostics::new(
            &EmitterContext::new(&FxHashMap::default()),
            &config,
            &diagnostics.inner,
        )
        .to_string();

        insta::with_settings!({
            omit_expression => true,
            filters => vec![
                (tempdir.path().to_str().unwrap(), "/home/ferris/project"),
            ]
        }, {
            insta::assert_snapshot!("ODOO063_project_scan", messages);
        });

        Ok(())
    }

    /// `_inherit` may be a list/tuple of model names (a class extending several models at
    /// once), not just a single string. A class extending several models is not a duplicate
    /// of one that only extends *one* of them — the full sets differ, so nothing should
    /// merge cleanly.
    #[test]
    fn duplicate_inherited_model_extension_list_and_string_with_different_models_not_flagged()
    -> Result<()> {
        let tempdir = TempDir::new()?;
        let module = tempdir.path().join("my_module");
        let models = module.join("models");
        fs::create_dir_all(&models)?;
        fs::write(module.join("__manifest__.py"), "{}\n")?;

        let changed = models.join("mixin_a.py");
        fs::write(
            &changed,
            r#"
from odoo import models


class MixinA(models.Model):
    _inherit = ["res.partner", "mail.thread"]
"#,
        )?;
        fs::write(
            models.join("mixin_b.py"),
            r#"
from odoo import models


class MixinB(models.Model):
    _inherit = "res.partner"
"#,
        )?;

        let settings = Settings {
            linter: LinterSettings::for_rule(Rule::ConsiderMergingClassesInherited),
            ..Settings::default()
        };
        let pyproject_config =
            PyprojectConfig::new(PyprojectDiscoveryStrategy::Fixed, settings, None);

        let diagnostics = check(
            &[changed],
            &pyproject_config,
            &ConfigArguments::default(),
            flags::Cache::Disabled,
            flags::Noqa::Enabled,
            flags::FixMode::Generate,
            UnsafeFixes::Enabled,
        )?;

        assert!(
            diagnostics.inner.is_empty(),
            "a list _inherit extending several models should not be flagged as a duplicate \
             of a string _inherit extending only one of them: {:?}",
            diagnostics.inner,
        );

        Ok(())
    }

    /// Two classes with the same full set of `_inherit` models — regardless of whether it's
    /// declared as a list, a tuple, or in a different order — are still a genuine duplicate.
    #[test]
    fn duplicate_inherited_model_extension_detects_list_form_inherit_regardless_of_order()
    -> Result<()> {
        let tempdir = TempDir::new()?;
        let module = tempdir.path().join("my_module");
        let models = module.join("models");
        fs::create_dir_all(&models)?;
        fs::write(module.join("__manifest__.py"), "{}\n")?;

        let changed = models.join("mixin_a.py");
        fs::write(
            &changed,
            r#"
from odoo import models


class MixinA(models.Model):
    _inherit = ["res.partner", "mail.thread"]
"#,
        )?;
        fs::write(
            models.join("mixin_b.py"),
            r#"
from odoo import models


class MixinB(models.Model):
    _inherit = ("mail.thread", "res.partner")
"#,
        )?;

        let settings = Settings {
            linter: LinterSettings::for_rule(Rule::ConsiderMergingClassesInherited),
            ..Settings::default()
        };
        let pyproject_config =
            PyprojectConfig::new(PyprojectDiscoveryStrategy::Fixed, settings, None);

        let diagnostics = check(
            &[changed],
            &pyproject_config,
            &ConfigArguments::default(),
            flags::Cache::Disabled,
            flags::Noqa::Enabled,
            flags::FixMode::Generate,
            UnsafeFixes::Enabled,
        )?;

        let config = DisplayDiagnosticConfig::new("ruff")
            .format(DiagnosticFormat::Concise)
            .hide_severity(true);
        let messages = DisplayDiagnostics::new(
            &EmitterContext::new(&FxHashMap::default()),
            &config,
            &diagnostics.inner,
        )
        .to_string();

        insta::with_settings!({
            omit_expression => true,
            filters => vec![
                (tempdir.path().to_str().unwrap(), "/home/ferris/project"),
            ]
        }, {
            insta::assert_snapshot!("ODOO063_list_inherit", messages);
        });

        Ok(())
    }

    /// Files matched by the project's `exclude`/`extend-exclude` configuration must not be
    /// scanned for cross-file duplicates, even though they live inside the same Odoo module as
    /// the file being linted.
    #[test]
    fn duplicate_inherited_model_extension_respects_exclude_config() -> Result<()> {
        let tempdir = TempDir::new()?;
        let module = tempdir.path().join("my_module");
        let models = module.join("models");
        fs::create_dir_all(&models)?;
        fs::write(module.join("__manifest__.py"), "{}\n")?;

        let changed = models.join("partner_a.py");
        fs::write(
            &changed,
            r#"
from odoo import models


class PartnerA(models.Model):
    _inherit = "res.partner"
"#,
        )?;
        fs::write(
            models.join("vendor_partner_b.py"),
            r#"
from odoo import models


class VendoredPartnerB(models.Model):
    _inherit = "res.partner"
"#,
        )?;

        let mut linter = LinterSettings::for_rule(Rule::ConsiderMergingClassesInherited);
        linter.exclude = FilePatternSet::try_from_iter([FilePattern::Config(
            "vendor_partner_b.py".to_string(),
        )])?;
        let settings = Settings {
            linter,
            ..Settings::default()
        };
        let pyproject_config =
            PyprojectConfig::new(PyprojectDiscoveryStrategy::Fixed, settings, None);

        let diagnostics = check(
            &[changed],
            &pyproject_config,
            &ConfigArguments::default(),
            flags::Cache::Disabled,
            flags::Noqa::Enabled,
            flags::FixMode::Generate,
            UnsafeFixes::Enabled,
        )?;

        assert!(
            diagnostics.inner.is_empty(),
            "excluded file should not participate in the duplicate-inherit scan: {:?}",
            diagnostics.inner,
        );

        Ok(())
    }

    /// Files ignored by `.gitignore` should not be scanned for cross-file duplicates.
    #[test]
    fn duplicate_inherited_model_extension_respects_gitignore() -> Result<()> {
        let tempdir = TempDir::new()?;
        let module = tempdir.path().join("my_module");
        let models = module.join("models");
        fs::create_dir_all(&models)?;
        fs::write(module.join("__manifest__.py"), "{}\n")?;
        fs::write(
            module.join(".gitignore"),
            "models/ignored_by_gitignore.py\n",
        )?;

        let changed = models.join("partner_a.py");
        fs::write(
            &changed,
            r#"
from odoo import models


class PartnerA(models.Model):
    _inherit = "res.partner"
"#,
        )?;
        fs::write(
            models.join("ignored_by_gitignore.py"),
            r#"
from odoo import models


class IgnoredByGitignore(models.Model):
    _inherit = "res.partner"
"#,
        )?;

        let settings = Settings {
            linter: LinterSettings::for_rule(Rule::ConsiderMergingClassesInherited),
            ..Settings::default()
        };
        let pyproject_config =
            PyprojectConfig::new(PyprojectDiscoveryStrategy::Fixed, settings, None);

        let diagnostics = check(
            &[changed],
            &pyproject_config,
            &ConfigArguments::default(),
            flags::Cache::Disabled,
            flags::Noqa::Enabled,
            flags::FixMode::Generate,
            UnsafeFixes::Enabled,
        )?;

        assert!(
            diagnostics.inner.is_empty(),
            ".gitignored file should not participate in the duplicate-inherit scan: {:?}",
            diagnostics.inner,
        );

        Ok(())
    }

    /// If the file being linted has no part in a duplicate-`_inherit` group elsewhere in the
    /// module, no diagnostic should be emitted for that group: it should only surface once one
    /// of its own members is actually checked.
    #[test]
    fn duplicate_inherited_model_extension_skips_group_without_checked_file() -> Result<()> {
        let tempdir = TempDir::new()?;
        let module = tempdir.path().join("my_module");
        let models = module.join("models");
        fs::create_dir_all(&models)?;
        fs::write(module.join("__manifest__.py"), "{}\n")?;

        let changed = models.join("unrelated.py");
        fs::write(
            &changed,
            r#"
from odoo import models


class Unrelated(models.Model):
    _inherit = "res.currency"
"#,
        )?;
        fs::write(
            models.join("partner_a.py"),
            r#"
from odoo import models


class PartnerA(models.Model):
    _inherit = "res.partner"
"#,
        )?;
        fs::write(
            models.join("partner_b.py"),
            r#"
from odoo import models


class PartnerB(models.Model):
    _inherit = "res.partner"
"#,
        )?;

        let settings = Settings {
            linter: LinterSettings::for_rule(Rule::ConsiderMergingClassesInherited),
            ..Settings::default()
        };
        let pyproject_config =
            PyprojectConfig::new(PyprojectDiscoveryStrategy::Fixed, settings, None);

        let diagnostics = check(
            &[changed],
            &pyproject_config,
            &ConfigArguments::default(),
            flags::Cache::Disabled,
            flags::Noqa::Enabled,
            flags::FixMode::Generate,
            UnsafeFixes::Enabled,
        )?;

        assert!(
            diagnostics.inner.is_empty(),
            "should not report a duplicate group that doesn't involve any checked file: {:?}",
            diagnostics.inner,
        );

        Ok(())
    }

    /// When none of the checked files define an unsuppressed `_inherit`, the module-level pass
    /// should stop before walking and parsing every Python file in the containing Odoo module.
    #[test]
    fn duplicate_inherited_model_extension_skips_module_scan_without_target_models() -> Result<()> {
        let tempdir = TempDir::new()?;
        let module = tempdir.path().join("my_module");
        let models = module.join("models");
        fs::create_dir_all(&models)?;
        fs::write(module.join("__manifest__.py"), "{}\n")?;

        let changed = models.join("unrelated.py");
        fs::write(&changed, "VALUE = 1\n")?;
        fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o000)
            .open(models.join("unreadable.py"))?;

        let settings = Settings {
            linter: LinterSettings::for_rule(Rule::ConsiderMergingClassesInherited),
            ..Settings::default()
        };
        let pyproject_config =
            PyprojectConfig::new(PyprojectDiscoveryStrategy::Fixed, settings, None);

        let diagnostics = check(
            &[changed],
            &pyproject_config,
            &ConfigArguments::default(),
            flags::Cache::Disabled,
            flags::Noqa::Enabled,
            flags::FixMode::Generate,
            UnsafeFixes::Enabled,
        )?;

        assert!(
            diagnostics.inner.is_empty(),
            "files with no `_inherit` should not trigger a module scan: {:?}",
            diagnostics.inner,
        );

        Ok(())
    }
}
