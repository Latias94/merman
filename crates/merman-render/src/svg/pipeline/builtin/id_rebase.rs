use crate::svg::pipeline::{
    SvgPostprocessContext, SvgPostprocessor, is_css_value_attribute, is_svg_idref_attribute,
};
use crate::{Error, Result};
use cssparser::{
    BasicParseErrorKind, Parser, ParserInput, Token, serialize_identifier, serialize_string,
};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesCData, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use std::borrow::Cow;
use std::collections::BTreeMap;

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
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        let ids = collect_ids(&svg, &self.prefix)?;
        if ids.is_empty() {
            return Ok(svg);
        }
        rebase_xml(&svg, &ids, &self.prefix).map(Cow::Owned)
    }
}

fn collect_ids(svg: &str, prefix: &str) -> Result<BTreeMap<String, String>> {
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| rebase_error(format!("invalid SVG XML: {error}")))?;
    let mut ids = BTreeMap::new();
    for node in document.descendants().filter(roxmltree::Node::is_element) {
        let Some(id) = node.attribute("id") else {
            continue;
        };
        if id.is_empty() {
            return Err(rebase_error("SVG id must not be empty"));
        }
        let rebased = format!("{prefix}-{id}");
        if ids.insert(id.to_string(), rebased).is_some() {
            return Err(rebase_error(format!("duplicate SVG id {id:?}")));
        }
    }
    Ok(ids)
}

fn rebase_xml(svg: &str, ids: &BTreeMap<String, String>, prefix: &str) -> Result<String> {
    let mut reader = Reader::from_str(svg);
    reader.config_mut().check_end_names = true;
    let mut writer = Writer::new(Vec::with_capacity(svg.len() + ids.len() * 24));
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

    String::from_utf8(writer.into_inner())
        .map_err(|error| rebase_error(format!("rebased SVG is not UTF-8: {error}")))
}

