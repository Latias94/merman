use std::fmt::Write as _;

use crate::svg_output::SvgOutput;

fn xml_attr_replacement(ch: char) -> Option<&'static str> {
    if !super::is_xml_1_0_char(ch) {
        return Some("");
    }
    match ch {
        '\n' => Some("&#10;"),
        '\r' => Some("&#13;"),
        '\t' => Some("&#9;"),
        '&' => Some("&amp;"),
        '<' => Some("&lt;"),
        '"' => Some("&quot;"),
        '\'' => Some("&#39;"),
        _ => None,
    }
}

pub fn escape_attr(text: &str) -> String {
    // Note: XML parsers normalize literal newlines/carriage-returns/tabs inside attribute values
    // into spaces. Mermaid's serialized SVGs typically encode those characters as numeric
    // character references (e.g. `&#10;`) to keep the attribute value stable across parsers.
    //
    // We mirror that behavior here to preserve parity for diagrams that embed newlines in IDs
    // (e.g. backtick-quoted multiline class names).
    let mut out = String::with_capacity(text.len());
    escape_attr_into(&mut out, text);
    out
}

pub fn escape_attr_into(out: &mut impl SvgOutput, text: &str) {
    let mut start = 0usize;
    for (i, ch) in text.char_indices() {
        let Some(replacement) = xml_attr_replacement(ch) else {
            continue;
        };
        if start < i {
            out.push_str(&text[start..i]);
        }
        out.push_str(replacement);
        start = i + ch.len_utf8();
    }
    if start < text.len() {
        out.push_str(&text[start..]);
    }
}

pub fn escaped_attr_len(text: &str) -> usize {
    text.chars().fold(0usize, |len, ch| {
        len.saturating_add(xml_attr_replacement(ch).map_or(ch.len_utf8(), str::len))
    })
}

pub fn escape_attr_display<T: std::fmt::Display>(value: T) -> EscapeAttrDisplay<T> {
    EscapeAttrDisplay(value)
}

pub struct EscapeAttrDisplay<T>(T);

impl<T: std::fmt::Display> std::fmt::Display for EscapeAttrDisplay<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct EscapedAttrWriter<'formatter, 'output> {
            formatter: &'formatter mut std::fmt::Formatter<'output>,
        }

        impl std::fmt::Write for EscapedAttrWriter<'_, '_> {
            fn write_str(&mut self, text: &str) -> std::fmt::Result {
                let mut start = 0usize;
                for (i, ch) in text.char_indices() {
                    let Some(replacement) = xml_attr_replacement(ch) else {
                        continue;
                    };
                    if start < i {
                        self.formatter.write_str(&text[start..i])?;
                    }
                    self.formatter.write_str(replacement)?;
                    start = i + ch.len_utf8();
                }
                if start < text.len() {
                    self.formatter.write_str(&text[start..])?;
                }
                Ok(())
            }
        }

        write!(EscapedAttrWriter { formatter: f }, "{}", self.0)
    }
}
