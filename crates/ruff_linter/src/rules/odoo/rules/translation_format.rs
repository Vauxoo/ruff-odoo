use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::parenthesize::parenthesized_range;
use ruff_python_ast::{self as ast, Expr, Operator, Stmt, StringFlags};
use ruff_python_stdlib::identifiers::is_identifier;
use ruff_python_stdlib::keyword::is_keyword;
use ruff_text_size::{Ranged, TextRange};

use crate::checkers::ast::Checker;
use crate::codes::Rule;
use crate::rules::odoo::helpers::odoo_version_applies;
use crate::rules::odoo::settings::OdooVersion;
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for translation calls that interpolate values eagerly with the `%` operator or
/// string concatenation (`_("Hello %s") % name`, `_("Hello %s" % name)`,
/// `_("Hello " + name)`) instead of passing them as arguments to the translation
/// function.
///
/// The rule only applies from Odoo 14.0 on, the version whose translation functions take
/// the interpolation arguments themselves; up to 13.0 `translation-contains-variable`
/// covers the eager interpolation instead. Configure the version with the `odoo-version`
/// setting; without it the rule stays enabled.
///
/// ## Why is this bad?
/// Since Odoo 14.0 the translation functions (`_`, `self.env._`) interpolate the values
/// themselves: `_("Hello %s", name)`. Interpolating before the call translates the
/// already-interpolated text, so the term never matches the exported translation entry.
/// Interpolating after the call works, but bypasses Odoo's own interpolation and, for
/// lazy translations, forces the evaluation at definition time instead of rendering time.
///
/// ## Example
/// ```python
/// _("Hello %s") % name
/// ```
///
/// Use instead:
/// ```python
/// _("Hello %s", name)
/// ```
///
/// ## Fix safety
/// A fix is offered when the interpolated values can be moved into the translation call
/// faithfully: a tuple becomes positional arguments (`_("%s %s") % (a, b)` becomes
/// `_("%s %s", a, b)`), a dict literal with valid identifier keys becomes keyword
/// arguments (`_("%(name)s") % {"name": name}` becomes `_("%(name)s", name=name)`), and
/// any other single value becomes one positional argument when the term only uses
/// positional placeholders.
///
/// The fix is marked unsafe because it relies on the translation-function signature
/// introduced in Odoo 14.0, and because a function named `_` that is not Odoo's
/// translation function (e.g. `gettext.gettext`) does not accept the extra arguments.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.9")]
pub(crate) struct TranslationNotLazy;

impl Violation for TranslationNotLazy {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Use lazy % formatting in odoo._ functions".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Pass the interpolated values as arguments to the translation call".to_string())
    }
}

/// ## What it does
/// Checks for translation calls interpolated with `str.format`, either on the term
/// (`_("Hello {}".format(name))`) or on the translated result
/// (`_("Hello {}").format(name)`).
///
/// The rule only applies from Odoo 14.0 on, the version whose translation functions take
/// the interpolation arguments themselves; up to 13.0 `translation-contains-variable`
/// covers the eager interpolation instead. Configure the version with the `odoo-version`
/// setting; without it the rule stays enabled.
///
/// ## Why is this bad?
/// Since Odoo 14.0 the translation functions (`_`, `self.env._`) interpolate the values
/// themselves using printf-style placeholders: `_("Hello %s", name)`. Formatting the
/// term interpolates before translation, so the looked-up term never matches the
/// exported translation entry; formatting the result lets a malicious translation
/// access attributes of the format arguments.
///
/// ## Example
/// ```python
/// _("Hello {}").format(name)
/// ```
///
/// Use instead:
/// ```python
/// _("Hello %s", name)
/// ```
///
/// ## Fix safety
/// A fix is offered when the template only uses bare `{}` fields, the `format` call passes
/// exactly that many positional arguments, and the literal spells no character through an
/// escape sequence: the fields become `%s`, literal braces and `%` are re-escaped, and the
/// arguments move into the translation call. The fix is marked unsafe because the term the
/// translation machinery looks up changes (`Hello {}` becomes `Hello %s`), so the exported
/// translation entries have to be regenerated.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.9")]
pub(crate) struct TranslationFormatInterpolation;

impl Violation for TranslationFormatInterpolation {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Use lazy % formatting in odoo._ functions instead of str.format".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Pass the values as arguments to the translation call".to_string())
    }
}

