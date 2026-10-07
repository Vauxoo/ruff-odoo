//! Python code embedded in Odoo XML data files.
//!
//! Odoo stores the body of a scheduled action (`ir.cron`) and of a server action
//! (`ir.actions.server`) in the record's `code` field, and runs it with
//! `safe_eval(self.code.strip(), eval_context, mode="exec")`. This crate finds those fields in an
//! XML data file, decodes them the way the XML parser does (entities, `CDATA` sections and line
//! endings), and keeps, for every byte of the decoded code, the offset it came from in the XML
//! document, so that diagnostics and edits computed on the code can be reported against, and
//! written back into, the original file.
//!
//! The decoded code is `str.strip()` of the field's text, exactly what Odoo compiles. Odoo does
//! not dedent it, so a body of several statements indented under its `<field>` tag fails with an
//! `IndentationError` when the record is loaded, and reporting that is the point. A single
//! compound statement indented that way does run, though, since only its first line loses its
//! indentation and every other line is still deeper than it; Odoo's own data files are full of
//! those. For that shape the code is dedented by the indentation of its first line, but only
//! after checking that both versions parse to the same syntax tree, so the linter sees the
//! program Odoo runs without the indentation the XML layout adds to it.

use std::path::Path;

pub use lint::lint_code_fields;

mod lint;
#[cfg(test)]
mod tests;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use ruff_formatter::Printed;
use ruff_formatter::printer::LineEnding;
use ruff_linter::rules::odoo::settings::OdooVersion;
use ruff_python_ast::PySourceType;
use ruff_python_ast::comparable::ComparableModModule;
use ruff_python_formatter::format_module_source;
use ruff_python_parser::parse_module;
use ruff_text_size::{Ranged, TextLen, TextRange, TextSize};
use ruff_workspace::FormatterSettings;

/// The models whose `code` field holds Python that Odoo executes. Up to 16.0, `base.automation`
/// delegates to `ir.actions.server` (`action_server_id` with `delegate=True`), so an automated
/// action carries its `state` and `code` fields directly.
pub const CODE_MODELS: &[&str] = &["ir.cron", "ir.actions.server", "base.automation"];

