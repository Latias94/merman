//! Lexical scanning and text handling for SVG postprocessing.

#[cfg(feature = "svg-parse")]
use std::{borrow::Cow, ops::Range};

const POSTPROCESS_CHECKPOINT_BATCH: usize = 64;

pub fn checkpoint_loop<E>(
    iteration: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    if iteration.is_multiple_of(POSTPROCESS_CHECKPOINT_BATCH) {
        checkpoint()?;
    }
    Ok(())
}

fn fill_pattern_prefix_table<E>(
    pattern: &[u8],
    prefix: &mut [usize],
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    let mut matched = 0usize;
    for index in 1..pattern.len() {
        checkpoint_loop(index, checkpoint)?;
        while matched > 0 && pattern[index] != pattern[matched] {
            matched = prefix[matched - 1];
        }
        if pattern[index] == pattern[matched] {
            matched += 1;
            prefix[index] = matched;
        }
    }
    Ok(())
}

fn find_pattern_with_checkpoints<E>(
    haystack: &str,
    needle: &str,
    find_last: bool,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    if needle.is_empty() {
        checkpoint()?;
        return Ok(Some(if find_last { haystack.len() } else { 0 }));
    }

    let needle = needle.as_bytes();
    // Tag and markup delimiters fit inline, including on the bounds scanner's hot path.
    let mut short_prefix = [0; 8];
    let mut long_prefix;
    let prefix = if needle.len() <= short_prefix.len() {
        &mut short_prefix[..needle.len()]
    } else {
        long_prefix = vec![0; needle.len()];
        &mut long_prefix
    };
    fill_pattern_prefix_table(needle, prefix, checkpoint)?;
    let mut matched = 0usize;
    let mut last_match = None;
    for (index, byte) in haystack.bytes().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        while matched > 0 && byte != needle[matched] {
            matched = prefix[matched - 1];
        }
        if byte != needle[matched] {
            continue;
        }
        matched += 1;
        if matched != needle.len() {
            continue;
        }

        let start = index + 1 - needle.len();
        if !find_last {
            return Ok(Some(start));
        }
        last_match = Some(start);
        matched = prefix[matched - 1];
    }
    checkpoint()?;
    Ok(last_match)
}

/// Finds a UTF-8 substring in linear time while observing a cooperative checkpoint cadence.
pub fn find_with_checkpoints<E>(
    haystack: &str,
    needle: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    find_pattern_with_checkpoints(haystack, needle, false, checkpoint)
}

/// Finds the last UTF-8 substring in linear time while observing a cooperative checkpoint cadence.
pub fn rfind_with_checkpoints<E>(
    haystack: &str,
    needle: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    find_pattern_with_checkpoints(haystack, needle, true, checkpoint)
}

pub fn trim_with_checkpoints<'a, E>(
    value: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<&'a str, E> {
    let mut start = None;
    for (iteration, (index, character)) in value.char_indices().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if !character.is_whitespace() {
            start = Some(index);
            break;
        }
    }
    let Some(start) = start else {
        checkpoint()?;
        return Ok(&value[value.len()..]);
    };

    let mut end = value.len();
    for (iteration, (index, character)) in value[start..].char_indices().rev().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if !character.is_whitespace() {
            end = start + index + character.len_utf8();
            break;
        }
    }
    checkpoint()?;
    Ok(&value[start..end])
}

pub fn extract_exact_double_quoted_attr_with_checkpoints<'a, E>(
    tag: &'a str,
    name: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<&'a str>, E> {
    let needle = format!(r#"{name}=""#);
    let Some(start) = find_with_checkpoints(tag, &needle, checkpoint)? else {
        return Ok(None);
    };
    let value_start = start + needle.len();
    let Some(value_end) = find_with_checkpoints(&tag[value_start..], "\"", checkpoint)? else {
        return Ok(None);
    };
    trim_with_checkpoints(&tag[value_start..value_start + value_end], checkpoint).map(Some)
}

