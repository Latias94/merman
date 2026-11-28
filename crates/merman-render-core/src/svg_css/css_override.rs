use super::postprocess::PostprocessWork;
use crate::resources::OperationWorkMeter;
use crate::svg_output::{BoundedSvgOutput, SvgOutput};
use crate::svg_scan::{
    CssXmlLocation, SvgTagScanner, XmlCssSource, checkpoint_loop,
    next_svg_quoted_attr_with_checkpoints, start_tag_name,
};
use crate::{Error, Result};
use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, ParseErrorKind, Parser, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser, Token,
    parse_important,
};
use std::borrow::Cow;
use std::ops::Range;

const CSS_NESTING_HARD_LIMIT: u8 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CssOverridePolicy {
    #[default]
    Preserve,
    StripExistingImportant,
}

/// Removes valid CSS declaration priorities while retaining the original XML spelling.
pub fn strip_css_important<'a>(
    svg: Cow<'a, str>,
    work_meter: &OperationWorkMeter,
) -> Result<Cow<'a, str>> {
    let execution = PostprocessWork(work_meter);
    let mut checkpoint = || execution.checkpoint();
    if !has_css_priority_syntax(&svg, &mut checkpoint)? {
        return Ok(svg);
    }
    let mut scanner = SvgTagScanner::new(&svg);
    let mut out = BoundedSvgOutput::new(work_meter);
    let mut cursor = 0;
    while let Some(tag) = scanner.next_with_checkpoints(&mut checkpoint)? {
        let Some(name) = start_tag_name(tag.raw()) else {
            continue;
        };
        let mut attr_cursor = 0;
        while let Some(attr) =
            next_svg_quoted_attr_with_checkpoints(tag.raw(), attr_cursor, &mut checkpoint)?
        {
            attr_cursor = attr.full_end;
            if tag.raw()[attr.name_start..attr.name_end].eq_ignore_ascii_case("style") {
                write_css_priority_edits(
                    &svg,
                    tag.start() + attr.value_start..tag.start() + attr.value_end,
                    CssXmlLocation::Attribute,
                    &mut out,
                    &mut cursor,
                    execution,
                )?;
            }
        }
        if name == "style" && !tag.is_self_closing() {
            let Some(close) = scanner.matching_end_with_checkpoints(tag, &mut checkpoint)? else {
                break;
            };
            write_css_priority_edits(
                &svg,
                tag.end()..close.start(),
                CssXmlLocation::Stylesheet,
                &mut out,
                &mut cursor,
                execution,
            )?;
        }
    }
    if cursor == 0 {
        return Ok(svg);
    }
    out.push_str(&svg[cursor..]);
    Ok(Cow::Owned(out.finish()?))
}