/// The names Odoo injects into the evaluation context of a server action or scheduled action,
/// on top of Python's builtins, for each Odoo version that changed them.
///
/// Each row is the full set from its version up to the next row, so a version without a row of
/// its own (13.0, 16.0, 18.0, 19.0) uses the closest one before it. The names come from
/// `ir.actions.actions._get_eval_context` and `ir.actions.server._get_eval_context` in `base`,
/// the overrides `website` and `base_automation` add to the latter, and the builtins `safe_eval`
/// provides that Python itself does not (`reduce`, `unicode`, `xrange`):
///
/// - 12.0: [`base`](https://github.com/odoo/odoo/blob/ba1ba88874e2ae9716e59ec199eb739d58c3f771/odoo/addons/base/models/ir_actions.py#L74-L86),
///   [server action](https://github.com/odoo/odoo/blob/ba1ba88874e2ae9716e59ec199eb739d58c3f771/odoo/addons/base/models/ir_actions.py#L481-L518),
///   [`website`](https://github.com/odoo/odoo/blob/ba1ba88874e2ae9716e59ec199eb739d58c3f771/addons/website/models/ir_actions.py#L44-L49),
///   [`safe_eval`](https://github.com/odoo/odoo/blob/ba1ba88874e2ae9716e59ec199eb739d58c3f771/odoo/tools/safe_eval.py#L273-L310).
/// - 14.0 adds `UserError` ([server action](https://github.com/odoo/odoo/blob/cc0060e889603eb2e47fa44a8a22a70d7d784185/odoo/addons/base/models/ir_actions.py#L545-L583))
///   and `json` ([`website`](https://github.com/odoo/odoo/blob/cc0060e889603eb2e47fa44a8a22a70d7d784185/addons/website/models/ir_actions.py#L47-L53)).
/// - 15.0 adds `Command` ([`base`](https://github.com/odoo/odoo/blob/3a28e5b0adbb36bdb1155a6854cdfbe4e7f9b187/odoo/addons/base/models/ir_actions.py#L75-L88)).
/// - 17.0 drops `Warning` and adds `_logger`
///   ([server action](https://github.com/odoo/odoo/blob/338d8d154c3b3fdb32585bf9192e562e0ba09306/odoo/addons/base/models/ir_actions.py#L848-L886)), plus `payload`
///   ([`base_automation`](https://github.com/odoo/odoo/blob/338d8d154c3b3fdb32585bf9192e562e0ba09306/addons/base_automation/models/ir_actions_server.py#L87-L94)).
/// - 20.0, read from `master`, adds `BinaryBytes`
///   ([`base`](https://github.com/odoo/odoo/blob/30e99dcaae61e23c856ce1a665b93114fd790352/odoo/addons/base/models/ir_actions.py#L121-L135)).
const EVAL_CONTEXTS: &[(OdooVersion, &[&str])] = &[
    (
        OdooVersion::new(12, 0),
        &[
            "Warning",
            "b64decode",
            "b64encode",
            "datetime",
            "dateutil",
            "env",
            "float_compare",
            "log",
            "model",
            "record",
            "records",
            "reduce",
            "request",
            "time",
            "timezone",
            "uid",
            "unicode",
            "user",
            "xrange",
        ],
    ),
    (
        OdooVersion::new(14, 0),
        &[
            "UserError",
            "Warning",
            "b64decode",
            "b64encode",
            "datetime",
            "dateutil",
            "env",
            "float_compare",
            "json",
            "log",
            "model",
            "record",
            "records",
            "reduce",
            "request",
            "time",
            "timezone",
            "uid",
            "unicode",
            "user",
            "xrange",
        ],
    ),
    (
        OdooVersion::new(15, 0),
        &[
            "Command",
            "UserError",
            "Warning",
            "b64decode",
            "b64encode",
            "datetime",
            "dateutil",
            "env",
            "float_compare",
            "json",
            "log",
            "model",
            "record",
            "records",
            "reduce",
            "request",
            "time",
            "timezone",
            "uid",
            "unicode",
            "user",
            "xrange",
        ],
    ),
    (
        OdooVersion::new(17, 0),
        &[
            "Command",
            "UserError",
            "_logger",
            "b64decode",
            "b64encode",
            "datetime",
            "dateutil",
            "env",
            "float_compare",
            "json",
            "log",
            "model",
            "payload",
            "record",
            "records",
            "reduce",
            "request",
            "time",
            "timezone",
            "uid",
            "unicode",
            "user",
            "xrange",
        ],
    ),
    (
        OdooVersion::new(20, 0),
        &[
            "BinaryBytes",
            "Command",
            "UserError",
            "_logger",
            "b64decode",
            "b64encode",
            "datetime",
            "dateutil",
            "env",
            "float_compare",
            "json",
            "log",
            "model",
            "payload",
            "record",
            "records",
            "reduce",
            "request",
            "time",
            "timezone",
            "uid",
            "unicode",
            "user",
            "xrange",
        ],
    ),
];

/// Returns the names Odoo injects into the evaluation context of a server action or scheduled
/// action in `odoo_version`: the set of the closest version at or before it, the oldest set for
/// a version older than all of them, and the newest set when no version is configured.
pub fn eval_context_names(odoo_version: Option<OdooVersion>) -> &'static [&'static str] {
    let Some(odoo_version) = odoo_version else {
        return EVAL_CONTEXTS[EVAL_CONTEXTS.len() - 1].1;
    };
    EVAL_CONTEXTS
        .iter()
        .rev()
        .find(|(since, _)| *since <= odoo_version)
        .unwrap_or(&EVAL_CONTEXTS[0])
        .1
}

