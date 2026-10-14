//! Generate a Markdown-compatible table of supported lint rules.
//!
//! Used for <https://docs.astral.sh/ruff/rules/>.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;

use itertools::Itertools;
use strum::IntoEnumIterator;

use ruff_linter::FixAvailability;
use ruff_linter::codes::{Category, RuleStatus};
use ruff_linter::registry::{Linter, Rule, RuleNamespace};
use ruff_linter::settings::LinterSettings;
use ruff_linter::settings::rule_table::RuleTable;
use ruff_linter::upstream_categories::UpstreamCategoryAndPrefix;
use ruff_options_metadata::OptionsMetadata;
use ruff_workspace::options::Options;

const DEFAULT_SYMBOL: &str = "✅";
const FIX_SYMBOL: &str = "🛠️";
const PREVIEW_SYMBOL: &str = "🧪";
const REMOVED_SYMBOL: &str = "❌";
const WARNING_SYMBOL: &str = "⚠️";
const SPACER: &str = "&nbsp;&nbsp;&nbsp;&nbsp;";

/// Style for the rule's default selection, fixability, and status icons.
const SYMBOL_STYLE: &str = "style='width: 1em; display: inline-block;'";
/// Style for the container wrapping the default selection, fixability, and status icons.
/// Keep the absolutely positioned screen-reader labels inside the table's scroll area.
const SYMBOLS_CONTAINER: &str =
    "style='position: relative; display: flex; gap: 0.5rem; justify-content: end;'";