/// ## What it does
/// Checks for translation calls whose term is an f-string, e.g. `_(f"Hello {name}")`.
///
/// The rule only applies from Odoo 14.0 on, the version whose translation functions take
/// the interpolation arguments themselves; up to 13.0 `translation-contains-variable`
/// covers the eager interpolation instead. Configure the version with the `odoo-version`
/// setting; without it the rule stays enabled.
///
/// ## Why is this bad?
/// An f-string interpolates its values before the translation call runs, so the
/// looked-up term contains the runtime values and never matches the exported
/// translation entry. Since Odoo 14.0 the translation functions (`_`, `self.env._`)
/// interpolate the values themselves: `_("Hello %s", name)`.
///
/// ## Example
/// ```python
/// _(f"Hello {name}")
/// ```
///
/// Use instead:
/// ```python
/// _("Hello %s", name)
/// ```
///
/// ## Fix safety
/// A fix is offered when the f-string is the call's only argument and every interpolation
/// is a plain expression — no conversion flag (`!r`), format spec, or `=` debug form — and
/// no literal piece spells a character through an escape sequence: each interpolation
/// becomes `%s` and its expression moves into the translation call. The fix is marked
/// unsafe because the term the translation machinery looks up changes, so the exported
/// translation entries have to be regenerated.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.9")]
pub(crate) struct TranslationFstringInterpolation;

impl Violation for TranslationFstringInterpolation {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        "Use lazy % formatting in odoo._ functions instead of an f-string".to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some("Pass the values as arguments to the translation call".to_string())
    }
}

/// ## What it does
/// Checks for translation terms using a printf conversion that Python's `%` formatting
/// does not support, e.g. `_("Hello %y", name)`.
///
/// The rule only applies from Odoo 14.0 on, the version whose translation functions take
/// the interpolation arguments themselves; up to 13.0 `translation-contains-variable`
/// covers the eager interpolation instead. Configure the version with the `odoo-version`
/// setting; without it the rule stays enabled.
///
/// ## Why is this bad?
/// The translation function formats the term with `%`, so an unsupported conversion
/// character raises a `ValueError` at runtime.
///
/// ## Example
/// ```python
/// _("Hello %y", name)
/// ```
///
/// Use instead:
/// ```python
/// _("Hello %s", name)
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.9")]
pub(crate) struct TranslationUnsupportedFormat {
    unsupported_char: char,
    index: usize,
}

impl Violation for TranslationUnsupportedFormat {
    #[derive_message_formats]
    fn message(&self) -> String {
        let TranslationUnsupportedFormat {
            unsupported_char,
            index,
        } = self;
        format!(
            "Unsupported odoo._ format character '{unsupported_char}' ({:#04x}) at index {index}",
            u32::from(*unsupported_char)
        )
    }
}

/// ## What it does
/// Checks for translation terms ending in the middle of a printf conversion specifier,
/// e.g. `_("Hello %", name)`.
///
/// The rule only applies from Odoo 14.0 on, the version whose translation functions take
/// the interpolation arguments themselves; up to 13.0 `translation-contains-variable`
/// covers the eager interpolation instead. Configure the version with the `odoo-version`
/// setting; without it the rule stays enabled.
///
/// ## Why is this bad?
/// The translation function formats the term with `%`, so a truncated conversion
/// specifier raises a `ValueError` at runtime.
///
/// ## Example
/// ```python
/// _("Progress: 100 %", progress)
/// ```
///
/// Use instead:
/// ```python
/// _("Progress: %s %%", progress)
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.9")]
pub(crate) struct TranslationFormatTruncated;

impl Violation for TranslationFormatTruncated {
    #[derive_message_formats]
    fn message(&self) -> String {
        "odoo._ format string ends in middle of conversion specifier".to_string()
    }
}

/// ## What it does
/// Checks for translation calls passing more values than the term's printf placeholders
/// consume, e.g. `_("Hello %s", name, extra)`.
///
/// The rule only applies from Odoo 14.0 on, the version whose translation functions take
/// the interpolation arguments themselves; up to 13.0 `translation-contains-variable`
/// covers the eager interpolation instead. Configure the version with the `odoo-version`
/// setting; without it the rule stays enabled.
///
/// ## Why is this bad?
/// The translation function formats the term with `%`, and `%` formatting raises a
/// `TypeError` at runtime when arguments are left over.
///
/// ## Example
/// ```python
/// _("Hello %s", name, extra)
/// ```
///
/// Use instead:
/// ```python
/// _("Hello %s", name)
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.9")]
pub(crate) struct TranslationTooManyArgs;

