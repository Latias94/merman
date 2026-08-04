use crate::Result;
use cssparser::{
    AtRuleParser, BasicParseErrorKind, CowRcStr, ParseError, Parser, ParserInput, ParserState,
    QualifiedRuleParser, SourcePosition, StyleSheetParser, ToCss, Token,
};
use std::borrow::Cow;
use std::fmt;

use super::attr_sanitize::is_unsafe_render_resource_url_value;
use super::util::find_tag_end;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};

const CSS_NESTING_HARD_LIMIT: u8 = 64;

#[derive(Debug, Clone, Copy, Default)]
pub struct SanitizeCssPostprocessor;

impl SvgPostprocessor for SanitizeCssPostprocessor {
    fn name(&self) -> &'static str {
        "sanitize-css"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        if !svg.contains("<style") {
            return Ok(svg);
        }
        Ok(Cow::Owned(sanitize_style_elements(&svg)))
    }
}

pub(crate) fn sanitize_style_elements(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0;

    while let Some(rel_start) = svg[cursor..].find("<style") {
        let start = cursor + rel_start;
        out.push_str(&svg[cursor..start]);

        let Some(open_end) = find_tag_end(svg, start) else {
            out.push_str(&svg[start..]);
            return out;
        };

        let content_start = open_end + 1;
        let Some(rel_close_start) = svg[content_start..].find("</style") else {
            out.push_str(&svg[start..]);
            return out;
        };
        let close_start = content_start + rel_close_start;
        let Some(close_end) = find_tag_end(svg, close_start) else {
            out.push_str(&svg[start..]);
            return out;
        };

        out.push_str(&svg[start..=open_end]);
        out.push_str(&sanitize_css(&svg[content_start..close_start]));
        out.push_str(&svg[close_start..=close_end]);
        cursor = close_end + 1;
    }

    out.push_str(&svg[cursor..]);
    out
}

pub(crate) fn sanitize_css(css: &str) -> String {
    process_stylesheet(css, CssProcessingMode::Sanitize).unwrap_or_default()
}

pub(super) fn sanitize_css_value(value: &str) -> Option<String> {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    rewrite_component_values(
        &mut parser,
        CssProcessingMode::Sanitize,
        CssNestingDepth::default(),
    )
    .ok()
}

pub(in crate::svg::pipeline) fn validate_resvg_css_stylesheet(
    css: &str,
) -> std::result::Result<(), String> {
    process_stylesheet(css, CssProcessingMode::Validate).map(|_| ())
}