fn generate_table(
    table_out: &mut String,
    rules: impl IntoIterator<Item = Rule>,
    default_rules: &RuleTable,
) {
    let table_start = table_out.len();
    table_out.push('\n');
    let default_categories = Category::default_categories().iter().join(" ");
    let _ = writeln!(
        table_out,
        "| Code {{ scope='col' .rule-code }} \
         | Rule {{ scope='col' .rule-identity }} \
         | Category {{ scope='col' .rule-category data-default-categories='{default_categories}' }} \
         | Linter {{ scope='col' .rule-linter }} \
         | Status {{ scope='col' .rule-status aria-label='Status, fix availability, and default selection' }} |"
    );
    table_out.push_str("| ---- | ---- | -------- | ------ | -: |");
    table_out.push('\n');
    let mut seen_anchors = HashSet::new();
    let mut linters = BTreeMap::new();
    for rule in rules {
        let status = rule.status();
        let status_token = match status {
            RuleStatus::Removed { since } => {
                format!(
                    "<span aria-hidden='true' {SYMBOL_STYLE} title='Rule was removed in {since}'>{REMOVED_SYMBOL}</span><span class='sr-only'>Rule was removed in {since}</span>"
                )
            }
            RuleStatus::Deprecated { since } => {
                format!(
                    "<span aria-hidden='true' {SYMBOL_STYLE} title='Rule has been deprecated since {since}'>{WARNING_SYMBOL}</span><span class='sr-only'>Rule has been deprecated since {since}</span>"
                )
            }
            RuleStatus::Preview { since } => {
                format!(
                    "<span aria-hidden='true' {SYMBOL_STYLE} title='Rule has been in preview since {since}'>{PREVIEW_SYMBOL}</span><span class='sr-only'>Rule has been in preview since {since}</span>"
                )
            }
            RuleStatus::Stable { since } => {
                format!(
                    "<span aria-hidden='true' {SYMBOL_STYLE} title='Rule has been stable since {since}'></span><span class='sr-only'>Rule has been stable since {since}</span>"
                )
            }
        };

        let fix_token = if matches!(
            rule.fixable(),
            FixAvailability::Always | FixAvailability::Sometimes
        ) {
            format!(
                "<span aria-hidden='true' {SYMBOL_STYLE} title='Automatic fix available'>{FIX_SYMBOL}</span><span class='sr-only'>Automatic fix available</span>"
            )
        } else {
            format!("<span {SYMBOL_STYLE}></span>")
        };

        let default_token = if default_rules.enabled(rule) {
            format!(
                "<span aria-hidden='true' {SYMBOL_STYLE} title='Enabled by default'>{DEFAULT_SYMBOL}</span><span class='sr-only'>Enabled by default</span>"
            )
        } else {
            format!("<span {SYMBOL_STYLE}></span>")
        };

        let rule_name = rule.name();

        let message = rule.message_formats()[0];

        // Start and end of style spans
        let mut ss = "";
        let mut se = "";
        if rule.is_removed() {
            ss = "<span style='opacity: 0.5', title='This rule has been removed'>";
            se = "</span>";
        } else if rule.is_deprecated() {
            ss = "<span style='opacity: 0.8', title='This rule has been deprecated'>";
            se = "</span>";
        }

        let code = rule.noqa_code().map(|code| code.to_string());
        let linter = code
            .as_deref()
            .and_then(|code| Linter::parse_code(code).map(|(linter, _)| linter));
        let (linter_name, linter_slug, linter_label) = if let Some(linter) = &linter {
            let prefixes = match linter.common_prefix() {
                "" => linter
                    .upstream_categories()
                    .into_iter()
                    .flatten()
                    .map(|category| category.prefix)
                    .join(", "),
                prefix => prefix.to_string(),
            };
            // The linter names for Ruff and NumPy are suffixed with "-specific rules."
            let name = linter.name().trim_end_matches("-specific rules");
            let slug = format!(
                "{}-{}",
                linter.name().to_lowercase().replace(' ', "-"),
                prefixes.to_lowercase().replace(", ", "-"),
            );
            (name, slug, format!("{name} ({prefixes})"))
        } else {
            (
                "—",
                "rules-without-codes".to_string(),
                "No originating linter".to_string(),
            )
        };
        // Preserve links to the old linter headings on their first table row.
        let linter_anchor = if seen_anchors.insert(linter_slug.clone()) {
            linters.insert(
                linter_label.to_lowercase(),
                (linter_slug.clone(), linter_label.clone()),
            );
            format!("#{linter_slug}")
        } else {
            String::new()
        };

        if let Some(code) = code {
            table_out.push_str("| ");
            // The former subgroups have separate anchors in addition to each rule's code.
            if let Some(linter) = linter
                && let Some(category) = rule.upstream_category(&linter)
            {
                let slug = format!(
                    "{}-{}{}",
                    category.category.to_lowercase().replace(' ', "-"),
                    linter.common_prefix().to_lowercase(),
                    category.prefix.to_lowercase(),
                );
                if seen_anchors.insert(slug.clone()) {
                    let _ = write!(table_out, "<span id='{slug}'></span>");
                }
            }
            let _ = write!(table_out, "{ss}`{code}`{se} {{ #{code} .rule-code }} ");
        } else {
            let _ = write!(table_out, "| {ss}—{se} {{ .rule-code }} ");
        }

        // Message placeholders can be mistaken for an attribute list on the table cell.
        // Keep the identity class on an HTML wrapper so messages retain their Markdown.
        #[expect(clippy::or_fun_call)]
        let _ = write!(
            table_out,
            "| <span class='rule-identity'>{ss}{explanation}<br>{message}{se}</span> \
             | {ss}{category}{se} {{ .rule-category }} \
             | {ss}{linter_name}{se} {{ {linter_anchor} .rule-linter data-linter='{linter_slug}' }} \
             | <div {SYMBOLS_CONTAINER}>{status_token}{fix_token}{default_token}</div> {{ .rule-status data-status='{status}' }} |",
            category = rule.category(),
            explanation = rule
                .explanation()
                .is_some()
                .then_some(format_args!("[`{rule_name}`](rules/{rule_name}.md)"))
                .unwrap_or(format_args!("`{rule_name}`")),
        );
        table_out.push('\n');
    }
    table_out.push('\n');

    // Reuse the table's linter labels and slugs for the filter options.
    let category_options = Category::iter()
        .map(|category| {
            format!("<label><input type='checkbox' name='category' value='{category}' checked> {category}</label>")
        })
        .join("\n");
    let linter_options = linters
        .values()
        .map(|(value, label)| {
            format!("<label><input type='checkbox' name='linter' value='{value}' checked> {label}</label>")
        })
        .join("\n");
    table_out.insert_str(
        table_start,
        &format!(
            include_str!("../../../docs/.overrides/partials/rule-filters.html"),
            category_options = category_options,
            linter_options = linter_options,
        ),
    );
}