impl Violation for TranslationTooManyArgs {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Too many arguments for odoo._ format string".to_string()
    }
}

/// ## What it does
/// Checks for translation calls passing fewer values than the term's printf
/// placeholders require, e.g. `_("%s of %s", count)`.
///
/// The rule only applies from Odoo 14.0 on, the version whose translation functions take
/// the interpolation arguments themselves; up to 13.0 `translation-contains-variable`
/// covers the eager interpolation instead. Configure the version with the `odoo-version`
/// setting; without it the rule stays enabled.
///
/// ## Why is this bad?
/// The translation function formats the term with `%`, and `%` formatting raises a
/// `TypeError` at runtime when placeholders are left unfilled.
///
/// ## Example
/// ```python
/// _("%s of %s", count)
/// ```
///
/// Use instead:
/// ```python
/// _("%s of %s", count, total)
/// ```
///
/// A translation call given no values of its own is normally left alone: the term is used
/// verbatim (`_("100%")`) or interpolated afterwards (`_("Hello %s") % name`). The exception
/// is the lone argument of a raised exception, where neither can happen and the placeholders
/// reach the user as they are:
///
/// ```python
/// raise UserError(_("record <%s: (%s)> can not be sent"))
/// ```
///
/// There the count is all that is checked, and only placeholders that are unmistakably
/// placeholders count: `_("100% off")` reads as prose even though `% o` is a valid
/// space-flagged conversion.
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.2.9")]
pub(crate) struct TranslationTooFewArgs;

impl Violation for TranslationTooFewArgs {
    #[derive_message_formats]
    fn message(&self) -> String {
        "Not enough arguments for odoo._ format string".to_string()
    }
}

/// Returns `true` if `func` refers to the `_` translation function: the bare `_` name or
/// an attribute path ending in it (e.g. `self.env._`).
///
/// `_lt` deliberately doesn't count: these rules mirror pylint-odoo's `translation-*`
/// checks, which only inspect `_` (lazy translations wrap the interpolation themselves).
pub(crate) fn is_translation_underscore(func: &Expr) -> bool {
    match func {
        Expr::Name(ast::ExprName { id, .. }) => id == "_",
        Expr::Attribute(ast::ExprAttribute { attr, .. }) => attr == "_",
        _ => false,
    }
}

/// The result of parsing a printf-style format string, mirroring pylint's
/// `parse_format_string` so the argument checks match `%` formatting exactly.
enum PrintfFormat {
    Parsed {
        /// The term uses `%(name)s`-style mapping keys.
        has_keywords: bool,
        /// Number of values consumed by positional conversions, including `*`
        /// width/precision.
        required_args: usize,
        /// Of those, how many are written without the space flag. `"100% off"` is a
        /// conversion to `%` formatting -- a space-flagged `%o` -- but prose far more
        /// often than not, so telling the two apart matters where the diagnostic rests
        /// on the conversion alone.
        unambiguous_args: usize,
    },
    /// A conversion uses a character `%` formatting doesn't support (char index given).
    UnsupportedChar(usize),
    /// The string ends in the middle of a conversion specifier.
    Truncated,
}