/// How the body of a `code` field is written in the XML document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyEncoding {
    /// Character data, with `&lt;`, `&amp;` and friends standing for the characters XML reserves.
    Text {
        /// Whether the body spells `>` as `&gt;` anywhere, which a rewrite then keeps doing.
        escapes_gt: bool,
    },
    /// A single `CDATA` section, with nothing but whitespace around it.
    CData,
    /// Any other mix of text and `CDATA` sections. It is linted, but never rewritten.
    Mixed,
}

/// The Python body of one `code` field.
#[derive(Debug, Clone)]
pub struct CodeField {
    /// The code Odoo compiles, followed by a newline, and dedented by [`CodeField::indent`].
    pub code: String,
    /// The indentation removed from every line of the code but the first; empty unless the code
    /// is a single compound statement indented under its `<field>` tag.
    pub indent: String,
    /// The names the record adds to the evaluation context on top of [`eval_context_names`]: the
    /// `ai` dictionary and the arguments of an AI tool.
    pub names: Vec<String>,
    /// How the body is written in the XML document.
    pub encoding: BodyEncoding,
    /// Whether the code starts a line of its own after some indentation, whether or not that
    /// indentation could be removed from the lines that follow.
    starts_indented: bool,
    /// The range of the code in the XML document (without the trailing newline, which is not
    /// written anywhere).
    range: TextRange,
    /// For every byte of `code`, the range of the XML text it was decoded from.
    origins: Vec<TextRange>,
}

impl CodeField {
    /// Maps an offset in [`CodeField::code`] to the offset it came from in the XML document.
    pub fn to_xml_offset(&self, offset: TextSize) -> TextSize {
        self.origins
            .get(offset.to_usize())
            .map_or(self.range.end(), Ranged::start)
    }

    /// Maps a range in [`CodeField::code`] to the range it came from in the XML document.
    ///
    /// The end is taken from the last byte inside the range rather than from the first one after
    /// it, so a range ending just before an entity or a `CDATA` boundary does not swallow it.
    pub fn to_xml_range(&self, range: TextRange) -> TextRange {
        let start = self.to_xml_offset(range.start());
        if range.is_empty() {
            return TextRange::empty(start);
        }
        let end = self
            .origins
            .get(range.end().to_usize() - 1)
            .map_or(self.range.end(), Ranged::end);
        TextRange::new(start, end.max(start))
    }
}

impl Ranged for CodeField {
    fn range(&self) -> TextRange {
        self.range
    }
}

