use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use ignore::{DirEntry, WalkBuilder};
use itertools::Itertools;
use ruff_db::diagnostic::{
    Annotation, Diagnostic, DiagnosticId, LintName, SecondaryCode, Severity, Span,
};
use ruff_linter::registry::Rule;
use ruff_linter::settings::flags;
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_parser::parse_module;
use ruff_source_file::SourceFileBuilder;
use ruff_text_size::TextRange;
use ruff_workspace::resolver::{Resolver, match_exclusion};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::diagnostics::Diagnostics;

const MANIFEST_FILES: [&str; 2] = ["__manifest__.py", "__openerp__.py"];

#[derive(Debug, Clone)]
struct InheritLocation {
    path: PathBuf,
    range: TextRange,
    source: String,
}

/// Check for pylint-odoo's cross-file `consider-merging-classes-inherited` pattern.
pub(crate) fn check_duplicate_inherited_model_extensions(
    paths: &[PathBuf],
    resolver: &Resolver<'_>,
    noqa: flags::Noqa,
) -> Result<Diagnostics> {
    let mut checked_paths_by_module: FxHashMap<PathBuf, FxHashSet<PathBuf>> = FxHashMap::default();

    for path in paths {
        if !resolver
            .resolve(path)
            .linter
            .rules
            .enabled(Rule::DuplicateInheritedModelExtension)
        {
            continue;
        }
        if let Some(root) = odoo_module_root(path) {
            checked_paths_by_module
                .entry(root)
                .or_default()
                .insert(path.clone());
        }
    }

    if checked_paths_by_module.is_empty() {
        return Ok(Diagnostics::default());
    }

    let mut diagnostics = Vec::new();
    for (root, checked_paths) in checked_paths_by_module
        .into_iter()
        .sorted_by(|left, right| left.0.cmp(&right.0))
    {
        diagnostics.extend(check_module(&root, &checked_paths, resolver, noqa)?);
    }

    Ok(Diagnostics::new(diagnostics, FxHashMap::default()))
}

fn check_module(
    root: &Path,
    checked_paths: &FxHashSet<PathBuf>,
    resolver: &Resolver<'_>,
    noqa: flags::Noqa,
) -> Result<Vec<Diagnostic>> {
    let target_models = checked_inherited_models(checked_paths, resolver, noqa)?;
    if target_models.is_empty() {
        return Ok(Vec::new());
    }

    let mut inherited: FxHashMap<String, Vec<InheritLocation>> = FxHashMap::default();

    let mut builder = WalkBuilder::new(root);
    if let Ok(cwd) = std::env::current_dir() {
        builder.current_dir(cwd);
    }
    builder.standard_filters(resolver.resolve(root).file_resolver.respect_gitignore);
    builder.require_git(false);
    builder.hidden(false);
    builder.filter_entry(include_entry);

    for entry in builder.build().filter_map(Result::ok) {
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "py") {
            continue;
        }
        if is_excluded(path, resolver) {
            continue;
        }
        collect_selected_inherits(path, &target_models, &mut inherited)?;
    }

    let mut inherited = inherited.into_iter().collect::<Vec<_>>();
    inherited.sort_by(|(left, _), (right, _)| left.cmp(right));

    let mut diagnostics = Vec::new();
    for (model, mut locations) in inherited {
        locations.retain(|location| !location_suppressed(location, resolver, noqa));
        if locations.len() <= 1 {
            continue;
        }

        locations.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.range.start().cmp(&right.range.start()))
        });

        // Only report a group if it involves one of the files this invocation was actually
        // asked to check; otherwise every lint of any file in the module would resurface
        // duplicate groups made up entirely of files outside the current run's scope.
        let Some(primary_index) = locations
            .iter()
            .position(|location| checked_paths.contains(&location.path))
        else {
            continue;
        };
        let primary = locations.remove(primary_index);

        let locations = locations
            .iter()
            .map(format_location)
            .collect::<Vec<_>>()
            .join(", ");
        diagnostics.push(create_diagnostic(primary, &model, &locations));
    }

    Ok(diagnostics)
}