pub(crate) fn generate() -> String {
    // Generate the table string.
    let mut table_out = String::new();

    table_out.push_str("<details markdown=\"1\">\n<summary>Rule legend</summary>\n\n### Legend");
    table_out.push('\n');

    let _ = write!(
        &mut table_out,
        "{SPACER}{PREVIEW_SYMBOL}{SPACER} The rule is unstable and is in [\"preview\"](faq.md#what-is-preview)."
    );
    table_out.push_str("<br />");

    let _ = write!(
        &mut table_out,
        "{SPACER}{WARNING_SYMBOL}{SPACER} The rule has been deprecated and will be removed in a future release."
    );
    table_out.push_str("<br />");

    let _ = write!(
        &mut table_out,
        "{SPACER}{REMOVED_SYMBOL}{SPACER} The rule has been removed only the documentation is available."
    );
    table_out.push_str("<br />");

    let _ = write!(
        &mut table_out,
        "{SPACER}{FIX_SYMBOL}{SPACER} The rule is automatically fixable by the `--fix` command-line option."
    );
    table_out.push_str("<br />");

    let _ = write!(
        &mut table_out,
        "{SPACER}{DEFAULT_SYMBOL}{SPACER} The rule is enabled by default."
    );
    table_out.push_str("\n\n");
    table_out.push_str("All rules not marked as preview, deprecated or removed are stable.");
    table_out.push_str("\n\n</details>\n\n");

    generate_table(
        &mut table_out,
        Linter::iter()
            .flat_map(|linter| linter.all_rules())
            .chain(Rule::iter().filter(|rule| rule.noqa_code().is_none())),
        &LinterSettings::default().rules,
    );

    table_out
}

// Vauxoo fork: the `docs-odoo` site renders only the `OD`/`OAPP` rules, one section per
// linter, the way upstream's rules page looked before it became a single filterable
// table. That table depends on the scripts and styles under `docs/.overrides`, which the
// smaller site does not ship, so the per-linter layout lives on here for it alone.

/// Write the legend explaining the status and fixability icons used in the tables.
///
/// `preview_link` is the link target for the word "preview"; it varies because the
/// page explaining preview mode is not in the same place in every docs site this
/// table is rendered into.
pub(crate) fn generate_legend(table_out: &mut String, preview_link: &str) {
    table_out.push_str("### Legend");
    table_out.push('\n');

    let _ = write!(
        table_out,
        "{SPACER}{PREVIEW_SYMBOL}{SPACER} The rule is unstable and is in [\"preview\"]({preview_link})."
    );
    table_out.push_str("<br />");

    let _ = write!(
        table_out,
        "{SPACER}{WARNING_SYMBOL}{SPACER} The rule has been deprecated and will be removed in a future release."
    );
    table_out.push_str("<br />");

    let _ = write!(
        table_out,
        "{SPACER}{REMOVED_SYMBOL}{SPACER} The rule has been removed only the documentation is available."
    );
    table_out.push_str("<br />");

    let _ = write!(
        table_out,
        "{SPACER}{FIX_SYMBOL}{SPACER} The rule is automatically fixable by the `--fix` command-line option."
    );
    table_out.push_str("\n\n");
    table_out.push_str("All rules not marked as preview, deprecated or removed are stable.");
    table_out.push('\n');
}

/// Write the `### <linter>` heading for `linter`, along with its rule table(s).
pub(crate) fn generate_linter_section(table_out: &mut String, linter: &Linter) {
    let codes_csv: String = match linter.common_prefix() {
        "" => linter
            .upstream_categories()
            .unwrap()
            .iter()
            .map(|c| c.prefix)
            .join(", "),
        prefix => prefix.to_string(),
    };
    let _ = write!(table_out, "### {} ({codes_csv})", linter.name());
    table_out.push('\n');
    table_out.push('\n');

    if let Some(url) = linter.url() {
        let host = url
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap();
        let _ = write!(
            table_out,
            "For more, see [{}]({}) on {}.",
            linter.name(),
            url,
            match host {
                "pypi.org" => "PyPI",
                "github.com" => "GitHub",
                host => panic!(
                    "unexpected host in URL of {}, expected pypi.org or github.com but found \
                     {host}",
                    linter.name()
                ),
            }
        );
        table_out.push('\n');
        table_out.push('\n');
    }

    // The linter names for Ruff and NumPy are suffixed with "-specific rules."
    let linter_name = linter.name().trim_end_matches("-specific rules");
    // Several linter names are capitalized, but their settings are not.
    let linter_name_lower = linter_name.to_lowercase();
    if Options::metadata().has(&format!("lint.{linter_name_lower}")) {
        let _ = write!(
            table_out,
            "For related settings, see [{linter_name}](settings.md#lint{linter_name_lower}).",
        );
        table_out.push('\n');
        table_out.push('\n');
    }

    let rules_by_upstream_category = linter
        .all_rules()
        .map(|rule| (rule.upstream_category(linter), rule))
        .into_group_map();

    let mut rules_by_upstream_category: Vec<_> = rules_by_upstream_category.iter().collect();

    // Sort the upstream categories alphabetically by prefix.
    rules_by_upstream_category.sort_by(|(a, _), (b, _)| {
        a.as_ref()
            .map(|category| category.prefix)
            .unwrap_or_default()
            .cmp(
                b.as_ref()
                    .map(|category| category.prefix)
                    .unwrap_or_default(),
            )
    });

    if rules_by_upstream_category.len() > 1 {
        for (opt, rules) in rules_by_upstream_category {
            if opt.is_some() {
                let UpstreamCategoryAndPrefix { category, prefix } = opt.unwrap();
                match codes_csv.as_str() {
                    "PL" => {
                        let _ = write!(table_out, "#### {category} ({codes_csv}{prefix})");
                    }
                    _ => {
                        let _ = write!(table_out, "#### {category} ({prefix})");
                    }
                }
            }
            table_out.push('\n');
            table_out.push('\n');
            generate_linter_table(table_out, rules.clone(), linter);
        }
    } else {
        generate_linter_table(table_out, linter.all_rules(), linter);
    }
}

