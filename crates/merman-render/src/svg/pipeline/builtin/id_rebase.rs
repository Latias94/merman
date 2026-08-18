use crate::svg::pipeline::{
    SvgPostprocessContext, SvgPostprocessor, is_css_value_attribute, is_svg_idref_attribute,
};
use crate::{Error, Result};
use cssparser::{
    BasicParseErrorKind, CssStringWriter, Parser, ParserInput, Token, serialize_identifier,
};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesCData, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::io::Write as IoWrite;

#[derive(Debug, Clone)]
pub(crate) struct RebaseSvgIdsPostprocessor {
    prefix: String,
}

impl RebaseSvgIdsPostprocessor {
    pub(crate) fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: super::super::super::sanitize_svg_id(&prefix.into()),
        }
    }
}

impl SvgPostprocessor for RebaseSvgIdsPostprocessor {
    fn name(&self) -> &'static str {
        "rebase-svg-ids"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        let execution = ctx.execution();
        execution.checkpoint()?;
        let ids = collect_ids(&svg)?;
        if ids.is_empty() {
            return Ok(svg);
        }

        let projected_bytes = projected_rebased_xml_bytes(&svg, &ids, &self.prefix)?;
        execution.checkpoint()?;
        let Some(projected_bytes) = projected_bytes else {
            return Err(execution.svg_byte_count_overflow());
        };
        execution.preflight_svg_byte_count(projected_bytes)?;
        execution.checkpoint()?;

        let ids = ids.into_rebased_map(&self.prefix);
        rebase_xml(&svg, &ids, &self.prefix, projected_bytes).map(Cow::Owned)
    }
}

#[derive(Debug)]
struct CollectedIds {
    ids: Vec<String>,
}

impl CollectedIds {
    fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    fn contains_id(&self, id: &str) -> bool {
        self.ids
            .binary_search_by(|candidate| candidate.as_str().cmp(id))
            .is_ok()
    }

    fn into_rebased_map(self, prefix: &str) -> BTreeMap<String, String> {
        self.ids
            .into_iter()
            .map(|id| {
                let rebased = format!("{prefix}-{id}");
                (id, rebased)
            })
            .collect()
    }
}

trait RebaseIdLookup {
    fn contains_id(&self, id: &str) -> bool;

    fn rebased_id(&self, id: &str) -> Option<&str>;
}

impl RebaseIdLookup for CollectedIds {
    fn contains_id(&self, id: &str) -> bool {
        CollectedIds::contains_id(self, id)
    }

    fn rebased_id(&self, _id: &str) -> Option<&str> {
        None
    }
}

impl RebaseIdLookup for BTreeMap<String, String> {
    fn contains_id(&self, id: &str) -> bool {
        self.contains_key(id)
    }

    fn rebased_id(&self, id: &str) -> Option<&str> {
        self.get(id).map(String::as_str)
    }
}

fn collect_ids(svg: &str) -> Result<CollectedIds> {
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| rebase_error(format!("invalid SVG XML: {error}")))?;
    let mut ids = Vec::new();
    for node in document.descendants().filter(roxmltree::Node::is_element) {
        let Some(id) = node.attribute("id") else {
            continue;
        };
        if id.is_empty() {
            return Err(rebase_error("SVG id must not be empty"));
        }
        ids.push(id.to_string());
    }
    ids.sort_unstable();
    if let Some(duplicate) = ids.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(rebase_error(format!("duplicate SVG id {:?}", duplicate[0])));
    }
    Ok(CollectedIds { ids })
}

#[derive(Default)]
struct ProjectedByteCounter {
    bytes: usize,
    overflowed: bool,
}

impl ProjectedByteCounter {
    fn add(&mut self, bytes: usize) {
        if !self.overflowed {
            match self.bytes.checked_add(bytes) {
                Some(bytes) => self.bytes = bytes,
                None => self.overflowed = true,
            }
        }
    }

    fn projected_bytes(self) -> Option<usize> {
        (!self.overflowed).then_some(self.bytes)
    }
}

impl IoWrite for ProjectedByteCounter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.add(buffer.len());
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum ProjectedTextEncoding {
    Raw,
    XmlEscaped,
}

struct ProjectedTextCounter<'a> {
    bytes: &'a mut ProjectedByteCounter,
    encoding: ProjectedTextEncoding,
}

impl<'a> ProjectedTextCounter<'a> {
    fn raw(bytes: &'a mut ProjectedByteCounter) -> Self {
        Self {
            bytes,
            encoding: ProjectedTextEncoding::Raw,
        }
    }

