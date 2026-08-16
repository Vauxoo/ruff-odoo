//! Generate the Markdown sources for this fork's `OD`/`OAPP` documentation site.
//!
//! Upstream's `generate-docs` renders a page for every rule Ruff ships, which is what
//! <https://docs.astral.sh/ruff/> needs. This fork publishes a much smaller site that
//! covers only the Odoo rules it adds, so that Odoo developers migrating from
//! `pylint-odoo` land on 70 rules rather than on a thousand.
//!
//! The generated tree is deliberately laid out like upstream's `docs/`:
//!
//! ```text
//! docs-odoo/
//!   index.md      (checked in)
//!   preview.md    (checked in)
//!   rules.md      (generated)
//!   settings.md   (generated)
//!   rules/*.md    (generated)
//! ```
//!
//! Rule pages link to `../rules.md`, `../preview.md` and `../settings.md` with paths
//! baked into [`crate::generate_docs`], so reusing those file names is what lets this
//! command render the pages without rewriting any of the link handling.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use anyhow::{Result, bail};

use ruff_linter::registry::{Linter, Rule};

use crate::ROOT_DIR;
use crate::generate_docs::{Repository, generate_rule_doc};
use crate::generate_options;
use crate::generate_rules_table::{generate_legend, generate_linter_section};

/// The linters this site documents: the fork's own rules, and nothing else.
const LINTERS: [Linter; 2] = [Linter::Odoo, Linter::OdooApp];

/// The Odoo rules live here, not in `astral-sh/ruff`.
pub(crate) const FORK: Repository = Repository {
    slug: "Vauxoo/ruff-odoo",
    release_tags: true,
};

/// The site's source directory, relative to the repository root.
const DOCS_DIR: &str = "docs-odoo";

/// The only option group the Odoo rules read.
///
/// Documenting the whole option tree here would drag in links to the upstream rule pages
/// this site deliberately doesn't render, which `mkdocs build --strict` rejects.
const SETTINGS_GROUP: &str = "lint.odoo";

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Write the generated docs to stdout (rather than to the filesystem).
    #[arg(long)]
    pub(crate) dry_run: bool,
}

pub(crate) fn main(args: &Args) -> Result<()> {
    let root = PathBuf::from(ROOT_DIR).join(DOCS_DIR);

    let mut rules_page = String::from(
        "# Rules\n\nThe rules this fork adds on top of Ruff. For the upstream rules, see \
         [docs.astral.sh/ruff/rules](https://docs.astral.sh/ruff/rules/).\n\n",
    );
    // Upstream points the legend at `faq.md`, which this site doesn't have; every rule
    // here is in preview, so `preview.md` is the page a reader actually wants.
    generate_legend(&mut rules_page, "preview.md");
    for linter in &LINTERS {
        generate_linter_section(&mut rules_page, linter);
    }

    let Some(options) = generate_options::generate_group(SETTINGS_GROUP) else {
        bail!("`{SETTINGS_GROUP}` is not an option group; has it been renamed?");
    };
    let settings_page = format!(
        "# Settings\n\nOptions read by the `OD` rules. Every other Ruff option is \
         documented at [docs.astral.sh/ruff/settings](https://docs.astral.sh/ruff/settings/).\n\n\
         {options}"
    );

    if args.dry_run {
        println!("{rules_page}");
        println!("{settings_page}");
    } else {
        fs::create_dir_all(root.join("rules"))?;
        fs::write(root.join("rules.md"), &rules_page)?;
        fs::write(root.join("settings.md"), &settings_page)?;
    }

    for rule in LINTERS.into_iter().flat_map(|linter| linter.all_rules()) {
        let Some(doc) = generate_rule_doc(rule, FORK) else {
            continue;
        };
        let doc = with_front_matter(&doc, rule);

        if args.dry_run {
            println!("{doc}");
        } else {
            fs::write(
                root.join("rules").join(&*rule.name()).with_extension("md"),
                doc,
            )?;
        }
    }

    Ok(())
}

/// Prepend the YAML front matter mkdocs-material uses for a page's `<meta>` description
/// and tags.
///
/// Upstream adds this in `scripts/generate_mkdocs.py` as a post-processing pass over the
/// generated files. Doing it here instead keeps this site buildable from `cargo dev`
/// alone, without a Python step.
///
/// The description is the first non-empty line under `## What it does`, which every rule
/// in this fork has: `crates/ruff_linter/src/registry.rs` has a test asserting that every
/// rule is documented.
fn with_front_matter(doc: &str, rule: Rule) -> String {
    let description = doc
        .split_once("## What it does\n")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.lines().find(|line| !line.trim().is_empty()))
        .unwrap_or_default();

    let mut output = String::new();
    let _ = writeln!(&mut output, "---");
    let _ = writeln!(&mut output, "description: |-");
    let _ = writeln!(&mut output, "  {description}");
    let _ = writeln!(&mut output, "tags:");
    let _ = writeln!(&mut output, "- {}", rule.noqa_code());
    let _ = writeln!(&mut output, "---");
    output.push('\n');
    output.push_str(doc);
    output
}