/// Returns the Python body of every `ir.cron` and `ir.actions.server` record in `source` that
/// Odoo would execute.
///
/// A record is skipped when its `state` field is set to anything other than `code`, since Odoo
/// then never looks at the `code` field; when the `state` is not set, the field is linted, because
/// a scheduled action defaults to `code` and an XML record that only overrides the `code` of an
/// existing server action keeps whatever `state` it already had. A `code` field whose value comes
/// from an `eval`, `ref` or `file` attribute is skipped as well, since its text is not the code.
///
/// Returns nothing for a document that is not well-formed XML. The fields come in document
/// order, including those of a record nested in a field of another record.
pub fn extract_code_fields(source: &str) -> Vec<CodeField> {
    // The reader is given the document without its byte order mark, so that the positions it
    // reports, shifted by the mark's length, index `source` itself.
    let bom = if source.starts_with('\u{feff}') {
        '\u{feff}'.text_len()
    } else {
        TextSize::default()
    };
    let mut reader = Reader::from_str(&source[bom.to_usize()..]);
    let mut fields = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();

    loop {
        let Ok(event) = reader.read_event() else {
            return Vec::new();
        };
        let after = bom + position(&reader);

        if let Some(Frame::Field(field)) = stack.last_mut()
            && field.open
        {
            match event {
                Event::Text(_) | Event::GeneralRef(_) | Event::CData(_) => {
                    field.body_end = after;
                    continue;
                }
                // `node.text` stops at the first child, comments included.
                _ => field.open = false,
            }
        }

        match event {
            Event::Start(start) => {
                let frame = match stack.last() {
                    _ if start.name().as_ref() == b"record" => Frame::Record(Record::new(&start)),
                    Some(Frame::Record(_)) if start.name().as_ref() == b"field" => {
                        Frame::Field(Field::new(&start, after))
                    }
                    _ => Frame::Other,
                };
                stack.push(frame);
            }
            Event::End(_) => match stack.pop() {
                Some(Frame::Field(field)) => {
                    let Some(Frame::Record(record)) = stack.last_mut() else {
                        continue;
                    };
                    if field.marks_ai_tool {
                        record.ai_tool = true;
                    }
                    if field.has_value_attribute {
                        if field.name.as_deref() == Some("state") {
                            // The state is computed; it may well be `code`.
                            record.state = None;
                        }
                        continue;
                    }
                    let body = TextRange::new(field.body_start, field.body_end);
                    match field.name.as_deref() {
                        Some("code") => record.code = Some(body),
                        Some("state") => {
                            record.state = decode(source, body)
                                .map(|decoded| decoded.text().trim().to_string());
                        }
                        Some("ai_tool_schema") => {
                            record.ai_tool_schema =
                                decode(source, body).map(|decoded| decoded.text());
                        }
                        _ => {}
                    }
                }
                Some(Frame::Record(record)) => {
                    if let Some(field) = record.into_code_field(source) {
                        fields.push(field);
                    }
                }
                _ => {}
            },
            Event::Empty(empty) => {
                if let Some(Frame::Record(record)) = stack.last_mut()
                    && empty.name().as_ref() == b"field"
                {
                    let field = Field::new(&empty, after);
                    if field.marks_ai_tool {
                        record.ai_tool = true;
                    }
                    if field.name.as_deref() == Some("state") {
                        // The state is computed, or empty; either way it is not known.
                        record.state = None;
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    // A record nested in a field closes, and is collected, before the record around it.
    fields.sort_by_key(Ranged::start);
    fields
}

/// The result of formatting the code fields of an XML document.
#[derive(Debug, PartialEq, Eq)]
pub enum XmlFormatResult {
    Formatted(String),
    Unchanged,
}

/// Formats the Python body of every `ir.cron` and `ir.actions.server` record in `source`.
///
/// Only the code itself is rewritten: the whitespace Odoo strips around it, the surrounding
/// markup, the line endings and the way the body is written (escaped text or a `CDATA` section)
/// are kept, and a compound statement indented under its tag stays indented. The line width is
/// the one `check` measures with `E501`: the code's own, not counting that indentation. A body
/// that does not parse, that mixes text with `CDATA` sections, that is indented under its tag but
/// could not be dedented, or whose re-indented result would no longer parse to the same syntax
/// tree, is left alone.
pub fn format_code_fields(
    source: &str,
    path: Option<&Path>,
    settings: &FormatterSettings,
) -> XmlFormatResult {
    let document_crlf = source.contains("\r\n");
    let mut output = String::with_capacity(source.len());
    let mut last = TextSize::default();

    for field in extract_code_fields(source) {
        // Formatting it would put the lines after the first back at the start of the line, under
        // a first line that stays indented: valid, but a layout nobody writes.
        if field.starts_indented && field.indent.is_empty() && field.code.trim_end().contains('\n')
        {
            continue;
        }
        // The decoded code always uses `\n`; the document's line endings are put back below,
        // whatever `line-ending` says.
        let options = settings
            .to_format_options(PySourceType::Python, &field.code, path)
            .with_line_ending(LineEnding::LineFeed);
        let Ok(formatted) = format_module_source(&field.code, options).map(Printed::into_code)
        else {
            continue;
        };
        if formatted == field.code {
            continue;
        }

        let formatted = reindent(formatted.trim_end(), &field.indent);
        if !field.indent.is_empty() && !same_syntax_tree(&field.code, &formatted) {
            continue;
        }
        let Some(mut replacement) = encode(&formatted, field.encoding) else {
            continue;
        };
        let raw = &source[field.range()];
        if raw.contains("\r\n") || (!raw.contains('\n') && document_crlf) {
            replacement = replacement.replace('\n', "\r\n");
        }

        output.push_str(&source[TextRange::new(last, field.start())]);
        output.push_str(&replacement);
        last = field.end();
    }

    if last == TextSize::default() {
        XmlFormatResult::Unchanged
    } else {
        output.push_str(&source[last.to_usize()..]);
        XmlFormatResult::Formatted(output)
    }
}

/// Prefixes every line of `code` but the first with `indent`, leaving empty lines empty.
fn reindent(code: &str, indent: &str) -> String {
    if indent.is_empty() {
        return code.to_string();
    }
    let mut reindented = String::with_capacity(code.len());
    for (index, line) in code.split('\n').enumerate() {
        if index > 0 {
            reindented.push('\n');
            if !line.is_empty() {
                reindented.push_str(indent);
            }
        }
        reindented.push_str(line);
    }
    reindented
}

/// Returns `true` if both sources parse, and to the same syntax tree.
fn same_syntax_tree(left: &str, right: &str) -> bool {
    let (Ok(left), Ok(right)) = (parse_module(left), parse_module(right)) else {
        return false;
    };
    ComparableModModule::from(left.syntax()) == ComparableModModule::from(right.syntax())
}

/// Writes `code` back in the given encoding, or returns `None` if it cannot be.
fn encode(code: &str, encoding: BodyEncoding) -> Option<String> {
    match encoding {
        BodyEncoding::Mixed => None,
        BodyEncoding::CData => (!code.contains("]]>")).then(|| code.to_string()),
        BodyEncoding::Text { escapes_gt } => {
            // `]]>` may not appear in character data, so its `>` is escaped whatever the style.
            let escape_gt = escapes_gt || code.contains("]]>");
            let mut encoded = String::with_capacity(code.len());
            for ch in code.chars() {
                match ch {
                    '&' => encoded.push_str("&amp;"),
                    '<' => encoded.push_str("&lt;"),
                    '>' if escape_gt => encoded.push_str("&gt;"),
                    _ => encoded.push(ch),
                }
            }
            Some(encoded)
        }
    }
}

fn position(reader: &Reader<&[u8]>) -> TextSize {
    TextSize::try_from(usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX))
        .unwrap_or(TextSize::new(u32::MAX))
}

fn attribute(start: &BytesStart, name: &str) -> Option<String> {
    let attribute = start.try_get_attribute(name).ok()??;
    Some(
        attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .ok()?
            .into_owned(),
    )
}

enum Frame {
    Record(Record),
    Field(Field),
    Other,
}

struct Record {
    model: Option<String>,
    /// The value of the `state` field, when the record sets it to a literal.
    state: Option<String>,
    /// The range of the `code` field's text, when the record sets it.
    code: Option<TextRange>,
    /// Whether the record is an AI tool, which Odoo runs with the `ai` dictionary and the tool's
    /// arguments in its evaluation context.
    ai_tool: bool,
    /// The JSON schema of the AI tool's arguments.
    ai_tool_schema: Option<String>,
}

impl Record {
    fn new(start: &BytesStart) -> Self {
        Self {
            model: attribute(start, "model"),
            state: None,
            code: None,
            ai_tool: false,
            ai_tool_schema: None,
        }
    }

    fn into_code_field(self, source: &str) -> Option<CodeField> {
        if !self
            .model
            .as_deref()
            .is_some_and(|model| CODE_MODELS.contains(&model))
        {
            return None;
        }
        if self.state.is_some_and(|state| state != "code") {
            return None;
        }
        let names = if self.ai_tool {
            let mut names = vec!["ai".to_string()];
            names.extend(
                self.ai_tool_schema
                    .as_deref()
                    .map(ai_tool_arguments)
                    .unwrap_or_default(),
            );
            names
        } else {
            Vec::new()
        };
        let decoded = decode(source, self.code?)?;
        decoded.into_code_field(names)
    }
}

/// Returns `true` for the fields only an AI tool sets: the flags that expose a server action to
/// Odoo's AI agents and MCP server, and the tool's name and argument schema.
fn is_ai_tool_field(name: &str) -> bool {
    matches!(
        name,
        "use_in_ai" | "use_in_mcp" | "ai_tool_name" | "ai_tool_schema"
    )
}

/// Returns the names of the arguments an AI tool's JSON schema declares, which Odoo passes to the
/// tool's code as variables.
fn ai_tool_arguments(schema: &str) -> Vec<String> {
    let Ok(serde_json::Value::Object(schema)) = serde_json::from_str(schema) else {
        return Vec::new();
    };
    match schema.get("properties") {
        Some(serde_json::Value::Object(properties)) => properties.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

struct Field {
    name: Option<String>,
    /// Whether the value comes from an attribute rather than from the text.
    has_value_attribute: bool,
    /// Whether the field marks its record as an AI tool: one only an AI tool sets, and not
    /// explicitly set to false.
    marks_ai_tool: bool,
    /// Whether the text before the first child is still being read.
    open: bool,
    body_start: TextSize,
    body_end: TextSize,
}

impl Field {
    fn new(start: &BytesStart, body_start: TextSize) -> Self {
        let name = attribute(start, "name");
        let disabled = attribute(start, "eval")
            .is_some_and(|value| matches!(value.trim(), "False" | "0" | "None" | ""));
        Self {
            marks_ai_tool: name.as_deref().is_some_and(is_ai_tool_field) && !disabled,
            name,
            has_value_attribute: ["eval", "ref", "file", "search"]
                .iter()
                .any(|name| matches!(start.try_get_attribute(name), Ok(Some(_)))),
            open: true,
            body_start,
            body_end: body_start,
        }
    }
}

/// One character of decoded text and where it came from.
#[derive(Clone, Copy)]
struct Char {
    ch: char,
    origin: TextRange,
    /// The `CDATA` section it was read from, numbered from 1; 0 for character data.
    cdata: u32,
}

struct Decoded {
    chars: Vec<Char>,
    /// Whether the raw text spells `>` as `&gt;` anywhere.
    escapes_gt: bool,
}

impl Decoded {
    fn text(&self) -> String {
        text(&self.chars)
    }

    /// Strips the text the way Python's `str.strip()` does and keeps what remains, or returns
    /// `None` if nothing does.
    fn into_code_field(self, names: Vec<String>) -> Option<CodeField> {
        let first = self
            .chars
            .iter()
            .position(|char| !is_python_space(char.ch))?;
        let last = self
            .chars
            .iter()
            .rposition(|char| !is_python_space(char.ch))?;
        let chars = &self.chars[first..=last];
        let (first_char, last_char) = (chars.first()?, chars.last()?);

        let sections = chars.iter().map(|char| char.cdata);
        let encoding = if sections.clone().all(|section| section == 0) {
            BodyEncoding::Text {
                escapes_gt: self.escapes_gt,
            }
        } else if first_char.cdata != 0
            && sections.clone().all(|section| section == first_char.cdata)
        {
            BodyEncoding::CData
        } else {
            BodyEncoding::Mixed
        };

        // The indentation of the first line, when the code starts a line of its own.
        let indent: String = self.chars[..first]
            .iter()
            .rposition(|char| char.ch == '\n')
            .map(|newline| {
                self.chars[newline + 1..first]
                    .iter()
                    .map(|char| char.ch)
                    .collect()
            })
            .unwrap_or_default();
        let starts_indented = !indent.is_empty();
        let (kept, indent) = match dedent(chars, &indent) {
            Some(dedented) if same_syntax_tree(&text(chars), &text(dedented.iter().copied())) => {
                (dedented, indent)
            }
            _ => (chars.iter().collect(), String::new()),
        };

        let mut code = String::with_capacity(kept.len() + 1);
        let mut origins = Vec::with_capacity(kept.len());
        for char in kept {
            code.push(char.ch);
            origins.extend(std::iter::repeat_n(char.origin, char.ch.len_utf8()));
        }
        code.push('\n');

        Some(CodeField {
            code,
            indent,
            names,
            encoding,
            starts_indented,
            range: TextRange::new(first_char.origin.start(), last_char.origin.end()),
            origins,
        })
    }
}

fn text<'a>(chars: impl IntoIterator<Item = &'a Char>) -> String {
    chars.into_iter().map(|char| char.ch).collect()
}

/// Returns `true` for the characters Python's `str.isspace()` accepts, which are Unicode's
/// whitespace plus the four ASCII separators U+001C to U+001F.
fn is_python_space(ch: char) -> bool {
    ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch)
}

/// Removes `indent` from every line of `chars` but the first, or returns `None` if a line that is
/// not blank does not start with it.
fn dedent<'a>(chars: &'a [Char], indent: &str) -> Option<Vec<&'a Char>> {
    if indent.is_empty() {
        return None;
    }
    let indent_len = indent.chars().count();
    let mut lines = chars.split_inclusive(|char| char.ch == '\n');
    let mut kept: Vec<&Char> = lines.next()?.iter().collect();
    let mut dedented = false;
    for line in lines {
        let skip = if line.iter().all(|char| char.ch.is_whitespace()) {
            line.iter()
                .take_while(|char| char.ch != '\n')
                .take(indent_len)
                .count()
        } else if line
            .iter()
            .map(|char| char.ch)
            .take(indent_len)
            .eq(indent.chars())
        {
            dedented = true;
            indent_len
        } else {
            return None;
        };
        kept.extend(&line[skip..]);
    }
    dedented.then_some(kept)
}

/// Decodes the text and `CDATA` sections in `range` the way an XML parser does: entities and
/// character references are replaced, and line endings are normalized to `\n`.
///
/// Returns `None` for an entity the XML specification does not predefine, which an XML parser
/// rejects anyway.
fn decode(source: &str, range: TextRange) -> Option<Decoded> {
    const CDATA_START: &str = "<![CDATA[";
    const CDATA_END: &str = "]]>";

    let mut chars = Vec::with_capacity(range.len().to_usize());
    let mut escapes_gt = false;
    let mut cdata = 0;
    let mut in_cdata = false;
    let mut offset = range.start();

    while offset < range.end() {
        let rest = &source[TextRange::new(offset, range.end())];
        if !in_cdata && rest.starts_with(CDATA_START) {
            in_cdata = true;
            cdata += 1;
            offset += CDATA_START.text_len();
            continue;
        }
        if in_cdata && rest.starts_with(CDATA_END) {
            in_cdata = false;
            offset += CDATA_END.text_len();
            continue;
        }

        let ch = rest.chars().next()?;
        let (ch, len) = match ch {
            '&' if !in_cdata => {
                let reference = &rest[1..rest.find(';')?];
                let decoded = match reference {
                    "lt" => '<',
                    "gt" => {
                        escapes_gt = true;
                        '>'
                    }
                    "amp" => '&',
                    "quot" => '"',
                    "apos" => '\'',
                    reference => {
                        let code = if let Some(hex) = reference.strip_prefix("#x") {
                            u32::from_str_radix(hex, 16).ok()?
                        } else {
                            reference.strip_prefix('#')?.parse().ok()?
                        };
                        char::from_u32(code)?
                    }
                };
                (
                    decoded,
                    '&'.text_len() + reference.text_len() + ';'.text_len(),
                )
            }
            '\r' if rest[1..].starts_with('\n') => ('\n', "\r\n".text_len()),
            '\r' => ('\n', '\r'.text_len()),
            ch => (ch, ch.text_len()),
        };
        chars.push(Char {
            ch,
            origin: TextRange::at(offset, len),
            cdata: if in_cdata { cdata } else { 0 },
        });
        offset += len;
    }

    Some(Decoded { chars, escapes_gt })
}