    fn xml_escaped(bytes: &'a mut ProjectedByteCounter) -> Self {
        Self {
            bytes,
            encoding: ProjectedTextEncoding::XmlEscaped,
        }
    }
}

impl fmt::Write for ProjectedTextCounter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        match self.encoding {
            ProjectedTextEncoding::Raw => self.bytes.add(value.len()),
            ProjectedTextEncoding::XmlEscaped => {
                let mut plain_start = 0usize;
                for (index, byte) in value.bytes().enumerate() {
                    let escaped_bytes = match byte {
                        b'<' | b'>' => 4,
                        b'&' => 5,
                        b'\'' | b'"' => 6,
                        _ => continue,
                    };
                    self.bytes.add(index - plain_start);
                    self.bytes.add(escaped_bytes);
                    plain_start = index + 1;
                }
                self.bytes.add(value.len() - plain_start);
            }
        }
        Ok(())
    }
}

fn projected_rebased_xml_bytes(
    svg: &str,
    ids: &CollectedIds,
    prefix: &str,
) -> Result<Option<usize>> {
    let mut writer = Writer::new(ProjectedByteCounter::default());
    let mut reader = Reader::from_str(svg);
    reader.config_mut().check_end_names = true;
    let mut style_depth = 0usize;

    loop {
        let event = reader
            .read_event()
            .map_err(|error| rebase_error(format!("invalid SVG XML: {error}")))?;
        match event {
            Event::Start(start) => {
                let is_style = start.local_name().as_ref().eq_ignore_ascii_case(b"style");
                project_start(&start, &reader, ids, prefix, writer.get_mut(), false)?;
                if is_style {
                    style_depth += 1;
                }
            }
            Event::Empty(start) => {
                project_start(&start, &reader, ids, prefix, writer.get_mut(), true)?;
            }
            Event::End(end) => {
                if end.local_name().as_ref().eq_ignore_ascii_case(b"style") {
                    style_depth = style_depth.saturating_sub(1);
                }
                writer.write_event(Event::End(end)).map_err(write_error)?;
            }
            Event::Text(text) if style_depth > 0 => {
                let encoded = text
                    .decode()
                    .map_err(|error| rebase_error(format!("invalid style text: {error}")))?;
                let css = quick_xml::escape::unescape(&encoded)
                    .map_err(|error| rebase_error(format!("invalid style entity: {error}")))?;
                let mut output = ProjectedTextCounter::xml_escaped(writer.get_mut());
                rewrite_stylesheet_to(&css, ids, prefix, &mut output)?;
            }
            Event::CData(text) if style_depth > 0 => {
                let css = text
                    .decode()
                    .map_err(|error| rebase_error(format!("invalid style CDATA: {error}")))?;
                writer.get_mut().add(b"<![CDATA[".len());
                let mut output = ProjectedTextCounter::raw(writer.get_mut());
                rewrite_stylesheet_to(&css, ids, prefix, &mut output)?;
                writer.get_mut().add(b"]]>".len());
            }
            Event::Eof => break,
            other => writer.write_event(other).map_err(write_error)?,
        }
    }
    Ok(writer.into_inner().projected_bytes())
}

fn project_start<I: RebaseIdLookup + ?Sized>(
    start: &BytesStart<'_>,
    reader: &Reader<&[u8]>,
    ids: &I,
    prefix: &str,
    output: &mut ProjectedByteCounter,
    empty: bool,
) -> Result<()> {
    let start_name = start.name();
    let name = reader
        .decoder()
        .decode(start_name.as_ref())
        .map_err(|error| rebase_error(format!("invalid element name: {error}")))?;
    output.add(1);
    output.add(name.len());
    for attribute in start.attributes().with_checks(true) {
        let attribute =
            attribute.map_err(|error| rebase_error(format!("invalid SVG attribute: {error}")))?;
        let key = reader
            .decoder()
            .decode(attribute.key.as_ref())
            .map_err(|error| rebase_error(format!("invalid attribute name: {error}")))?;
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|error| rebase_error(format!("invalid attribute value: {error}")))?;
        output.add(1 + key.len() + 2);
        let mut escaped = ProjectedTextCounter::xml_escaped(output);
        rewrite_attribute_to(&key, &value, ids, prefix, &mut escaped)?;
        output.add(1);
    }
    output.add(if empty { 2 } else { 1 });
    Ok(())
}