fn checked_inherited_models(
    checked_paths: &FxHashSet<PathBuf>,
    resolver: &Resolver<'_>,
    noqa: flags::Noqa,
) -> Result<FxHashSet<String>> {
    let mut inherited: FxHashMap<String, Vec<InheritLocation>> = FxHashMap::default();
    for path in checked_paths.iter().sorted() {
        collect_inherits(path, &mut inherited)?;
    }
    Ok(inherited
        .into_iter()
        .filter(|(_, locations)| {
            locations
                .iter()
                .any(|location| !location_suppressed(location, resolver, noqa))
        })
        .map(|(model, _)| model)
        .collect())
}

fn collect_inherits(
    path: &Path,
    inherited: &mut FxHashMap<String, Vec<InheritLocation>>,
) -> Result<()> {
    let target_models = FxHashSet::default();
    collect_selected_inherits(path, &target_models, inherited)
}

fn collect_selected_inherits(
    path: &Path,
    target_models: &FxHashSet<String>,
    inherited: &mut FxHashMap<String, Vec<InheritLocation>>,
) -> Result<()> {
    let bytes = fs::read(path)?;
    if !bytes
        .windows(b"_inherit".len())
        .any(|window| window == b"_inherit")
    {
        return Ok(());
    }
    let Ok(source) = std::str::from_utf8(&bytes) else {
        return Ok(());
    };
    let Ok(parsed) = parse_module(source) else {
        return Ok(());
    };

    collect_inherits_from_suite(path, source, parsed.suite(), target_models, inherited);
    Ok(())
}

fn collect_inherits_from_suite(
    path: &Path,
    source: &str,
    suite: &[Stmt],
    target_models: &FxHashSet<String>,
    inherited: &mut FxHashMap<String, Vec<InheritLocation>>,
) {
    for stmt in suite {
        match stmt {
            Stmt::ClassDef(class_def) => {
                for (model, range) in inherited_models(class_def) {
                    if !target_models.is_empty() && !target_models.contains(&model) {
                        continue;
                    }
                    inherited.entry(model).or_default().push(InheritLocation {
                        path: path.to_path_buf(),
                        range,
                        source: source.to_string(),
                    });
                }
                collect_inherits_from_suite(
                    path,
                    source,
                    &class_def.body,
                    target_models,
                    inherited,
                );
            }
            Stmt::FunctionDef(function_def) => {
                collect_inherits_from_suite(
                    path,
                    source,
                    &function_def.body,
                    target_models,
                    inherited,
                );
            }
            _ => {}
        }
    }
}

fn inherited_models(class_def: &ast::StmtClassDef) -> Vec<(String, TextRange)> {
    let mut models = Vec::new();
    let mut has_name = false;

    for stmt in &class_def.body {
        let Stmt::Assign(ast::StmtAssign {
            targets,
            value,
            range,
            ..
        }) = stmt
        else {
            continue;
        };
        let values = string_literals(value);
        if values.is_empty() {
            continue;
        }
        for target in targets {
            let Expr::Name(name) = target else {
                continue;
            };
            match name.id.as_str() {
                "_inherit" => {
                    models = values.iter().map(|value| (value.clone(), *range)).collect();
                }
                "_name" => has_name = true,
                _ => {}
            }
        }
    }

    if has_name { Vec::new() } else { models }
}