pub(in crate::svg::pipeline) fn validate_resvg_css_declaration_list(
    css: &str,
) -> std::result::Result<(), String> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    rewrite_declaration_list(
        &mut parser,
        CssProcessingMode::Validate,
        CssNestingDepth::default(),
    )
    .map(|_| ())
    .map_err(format_parse_error)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CssProcessingMode {
    Sanitize,
    Validate,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CssNestingDepth(u8);

impl CssNestingDepth {
    fn descend<'i, 't>(
        self,
        input: &Parser<'i, 't>,
    ) -> std::result::Result<Self, ParseError<'i, CssViolation>> {
        if self.0 >= CSS_NESTING_HARD_LIMIT {
            return Err(input.new_custom_error(CssViolation::NestingLimit));
        }
        Ok(Self(self.0 + 1))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CssViolation {
    Animation,
    BadToken,
    Degrees,
    EmptyDeclaration,
    MarkerReference,
    NestingLimit,
    RootSelector,
    UnclosedBlock,
    UnsafeUrl,
    UnsupportedAtRule(String),
}

impl fmt::Display for CssViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Animation => f.write_str("CSS animation is not part of the resvg-safe contract"),
            Self::BadToken => f.write_str("malformed CSS token"),
            Self::Degrees => {
                f.write_str("CSS angle units are not accepted by the resvg-safe contract")
            }
            Self::EmptyDeclaration => f.write_str("empty CSS declaration"),
            Self::MarkerReference => {
                f.write_str("CSS marker references are not part of the resvg-safe contract")
            }
            Self::NestingLimit => write!(
                f,
                "CSS nesting exceeds the resvg-safe hard limit of {CSS_NESTING_HARD_LIMIT}"
            ),
            Self::RootSelector => {
                f.write_str(":root rules are not part of the resvg-safe contract")
            }
            Self::UnclosedBlock => f.write_str("unclosed CSS block or function"),
            Self::UnsafeUrl => f.write_str("unsafe CSS URL"),
            Self::UnsupportedAtRule(name) => {
                write!(f, "@{name} is not part of the resvg-safe contract")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtRuleBody {
    Declarations,
    RuleList,
}

#[derive(Debug)]
enum AtRulePrelude {
    Drop,
    Keep {
        name: String,
        prelude: String,
        body: AtRuleBody,
    },
}

struct ResvgCssRuleParser {
    mode: CssProcessingMode,
    depth: CssNestingDepth,
}

impl<'i> AtRuleParser<'i> for ResvgCssRuleParser {
    type Prelude = AtRulePrelude;
    type AtRule = String;
    type Error = CssViolation;

    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> std::result::Result<Self::Prelude, ParseError<'i, Self::Error>> {
        let prelude = rewrite_component_values(input, self.mode, self.depth)?;
        let normalized_name = name.to_ascii_lowercase();
        let body = match normalized_name.as_str() {
            "font-face" | "page" => Some(AtRuleBody::Declarations),
            "container" | "document" | "layer" | "media" | "scope" | "supports" => {
                Some(AtRuleBody::RuleList)
            }
            _ => None,
        };

        let explicitly_removed = matches!(
            normalized_name.as_str(),
            "import" | "keyframes" | "-webkit-keyframes"
        );
        if explicitly_removed || body.is_none() {
            if self.mode == CssProcessingMode::Validate {
                return Err(
                    input.new_custom_error(CssViolation::UnsupportedAtRule(normalized_name))
                );
            }
            return Ok(AtRulePrelude::Drop);
        }

        Ok(AtRulePrelude::Keep {
            name: name.to_string(),
            prelude,
            body: body.expect("checked above"),
        })
    }

    fn rule_without_block(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
    ) -> std::result::Result<Self::AtRule, ()> {
        match prelude {
            AtRulePrelude::Drop => Ok(String::new()),
            AtRulePrelude::Keep { .. } => Err(()),
        }
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> std::result::Result<Self::AtRule, ParseError<'i, Self::Error>> {
        let depth = self.depth.descend(input)?;
        let AtRulePrelude::Keep {
            name,
            prelude,
            body,
        } = prelude
        else {
            consume_component_values(input, depth)?;
            return Ok(String::new());
        };

        let body = match body {
            AtRuleBody::Declarations => rewrite_declaration_list(input, self.mode, depth)?,
            AtRuleBody::RuleList => rewrite_rule_list(input, self.mode, depth)?,
        };
        Ok(format!("@{name}{prelude}{{{body}}}"))
    }
}

impl<'i> QualifiedRuleParser<'i> for ResvgCssRuleParser {
    type Prelude = Option<String>;
    type QualifiedRule = String;
    type Error = CssViolation;

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> std::result::Result<Self::Prelude, ParseError<'i, Self::Error>> {
        let prelude = rewrite_component_values(input, self.mode, self.depth)?;
        if selector_contains_root(&prelude, self.depth)
            .map_err(|violation| input.new_custom_error(violation))?
        {
            if self.mode == CssProcessingMode::Validate {
                return Err(input.new_custom_error(CssViolation::RootSelector));
            }
            return Ok(None);
        }
        Ok(Some(prelude))
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> std::result::Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        let depth = self.depth.descend(input)?;
        let Some(prelude) = prelude else {
            consume_component_values(input, depth)?;
            return Ok(String::new());
        };
        let declarations = rewrite_declaration_list(input, self.mode, depth)?;
        Ok(format!("{prelude}{{{declarations}}}"))
    }
}

fn process_stylesheet(css: &str, mode: CssProcessingMode) -> std::result::Result<String, String> {
    let mut input = ParserInput::new(css);
    let mut input = Parser::new(&mut input);
    rewrite_rule_list(&mut input, mode, CssNestingDepth::default()).map_err(format_parse_error)
}

fn rewrite_rule_list<'i, 't>(
    input: &mut Parser<'i, 't>,
    mode: CssProcessingMode,
    depth: CssNestingDepth,
) -> std::result::Result<String, ParseError<'i, CssViolation>> {
    let mut parser = ResvgCssRuleParser { mode, depth };
    let mut output = String::new();

    for rule in StyleSheetParser::new(input, &mut parser) {
        match rule {
            Ok(rule) => output.push_str(&rule),
            Err((error, _)) if mode == CssProcessingMode::Validate => return Err(error),
            Err(_) => {}
        }
    }

    Ok(output)
}