/// Write one linter's rules as a code/name/message table.
fn generate_linter_table(
    table_out: &mut String,
    rules: impl IntoIterator<Item = Rule>,
    linter: &Linter,
) {
    table_out.push_str("| Code { scope='col' } | Name { scope='col' } | Message { scope='col' } | Fix/Status { scope='col' .sr-only } |");
    table_out.push('\n');
    table_out.push_str("| ---- | ---- | ------- | -: |");
    table_out.push('\n');
    for rule in rules {
        let status_token = match rule.status() {
            RuleStatus::Removed { since } => {
                format!(
                    "<span aria-hidden='true' {SYMBOL_STYLE} title='Rule was removed in {since}'>{REMOVED_SYMBOL}</span><span class='sr-only'>Rule was removed in {since}</span>"
                )
            }
            RuleStatus::Deprecated { since } => {
                format!(
                    "<span aria-hidden='true' {SYMBOL_STYLE} title='Rule has been deprecated since {since}'>{WARNING_SYMBOL}</span><span class='sr-only'>Rule has been deprecated since {since}</span>"
                )
            }
            RuleStatus::Preview { since } => {
                format!(
                    "<span aria-hidden='true' {SYMBOL_STYLE} title='Rule has been in preview since {since}'>{PREVIEW_SYMBOL}</span><span class='sr-only'>Rule has been in preview since {since}</span>"
                )
            }
            RuleStatus::Stable { since } => {
                format!(
                    "<span aria-hidden='true' {SYMBOL_STYLE} title='Rule has been stable since {since}'></span><span class='sr-only'>Rule has been stable since {since}</span>"
                )
            }
        };

        let fix_token = match rule.fixable() {
            FixAvailability::Always | FixAvailability::Sometimes => {
                format!(
                    "<span aria-hidden='true' {SYMBOL_STYLE} title='Automatic fix available'>{FIX_SYMBOL}</span><span class='sr-only'>Automatic fix available</span>"
                )
            }
            FixAvailability::None => format!("<span {SYMBOL_STYLE}></span>"),
        };

        let rule_name = rule.name();

        // If the message ends in a bracketed expression (like: "Use {replacement}"), escape the
        // brackets. Otherwise, it'll be interpreted as an HTML attribute via the `attr_list`
        // plugin. (Above, we'd convert to "Use {replacement\}".)
        let message = rule.message_formats()[0];
        let message = if let Some(prefix) = message.strip_suffix('}') {
            Cow::Owned(format!("{prefix}\\}}"))
        } else {
            Cow::Borrowed(message)
        };

        // Start and end of style spans
        let mut ss = "";
        let mut se = "";
        if rule.is_removed() {
            ss = "<span style='opacity: 0.5', title='This rule has been removed'>";
            se = "</span>";
        } else if rule.is_deprecated() {
            ss = "<span style='opacity: 0.8', title='This rule has been deprecated'>";
            se = "</span>";
        }

        #[expect(clippy::or_fun_call)]
        let _ = write!(
            table_out,
            "| {ss}{prefix}{code}{se} {{ #{prefix}{code} }} | {ss}{explanation}{se} | {ss}{message}{se} | <div {SYMBOLS_CONTAINER}>{status_token}{fix_token}</div>|",
            prefix = linter.common_prefix(),
            code = linter.code_for_rule(rule).unwrap(),
            explanation = rule
                .explanation()
                .is_some()
                .then_some(format_args!("[{rule_name}](rules/{rule_name}.md)"))
                .unwrap_or(format_args!("{rule_name}")),
        );
        table_out.push('\n');
    }
    table_out.push('\n');
}