fn has_css_priority_syntax(
    source: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<bool> {
    // A numeric XML reference can supply the exclamation mark.
    for (index, byte) in source.bytes().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        if byte == b'!' || (byte == b'&' && source.as_bytes().get(index + 1) == Some(&b'#')) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn write_css_priority_edits(
    svg: &str,
    range: Range<usize>,
    location: CssXmlLocation,
    out: &mut BoundedSvgOutput<'_>,
    cursor: &mut usize,
    execution: PostprocessWork<'_>,
) -> Result<()> {
    let serialized = &svg[range.clone()];
    let mut checkpoint = || execution.checkpoint();
    if !has_css_priority_syntax(serialized, &mut checkpoint)? {
        return Ok(());
    }
    let Some(css) = XmlCssSource::new(serialized, location, &mut checkpoint)? else {
        return Ok(());
    };
    let edits = priority_edits(css.logical(), location, execution)?;
    css.visit_source_spans(&edits, &mut checkpoint, |_, edit| {
        out.push_str(&svg[*cursor..range.start + edit.start]);
        *cursor = range.start + edit.end;
    })?;
    out.checkpoint()
}

type CssParseError = ParseError<Box<Error>>;

fn priority_edits(
    css: &str,
    location: CssXmlLocation,
    execution: PostprocessWork<'_>,
) -> Result<Vec<Range<usize>>> {
    let mut edits = Vec::new();
    let mut css = Parser::new(css);
    let mut parser = PriorityParser {
        execution,
        edits: &mut edits,
        depth: 0,
        location,
    };
    let parsed = if location == CssXmlLocation::Stylesheet {
        visit_css_items(StyleSheetParser::new(&mut css, &mut parser), execution)
    } else {
        visit_css_items(RuleBodyParser::new(&mut css, &mut parser), execution)
    };
    if let Err(ParseError {
        kind: ParseErrorKind::Custom(error),
        ..
    }) = parsed
    {
        return Err(*error);
    }
    Ok(edits)
}

struct PriorityParser<'a, 'op> {
    execution: PostprocessWork<'op>,
    edits: &'a mut Vec<Range<usize>>,
    depth: u8,
    location: CssXmlLocation,
}

impl PriorityParser<'_, '_> {
    fn parse_rule_body(
        &mut self,
        input: &mut Parser<'_>,
    ) -> std::result::Result<(), CssParseError> {
        if self.location == CssXmlLocation::Attribute {
            return Err(ParseError::unexpected_token());
        }
        if self.depth == CSS_NESTING_HARD_LIMIT {
            return Err(ParseError::custom(Box::new(Error::svg_postprocess(
                "css-override",
                format!("CSS override nesting exceeds the hard limit of {CSS_NESTING_HARD_LIMIT}"),
            ))));
        }
        self.depth += 1;
        let execution = self.execution;
        let result = visit_css_items(RuleBodyParser::new(input, self), execution);
        self.depth -= 1;
        result
    }

    fn consume_tokens(&self, input: &mut Parser<'_>) -> std::result::Result<(), CssParseError> {
        let mut checkpoint = || {
            self.execution
                .checkpoint()
                .map_err(|error| ParseError::custom(Box::new(error)))
        };
        for index in 0.. {
            checkpoint_loop(index, &mut checkpoint)?;
            if input.next().is_err() {
                break;
            }
        }
        Ok(())
    }
}

fn visit_css_items<'i>(
    items: impl Iterator<
        Item = std::result::Result<(), (CssParseError, &'i str, cssparser::SourceLocation)>,
    >,
    execution: PostprocessWork<'_>,
) -> std::result::Result<(), CssParseError> {
    for item in items {
        if let Err((error, _, _)) = item
            && matches!(error.kind, ParseErrorKind::Custom(_))
        {
            return Err(error);
        }
        execution
            .checkpoint()
            .map_err(|error| ParseError::custom(Box::new(error)))?;
    }
    Ok(())
}

impl<'i> DeclarationParser<'i> for PriorityParser<'_, '_> {
    type Declaration = ();
    type Error = Box<Error>;

    fn parse_value(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i>,
        _declaration_start: &ParserState,
    ) -> std::result::Result<(), CssParseError> {
        let mut last_content = input.position().byte_index();
        loop {
            self.execution
                .checkpoint()
                .map_err(|error| ParseError::custom(Box::new(error)))?;
            let state = input.state();
            let Ok(token) = input.next_including_whitespace_and_comments() else {
                return Ok(());
            };
            match token {
                Token::WhiteSpace(_) => continue,
                Token::Delim('!') => {
                    input.reset(&state);
                    if input.try_parse(parse_important).is_ok() {
                        let end = input.position();
                        if input.is_exhausted() {
                            // Comments embedded in the priority are independent of its tokens.
                            let marker_start = state.position().byte_index();
                            let mut marker = Parser::new(input.slice(state.position()..end));
                            loop {
                                let comment_start = marker_start + marker.position().byte_index();
                                match marker.next_including_whitespace_and_comments() {
                                    Ok(Token::Comment(_)) => {
                                        self.edits.push(last_content..comment_start);
                                        last_content =
                                            marker_start + marker.position().byte_index();
                                    }
                                    Ok(_) => {}
                                    Err(_) => break,
                                }
                            }
                            self.edits.push(last_content..end.byte_index());
                            return Ok(());
                        }
                    }
                    input.reset(&state);
                    input.next_including_whitespace_and_comments()?;
                }
                Token::CurlyBracketBlock if !name.starts_with("--") => {
                    // A nested type selector with a pseudo-class can start like a declaration.
                    return Err(ParseError::unexpected_token());
                }
                Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock => {
                    input.parse_nested_block(|nested| self.consume_tokens(nested))?;
                }
                Token::BadString(_)
                | Token::BadUrl(_)
                | Token::CloseParenthesis
                | Token::CloseSquareBracket
                | Token::CloseCurlyBracket => {
                    return Err(ParseError::unexpected_token());
                }
                _ => {}
            }
            last_content = input.position().byte_index();
        }
    }
}

impl<'i> AtRuleParser<'i> for PriorityParser<'_, '_> {
    type Prelude = ();
    type AtRule = ();
    type Error = Box<Error>;

    fn parse_prelude(
        &mut self,
        _name: CowRcStr<'i>,
        input: &mut Parser<'i>,
    ) -> std::result::Result<(), CssParseError> {
        self.consume_tokens(input)
    }

    fn parse_block(
        &mut self,
        (): (),
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> std::result::Result<(), CssParseError> {
        self.parse_rule_body(input)
    }
}

impl<'i> QualifiedRuleParser<'i> for PriorityParser<'_, '_> {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = Box<Error>;

    fn parse_prelude(&mut self, input: &mut Parser<'i>) -> std::result::Result<(), CssParseError> {
        self.consume_tokens(input)
    }

    fn parse_block(
        &mut self,
        (): (),
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> std::result::Result<(), CssParseError> {
        self.parse_rule_body(input)
    }
}

impl<'i> RuleBodyItemParser<'i, (), Box<Error>> for PriorityParser<'_, '_> {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        self.location == CssXmlLocation::Stylesheet
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;

    fn strip(svg: &str) -> String {
        let work = OperationWorkMeter::new(RenderResourcePolicy::interactive());
        strip_css_important(Cow::Borrowed(svg), &work)
            .unwrap()
            .into_owned()
    }

    #[test]
    fn css_override_only_removes_a_trailing_declaration_priority() {
        let svg = "<svg><style>.a{fill:red\t!important;stroke:blue ! IMPORTANT;color:green !importantfoo;}
            .b{fill:red!important-border;color:blue !importanté;}</style></svg>";
        assert_eq!(
            strip(svg),
            "<svg><style>.a{fill:red;stroke:blue;color:green !importantfoo;}
            .b{fill:red!important-border;color:blue !importanté;}</style></svg>"
        );
    }

    #[test]
    fn css_override_preserves_literal_priority_text_and_xml_markup() {
        let svg = concat!(
            r#"<svg><?note <style>.p{color:red !important;}</style>?>"#,
            r#"<!-- <style>.c{color:red !important;}</style><g style='fill:red !important'/> -->"#,
            r#"<![CDATA[<style>.t{color:red !important;}</style>]]><style/>"#,
            r#"<style data-note='a > b'><![CDATA[@media all{.x{content:"A; !important < B";"#,
            r#"/* keep !important */ fill: red !important;}}]]></style >"#,
            r#"<g style='--label: &quot;A; !important&quot;; stroke: blue !important;'/></svg>"#,
        );
        let expected = svg
            .replace("fill: red !important;", "fill: red;")
            .replace("stroke: blue !important;", "stroke: blue;");
        let out = strip(svg);
        assert_eq!(out, expected);
    }

    #[test]
    fn css_override_preserves_comments_between_priority_tokens_and_entity_spelling() {
        let svg = concat!(
            r#"<svg><style>.x{fill:red <![CDATA[!]]><!-- note -->/* priority */ important;"#,
            r#"stroke:var(--paint, blue) !important;}</style>"#,
            r#"<g style='stroke: blue &#33;impor&#116;ant; --label: "A; !important";'/></svg>"#,
        );
        let out = strip(svg);
        assert_eq!(
            out,
            concat!(
                r#"<svg><style>.x{fill:red<![CDATA[]]><!-- note -->/* priority */;"#,
                r#"stroke:var(--paint, blue);}</style>"#,
                r#"<g style='stroke: blue; --label: "A; !important";'/></svg>"#,
            )
        );
    }

    #[test]
    fn css_override_keeps_the_encoded_opening_of_adjacent_priority_comments() {
        let svg = "<svg><style>.x{fill:red !/* first */&#47;* second */important;}</style></svg>";
        assert_eq!(
            strip(svg),
            "<svg><style>.x{fill:red/* first */&#47;* second */;}</style></svg>"
        );
    }

    #[test]
    fn css_override_stops_at_the_css_nesting_limit() {
        let svg = format!(
            "<svg><style>{}.x{{fill:red !important;}}{}</style></svg>",
            "@media all{".repeat(CSS_NESTING_HARD_LIMIT.into()),
            "}".repeat(CSS_NESTING_HARD_LIMIT.into()),
        );
        let work = OperationWorkMeter::new(RenderResourcePolicy::interactive());
        let error = strip_css_important(Cow::Borrowed(&svg), &work).unwrap_err();
        assert!(
            error.to_string().contains("CSS override nesting"),
            "{error}"
        );
    }

    #[test]
    fn css_override_strips_important_only_from_css_contexts() {
        let svg = r#"<svg><style>.node{fill:red !important;}</style><text>keep !important</text><path data-note="keep !important" style="stroke: blue !important; fill: green"/></svg>"#;
        let out = strip(svg);

        assert!(out.contains(".node{fill:red;}"), "got: {out}");
        assert!(
            out.contains("style=\"stroke: blue; fill: green\""),
            "got: {out}"
        );
        assert!(out.contains("<text>keep !important</text>"), "got: {out}");
        assert!(out.contains("data-note=\"keep !important\""), "got: {out}");
    }

    #[test]
    fn css_override_handles_single_quoted_style_attributes() {
        let svg = r#"<svg><path style='stroke: blue !important; fill: green'/></svg>"#;
        let out = strip(svg);

        assert!(
            out.contains("style='stroke: blue; fill: green'"),
            "got: {out}"
        );
    }
}
