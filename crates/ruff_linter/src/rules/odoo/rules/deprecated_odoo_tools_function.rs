use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::visitor::{Visitor, walk_expr};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_semantic::{BindingKind, Imported, NodeId, Symbol};
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::codes::Category;
use crate::importer::ImportRequest;
use crate::rules::odoo::helpers::odoo_version_applies;
use crate::rules::odoo::settings::OdooVersion;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for uses of the helpers Odoo deprecated in 18.0 and deleted in 20.0:
/// `odoo.tools.ustr` (also reachable as `odoo.tools.misc.ustr` and `odoo.loglevels.ustr`),
/// `odoo.tools.get_encodings`, and `odoo.tools.pycompat.csv_reader`, `csv_writer` and
/// `to_text`.
///
/// The name is resolved through the imports of the file, so a local `ustr` or a `csv_reader`
/// variable is not reported.
///
/// ## Why is this bad?
/// Each call raises a `DeprecationWarning` on 18.0 and 19.0, and on 20.0 the helper no longer
/// exists, so the module fails to import. `ustr` is the common one: it tries several
/// encodings in turn, and the only fallback that ever fires is `latin-1`, which accepts any
/// byte, so it never fails and never tells the caller that the data was not UTF-8.
///
/// ## Example
/// ```python
/// from odoo.tools import ustr
///
/// label = ustr(word, errors="replace")
/// message = ustr(error)
/// ```
///
/// Use instead:
/// ```python
/// from odoo.tools import exception_to_unicode
///
/// label = word.decode(errors="replace")
/// message = exception_to_unicode(error)
/// ```
///
/// ## Fix safety
/// `ustr(value)` behaves differently depending on what `value` is, and the rule cannot see
/// the type, so only the cases below are rewritten. The equivalence was measured by running
/// `ustr` from Odoo 18.0 and 19.0 against the replacement, not assumed.
///
/// The fix is **safe** when the result is proven identical:
///
/// - the argument is a `str` by construction (a string literal, an f-string or a `str(...)`
///   call), where `ustr` returns it untouched and ignores `hint_encoding` and `errors`:
///   `ustr("a")` becomes `"a"`;
/// - the argument is the name bound by `except ... as error`, where `ustr` delegates to
///   `exception_to_unicode`: `ustr(error)` becomes `exception_to_unicode(error)`. This is not
///   `str(error)`, which differs for an exception with several arguments and for `KeyError`;
/// - the argument of `to_text` is a `str` by construction.
///
/// The fix is **unsafe** when it holds only for the type the author most likely has in mind:
///
/// - `ustr(value)` with no other argument becomes `str(value)`. That is wrong when `value`
///   is `bytes`: `ustr` decodes it, `str` returns its `b'...'` representation;
/// - `ustr(value, hint_encoding, errors)` with either of the two becomes
///   `value.decode(...)`, since passing them only makes sense for `bytes`. It is wrong when
///   `value` is already a `str`, which has no `decode`, and when the bytes are not valid in
///   the encoding: with `errors="strict"` `ustr` falls back to `latin-1`, `decode` raises;
/// - `csv_reader(stream, ...)` and `csv_writer(stream, ...)` become `csv.reader` and
///   `csv.writer` over `codecs.getreader("utf-8")` and `codecs.getwriter("utf-8")`, which is
///   their body in 19.0 minus the `assert` that refuses a text stream.
///
/// No fix is offered for `get_encodings`, for `to_text` of a value that is not known to be a
/// `str` (it turns `None` and `False` into `""`, and decodes `bytes`, which no single
/// expression reproduces), or for a call with unpacked arguments.
///
/// The import left without a use is removed along with a fix that does not add an import of
/// its own. Where the fix adds one, `F401` removes the old one on the next pass.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.17.0.37", category = Category::Suspicious)]
pub(crate) struct DeprecatedOdooToolsFunction {
    name: &'static str,
    /// The version that deleted the helper, set only when the configured `odoo-version` is
    /// already at or past it, so that the message can say "removed" instead of "deprecated".
    removed: Option<OdooVersion>,
    replacement: &'static str,
    fix_title: Option<String>,
}

impl Violation for DeprecatedOdooToolsFunction {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        let DeprecatedOdooToolsFunction {
            name,
            removed,
            replacement,
            ..
        } = self;
        match removed {
            Some(removed) => format!(
                "`{name}` was removed in Odoo {removed} (deprecated since {SINCE}). {replacement}"
            ),
            None => format!("`{name}` is deprecated since Odoo {SINCE}. {replacement}"),
        }
    }

    fn fix_title(&self) -> Option<String> {
        self.fix_title.clone()
    }
}

/// The release that deprecated all of them. 18.0 is when `ustr`, `get_encodings` and the
/// three `pycompat` functions started to warn.
const SINCE: OdooVersion = OdooVersion::new(18, 0);

