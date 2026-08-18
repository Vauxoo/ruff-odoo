use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Stmt};
use ruff_text_size::Ranged;

use crate::checkers::ast::Checker;
use crate::fix;
use crate::{Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for imports of the `tests` folder in a module's `__init__.py`.
///
/// ## Why is this bad?
/// Odoo discovers and loads tests on its own when running with `--test-enable`; importing
/// the `tests` package from `__init__.py` loads test code (and its extra dependencies) in
/// production too.
///
/// ## Example
/// ```python
/// from . import models, tests
/// ```
///
/// Use instead:
/// ```python
/// from . import models
/// ```
///
/// ## Fix safety
/// The fix removes the import — just the `tests` name when the statement imports other
/// names too, the whole statement otherwise. It is marked as unsafe because a name the
/// import bound may still be referenced elsewhere in the file, and because dropping the
/// import also drops any side effects of loading the `tests` package.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct TestFolderImported;

impl Violation for TestFolderImported {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Test folder imported in module".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Remove the `tests` import".to_string())
    }
}

/// How the statement pulls the `tests` package in, and therefore what the fix removes.
enum TestsImport<'a> {
    /// Every name comes out of `tests` (`from .tests import common`): the whole statement
    /// goes.
    WholeStatement,
    /// One alias among possibly several (`from . import models, tests`, `import tests.foo`):
    /// only that alias goes, spelled the way the statement writes it.
    Alias(&'a str),
}

/// ODE8130
pub(crate) fn test_folder_imported(checker: &Checker, stmt: &Stmt, path: &Path) {
    if path.file_name().and_then(|name| name.to_str()) != Some("__init__.py") {
        return;
    }

    let imports_tests = match stmt {
        // `from .tests import ...` (module is "tests" or "tests.something")
        Stmt::ImportFrom(ast::StmtImportFrom {
            module: Some(module),
            ..
        }) => (module.split('.').next() == Some("tests")).then_some(TestsImport::WholeStatement),
        // `from . import tests`
        Stmt::ImportFrom(ast::StmtImportFrom {
            module: None,
            names,
            ..
        }) => names
            .iter()
            .find(|alias| alias.name.as_str() == "tests")
            .map(|alias| TestsImport::Alias(alias.name.as_str())),
        // `import tests`
        Stmt::Import(ast::StmtImport { names, .. }) => names
            .iter()
            .find(|alias| alias.name.split('.').next() == Some("tests"))
            .map(|alias| TestsImport::Alias(alias.name.as_str())),
        _ => None,
    };
    let Some(imports_tests) = imports_tests else {
        return;
    };

    let mut diagnostic = checker.report_diagnostic(TestFolderImported, stmt.range());
    diagnostic.try_set_fix(|| {
        let parent = checker.semantic().current_statement_parent();
        let edit = match imports_tests {
            TestsImport::WholeStatement => {
                fix::edits::delete_stmt(stmt, parent, checker.locator(), checker.indexer())
            }
            TestsImport::Alias(member) => fix::edits::remove_unused_imports(
                std::iter::once(member),
                stmt,
                parent,
                checker.locator(),
                checker.stylist(),
                checker.indexer(),
            )?,
        };
        Ok(Fix::unsafe_edit(edit).isolate(Checker::isolation(
            checker.semantic().current_statement_parent_id(),
        )))
    });
}
