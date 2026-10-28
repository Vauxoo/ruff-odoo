//! Domains written in an Odoo XML file.
//!
//! A domain is a Python expression that Odoo evaluates (`safe_eval`) with the fields of the form,
//! `uid`, `user`, `context_today` and a few more. This module finds the places of an XML file
//! that hold one and returns each as a [`CodeField`], so that it can be linted and its
//! diagnostics reported at the line and column of the XML file, like the body of a `code` field.
//!
//! The places are:
//!
//! - the `domain`, `filter_domain` and `attrs` attributes of any element (`<field domain="...">`,
//!   `<filter domain="...">`, `<field filter_domain="...">`, `<field attrs="...">`);
//! - the text of `<attribute name="domain">` and `<attribute name="attrs">`, the form an
//!   inheriting view changes them in;
//! - the text, or the `eval` attribute, of a `domain`, `domain_force` or `filter_domain` field of
//!   a record (`ir.rule`, `ir.actions.act_window`, `ir.filters`, `ir.model.fields`).
//!
//! A field whose value comes from `ref`, `file` or `search` is skipped, since its text is not a
//! domain. Nothing here knows which model the domain is for.

use ruff_text_size::{TextLen, TextRange, TextSize};

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::{CodeField, decode, position};

/// The attributes of any element that hold a domain, or a dictionary of them.
const DOMAIN_ATTRIBUTES: &[&str] = &["domain", "filter_domain", "attrs"];

/// The names of an `<attribute name="...">` or a `<field name="...">` that hold a domain.
const DOMAIN_NAMES: &[&str] = &["domain", "filter_domain", "domain_force", "attrs"];

/// An attribute of a start tag and where its value is written in the document.
struct RawAttribute<'a> {
    name: &'a str,
    value: TextRange,
}

/// The attributes of the start tag `tag`, which begins at `start` in the document.
///
/// `quick_xml` does not tell where in the document an attribute is, and the value has to be
/// mapped back to the document to report a diagnostic there, so the tag is read by hand: a name,
/// an `=`, and a value in single or double quotes, which is how XML spells every attribute.
fn raw_attributes(tag: &str, start: TextSize) -> Vec<RawAttribute<'_>> {
    let bytes = tag.as_bytes();
    let mut attributes = Vec::new();
    // Skip `<` and the name of the element.
    let mut index = 1;
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'>' {
        index += 1;
    }
    loop {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let name_start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && !matches!(bytes[index], b'=' | b'>' | b'/')
        {
            index += 1;
        }
        if name_start == index {
            return attributes;
        }
        let name = &tag[name_start..index];
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            return attributes;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let Some(&quote) = bytes.get(index).filter(|byte| matches!(byte, b'"' | b'\'')) else {
            return attributes;
        };
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index >= bytes.len() {
            return attributes;
        }
        let offset = |index: usize| start + TextSize::try_from(index).unwrap_or_default();
        attributes.push(RawAttribute {
            name,
            value: TextRange::new(offset(value_start), offset(index)),
        });
        index += 1;
    }
}

/// An element whose text is a domain, while its text is being read.
struct Pending {
    body_start: TextSize,
    body_end: TextSize,
}

/// Returns every domain in `source`, in document order.
///
/// Returns nothing for a document that is not well-formed XML.
pub(crate) fn extract_domain_fields(source: &str) -> Vec<CodeField> {
    let bom = if source.starts_with('\u{feff}') {
        '\u{feff}'.text_len()
    } else {
        TextSize::default()
    };
    let mut reader = Reader::from_str(&source[usize::from(bom)..]);
    let mut fields = Vec::new();
    // One entry per open element: whether its text is a domain.
    let mut stack: Vec<Option<Pending>> = Vec::new();

    loop {
        let before = bom + position(&reader);
        let Ok(event) = reader.read_event() else {
            return Vec::new();
        };
        let after = bom + position(&reader);

        if let Some(Some(pending)) = stack.last_mut()
            && matches!(
                event,
                Event::Text(_) | Event::GeneralRef(_) | Event::CData(_)
            )
        {
            pending.body_end = after;
            continue;
        }

        match event {
            Event::Start(_) | Event::Empty(_) => {
                let tag = &source[TextRange::new(before, after)];
                let attributes = raw_attributes(tag, before);
                let is_empty = matches!(event, Event::Empty(_));
                let value_of = |name: &str| -> Option<&str> {
                    attributes
                        .iter()
                        .find(|attribute| attribute.name == name)
                        .map(|attribute| &source[attribute.value])
                };

                for attribute in &attributes {
                    if DOMAIN_ATTRIBUTES.contains(&attribute.name) {
                        push(&mut fields, source, attribute.value);
                    }
                }

                let named = value_of("name").filter(|name| DOMAIN_NAMES.contains(name));
                let element = tag[1..]
                    .split(|ch: char| ch.is_whitespace() || matches!(ch, '/' | '>'))
                    .next()
                    .unwrap_or_default();
                let is_attribute = element == "attribute";
                let is_field = element == "field";
                let value_attribute = attributes.iter().find(|attribute| attribute.name == "eval");
                let from_elsewhere = ["ref", "file", "search"]
                    .iter()
                    .any(|name| value_of(name).is_some());

                let mut body = None;
                if named.is_some() && (is_attribute || is_field) && !from_elsewhere {
                    match value_attribute {
                        Some(eval) if is_field => push(&mut fields, source, eval.value),
                        _ if !is_empty => {
                            body = Some(Pending {
                                body_start: after,
                                body_end: after,
                            });
                        }
                        _ => {}
                    }
                }
                if !is_empty {
                    stack.push(body);
                }
            }
            Event::End(_) => {
                if let Some(Some(pending)) = stack.pop() {
                    push(
                        &mut fields,
                        source,
                        TextRange::new(pending.body_start, pending.body_end),
                    );
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    fields
}

/// Decodes the text in `range` and adds it to `fields` when it can hold a condition.
fn push(fields: &mut Vec<CodeField>, source: &str, range: TextRange) {
    // Every domain operator that takes a collection is spelled with `in`.
    if !source[range].contains("in") {
        return;
    }
    if let Some(field) =
        decode(source, range).and_then(|decoded| decoded.into_code_field(Vec::new(), None))
    {
        fields.push(field);
    }
}