/// Returns the byte index of the first unquoted `>` at or after `start`.
pub fn find_tag_end(input: &str, start: usize) -> Option<usize> {
    match find_tag_end_with_checkpoints(
        input,
        start,
        &mut || Ok::<(), std::convert::Infallible>(()),
    ) {
        Ok(end) => end,
        Err(error) => match error {},
    }
}

pub fn find_tag_end_with_checkpoints<E>(
    input: &str,
    start: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    let Some(tail) = input.get(start..) else {
        checkpoint()?;
        return Ok(None);
    };
    let mut quote = None;
    for (iteration, (offset, character)) in tail.char_indices().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        match character {
            '\'' | '"' if quote == Some(character) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(character),
            '>' if quote.is_none() => return Ok(Some(start + offset)),
            _ => {}
        }
    }
    checkpoint()?;
    Ok(None)
}

fn find_declaration_end_with_checkpoints<E>(
    input: &str,
    start: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    let Some(tail) = input.get(start..) else {
        checkpoint()?;
        return Ok(None);
    };
    let mut quote = None;
    let mut subset_depth = 0usize;
    for (iteration, (offset, character)) in tail.char_indices().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        match character {
            '\'' | '"' if quote == Some(character) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(character),
            '[' if quote.is_none() => subset_depth = subset_depth.saturating_add(1),
            ']' if quote.is_none() => subset_depth = subset_depth.saturating_sub(1),
            '>' if quote.is_none() && subset_depth == 0 => return Ok(Some(start + offset)),
            _ => {}
        }
    }
    checkpoint()?;
    Ok(None)
}

fn find_delimited_markup_end_with_checkpoints<E>(
    input: &str,
    start: usize,
    prefix: &str,
    terminator: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    let Some(content) = input
        .get(start..)
        .and_then(|tail| tail.strip_prefix(prefix))
    else {
        return Ok(None);
    };
    find_with_checkpoints(content, terminator, checkpoint)
        .map(|end| end.map(|offset| start + prefix.len() + offset + terminator.len() - 1))
}

fn find_markup_end_with_checkpoints<E>(
    input: &str,
    start: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    if input
        .get(start..)
        .is_some_and(|tail| tail.starts_with("<!--"))
    {
        return find_delimited_markup_end_with_checkpoints(input, start, "<!--", "-->", checkpoint);
    }
    if input
        .get(start..)
        .is_some_and(|tail| tail.starts_with("<![CDATA["))
    {
        return find_delimited_markup_end_with_checkpoints(
            input,
            start,
            "<![CDATA[",
            "]]>",
            checkpoint,
        );
    }
    if input
        .get(start..)
        .is_some_and(|tail| tail.starts_with("<?"))
    {
        return find_delimited_markup_end_with_checkpoints(input, start, "<?", "?>", checkpoint);
    }
    if input
        .get(start..)
        .is_some_and(|tail| tail.starts_with("<!"))
    {
        return find_declaration_end_with_checkpoints(input, start, checkpoint);
    }
    find_tag_end_with_checkpoints(input, start, checkpoint)
}

#[derive(Debug, Clone, Copy)]
pub struct SvgTag<'a> {
    source: &'a str,
    start: usize,
    end: usize,
}

impl<'a> SvgTag<'a> {
    pub fn raw(self) -> &'a str {
        &self.source[self.start..=self.end]
    }

    pub fn start(self) -> usize {
        self.start
    }

    /// The exclusive byte offset of the tag's end.
    pub fn end(self) -> usize {
        self.end + 1
    }

    pub fn is_self_closing(self) -> bool {
        self.raw().ends_with("/>")
    }
}

pub struct SvgTagScanner<'a> {
    source: &'a str,
    cursor: usize,
}