fn rebase_xml(
    svg: &str,
    ids: &BTreeMap<String, String>,
    prefix: &str,
    projected_bytes: usize,
) -> Result<String> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(projected_bytes)
        .map_err(|error| rebase_error(format!("failed to allocate rebased SVG: {error}")))?;
    let mut writer = Writer::new(output);
    write_rebased_xml(svg, ids, prefix, &mut writer)?;
    let output = writer.into_inner();
    if output.len() != projected_bytes {
        return Err(rebase_error(
            "rebased SVG byte projection changed during materialization",
        ));
    }
    String::from_utf8(output)
        .map_err(|error| rebase_error(format!("rebased SVG is not UTF-8: {error}")))
}

fn write_rebased_xml<W: IoWrite, I: RebaseIdLookup + ?Sized>(
    svg: &str,
    ids: &I,
    prefix: &str,
    writer: &mut Writer<W>,
) -> Result<()> {
    let mut reader = Reader::from_str(svg);
    reader.config_mut().check_end_names = true;
    let mut style_depth = 0usize;

    loop {
        let event = reader
            .read_event()
            .map_err(|error| rebase_error(format!("invalid SVG XML: {error}")))?;
        match event {
            Event::Start(start) => {
                let is_style = start.local_name().as_ref().eq_ignore_ascii_case(b"style");
                let rewritten = rewrite_start(start, &reader, ids, prefix)?;
                writer
                    .write_event(Event::Start(rewritten))
                    .map_err(write_error)?;
                if is_style {
                    style_depth += 1;
                }
            }
            Event::Empty(start) => {
                let rewritten = rewrite_start(start, &reader, ids, prefix)?;
                writer
                    .write_event(Event::Empty(rewritten))
                    .map_err(write_error)?;
            }
            Event::End(end) => {
                if end.local_name().as_ref().eq_ignore_ascii_case(b"style") {
                    style_depth = style_depth.saturating_sub(1);
                }
                writer.write_event(Event::End(end)).map_err(write_error)?;
            }
            Event::Text(text) if style_depth > 0 => {
                let encoded = text
                    .decode()
                    .map_err(|error| rebase_error(format!("invalid style text: {error}")))?;
                let css = quick_xml::escape::unescape(&encoded)
                    .map_err(|error| rebase_error(format!("invalid style entity: {error}")))?;
                let css = rewrite_stylesheet(&css, ids, prefix)?;
                writer
                    .write_event(Event::Text(BytesText::new(&css)))
                    .map_err(write_error)?;
            }
            Event::CData(text) if style_depth > 0 => {
                let css = text
                    .decode()
                    .map_err(|error| rebase_error(format!("invalid style CDATA: {error}")))?;
                let css = rewrite_stylesheet(&css, ids, prefix)?;
                writer
                    .write_event(Event::CData(BytesCData::new(&css)))
                    .map_err(write_error)?;
            }
            Event::Eof => break,
            other => writer.write_event(other).map_err(write_error)?,
        }
    }
    Ok(())
}

fn rewrite_start<I: RebaseIdLookup + ?Sized>(
    start: BytesStart<'_>,
    reader: &Reader<&[u8]>,
    ids: &I,
    prefix: &str,
) -> Result<BytesStart<'static>> {
    let name = reader
        .decoder()
        .decode(start.name().as_ref())
        .map_err(|error| rebase_error(format!("invalid element name: {error}")))?
        .into_owned();
    let mut rewritten = BytesStart::new(name);
    for attribute in start.attributes().with_checks(true) {
        let attribute =
            attribute.map_err(|error| rebase_error(format!("invalid SVG attribute: {error}")))?;
        let key = reader
            .decoder()
            .decode(attribute.key.as_ref())
            .map_err(|error| rebase_error(format!("invalid attribute name: {error}")))?
            .into_owned();
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|error| rebase_error(format!("invalid attribute value: {error}")))?;
        let value = rewrite_attribute(&key, &value, ids, prefix)?;
        rewritten.push_attribute((key.as_str(), value.as_str()));
    }
    Ok(rewritten)
}

fn rewrite_attribute<I: RebaseIdLookup + ?Sized>(
    qualified_name: &str,
    value: &str,
    ids: &I,
    prefix: &str,
) -> Result<String> {
    let mut output = String::new();
    rewrite_attribute_to(qualified_name, value, ids, prefix, &mut output)?;
    Ok(output)
}

