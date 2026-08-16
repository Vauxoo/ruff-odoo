//! Generate a Markdown-compatible listing of configuration options for `pyproject.toml`.
//!
//! Used for <https://docs.astral.sh/ruff/settings/>.
use itertools::Itertools;
use std::fmt::Write;

use ruff_options_metadata::{OptionEntry, OptionField, OptionSet, OptionsMetadata, Visit};
use ruff_python_trivia::textwrap;
use ruff_workspace::options::Options;

pub(crate) fn generate() -> String {
    let mut output = String::new();

    generate_set(
        &mut output,
        Set::Toplevel(Options::metadata()),
        &mut Vec::new(),
    );

    output
}

/// Render the documentation for a single group of options, e.g. `lint.odoo`, exactly as
/// it appears within the full settings page — same headings, same anchors.
///
/// The fork's `OD`/`OAPP` docs site documents only the settings its own rules read;
/// every other Ruff option stays documented upstream. Returns `None` if `path` doesn't
/// name a group.
pub(crate) fn generate_group(path: &str) -> Option<String> {
    let mut parents = Vec::new();
    let mut group = None;
    let mut prefix = String::new();

    let mut segments = path.split('.').peekable();
    while let Some(segment) = segments.next() {
        if !prefix.is_empty() {
            prefix.push('.');
        }
        prefix.push_str(segment);

        let OptionEntry::Set(set) = Options::metadata().find(&prefix)? else {
            return None;
        };
        let set = Set::Named {
            name: segment.to_owned(),
            set,
        };

        if segments.peek().is_some() {
            parents.push(set);
        } else {
            group = Some(set);
        }
    }

    let mut output = String::new();
    generate_set(&mut output, group?, &mut parents);
    Some(output)
}

fn generate_set(output: &mut String, set: Set, parents: &mut Vec<Set>) {
    match &set {
        Set::Toplevel(_) => {
            output.push_str("### Top-level\n");
        }
        Set::Named { name, .. } => {
            let title = parents
                .iter()
                .filter_map(|set| set.name())
                .chain(std::iter::once(name.as_str()))
                .join(".");
            writeln!(output, "#### `{title}`\n").unwrap();
        }
    }

    if let Some(documentation) = set.metadata().documentation() {
        output.push_str(documentation);
        output.push('\n');
        output.push('\n');
    }

    let mut visitor = CollectOptionsVisitor::default();
    set.metadata().record(&mut visitor);

    let (mut fields, mut sets) = (visitor.fields, visitor.groups);

    fields.sort_unstable_by(|(name, _), (name2, _)| name.cmp(name2));
    sets.sort_unstable_by(|(name, _), (name2, _)| name.cmp(name2));

    parents.push(set);

    // Generate the fields.
    for (name, field) in &fields {
        emit_field(output, name, field, parents.as_slice());
        output.push_str("---\n\n");
    }

    // Generate all the sub-sets.
    for (set_name, sub_set) in &sets {
        generate_set(
            output,
            Set::Named {
                name: set_name.clone(),
                set: *sub_set,
            },
            parents,
        );
    }

    parents.pop();
}

enum Set {
    Toplevel(OptionSet),
    Named { name: String, set: OptionSet },
}

impl Set {
    fn name(&self) -> Option<&str> {
        match self {
            Set::Toplevel(_) => None,
            Set::Named { name, .. } => Some(name),
        }
    }

    fn metadata(&self) -> &OptionSet {
        match self {
            Set::Toplevel(set) => set,
            Set::Named { set, .. } => set,
        }
    }
}

fn emit_field(output: &mut String, name: &str, field: &OptionField, parents: &[Set]) {
    let header_level = if parents.is_empty() { "####" } else { "#####" };
    let parents_anchor = parents.iter().filter_map(|parent| parent.name()).join("_");

    if parents_anchor.is_empty() {
        let _ = writeln!(output, "{header_level} [`{name}`](#{name}) {{: #{name} }}");
    } else {
        let _ = writeln!(
            output,
            "{header_level} [`{name}`](#{parents_anchor}_{name}) {{: #{parents_anchor}_{name} }}"
        );

        // the anchor used to just be the name, but now it's the group name
        // for backwards compatibility, we need to keep the old anchor
        let _ = writeln!(output, "<span id=\"{name}\"></span>");
    }

    output.push('\n');

    if let Some(deprecated) = &field.deprecated {
        output.push_str("!!! warning \"Deprecated\"\n");
        output.push_str("    This option has been deprecated");

        if let Some(since) = deprecated.since {
            write!(output, " in {since}").unwrap();
        }

        output.push('.');

        if let Some(message) = deprecated.message {
            writeln!(output, " {message}").unwrap();
        }

        output.push('\n');
    }

    output.push_str(field.doc);
    output.push_str("\n\n");
    if parents_anchor == "lint" && name == "select" {
        output.push_str("**Default value**: See [Default Rules](default-rules.md).\n");
    } else {
        let _ = writeln!(output, "**Default value**: `{}`", field.default);
    }
    output.push('\n');
    let _ = writeln!(output, "**Type**: `{}`", field.value_type);
    output.push('\n');
    output.push_str("**Example usage**:\n\n");
    output.push_str(&format_tab(
        "pyproject.toml",
        &format_header(field.scope, parents, ConfigurationFile::PyprojectToml),
        field.example,
    ));
    output.push_str(&format_tab(
        "ruff.toml",
        &format_header(field.scope, parents, ConfigurationFile::RuffToml),
        field.example,
    ));
    output.push('\n');
}

fn format_tab(tab_name: &str, header: &str, content: &str) -> String {
    format!(
        "=== \"{}\"\n\n    ```toml\n    {}\n{}\n    ```\n",
        tab_name,
        header,
        textwrap::indent(content, "    ")
    )
}

/// Format the TOML header for the example usage for a given option.
///
/// For example: `[tool.ruff.format]` or `[tool.ruff.lint.isort]`.
fn format_header(scope: Option<&str>, parents: &[Set], configuration: ConfigurationFile) -> String {
    let tool_parent = match configuration {
        ConfigurationFile::PyprojectToml => Some("tool.ruff"),
        ConfigurationFile::RuffToml => None,
    };

    let header = tool_parent
        .into_iter()
        .chain(parents.iter().filter_map(|parent| parent.name()))
        .chain(scope)
        .join(".");

    if header.is_empty() {
        String::new()
    } else {
        format!("[{header}]")
    }
}

#[derive(Debug, Copy, Clone)]
enum ConfigurationFile {
    PyprojectToml,
    RuffToml,
}

#[derive(Default)]
struct CollectOptionsVisitor {
    groups: Vec<(String, OptionSet)>,
    fields: Vec<(String, OptionField)>,
}

impl Visit for CollectOptionsVisitor {
    fn record_set(&mut self, name: &str, group: OptionSet) {
        self.groups.push((name.to_owned(), group));
    }

    fn record_field(&mut self, name: &str, field: OptionField) {
        self.fields.push((name.to_owned(), field));
    }
}