/// The release that deleted them: `odoo/tools/pycompat.py` and the `ustr` and `get_encodings`
/// definitions are gone from `master` since the commit `080bbf6153a0` ("[IMP] orm: remove
/// stuff deprecated since Odoo 19"), and from the 20.0 series cut from it.
const REMOVED_IN: OdooVersion = OdooVersion::new(20, 0);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Ustr,
    GetEncodings,
    CsvReader,
    CsvWriter,
    ToText,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Ustr => "ustr",
            Kind::GetEncodings => "get_encodings",
            Kind::CsvReader => "csv_reader",
            Kind::CsvWriter => "csv_writer",
            Kind::ToText => "to_text",
        }
    }

    fn replacement(self) -> &'static str {
        match self {
            Kind::Ustr => {
                "Use `str(value)`, or `value.decode(...)` when `value` is `bytes`, or \
                 `exception_to_unicode` for an exception."
            }
            Kind::GetEncodings => {
                "Decode with the one encoding the data is known to have instead of guessing."
            }
            Kind::CsvReader => {
                "Use `csv.reader` over a text stream, or over `codecs.getreader(\"utf-8\")` of a \
                 bytes stream."
            }
            Kind::CsvWriter => {
                "Use `csv.writer` over a text stream, or over `codecs.getwriter(\"utf-8\")` of a \
                 bytes stream."
            }
            Kind::ToText => {
                "Use `str(value)`, decoding `bytes` first; `None` and `False` no longer become \
                 an empty string."
            }
        }
    }
}

/// Which deprecated helper a fully-qualified name is, if any.
fn classify(segments: &[&str]) -> Option<Kind> {
    match segments {
        ["odoo", "tools" | "loglevels", "ustr"] | ["odoo", "tools", "misc", "ustr"] => {
            Some(Kind::Ustr)
        }
        ["odoo", "tools" | "loglevels", "get_encodings"]
        | ["odoo", "tools", "misc", "get_encodings"] => Some(Kind::GetEncodings),
        ["odoo", "tools", "pycompat", "csv_reader"] => Some(Kind::CsvReader),
        ["odoo", "tools", "pycompat", "csv_writer"] => Some(Kind::CsvWriter),
        ["odoo", "tools", "pycompat", "to_text"] => Some(Kind::ToText),
        _ => None,
    }
}

/// ODW9506
pub(crate) fn deprecated_odoo_tools_function(checker: &Checker, expr: &Expr) {
    if !odoo_version_applies(checker, Some(SINCE), None) {
        return;
    }
    let semantic = checker.semantic();
    let Some(qualified_name) = semantic.resolve_qualified_name(expr) else {
        return;
    };
    let Some(kind) = classify(qualified_name.segments()) else {
        return;
    };
    // Say "removed" only once the project is known to be on a version without the helper; an
    // unconfigured project keeps the deprecation wording.
    let removed = checker
        .settings()
        .odoo
        .odoo_version
        .filter(|configured| *configured >= REMOVED_IN)
        .map(|_| REMOVED_IN);

    let call = semantic
        .current_expression_parent()
        .and_then(Expr::as_call_expr)
        .filter(|call| call.func.range() == expr.range());
    let fix = call.and_then(|call| build_fix(checker, expr, call, kind));

    let mut diagnostic = checker.report_diagnostic(
        DeprecatedOdooToolsFunction {
            name: kind.name(),
            removed,
            replacement: kind.replacement(),
            fix_title: fix.as_ref().map(|(_, title)| title.clone()),
        },
        expr.range(),
    );
    if let Some((fix, _)) = fix {
        diagnostic.set_fix(fix);
    }
}

/// The arguments of a `ustr(value, hint_encoding='utf-8', errors='strict')` call, resolved
/// whether they were given by position or by keyword.
struct UstrArguments<'a> {
    value: &'a Expr,
    hint_encoding: Option<(&'a Expr, bool)>,
    errors: Option<(&'a Expr, bool)>,
}

/// Read the arguments of a `ustr` call, or `None` when they cannot be told apart statically
/// (unpacked arguments, an unknown keyword, an argument given twice).
fn ustr_arguments(call: &ast::ExprCall) -> Option<UstrArguments<'_>> {
    let args = &call.arguments.args;
    if args.len() > 3 || args.iter().any(Expr::is_starred_expr) {
        return None;
    }
    let mut value = args.first();
    let mut hint_encoding = args.get(1).map(|expr| (expr, false));
    let mut errors = args.get(2).map(|expr| (expr, false));
    for keyword in &call.arguments.keywords {
        let name = keyword.arg.as_ref()?.as_str();
        let slot = match name {
            "value" => &mut value,
            "hint_encoding" => {
                if hint_encoding.is_some() {
                    return None;
                }
                hint_encoding = Some((&keyword.value, true));
                continue;
            }
            "errors" => {
                if errors.is_some() {
                    return None;
                }
                errors = Some((&keyword.value, true));
                continue;
            }
            _ => return None,
        };
        if slot.is_some() {
            return None;
        }
        *slot = Some(&keyword.value);
    }
    Some(UstrArguments {
        value: value?,
        hint_encoding,
        errors,
    })
}