fn rewrite_attribute_to<I: RebaseIdLookup + ?Sized, W: fmt::Write>(
    qualified_name: &str,
    value: &str,
    ids: &I,
    prefix: &str,
    output: &mut W,
) -> Result<()> {
    let name = qualified_name.rsplit(':').next().unwrap_or(qualified_name);
    let normalized_name = name.to_ascii_lowercase();
    if is_css_value_attribute(name) {
        return rewrite_component_values_to(value, ids, prefix, false, output);
    }
    match normalized_name.as_str() {
        "id" => {
            if ids.contains_id(value) {
                write_rebased_reference(ids, prefix, value, output)?;
            } else {
                write_rebased(output, value)?;
            }
        }
        "href" if value.starts_with('#') => {
            write_rebased(output, "#")?;
            write_rebased_reference(ids, prefix, &value[1..], output)?;
        }
        name if is_svg_idref_attribute(name) => {
            for (index, id) in value.split_ascii_whitespace().enumerate() {
                if index != 0 {
                    write_rebased(output, " ")?;
                }
                write_rebased_reference(ids, prefix, id, output)?;
            }
        }
        "begin" | "end" => rewrite_smil_timing_to(value, ids, prefix, output)?,
        _ => write_rebased(output, value)?,
    }
    Ok(())
}

fn rebased_reference<I: RebaseIdLookup + ?Sized>(ids: &I, prefix: &str, id: &str) -> String {
    ids.rebased_id(id)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{prefix}-{id}"))
}

fn write_rebased_reference<I: RebaseIdLookup + ?Sized, W: fmt::Write>(
    ids: &I,
    prefix: &str,
    id: &str,
    output: &mut W,
) -> Result<()> {
    if let Some(rebased) = ids.rebased_id(id) {
        return write_rebased(output, rebased);
    }
    write_rebased(output, prefix)?;
    write_rebased(output, "-")?;
    write_rebased(output, id)
}

fn write_rebased(output: &mut (impl fmt::Write + ?Sized), value: &str) -> Result<()> {
    output
        .write_str(value)
        .map_err(|_| rebase_error("failed to write rebased SVG component"))
}

fn rewrite_smil_timing_to<I: RebaseIdLookup + ?Sized, W: fmt::Write>(
    value: &str,
    ids: &I,
    prefix: &str,
    output: &mut W,
) -> Result<()> {
    for (part_index, part) in value.split(';').enumerate() {
        if part_index != 0 {
            write_rebased(output, ";")?;
        }
        let trimmed = part.trim();
        let Some(index) = trimmed.find('.') else {
            write_rebased(output, trimmed)?;
            continue;
        };
        let eventbase = &trimmed[..index];
        if !ids.contains_id(eventbase)
            || !trimmed[index + 1..]
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_alphabetic())
        {
            write_rebased(output, trimmed)?;
            continue;
        }
        write_rebased_reference(ids, prefix, eventbase, output)?;
        write_rebased(output, &trimmed[index..])?;
    }
    Ok(())
}

fn rewrite_stylesheet<I: RebaseIdLookup + ?Sized>(
    css: &str,
    ids: &I,
    prefix: &str,
) -> Result<String> {
    let mut output = String::new();
    rewrite_stylesheet_to(css, ids, prefix, &mut output)?;
    Ok(output)
}

fn rewrite_stylesheet_to<I: RebaseIdLookup + ?Sized, W: fmt::Write>(
    css: &str,
    ids: &I,
    prefix: &str,
    output: &mut W,
) -> Result<()> {
    rewrite_component_values_to(css, ids, prefix, true, output)
}

fn rewrite_component_values_to<I: RebaseIdLookup + ?Sized, W: fmt::Write>(
    css: &str,
    ids: &I,
    prefix: &str,
    rewrite_hashes: bool,
    output: &mut W,
) -> Result<()> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    rewrite_parser(&mut parser, ids, prefix, rewrite_hashes, output)
}