fn rewrite_start(
    start: BytesStart<'_>,
    reader: &Reader<&[u8]>,
    ids: &BTreeMap<String, String>,
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

fn rewrite_attribute(
    qualified_name: &str,
    value: &str,
    ids: &BTreeMap<String, String>,
    prefix: &str,
) -> Result<String> {
    let name = qualified_name.rsplit(':').next().unwrap_or(qualified_name);
    let normalized_name = name.to_ascii_lowercase();
    let mut value = match normalized_name.as_str() {
        "id" => ids.get(value).cloned().unwrap_or_else(|| value.to_string()),
        "href" if value.starts_with('#') => {
            format!("#{}", rebased_reference(ids, prefix, &value[1..]))
        }
        name if is_svg_idref_attribute(name) => value
            .split_ascii_whitespace()
            .map(|id| rebased_reference(ids, prefix, id))
            .collect::<Vec<_>>()
            .join(" "),
        "begin" | "end" => rewrite_smil_timing(value, ids, prefix),
        _ => value.to_string(),
    };
    if is_css_value_attribute(name) {
        value = rewrite_component_values(&value, ids, prefix, false)?;
    }
    Ok(value)
}

fn rebased_reference(ids: &BTreeMap<String, String>, prefix: &str, id: &str) -> String {
    ids.get(id)
        .cloned()
        .unwrap_or_else(|| format!("{prefix}-{id}"))
}

fn rewrite_smil_timing(value: &str, ids: &BTreeMap<String, String>, prefix: &str) -> String {
    value
        .split(';')
        .map(|part| {
            let trimmed = part.trim();
            let Some(index) = trimmed.find('.') else {
                return trimmed.to_string();
            };
            let eventbase = &trimmed[..index];
            if !ids.contains_key(eventbase)
                || !trimmed[index + 1..]
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_alphabetic())
            {
                return trimmed.to_string();
            }
            format!(
                "{}{}",
                rebased_reference(ids, prefix, eventbase),
                &trimmed[index..]
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn rewrite_stylesheet(css: &str, ids: &BTreeMap<String, String>, prefix: &str) -> Result<String> {
    rewrite_component_values(css, ids, prefix, true)
}

fn rewrite_component_values(
    css: &str,
    ids: &BTreeMap<String, String>,
    prefix: &str,
    rewrite_hashes: bool,
) -> Result<String> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    rewrite_parser(&mut parser, ids, prefix, rewrite_hashes)
}

fn rewrite_parser<'i, 't>(
    input: &mut Parser<'i, 't>,
    ids: &BTreeMap<String, String>,
    prefix: &str,
    rewrite_hashes: bool,
) -> Result<String> {
    let mut output = String::new();
    let mut group_at_rule = None::<String>;
    loop {
        let start = input.position();
        let token = match input.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => {
                output.push_str(input.slice_from(start));
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
                output.push_str(input.slice(start..end));
            }
            Token::IDHash(id) | Token::Hash(id)
                if rewrite_hashes && group_at_rule.is_none() && ids.contains_key(id.as_ref()) =>
            {
                output.push('#');
                serialize_identifier(ids[id.as_ref()].as_str(), &mut output)
                    .map_err(|_| rebase_error("failed to serialize rebased CSS selector"))?;
            }
            Token::UnquotedUrl(url) if url.starts_with('#') => {
                output.push_str("url(#");
                output.push_str(&rebased_reference(ids, prefix, &url[1..]));
                output.push(')');
            }
            Token::Function(name) => {
                output.push_str(input.slice(start..end));
                let is_url = name.eq_ignore_ascii_case("url");
                let nested_hashes = name.eq_ignore_ascii_case("selector")
                    || (rewrite_hashes && group_at_rule.is_none());
                let nested = input
                    .parse_nested_block(|nested| {
                        rewrite_url_or_nested(nested, ids, prefix, nested_hashes, is_url)
                            .map_err(|_| nested.new_custom_error::<(), ()>(()))
                    })
                    .map_err(|_| rebase_error("invalid CSS function"))?;
                output.push_str(&nested);
                output.push(')');
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                output.push_str(input.slice(start..end));
                let nested_hashes = if matches!(token, Token::CurlyBracketBlock) {
                    group_at_rule.is_some()
                } else if matches!(token, Token::ParenthesisBlock)
                    && group_at_rule.as_deref() == Some("scope")
                {
                    true
                } else {
                    rewrite_hashes && group_at_rule.is_none()
                };
                let nested = input
                    .parse_nested_block(|nested| {
                        let result = if matches!(token, Token::SquareBracketBlock) && nested_hashes
                        {
                            rewrite_attribute_selector(nested, ids, prefix)
                        } else {
                            rewrite_parser(nested, ids, prefix, nested_hashes)
                        };
                        result.map_err(|_| nested.new_custom_error::<(), ()>(()))
                    })
                    .map_err(|_| rebase_error("invalid nested CSS block"))?;
                output.push_str(&nested);
                output.push(match token {
                    Token::ParenthesisBlock => ')',
                    Token::SquareBracketBlock => ']',
                    Token::CurlyBracketBlock => '}',
                    _ => unreachable!(),
                });
                if matches!(token, Token::CurlyBracketBlock) {
                    group_at_rule = None;
                }
            }
            Token::Semicolon => {
                group_at_rule = None;
                output.push_str(input.slice(start..end));
            }
            Token::BadUrl(_) | Token::BadString(_) => {
                return Err(rebase_error("invalid CSS token"));
            }
            _ => output.push_str(input.slice(start..end)),
        }
    }
    Ok(output)
}

fn rewrite_attribute_selector<'i, 't>(
    input: &mut Parser<'i, 't>,
    ids: &BTreeMap<String, String>,
    prefix: &str,
) -> Result<String> {
    let mut output = String::new();
    let mut attribute_name = None::<String>;
    let mut operator = None::<AttributeMatchOperator>;
    loop {
        let start = input.position();
        let token = match input.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => {
                output.push_str(input.slice_from(start));
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
                output.push_str(input.slice(start..end));
            }
            Token::Delim('=') => {
                operator = Some(AttributeMatchOperator::Exact);
                output.push_str(input.slice(start..end));
            }
            Token::IncludeMatch => {
                operator = Some(AttributeMatchOperator::Includes);
                output.push_str(input.slice(start..end));
            }
            Token::DashMatch => {
                operator = Some(AttributeMatchOperator::Dash);
                output.push_str(input.slice(start..end));
            }
            Token::PrefixMatch => {
                operator = Some(AttributeMatchOperator::Prefix);
                output.push_str(input.slice(start..end));
            }
            Token::SuffixMatch => {
                operator = Some(AttributeMatchOperator::Suffix);
                output.push_str(input.slice(start..end));
            }
            Token::SubstringMatch => {
                operator = Some(AttributeMatchOperator::Substring);
                output.push_str(input.slice(start..end));
            }
            Token::QuotedString(value) if operator.is_some() => {
                let value = rewrite_selector_attribute_value(
                    attribute_name.as_deref(),
                    operator.expect("guarded above"),
                    &value,
                    ids,
                    prefix,
                );
                serialize_string(&value, &mut output).map_err(|_| {
                    rebase_error("failed to serialize rebased CSS attribute selector")
                })?;
            }
            Token::Ident(value) if operator.is_some() => {
                let value = rewrite_selector_attribute_value(
                    attribute_name.as_deref(),
                    operator.expect("guarded above"),
                    &value,
                    ids,
                    prefix,
                );
                serialize_identifier(&value, &mut output).map_err(|_| {
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
                );
                output.push('#');
                serialize_identifier(value.trim_start_matches('#'), &mut output).map_err(|_| {
                    rebase_error("failed to serialize rebased CSS attribute selector")
                })?;
            }
            Token::BadUrl(_) | Token::BadString(_) => {
                return Err(rebase_error("invalid CSS token"));
            }
            _ => output.push_str(input.slice(start..end)),
        }
    }
    Ok(output)
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

fn rewrite_selector_attribute_value(
    attribute_name: Option<&str>,
    operator: AttributeMatchOperator,
    value: &str,
    ids: &BTreeMap<String, String>,
    prefix: &str,
) -> String {
    match attribute_name {
        Some("id") if operator.preserves_id_value() => value.to_string(),
        Some("id") => rebased_reference(ids, prefix, value),
        Some("href" | "xlink:href") if value.starts_with('#') => {
            format!("#{}", rebased_reference(ids, prefix, &value[1..]))
        }
        Some(name) if is_svg_idref_attribute(name) => value
            .split_ascii_whitespace()
            .map(|id| rebased_reference(ids, prefix, id))
            .collect::<Vec<_>>()
            .join(" "),
        _ => value.to_string(),
    }
}

fn rewrite_url_or_nested<'i, 't>(
    input: &mut Parser<'i, 't>,
    ids: &BTreeMap<String, String>,
    prefix: &str,
    rewrite_hashes: bool,
    is_url: bool,
) -> Result<String> {
    if !is_url {
        return rewrite_parser(input, ids, prefix, rewrite_hashes);
    }
    let start = input.position();
    let token = match input.next_including_whitespace_and_comments() {
        Ok(token) => token.clone(),
        Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => {
            return Ok(String::new());
        }
        Err(error) => return Err(rebase_error(format!("invalid CSS URL: {error:?}"))),
    };
    let mut output = String::new();
    match token {
        Token::QuotedString(value) if value.starts_with('#') => {
            serialize_string(
                &format!("#{}", rebased_reference(ids, prefix, &value[1..])),
                &mut output,
            )
            .map_err(|_| rebase_error("failed to serialize rebased CSS URL"))?;
        }
        Token::IDHash(value) => {
            output.push('#');
            serialize_identifier(&rebased_reference(ids, prefix, value.as_ref()), &mut output)
                .map_err(|_| rebase_error("failed to serialize rebased CSS URL"))?;
        }
        _ => output.push_str(input.slice_from(start)),
    }
    while input.next_including_whitespace_and_comments().is_ok() {}
    Ok(output)
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
    use crate::svg::pipeline::SvgPipeline;

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
