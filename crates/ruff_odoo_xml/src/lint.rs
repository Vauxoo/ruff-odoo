//! Linting the Python code of an XML data file.

use std::path::Path;

use ruff_db::diagnostic::{Annotation, Diagnostic, Span};
use ruff_linter::linter::{ParseSource, lint_only};
use ruff_linter::package::PackageRoot;
use ruff_linter::registry::Rule;
use ruff_linter::rules::odoo::settings::CodeFieldContext;
use ruff_linter::settings::{LinterSettings, flags};
use ruff_linter::source_kind::SourceKind;
use ruff_python_ast::PySourceType;
use ruff_source_file::{SourceFile, SourceFileBuilder};

use crate::{CodeField, eval_context_names, extract_code_fields};

/// The rules that judge the file a code field lives in rather than the code itself, and therefore
/// say nothing useful about a field.
///
/// - The module-level ones expect a Python module: the field has no module docstring
///   (`D100`), no license header (`CPY001`) and no required imports (`I002`; `safe_eval` refuses
///   imports anyway).
/// - The path-based ones would judge the XML file's name, directory and permissions: `INP001`,
///   `N999`, `A005`, `EXE001` and `EXE002`.
/// - `ODC8501` would read a comment on the field's first line as the header of a file.
const FILE_LEVEL_RULES: &[Rule] = &[
    Rule::HeaderComments,
    Rule::UndocumentedPublicModule,
    Rule::MissingCopyrightNotice,
    Rule::MissingRequiredImport,
    Rule::ImplicitNamespacePackage,
    Rule::InvalidModuleName,
    Rule::StdlibModuleShadowing,
    Rule::ShebangNotExecutable,
    Rule::ShebangMissingExecutableFile,
];

/// Lints the Python body of every `ir.cron` and `ir.actions.server` record in `contents`, and
/// reports each diagnostic at the place in the XML document its code came from.
///
/// Each field is linted on its own, as the module Odoo compiles it into, with the names Odoo
/// injects into its evaluation context in the configured `odoo-version` (and, for an AI tool,
/// the tool's arguments) added to the
/// builtins, and the rules that judge the file rather than the code turned off. The `OD` rules
/// are told they are in a code field, and which model its record's `model_id` names, so that
/// they read `env`, `model`, `record` and `records` the way they read `self.env` and `self` in
/// a model. Fixes are dropped: rewriting the XML document is left to the formatter.
pub fn lint_code_fields(
    path: &Path,
    package: Option<PackageRoot<'_>>,
    contents: &str,
    settings: &LinterSettings,
    noqa: flags::Noqa,
) -> Vec<Diagnostic> {
    let fields = extract_code_fields(contents);
    if fields.is_empty() {
        return Vec::new();
    }

    let settings = code_field_settings(settings);
    let file = SourceFileBuilder::new(path.to_string_lossy(), contents).finish();

    let mut diagnostics = Vec::new();
    for field in &fields {
        let mut settings = with_builtins(&settings, &field.names);
        settings.odoo.code_field = Some(CodeFieldContext {
            model_xmlid: field.model_xmlid.clone(),
        });
        let settings = &settings;
        let source_kind = SourceKind::Python {
            code: field.code.clone(),
            is_stub: false,
        };
        let result = lint_only(
            path,
            package,
            settings,
            noqa,
            &source_kind,
            PySourceType::Python,
            ParseSource::None,
        );
        diagnostics.extend(
            result
                .diagnostics
                .into_iter()
                .map(|diagnostic| relocate(diagnostic, field, &file)),
        );
    }
    diagnostics
}

fn code_field_settings(settings: &LinterSettings) -> LinterSettings {
    let mut settings = with_builtins(settings, eval_context_names(settings.odoo.odoo_version));
    for rule in FILE_LEVEL_RULES {
        settings.rules.disable(*rule);
    }
    settings
}

fn with_builtins(settings: &LinterSettings, names: &[impl ToString]) -> LinterSettings {
    let mut settings = settings.clone();
    settings
        .builtins
        .extend(names.iter().map(ToString::to_string));
    settings
}

/// Moves a diagnostic computed on a field's code to the XML document the code came from.
fn relocate(mut diagnostic: Diagnostic, field: &CodeField, file: &SourceFile) -> Diagnostic {
    let relocate_annotation = |annotation: &mut Annotation| {
        let range = annotation
            .get_span()
            .range()
            .map(|range| field.to_xml_range(range));
        annotation.set_span(Span::from(file.clone()).with_optional_range(range));
    };

    diagnostic.annotations_mut().for_each(relocate_annotation);
    for sub in diagnostic.sub_diagnostics_mut() {
        sub.annotations_mut().for_each(relocate_annotation);
    }
    if let Some(parent) = diagnostic.parent() {
        diagnostic.set_parent(field.to_xml_offset(parent));
    }
    // The linter moves the offset of a diagnostic inside a multi-line string to the end of the
    // string, where its `noqa` comment has to go, but `ruff_db` keeps that offset private, so the
    // start of the relocated range is the closest available.
    if let Some(range) = diagnostic.range() {
        diagnostic.set_noqa_offset(range.start());
    }
    diagnostic.remove_fix();
    diagnostic
}
