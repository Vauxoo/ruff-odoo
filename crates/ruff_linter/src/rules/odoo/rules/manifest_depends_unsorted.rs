use std::path::Path;

use ruff_macros::{ViolationMetadata, derive_message_formats};
use ruff_python_ast::{self as ast, Expr};
use ruff_python_trivia::leading_indentation;
use ruff_source_file::LineRanges;
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::checkers::ast::Checker;
use crate::rules::odoo::helpers::{is_manifest_root_dict, manifest_item};
use crate::{Edit, Fix, FixAvailability, Violation};

/// ## What it does
/// Checks for the `depends` key of an Odoo module's `__manifest__.py` listing its modules
/// in an order other than alphabetical.
///
/// ## Why is this bad?
/// Odoo resolves the dependency graph itself, so the order of `depends` carries no
/// meaning: it is purely a reading aid. Keeping the list alphabetical makes a module easy
/// to look up by eye, and keeps merge conflicts down — two branches adding a dependency
/// both append to the end of an unsorted list and collide on the same line, while a sorted
/// list spreads the additions out.
///
/// Entries are ordered lexicographically, the order Python's own `sorted()` gives, so
/// `sale_stock` comes before `salemodule`.
///
/// ## Example
/// ```python
/// {
///     "depends": [
///         "sale",
///         # needed for the analytic distribution widget
///         "account",
///         "base",
///     ],
/// }
/// ```
///
/// Use instead:
/// ```python
/// {
///     "depends": [
///         # needed for the analytic distribution widget
///         "account",
///         "base",
///         "sale",
///     ],
/// }
/// ```
///
/// ## Fix safety
/// A list written across several lines is rewritten one entry per line, always with a
/// trailing comma, whatever layout it had before. A comment written on its own line above
/// an entry travels with that entry, and so does a comment trailing it on the same line;
/// no comment is ever dropped. A list written entirely on one line is only reordered — its
/// layout and its lack of a trailing comma are left alone.
///
/// Note that a comment introducing a *group* of entries moves with the single entry right
/// below it, which is rarely what a group header means — review the fix when the list is
/// organized in commented sections.
///
/// No fix is offered when the list cannot be rewritten without guessing:
/// - a blank line groups the entries, and reordering across it would scramble the grouping;
/// - a line holds several entries *and* a trailing comment, so there is no telling which
///   entry the comment belongs to;
/// - an entry spans more than one line (implicit concatenation).
#[derive(ViolationMetadata)]
#[violation_metadata(preview_since = "0.16.3.33")]
pub(crate) struct ManifestDependsUnsorted;

impl Violation for ManifestDependsUnsorted {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    #[derive_message_formats]
    fn message(&self) -> String {
        r#"Manifest key "depends" is not sorted alphabetically"#.to_string()
    }

    fn fix_title(&self) -> Option<String> {
        Some(r#"Sort "depends" alphabetically"#.to_string())
    }
}

/// ODC9501
pub(crate) fn manifest_depends_unsorted(checker: &Checker, dict: &ast::ExprDict, path: &Path) {
    if !is_manifest_root_dict(checker, dict, path) {
        return;
    }

    let Some((key, value)) = manifest_item(dict, "depends") else {
        return;
    };
    let Expr::List(list) = value else {
        return;
    };

    // Every entry has to be a plain string literal for "alphabetical" to mean anything.
    let mut modules = Vec::with_capacity(list.elts.len());
    for element in &list.elts {
        let Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) = element else {
            return;
        };
        modules.push(value.to_str());
    }
    if modules.is_sorted() {
        return;
    }

    // The permutation that sorts the entries. `sort_by_key` is stable, so duplicated module
    // names keep their relative order.
    let mut order: Vec<usize> = (0..modules.len()).collect();
    order.sort_by_key(|&index| modules[index]);

    let mut diagnostic = checker.report_diagnostic(ManifestDependsUnsorted, key.range());
    let locator = checker.locator();
    let fix = if locator.line_start(list.start()) == locator.line_start(list.end()) {
        sort_single_line(checker, list, &order)
    } else {
        rewrite_multiline(checker, list, &order)
    };
    if let Some(fix) = fix {
        diagnostic.set_fix(fix);
    }
}

/// Reorders a list written entirely on one line by swapping the entry sources in place.
///
/// No comment can be attached to an entry there — a `#` would swallow the closing bracket —
/// so nothing but the entries themselves has to move, and the result is a permutation of
/// the same text on the same line: it cannot make that line any longer, and it leaves the
/// list's layout, and its lack of a trailing comma, exactly as the author wrote them.
fn sort_single_line(checker: &Checker, list: &ast::ExprList, order: &[usize]) -> Option<Fix> {
    let locator = checker.locator();
    let mut edits = order
        .iter()
        .enumerate()
        .filter(|&(position, &index)| position != index)
        .map(|(position, &index)| {
            Edit::range_replacement(
                locator.slice(list.elts[index].range()).to_string(),
                list.elts[position].range(),
            )
        });
    // The list is known to be unsorted, so at least two entries move.
    let first = edits.next()?;
    Some(Fix::safe_edits(first, edits))
}

