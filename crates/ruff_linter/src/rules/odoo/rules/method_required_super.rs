use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::helpers::any_over_body;
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::ScopeKind;
use ruff_text_size::Ranged;

use crate::Violation;
use crate::checkers::ast::Checker;

/// ## What it does
/// Checks that common Odoo ORM methods (`create`, `write`, `unlink`, `init`, ...) call
/// `super()` somewhere in their body.
///
/// ## Why is this bad?
/// Overriding one of these methods without calling `super()` usually means the base
/// implementation (and any other module's override in the resolution order) never
/// runs, silently breaking the inheritance chain.
///
/// `init` is reported only on a class that carries `_inherit` or `_inherits`, because that
/// is where the model being extended already has an `init` of its own building indexes and
/// SQL constraints. `sale.order.line` inherits [`analytic.mixin`][analytic-mixin], whose
/// `init` creates the `sale_order_line_analytic_distribution_accounts_gin_index` GIN index,
/// so a module overriding `init` on `sale.order.line` without chaining silently drops it. A
/// class declaring only `_name` defines a brand-new model, where `models.Model.init` does
/// nothing and the call is not required.
///
/// ## Example
/// ```python
/// class SaleOrderLine(models.Model):
///     _inherit = "sale.order.line"
///
///     def init(self):
///         self.env.cr.execute("CREATE INDEX ...")
/// ```
///
/// Use instead:
/// ```python
/// class SaleOrderLine(models.Model):
///     _inherit = "sale.order.line"
///
///     def init(self):
///         # analytic.mixin builds its analytic_distribution gin index in init() too;
///         # without the super() call it never runs for sale_order_line.
///         super().init()
///         self.env.cr.execute("CREATE INDEX ...")
/// ```
///
/// ## Options
/// - `lint.odoo.method-required-super`
///
/// The default is the ORM and test methods whose override must chain.
///
/// ## References
/// - [`analytic.mixin.init`][analytic-mixin] — creates the `analytic_distribution` GIN index,
///   and chains to `super().init()` itself.
/// - [`sale.order.line`][sale-order-line] — its `_inherit = ['analytic.mixin']` is what puts
///   that `init` in the resolution order of every module extending the model.
///
/// [analytic-mixin]: https://github.com/odoo/odoo/blob/42caf937f8e5f90a118ac0e4838d82df61448446/addons/analytic/models/analytic_mixin.py#L32-L40
/// [sale-order-line]: https://github.com/odoo/odoo/blob/a12f48792482f5ea3d51ca86b6e32d8985fe6afb/addons/sale/models/sale_order_line.py#L12-L14
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.1")]
pub(crate) struct MethodRequiredSuper {
    name: String,
}

impl Violation for MethodRequiredSuper {
    #[derive_message_formats]
    fn message(&self) -> String {
        let MethodRequiredSuper { name } = self;
        format!("Missing `super` call in \"{name}\" method")
    }
}

const METHODS_REQUIRING_SUPER: &[&str] = &[
    "copy",
    "create",
    "default_get",
    "init",
    "read",
    "setUp",
    "setUpClass",
    "tearDown",
    "tearDownClass",
    "unlink",
    "write",
];

/// Methods only worth reporting when the class extends an existing model: their base
/// implementation on a freshly declared model is a no-op, so the missing `super()` call
/// costs nothing there.
const METHODS_REQUIRING_INHERIT: &[&str] = &["init"];

/// ODW8106
pub(crate) fn method_required_super(checker: &Checker, function_def: &ast::StmtFunctionDef) {
    let ScopeKind::Class(class_def) = checker.semantic().current_scope().kind else {
        return;
    };
    let name = function_def.name.as_str();
    if !checker
        .settings()
        .odoo
        .method_required_super
        .contains(name, METHODS_REQUIRING_SUPER)
    {
        return;
    }
    if METHODS_REQUIRING_INHERIT.contains(&name) && !class_extends_model(class_def) {
        return;
    }

    let calls_super = any_over_body(&function_def.body, |expr| {
        matches!(
            expr,
            Expr::Call(ast::ExprCall { func, .. })
                if matches!(func.as_ref(), Expr::Name(name) if name.id == "super")
        )
    });
    if !calls_super {
        checker.report_diagnostic(
            MethodRequiredSuper {
                name: function_def.name.to_string(),
            },
            function_def.name.range(),
        );
    }
}

/// Returns `true` if the class body assigns `_inherit` or `_inherits`, i.e. it extends a
/// model declared elsewhere rather than declaring one of its own.
fn class_extends_model(class_def: &ast::StmtClassDef) -> bool {
    class_def.body.iter().any(|stmt| match stmt {
        ast::Stmt::Assign(assign) => assign.targets.iter().any(is_inherit_target),
        ast::Stmt::AnnAssign(assign) => is_inherit_target(&assign.target),
        _ => false,
    })
}

fn is_inherit_target(target: &Expr) -> bool {
    matches!(target, Expr::Name(name) if name.id == "_inherit" || name.id == "_inherits")
}
