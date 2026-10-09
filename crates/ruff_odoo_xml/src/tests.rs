use std::path::Path;

use insta::assert_snapshot;
use ruff_linter::registry::Rule;
use ruff_linter::settings::{LinterSettings, flags};
use ruff_source_file::LineIndex;
use ruff_text_size::{Ranged, TextLen, TextRange, TextSize};
use ruff_workspace::FormatterSettings;

use ruff_linter::rules::odoo::settings::OdooVersion;

use crate::{
    BodyEncoding, XmlFormatResult, eval_context_names, extract_code_fields, format_code_fields,
    lint_code_fields,
};

impl std::fmt::Display for XmlFormatResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Formatted(source) => write!(f, "{source}"),
            Self::Unchanged => write!(f, "Unchanged"),
        }
    }
}

fn codes(source: &str) -> Vec<String> {
    extract_code_fields(source)
        .into_iter()
        .map(|field| field.code)
        .collect()
}

fn format(source: &str) -> XmlFormatResult {
    format_code_fields(source, None, &FormatterSettings::default())
}

/// The text of `source` that the first occurrence of `needle` in the first code field maps to.
fn xml_text_of<'a>(source: &'a str, needle: &str) -> &'a str {
    let field = &extract_code_fields(source)[0];
    let start = TextSize::try_from(field.code.find(needle).unwrap()).unwrap();
    let range = TextRange::at(start, needle.text_len());
    &source[field.to_xml_range(range)]
}

#[test]
fn extract_cron() {
    let source = r#"<odoo>
    <record id="ir_cron_sync" model="ir.cron">
        <field name="name">Sync</field>
        <field name="model_id" ref="model_res_partner"/>
        <field name="state">code</field>
        <field name="code">model._cron_sync()</field>
    </record>
</odoo>
"#;
    let fields = extract_code_fields(source);
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].code, "model._cron_sync()\n");
    assert_eq!(&source[fields[0].range()], "model._cron_sync()");
}

#[test]
fn extract_server_action() {
    let source = r#"<odoo>
    <record id="action_archive" model="ir.actions.server">
        <field name="state">code</field>
        <field name="code">
records.action_archive()
</field>
    </record>
</odoo>
"#;
    assert_eq!(codes(source), ["records.action_archive()\n"]);
}

#[test]
fn extract_skips_other_states() {
    let source = r#"<odoo>
    <record id="write" model="ir.actions.server">
        <field name="state">object_write</field>
        <field name="code">records.unused()</field>
    </record>
    <record id="multi" model="ir.actions.server">
        <field name="code">records.run()</field>
        <field name="state"> multi </field>
    </record>
</odoo>
"#;
    assert!(codes(source).is_empty());
}

#[test]
fn extract_without_state() {
    // A scheduled action defaults to `code`, and a record that overrides only the `code` of an
    // existing server action keeps the state it already has.
    let source = r#"<odoo>
    <record id="ir_cron_sync" model="ir.cron">
        <field name="code">model._cron_sync()</field>
    </record>
    <record id="base.existing_action" model="ir.actions.server">
        <field name="code">records.run()</field>
    </record>
</odoo>
"#;
    assert_eq!(codes(source), ["model._cron_sync()\n", "records.run()\n"]);
}

#[test]
fn extract_skips_computed_values_and_other_models() {
    let source = r#"<odoo>
    <record id="from_eval" model="ir.cron">
        <field name="code" eval="'model.run()'"/>
    </record>
    <record id="from_file" model="ir.actions.server">
        <field name="code" file="my_module/data/code.py"/>
    </record>
    <record id="other" model="res.partner">
        <field name="code">not python</field>
    </record>
    <record id="empty" model="ir.cron">
        <field name="code">   </field>
    </record>
</odoo>
"#;
    assert!(codes(source).is_empty());
}

#[test]
fn extract_computed_state_is_linted() {
    let source = r#"<odoo>
    <record id="computed_state" model="ir.actions.server">
        <field name="state" eval="'code'"/>
        <field name="code">records.run()</field>
    </record>
</odoo>
"#;
    assert_eq!(codes(source), ["records.run()\n"]);
}

