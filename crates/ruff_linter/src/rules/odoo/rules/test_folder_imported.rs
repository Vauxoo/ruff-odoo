use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Stmt};
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

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
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.2")]
pub(crate) struct TestFolderImported;

impl Violation for TestFolderImported {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Test folder imported in module".to_string()
    }
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
        }) => module.split('.').next() == Some("tests"),
        // `from . import tests`
        Stmt::ImportFrom(ast::StmtImportFrom {
            module: None,
            names,
            ..
        }) => names.iter().any(|alias| alias.name.as_str() == "tests"),
        // `import tests`
        Stmt::Import(ast::StmtImport { names, .. }) => names
            .iter()
            .any(|alias| alias.name.split('.').next() == Some("tests")),
        _ => false,
    };
    if imports_tests {
        checker.report_diagnostic(TestFolderImported, stmt.range());
    }
}