fn rewrite_parser<'i, 't, I: RebaseIdLookup + ?Sized, W: fmt::Write>(
    input: &mut Parser<'i, 't>,
    ids: &I,
    prefix: &str,
    rewrite_hashes: bool,
    output: &mut W,
) -> Result<()> {
    let mut group_at_rule = None::<String>;
    loop {
        let start = input.position();
        let token = match input.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => {
                write_rebased(output, input.slice_from(start))?;
                break;
            }
            Err(error) => return Err(rebase_error(format!("invalid CSS: {error:?}"))),
        };
        let end = input.position();
        match token {
            Token::AtKeyword(name) => {
                let name = name.to_ascii_lowercase();
                group_at_rule = matches!(
                    name.as_str(),
                    "container" | "document" | "layer" | "media" | "scope" | "supports"
                )
                .then_some(name);
                write_rebased(output, input.slice(start..end))?;
            }
            Token::IDHash(id) | Token::Hash(id)
                if rewrite_hashes && group_at_rule.is_none() && ids.contains_id(id.as_ref()) =>
            {
                output
                    .write_char('#')
                    .map_err(|_| rebase_error("failed to write rebased CSS selector prefix"))?;
                serialize_identifier(&rebased_reference(ids, prefix, id.as_ref()), output)
                    .map_err(|_| rebase_error("failed to serialize rebased CSS selector"))?;
            }
            Token::UnquotedUrl(url) if url.starts_with('#') => {
                write_rebased(output, "url(#")?;
                write_rebased_reference(ids, prefix, &url[1..], output)?;
                output
                    .write_char(')')
                    .map_err(|_| rebase_error("failed to write rebased CSS URL terminator"))?;
            }
            Token::Function(name) => {
                write_rebased(output, input.slice(start..end))?;
                let is_url = name.eq_ignore_ascii_case("url");
                let nested_hashes = name.eq_ignore_ascii_case("selector")
                    || (rewrite_hashes && group_at_rule.is_none());
                input
                    .parse_nested_block(|nested| {
                        rewrite_url_or_nested_to(nested, ids, prefix, nested_hashes, is_url, output)
                            .map_err(|_| nested.new_custom_error::<(), ()>(()))
                    })
                    .map_err(|_| rebase_error("invalid CSS function"))?;
                output
                    .write_char(')')
                    .map_err(|_| rebase_error("failed to write rebased CSS function terminator"))?;
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                write_rebased(output, input.slice(start..end))?;
                let nested_hashes = if matches!(token, Token::CurlyBracketBlock) {
                    group_at_rule.is_some()
                } else if matches!(token, Token::ParenthesisBlock)
                    && group_at_rule.as_deref() == Some("scope")
                {
                    true
                } else {
                    rewrite_hashes && group_at_rule.is_none()
                };
                input
                    .parse_nested_block(|nested| {
                        let result = if matches!(token, Token::SquareBracketBlock) && nested_hashes
                        {
                            rewrite_attribute_selector_to(nested, ids, prefix, output)
                        } else {
                            rewrite_parser(nested, ids, prefix, nested_hashes, output)
                        };
                        result.map_err(|_| nested.new_custom_error::<(), ()>(()))
                    })
                    .map_err(|_| rebase_error("invalid nested CSS block"))?;
                output
                    .write_char(match token {
                        Token::ParenthesisBlock => ')',
                        Token::SquareBracketBlock => ']',
                        Token::CurlyBracketBlock => '}',
                        _ => unreachable!(),
                    })
                    .map_err(|_| rebase_error("failed to write rebased CSS block terminator"))?;
                if matches!(token, Token::CurlyBracketBlock) {
                    group_at_rule = None;
                }
            }
            Token::Semicolon => {
                group_at_rule = None;
                write_rebased(output, input.slice(start..end))?;
            }
            Token::BadUrl(_) | Token::BadString(_) => {
                return Err(rebase_error("invalid CSS token"));
            }
            _ => write_rebased(output, input.slice(start..end))?,
        }
    }
    Ok(())
}