/// Extract the model names assigned to `_inherit`, which pylint-odoo's `_inherit` convention
/// allows to be either a single string (`_inherit = "res.partner"`) or a list/tuple of strings
/// (`_inherit = ["res.partner", "mail.thread"]`) when a class extends several models at once.
fn string_literals(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => {
            vec![value.to_str().to_string()]
        }
        Expr::List(ast::ExprList { elts, .. }) | Expr::Tuple(ast::ExprTuple { elts, .. }) => elts
            .iter()
            .filter_map(|elt| match elt {
                Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => {
                    Some(value.to_str().to_string())
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn create_diagnostic(primary: InheritLocation, model: &str, locations: &str) -> Diagnostic {
    let rule = Rule::DuplicateInheritedModelExtension;
    let source_file =
        SourceFileBuilder::new(primary.path.to_string_lossy(), primary.source).finish();
    let mut diagnostic = Diagnostic::new(
        DiagnosticId::Lint(LintName::of(rule.into())),
        Severity::Error,
        format!("Consider merging classes inherited to \"{model}\" from {locations}."),
    );
    diagnostic.annotate(Annotation::primary(
        Span::from(source_file).with_range(primary.range),
    ));
    diagnostic.set_secondary_code(SecondaryCode::new(rule.noqa_code().to_string()));
    diagnostic.set_documentation_url(rule.url());
    diagnostic
}

fn format_location(location: &InheritLocation) -> String {
    let source_file =
        SourceFileBuilder::new(location.path.to_string_lossy(), location.source.as_str()).finish();
    let source_code = source_file.to_source_code();
    let line_column = source_code.line_column(location.range.start());
    format!(
        "{}:{line_column}",
        ruff_linter::fs::relativize_path(&location.path)
    )
}

fn location_suppressed(
    location: &InheritLocation,
    resolver: &Resolver<'_>,
    noqa: flags::Noqa,
) -> bool {
    let rule = Rule::DuplicateInheritedModelExtension;
    let settings = resolver.resolve(&location.path);
    if !settings.linter.rules.enabled(rule)
        || ruff_linter::fs::ignores_from_path(&location.path, &settings.linter.per_file_ignores)
            .contains(rule)
    {
        return true;
    }

    (noqa.is_enabled()
        && (source_has_file_noqa(&location.source)
            || line_has_noqa(&location.source, location.range)))
        || line_has_pylint_disable(&location.source, location.range)
}

fn source_has_file_noqa(source: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim_start();
        (line.starts_with("# ruff: noqa") || line.starts_with("# flake8: noqa"))
            && noqa_comment_suppresses_rule(line)
    })
}

fn line_has_noqa(source: &str, range: TextRange) -> bool {
    lines_for_range(source, range).any(|line| {
        let Some((_, comment)) = line.split_once('#') else {
            return false;
        };
        noqa_comment_suppresses_rule(comment)
    })
}

fn noqa_comment_suppresses_rule(comment: &str) -> bool {
    let comment_lowercase = comment.to_ascii_lowercase();
    let Some(noqa_start) = comment_lowercase.find("noqa") else {
        return false;
    };
    let rest = comment[noqa_start + "noqa".len()..].trim_start();
    if !rest.starts_with(':') {
        return true;
    }
    rest[1..]
        .split(|char: char| char == ',' || char.is_ascii_whitespace())
        .any(|code| code.eq_ignore_ascii_case("ODOO063"))
}

fn line_has_pylint_disable(source: &str, range: TextRange) -> bool {
    lines_for_range(source, range).any(|line| {
        let Some((_, comment)) = line.split_once('#') else {
            return false;
        };
        let comment = comment.to_ascii_lowercase();
        comment.contains("pylint:")
            && comment.contains("disable")
            && (comment.contains("consider-merging-classes-inherited") || comment.contains("r8180"))
    })
}

fn lines_for_range(source: &str, range: TextRange) -> impl Iterator<Item = &str> {
    let start = usize::from(range.start());
    let end = usize::from(range.end());
    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[end..]
        .find('\n')
        .map_or(source.len(), |index| end + index);
    source[line_start..line_end].lines()
}

fn odoo_module_root(path: &Path) -> Option<PathBuf> {
    let mut current = path.parent()?;
    loop {
        if MANIFEST_FILES
            .iter()
            .any(|manifest| current.join(manifest).is_file())
        {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

/// Return `true` if `path` matches the resolved `exclude`/`extend-exclude` configuration for
/// its directory. These files were discovered by walking the module rather than requested on
/// the command line, so exclusion always applies (mirroring how ruff's own file discovery
/// treats non-root files).
fn is_excluded(path: &Path, resolver: &Resolver<'_>) -> bool {
    let settings = resolver.resolve(path);
    let file_name = path.file_name().unwrap_or(path.as_os_str());
    match_exclusion(path, file_name, &settings.linter.exclude)
}

fn include_entry(entry: &DirEntry) -> bool {
    !matches!(
        entry.file_name().to_str(),
        Some(".git" | ".hg" | ".mypy_cache" | ".ruff_cache" | ".tox" | ".venv" | "__pycache__")
    )
}