#[test]
fn extract_entities() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
if len(records) &lt; 10 and records.name != &quot;x&quot; &amp;&amp; 1 &gt; 0:
    log(&#39;few&#x21;&#39;)
</field>
    </record>
</odoo>
"#;
    let fields = extract_code_fields(source);
    assert_eq!(
        fields[0].code,
        "if len(records) < 10 and records.name != \"x\" && 1 > 0:\n    log('few!')\n"
    );
    assert_eq!(fields[0].encoding, BodyEncoding::Text { escapes_gt: true });
    assert_eq!(xml_text_of(source, "< 10"), "&lt; 10");
    assert_eq!(xml_text_of(source, "log"), "log");
    assert_eq!(xml_text_of(source, "'few!'"), "&#39;few&#x21;&#39;");
}

#[test]
fn extract_cdata() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code"><![CDATA[
for partner in records:
    if partner.credit < 0 and partner.active:
        partner.write({"note": "&amp;"})
]]></field>
    </record>
</odoo>
"#;
    let fields = extract_code_fields(source);
    assert_eq!(
        fields[0].code,
        "for partner in records:\n    if partner.credit < 0 and partner.active:\n        partner.write({\"note\": \"&amp;\"})\n"
    );
    assert_eq!(fields[0].encoding, BodyEncoding::CData);
    assert_eq!(
        xml_text_of(source, "partner.credit < 0"),
        "partner.credit < 0"
    );
}

#[test]
fn extract_mixed_text_and_cdata() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">x = 1 &lt; 2<![CDATA[ and 3 < 4]]></field>
    </record>
</odoo>
"#;
    let fields = extract_code_fields(source);
    assert_eq!(fields[0].code, "x = 1 < 2 and 3 < 4\n");
    assert_eq!(fields[0].encoding, BodyEncoding::Mixed);
    assert_eq!(xml_text_of(source, "2 and 3"), "2<![CDATA[ and 3");
}

#[test]
fn extract_keeps_indentation() {
    // Odoo compiles `code.strip()`, which removes the whitespace before the first line only, so
    // the body below fails with an `IndentationError`. It must reach the linter as it is.
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
            records.check()
            records.done()
        </field>
    </record>
</odoo>
"#;
    assert_eq!(
        codes(source),
        ["records.check()\n            records.done()\n"]
    );
}

#[test]
fn extract_stops_at_first_child() {
    // `node.text` ends at the first child node, comments included.
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">records.check()
<!-- records.done() -->
records.ignored()</field>
    </record>
</odoo>
"#;
    assert_eq!(codes(source), ["records.check()\n"]);
}

#[test]
fn extract_crlf() {
    let source = "<odoo>\r\n<record id=\"c\" model=\"ir.cron\">\r\n<field name=\"code\">\r\nif records:\r\n    records.run()\r\n</field>\r\n</record>\r\n</odoo>\r\n";
    let fields = extract_code_fields(source);
    assert_eq!(fields[0].code, "if records:\n    records.run()\n");
    assert_eq!(xml_text_of(source, ":\n    r"), ":\r\n    r");
}

#[test]
fn extract_malformed_xml() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">records.check()</record>
</odoo>
"#;
    assert!(codes(source).is_empty());
}

#[test]
fn format_text() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="state">code</field>
        <field name="code">
for rec in records.filtered(lambda r: r.qty&lt;0 and r.name!='x'):
    rec.write({'qty':0})
</field>
    </record>
</odoo>
"#;
    assert_snapshot!(format(source), @r#"
    <odoo>
        <record id="cron" model="ir.cron">
            <field name="state">code</field>
            <field name="code">
    for rec in records.filtered(lambda r: r.qty &lt; 0 and r.name != "x"):
        rec.write({"qty": 0})
    </field>
        </record>
    </odoo>
    "#);
}

#[test]
fn format_keeps_gt_style() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">x = 1&gt;0 and 2&lt;3</field>
    </record>
</odoo>
"#;
    assert_snapshot!(format(source), @r#"
    <odoo>
        <record id="cron" model="ir.cron">
            <field name="code">x = 1 &gt; 0 and 2 &lt; 3</field>
        </record>
    </odoo>
    "#);
}

#[test]
fn format_cdata() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code"><![CDATA[
if records and records[0].qty<0 :
    records.write({'qty':0})
]]></field>
    </record>
</odoo>
"#;
    assert_snapshot!(format(source), @r#"
    <odoo>
        <record id="cron" model="ir.cron">
            <field name="code"><![CDATA[
    if records and records[0].qty < 0:
        records.write({"qty": 0})
    ]]></field>
        </record>
    </odoo>
    "#);
}