/// Parses `text` as a printf-style format string (`%[(key)][flags][width][.precision]
/// [length]conversion`). Indices are character offsets, matching Python string indexing.
fn parse_printf_format(text: &str) -> PrintfFormat {
    let chars: Vec<char> = text.chars().collect();
    let mut has_keywords = false;
    let mut required_args = 0;
    let mut unambiguous_args = 0;
    let mut i = 0;

    let next = |i: &mut usize| -> Option<char> {
        *i += 1;
        chars.get(*i).copied()
    };

    while i < chars.len() {
        if chars[i] != '%' {
            i += 1;
            continue;
        }
        let Some(mut char) = next(&mut i) else {
            return PrintfFormat::Truncated;
        };
        // The mapping key (optional), with nested parentheses: `%(a(b))s`.
        let mut has_key = false;
        if char == '(' {
            let mut depth = 1;
            let key_start = i + 1;
            loop {
                let Some(next_char) = next(&mut i) else {
                    return PrintfFormat::Truncated;
                };
                char = next_char;
                if depth == 0 {
                    break;
                }
                if char == '(' {
                    depth += 1;
                } else if char == ')' {
                    depth -= 1;
                }
            }
            has_key = i - 1 > key_start;
        }
        // The conversion flags (optional).
        let mut space_flagged = false;
        while matches!(char, '#' | '0' | '-' | ' ' | '+') {
            space_flagged |= char == ' ';
            let Some(next_char) = next(&mut i) else {
                return PrintfFormat::Truncated;
            };
            char = next_char;
        }
        // The minimum field width (optional): `*` consumes one argument.
        if char == '*' {
            required_args += 1;
            let Some(next_char) = next(&mut i) else {
                return PrintfFormat::Truncated;
            };
            char = next_char;
        } else {
            while char.is_ascii_digit() {
                let Some(next_char) = next(&mut i) else {
                    return PrintfFormat::Truncated;
                };
                char = next_char;
            }
        }
        // The precision (optional): `.*` consumes one argument.
        if char == '.' {
            let Some(next_char) = next(&mut i) else {
                return PrintfFormat::Truncated;
            };
            char = next_char;
            if char == '*' {
                required_args += 1;
                let Some(next_char) = next(&mut i) else {
                    return PrintfFormat::Truncated;
                };
                char = next_char;
            } else {
                while char.is_ascii_digit() {
                    let Some(next_char) = next(&mut i) else {
                        return PrintfFormat::Truncated;
                    };
                    char = next_char;
                }
            }
        }
        // The length modifier (optional).
        if matches!(char, 'h' | 'l' | 'L') {
            let Some(next_char) = next(&mut i) else {
                return PrintfFormat::Truncated;
            };
            char = next_char;
        }
        // The conversion type (mandatory).
        if !"diouxXeEfFgGcrs%a".contains(char) {
            return PrintfFormat::UnsupportedChar(i);
        }
        if has_key {
            has_keywords = true;
        } else if char != '%' {
            required_args += 1;
            if !space_flagged {
                unambiguous_args += 1;
            }
        }
        i += 1;
    }
    PrintfFormat::Parsed {
        has_keywords,
        required_args,
        unambiguous_args,
    }
}

/// Returns `true` if `text`, read as a `str.format` template, carries a non-empty format
/// spec in any replacement field (e.g. `{amount:>10}`). Such terms are considered too
/// complex to rewrite as printf-style placeholders, so they are not reported.
fn has_nonempty_format_spec(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(char) = chars.next() {
        match char {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
            }
            '{' => {
                let mut depth = 1;
                let mut in_spec = false;
                let mut spec_is_empty = true;
                for field_char in chars.by_ref() {
                    match field_char {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        ':' if depth == 1 && !in_spec => in_spec = true,
                        _ if in_spec => spec_is_empty = false,
                        _ => {}
                    }
                }
                if depth != 0 {
                    // Unbalanced braces: the term isn't a valid template at all.
                    return false;
                }
                if in_spec && !spec_is_empty {
                    return true;
                }
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
            }
            // A lone `}` makes the whole template invalid, like `str.format` itself.
            '}' => return false,
            _ => {}
        }
    }
    false
}

/// The `str.format` template's source rewritten with printf placeholders: bare `{}` fields
/// become `%s`, literal braces unescape (`{{` → `{`), and `%` escapes to `%%`, so the
/// rewritten literal renders exactly what the template did once the translation function
/// formats it. Returns the rewritten source and the number of fields.
///
/// `None` when a field is anything but a bare `{}`, or when the source contains a
/// backslash: an escape sequence could spell a brace or `%` the source-level rewrite
/// cannot see (e.g. `\x7b`), so such literals are left alone.
fn printf_template_from_format_source(source: &str) -> Option<(String, usize)> {
    if source.contains('\\') {
        return None;
    }
    let mut out = String::with_capacity(source.len());
    let mut fields = 0;
    let mut chars = source.chars().peekable();
    while let Some(char) = chars.next() {
        match char {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '{' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push_str("%s");
                fields += 1;
            }
            // Anything but a bare `{}` field (`{0}`, `{name}`, ...) or a doubled brace.
            '{' | '}' if chars.peek() != Some(&char) => return None,
            '}' => {
                chars.next();
                out.push('}');
            }
            '%' => out.push_str("%%"),
            _ => out.push(char),
        }
    }
    Some((out, fields))
}

