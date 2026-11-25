//! Native SVG compatibility for Mermaid marker selectors.
//!
//! Mermaid 11.14 introduced ID suffix selectors. usvg 0.47.0 uses
//! simplecss 0.2.2, which rejects these and loses class fallbacks in the
//! same selector list. The upstream change is Mermaid commit
//! 107edad02d0efbb4aa504a9afe3911399f173bb0 (class/styles.js).
//! Transparent paint is supported; the lost rule is
//! the cause of filled aggregation diamonds. See Merman issue #181:
//! https://github.com/Latias94/merman/issues/181
//! Related parser limitation, with a different trigger: Typst issue #6595.
//! https://github.com/typst/typst/issues/6595
//!
//! Exact attribute matches retain specificity. Only resvg-safe output
//! applies this conversion, after IDs and user styles have been finalized.

use crate::resources::{OperationWorkMeter, ResourceLimitId, ResourceLimitPhase};
use crate::svg_scan::find_with_checkpoints;
use crate::{Error, Result};
use cssparser::{Delimiter, ParseErrorKind, Parser, Token, serialize_string};
use merman_model::OperationPhase;
use quick_xml::{Reader, events::Event};
use std::borrow::Cow;
use std::fmt;

fn error(message: impl std::fmt::Debug) -> Error {
    Error::svg_postprocess("lower-id-suffix-selectors", format!("{message:?}"))
}

fn css_error(parse_error: cssparser::ParseError<Error>) -> Error {
    match parse_error.kind {
        ParseErrorKind::Custom(error) => error,
        _ => error(parse_error),
    }
}

#[derive(Clone, Copy)]
struct PostprocessWork<'a>(&'a OperationWorkMeter);

impl PostprocessWork<'_> {
    fn checkpoint(self) -> Result<()> {
        self.0.checkpoint(OperationPhase::Postprocess)?;
        Ok(())
    }

    fn preflight_svg_byte_count(self, bytes: usize) -> Result<()> {
        self.0.preflight_svg_byte_count(
            bytes,
            ResourceLimitPhase::SvgPostprocess,
            OperationPhase::Postprocess,
        )?;
        Ok(())
    }

    fn svg_byte_count_overflow(self) -> Error {
        self.0
            .terminate_svg_byte_count_overflow(
                ResourceLimitPhase::SvgPostprocess,
                OperationPhase::Postprocess,
            )
            .into()
    }
}

#[derive(Default)]
struct ByteCount(usize);