fn rewrite_declaration_list<'i, 't>(
    input: &mut Parser<'i, 't>,
    mode: CssProcessingMode,
    depth: CssNestingDepth,
) -> std::result::Result<String, ParseError<'i, CssViolation>> {
    let mut output = String::new();

    loop {
        let declaration_start = input.position();
        if input.is_exhausted() {
            output.push_str(input.slice_from(declaration_start));
            return Ok(output);
        }

        let declaration = input.parse_until_after(cssparser::Delimiter::Semicolon, |declaration| {
            let prefix_start = declaration.position();
            let property = declaration.expect_ident_cloned()?;
            declaration.expect_colon()?;
            let value_start = declaration.position();
            let prefix = declaration.slice(prefix_start..value_start).to_string();

            if is_animation_property(&property) {
                consume_component_values(declaration, depth)?;
                if mode == CssProcessingMode::Validate {
                    return Err(declaration.new_custom_error(CssViolation::Animation));
                }
                return Ok(None);
            }

            if is_marker_reference_property(&property) {
                consume_component_values(declaration, depth)?;
                if mode == CssProcessingMode::Validate {
                    return Err(declaration.new_custom_error(CssViolation::MarkerReference));
                }
                return Ok(None);
            }

            let value = rewrite_component_values(declaration, mode, depth)?;
            if value.trim().is_empty() {
                return Err(declaration.new_custom_error(CssViolation::EmptyDeclaration));
            }
            Ok(Some((prefix, value)))
        });
        let declaration_end = input.position();
        let had_semicolon = input
            .slice(declaration_start..declaration_end)
            .trim_end()
            .ends_with(';');

        match declaration {
            Ok(Some((prefix, value))) => {
                output.push_str(&prefix);
                output.push_str(&value);
                if had_semicolon {
                    output.push(';');
                }
            }
            Ok(None) => {}
            Err(error) if mode == CssProcessingMode::Validate => return Err(error),
            Err(_) => {}
        }
    }
}

