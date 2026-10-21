use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::token::parenthesized_range;
use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};
use ruff_python_semantic::ScopeKind;
use ruff_source_file::LineRanges;
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::fix::edits::fits;
use crate::rules::odoo::helpers::{
    dotted_name, inherits_non_builtin, is_recordset_name, odoo_version_applies,
};
use crate::rules::odoo::settings::OdooVersion;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for writes to the cache of an Odoo record through its `_cache` attribute:
/// assigning to `record._cache[name]`, deleting it, and calling the mapping methods that
/// change it (`update`, `pop`, `setdefault`, `clear`, `popitem`).
///
/// ## Why is this bad?
/// Odoo 19.0 made `record._cache` read-only. Up to 18.0 it returned a `RecordCache` that
/// was a `MutableMapping`, so `record._cache["name"] = value` stored `value` in the cache of
/// the record. From 19.0 on `RecordCache` is a plain `Mapping`: reading it still works, but
/// the assignment and `del` raise `TypeError` and the mapping methods raise
/// `AttributeError`. The line reads as valid code and only fails when it runs, which is
/// typically inside a `try` that logs the error and carries on.
///
/// Odoo's own code moved to the field's cache API in 19.0: what was
/// `menu._cache["child_id"] = ()` in `website` is now
/// `menu._fields["child_id"]._update_cache(menu, ())`. `Field._update_cache` is also what
/// `env.cache.set` delegates to in 19.0, and unlike `env.cache` it is still there in the
/// next version, where Odoo removed the environment-wide `Cache` object.
///
/// The cache holds values in the *cache* format, not the record format, and the fix
/// carries the value over untouched: a many2one takes an id or `None`, a one2many or
/// many2many a tuple of ids (`()` when empty). `None` in an x2many cache only ever worked
/// where the value was merely tested for truth, and breaks as soon as it is iterated.
///
/// ## Scope
/// Reading the cache is not reported: `record._cache["name"]`, `record._cache.get(...)`,
/// `"name" in record._cache` and iterating it all still work in 19.0.
///
/// A bare `self` counts only inside an Odoo model class, and an attribute chain rooted at
/// `self` only inside a class that inherits something which is not a Python builtin, so a
/// helper class keeping a `self._cache` dict of its own is left alone. A name bound to an
/// import, such as the `re` module's `re._cache`, is not a record either.
///
/// ## Example
/// ```python
/// template._cache["report_template_ids"] = ()
/// ```
///
/// Use instead:
/// ```python
/// template._fields["report_template_ids"]._update_cache(template, ())
/// ```
///
/// ## Options
/// - `lint.odoo.odoo-version`
///
/// ## Fix safety
/// A fix is offered for a plain assignment, `record._cache[name] = value`, and rewrites it
/// to `record._fields[name]._update_cache(record, value)`. That is the call 18.0's
/// `RecordCache.__setitem__` ended up making through `env.cache.set`: the same value
/// stored in the same cache entry, with an error logged if the field is dirty on the record.
///
/// It is marked as unsafe unless [`odoo-version`](../settings.md#lint_odoo_odoo-version) is
/// set to 19.0 or later, because `Field._update_cache` does not exist before 19.0. With the
/// version configured, the fix is safe.
///
/// The record is written twice by the rewrite, so the fix is only offered when it is a name
/// or a chain of attributes on one, which reads the same both times. It is not offered for
/// an augmented assignment, a deletion, a mapping method, an assignment with several
/// targets, or one with a comment the rewrite would drop.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.17.0.36", category = Category::Correctness)]
pub(crate) struct RecordCacheWrite {
    operation: CacheWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CacheWrite {
    Assignment,
    Deletion,
    Method(&'static str),
}

impl Violation for RecordCacheWrite {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        match self.operation {
            CacheWrite::Assignment => "A record's `_cache` is read-only since Odoo 19.0, \
                 assigning to it raises `TypeError`"
                .to_string(),
            CacheWrite::Deletion => "A record's `_cache` is read-only since Odoo 19.0, \
                 deleting from it raises `TypeError`"
                .to_string(),
            CacheWrite::Method(method) => format!(
                "A record's `_cache` is read-only since Odoo 19.0, \
                 `_cache.{method}()` raises `AttributeError`"
            ),
        }
    }

    fn fix_title(&self) -> Option<String> {
        match self.operation {
            CacheWrite::Assignment => {
                Some("Store the value with `Field._update_cache`".to_string())
            }
            CacheWrite::Deletion | CacheWrite::Method(_) => None,
        }
    }
}

/// The Odoo version that made `RecordCache` read-only.
const SINCE: OdooVersion = OdooVersion::new(19, 0);

/// The `MutableMapping` methods `RecordCache` lost when it became a `Mapping`.
const WRITE_METHODS: &[&str] = &["clear", "pop", "popitem", "setdefault", "update"];

/// ODE9504, for `record._cache[name] = value`, `record._cache[name] += value` and
/// `del record._cache[name]`.
pub(crate) fn record_cache_write_subscript(checker: &Checker, subscript: &ast::ExprSubscript) {
    let operation = match subscript.ctx {
        ExprContext::Store => CacheWrite::Assignment,
        ExprContext::Del => CacheWrite::Deletion,
        _ => return,
    };
    let Some(record) = record_of_cache(subscript.value.as_ref()) else {
        return;
    };
    // Up to 18.0 the record cache is writable, and this is how one writes it.
    if !odoo_version_applies(checker, Some(SINCE), None) {
        return;
    }
    if !is_record_receiver(checker, record) {
        return;
    }

    let mut diagnostic =
        checker.report_diagnostic(RecordCacheWrite { operation }, subscript.range());
    if operation == CacheWrite::Assignment {
        if let Some(fix) = update_cache_fix(checker, subscript, record) {
            diagnostic.set_fix(fix);
        }
    }
}

/// ODE9504, for the mapping methods that change the cache, as in `record._cache.update(...)`.
pub(crate) fn record_cache_write_call(checker: &Checker, call: &ast::ExprCall) {
    let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref() else {
        return;
    };
    let Some(method) = WRITE_METHODS
        .iter()
        .find(|method| **method == attr.as_str())
    else {
        return;
    };
    let Some(record) = record_of_cache(value) else {
        return;
    };
    if !odoo_version_applies(checker, Some(SINCE), None) {
        return;
    }
    if !is_record_receiver(checker, record) {
        return;
    }
    checker.report_diagnostic(
        RecordCacheWrite {
            operation: CacheWrite::Method(method),
        },
        call.range(),
    );
}

/// The `record` of a `record._cache` attribute access.
fn record_of_cache(expr: &Expr) -> Option<&Expr> {
    let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = expr else {
        return None;
    };
    (attr.as_str() == "_cache").then_some(value.as_ref())
}

/// Returns `true` unless `record` is known not to be an Odoo record: `self` outside an Odoo
/// class, `cls`, or a name bound to an import.
fn is_record_receiver(checker: &Checker, record: &Expr) -> bool {
    let semantic = checker.semantic();
    let mut root = record;
    while let Expr::Attribute(ast::ExprAttribute { value, .. }) = root {
        root = value;
    }
    let Expr::Name(name) = root else {
        return true;
    };
    match name.id.as_str() {
        // A bare `self` is a record inside a model class. Elsewhere it is whatever object
        // the class builds, and one keeping a `_cache` dict of its own is common enough:
        // Odoo's own `LangProxyDict` and source-map generator both do.
        "self" if std::ptr::eq(root, record) => is_recordset_name(checker, "self"),
        // An attribute of `self` can be a record wherever Odoo code runs, a test case's
        // `self.partner` say, which is in a class inheriting something from Odoo.
        "self" => semantic.current_scopes().any(|scope| {
            matches!(scope.kind, ScopeKind::Class(class_def) if inherits_non_builtin(semantic, class_def))
        }),
        "cls" => false,
        // `re._cache` and the like: a module, not a record.
        _ => semantic.resolve_qualified_name(root).is_none(),
    }
}

/// Builds the fix that rewrites `record._cache[name] = value` into
/// `record._fields[name]._update_cache(record, value)`.
fn update_cache_fix(
    checker: &Checker,
    subscript: &ast::ExprSubscript,
    record: &Expr,
) -> Option<Fix> {
    let Stmt::Assign(assign) = checker.semantic().current_statement() else {
        return None;
    };
    let [target] = assign.targets.as_slice() else {
        return None;
    };
    if target.range() != subscript.range() {
        return None;
    }
    // The record appears twice in the rewrite, so it has to read the same both times.
    let record = dotted_name(record)?;
    if subscript.slice.is_slice_expr() {
        return None;
    }

    let locator = checker.locator();
    let value_range = parenthesized_range(
        assign.value.as_ref().into(),
        assign.into(),
        checker.tokens(),
    )
    .unwrap_or(assign.value.range());
    // The rewrite is rebuilt from the key and the value, so a comment anywhere between the
    // start of the target and the start of the value would be dropped.
    if checker
        .comment_ranges()
        .intersects(TextRange::new(assign.start(), value_range.start()))
    {
        return None;
    }

    let key = locator.slice(subscript.slice.range());
    let value = locator.slice(value_range);
    // As an argument, a bare tuple would turn into several arguments and a bare `yield`
    // into a syntax error. A parenthesized tuple, `()` included, carries its parentheses in
    // its own range.
    let bare = value_range == assign.value.range()
        && match assign.value.as_ref() {
            Expr::Tuple(tuple) => !tuple.parenthesized,
            Expr::Yield(_) | Expr::YieldFrom(_) => true,
            _ => false,
        };
    let value = if bare {
        format!("({value})")
    } else {
        value.to_string()
    };

    let applicability_safe = checker.settings().odoo.odoo_version.is_some();
    let make_fix = |content: String| {
        let edit = Edit::range_replacement(content, assign.range());
        if applicability_safe {
            Fix::safe_edit(edit)
        } else {
            Fix::unsafe_edit(edit)
        }
    };

    let single_line = format!("{record}._fields[{key}]._update_cache({record}, {value})");
    let suffix = locator.slice(TextRange::new(assign.end(), locator.line_end(assign.end())));
    if fits(
        &format!("{single_line}{suffix}"),
        assign.into(),
        locator,
        checker.settings().pycodestyle.max_line_length,
        checker.settings().tab_size,
    ) {
        return Some(make_fix(single_line));
    }

    // Too long for one line: one argument per line, with a trailing comma so formatters keep
    // it that way. That needs the statement to start its own line, to indent from.
    let line_start = locator.line_start(assign.start());
    let indent = locator.slice(TextRange::new(line_start, assign.start()));
    if !indent.chars().all(char::is_whitespace) {
        return None;
    }
    let inner_indent = format!("{indent}{}", checker.stylist().indentation().as_str());
    let line_ending = checker.stylist().line_ending().as_str();
    Some(make_fix(format!(
        "{record}._fields[{key}]._update_cache({line_ending}\
         {inner_indent}{record},{line_ending}\
         {inner_indent}{value},{line_ending}\
         {indent})"
    )))
}