#[test]
fn format_unchanged() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">model._cron_sync()</field>
    </record>
    <record id="write" model="ir.actions.server">
        <field name="state">object_write</field>
        <field name="code">records.unused( )</field>
    </record>
</odoo>
"#;
    assert_snapshot!(format(source), @"Unchanged");
}

#[test]
fn format_syntax_error() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
            records.check( )
            records.done( )
        </field>
    </record>
</odoo>
"#;
    assert_snapshot!(format(source), @"Unchanged");
}

#[test]
fn format_mixed_is_left_alone() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">x=1 &lt; 2<![CDATA[ and 3 < 4]]></field>
    </record>
</odoo>
"#;
    assert_snapshot!(format(source), @"Unchanged");
}

#[test]
fn format_crlf() {
    let source = "<odoo>\r\n<record id=\"c\" model=\"ir.cron\">\r\n<field name=\"code\">\r\nif records :\r\n    records.run( )\r\n</field>\r\n</record>\r\n</odoo>\r\n";
    let XmlFormatResult::Formatted(formatted) = format(source) else {
        panic!("expected the code to be formatted");
    };
    assert_eq!(
        formatted,
        "<odoo>\r\n<record id=\"c\" model=\"ir.cron\">\r\n<field name=\"code\">\r\nif records:\r\n    records.run()\r\n</field>\r\n</record>\r\n</odoo>\r\n"
    );
}