impl<'a> SvgTagScanner<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source, cursor: 0 }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Seeks to the UTF-8 boundary at or immediately before the requested byte.
    pub fn skip_to(&mut self, cursor: usize) {
        self.cursor = self.source.floor_char_boundary(cursor);
    }

    pub fn next_tag(&mut self) -> Option<SvgTag<'a>> {
        let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
        match self.next_with_checkpoints(&mut checkpoint) {
            Ok(tag) => tag,
            Err(error) => match error {},
        }
    }

    pub fn next_with_checkpoints<E>(
        &mut self,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Option<SvgTag<'a>>, E> {
        let Some(rel_start) = find_with_checkpoints(&self.source[self.cursor..], "<", checkpoint)?
        else {
            return Ok(None);
        };
        let start = self.cursor + rel_start;
        let Some(end) = find_markup_end_with_checkpoints(self.source, start, checkpoint)? else {
            self.cursor = start;
            return Ok(None);
        };
        self.cursor = end + 1;
        Ok(Some(SvgTag {
            source: self.source,
            start,
            end,
        }))
    }

    /// Consumes the element just read, including nested elements with the same name.
    /// For an empty element, its opening tag also serves as the end.
    pub fn matching_end_with_checkpoints<E>(
        &mut self,
        opening: SvgTag<'a>,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Option<SvgTag<'a>>, E> {
        let Some(name) = start_tag_name(opening.raw()) else {
            return Ok(None);
        };
        if opening.is_self_closing() {
            return Ok(Some(opening));
        }
        let mut depth = 1usize;
        while let Some(tag) = self.next_with_checkpoints(checkpoint)? {
            if end_tag_name(tag.raw()) == Some(name) {
                depth -= 1;
                if depth == 0 {
                    return Ok(Some(tag));
                }
            } else if start_tag_name(tag.raw()) == Some(name) && !tag.is_self_closing() {
                depth += 1;
            }
        }
        Ok(None)
    }
}

/// SVG containers whose descendants do not paint at their definition site.
pub fn is_non_rendering_container(name: &str) -> bool {
    matches!(
        name,
        "defs"
            | "symbol"
            | "clipPath"
            | "mask"
            | "marker"
            | "pattern"
            | "filter"
            | "linearGradient"
            | "radialGradient"
            | "metadata"
            | "title"
            | "desc"
            | "style"
            | "script"
    )
}

#[derive(Debug, Clone, Copy)]
pub struct SvgQuotedAttr {
    pub full_start: usize,
    pub full_end: usize,
    pub name_start: usize,
    pub name_end: usize,
    pub value_start: usize,
    pub value_end: usize,
}