fn rewrite_component_values<'i, 't>(
    input: &mut Parser<'i, 't>,
    mode: CssProcessingMode,
    depth: CssNestingDepth,
) -> std::result::Result<String, ParseError<'i, CssViolation>> {
    let mut output = String::new();

    loop {
        let token_start = input.position();
        let token = match input.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => {
                output.push_str(input.slice_from(token_start));
                return Ok(output);
            }
            Err(error) => return Err(error.into()),
        };
        let token_end = input.position();

        match token {
            Token::Dimension {
                value,
                int_value,
                has_sign,
                unit,
            } if unit.eq_ignore_ascii_case("deg") => {
                if mode == CssProcessingMode::Validate {
                    return Err(input.new_custom_error(CssViolation::Degrees));
                }
                output.push_str(
                    &Token::Number {
                        value,
                        int_value,
                        has_sign,
                    }
                    .to_css_string(),
                );
            }
            Token::UnquotedUrl(url) => {
                if is_unsafe_render_resource_url_value(&url) {
                    return Err(input.new_custom_error(CssViolation::UnsafeUrl));
                }
                output.push_str(input.slice(token_start..token_end));
            }
            Token::Function(name) => {
                let nested_depth = depth.descend(input)?;
                output.push_str(input.slice(token_start..token_end));
                let nested = input.parse_nested_block(|nested| {
                    if name.eq_ignore_ascii_case("url") {
                        rewrite_quoted_url(nested, mode, nested_depth)
                    } else {
                        rewrite_component_values(nested, mode, nested_depth)
                    }
                })?;
                ensure_source_closed_block(input, token_start, ')')?;
                output.push_str(&nested);
                output.push(')');
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                let nested_depth = depth.descend(input)?;
                output.push_str(input.slice(token_start..token_end));
                let nested = input.parse_nested_block(|nested| {
                    rewrite_component_values(nested, mode, nested_depth)
                })?;
                let close = match token {
                    Token::ParenthesisBlock => ')',
                    Token::SquareBracketBlock => ']',
                    Token::CurlyBracketBlock => '}',
                    _ => unreachable!(),
                };
                ensure_source_closed_block(input, token_start, close)?;
                output.push_str(&nested);
                output.push(close);
            }
            Token::BadUrl(_) | Token::BadString(_) => {
                return Err(input.new_custom_error(CssViolation::BadToken));
            }
            _ => output.push_str(input.slice(token_start..token_end)),
        }
    }
}

fn ensure_source_closed_block<'i, 't>(
    input: &Parser<'i, 't>,
    token_start: SourcePosition,
    close: char,
) -> std::result::Result<(), ParseError<'i, CssViolation>> {
    let raw_block = input.slice(token_start..input.position());
    if raw_block.trim_end().ends_with(close) && source_closes_initial_block(raw_block, close) {
        return Ok(());
    }

    Err(input.new_custom_error(CssViolation::UnclosedBlock))
}

fn source_closes_initial_block(raw_block: &str, close: char) -> bool {
    const SENTINEL: &str = "__merman_css_closed_block_sentinel__";

    let mut probe = String::with_capacity(raw_block.len() + SENTINEL.len() + 1);
    probe.push_str(raw_block);
    probe.push(' ');
    probe.push_str(SENTINEL);

    let mut input = ParserInput::new(&probe);
    let mut parser = Parser::new(&mut input);
    let Ok(token) = parser.next_including_whitespace().cloned() else {
        return false;
    };
    if !opening_token_matches_close(&token, close) {
        return false;
    }

    if parser
        .parse_nested_block(|nested| {
            while nested.next_including_whitespace().is_ok() {}
            Ok::<_, ParseError<'_, CssViolation>>(())
        })
        .is_err()
    {
        return false;
    }

    matches!(
        parser.next(),
        Ok(Token::Ident(name)) if name.as_ref() == SENTINEL
    )
}

fn opening_token_matches_close(token: &Token<'_>, close: char) -> bool {
    match token {
        Token::Function(_) | Token::ParenthesisBlock => close == ')',
        Token::SquareBracketBlock => close == ']',
        Token::CurlyBracketBlock => close == '}',
        _ => false,
    }
}

fn rewrite_quoted_url<'i, 't>(
    input: &mut Parser<'i, 't>,
    mode: CssProcessingMode,
    depth: CssNestingDepth,
) -> std::result::Result<String, ParseError<'i, CssViolation>> {
    let url_start = input.position();
    let url = input.expect_string_cloned()?;
    input.expect_exhausted()?;
    if is_unsafe_render_resource_url_value(&url) {
        return Err(input.new_custom_error(CssViolation::UnsafeUrl));
    }

    let raw = input.slice_from(url_start);
    let mut raw_input = ParserInput::new(raw);
    let mut raw_parser = Parser::new(&mut raw_input);
    rewrite_component_values(&mut raw_parser, mode, depth)
}