/// The source of `arguments`' positional arguments, written the way the call passed them,
/// when they are the whole argument list (no keywords, no unpacking).
fn positional_arguments_source(checker: &Checker, arguments: &ast::Arguments) -> Option<String> {
    if arguments.args.is_empty()
        || !arguments.keywords.is_empty()
        || arguments.args.iter().any(Expr::is_starred_expr)
    {
        return None;
    }
    let sources: Vec<&str> = arguments
        .args
        .iter()
        .map(|arg| {
            let range = parenthesized_range(
                arg.into(),
                arguments.into(),
                checker.comment_ranges(),
                checker.locator().contents(),
            )
            .unwrap_or(arg.range());
            checker.locator().slice(range)
        })
        .collect();
    Some(sources.join(", "))
}

/// Rewrites the f-string as a printf-style literal plus its interpolated expressions,
/// ready to become translation-call arguments: `f"Hello {name}"` yields
/// (`"Hello %s"`, `name`), keeping the f-string's own quoting.
///
/// `None` when the term mixes implicitly concatenated parts, an interpolation carries a
/// conversion flag, format spec, or `=` debug form, a literal piece spells a character
/// through an escape sequence (a backslash could encode a brace or `%` the source-level
/// rewrite cannot see), or nothing is interpolated at all — without arguments the
/// translation function never `%`-formats, so the escaping would show through.
fn printf_template_from_fstring(
    checker: &Checker,
    fstring: &ast::ExprFString,
) -> Option<(String, String)> {
    let mut parts = fstring.value.iter();
    let (Some(ast::FStringPart::FString(part)), None) = (parts.next(), parts.next()) else {
        return None;
    };
    let quote_char = part.flags.quote_style().as_char();
    let quote = if part.flags.is_triple_quoted() {
        quote_char.to_string().repeat(3)
    } else {
        quote_char.to_string()
    };
    let mut template = String::new();
    let mut arguments: Vec<&str> = Vec::new();
    for element in &part.elements {
        match element {
            ast::InterpolatedStringElement::Literal(literal) => {
                let source = checker.locator().slice(literal.range());
                if source.contains('\\') {
                    return None;
                }
                template.push_str(
                    &source
                        .replace('%', "%%")
                        .replace("{{", "{")
                        .replace("}}", "}"),
                );
            }
            ast::InterpolatedStringElement::Interpolation(interpolation) => {
                if interpolation.conversion != ast::ConversionFlag::None
                    || interpolation.format_spec.is_some()
                    || interpolation.debug_text.is_some()
                {
                    return None;
                }
                template.push_str("%s");
                arguments.push(checker.locator().slice(interpolation.expression.range()));
            }
        }
    }
    if arguments.is_empty() {
        return None;
    }
    Some((format!("{quote}{template}{quote}"), arguments.join(", ")))
}

/// Returns `true` for a string literal or a `+`-concatenation built only from string
/// literals, which `%` formatting treats as one literal (e.g. `"a" + "b"`).
fn is_literal_str_concat(expr: &Expr) -> bool {
    match expr {
        Expr::StringLiteral(_) => true,
        Expr::BinOp(ast::ExprBinOp {
            op: Operator::Add,
            left,
            right,
            ..
        }) => is_literal_str_concat(left) && is_literal_str_concat(right),
        _ => false,
    }
}

/// Returns `true` if any literal piece of the f-string contains `%`-formatting
/// (e.g. `f"Hello %s"`): pylint skips such f-strings since the `%` placeholders show the
/// author is mid-migration and the interpolation is not the f-string's own.
fn fstring_contains_printf(value: &ast::FStringValue) -> bool {
    const MOST_COMMON_FORMATTING: [&str; 4] = ["%s", "%d", "%f", "%r"];
    let contains_printf = |text: &str| {
        text.contains('%')
            && MOST_COMMON_FORMATTING
                .iter()
                .any(|conversion| text.contains(conversion))
    };
    value
        .literals()
        .map(|literal| literal.value.as_ref())
        .chain(
            value
                .elements()
                .filter_map(ast::InterpolatedStringElement::as_literal)
                .map(|literal| literal.value.as_ref()),
        )
        .any(contains_printf)
}

/// Returns `true` if the `translation-*` family applies to the configured Odoo version.
///
/// pylint-odoo builds this whole family by re-publishing pylint's `logging-*` checks
/// (`custom_logging.py`), and its constructor sets `odoo_minversion = "14.0"` on every one
/// of them: before 14.0 the terms were interpolated eagerly, which is what
/// `translation-contains-variable` reports for versions up to 13.0 instead.
fn translation_family_applies(checker: &Checker) -> bool {
    odoo_version_applies(checker, Some(OdooVersion::new(14, 0)), None)
}