/// Whether the expression is a `str` by construction, so that `ustr` and `to_text` return it
/// unchanged.
fn is_str(checker: &Checker, expr: &Expr) -> bool {
    match expr {
        Expr::StringLiteral(_) | Expr::FString(_) => true,
        Expr::Call(call) => checker.semantic().match_builtin_expr(&call.func, "str"),
        _ => false,
    }
}

/// Whether the expression is a constant, so that dropping it from the call loses no effect.
fn is_constant(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::StringLiteral(_)
            | Expr::NoneLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NumberLiteral(_)
    )
}

/// Whether `expr` can take a `.attribute` without parentheses.
fn is_atom(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Name(_)
            | Expr::Attribute(_)
            | Expr::Call(_)
            | Expr::Subscript(_)
            | Expr::StringLiteral(_)
            | Expr::BytesLiteral(_)
    )
}

/// The edit that removes the `from ... import name` this reference is the only use of, or
/// `None` when the name is used elsewhere or is not a plain from-import.
fn remove_unused_import(checker: &Checker, expr: &Expr) -> Option<(Edit, Option<NodeId>)> {
    let Expr::Name(name) = expr else {
        return None;
    };
    let semantic = checker.semantic();
    let Symbol::Binding(binding_id) = semantic.lookup_symbol(name.id.as_str()) else {
        return None;
    };
    let binding = semantic.binding(binding_id);
    let BindingKind::FromImport(import) = &binding.kind else {
        return None;
    };
    // Any other use keeps the import alive; the last one to be rewritten takes it away. The
    // checker has only seen the references up to this one, so the whole module is counted
    // rather than asking the binding.
    let mut uses = NameUses {
        name: name.id.as_str(),
        count: 0,
    };
    uses.visit_body(checker.module_body());
    if uses.count != 1 || binding.is_explicit_export() {
        return None;
    }
    let source = binding.source?;
    let edit = crate::fix::edits::remove_unused_imports(
        std::iter::once(import.member_name().as_ref()),
        semantic.statement(source),
        semantic.parent_statement(source),
        checker.locator(),
        checker.stylist(),
        checker.indexer(),
    )
    .ok()?;
    Some((edit, semantic.parent_statement_id(source)))
}

/// Counts the uses of one name in a module, in any scope. Deliberately over-counts (a local
/// that shadows the name is counted too): a missed removal is left to `F401`, a wrong one
/// breaks the file.
struct NameUses<'a> {
    name: &'a str,
    count: usize,
}

impl<'a> Visitor<'a> for NameUses<'_> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Name(name) = expr
            && name.id == self.name
        {
            self.count += 1;
        }
        walk_expr(self, expr);
    }
}

/// Build the fix of a call to a deprecated helper, with its title, when one is demonstrated.
fn build_fix(
    checker: &Checker,
    func: &Expr,
    call: &ast::ExprCall,
    kind: Kind,
) -> Option<(Fix, String)> {
    // The rewrite copies source text between the arguments, and a comment in there would be
    // lost with it.
    if checker.comment_ranges().intersects(call.range()) {
        return None;
    }
    match kind {
        Kind::GetEncodings => None,
        Kind::ToText => {
            let [value] = &*call.arguments.args else {
                return None;
            };
            if !call.arguments.keywords.is_empty() || !is_str(checker, value) {
                return None;
            }
            Some(replace_with_removed_import(
                checker,
                func,
                call.range(),
                checker.locator().slice(value).to_string(),
                true,
                "Replace with its argument".to_string(),
            ))
        }
        Kind::Ustr => ustr_fix(checker, func, call),
        Kind::CsvReader | Kind::CsvWriter => csv_fix(checker, call, kind),
    }
}

/// Replace the call with `content`, taking the import of `func` out when this was its last
/// use.
fn replace_with_removed_import(
    checker: &Checker,
    func: &Expr,
    range: TextRange,
    content: String,
    safe: bool,
    title: String,
) -> (Fix, String) {
    let edit = Edit::range_replacement(content, range);
    let fix = match remove_unused_import(checker, func) {
        Some((import_edit, parent)) => {
            let fix = if safe {
                Fix::safe_edits(edit, [import_edit])
            } else {
                Fix::unsafe_edits(edit, [import_edit])
            };
            fix.isolate(Checker::isolation(parent))
        }
        None if safe => Fix::safe_edit(edit),
        None => Fix::unsafe_edit(edit),
    };
    (fix, title)
}