impl fmt::Write for ByteCount {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0 = self.0.checked_add(value.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}

fn append(output: &mut String, value: &str, execution: PostprocessWork<'_>) -> Result<()> {
    reserve_append(output, value.len(), execution)?;
    output.push_str(value);
    Ok(())
}

fn reserve_append(
    output: &mut String,
    additional: usize,
    execution: PostprocessWork<'_>,
) -> Result<()> {
    let bytes = output
        .len()
        .checked_add(additional)
        .ok_or_else(|| execution.svg_byte_count_overflow())?;
    execution.preflight_svg_byte_count(bytes)?;
    if bytes > output.capacity() {
        let limit = execution
            .0
            .policy()
            .value(ResourceLimitId::MaxSvgBytes)
            .unwrap_or(usize::MAX);
        let capacity = bytes.max(output.capacity().saturating_mul(2).min(limit));
        output
            .try_reserve_exact(capacity - output.len())
            .map_err(error)?;
    }
    Ok(())
}

fn write_xml_style_text(css: &str, output: &mut impl fmt::Write) -> fmt::Result {
    // Match quick_xml's partial text escape, including CR preservation.
    let mut cursor = 0;
    for (index, matched) in css.match_indices(['<', '>', '&', '\r']) {
        output.write_str(&css[cursor..index])?;
        output.write_str(match matched {
            "<" => "&lt;",
            ">" => "&gt;",
            "&" => "&amp;",
            "\r" => "&#13;",
            _ => unreachable!("match_indices returned an unmatched character"),
        })?;
        cursor = index + matched.len();
    }
    output.write_str(&css[cursor..])
}

/// Lowers supported ID suffix selectors against IDs in this document only.
///
/// Run after XML admission. The caller owns terminal validation and resource policy.
/// Rewriting is limited to supported branches in top-level CSS rule preludes.
/// Byte admission and cancellation use the caller's existing postprocess operation meter.
pub fn lower_id_suffix_selectors<'a>(
    svg: Cow<'a, str>,
    work_meter: &OperationWorkMeter,
) -> Result<Cow<'a, str>> {
    let execution = PostprocessWork(work_meter);
    execution.preflight_svg_byte_count(svg.len())?;
    if find_with_checkpoints(&svg, "$=", &mut || execution.checkpoint())?.is_none() {
        return Ok(svg);
    }
    let mut ids = Vec::new();
    let mut styles = Vec::new();
    let mut style = None::<(usize, String)>;
    let mut reader = Reader::from_str(&svg);
    loop {
        execution.checkpoint()?;
        let event_start = reader.buffer_position() as usize;
        match reader.read_event().map_err(error)? {
            Event::Start(tag) => {
                collect_ids(&tag, &mut ids, execution)?;
                if tag.local_name().as_ref() == "style" {
                    style = Some((reader.buffer_position() as usize, String::new()));
                }
            }
            Event::Empty(tag) => collect_ids(&tag, &mut ids, execution)?,
            Event::End(tag) if tag.local_name().as_ref() == "style" => {
                if let Some((start, css)) = style.take() {
                    styles.push((start, event_start, css));
                }
            }
            Event::Text(text) if style.is_some() => {
                style
                    .as_mut()
                    .expect("guarded above")
                    .1
                    .push_str(&text.xml10_content());
            }
            Event::CData(text) if style.is_some() => {
                style
                    .as_mut()
                    .expect("guarded above")
                    .1
                    .push_str(&text.xml10_content());
            }
            Event::GeneralRef(reference) if style.is_some() => {
                let reference = format!("&{};", reference.as_ref());
                style
                    .as_mut()
                    .expect("guarded above")
                    .1
                    .push_str(&quick_xml::escape::unescape(&reference).map_err(error)?);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    ids.sort_unstable();
    ids.dedup();
    let mut output = String::new();
    let mut cursor = 0;
    for (start, end, css) in styles {
        execution.checkpoint()?;
        let lowered = lower_stylesheet(&css, &ids, execution)?;
        if lowered == css {
            continue;
        }
        append(&mut output, &svg[cursor..start], execution)?;
        let mut escaped = ByteCount::default();
        write_xml_style_text(&lowered, &mut escaped)
            .map_err(|_| execution.svg_byte_count_overflow())?;
        reserve_append(&mut output, escaped.0, execution)?;
        write_xml_style_text(&lowered, &mut output).map_err(error)?;
        cursor = end;
    }
    if cursor == 0 {
        return Ok(svg);
    }
    append(&mut output, &svg[cursor..], execution)?;
    Ok(Cow::Owned(output))
}

fn collect_ids(
    tag: &quick_xml::events::BytesStart<'_>,
    ids: &mut Vec<String>,
    execution: PostprocessWork<'_>,
) -> Result<()> {
    for attribute in tag.attributes() {
        execution.checkpoint()?;
        let attribute = attribute.map_err(error)?;
        if attribute.key.as_ref() == "id" {
            ids.push(
                attribute
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .map_err(error)?
                    .into_owned(),
            );
        }
    }
    Ok(())
}

fn consume_values(
    input: &mut Parser<'_>,
    execution: PostprocessWork<'_>,
    depth: u8,
) -> std::result::Result<(), cssparser::ParseError<Error>> {
    loop {
        execution
            .checkpoint()
            .map_err(cssparser::ParseError::custom)?;
        let token = match input.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(parse_error)
                if matches!(parse_error.kind, cssparser::BasicParseErrorKind::EndOfInput) =>
            {
                return Ok(());
            }
            Err(parse_error) => return Err(parse_error.into()),
        };
        if matches!(
            token,
            Token::Function(_)
                | Token::SquareBracketBlock
                | Token::ParenthesisBlock
                | Token::CurlyBracketBlock
        ) {
            if depth == 64 {
                return Err(cssparser::ParseError::custom(error("CSS nesting limit")));
            }
            input.parse_nested_block(|nested| consume_values(nested, execution, depth + 1))?;
        }
    }
}

fn lower_stylesheet(css: &str, ids: &[String], execution: PostprocessWork<'_>) -> Result<String> {
    let mut input = Parser::new(css);
    let mut rule_start = input.position();
    let mut output = String::new();
    while !input.is_exhausted() {
        execution.checkpoint()?;
        let start = input.position();
        let token = input
            .next_including_whitespace_and_comments()
            .map_err(error)?
            .clone();
        if token == Token::Semicolon {
            append(
                &mut output,
                input.slice(rule_start..input.position()),
                execution,
            )?;
            rule_start = input.position();
            continue;
        }
        if matches!(
            token,
            Token::Function(_) | Token::SquareBracketBlock | Token::ParenthesisBlock
        ) {
            input
                .parse_nested_block(|nested| consume_values(nested, execution, 0))
                .map_err(css_error)?;
            continue;
        }
        if token != Token::CurlyBracketBlock {
            continue;
        }
        let prelude = input.slice(rule_start..start).to_string();
        input
            .parse_nested_block(|body| consume_values(body, execution, 0))
            .map_err(css_error)?;
        let end = input.position();
        let is_at_rule = matches!(Parser::new(&prelude).next(), Ok(Token::AtKeyword(_)));
        // usvg does not evaluate conditional at-rules. Preserve them verbatim;
        // do not reinterpret nested CSS or declaration values as selectors.
        let lowered = if is_at_rule {
            prelude
        } else {
            lower_selector_list(&prelude, ids, execution)?
        };
        if !lowered.trim().is_empty() {
            append(&mut output, &lowered, execution)?;
            append(&mut output, input.slice(start..end), execution)?;
        }
        rule_start = end;
    }
    append(&mut output, input.slice_from(rule_start), execution)?;
    Ok(output)
}

fn lower_selector_list(
    css: &str,
    ids: &[String],
    execution: PostprocessWork<'_>,
) -> Result<String> {
    let mut input = Parser::new(css);
    let mut output = String::new();
    while !input.is_exhausted() {
        execution.checkpoint()?;
        let selector = input
            .parse_until_before(Delimiter::Comma, |branch| {
                let start = branch.position();
                consume_values(branch, execution, 0)?;
                Ok(branch.slice_from(start).to_string())
            })
            .map_err(css_error)?;
        for expanded in lower_selector(&selector, ids, execution)? {
            if !output.is_empty() {
                append(&mut output, ",", execution)?;
            }
            append(&mut output, &expanded, execution)?;
        }
        if !input.is_exhausted() {
            input.expect_comma().map_err(error)?;
        }
    }
    Ok(output)
}

fn lower_selector(
    css: &str,
    ids: &[String],
    execution: PostprocessWork<'_>,
) -> Result<Vec<String>> {
    let mut input = Parser::new(css);
    let mut suffix_match = None;
    while !input.is_exhausted() {
        execution.checkpoint()?;
        let start = input.position().byte_index();
        match input
            .next_including_whitespace_and_comments()
            .map_err(error)?
            .clone()
        {
            Token::SquareBracketBlock => {
                let suffix = input
                    .parse_nested_block(|attribute| {
                        if attribute.expect_ident()?.as_ref() != "id" {
                            return Err(cssparser::ParseError::<()>::custom(()));
                        }
                        if *attribute.next()? != Token::SuffixMatch {
                            return Err(cssparser::ParseError::<()>::custom(()));
                        }
                        let suffix = attribute.expect_ident_or_string()?.to_string();
                        attribute.expect_exhausted()?;
                        Ok(suffix)
                    })
                    .ok();
                if let Some(suffix) = suffix {
                    if suffix_match.is_some() {
                        // Mermaid emits one suffix per branch. Avoid a Cartesian
                        // expansion for arbitrary compound user selectors.
                        return Ok(vec![css.to_string()]);
                    }
                    suffix_match = Some((start, input.position().byte_index(), suffix));
                }
            }
            Token::Function(_) | Token::ParenthesisBlock => return Ok(vec![css.to_string()]),
            _ => {}
        }
    }
    let Some((start, end, suffix)) = suffix_match else {
        return Ok(vec![css.to_string()]);
    };
    let mut output = Vec::new();
    let mut projected = 0usize;
    for id in ids {
        execution.checkpoint()?;
        // CSS suffix matching with an empty operand never matches.
        if suffix.is_empty() || !id.ends_with(&suffix) {
            continue;
        }
        let mut value_bytes = ByteCount::default();
        serialize_string(id, &mut value_bytes).map_err(|_| execution.svg_byte_count_overflow())?;
        let bytes = start
            .checked_add(5)
            .and_then(|size| size.checked_add(value_bytes.0))
            .and_then(|size| size.checked_add(css.len() - end))
            .ok_or_else(|| execution.svg_byte_count_overflow())?;
        projected = projected
            .checked_add(bytes)
            .and_then(|size| size.checked_add(1))
            .ok_or_else(|| execution.svg_byte_count_overflow())?;
        execution.preflight_svg_byte_count(projected)?;
        let mut expanded = String::new();
        expanded.try_reserve_exact(bytes).map_err(error)?;
        expanded.push_str(&css[..start]);
        expanded.push_str("[id=");
        serialize_string(id, &mut expanded).map_err(error)?;
        expanded.push(']');
        expanded.push_str(&css[end..]);
        output.push(expanded);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;
    use merman_model::{CancelReason, OperationControl};

    #[test]
    fn preflights_escaped_stylesheet_bytes_at_the_xml_boundary() {
        let svg = format!(
            "<svg id='{}-end'><style>[id$='-end']{{fill:red;/*&lt;&gt;&amp;&#13;*/}}</style></svg>",
            "&amp;".repeat(16),
        );
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let output =
            lower_id_suffix_selectors(Cow::Borrowed(&svg), &OperationWorkMeter::new(policy))
                .unwrap();
        assert!(output.contains("/*&lt;&gt;&amp;&#13;*/"), "{output}");

        let exact = policy
            .with_limit(ResourceLimitId::MaxSvgBytes, output.len())
            .unwrap();
        let exact = lower_id_suffix_selectors(Cow::Borrowed(&svg), &OperationWorkMeter::new(exact))
            .unwrap();
        assert_eq!(exact, output);

        let escaped_end = output.find("</style>").unwrap();
        assert!(escaped_end > svg.len());
        let limited = policy
            .with_limit(ResourceLimitId::MaxSvgBytes, escaped_end - 1)
            .unwrap();
        let failure =
            lower_id_suffix_selectors(Cow::Borrowed(&svg), &OperationWorkMeter::new(limited))
                .unwrap_err();
        let Error::ResourceLimitExceeded(details) = failure else {
            panic!("expected structured byte ceiling, got {failure}");
        };
        assert_eq!(details.phase, ResourceLimitPhase::SvgPostprocess);
        assert_eq!(details.limit, "max_svg_bytes");
        assert_eq!(details.actual, escaped_end);
        assert_eq!(details.max, escaped_end - 1);
    }

    #[test]
    fn admits_input_before_the_no_selector_fast_path() {
        let svg = "<svg/>";
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let exact = policy
            .with_limit(ResourceLimitId::MaxSvgBytes, svg.len())
            .unwrap();
        let borrowed =
            lower_id_suffix_selectors(Cow::Borrowed(svg), &OperationWorkMeter::new(exact)).unwrap();
        assert!(matches!(borrowed, Cow::Borrowed(value) if value == svg));

        let limited = policy
            .with_limit(ResourceLimitId::MaxSvgBytes, svg.len() - 1)
            .unwrap();
        let failure =
            lower_id_suffix_selectors(Cow::Borrowed(svg), &OperationWorkMeter::new(limited))
                .unwrap_err();
        let Error::ResourceLimitExceeded(details) = failure else {
            panic!("expected structured byte ceiling, got {failure}");
        };
        assert_eq!(details.phase, ResourceLimitPhase::SvgPostprocess);
        assert_eq!(details.limit, "max_svg_bytes");
        assert_eq!(details.actual, svg.len());
    }

    #[test]
    fn expansion_obeys_svg_byte_limit() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 64)
            .unwrap();
        let work_meter = OperationWorkMeter::new(policy);
        let ids = vec![
            "very-long-diagram-prefix-aggregationEnd".to_string(),
            "another-long-diagram-prefix-aggregationEnd".to_string(),
        ];
        let result = lower_stylesheet(
            "[id$='-aggregationEnd']{fill:transparent}",
            &ids,
            PostprocessWork(&work_meter),
        );
        let error = result.unwrap_err();
        assert!(error.to_string().contains("max_svg_bytes"), "{error}");
    }

    #[test]
    fn css_walk_preserves_cancellation_provenance() {
        let control = OperationControl::new();
        let work_meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::interactive(),
            control.clone(),
        );
        control.cancel_after_checkpoints(3);
        let error = lower_stylesheet(
            "[id$='-end']{fill:rgb(1,2,3)}",
            &[],
            PostprocessWork(&work_meter),
        )
        .unwrap_err();
        let Error::Cancelled(cancelled) = error else {
            panic!("expected cancellation, got {error}");
        };
        assert_eq!(cancelled.phase, OperationPhase::Postprocess);
        assert_eq!(cancelled.reason, CancelReason::Requested);
    }
}