/// ODW8302, ODE8301, ODW8303, ODW8301, ODE8306, ODE8305, ODE8300
pub(crate) fn translation_format(checker: &Checker, call: &ast::ExprCall) {
    if !translation_family_applies(checker) {
        return;
    }
    // ODW8302 on the translated result: `_("Hello {}").format(name)`.
    if checker.is_rule_enabled(Rule::TranslationFormatInterpolation)
        && let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = call.func.as_ref()
        && attr == "format"
        && let Expr::Call(inner) = value.as_ref()
        && is_translation_underscore(&inner.func)
        && inner.arguments.keywords.is_empty()
        && let [Expr::StringLiteral(term)] = &*inner.arguments.args
        && !has_nonempty_format_spec(term.value.to_str())
    {
        let mut diagnostic =
            checker.report_diagnostic(TranslationFormatInterpolation, call.range());
        // `_("Hello {}").format(name)` becomes `_("Hello %s", name)`: the fields become
        // placeholders and the values move into the translation call.
        if !checker.comment_ranges().intersects(call.range())
            && let Some((template, fields)) =
                printf_template_from_format_source(checker.locator().slice(term.range()))
            && fields > 0
            && fields == call.arguments.args.len()
            && let Some(arguments) = positional_arguments_source(checker, &call.arguments)
        {
            let func_source = checker.locator().slice(inner.func.range());
            diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
                format!("{func_source}({template}, {arguments})"),
                call.range(),
            )));
        }
    }

    if !is_translation_underscore(&call.func) {
        return;
    }
    // Mirror pylint's gate: `*args`/`**kwargs` and argument-less calls are out of scope.
    if call.arguments.args.is_empty()
        || call.arguments.args.iter().any(Expr::is_starred_expr)
        || call
            .arguments
            .keywords
            .iter()
            .any(|keyword| keyword.arg.is_none())
    {
        return;
    }

    match &call.arguments.args[0] {
        // ODW8301: `_("Hello %s" % name)` or `_("Hello " + name)`.
        Expr::BinOp(binop) => {
            translation_not_lazy_term(checker, call, binop);
        }
        // ODW8302 on the term: `_("Hello {}".format(name))`.
        Expr::Call(inner) => {
            if checker.is_rule_enabled(Rule::TranslationFormatInterpolation)
                && let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = inner.func.as_ref()
                && attr == "format"
                && let Expr::StringLiteral(term) = value.as_ref()
                && !has_nonempty_format_spec(term.value.to_str())
            {
                let mut diagnostic =
                    checker.report_diagnostic(TranslationFormatInterpolation, call.range());
                // `_("Hello {}".format(name))` becomes `_("Hello %s", name)`: keep the
                // template, move the values into the call. Only offered when the term is
                // the call's lone argument.
                if call.arguments.args.len() == 1
                    && call.arguments.keywords.is_empty()
                    && !checker.comment_ranges().intersects(call.range())
                    && let Some((template, fields)) =
                        printf_template_from_format_source(checker.locator().slice(term.range()))
                    && fields > 0
                    && fields == inner.arguments.args.len()
                    && let Some(arguments) = positional_arguments_source(checker, &inner.arguments)
                {
                    diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
                        format!("{template}, {arguments}"),
                        inner.range(),
                    )));
                }
            }
        }
        // ODE8301, ODE8306, ODE8305, ODE8300: the term is a plain literal, so its
        // placeholders can be checked against the supplied arguments.
        Expr::StringLiteral(term) => {
            check_format_string(checker, call, term);
        }
        // ODW8303: `_(f"Hello {name}")`.
        Expr::FString(fstring) => {
            if checker.is_rule_enabled(Rule::TranslationFstringInterpolation)
                && !fstring_contains_printf(&fstring.value)
            {
                let mut diagnostic =
                    checker.report_diagnostic(TranslationFstringInterpolation, call.range());
                // `_(f"Hello {name}")` becomes `_("Hello %s", name)`. Only offered when
                // the f-string is the call's lone argument.
                if call.arguments.args.len() == 1
                    && call.arguments.keywords.is_empty()
                    && !checker.comment_ranges().intersects(call.range())
                    && let Some((template, arguments)) =
                        printf_template_from_fstring(checker, fstring)
                {
                    diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
                        format!("{template}, {arguments}"),
                        fstring.range(),
                    )));
                }
            }
        }
        _ => {}
    }
}

