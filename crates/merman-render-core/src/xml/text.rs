use crate::entities::decode_mermaid_entities_for_render_text;
use crate::svg_output::SvgOutput;

pub fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_xml_into(&mut out, text);
    out
}

fn xml_text_is_plain_ascii(text: &str) -> bool {
    !text.contains("]]>")
        && text.bytes().all(|b| {
            matches!(b, b'\t' | b'\n' | b'\r' | 0x20..=0x7f)
                && !matches!(b, b'&' | b'<' | b'"' | b'\'' | b'#')
        })
}

fn xml_raw_text_is_plain_ascii(text: &str) -> bool {
    !text.contains("]]>")
        && text.bytes().all(|b| {
            matches!(b, b'\t' | b'\n' | b'\r' | 0x20..=0x7f)
                && !matches!(b, b'&' | b'<' | b'"' | b'\'')
        })
}

fn xml_text_replacement(
    ch: char,
    trailing_brackets: &mut u8,
    escape_greater_than: bool,
) -> Option<&'static str> {
    if !super::is_xml_1_0_char(ch) {
        // Removing a scalar preserves the bracket run in the text that will actually be emitted.
        return Some("");
    }
    let replacement = match ch {
        '&' => Some("&amp;"),
        '<' => Some("&lt;"),
        '"' => Some("&quot;"),
        '\'' => Some("&#39;"),
        '>' if escape_greater_than || *trailing_brackets == 2 => Some("&gt;"),
        _ => None,
    };
    *trailing_brackets = if ch == ']' {
        (*trailing_brackets + 1).min(2)
    } else {
        0
    };
    replacement
}

pub fn escape_xml_into(out: &mut impl SvgOutput, text: &str) {
    if xml_text_is_plain_ascii(text) {
        out.push_str(text);
        return;
    }

    let decoded = decode_mermaid_entities_for_render_text(text);
    escape_xml_raw_into(out, decoded.as_ref());
}

pub fn escape_xml_raw_into(out: &mut impl SvgOutput, text: &str) {
    escape_xml_raw_into_mode(out, text, false);
}

/// Serialize a text node with the same conservative escaping used by browser XMLSerializer.
/// Chromium emits `&gt;` for every literal greater-than character in text content, even though
/// XML only requires that escape when it would close a CDATA section.
pub fn escape_xml_serialized_text_into(out: &mut impl SvgOutput, text: &str) {
    escape_xml_raw_into_mode(out, text, true);
}

fn escape_xml_raw_into_mode(out: &mut impl SvgOutput, text: &str, escape_greater_than: bool) {
    let plain_ascii = if escape_greater_than {
        text.bytes().all(|b| {
            matches!(b, b'\t' | b'\n' | b'\r' | 0x20..=0x7f)
                && !matches!(b, b'&' | b'<' | b'"' | b'\'' | b'>')
        })
    } else {
        xml_raw_text_is_plain_ascii(text)
    };
    if plain_ascii && !text.contains("]]>") {
        out.push_str(text);
        return;
    }

    let mut start = 0usize;
    let mut trailing_brackets = 0;
    for (i, ch) in text.char_indices() {
        let replacement = xml_text_replacement(ch, &mut trailing_brackets, escape_greater_than);
        let Some(replacement) = replacement else {
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

pub fn escape_xml_display(text: &str) -> EscapeXmlDisplay<'_> {
    EscapeXmlDisplay(text)
}

pub fn escaped_xml_len(text: &str) -> usize {
    if xml_text_is_plain_ascii(text) {
        return text.len();
    }

    let decoded = decode_mermaid_entities_for_render_text(text);
    let text = decoded.as_ref();
    let mut trailing_brackets = 0;
    text.chars().fold(0usize, |len, ch| {
        let replacement = xml_text_replacement(ch, &mut trailing_brackets, false);
        len.saturating_add(replacement.map_or(ch.len_utf8(), str::len))
    })
}

pub struct EscapeXmlDisplay<'a>(&'a str);

impl std::fmt::Display for EscapeXmlDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if xml_text_is_plain_ascii(self.0) {
            return f.write_str(self.0);
        }

        let decoded = decode_mermaid_entities_for_render_text(self.0);
        let text = decoded.as_ref();
        let mut start = 0usize;
        let mut trailing_brackets = 0;
        for (i, ch) in text.char_indices() {
            let replacement = xml_text_replacement(ch, &mut trailing_brackets, false);
            let Some(replacement) = replacement else {
                continue;
            };
            if start < i {
                f.write_str(&text[start..i])?;
            }
            f.write_str(replacement)?;
            start = i + ch.len_utf8();
        }
        if start < text.len() {
            f.write_str(&text[start..])?;
        }
        Ok(())
    }
}