fn consume_component_values<'i, 't>(
    input: &mut Parser<'i, 't>,
    depth: CssNestingDepth,
) -> std::result::Result<(), ParseError<'i, CssViolation>> {
    loop {
        let token = match input.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if matches!(
            token,
            Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock
        ) {
            let nested_depth = depth.descend(input)?;
            input.parse_nested_block(|nested| consume_component_values(nested, nested_depth))?;
        }
    }
}

fn is_animation_property(property: &str) -> bool {
    let property = property.to_ascii_lowercase();
    property == "animation" || property.starts_with("animation-")
}

fn is_marker_reference_property(property: &str) -> bool {
    matches!(
        property.to_ascii_lowercase().as_str(),
        "marker" | "marker-start" | "marker-mid" | "marker-end"
    )
}

fn selector_contains_root(
    selector: &str,
    depth: CssNestingDepth,
) -> std::result::Result<bool, CssViolation> {
    let mut input = ParserInput::new(selector);
    let mut parser = Parser::new(&mut input);
    parser_contains_root_selector(&mut parser, depth).map_err(|error| match error.kind {
        cssparser::ParseErrorKind::Custom(violation) => violation,
        cssparser::ParseErrorKind::Basic(_) => CssViolation::BadToken,
    })
}

fn parser_contains_root_selector<'i, 't>(
    input: &mut Parser<'i, 't>,
    depth: CssNestingDepth,
) -> std::result::Result<bool, ParseError<'i, CssViolation>> {
    let mut after_colon = false;
    loop {
        let token = match input.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => {
                return Ok(false);
            }
            Err(error) => return Err(error.into()),
        };
        match token {
            Token::Colon => after_colon = true,
            Token::Ident(name) if after_colon && name.eq_ignore_ascii_case("root") => {
                return Ok(true);
            }
            Token::Function(name) => {
                if after_colon && name.eq_ignore_ascii_case("root") {
                    return Ok(true);
                }
                let nested_depth = depth.descend(input)?;
                if input.parse_nested_block(|nested| {
                    parser_contains_root_selector(nested, nested_depth)
                })? {
                    return Ok(true);
                }
                after_colon = false;
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                let nested_depth = depth.descend(input)?;
                if input.parse_nested_block(|nested| {
                    parser_contains_root_selector(nested, nested_depth)
                })? {
                    return Ok(true);
                }
                after_colon = false;
            }
            Token::WhiteSpace(_) => {}
            _ => after_colon = false,
        }
    }
}