#[test]
fn format_several_fields() {
    let source = r#"<odoo>
    <record id="a" model="ir.cron">
        <field name="code">model.a( )</field>
    </record>
    <record id="b" model="ir.actions.server">
        <field name="state">code</field>
        <field name="code">records.b( )</field>
    </record>
</odoo>
"#;
    assert_snapshot!(format(source), @r#"
    <odoo>
        <record id="a" model="ir.cron">
            <field name="code">model.a()</field>
        </record>
        <record id="b" model="ir.actions.server">
            <field name="state">code</field>
            <field name="code">records.b()</field>
        </record>
    </odoo>
    "#);
}

/// Lints `source` as `my_module/data/test.xml` and renders each diagnostic as
/// `row:column code message` against the XML document.
fn lint(source: &str, rules: impl IntoIterator<Item = Rule>) -> String {
    let path = Path::new("my_module/data/test.xml");
    let settings = LinterSettings::for_rules(rules);
    let index = LineIndex::from_source_text(source);
    lint_code_fields(path, None, source, &settings, flags::Noqa::Enabled)
        .iter()
        .map(|diagnostic| {
            let location = index.line_column(diagnostic.range().unwrap().start(), source);
            format!(
                "{}:{} {} {}",
                location.line,
                location.column,
                diagnostic.secondary_code_or_id(),
                diagnostic.concise_message()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn lint_reports_xml_positions() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="state">code</field>
        <field name="code">
for partner in records:
    if partner.credit &lt; 0 and undefined_name:
        log(&quot;negative&quot;)
</field>
    </record>
    <record id="action" model="ir.actions.server">
        <field name="state">code</field>
        <field name="code"><![CDATA[
if records and missing:
    action = env["ir.actions.act_window"]._for_xml_id(xml_id)
]]></field>
    </record>
</odoo>
"#;
    assert_snapshot!(lint(source, [Rule::UndefinedName]), @"
    6:34 F821 Undefined name `undefined_name`
    13:16 F821 Undefined name `missing`
    14:55 F821 Undefined name `xml_id`
    ");
}

#[test]
fn lint_knows_the_eval_context() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
for rec in records | record | model.browse(uid) | env.user | user:
    log(str(time.time()) + str(datetime.date.today()) + str(dateutil) + str(timezone))
    _logger.info(b64encode(b64decode(b"")))
    if float_compare(1.0, 2.0, 2) or json.dumps({}) or payload or request:
        raise UserError(BinaryBytes)
    Command.clear()
    reduce(lambda a, b: a, xrange(unicode(1)))
action = {"type": "ir.actions.act_window_close"}
</field>
    </record>
</odoo>
"#;
    assert_snapshot!(lint(source, [Rule::UndefinedName]), @"");
}

#[test]
fn lint_reports_syntax_errors() {
    let source = r#"<odoo>
    <record id="indented" model="ir.cron">
        <field name="code">
            records.check()
            records.done()
        </field>
    </record>
    <record id="broken" model="ir.cron">
        <field name="code">records.check(</field>
    </record>
    <record id="return" model="ir.actions.server">
        <field name="code">return records</field>
    </record>
</odoo>
"#;
    assert_snapshot!(lint(source, [Rule::ReturnOutsideFunction]), @"
    5:1 invalid-syntax Unexpected indentation
    9:42 invalid-syntax unexpected EOF while parsing
    12:28 F706 `return` statement outside of a function/method
    ");
}

#[test]
fn lint_skips_other_states() {
    let source = r#"<odoo>
    <record id="write" model="ir.actions.server">
        <field name="state">object_write</field>
        <field name="code">undefined_name(</field>
    </record>
    <record id="partner" model="res.partner">
        <field name="code">undefined_name</field>
    </record>
</odoo>
"#;
    assert_snapshot!(lint(source, [Rule::UndefinedName]), @"");
}

#[test]
fn lint_ignores_file_level_rules() {
    // The field is not a module: it has no docstring, no license header and no trailing newline
    // of its own, and the XML file's name and directory say nothing about it.
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
# This cron syncs the partners
model._cron_sync()
</field>
    </record>
</odoo>
"#;
    assert_snapshot!(
        lint(
            source,
            [
                Rule::UndocumentedPublicModule,
                Rule::MissingCopyrightNotice,
                Rule::ImplicitNamespacePackage,
                Rule::InvalidModuleName,
                Rule::MissingNewlineAtEndOfFile,
                Rule::TrailingWhitespace,
                Rule::HeaderComments,
            ]
        ),
        @""
    );
}

#[test]
fn lint_honours_noqa() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
undefined_name  # noqa: F821
other_name
</field>
    </record>
</odoo>
"#;
    assert_snapshot!(lint(source, [Rule::UndefinedName]), @"5:1 F821 Undefined name `other_name`");
}

#[test]
fn lint_drops_fixes() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">x = 1 if records == None else 2</field>
    </record>
</odoo>
"#;
    let path = Path::new("my_module/data/test.xml");
    let settings = LinterSettings::for_rules([Rule::NoneComparison]);
    let diagnostics = lint_code_fields(path, None, source, &settings, flags::Noqa::Enabled);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].fix().is_none());
}

#[test]
fn extract_dedents_an_indented_compound_statement() {
    // Odoo's own layout: `strip()` only removes the first line's indentation, and the body is
    // still deeper than it, so this runs. The linter sees it without the XML indentation.
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
            if records:
                records.write({
                    'done': True,
                })
        </field>
    </record>
</odoo>
"#;
    let fields = extract_code_fields(source);
    assert_eq!(
        fields[0].code,
        "if records:\n    records.write({\n        'done': True,\n    })\n"
    );
    assert_eq!(fields[0].indent, "            ");
    assert_eq!(xml_text_of(source, "'done'"), "'done'");
}

#[test]
fn extract_keeps_indentation_that_changes_a_string() {
    // Dedenting would change the value of the string, so the code is linted as Odoo runs it.
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
            if records:
                log("""first
                second""")
        </field>
    </record>
</odoo>
"#;
    let fields = extract_code_fields(source);
    assert_eq!(fields[0].indent, "");
    assert_eq!(
        fields[0].code,
        "if records:\n                log(\"\"\"first\n                second\"\"\")\n"
    );
}

#[test]
fn format_keeps_the_indentation_under_the_tag() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
            if records:
                records.write({'done':True})
        </field>
    </record>
</odoo>
"#;
    assert_snapshot!(format(source), @r#"
    <odoo>
        <record id="cron" model="ir.cron">
            <field name="code">
                if records:
                    records.write({"done": True})
            </field>
        </record>
    </odoo>
    "#);
}

#[test]
fn format_measures_the_line_width_without_the_indentation() {
    // The code fits in the default 88 columns on its own, which is what `E501` measures too, so
    // the 12 columns of indentation under the tag do not make the formatter split it.
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
            if records:
                records.write({"state": "done", "date": datetime.date.today(), "user_id": uid})
        </field>
    </record>
</odoo>
"#;
    assert_snapshot!(format(source), @"Unchanged");
}

#[test]
fn lint_ignores_the_indentation_under_the_tag() {
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
            if records:
                records.write({'done': undefined_name})
        </field>
    </record>
</odoo>
"#;
    assert_snapshot!(
        lint(source, [Rule::UndefinedName, Rule::OverIndented, Rule::IndentationWithInvalidMultiple]),
        @"5:40 F821 Undefined name `undefined_name`"
    );
}

#[test]
fn lint_knows_ai_tool_arguments() {
    let source = r#"<odoo>
    <record id="tool" model="ir.actions.server">
        <field name="state">code</field>
        <field name="use_in_ai" eval="True"/>
        <field name="code">ai['result'] = record._run(menu_id, action_type, undefined_name)</field>
        <field name="ai_tool_schema">
        {"type": "object", "properties": {"menu_id": {"type": "number"}, "action_type": {"type": "string"}}}
        </field>
    </record>
    <record id="tool_without_arguments" model="ir.actions.server">
        <field name="use_in_ai" eval="True"/>
        <field name="code">ai['result'] = record._run()</field>
    </record>
    <record id="mcp_tool" model="ir.actions.server">
        <field name="use_in_mcp" eval="True"/>
        <field name="ai_tool_name">ai_tool_context</field>
        <field name="code">ai['result'] = record._context()</field>
    </record>
    <record id="not_a_tool" model="ir.actions.server">
        <field name="state">code</field>
        <field name="code">ai['result'] = menu_id</field>
    </record>
</odoo>
"#;
    assert_snapshot!(lint(source, [Rule::UndefinedName]), @"
    5:77 F821 Undefined name `undefined_name`
    21:43 F821 Undefined name `menu_id`
    21:28 F821 Undefined name `ai`
    ");
}

#[test]
fn eval_context_names_by_version() {
    let names = |version: Option<(u16, u16)>| {
        eval_context_names(version.map(|(major, minor)| OdooVersion::new(major, minor)))
    };
    // A version with a row of its own, and those without one, which use the closest before.
    assert_eq!(names(Some((12, 0))), names(Some((13, 0))));
    assert_eq!(names(Some((15, 0))), names(Some((16, 0))));
    assert_eq!(names(Some((17, 0))), names(Some((19, 0))));
    // Older than every row: the oldest one. Unset: the newest one.
    assert_eq!(names(Some((11, 0))), names(Some((12, 0))));
    assert_eq!(names(None), names(Some((20, 0))));
    assert_eq!(names(Some((21, 0))), names(Some((20, 0))));

    assert!(!names(Some((13, 0))).contains(&"UserError"));
    assert!(names(Some((14, 0))).contains(&"UserError"));
    assert!(!names(Some((14, 0))).contains(&"Command"));
    assert!(names(Some((15, 0))).contains(&"Command"));
    assert!(names(Some((16, 0))).contains(&"Warning"));
    assert!(!names(Some((17, 0))).contains(&"Warning"));
    assert!(names(Some((17, 0))).contains(&"payload"));
    assert!(!names(Some((19, 0))).contains(&"BinaryBytes"));
    assert!(names(Some((20, 0))).contains(&"BinaryBytes"));
}

/// Lints `source` with `odoo-version` set to `version`, reporting undefined names only.
fn lint_for_version(source: &str, version: (u16, u16)) -> String {
    let path = Path::new("my_module/data/test.xml");
    let mut settings = LinterSettings::for_rules([Rule::UndefinedName]);
    settings.odoo.odoo_version = Some(OdooVersion::new(version.0, version.1));
    let index = LineIndex::from_source_text(source);
    let mut diagnostics = lint_code_fields(path, None, source, &settings, flags::Noqa::Enabled);
    diagnostics.sort_by_key(|diagnostic| diagnostic.range().map(TextRange::start));
    diagnostics
        .iter()
        .map(|diagnostic| {
            let location = index.line_column(diagnostic.range().unwrap().start(), source);
            format!(
                "{}:{} {}",
                location.line,
                location.column,
                diagnostic.concise_message()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn lint_follows_the_odoo_version() {
    // `Warning` left the context in 17.0 but is never reported: Python has a builtin of the same
    // name, and the linter assumes every Python builtin is available, which `safe_eval` is not.
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
if records:
    raise UserError(Warning)
records.write({"line_ids": [Command.clear()]})
_logger.info(payload)
</field>
    </record>
</odoo>
"#;
    assert_snapshot!(lint_for_version(source, (13, 0)), @"
    5:11 Undefined name `UserError`
    6:29 Undefined name `Command`
    7:1 Undefined name `_logger`
    7:14 Undefined name `payload`
    ");
    assert_snapshot!(lint_for_version(source, (16, 0)), @"
    7:1 Undefined name `_logger`
    7:14 Undefined name `payload`
    ");
    assert_snapshot!(lint_for_version(source, (19, 0)), @"");
}

#[test]
fn extract_nested_records_in_document_order() {
    // The inner record closes first, but its field comes after the outer one in the document.
    let source = r#"<odoo>
    <record id="outer" model="ir.actions.server">
        <field name="code">records.outer( )</field>
        <field name="child_ids">
            <record id="inner" model="ir.actions.server"><field name="code">records.inner( )</field></record>
        </field>
    </record>
</odoo>
"#;
    assert_eq!(codes(source), ["records.outer( )\n", "records.inner( )\n"]);
    assert_snapshot!(format(source), @r#"
    <odoo>
        <record id="outer" model="ir.actions.server">
            <field name="code">records.outer()</field>
            <field name="child_ids">
                <record id="inner" model="ir.actions.server"><field name="code">records.inner()</field></record>
            </field>
        </record>
    </odoo>
    "#);
}

#[test]
fn extract_after_a_byte_order_mark() {
    let source = "\u{feff}<odoo>\n<record id=\"c\" model=\"ir.cron\">\n<field name=\"code\">undefined_bom( )</field>\n</record>\n</odoo>\n";
    assert_eq!(codes(source), ["undefined_bom( )\n"]);
    assert_eq!(xml_text_of(source, "undefined_bom"), "undefined_bom");
    assert_snapshot!(lint(source, [Rule::UndefinedName]), @"3:20 F821 Undefined name `undefined_bom`");
}

#[test]
fn format_keeps_the_document_line_endings_whatever_the_setting() {
    let crlf = "<odoo>\r\n<record id=\"c\" model=\"ir.cron\">\r\n<field name=\"code\">\r\nif records :\r\n    records.run( )\r\n</field>\r\n</record>\r\n</odoo>\r\n";
    let lf = crlf.replace("\r\n", "\n");
    // The setting's type is not exported; it is read the way a configuration file spells it.
    for line_ending in ["\"lf\"", "\"cr-lf\"", "\"auto\""] {
        let settings = FormatterSettings {
            line_ending: serde_json::from_str(line_ending).unwrap(),
            ..FormatterSettings::default()
        };
        for (source, newline) in [(crlf, "\r\n"), (lf.as_str(), "\n")] {
            let XmlFormatResult::Formatted(formatted) = format_code_fields(source, None, &settings)
            else {
                panic!("expected the code to be formatted");
            };
            assert_eq!(
                formatted,
                format!(
                    "<odoo>{newline}<record id=\"c\" model=\"ir.cron\">{newline}<field name=\"code\">{newline}if records:{newline}    records.run(){newline}</field>{newline}</record>{newline}</odoo>{newline}"
                )
            );
            assert_eq!(
                format_code_fields(&formatted, None, &settings),
                XmlFormatResult::Unchanged
            );
        }
    }
}

#[test]
fn extract_automated_action() {
    // Up to 16.0, an automated action delegates to a server action and carries its code.
    let source = r#"<odoo>
    <record id="rule" model="base.automation">
        <field name="trigger">on_create</field>
        <field name="state">code</field>
        <field name="code">records.check()</field>
    </record>
</odoo>
"#;
    assert_eq!(codes(source), ["records.check()\n"]);
}

#[test]
fn format_leaves_an_indented_body_it_cannot_dedent_alone() {
    // Dedenting would change the string, so the lines after the first would end up at the start
    // of the line under a first line that stays indented.
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="code">
            if records :
                log("""first
                second""")
        </field>
    </record>
</odoo>
"#;
    assert_snapshot!(format(source), @"Unchanged");
}

#[test]
fn extract_ai_tool_flag_set_to_false() {
    let source = r#"<odoo>
    <record id="not_a_tool" model="ir.actions.server">
        <field name="use_in_ai" eval="False"/>
        <field name="code">ai['result'] = record._run()</field>
    </record>
</odoo>
"#;
    assert_snapshot!(lint(source, [Rule::UndefinedName]), @"4:28 F821 Undefined name `ai`");
}

#[test]
fn extract_skips_a_code_found_by_search() {
    let source = r#"<odoo>
    <record id="from_search" model="ir.actions.server">
        <field name="code" search="[('id', '=', 1)]"/>
    </record>
    <record id="from_search_with_text" model="ir.actions.server">
        <field name="code" search="[('id', '=', 1)]">not python</field>
    </record>
</odoo>
"#;
    assert!(codes(source).is_empty());
}

#[test]
fn extract_strips_like_python() {
    // `str.isspace()` also accepts the ASCII separators U+001C to U+001F.
    let source = "<odoo><record id=\"c\" model=\"ir.cron\"><field name=\"code\">\u{1c}records.run()\u{1f}</field></record></odoo>";
    assert_eq!(codes(source), ["records.run()\n"]);
}

fn lint_odoo(source: &str, version: (u16, u16), rules: impl IntoIterator<Item = Rule>) -> String {
    let path = Path::new("my_module/data/test.xml");
    let mut settings = LinterSettings::for_rules(rules);
    settings.odoo.odoo_version = Some(OdooVersion::new(version.0, version.1));
    let index = LineIndex::from_source_text(source);
    let mut diagnostics = lint_code_fields(path, None, source, &settings, flags::Noqa::Enabled);
    diagnostics.sort_by_key(|diagnostic| diagnostic.range().map(TextRange::start));
    diagnostics
        .iter()
        .map(|diagnostic| {
            let location = index.line_column(diagnostic.range().unwrap().start(), source);
            format!(
                "{}:{} {} {}",
                location.line,
                location.column,
                diagnostic.secondary_code_or_id(),
                diagnostic.concise_message()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn extract_the_model_reference() {
    let source = r#"<odoo>
    <record id="by_ref" model="ir.cron">
        <field name="model_id" ref="account.model_account_move"/>
        <field name="code">model.action_post()</field>
    </record>
    <record id="by_eval" model="ir.actions.server">
        <field name="code">model.action_post()</field>
        <field name="model_id" eval="ref('sale.model_sale_order')"/>
    </record>
    <record id="none" model="ir.cron">
        <field name="code">model.action_post()</field>
    </record>
</odoo>
"#;
    let references: Vec<_> = extract_code_fields(source)
        .into_iter()
        .map(|field| field.model_xmlid)
        .collect();
    assert_eq!(
        references,
        [
            Some("account.model_account_move".to_string()),
            Some("sale.model_sale_order".to_string()),
            None,
        ]
    );
}

#[test]
fn lint_runs_the_odoo_checks_on_the_injected_names() {
    // `env`, `model`, `record` and `records` are what `self.env` and `self` are in a model.
    let source = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="model_id" ref="account.model_account_move"/>
        <field name="code">
env.cr.execute("DELETE FROM %s" % env.context["table"])
env.cr.execute("DELETE FROM account_move WHERE id = %s", (record.id,))
env.cr.commit()
records.read_group([], [], [])
env["res.partner"].check_access_rights("read")
payload.check_access_rights("read")
records.write()
records.name_get()
payload.name_get()
records._cr.rollback()
</field>
    </record>
</odoo>
"#;
    assert_snapshot!(lint_odoo(source, (19, 0), [
        Rule::SqlInjection,
        Rule::InvalidCommit,
        Rule::DeprecatedOdooMethodCall,
        Rule::InvalidOdooMethodCall,
        Rule::RemovedOdooMethodCall,
        Rule::PreferEnvAttribute,
    ]), @r#"
    5:1 ODE8103 SQL injection risk. Use parameters if you can. - More info https://github.com/OCA/odoo-community.org/blob/master/website/Contribution/CONTRIBUTING.rst#no-sql-injection
    7:1 ODE8102 Use of cr.commit() directly
    8:9 ODW8502 `read_group` is deprecated since Odoo 19.0. Use `_read_group` in backend code, or `formatted_read_group` for a formatted result.
    9:20 ODW8502 `check_access_rights` is deprecated since Odoo 18.0. Use `check_access` instead, or `has_access` where a boolean is needed; an override belongs in `_check_access`.
    11:1 ODE9502 `write` requires argument `vals` in Odoo 19.0
    12:1 ODE9503 `name_get` was removed from the Odoo ORM in 18.0
    14:1 ODW8165 Use "records.env.cr" instead of "records._cr" (deprecated since 19.0)
    "#);
}

#[test]
fn no_search_all_reports_an_unbounded_search_in_a_cron() {
    // The shape of a cron that kept deadlocking against Odoo's own auto-post cron: it posted
    // every draft journal entry its domain matched, in one transaction. The fix was a `limit`.
    let before = r#"<odoo>
    <record id="cron" model="ir.cron">
        <field name="model_id" ref="account.model_account_move" />
        <field name="state">code</field>
        <field name="code">
today = datetime.date.today()
am_ids = model.search([('state', '=', 'draft'), ('asset_id','!=', False), ('date', '=', today)])
for am_id in am_ids:
  am_id.action_post()
        </field>
    </record>
</odoo>
"#;
    assert_snapshot!(lint_odoo(before, (16, 0), [Rule::NoSearchAll]), @r#"7:10 ODW8163 Using `search(...)` without a `limit` in a cron or server action will load all matching records of "account.model_account_move" in one transaction, may impact performance."#);
    let after = before.replace("today)])", "today)], limit=1000)");
    assert_snapshot!(lint_odoo(&after, (16, 0), [Rule::NoSearchAll]), @"");
}

#[test]
fn no_search_all_resolves_the_model_of_the_record() {
    let source = r#"<odoo>
    <record id="heavy" model="ir.cron">
        <field name="model_id" ref="account.model_account_move"/>
        <field name="code">
model.search([])
records.sudo().search_read([("state", "=", "draft")])
model.search([], limit=1)
env["res.users"].search([])
env["stock.move"].search([])
domain = []
model.search(domain)
</field>
    </record>
    <record id="underscored" model="ir.actions.server">
        <field name="model_id" ref="data_merge.model_data_merge_group"/>
        <field name="code">model.search([])</field>
    </record>
    <record id="light" model="ir.cron">
        <field name="model_id" ref="base.model_res_users"/>
        <field name="code">model.search([])</field>
    </record>
    <record id="unknown" model="ir.cron">
        <field name="code">model.search([])</field>
    </record>
    <record id="rebound" model="ir.cron">
        <field name="model_id" ref="account.model_account_move"/>
        <field name="code">
model = env["res.users"]
model.search([])
</field>
    </record>
</odoo>
"#;
    assert_snapshot!(lint_odoo(source, (19, 0), [Rule::NoSearchAll]), @r#"
    5:1 ODW8163 Using an empty domain `search([])` without a `limit` will load all records of "account.model_account_move", may impact performance.
    6:1 ODW8163 Using `search_read(...)` without a `limit` in a cron or server action will load all matching records of "account.model_account_move" in one transaction, may impact performance.
    9:1 ODW8163 Using an empty domain `search([])` without a `limit` will load all records of "stock.move", may impact performance.
    11:1 ODW8163 Using an empty domain `search([])` without a `limit` will load all records of "account.model_account_move", may impact performance.
    16:28 ODW8163 Using an empty domain `search([])` without a `limit` will load all records of "data_merge.model_data_merge_group", may impact performance.
    "#);
}

#[test]
fn lint_keeps_module_semantics_for_the_odoo_checks() {
    // A nested `def unlink()` is no model method, `action` is read back by Odoo, and a field
    // has no `self`.
    let source = r#"<odoo>
    <record id="action" model="ir.actions.server">
        <field name="code">
def unlink():
    raise UserError(env._("No"))
action = {"type": "ir.actions.act_window_close"}
self.env.cr.commit()
</field>
    </record>
</odoo>
"#;
    assert_snapshot!(lint_odoo(source, (19, 0), [
        Rule::NoRaiseUnlink,
        Rule::UnusedVariable,
        Rule::UndefinedName,
    ]), @"7:1 F821 Undefined name `self`");
}

#[test]
fn translation_checks_name_env_in_code_fields() {
    let source = r#"<odoo>
    <record id="action" model="ir.actions.server">
        <field name="code">
raise UserError("Nothing to post")
_("Nothing to post")
</field>
    </record>
</odoo>
"#;
    let rules = [Rule::TranslationRequired, Rule::PreferEnvTranslation];
    assert_snapshot!(lint_odoo(source, (19, 0), rules), @r#"
    4:17 ODC8107 String parameter on "UserError" requires translation. Use env._(...)
    5:1 ODW8161 Better using env._
    "#);
    // Before 18.0 Odoo puts no translation function in the evaluation context at all.
    assert_snapshot!(lint_odoo(source, (17, 0), rules), @"");
}