pub fn next_svg_quoted_attr(tag: &str, from: usize) -> Option<SvgQuotedAttr> {
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

pub fn next_svg_quoted_attr_with_checkpoints<E>(
    tag: &str,
    from: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<SvgQuotedAttr>, E> {
    let mut cursor = from;
    let mut iteration = 0usize;
    while cursor < tag.len() {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        let Some(ch) = tag.get(cursor..).and_then(|tail| tail.chars().next()) else {
            return Ok(None);
        };
        if ch.is_whitespace() {
            let full_start = cursor;
            let name_start = skip_svg_attr_whitespace_with_checkpoints(tag, cursor, checkpoint)?;
            if let Some(attr_match) =
                svg_quoted_attr_at_with_checkpoints(tag, full_start, name_start, checkpoint)?
            {
                return Ok(Some(attr_match));
            }
            cursor = name_start;
        } else {
            cursor += ch.len_utf8();
        }
    }
    checkpoint()?;
    Ok(None)
}

pub fn start_tag_name(tag: &str) -> Option<&str> {
    let tag = tag.trim_start().strip_prefix('<')?.strip_suffix('>')?;
    if !tag
        .as_bytes()
        .first()
        .is_some_and(|byte| is_svg_attr_name_start_byte(*byte))
        || (!tag.ends_with('/') && tag.trim_end().ends_with('/'))
    {
        return None;
    }

    let end = tag
        .find(|ch: char| ch.is_whitespace() || ch == '/' || ch == '>')
        .unwrap_or(tag.len());
    (end > 0).then_some(&tag[..end])
}

pub fn end_tag_name(tag: &str) -> Option<&str> {
    let name = tag
        .trim_start()
        .strip_prefix("</")?
        .strip_suffix('>')?
        .trim_end();
    let mut bytes = name.bytes();
    (bytes.next().is_some_and(is_svg_attr_name_start_byte)
        && bytes.all(is_svg_attr_name_continue_byte))
    .then_some(name)
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

fn svg_quoted_attr_at_with_checkpoints<E>(
    tag: &str,
    full_start: usize,
    name_start: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<SvgQuotedAttr>, E> {
    let Some(first) = tag.as_bytes().get(name_start).copied() else {
        return Ok(None);
    };
    if !is_svg_attr_name_start_byte(first) {
        return Ok(None);
    }

    let name_end = consume_svg_attr_name_with_checkpoints(tag, name_start, checkpoint)?;
    let mut cursor = skip_svg_attr_whitespace_with_checkpoints(tag, name_end, checkpoint)?;
    if !tag.get(cursor..).is_some_and(|tail| tail.starts_with('=')) {
        return Ok(None);
    }
    cursor += 1;
    cursor = skip_svg_attr_whitespace_with_checkpoints(tag, cursor, checkpoint)?;

    let Some(quote) = tag.get(cursor..).and_then(|tail| tail.chars().next()) else {
        return Ok(None);
    };
    if !matches!(quote, '"' | '\'') {
        return Ok(None);
    }

    let value_start = cursor + quote.len_utf8();
    let quote = if quote == '"' { "\"" } else { "'" };
    let Some(relative_end) = find_with_checkpoints(
        tag.get(value_start..).unwrap_or_default(),
        quote,
        checkpoint,
    )?
    else {
        return Ok(None);
    };
    let value_end = value_start + relative_end;
    Ok(Some(SvgQuotedAttr {
        full_start,
        full_end: value_end + quote.len(),
        name_start,
        name_end,
        value_start,
        value_end,
    }))
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

fn skip_svg_attr_whitespace_with_checkpoints<E>(
    tag: &str,
    mut cursor: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<usize, E> {
    let mut iteration = 0usize;
    while let Some(ch) = tag.get(cursor..).and_then(|tail| tail.chars().next()) {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        if !ch.is_whitespace() {
            break;
        }
        cursor += ch.len_utf8();
    }
    Ok(cursor)
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

fn consume_svg_attr_name_with_checkpoints<E>(
    tag: &str,
    mut cursor: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<usize, E> {
    let mut iteration = 0usize;
    while let Some(byte) = tag.as_bytes().get(cursor) {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        if !is_svg_attr_name_continue_byte(*byte) {
            break;
        }
        cursor += 1;
    }
    Ok(cursor)
}

fn is_svg_attr_name_start_byte(b: u8) -> bool {
    // Name validity is checked at document admission. Consume Unicode names whole so
    // attribute-like text within their quoted values cannot be mistaken for attributes.
    !b.is_ascii() || b.is_ascii_alphabetic() || matches!(b, b'_' | b':')
}

fn is_svg_attr_name_continue_byte(b: u8) -> bool {
    is_svg_attr_name_start_byte(b) || b.is_ascii_digit() || matches!(b, b'-' | b'.')
}

pub fn extract_quoted_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let (start, end) = find_quoted_attr_value_span(tag, name)?;
    Some(tag[start..end].trim())
}

pub fn extract_exact_quoted_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let (start, end) = find_quoted_attr_value_span_by(tag, |candidate| candidate == name)?;
    Some(tag[start..end].trim())
}

pub fn extract_quoted_attr_with_checkpoints<'a, E>(
    tag: &'a str,
    name: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<&'a str>, E> {
    let mut cursor = 0;
    while let Some(attr) = next_svg_quoted_attr_with_checkpoints(tag, cursor, checkpoint)? {
        if tag[attr.name_start..attr.name_end].eq_ignore_ascii_case(name) {
            return trim_with_checkpoints(&tag[attr.value_start..attr.value_end], checkpoint)
                .map(Some);
        }
        cursor = attr.full_end;
    }
    Ok(None)
}

pub fn find_quoted_attr_value_span(tag: &str, name: &str) -> Option<(usize, usize)> {
    find_quoted_attr_value_span_by(tag, |candidate| candidate.eq_ignore_ascii_case(name))
}

fn find_quoted_attr_value_span_by(
    tag: &str,
    matches_name: impl Fn(&str) -> bool,
) -> Option<(usize, usize)> {
    let mut cursor = 0usize;
    while let Some(attr) = next_svg_quoted_attr(tag, cursor) {
        if matches_name(&tag[attr.name_start..attr.name_end]) {
            return Some((attr.value_start, attr.value_end));
        }
        cursor = attr.full_end;
    }
    None
}

/// Replaces an attribute value or inserts a new double-quoted attribute.
///
/// The caller supplies a valid XML attribute name and a value already escaped for use in
/// either quote style. An existing attribute keeps its original delimiter.
pub fn set_or_insert_quoted_attr(tag: &str, name: &str, value: &str) -> String {
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

#[cfg(feature = "svg-parse")]
pub fn escape_xml_attr(value: &str) -> String {
    let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
    match escape_xml_attr_with_checkpoints(value, &mut checkpoint) {
        Ok(value) => value,
        Err(error) => match error {},
    }
}

/// Reads logical XML stylesheet text exactly once before handing it to a CSS parser.
/// CDATA stays literal, while XML references never enter CSS tokenization as source spelling.
#[cfg(feature = "svg-parse")]
pub fn decode_xml_style_text_with_checkpoints<'a, E>(
    serialized: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<std::borrow::Cow<'a, str>>, E> {
    XmlCssSource::new(serialized, CssXmlLocation::Stylesheet, checkpoint)
        .map(|source| source.map(|source| source.logical))
}

#[cfg(feature = "svg-parse")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CssXmlLocation {
    Attribute,
    Stylesheet,
}

/// Logical CSS and its original XML spelling. Re-project edits without serializing unaffected
/// entities, XML comments, or CDATA delimiters.
#[cfg(feature = "svg-parse")]
pub struct XmlCssSource<'a> {
    source: &'a str,
    logical: Cow<'a, str>,
    location: CssXmlLocation,
}

#[cfg(feature = "svg-parse")]
impl<'a> XmlCssSource<'a> {
    pub fn new<E>(
        source: &'a str,
        location: CssXmlLocation,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Option<Self>, E> {
        checkpoint()?;
        let logical = if source.contains(['&', '<'])
            || (location == CssXmlLocation::Attribute && source.contains(['\r', '\n', '\t']))
        {
            let mut decoded = String::with_capacity(source.len());
            if !visit_css_xml_fragments(source, location, checkpoint, |_, text| {
                decoded.push_str(text);
            })? {
                return Ok(None);
            }
            Cow::Owned(decoded)
        } else {
            Cow::Borrowed(source)
        };
        Ok(Some(Self {
            source,
            logical,
            location,
        }))
    }

    pub fn logical(&self) -> &str {
        &self.logical
    }

    /// Visits nonempty, ordered edits in source order. One logical range can cross several XML
    /// fragments, so each callback also receives the index of its originating edit.
    pub fn visit_source_spans<E>(
        &self,
        edits: &[Range<usize>],
        checkpoint: &mut impl FnMut() -> Result<(), E>,
        mut visit: impl FnMut(usize, Range<usize>),
    ) -> Result<(), E> {
        if edits.is_empty() {
            return Ok(());
        }
        if matches!(self.logical, Cow::Borrowed(_)) {
            for (index, edit) in edits.iter().enumerate() {
                checkpoint_loop(index, checkpoint)?;
                if !edit.is_empty() {
                    visit(index, edit.clone());
                }
            }
            return Ok(());
        }

        let mut index = 0;
        let mut logical_start = 0;
        visit_css_xml_fragments(self.source, self.location, checkpoint, |source, text| {
            let logical_end = logical_start + text.len();
            while let Some(edit) = edits.get(index).filter(|edit| edit.start < logical_end) {
                if edit.is_empty() {
                    index += 1;
                    continue;
                }
                // References and normalized line endings are each one logical character.
                let range = if source.len() == text.len() {
                    let start = edit.start.max(logical_start) - logical_start;
                    let end = edit.end.min(logical_end) - logical_start;
                    source.start + start..source.start + end
                } else {
                    source.clone()
                };
                visit(index, range);
                if edit.end > logical_end {
                    break;
                }
                index += 1;
            }
            logical_start = logical_end;
        })?;
        Ok(())
    }
}

#[cfg(feature = "svg-parse")]
fn visit_css_xml_fragments<E>(
    source: &str,
    location: CssXmlLocation,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
    mut visit: impl FnMut(Range<usize>, &str),
) -> Result<bool, E> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(source);
    loop {
        checkpoint()?;
        let start = reader.buffer_position() as usize;
        let event = reader.read_event();
        let end = reader.buffer_position() as usize;
        checkpoint()?;
        let fragment = match event {
            Ok(Event::Text(_)) => start..end,
            Ok(Event::CData(_)) if location == CssXmlLocation::Stylesheet => {
                start + "<![CDATA[".len()..end - "]]>".len()
            }
            Ok(Event::GeneralRef(reference)) => {
                let Ok(character) = crate::svg_xml::resolve_xml_reference_value(&reference) else {
                    return Ok(false);
                };
                visit(start..end, character.encode_utf8(&mut [0; 4]));
                continue;
            }
            Ok(Event::Comment(_) | Event::PI(_)) if location == CssXmlLocation::Stylesheet => {
                continue;
            }
            Ok(Event::Eof) => return Ok(true),
            _ => return Ok(false),
        };
        visit_normalized_css_xml_literal(source, fragment, location, checkpoint, &mut visit)?;
    }
}

#[cfg(feature = "svg-parse")]
fn visit_normalized_css_xml_literal<E>(
    source: &str,
    fragment: Range<usize>,
    location: CssXmlLocation,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
    visit: &mut impl FnMut(Range<usize>, &str),
) -> Result<(), E> {
    let attribute = location == CssXmlLocation::Attribute;
    // XML normalizes literal CRLF, and literal attribute whitespace becomes spaces.
    // Character references take a separate path and retain their values.
    let mut from = fragment.start;
    let text = &source[fragment.clone()];
    if attribute || text.contains('\r') {
        for (index, byte) in text.bytes().enumerate() {
            checkpoint_loop(index, checkpoint)?;
            let at = fragment.start + index;
            if at < from || !(byte == b'\r' || (attribute && matches!(byte, b'\t' | b'\n'))) {
                continue;
            }
            if at > from {
                visit(from..at, &source[from..at]);
            }
            from = at + 1;
            if byte == b'\r' && from < fragment.end && source.as_bytes()[from] == b'\n' {
                from += 1;
            }
            visit(at..from, if attribute { " " } else { "\n" });
        }
    }
    if from < fragment.end {
        visit(from..fragment.end, &source[from..fragment.end]);
    }
    Ok(())
}

#[cfg(feature = "svg-parse")]
pub fn escape_xml_text_with_checkpoints<E>(
    value: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    let mut out = String::with_capacity(value.len());
    for (iteration, ch) in value.chars().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if !crate::xml::is_xml_1_0_char(ch) {
            continue;
        }
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    checkpoint()?;
    Ok(out)
}

#[cfg(feature = "svg-parse")]
pub fn escape_xml_attr_with_checkpoints<E>(
    value: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    let mut out = String::with_capacity(value.len());
    for (iteration, ch) in value.chars().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if !crate::xml::is_xml_1_0_char(ch) {
            continue;
        }
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    checkpoint()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_end_ignores_quoted_delimiters_and_reports_utf8_byte_offset() {
        let input = "<tag title='x > y'>\u{03bb}</tag>";
        assert_eq!(find_tag_end(input, 0), input.find(">\u{03bb}"));
        let mut scanner = SvgTagScanner::new(input);
        let text_start = input.find('\u{03bb}').unwrap();
        scanner.skip_to(text_start + 1);
        assert_eq!(scanner.cursor(), text_start);
        assert_eq!(scanner.next_tag().unwrap().raw(), "</tag>");
    }

    #[test]
    #[cfg(feature = "svg-parse")]
    fn logical_stylesheet_xml_text_decodes_only_xml_references_once() {
        let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
        for (serialized, expected) in [
            (r#"&quot;A&amp;B&quot; &#x23;123456;"#, r#""A&B" #123456;"#),
            (r#"&amp;quot;#123456;"#, r#"&quot;#123456;"#),
            (r#"<![CDATA["A&amp;B" #123456;]]>"#, r#""A&amp;B" #123456;"#),
            (
                r#"font-family:monospace;fill:#123456;"#,
                r#"font-family:monospace;fill:#123456;"#,
            ),
        ] {
            assert_eq!(
                decode_xml_style_text_with_checkpoints(serialized, &mut checkpoint)
                    .unwrap()
                    .unwrap(),
                expected
            );
        }
        assert!(
            decode_xml_style_text_with_checkpoints("&unknown;", &mut checkpoint)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    #[cfg(feature = "svg-parse")]
    fn logical_stylesheet_xml_text_observes_cancellation_between_reference_events() {
        let serialized = "&amp;".repeat(256);
        let mut checkpoints = 0;
        let result = decode_xml_style_text_with_checkpoints(&serialized, &mut || {
            checkpoints += 1;
            if checkpoints == 5 {
                Err("cancelled")
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err("cancelled"));
    }

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

        let unicode = r#"<g 属性="before style='fake'" class="ordinary">"#;
        assert_eq!(
            extract_quoted_attr(unicode, "属性"),
            Some("before style='fake'")
        );
        assert_eq!(extract_quoted_attr(unicode, "style"), None);
        let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
        let attribute = next_svg_quoted_attr_with_checkpoints(unicode, 0, &mut checkpoint)
            .unwrap()
            .unwrap();
        assert_eq!(&unicode[attribute.name_start..attribute.name_end], "属性");
    }

    #[test]
    fn svg_tag_scanner_reports_tag_spans_and_names() {
        let svg = r#"<svg><g class="a > b"><rect width="1"/></g><!-- > <style>x</style> --><![CDATA[</style>]]></svg>"#;
        let mut scanner = SvgTagScanner::new(svg);

        let svg_tag = scanner.next_tag().unwrap();
        assert_eq!(svg_tag.raw(), "<svg>");
        assert_eq!(svg_tag.start(), 0);
        assert!(!svg_tag.is_self_closing());

        let g_tag = scanner.next_tag().unwrap();
        assert_eq!(g_tag.raw(), r#"<g class="a > b">"#);

        let rect_tag = scanner.next_tag().unwrap();
        assert!(rect_tag.is_self_closing());

        let g_close = scanner.next_tag().unwrap();
        assert_eq!(g_close.raw(), "</g>");

        let comment = scanner.next_tag().unwrap();
        assert_eq!(comment.raw(), "<!-- > <style>x</style> -->");

        let cdata = scanner.next_tag().unwrap();
        assert_eq!(cdata.raw(), "<![CDATA[</style>]]>");
    }
}