/// ODW8301 for interpolation inside the term: `_("Hello %s" % name)` and
/// `_("Hello " + name)`.
fn translation_not_lazy_term(checker: &Checker, call: &ast::ExprCall, binop: &ast::ExprBinOp) {
    if !checker.is_rule_enabled(Rule::TranslationNotLazy) {
        return;
    }
    let emit = match binop.op {
        Operator::Mod => true,
        // Concatenation counts when a string literal is glued to a non-literal;
        // literal-only concatenation (`"a" + "b"`) is one constant term, which is fine.
        Operator::Add => is_literal_str_concat(&binop.left) != is_literal_str_concat(&binop.right),
        _ => false,
    };
    if !emit {
        return;
    }
    let mut diagnostic = checker.report_diagnostic(TranslationNotLazy, call.range());
    // `_("%s" % name)` becomes `_("%s", name)`: keep the term, move the values into the
    // call. Only offered when the term is a literal and the call has no other arguments.
    if binop.op == Operator::Mod
        && call.arguments.args.len() == 1
        && call.arguments.keywords.is_empty()
        && !checker.comment_ranges().intersects(binop.range())
        && let Expr::StringLiteral(term) = binop.left.as_ref()
        && let Some(arguments) = lazy_arguments(checker, term, &binop.right)
    {
        let term_source = checker.locator().slice(binop.left.range());
        diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
            format!("{term_source}, {arguments}"),
            binop.range(),
        )));
    }
}

/// ODW8301 for interpolation applied to the translated result: `_("Hello %s") % name`.
///
/// pylint-odoo funnels the whole expression back through its call check, which only
/// fires when the term is the call's lone argument, so the same restriction applies.
pub(crate) fn translation_not_lazy_binop(checker: &Checker, binop: &ast::ExprBinOp) {
    if !checker.is_rule_enabled(Rule::TranslationNotLazy) || !translation_family_applies(checker) {
        return;
    }
    if binop.op != Operator::Mod {
        return;
    }
    let Expr::Call(call) = binop.left.as_ref() else {
        return;
    };
    if !is_translation_underscore(&call.func) {
        return;
    }
    let [term] = &*call.arguments.args else {
        return;
    };
    if !call.arguments.keywords.is_empty() || term.is_starred_expr() {
        return;
    }
    let mut diagnostic = checker.report_diagnostic(TranslationNotLazy, binop.range());
    let literal_term = match term {
        Expr::StringLiteral(literal) => Some(literal),
        _ => None,
    };
    if let Some(arguments) = literal_term
        .map_or_else(
            // Without a literal term the placeholders are unknown; only tuple/dict
            // values convert faithfully (Odoo formats them exactly like `%` does).
            || tuple_or_dict_arguments(checker, &binop.right),
            |term| lazy_arguments(checker, term, &binop.right),
        )
        .filter(|_| !checker.comment_ranges().intersects(binop.range()))
    {
        let call_source = checker
            .locator()
            .slice(TextRange::new(call.start(), term.end()));
        diagnostic.set_fix(Fix::unsafe_edit(Edit::range_replacement(
            format!("{call_source}, {arguments})"),
            binop.range(),
        )));
    }
}

/// Renders the interpolated values as translation-call arguments, or `None` when they
/// can't be moved faithfully.
fn lazy_arguments(checker: &Checker, term: &ast::ExprStringLiteral, rhs: &Expr) -> Option<String> {
    if let Some(arguments) = tuple_or_dict_arguments(checker, rhs) {
        return Some(arguments);
    }
    if matches!(rhs, Expr::Tuple(_) | Expr::Dict(_)) {
        return None;
    }
    // A single non-tuple/dict value only converts when the term takes positional
    // conversions: Odoo wraps positional arguments in a tuple before formatting, and
    // `"%(name)s" % (value,)` no longer resolves a mapping key.
    match parse_printf_format(term.value.to_str()) {
        PrintfFormat::Parsed {
            has_keywords: false,
            required_args,
            ..
        } if required_args > 0 => Some(checker.locator().slice(rhs.range()).to_string()),
        _ => None,
    }
}