fn format_parse_error(error: ParseError<'_, CssViolation>) -> String {
    format!(
        "{} at line {}, column {}",
        error.kind, error.location.line, error.location.column
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested_function(depth: usize, leaf: &str) -> String {
        format!("{}{}{}", "f(".repeat(depth), leaf, ")".repeat(depth))
    }

    fn nested_media(depth: usize, rule: &str) -> String {
        format!(
            "{}{}{}",
            "@media all{".repeat(depth),
            rule,
            "}".repeat(depth)
        )
    }

    #[test]
    fn css_sanitize_removes_active_rules_and_declarations_after_tokenization() {
        let css = r#"@\69mport url(\"https://example.com/a.css\");@k\65yframes spin{to{opacity:1}}:r\6fot{--x:red}.a{a\6eimation:spin 1s;fill:red}"#;
        let out = sanitize_css(css);

        assert_eq!(out, ".a{fill:red}");
    }

    #[test]
    fn css_sanitize_rewrites_only_dimension_tokens() {
        let css = r##".a{transform:rotate(45deg) rotate(-10.5DEG);content:\"45deg\";background:url(#a45deg);--name:foo45deg;--near:90deg-foo}"##;
        let out = sanitize_css(css);

        assert!(out.contains("rotate(45) rotate(-10.5)"), "{out}");
        assert!(out.contains(r#"content:\"45deg\""#), "{out}");
        assert!(out.contains("url(#a45deg)"), "{out}");
        assert!(out.contains("--name:foo45deg"), "{out}");
        assert!(out.contains("--near:90deg-foo"), "{out}");
    }

    #[test]
    fn css_sanitize_drops_unsafe_urls_and_preserves_safe_image_sources() {
        let css = r##".bad{fill:u\72l(\"j\61vascript:alert(1)\");stroke:red}.fragment{fill:url(#paint)}.image{background:url(data:image/png;base64,AAAA)}"##;
        let out = sanitize_css(css);

        assert_eq!(
            out,
            r##".bad{stroke:red}.fragment{fill:url(#paint)}.image{background:url(data:image/png;base64,AAAA)}"##
        );
    }

    #[test]
    fn css_sanitize_removes_marker_references_that_cannot_be_preflighted() {
        let css = r##".edge{marker:url(#all);marker-start:url(#start);marker-mid:url(#mid);marker-end:url(#end);stroke:red}"##;

        assert_eq!(sanitize_css(css), ".edge{stroke:red}");
        assert!(validate_resvg_css_stylesheet(css).is_err());
        assert!(validate_resvg_css_declaration_list("marker-end:url(#end);stroke:red").is_err());
    }

    #[test]
    fn css_sanitize_drops_external_render_resources() {
        let css = r##".local{background:url(../image.png);stroke:red}.root{cursor:url("/tmp/cursor.svg"),auto;fill:blue}.remote{filter:url(https://example.com/filter.svg#blur);color:black}.fragment{fill:url(#paint)}.embedded{background:url(data:image/png;base64,AAAA)}.missing-comma{background:url(data:image/png);opacity:.5}.spaced{background:url("d a t a:image/png;base64,BBBB");opacity:.75}"##;

        let out = sanitize_css(css);

        assert!(!out.contains("../image.png"), "{out}");
        assert!(!out.contains("/tmp/cursor.svg"), "{out}");
        assert!(!out.contains("https://example.com"), "{out}");
        assert!(out.contains(".local{stroke:red}"), "{out}");
        assert!(out.contains(".root{fill:blue}"), "{out}");
        assert!(out.contains(".remote{color:black}"), "{out}");
        assert!(out.contains(".fragment{fill:url(#paint)}"), "{out}");
        assert!(
            out.contains(".embedded{background:url(data:image/png;base64,AAAA)}"),
            "{out}"
        );
        assert!(out.contains(".missing-comma{opacity:.5}"), "{out}");
        assert!(out.contains(".spaced{opacity:.75}"), "{out}");
        assert!(!out.contains("url(data:image/png)"), "{out}");
        assert!(!out.contains("d a t a:"), "{out}");
    }

    #[test]
    fn css_validation_rejects_non_terminal_constructs() {
        assert!(validate_resvg_css_stylesheet("@import 'a.css';").is_err());
        assert!(validate_resvg_css_stylesheet(".a{animation:spin 1s}").is_err());
        assert!(validate_resvg_css_stylesheet(".a{transform:rotate(45deg)}").is_err());
        assert!(validate_resvg_css_stylesheet(".a{fill:url(javascript:x)}").is_err());
        assert!(validate_resvg_css_stylesheet(".a{content:\"45deg\"}").is_ok());
    }

    #[test]
    fn deg_component_rewrite_does_not_touch_strings_or_urls() {
        assert_eq!(
            sanitize_css_value(r##"rotate(.5deg) \"45deg\" url(#a45deg) foo45deg 90deg-foo"##)
                .as_deref(),
            Some(r##"rotate(0.5) \"45deg\" url(#a45deg) foo45deg 90deg-foo"##)
        );
    }

    #[test]
    fn css_sanitize_rejects_unclosed_component_value_blocks() {
        assert_eq!(sanitize_css_value("5rl('file:///{animatiEtroke:#333"), None);
        assert_eq!(sanitize_css_value("rotate(45deg"), None);
        assert_eq!(sanitize_css_value("outer((red)"), None);
    }

    #[test]
    fn css_nesting_limit_is_inclusive_and_validation_reports_the_limit() {
        let exact = nested_function(CSS_NESTING_HARD_LIMIT.into(), "red");
        let over = nested_function(usize::from(CSS_NESTING_HARD_LIMIT) + 1, "red");

        assert_eq!(sanitize_css_value(&exact).as_deref(), Some(exact.as_str()));
        assert_eq!(sanitize_css_value(&over), None);
        assert!(validate_resvg_css_declaration_list(&format!("fill:{exact}")).is_ok());

        let error = validate_resvg_css_declaration_list(&format!("fill:{over}"))
            .expect_err("one nesting level past the hard limit must be rejected");
        assert!(error.contains("CSS nesting exceeds"), "{error}");
        assert!(
            error.contains(&CSS_NESTING_HARD_LIMIT.to_string()),
            "{error}"
        );
    }

    #[test]
    fn css_nesting_limit_bounds_rules_selectors_and_drop_traversal() {
        let exact_rule_value = nested_function(usize::from(CSS_NESTING_HARD_LIMIT) - 1, "red");
        let over_rule_value = nested_function(CSS_NESTING_HARD_LIMIT.into(), "red");
        let declarations =
            format!(".exact{{fill:{exact_rule_value}}}.over{{fill:{over_rule_value};stroke:blue}}");
        assert_eq!(
            sanitize_css(&declarations),
            format!(".exact{{fill:{exact_rule_value}}}.over{{stroke:blue}}")
        );

        let selector_exact = format!(
            "{}.exact-selector{}{{fill:red}}",
            ":is(".repeat(CSS_NESTING_HARD_LIMIT.into()),
            ")".repeat(CSS_NESTING_HARD_LIMIT.into())
        );
        let selector_over = format!(
            "{}.over-selector{}{{fill:red}}",
            ":is(".repeat(usize::from(CSS_NESTING_HARD_LIMIT) + 1),
            ")".repeat(usize::from(CSS_NESTING_HARD_LIMIT) + 1)
        );
        assert!(validate_resvg_css_stylesheet(&selector_exact).is_ok());
        let selector_error = validate_resvg_css_stylesheet(&selector_over)
            .expect_err("an over-limit selector function must be rejected");
        assert!(selector_error.contains("CSS nesting exceeds"));
        assert!(sanitize_css(&selector_over).is_empty());

        let media_exact = nested_media(CSS_NESTING_HARD_LIMIT.into(), "");
        let media_over = nested_media(usize::from(CSS_NESTING_HARD_LIMIT) + 1, "");
        assert!(validate_resvg_css_stylesheet(&media_exact).is_ok());
        let media_error = validate_resvg_css_stylesheet(&media_over)
            .expect_err("an over-limit grouping rule must be rejected");
        assert!(media_error.contains("CSS nesting exceeds"));
        assert_eq!(
            sanitize_css(&media_over).matches("@media").count(),
            usize::from(CSS_NESTING_HARD_LIMIT)
        );

        let deeply_nested = nested_function(4_096, "spin");
        let dropped = format!(
            ".animation{{animation:{deeply_nested};stroke:blue}}:root{{--x:{deeply_nested}}}@unknown x{{value:{deeply_nested}}}.safe{{fill:red}}"
        );
        assert_eq!(
            sanitize_css(&dropped),
            ".animation{stroke:blue}.safe{fill:red}"
        );
    }
}