fn rewrite_attribute_selector_to<'i, 't, I: RebaseIdLookup + ?Sized, W: fmt::Write>(
    input: &mut Parser<'i, 't>,
    ids: &I,
    prefix: &str,
    output: &mut W,
) -> Result<()> {
    let mut attribute_name = None::<String>;
    let mut operator = None::<AttributeMatchOperator>;
    loop {
        let start = input.position();
        let token = match input.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => {
                write_rebased(output, input.slice_from(start))?;
                break;
            }
            Err(error) => {
                return Err(rebase_error(format!(
                    "invalid CSS attribute selector: {error:?}"
                )));
            }
        };
        let end = input.position();
        match token {
            Token::Ident(name) if operator.is_none() => {
                attribute_name = Some(name.to_ascii_lowercase());
                write_rebased(output, input.slice(start..end))?;
            }
            Token::Delim('=') => {
                operator = Some(AttributeMatchOperator::Exact);
                write_rebased(output, input.slice(start..end))?;
            }
            Token::IncludeMatch => {
                operator = Some(AttributeMatchOperator::Includes);
                write_rebased(output, input.slice(start..end))?;
            }
            Token::DashMatch => {
                operator = Some(AttributeMatchOperator::Dash);
                write_rebased(output, input.slice(start..end))?;
            }
            Token::PrefixMatch => {
                operator = Some(AttributeMatchOperator::Prefix);
                write_rebased(output, input.slice(start..end))?;
            }
            Token::SuffixMatch => {
                operator = Some(AttributeMatchOperator::Suffix);
                write_rebased(output, input.slice(start..end))?;
            }
            Token::SubstringMatch => {
                operator = Some(AttributeMatchOperator::Substring);
                write_rebased(output, input.slice(start..end))?;
            }
            Token::QuotedString(value) if operator.is_some() => {
                output.write_char('"').map_err(|_| {
                    rebase_error("failed to write rebased CSS attribute selector quote")
                })?;
                {
                    let mut escaped = CssStringWriter::new(output);
                    rewrite_selector_attribute_value_to(
                        attribute_name.as_deref(),
                        operator.expect("guarded above"),
                        &value,
                        ids,
                        prefix,
                        &mut escaped,
                    )?;
                }
                output.write_char('"').map_err(|_| {
                    rebase_error("failed to write rebased CSS attribute selector quote")
                })?;
            }
            Token::Ident(value) if operator.is_some() => {
                let value = rewrite_selector_attribute_value(
                    attribute_name.as_deref(),
                    operator.expect("guarded above"),
                    &value,
                    ids,
                    prefix,
                )?;
                serialize_identifier(&value, output).map_err(|_| {
                    rebase_error("failed to serialize rebased CSS attribute selector")
                })?;
            }
            Token::IDHash(value) if operator.is_some() => {
                let original = format!("#{value}");
                let value = rewrite_selector_attribute_value(
                    attribute_name.as_deref(),
                    operator.expect("guarded above"),
                    &original,
                    ids,
                    prefix,
                )?;
                output.write_char('#').map_err(|_| {
                    rebase_error("failed to write rebased CSS attribute selector prefix")
                })?;
                serialize_identifier(value.trim_start_matches('#'), output).map_err(|_| {
                    rebase_error("failed to serialize rebased CSS attribute selector")
                })?;
            }
            Token::BadUrl(_) | Token::BadString(_) => {
                return Err(rebase_error("invalid CSS token"));
            }
            _ => write_rebased(output, input.slice(start..end))?,
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum AttributeMatchOperator {
    Exact,
    Includes,
    Dash,
    Prefix,
    Suffix,
    Substring,
}

impl AttributeMatchOperator {
    fn preserves_id_value(self) -> bool {
        matches!(self, Self::Suffix | Self::Substring)
    }
}

fn rewrite_selector_attribute_value<I: RebaseIdLookup + ?Sized>(
    attribute_name: Option<&str>,
    operator: AttributeMatchOperator,
    value: &str,
    ids: &I,
    prefix: &str,
) -> Result<String> {
    let mut output = String::new();
    rewrite_selector_attribute_value_to(attribute_name, operator, value, ids, prefix, &mut output)?;
    Ok(output)
}

fn rewrite_selector_attribute_value_to<I: RebaseIdLookup + ?Sized, W: fmt::Write>(
    attribute_name: Option<&str>,
    operator: AttributeMatchOperator,
    value: &str,
    ids: &I,
    prefix: &str,
    output: &mut W,
) -> Result<()> {
    match attribute_name {
        Some("id") if operator.preserves_id_value() => write_rebased(output, value)?,
        Some("id") => write_rebased_reference(ids, prefix, value, output)?,
        Some("href" | "xlink:href") if value.starts_with('#') => {
            write_rebased(output, "#")?;
            write_rebased_reference(ids, prefix, &value[1..], output)?;
        }
        Some(name) if is_svg_idref_attribute(name) => {
            for (index, id) in value.split_ascii_whitespace().enumerate() {
                if index != 0 {
                    write_rebased(output, " ")?;
                }
                write_rebased_reference(ids, prefix, id, output)?;
            }
        }
        _ => write_rebased(output, value)?,
    }
    Ok(())
}

fn rewrite_url_or_nested_to<'i, 't, I: RebaseIdLookup + ?Sized, W: fmt::Write>(
    input: &mut Parser<'i, 't>,
    ids: &I,
    prefix: &str,
    rewrite_hashes: bool,
    is_url: bool,
    output: &mut W,
) -> Result<()> {
    if !is_url {
        return rewrite_parser(input, ids, prefix, rewrite_hashes, output);
    }
    let start = input.position();
    let token = match input.next_including_whitespace_and_comments() {
        Ok(token) => token.clone(),
        Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => {
            return Ok(());
        }
        Err(error) => return Err(rebase_error(format!("invalid CSS URL: {error:?}"))),
    };
    match token {
        Token::QuotedString(value) if value.starts_with('#') => {
            output
                .write_char('"')
                .map_err(|_| rebase_error("failed to write rebased CSS URL quote"))?;
            {
                let mut escaped = CssStringWriter::new(output);
                write_rebased(&mut escaped, "#")?;
                write_rebased_reference(ids, prefix, &value[1..], &mut escaped)?;
            }
            output
                .write_char('"')
                .map_err(|_| rebase_error("failed to write rebased CSS URL quote"))?;
        }
        Token::IDHash(value) => {
            output
                .write_char('#')
                .map_err(|_| rebase_error("failed to write rebased CSS URL prefix"))?;
            serialize_identifier(&rebased_reference(ids, prefix, value.as_ref()), output)
                .map_err(|_| rebase_error("failed to serialize rebased CSS URL"))?;
        }
        _ => write_rebased(output, input.slice_from(start))?,
    }
    while input.next_including_whitespace_and_comments().is_ok() {}
    Ok(())
}

fn rebase_error(message: impl Into<String>) -> Error {
    Error::svg_postprocess("rebase-svg-ids", message)
}

fn write_error(error: std::io::Error) -> Error {
    rebase_error(format!("failed to write rebased SVG: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::RenderEnvironment;
    use crate::resources::{
        RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
    };
    use crate::svg::pipeline::{SvgPipeline, SvgPipelinePreset, SvgPostprocessMetadata};

    fn rebase(svg: &str) -> String {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        SvgPipeline::parity()
            .with_postprocessor(RebaseSvgIdsPostprocessor::new("fragment-light"))
            .process_to_string(svg, &session)
            .unwrap()
    }

    #[test]
    fn rebases_ids_and_every_supported_local_reference_shape() {
        let svg = r##"<svg id="root" aria-labelledby="title desc" xmlns:xlink="http://www.w3.org/1999/xlink"><style>#root #shape{fill:url(#paint)}#fff{stroke:#fff}.optional{fill:url(#missing-gradient)}</style><title id="title">T</title><desc id="desc">D</desc><defs><linearGradient id="paint"/><path id="shape"/><path id="fff"/></defs><use href="#shape" xlink:href="#shape"/><animate id="pulse" begin="shape.begin+1s; pulse.end"/><path style="fill:url('#paint')" marker-end="url(#shape)"/></svg>"##;
        let output = rebase(svg);

        for expected in [
            r#"id="fragment-light-root""#,
            r#"aria-labelledby="fragment-light-title fragment-light-desc""#,
            "#fragment-light-root #fragment-light-shape",
            "#fragment-light-fff{stroke:#fff}",
            "url(#fragment-light-paint)",
            "url(#fragment-light-missing-gradient)",
            r##"href="#fragment-light-shape""##,
            "fragment-light-shape.begin+1s;fragment-light-pulse.end",
        ] {
            assert!(output.contains(expected), "missing {expected:?}: {output}");
        }
        roxmltree::Document::parse(&output).expect("rebased SVG XML");
    }

    #[test]
    fn duplicate_ids_fail_closed() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let error = SvgPipeline::parity()
            .with_postprocessor(RebaseSvgIdsPostprocessor::new("fragment"))
            .process_to_string(r#"<svg><g id="same"/><path id="same"/></svg>"#, &session)
            .unwrap_err();

        assert!(error.to_string().contains("duplicate SVG id"), "{error}");
    }

    #[test]
    fn projected_n_minus_one_returns_the_exact_svg_byte_limit_error() {
        let svg = r##"<svg id="root" aria-labelledby="paint shape"><style>#root[aria-labelledby="paint shape"] #shape{fill:url(#paint);content:"&lt;&amp;"}</style><style><![CDATA[#shape{stroke:url(#paint)}]]></style><defs><linearGradient id="paint"/></defs><path id="shape" fill="url(#paint)"/><use href="#shape"/></svg>"##;
        let processor = RebaseSvgIdsPostprocessor::new("fragment-with-a-long-scope");
        let unbounded_session = RenderEnvironment::deterministic()
            .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .begin_session()
            .unwrap();
        let metadata = SvgPostprocessMetadata::from_svg(svg);
        let unbounded_context = SvgPostprocessContext::new(
            SvgPipelinePreset::Parity,
            0,
            "rebase-svg-ids",
            &metadata,
            &unbounded_session,
        );
        let projected_bytes = processor
            .process(Cow::Borrowed(svg), &unbounded_context)
            .expect("unbounded ID rebase should materialize")
            .len();
        assert!(projected_bytes > svg.len());

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, projected_bytes)
            .unwrap();
        let exact_session = RenderEnvironment::deterministic()
            .with_resource_policy(exact_policy)
            .begin_session()
            .unwrap();
        let exact_context = SvgPostprocessContext::new(
            SvgPipelinePreset::Parity,
            0,
            "rebase-svg-ids",
            &metadata,
            &exact_session,
        );
        let exact = processor
            .process(Cow::Borrowed(svg), &exact_context)
            .expect("the exact SVG byte limit should admit ID rebasing");
        assert_eq!(exact.len(), projected_bytes);

        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, projected_bytes - 1)
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .unwrap();
        let context = SvgPostprocessContext::new(
            SvgPipelinePreset::Parity,
            0,
            "rebase-svg-ids",
            &metadata,
            &session,
        );
        let error = processor.process(Cow::Borrowed(svg), &context).unwrap_err();

        let Error::ResourceLimitExceeded(details) = error else {
            panic!("expected SVG byte resource rejection, got {error}");
        };
        assert_eq!(details.cause, ResourceLimitCause::Ceiling);
        assert_eq!(details.phase, ResourceLimitPhase::SvgPostprocess);
        assert_eq!(details.limit, "max_svg_bytes");
        assert_eq!(details.actual, projected_bytes);
        assert_eq!(details.max, projected_bytes - 1);
    }

    #[test]
    fn rebases_group_rule_selectors_without_rewriting_prelude_or_value_colors() {
        let svg = r##"<svg><style>@media (prefers-color-scheme:dark){#root{fill:#fff}}@supports (color:#fff){#root{stroke:#fff}}</style><g id="root"/><g id="fff"/></svg>"##;
        let output = rebase(svg);

        assert!(
            output.contains("@media (prefers-color-scheme:dark){#fragment-light-root{fill:#fff}}"),
            "{output}"
        );
        assert!(
            output.contains("@supports (color:#fff){#fragment-light-root{stroke:#fff}}"),
            "{output}"
        );
    }

    #[test]
    fn rebases_escaped_urls_selector_preludes_attribute_selectors_and_smil_events() {
        let svg = r##"<svg><style>@scope (#root){[href="#shape"],[id="shape"]{fill:u\72l("#paint")}}@supports selector(#root){#root{stroke:#fff}}</style><defs><linearGradient id="paint"/><path id="shape"/><g id="root"/><g id="fff"/></defs><path fill="u\72l(#paint)"/><animate begin="shape.click;shape.repeat(2);shape.begin"/></svg>"##;
        let output = rebase(svg);

        for expected in [
            "@scope (#fragment-light-root)",
            "@supports selector(#fragment-light-root)",
            "[href=&quot;#fragment-light-shape&quot;]",
            "[id=&quot;fragment-light-shape&quot;]",
            "#fragment-light-root{stroke:#fff}",
            "#fragment-light-paint",
            "fragment-light-shape.click;fragment-light-shape.repeat(2);fragment-light-shape.begin",
        ] {
            assert!(output.contains(expected), "missing {expected:?}: {output}");
        }
        assert!(!output.contains("#fragment-light-fff}"), "{output}");
        roxmltree::Document::parse(&output).expect("rebased SVG XML");
    }

    #[test]
    fn leaves_non_css_prose_attributes_opaque() {
        let svg = r#"<svg id="root" aria-label="prose https://example.test/url(foo&amp;bar)"><g id="shape"/></svg>"#;
        let output = rebase(svg);

        assert!(
            output.contains(r#"aria-label="prose https://example.test/url(foo&amp;bar)""#),
            "{output}"
        );
    }

    #[test]
    fn rebases_all_aria_idrefs_in_attributes_and_selectors() {
        let svg = r##"<svg id="root" aria-activedescendant="shape" aria-controls="shape" aria-details="shape" aria-errormessage="shape" aria-flowto="shape" aria-owns="shape"><style>#root [aria-controls="shape"],#root [aria-owns="shape"],#root [id$="-arrowhead"],#root [id*="arrow"],#root [id^="shape"]{fill:red}</style><path id="shape"/><path id="shape-arrowhead"/></svg>"##;
        let output = rebase(svg);

        for attribute in [
            "aria-activedescendant",
            "aria-controls",
            "aria-details",
            "aria-errormessage",
            "aria-flowto",
            "aria-owns",
        ] {
            assert!(
                output.contains(&format!(r#"{attribute}="fragment-light-shape""#)),
                "{attribute}: {output}"
            );
        }
        assert!(
            output.contains("[aria-controls=&quot;fragment-light-shape&quot;]"),
            "{output}"
        );
        assert!(
            output.contains("[aria-owns=&quot;fragment-light-shape&quot;]"),
            "{output}"
        );
        assert!(output.contains("[id$=&quot;-arrowhead&quot;]"), "{output}");
        assert!(output.contains("[id*=&quot;arrow&quot;]"), "{output}");
        assert!(
            output.contains("[id^=&quot;fragment-light-shape&quot;]"),
            "{output}"
        );
    }
}
