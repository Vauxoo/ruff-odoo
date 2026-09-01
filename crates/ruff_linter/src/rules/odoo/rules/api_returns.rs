use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_semantic::ScopeKind;
use ruff_source_file::LineRanges;
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_odoo_model_class, odoo_version_applies};
use crate::rules::odoo::settings::OdooVersion;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for an override of an Odoo ORM method that the framework decorates with
/// `@api.returns`, where the override does not repeat the decorator.
///
/// ## Why is this bad?
/// `@api.returns(model, downgrade)` is what turns a recordset into an id on the way out of an
/// RPC call. Odoo's own docstring says the decorator is "automatically inherited", and it is —
/// but only along real Python bases, and `_inherit` is not one.
///
/// `Meta.__new__` (`odoo/api.py`) builds `parent = type.__new__(meta, name, bases, {})` from the
/// class's **Python** bases and copies `_returns` off it. For `class X(models.Model): _inherit =
/// "sale.order"` that parent is `BaseModel`, so an override of a method `BaseModel` itself
/// defines — `search`, `copy`, `create`, `exists` — does inherit the decorator and needs
/// nothing. But for a method defined on a model or an `AbstractModel`, such as
/// `mail.thread.message_post`, the Python base is only `models.Model`, which does not define it,
/// so there is nothing to copy and `_returns` is lost. The registry never repairs it either:
/// `_build_model` calls `type(name, (cls,), attrs)` with no methods in `attrs`, then assigns
/// `__bases__` directly, which does not re-run the metaclass.
///
/// The override then wins the MRO without a `_returns`, and the RPC layer stops downgrading:
/// a JSON-RPC client that used to receive `1624` receives the recordset's repr instead.
///
/// This rule only reports methods that are RPC-callable and are declared outside
/// `odoo/models.py`. Private methods are left alone — they cannot be reached over RPC, so the
/// downgrade never runs and the result never changes.
///
/// ## Example
/// ```python
/// class MailThread(models.AbstractModel):
///     _inherit = "mail.thread"
///
///     def message_post(self, **kwargs):
///         return super().message_post(**kwargs)
/// ```
///
/// Use instead:
/// ```python
/// class MailThread(models.AbstractModel):
///     _inherit = "mail.thread"
///
///     @api.returns("mail.message", lambda value: value.id)
///     def message_post(self, **kwargs):
///         return super().message_post(**kwargs)
/// ```
///
/// ## Options
/// - `lint.odoo.odoo-version`
///
/// The set of decorated methods, and the decorator each one takes, is read from the configured
/// version: `main_partner` only exists up to 17.0, `send_mail_batch` only from 17.0, and
/// `discuss.channel`'s `channel_get` takes a different `downgrade` in 17.0 than in 18.0. From
/// 19.0 the set is empty and the rule never fires — see [`removed-api-returns`][ODE9502].
///
/// ## Fix safety
/// The fix is marked unsafe because adding the decorator deliberately changes what RPC clients
/// receive, from a recordset to an id, and because it assumes the override really does return
/// what the base method returns. It is not offered where the decorator needs a symbol the file
/// may not import (`Store`, for `discuss.channel` on 18.0).
///
/// ## References
/// - [`api.propagate`][propagate] — the three lines that decide everything: `_returns` is copied
///   off the Python base and nowhere else.
/// - [`api.Meta.__new__`][meta] — builds that base from `bases`, which `_inherit` is not part of.
/// - [`api.returns`][returns] — the decorator, and the docstring that says it is inherited.
/// - [`BaseModel._build_model`][build] — the registry assembly, which does not re-propagate.
/// - [`mail.thread.message_post`][message-post] — the most common case in practice.
///
/// [propagate]: https://github.com/odoo/odoo/blob/5e9620b2a9cefbf688ff055e9f20b4f337444536/odoo/api.py#L151-L159
/// [meta]: https://github.com/odoo/odoo/blob/5e9620b2a9cefbf688ff055e9f20b4f337444536/odoo/api.py#L115-L131
/// [returns]: https://github.com/odoo/odoo/blob/5e9620b2a9cefbf688ff055e9f20b4f337444536/odoo/api.py#L359-L393
/// [build]: https://github.com/odoo/odoo/blob/5e9620b2a9cefbf688ff055e9f20b4f337444536/odoo/models.py#L716-L788
/// [message-post]: https://github.com/odoo/odoo/blob/5e9620b2a9cefbf688ff055e9f20b4f337444536/addons/mail/models/mail_thread.py#L2160-L2161
/// [ODE9502]: removed-api-returns.md
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.3.34")]
pub(crate) struct MissingApiReturns {
    name: String,
    decorator: String,
}