fn ustr_fix(checker: &Checker, func: &Expr, call: &ast::ExprCall) -> Option<(Fix, String)> {
    let arguments = ustr_arguments(call)?;
    let UstrArguments {
        value,
        hint_encoding,
        errors,
    } = arguments;
    let semantic = checker.semantic();
    let locator = checker.locator();
    let has_options = hint_encoding.is_some() || errors.is_some();

    // A `str` is returned as it is, whatever the other arguments say.
    if is_str(checker, value) {
        let options_are_constant = [hint_encoding, errors]
            .into_iter()
            .flatten()
            .all(|(expr, _)| is_constant(expr));
        if !options_are_constant {
            return None;
        }
        return Some(replace_with_removed_import(
            checker,
            func,
            call.range(),
            locator.slice(value).to_string(),
            true,
            "Replace with its argument".to_string(),
        ));
    }

    // The name bound by `except ... as error`: `ustr` hands an exception to
    // `exception_to_unicode`, which is not `str(error)`.
    if !has_options
        && let Expr::Name(name) = value
        && let Symbol::Binding(binding_id) = semantic.lookup_symbol(name.id.as_str())
        && matches!(
            semantic.binding(binding_id).kind,
            BindingKind::BoundException
        )
    {
        let (import_edit, binding) = checker
            .importer()
            .get_or_import_symbol(
                &ImportRequest::import_from("odoo.tools", "exception_to_unicode"),
                call.start(),
                semantic,
            )
            .ok()?;
        let edit =
            Edit::range_replacement(format!("{binding}({})", locator.slice(value)), call.range());
        return Some((
            Fix::safe_edits(edit, [import_edit]),
            "Replace with `exception_to_unicode(...)`".to_string(),
        ));
    }

    if !has_options {
        // `str` can be shadowed by a local of the same name.
        if !semantic.has_builtin_binding("str") {
            return None;
        }
        return Some(replace_with_removed_import(
            checker,
            func,
            call.range(),
            format!("str({})", locator.slice(value)),
            false,
            "Replace with `str(...)`".to_string(),
        ));
    }

    // An encoding or an error handler is only meaningful for `bytes`.
    let receiver = if is_atom(value) {
        locator.slice(value).to_string()
    } else {
        format!("({})", locator.slice(value))
    };
    let mut parts = Vec::new();
    if let Some((expr, keyword)) = hint_encoding {
        let text = locator.slice(expr);
        parts.push(if keyword {
            format!("encoding={text}")
        } else {
            text.to_string()
        });
    }
    if let Some((expr, keyword)) = errors {
        let text = locator.slice(expr);
        parts.push(if keyword {
            format!("errors={text}")
        } else {
            text.to_string()
        });
    }
    Some(replace_with_removed_import(
        checker,
        func,
        call.range(),
        format!("{receiver}.decode({})", parts.join(", ")),
        false,
        "Replace with `.decode(...)`".to_string(),
    ))
}

fn csv_fix(checker: &Checker, call: &ast::ExprCall, kind: Kind) -> Option<(Fix, String)> {
    // The stream has to be the first positional argument for the rest of the call to stay
    // where it is.
    let stream = call.arguments.args.first()?;
    if call.arguments.args.iter().any(Expr::is_starred_expr) {
        return None;
    }
    let (function, codec) = match kind {
        Kind::CsvReader => ("reader", "getreader"),
        _ => ("writer", "getwriter"),
    };
    let semantic = checker.semantic();
    let (csv_edit, csv) = checker
        .importer()
        .get_or_import_symbol(
            &ImportRequest::import("csv", function),
            call.start(),
            semantic,
        )
        .ok()?;
    let (codecs_edit, codecs) = checker
        .importer()
        .get_or_import_symbol(
            &ImportRequest::import("codecs", codec),
            call.start(),
            semantic,
        )
        .ok()?;
    let quote = checker.stylist().quote();
    let edit = Edit::range_replacement(
        format!(
            "{csv}({codecs}({quote}utf-8{quote})({})",
            checker.locator().slice(stream)
        ),
        TextRange::new(call.start(), stream.end()),
    );
    // The closing parenthesis of the original call, and anything after the stream, stay as
    // they were. `csv_edit` and `codecs_edit` can be the same statement; keep them apart.
    let mut edits = vec![csv_edit];
    if codecs_edit != edits[0] {
        edits.push(codecs_edit);
    }
    Some((
        Fix::unsafe_edits(edit, edits),
        format!("Replace with `csv.{function}` over `codecs.{codec}`"),
    ))
}