/// Renders a tuple as positional arguments or a dict literal as keyword arguments;
/// `None` for anything else (or when a dict key can't become a keyword).
fn tuple_or_dict_arguments(checker: &Checker, rhs: &Expr) -> Option<String> {
    let locator = checker.locator();
    match rhs {
        Expr::Tuple(tuple) => {
            if tuple.elts.is_empty() || tuple.elts.iter().any(Expr::is_starred_expr) {
                return None;
            }
            Some(
                tuple
                    .elts
                    .iter()
                    .map(|element| locator.slice(element.range()))
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        }
        Expr::Dict(dict) => {
            let mut seen = Vec::with_capacity(dict.items.len());
            let mut rendered = Vec::with_capacity(dict.items.len());
            for item in &dict.items {
                let Some(Expr::StringLiteral(key)) = &item.key else {
                    return None;
                };
                let key = key.value.to_str();
                // The name must survive as a Python keyword argument, and `source` is
                // taken by the translation function's own first parameter.
                if !is_identifier(key) || is_keyword(key) || key == "source" {
                    return None;
                }
                if seen.contains(&key.to_string()) {
                    return None;
                }
                seen.push(key.to_string());
                rendered.push(format!("{key}={}", locator.slice(item.value.range())));
            }
            if rendered.is_empty() {
                return None;
            }
            Some(rendered.join(", "))
        }
        _ => None,
    }
}

/// Returns `true` if nothing can interpolate the translated term after the call returns.
///
/// pylint-odoo exempts a translation call given no values of its own, because the term is then
/// used verbatim (`_("100%")`) or interpolated afterwards (`_("Hello %s") % name`). That
/// reasoning stops holding when the call is the lone argument of the exception being raised:
/// the string goes straight into the exception, so a placeholder in it is never filled.
fn is_lone_argument_of_raised_exception(checker: &Checker, call: &ast::ExprCall) -> bool {
    let Some(Expr::Call(exception)) = checker.semantic().current_expression_parent() else {
        return false;
    };
    if !exception.arguments.keywords.is_empty()
        || !matches!(&*exception.arguments.args, [Expr::Call(argument)] if argument.range() == call.range())
    {
        return false;
    }
    matches!(
        checker.semantic().current_statement(),
        Stmt::Raise(ast::StmtRaise { exc: Some(exc), .. }) if exc.range() == exception.range()
    )
}

/// ODE8301, ODE8306, ODE8305, ODE8300: checks a literal term's printf placeholders
/// against the values supplied to the translation call, mirroring pylint's
/// `_check_format_string` plus pylint-odoo's no-arguments exemption (a term without
/// supplied values is used verbatim, so `_("100%")` is fine).
///
/// The exemption is lifted for the one shape where the term can not be interpolated later
/// either -- the lone argument of a raised exception -- and there only the count of
/// placeholders is checked. A malformed conversion is a diagnostic about `%` formatting that
/// never runs, and `"50%"` or `"100% off"` is prose in a user-facing message far more often
/// than a truncated or space-flagged conversion.
fn check_format_string(checker: &Checker, call: &ast::ExprCall, term: &ast::ExprStringLiteral) {
    let num_supplied = call.arguments.args.len() - 1;
    let uninterpolated = num_supplied == 0;
    if uninterpolated && !is_lone_argument_of_raised_exception(checker, call) {
        return;
    }
    let text = term.value.to_str();
    match parse_printf_format(text) {
        PrintfFormat::UnsupportedChar(index) => {
            if !uninterpolated
                && checker.is_rule_enabled(Rule::TranslationUnsupportedFormat)
                && let Some(unsupported_char) = text.chars().nth(index)
            {
                checker.report_diagnostic(
                    TranslationUnsupportedFormat {
                        unsupported_char,
                        index,
                    },
                    call.range(),
                );
            }
        }
        PrintfFormat::Truncated => {
            if !uninterpolated && checker.is_rule_enabled(Rule::TranslationFormatTruncated) {
                checker.report_diagnostic(TranslationFormatTruncated, call.range());
            }
        }
        PrintfFormat::Parsed {
            has_keywords: true, ..
        } => {
            // Mapping keys pair with keyword arguments, whose special names make the
            // check ambiguous: out of scope, as in pylint.
        }
        PrintfFormat::Parsed {
            has_keywords: false,
            required_args,
            unambiguous_args,
        } => {
            // Nothing supplies the values, so the count only means something when at least one
            // placeholder is unmistakably one.
            let required_args = if uninterpolated && unambiguous_args == 0 {
                0
            } else {
                required_args
            };
            if num_supplied > required_args {
                if checker.is_rule_enabled(Rule::TranslationTooManyArgs) {
                    checker.report_diagnostic(TranslationTooManyArgs, call.range());
                }
            } else if num_supplied < required_args
                && checker.is_rule_enabled(Rule::TranslationTooFewArgs)
            {
                checker.report_diagnostic(TranslationTooFewArgs, call.range());
            }
        }
    }
}
