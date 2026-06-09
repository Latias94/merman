pub(crate) fn find_matching_brace(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in text[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn find_tag_end(svg: &str, start: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, ch) in svg[start..].char_indices() {
        match ch {
            '"' | '\'' if quote == Some(ch) => quote = None,
            '"' | '\'' if quote.is_none() => quote = Some(ch),
            '>' if quote.is_none() => return Some(start + offset),
            _ => {}
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SvgQuotedAttr {
    pub(crate) full_start: usize,
    pub(crate) full_end: usize,
    pub(crate) name_start: usize,
    pub(crate) name_end: usize,
    pub(crate) value_start: usize,
    pub(crate) value_end: usize,
}

pub(crate) fn next_svg_quoted_attr(tag: &str, from: usize) -> Option<SvgQuotedAttr> {
    let mut cursor = from;
    while cursor < tag.len() {
        let ch = tag.get(cursor..)?.chars().next()?;
        if ch.is_whitespace() {
            let full_start = cursor;
            let name_start = skip_svg_attr_whitespace(tag, cursor);
            if let Some(attr_match) = svg_quoted_attr_at(tag, full_start, name_start) {
                return Some(attr_match);
            }
            cursor = name_start;
        } else {
            cursor += ch.len_utf8();
        }
    }
    None
}

fn svg_quoted_attr_at(tag: &str, full_start: usize, name_start: usize) -> Option<SvgQuotedAttr> {
    let first = *tag.as_bytes().get(name_start)?;
    if !is_svg_attr_name_start_byte(first) {
        return None;
    }

    let name_end = consume_svg_attr_name(tag, name_start);
    let mut cursor = skip_svg_attr_whitespace(tag, name_end);
    if !tag.get(cursor..)?.starts_with('=') {
        return None;
    }
    cursor += 1;
    cursor = skip_svg_attr_whitespace(tag, cursor);

    let quote = tag.get(cursor..)?.chars().next()?;
    if !matches!(quote, '"' | '\'') {
        return None;
    }

    let value_start = cursor + quote.len_utf8();
    let value_end = value_start + tag.get(value_start..)?.find(quote)?;
    Some(SvgQuotedAttr {
        full_start,
        full_end: value_end + quote.len_utf8(),
        name_start,
        name_end,
        value_start,
        value_end,
    })
}

fn skip_svg_attr_whitespace(tag: &str, mut cursor: usize) -> usize {
    while let Some(ch) = tag.get(cursor..).and_then(|tail| tail.chars().next()) {
        if !ch.is_whitespace() {
            break;
        }
        cursor += ch.len_utf8();
    }
    cursor
}

fn consume_svg_attr_name(tag: &str, mut cursor: usize) -> usize {
    while let Some(b) = tag.as_bytes().get(cursor) {
        if !is_svg_attr_name_continue_byte(*b) {
            break;
        }
        cursor += 1;
    }
    cursor
}

fn is_svg_attr_name_start_byte(b: u8) -> bool {
    b.is_ascii_alphabetic() || matches!(b, b'_' | b':')
}

fn is_svg_attr_name_continue_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.')
}

pub(crate) fn extract_root_svg_id(svg: &str) -> Option<String> {
    let start = svg.find("<svg")?;
    let end = find_tag_end(svg, start)?;
    let tag = &svg[start..=end];
    extract_quoted_attr(tag, "id").map(ToOwned::to_owned)
}

pub(crate) fn extract_quoted_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let (start, end) = find_quoted_attr_value_span(tag, name)?;
    Some(tag[start..end].trim())
}

pub(crate) fn find_quoted_attr_value_span(tag: &str, name: &str) -> Option<(usize, usize)> {
    let mut cursor = 0usize;
    while let Some(attr) = next_svg_quoted_attr(tag, cursor) {
        if tag[attr.name_start..attr.name_end].eq_ignore_ascii_case(name) {
            return Some((attr.value_start, attr.value_end));
        }
        cursor = attr.full_end;
    }
    None
}

pub(crate) fn set_or_insert_quoted_attr(tag: &str, name: &str, value: &str) -> String {
    if let Some((value_start, value_end)) = find_quoted_attr_value_span(tag, name) {
        let mut out = String::with_capacity(tag.len() + value.len());
        out.push_str(&tag[..value_start]);
        out.push_str(value);
        out.push_str(&tag[value_end..]);
        return out;
    }

    let insert_at = tag
        .trim_end()
        .strip_suffix("/>")
        .map(|prefix| prefix.trim_end().len())
        .unwrap_or_else(|| tag.rfind('>').unwrap_or(tag.len()));
    let mut out = String::with_capacity(tag.len() + name.len() + value.len() + 4);
    out.push_str(&tag[..insert_at]);
    out.push(' ');
    out.push_str(name);
    out.push_str(r#"=""#);
    out.push_str(value);
    out.push('"');
    out.push_str(&tag[insert_at..]);
    out
}

pub(crate) fn escape_xml_attr(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_attr_helpers_handle_spacing_case_and_quote_style() {
        let tag = r#"<path DATA-note = 'keep' style = "fill:red" />"#;

        assert_eq!(extract_quoted_attr(tag, "data-note"), Some("keep"));
        assert_eq!(extract_quoted_attr(tag, "STYLE"), Some("fill:red"));

        let rewritten = set_or_insert_quoted_attr(tag, "style", "stroke:blue");
        assert!(
            rewritten.contains(r#"style = "stroke:blue""#),
            "{rewritten}"
        );

        let inserted = set_or_insert_quoted_attr(tag, "x", "10");
        assert!(inserted.contains(r#" x="10" />"#), "{inserted}");
    }
}