/// Rewrites a list spanning several lines as one entry per line, sorted, each entry
/// followed by a comma and by whatever comments belong to it.
///
/// Returns `None` when the list cannot be rewritten without guessing what the author meant;
/// the diagnostic is then reported without a fix.
fn rewrite_multiline(checker: &Checker, list: &ast::ExprList, order: &[usize]) -> Option<Fix> {
    let locator = checker.locator();

    // A blank line groups the entries. Sorting across it would scramble the grouping, and
    // rewriting the list would drop the line altogether, so leave the whole thing alone.
    if locator
        .slice(list.range())
        .lines()
        .any(|line| line.trim().is_empty())
    {
        return None;
    }

    // Every entry has to sit on a single line for "the comment trailing it" to mean
    // anything.
    let entry_lines: Vec<TextSize> = list
        .elts
        .iter()
        .map(|element| locator.line_start(element.start()))
        .collect();
    for (element, &line) in list.elts.iter().zip(&entry_lines) {
        if locator.line_start(element.end()) != line {
            return None;
        }
    }

    // Attribute every comment inside the brackets to the entry it belongs to. A comment
    // runs to the end of its line, so a line holds at most one — either trailing the last
    // entry that starts on it, or, when no entry does, standing on its own line.
    let mut leading: Vec<Vec<TextRange>> = vec![Vec::new(); list.elts.len()];
    let mut trailing: Vec<Option<TextRange>> = vec![None; list.elts.len()];
    let mut bracket_comment = None;
    let mut tail = Vec::new();
    for &comment in checker.comment_ranges().comments_in_range(list.range()) {
        let comment_line = locator.line_start(comment.start());
        if locator
            .slice(TextRange::new(comment_line, comment.start()))
            .trim()
            .is_empty()
        {
            // A comment on a line of its own belongs to the next entry. Comment lines can
            // only be followed by more comment lines or by an entry — blank lines are out
            // above, and nothing else lives between the brackets — so "the next entry" is
            // never separated from it by anything the author meant to keep them apart.
            match list
                .elts
                .iter()
                .position(|element| element.start() > comment.end())
            {
                Some(index) => leading[index].push(comment),
                None => tail.push(comment),
            }
            continue;
        }
        let on_this_line: Vec<usize> = entry_lines
            .iter()
            .enumerate()
            .filter(|&(_, &line)| line == comment_line)
            .map(|(index, _)| index)
            .collect();
        match on_this_line[..] {
            // The opening bracket's own line, with the bracket alone on it: the comment
            // annotates the list rather than any one entry, so it stays put.
            [] if comment_line == locator.line_start(list.start()) => {
                bracket_comment = Some(comment);
            }
            [] => return None,
            [index] => trailing[index] = Some(comment),
            // Several entries share the line: there is no telling which one the comment
            // was written about.
            _ => return None,
        }
    }

    let line_start = locator.line_start(list.start());
    let outer_indent =
        leading_indentation(locator.slice(TextRange::new(line_start, list.start()))).to_string();
    let entry_indent = format!("{outer_indent}{}", checker.stylist().indentation().as_str());
    let line_ending = checker.stylist().line_ending().as_str();

    let mut rewritten = String::from("[");
    if let Some(comment) = bracket_comment {
        rewritten.push_str("  ");
        rewritten.push_str(locator.slice(comment));
    }
    rewritten.push_str(line_ending);
    for &index in order {
        for &comment in &leading[index] {
            rewritten.push_str(&entry_indent);
            rewritten.push_str(locator.slice(comment));
            rewritten.push_str(line_ending);
        }
        rewritten.push_str(&entry_indent);
        rewritten.push_str(locator.slice(list.elts[index].range()));
        rewritten.push(',');
        if let Some(comment) = trailing[index] {
            rewritten.push_str("  ");
            rewritten.push_str(locator.slice(comment));
        }
        rewritten.push_str(line_ending);
    }
    for comment in tail {
        rewritten.push_str(&entry_indent);
        rewritten.push_str(locator.slice(comment));
        rewritten.push_str(line_ending);
    }
    rewritten.push_str(&outer_indent);
    rewritten.push(']');

    Some(Fix::safe_edit(Edit::range_replacement(
        rewritten,
        list.range(),
    )))
}