impl Violation for MissingApiReturns {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let MissingApiReturns { name, decorator } = self;
        format!(
            "`{name}` overrides a method decorated with `@api.returns`, which `_inherit` does \
             not carry over — only Python bases do. Without it the override returns a recordset \
             over RPC instead of an id. Add `{decorator}`."
        )
    }

    fn fix_title(&self) -> Option<String> {
        Some(format!("Add `{}`", self.decorator))
    }
}

/// Which classes an entry applies to.
enum Applies {
    /// Any model class that declares `_inherit`. Used for the `mail.thread` methods, whose
    /// mixin reaches most models: a single file cannot resolve the `_inherit` graph, and a
    /// class carrying only `_name` defines a brand-new model, where a `message_post` of its
    /// own is a fresh API rather than an override.
    AnyInheritingModel,
    /// Only classes whose `_name` or `_inherit` names one of these models.
    Models(&'static [&'static str]),
}

/// A method Odoo decorates with `@api.returns` outside `odoo/models.py`.
struct DecoratedMethod {
    /// The method name.
    name: &'static str,
    /// First Odoo version that decorates it.
    since: OdooVersion,
    /// Last Odoo version that decorates it.
    until: OdooVersion,
    /// The classes the entry applies to.
    applies: Applies,
    /// The decorator to add, spelled as that version's core spells it.
    decorator: &'static str,
    /// Whether the decorator can be inserted as-is. `false` where it references a symbol the
    /// file may not import.
    fixable: bool,
}

/// Every public method Odoo decorates with `@api.returns` outside `odoo/models.py`, in core and
/// in enterprise, for 16.0 through 18.0.
///
/// Methods `BaseModel` itself decorates (`search`, `search_fetch`, `copy`, `copy_data`,
/// `copy_multi`, `create`, `exists`) are deliberately absent: `models.Model` is a real Python
/// base of every model class, so `Meta.__new__` propagates `_returns` to an override of those
/// on its own and requiring the decorator would be noise. Private methods are absent for the
/// same reason in reverse — they are not RPC-callable, so no downgrade ever runs.
///
/// From 19.0 the decorator does not exist at all, so every entry stops at 18.0.
const DECORATED_METHODS: &[DecoratedMethod] = &[
    DecoratedMethod {
        name: "message_post",
        since: OdooVersion::new(16, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::AnyInheritingModel,
        decorator: r#"@api.returns("mail.message", lambda value: value.id)"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "message_notify",
        since: OdooVersion::new(17, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::AnyInheritingModel,
        decorator: r#"@api.returns("mail.message", lambda value: value.id)"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "find_or_create",
        since: OdooVersion::new(16, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["res.partner"]),
        decorator: r#"@api.returns("self", lambda value: value.id)"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "main_partner",
        since: OdooVersion::new(16, 0),
        // Dropped in 18.0 along with the method itself.
        until: OdooVersion::new(17, 0),
        applies: Applies::Models(&["res.partner"]),
        decorator: r#"@api.returns("self")"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "create_or_replace",
        since: OdooVersion::new(16, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["ir.filters"]),
        decorator: r#"@api.returns("self", lambda value: value.id)"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "get_user_roots",
        since: OdooVersion::new(16, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["ir.ui.menu"]),
        decorator: r#"@api.returns("self")"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "upstream_dependencies",
        since: OdooVersion::new(16, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["ir.module.module"]),
        decorator: r#"@api.returns("self")"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "downstream_dependencies",
        since: OdooVersion::new(16, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["ir.module.module"]),
        decorator: r#"@api.returns("self")"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "get_module_list",
        since: OdooVersion::new(16, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["base.module.upgrade"]),
        decorator: r#"@api.returns("ir.module.module")"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "send_mail_batch",
        since: OdooVersion::new(17, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["mail.template"]),
        decorator: r#"@api.returns("self", lambda value: value.ids)"#,
        fixable: true,
    },
    // `discuss.channel`'s trio changed its `downgrade` in 18.0, when the channel payload moved
    // to `Store`. Two entries with disjoint ranges rather than one, so that each version is
    // told the decorator its own core carries.
    DecoratedMethod {
        name: "channel_get",
        since: OdooVersion::new(17, 0),
        until: OdooVersion::new(17, 0),
        applies: Applies::Models(&["discuss.channel"]),
        decorator: r#"@api.returns("self", lambda channel: channel._channel_info()[0])"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "channel_get",
        since: OdooVersion::new(18, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["discuss.channel"]),
        decorator: r#"@api.returns("self", lambda channels: Store(channels).get_result())"#,
        // `Store` comes from `odoo.addons.mail.tools.discuss`, which the file may not import.
        fixable: false,
    },
    DecoratedMethod {
        name: "channel_create",
        since: OdooVersion::new(17, 0),
        until: OdooVersion::new(17, 0),
        applies: Applies::Models(&["discuss.channel"]),
        decorator: r#"@api.returns("self", lambda channel: channel._channel_info()[0])"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "channel_create",
        since: OdooVersion::new(18, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["discuss.channel"]),
        decorator: r#"@api.returns("self", lambda channels: Store(channels).get_result())"#,
        fixable: false,
    },
    DecoratedMethod {
        name: "create_group",
        since: OdooVersion::new(17, 0),
        until: OdooVersion::new(17, 0),
        applies: Applies::Models(&["discuss.channel"]),
        decorator: r#"@api.returns("self", lambda channel: channel._channel_info()[0])"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "create_group",
        since: OdooVersion::new(18, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["discuss.channel"]),
        decorator: r#"@api.returns("self", lambda channels: Store(channels).get_result())"#,
        fixable: false,
    },
    // Enterprise.
    DecoratedMethod {
        name: "article_create",
        since: OdooVersion::new(16, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["knowledge.article"]),
        decorator: r#"@api.returns("knowledge.article", lambda article: article.id)"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "action_make_private_copy",
        since: OdooVersion::new(16, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["knowledge.article"]),
        decorator: r#"@api.returns("self", lambda value: value.id)"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "action_clone",
        since: OdooVersion::new(17, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["knowledge.article"]),
        decorator: r#"@api.returns("self", lambda value: value.id)"#,
        fixable: true,
    },
    DecoratedMethod {
        name: "action_freeze_and_copy",
        since: OdooVersion::new(18, 0),
        until: OdooVersion::new(18, 0),
        applies: Applies::Models(&["documents.document"]),
        decorator: r#"@api.returns("documents.document", lambda d: {"id": d.id, "shortcut_document_id": d.shortcut_document_id.id})"#,
        // Too long to insert without reflowing the call.
        fixable: false,
    },
];

/// Returns `true` if `decorator` resolves to `odoo.api.returns`, however it is spelled:
/// `@api.returns`, `@odoo.api.returns`, or `@returns` after `from odoo.api import returns`.
fn is_api_returns(checker: &Checker, decorator: &ast::Decorator) -> bool {
    let expression = match &decorator.expression {
        Expr::Call(call) => call.func.as_ref(),
        expression => expression,
    };
    checker
        .semantic()
        .resolve_qualified_name(expression)
        .is_some_and(|qualified_name| {
            matches!(qualified_name.segments(), ["odoo", "api", "returns"])
        })
}

/// Returns the model attribute `expr` assigns to, if it is `_name` or `_inherit`.
fn model_attribute(expr: &Expr) -> Option<&'static str> {
    let Expr::Name(name) = expr else {
        return None;
    };
    match name.id.as_str() {
        "_name" => Some("_name"),
        "_inherit" => Some("_inherit"),
        _ => None,
    }
}

/// Collects the model names a class declares through `_name` and `_inherit`, and whether it
/// declares `_inherit` at all. Both the string and the list form are read.
fn declared_models(class_def: &ast::StmtClassDef) -> (Vec<String>, bool) {
    let mut models = Vec::new();
    let mut inherits = false;
    for stmt in &class_def.body {
        let (attribute, value) = match stmt {
            Stmt::Assign(assign) => {
                let Some(attribute) = assign.targets.iter().find_map(model_attribute) else {
                    continue;
                };
                (attribute, Some(assign.value.as_ref()))
            }
            Stmt::AnnAssign(assign) => {
                let Some(attribute) = model_attribute(&assign.target) else {
                    continue;
                };
                (attribute, assign.value.as_deref())
            }
            _ => continue,
        };
        if attribute == "_inherit" {
            inherits = true;
        }
        let Some(value) = value else { continue };
        match value {
            Expr::StringLiteral(literal) => models.push(literal.value.to_string()),
            Expr::List(ast::ExprList { elts, .. }) | Expr::Tuple(ast::ExprTuple { elts, .. }) => {
                models.extend(elts.iter().filter_map(|elt| match elt {
                    Expr::StringLiteral(literal) => Some(literal.value.to_string()),
                    _ => None,
                }));
            }
            _ => {}
        }
    }
    (models, inherits)
}

/// ODW9503
pub(crate) fn missing_api_returns(checker: &Checker, function_def: &ast::StmtFunctionDef) {
    let ScopeKind::Class(class_def) = checker.semantic().current_scope().kind else {
        return;
    };
    // A literal `models.Model` / `TransientModel` / `AbstractModel` base. A class deriving from
    // another class in the same file is skipped on purpose: that is real Python inheritance, so
    // `Meta.__new__` does propagate `_returns` and there is nothing to report.
    if !is_odoo_model_class(checker.semantic(), class_def) {
        return;
    }
    if function_def
        .decorator_list
        .iter()
        .any(|decorator| is_api_returns(checker, decorator))
    {
        return;
    }

    let (models, inherits) = declared_models(class_def);
    let matches: Vec<&DecoratedMethod> = DECORATED_METHODS
        .iter()
        .filter(|method| method.name == function_def.name.as_str())
        .filter(|method| odoo_version_applies(checker, Some(method.since), Some(method.until)))
        .filter(|method| match method.applies {
            Applies::AnyInheritingModel => inherits,
            Applies::Models(names) => models.iter().any(|model| names.contains(&model.as_str())),
        })
        .collect();
    let Some(method) = matches.first() else {
        return;
    };
    // With no `odoo-version` configured, several versions' entries can match at once. Report the
    // oldest spelling, and offer no fix when the versions disagree on what the decorator is.
    let unambiguous = matches
        .iter()
        .all(|other| other.decorator == method.decorator);

    let mut diagnostic = checker.report_diagnostic(
        MissingApiReturns {
            name: function_def.name.to_string(),
            decorator: method.decorator.to_string(),
        },
        function_def.name.range(),
    );
    if method.fixable && unambiguous {
        let locator = checker.locator();
        let anchor = function_def.name.start();
        let line_start = locator.line_start(anchor);
        let indentation: String = locator
            .slice(TextRange::new(line_start, anchor))
            .chars()
            .take_while(|character| character.is_whitespace())
            .collect();
        diagnostic.set_fix(Fix::unsafe_edit(Edit::insertion(
            format!("{indentation}{}\n", method.decorator),
            line_start,
        )));
    }
}

/// ## What it does
/// Checks for uses of `odoo.api.returns` on Odoo 19.0 and later, where the symbol no longer
/// exists.
///
/// ## Why is this bad?
/// Odoo 19.0 removed the decorator and the whole mechanism behind it. The decorators that
/// survive live in `odoo/orm/decorators.py`, which does not export `returns`, and no `_returns`
/// identifier is left anywhere in the tree — the textual matches that remain are all substrings
/// of unrelated names such as `action_create_returns`.
///
/// `odoo.api` is still a package, so the failure is not an import of the module but of the name:
/// `@api.returns(...)` raises `AttributeError` at import time, and `from odoo.api import returns`
/// raises `ImportError`. Either one takes down every module that loads the file.
///
/// Nothing is lost by deleting it. `call_kw` now downgrades unconditionally, whatever the
/// method and whatever decorators it carries:
///
/// ```python
/// if name == "create":
///     result = result.id if isinstance(args[0], Mapping) else result.ids
/// elif isinstance(result, BaseModel):
///     result = result.ids
/// ```
///
/// Note that this is not the same wire format: `@api.returns("mail.message", lambda v: v.id)`
/// downgraded to a scalar id, while 19.0 returns `.ids`, a list. That change is Odoo's, and
/// removing the decorator does not cause it — but an RPC client written against 18.0 has to be
/// updated either way.
///
/// ## Example
/// ```python
/// class MailThread(models.AbstractModel):
///     _inherit = "mail.thread"
///
///     @api.returns("mail.message", lambda value: value.id)
///     def message_post(self, **kwargs):
///         return super().message_post(**kwargs)
/// ```
///
/// Use instead:
/// ```python
/// class MailThread(models.AbstractModel):
///     _inherit = "mail.thread"
///
///     def message_post(self, **kwargs):
///         return super().message_post(**kwargs)
/// ```
///
/// ## Options
/// - `lint.odoo.odoo-version`
///
/// The rule stays silent when no `odoo-version` is configured: the decorator is correct on
/// every version up to 18.0, so reporting it without knowing the target would fire on healthy
/// code. See [`missing-api-returns`][ODW9503] for the versions where it is required instead.
///
/// ## References
/// - [`odoo/orm/decorators.py`][decorators] — the 19.0 decorator set, without `returns`.
/// - [`call_kw`][call-kw] — the unconditional downgrade that replaced it.
///
/// [decorators]: https://github.com/odoo/odoo/blob/928ae2ba164022a51cdfe548dec9491c61339a5f/odoo/orm/decorators.py
/// [call-kw]: https://github.com/odoo/odoo/blob/928ae2ba164022a51cdfe548dec9491c61339a5f/odoo/service/model.py#L74-L106
/// [ODW9503]: missing-api-returns.md
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.3.34")]
pub(crate) struct RemovedApiReturns;

impl Violation for RemovedApiReturns {
    #[derive_message_formats]
    fn message(&self) -> String {
        "`api.returns` was removed in Odoo 19.0 and raises at import. Remove it — `call_kw` \
         downgrades every recordset result on its own."
            .to_string()
    }
}

/// Returns `true` if the project is known to be on a version without `api.returns`.
///
/// Unlike the other version-scoped rules this reads the setting directly instead of going
/// through `odoo_version_applies`, which answers "applies" to an unconfigured project. Here that
/// default is wrong: the decorator is correct on 16.0 through 18.0, so firing without knowing
/// the target would report healthy code.
fn returns_is_gone(checker: &Checker) -> bool {
    checker
        .settings()
        .odoo
        .odoo_version
        .is_some_and(|version| version >= OdooVersion::new(19, 0))
}

/// ODE9502
pub(crate) fn removed_api_returns(checker: &Checker, expr: &Expr) {
    if !returns_is_gone(checker) {
        return;
    }
    if checker
        .semantic()
        .resolve_qualified_name(expr)
        .is_some_and(|qualified_name| {
            matches!(qualified_name.segments(), ["odoo", "api", "returns"])
        })
    {
        checker.report_diagnostic(RemovedApiReturns, expr.range());
    }
}

/// ODE9502, on the import itself.
///
/// `from odoo.api import returns` raises `ImportError` on 19.0 even when the name is never used,
/// so the import is reported on its own rather than only through its call sites.
pub(crate) fn removed_api_returns_import(checker: &Checker, import_from: &ast::StmtImportFrom) {
    if !returns_is_gone(checker) {
        return;
    }
    if import_from.level != 0 {
        return;
    }
    let Some(module) = import_from.module.as_ref() else {
        return;
    };
    if module.as_str() != "odoo.api" {
        return;
    }
    for alias in &import_from.names {
        if alias.name.as_str() == "returns" {
            checker.report_diagnostic(RemovedApiReturns, alias.range());
        }
    }
}
