//! Mermaid CSS/style helpers shared by layout and SVG parity code.

use cssparser::{
    BasicParseErrorKind, ParseError, Parser, ParserInput, SourcePosition, Token, parse_important,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedStyleDeclaration<'a> {
    property: String,
    property_source: &'a str,
    source_value: &'a str,
    value: &'a str,
    important: bool,
    analysis: CssValueAnalysis,
}

impl<'a> ParsedStyleDeclaration<'a> {
    pub(crate) fn property(&self) -> &str {
        &self.property
    }

    /// Returns the source spelling of the property for compatibility callers that
    /// must borrow the original declaration instead of allocating a decoded name.
    pub(crate) const fn property_source(&self) -> &'a str {
        self.property_source
    }

    pub(crate) const fn property_css(&self) -> &'a str {
        self.property_source
    }

    /// Returns the declaration value including a trailing `!important`, if present.
    pub(crate) const fn source_value(&self) -> &'a str {
        self.source_value
    }

    pub(crate) const fn value(&self) -> &'a str {
        self.value
    }

    pub(crate) const fn important(&self) -> bool {
        self.important
    }

    pub(crate) const fn analysis(&self) -> &CssValueAnalysis {
        &self.analysis
    }

    pub(crate) const fn is_single_component_value(&self) -> bool {
        self.analysis.is_single_component()
    }

    pub(crate) fn svg_number_or_px(&self) -> Option<f64> {
        self.analysis.svg_number_or_px()
    }

    pub(crate) fn resolve_font_size_px(&self, context: CssFontSizeContext) -> Option<f64> {
        self.analysis.resolve_font_size_px(context)
    }
}

pub(crate) fn parse_style_declaration(raw: &str) -> Option<ParsedStyleDeclaration<'_>> {
    let raw = raw.trim();
    let raw = raw.strip_suffix(';').unwrap_or(raw).trim();
    if raw.is_empty() {
        return None;
    }

    let mut input = ParserInput::new(raw);
    let mut parser = Parser::new(&mut input);
    let property_start = parser.position();
    let property = parser.expect_ident_cloned().ok()?;
    if property == "--" {
        return None;
    }
    let property_end = parser.position();
    parser.expect_colon().ok()?;
    let value_start = parser.position();
    let (value_end, analysis, important) =
        parse_css_value_from_parser(&mut parser, value_start, CssValuePolicy::MermaidSourceStyle)
            .ok()?;
    let source_value = parser.slice(value_start..parser.position()).trim();
    let value = parser.slice(value_start..value_end).trim();
    if value.is_empty() {
        return None;
    }

    let property_source = parser.slice(property_start..property_end).trim();
    let property = property.to_string();
    let property = if property.starts_with("--") {
        property
    } else {
        property.to_ascii_lowercase()
    };
    Some(ParsedStyleDeclaration {
        property,
        property_source,
        source_value,
        value,
        important,
        analysis,
    })
}

pub(crate) fn parse_safe_style_decl(s: &str) -> Option<(&str, &str)> {
    let parsed = parse_style_declaration(s)?;
    Some((parsed.property_source(), parsed.source_value()))
}

pub(crate) fn is_safe_css_font_family_value(value: &str) -> bool {
    css_value_is_safe(value, CssValuePolicy::MermaidSourceStyle) && !value.contains(':')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CssFontFamilyOwnership {
    Inherited,
    SourceOwned,
    Unverified,
}

pub(crate) fn css_font_family_ownership<'a>(
    declarations: impl IntoIterator<Item = &'a str>,
) -> CssFontFamilyOwnership {
    css_font_family_ownership_from_parsed(
        declarations
            .into_iter()
            .flat_map(|declaration| declaration.split(';'))
            .filter_map(parse_style_declaration),
    )
}

/// Resolves a sequence whose items are already individual CSS declarations.
///
/// Unlike [`css_font_family_ownership`], this preserves the writer's fail-closed behavior for a
/// malformed item that smuggles in an extra semicolon-delimited declaration.
pub(crate) fn css_font_family_declaration_ownership<'a>(
    declarations: impl IntoIterator<Item = &'a str>,
) -> CssFontFamilyOwnership {
    css_font_family_ownership_from_parsed(
        declarations.into_iter().filter_map(parse_style_declaration),
    )
}

fn css_font_family_ownership_from_parsed<'a>(
    declarations: impl IntoIterator<Item = ParsedStyleDeclaration<'a>>,
) -> CssFontFamilyOwnership {
    let mut winner: Option<(bool, CssFontFamilyOwnership)> = None;
    for declaration in declarations
        .into_iter()
        .filter(|declaration| declaration.property() == "font-family")
    {
        if winner.is_none_or(|(important, _)| declaration.important() || !important) {
            let ownership = if matches!(
                declaration.analysis().single_ident(),
                Some("inherit" | "unset")
            ) {
                CssFontFamilyOwnership::Inherited
            } else if declaration.analysis().has_dynamic_reference_function() {
                CssFontFamilyOwnership::Unverified
            } else {
                CssFontFamilyOwnership::SourceOwned
            };
            winner = Some((declaration.important(), ownership));
        }
    }
    winner.map_or(CssFontFamilyOwnership::Inherited, |(_, owner)| owner)
}

pub(crate) fn is_resource_free_css_value(value: &str) -> bool {
    css_value_is_safe(value, CssValuePolicy::PortableTheme)
}

pub(crate) fn is_label_style_key(key: &str) -> bool {
    let key = key.trim().to_ascii_lowercase();
    matches!(
        key.as_str(),
        "color"
            | "font-size"
            | "font-family"
            | "font-weight"
            | "font-style"
            | "text-decoration"
            | "text-align"
            | "text-transform"
            | "line-height"
            | "letter-spacing"
            | "word-spacing"
            | "text-shadow"
            | "text-overflow"
            | "white-space"
            | "word-wrap"
            | "word-break"
            | "overflow-wrap"
            | "hyphens"
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CssFontSizeContext {
    inherited_px: f64,
    root_px: f64,
    medium_px: f64,
}

impl CssFontSizeContext {
    pub(crate) fn new(inherited_px: f64, root_px: f64) -> Self {
        Self {
            inherited_px: inherited_px.max(1.0),
            root_px: root_px.max(1.0),
            medium_px: 16.0,
        }
    }

    pub(crate) fn uniform(font_size_px: f64) -> Self {
        Self::new(font_size_px, font_size_px)
    }

    pub(crate) fn with_medium_px(mut self, medium_px: f64) -> Self {
        self.medium_px = medium_px.max(1.0);
        self
    }
}

pub(crate) fn resolve_mermaid_font_size_px(raw: &str, context: CssFontSizeContext) -> Option<f64> {
    parse_css_value(raw, CssValuePolicy::MermaidSourceStyle)?
        .analysis
        .resolve_font_size_px(context)
}

pub(crate) fn parse_svg_number_or_px(raw: &str) -> Option<f64> {
    parse_css_value(raw, CssValuePolicy::MermaidSourceStyle)?
        .analysis
        .svg_number_or_px()
}

pub(crate) fn strip_css_important(raw: &str) -> &str {
    let raw = raw.trim();
    let raw = raw.strip_suffix(';').unwrap_or(raw).trim();
    parse_css_value(raw, CssValuePolicy::MermaidSourceStyle)
        .map(|parsed| parsed.value)
        .unwrap_or(raw)
}

pub(crate) fn is_supported_css_font_weight_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "normal" | "bold" | "bolder" | "lighter"
    ) || value
        .trim()
        .parse::<u16>()
        .is_ok_and(|weight| (1..=1000).contains(&weight))
}

pub(crate) fn is_supported_css_font_style_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "normal" | "italic" | "oblique"
    )
}

pub(crate) fn is_supported_css_color_value(value: &str) -> bool {
    merman_core::theme_color::ThemeColor::parse(value.trim()).is_ok()
}

pub(crate) fn is_safe_browser_css_color_value(value: &str) -> bool {
    if is_supported_css_color_value(value) {
        return true;
    }

    let value = value.trim();
    if [
        "currentcolor",
        "inherit",
        "initial",
        "unset",
        "revert",
        "revert-layer",
    ]
    .into_iter()
    .any(|keyword| value.eq_ignore_ascii_case(keyword))
    {
        return true;
    }

    if !value
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("var("))
    {
        return false;
    }
    parse_style_declaration(&format!("color:{value}"))
        .is_some_and(|declaration| declaration.property() == "color" && !declaration.important())
}

pub(crate) fn is_supported_css_text_decoration_value(value: &str) -> bool {
    if value.trim().eq_ignore_ascii_case("none") {
        return true;
    }
    let mut count = 0;
    for token in value.split_ascii_whitespace() {
        count += 1;
        if !matches!(
            token.to_ascii_lowercase().as_str(),
            "underline" | "overline" | "line-through" | "blink"
        ) {
            return false;
        }
    }
    count != 0
}

pub(crate) fn is_supported_css_text_align_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "start" | "end" | "left" | "right" | "center" | "justify" | "match-parent" | "justify-all"
    )
}

pub(crate) fn is_supported_css_line_height_value(value: &str) -> bool {
    let Some(parsed) = parse_css_value(value, CssValuePolicy::MermaidSourceStyle) else {
        return false;
    };
    if !parsed.analysis.is_single_component() {
        return false;
    }
    match parsed.analysis.scalar.as_ref() {
        Some(CssScalar::Ident(value)) => value == "normal",
        Some(
            CssScalar::Number(value)
            | CssScalar::Percentage(value)
            | CssScalar::Px(value)
            | CssScalar::Em(value),
        ) => value.is_finite() && *value > 0.0,
        Some(CssScalar::Rem(_)) | None => false,
    }
}

pub(crate) fn is_supported_css_spacing_value(value: &str) -> bool {
    let Some(parsed) = parse_css_value(value, CssValuePolicy::MermaidSourceStyle) else {
        return false;
    };
    if !parsed.analysis.is_single_component() {
        return false;
    }
    match parsed.analysis.scalar.as_ref() {
        Some(CssScalar::Ident(value)) => value == "normal",
        Some(CssScalar::Number(value)) => *value == 0.0,
        Some(CssScalar::Px(value) | CssScalar::Em(value)) => value.is_finite(),
        Some(CssScalar::Percentage(_) | CssScalar::Rem(_)) | None => false,
    }
}

#[derive(Debug, Clone, PartialEq)]
enum CssScalar {
    Number(f64),
    Percentage(f64),
    Px(f64),
    Em(f64),
    Rem(f64),
    Ident(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CssValueAnalysis {
    scalar: Option<CssScalar>,
    component_count: usize,
    has_dynamic_reference_function: bool,
}

impl CssValueAnalysis {
    pub(crate) const fn is_single_component(&self) -> bool {
        self.component_count == 1
    }

    fn single_ident(&self) -> Option<&str> {
        if !self.is_single_component() {
            return None;
        }
        match self.scalar.as_ref()? {
            CssScalar::Ident(value) => Some(value.as_str()),
            CssScalar::Number(_)
            | CssScalar::Percentage(_)
            | CssScalar::Px(_)
            | CssScalar::Em(_)
            | CssScalar::Rem(_) => None,
        }
    }

    pub(crate) const fn has_dynamic_reference_function(&self) -> bool {
        self.has_dynamic_reference_function
    }

    pub(crate) fn svg_number_or_px(&self) -> Option<f64> {
        if !self.is_single_component() {
            return None;
        }
        let value = match self.scalar.as_ref()? {
            CssScalar::Number(value) | CssScalar::Px(value) => *value,
            CssScalar::Percentage(_)
            | CssScalar::Em(_)
            | CssScalar::Rem(_)
            | CssScalar::Ident(_) => return None,
        };
        (value.is_finite() && value >= 0.0).then_some(value)
    }

    pub(crate) fn resolve_font_size_px(&self, context: CssFontSizeContext) -> Option<f64> {
        if !self.is_single_component() {
            return None;
        }
        let value = match self.scalar.as_ref()? {
            CssScalar::Number(value) | CssScalar::Px(value) => *value,
            CssScalar::Percentage(factor) => context.inherited_px * factor,
            CssScalar::Em(scale) => context.inherited_px * scale,
            CssScalar::Rem(scale) => context.root_px * scale,
            CssScalar::Ident(keyword) => match keyword.as_str() {
                "xx-small" => context.medium_px * 0.6,
                "x-small" => context.medium_px * 0.75,
                "small" => context.medium_px * 0.89,
                "medium" => context.medium_px,
                "large" => context.medium_px * 1.2,
                "x-large" => context.medium_px * 1.5,
                "xx-large" => context.medium_px * 2.0,
                "smaller" => context.inherited_px * 0.8,
                "larger" => context.inherited_px * 1.2,
                _ => return None,
            },
        };
        (value.is_finite() && value > 0.0).then_some(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ParsedCssValue<'a> {
    value: &'a str,
    analysis: CssValueAnalysis,
}

fn parse_css_value(raw: &str, policy: CssValuePolicy) -> Option<ParsedCssValue<'_>> {
    let raw = raw.trim().strip_suffix(';').unwrap_or(raw.trim()).trim();
    if !css_value_source_is_safe(raw) {
        return None;
    }

    let mut input = ParserInput::new(raw);
    let mut parser = Parser::new(&mut input);
    let value_start = parser.position();
    let (value_end, analysis, _) =
        parse_css_value_from_parser(&mut parser, value_start, policy).ok()?;
    let value = parser.slice(value_start..value_end).trim();
    (!value.is_empty()).then_some(ParsedCssValue { value, analysis })
}

fn parse_css_value_from_parser<'i, 't>(
    parser: &mut Parser<'i, 't>,
    value_start: SourcePosition,
    policy: CssValuePolicy,
) -> Result<(SourcePosition, CssValueAnalysis, bool), ParseError<'i, ()>> {
    let mut component_count = 0;
    let mut scalar = None;
    let mut has_dynamic_reference_function = false;
    loop {
        let token_start = parser.position();
        let state = parser.state();
        let token = match parser.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => {
                return finish_css_value(
                    parser,
                    value_start,
                    parser.position(),
                    CssValueAnalysis {
                        scalar,
                        component_count,
                        has_dynamic_reference_function,
                    },
                    false,
                );
            }
            Err(error) => return Err(error.into()),
        };

        if matches!(token, Token::WhiteSpace(_)) {
            continue;
        }

        if matches!(token, Token::Delim('!')) {
            parser.reset(&state);
            if parser.try_parse(parse_important).is_ok() {
                if parser.is_exhausted() && component_count > 0 {
                    while parser.next_including_whitespace().is_ok() {}
                    return finish_css_value(
                        parser,
                        value_start,
                        state.position(),
                        CssValueAnalysis {
                            scalar,
                            component_count,
                            has_dynamic_reference_function,
                        },
                        true,
                    );
                }
                return Err(parser.new_custom_error(()));
            }
            return Err(parser.new_custom_error(()));
        }

        let token_source = parser.slice(token_start..parser.position());
        let scalar_candidate = (component_count == 0)
            .then(|| parse_css_scalar(&token, token_source))
            .flatten();
        validate_css_component(
            parser,
            token_start,
            token,
            policy,
            0,
            &mut has_dynamic_reference_function,
        )?;
        component_count += 1;
        if component_count == 1 {
            scalar = scalar_candidate;
        } else {
            scalar = None;
        }
    }
}

fn finish_css_value<'i, 't>(
    parser: &Parser<'i, 't>,
    value_start: SourcePosition,
    value_end: SourcePosition,
    analysis: CssValueAnalysis,
    important: bool,
) -> Result<(SourcePosition, CssValueAnalysis, bool), ParseError<'i, ()>> {
    let source = parser.slice(value_start..parser.position());
    if !css_value_source_is_safe(source) {
        return Err(parser.new_custom_error(()));
    }
    Ok((value_end, analysis, important))
}

fn parse_css_scalar(token: &Token<'_>, token_source: &str) -> Option<CssScalar> {
    match token {
        Token::Number { .. } => Some(CssScalar::Number(parse_css_number_prefix(token_source)?)),
        Token::Percentage { .. } => Some(CssScalar::Percentage(
            parse_css_number_prefix(token_source)? / 100.0,
        )),
        Token::Dimension { unit, .. } if unit.eq_ignore_ascii_case("px") => {
            Some(CssScalar::Px(parse_css_number_prefix(token_source)?))
        }
        Token::Dimension { unit, .. } if unit.eq_ignore_ascii_case("em") => {
            Some(CssScalar::Em(parse_css_number_prefix(token_source)?))
        }
        Token::Dimension { unit, .. } if unit.eq_ignore_ascii_case("rem") => {
            Some(CssScalar::Rem(parse_css_number_prefix(token_source)?))
        }
        Token::Ident(value) => Some(CssScalar::Ident(value.to_ascii_lowercase())),
        _ => None,
    }
}

fn parse_css_number_prefix(raw: &str) -> Option<f64> {
    let bytes = raw.as_bytes();
    let mut end = 0;
    if bytes
        .get(end)
        .is_some_and(|byte| matches!(byte, b'+' | b'-'))
    {
        end += 1;
    }

    let integer_start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let mut has_digit = end > integer_start;
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        let fraction_start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        has_digit |= end > fraction_start;
    }
    if !has_digit {
        return None;
    }

    if bytes
        .get(end)
        .is_some_and(|byte| matches!(byte, b'e' | b'E'))
    {
        let exponent_marker = end;
        end += 1;
        if bytes
            .get(end)
            .is_some_and(|byte| matches!(byte, b'+' | b'-'))
        {
            end += 1;
        }
        let exponent_start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if end == exponent_start {
            end = exponent_marker;
        }
    }

    let value = raw[..end].parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CssValuePolicy {
    MermaidSourceStyle,
    PortableTheme,
}

fn css_value_is_safe(value: &str, policy: CssValuePolicy) -> bool {
    parse_css_value(value, policy).is_some()
}

fn css_value_source_is_safe(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && !value.contains("/*")
        && !value.contains("*/")
        && !value.chars().any(|character| {
            character.is_control() || matches!(character, '<' | '>' | '{' | '}' | ';')
        })
}

fn validate_css_component<'i, 't>(
    parser: &mut Parser<'i, 't>,
    token_start: SourcePosition,
    token: Token<'i>,
    policy: CssValuePolicy,
    depth: u8,
    has_dynamic_reference_function: &mut bool,
) -> Result<(), ParseError<'i, ()>> {
    const MAX_NESTING: u8 = 32;

    match token {
        Token::UnquotedUrl(_)
        | Token::AtKeyword(_)
        | Token::Comment(_)
        | Token::Semicolon
        | Token::CurlyBracketBlock
        | Token::BadUrl(_)
        | Token::BadString(_)
        | Token::CloseParenthesis
        | Token::CloseSquareBracket
        | Token::CloseCurlyBracket
        | Token::CDO
        | Token::CDC
        | Token::Delim('!') => Err(parser.new_custom_error(())),
        Token::Function(name) => {
            *has_dynamic_reference_function |=
                matches!(name.to_ascii_lowercase().as_str(), "var" | "env");
            if function_is_forbidden(&name, policy) || depth >= MAX_NESTING {
                return Err(parser.new_custom_error(()));
            }
            parser.parse_nested_block(|nested| {
                consume_safe_component_values(
                    nested,
                    policy,
                    depth + 1,
                    has_dynamic_reference_function,
                )
            })?;
            ensure_source_closed_block(parser, token_start, ')')
        }
        Token::ParenthesisBlock | Token::SquareBracketBlock => {
            if depth >= MAX_NESTING {
                return Err(parser.new_custom_error(()));
            }
            let close = if matches!(token, Token::ParenthesisBlock) {
                ')'
            } else {
                ']'
            };
            parser.parse_nested_block(|nested| {
                consume_safe_component_values(
                    nested,
                    policy,
                    depth + 1,
                    has_dynamic_reference_function,
                )
            })?;
            ensure_source_closed_block(parser, token_start, close)
        }
        _ => Ok(()),
    }
}

fn consume_safe_component_values<'i, 't>(
    parser: &mut Parser<'i, 't>,
    policy: CssValuePolicy,
    depth: u8,
    has_dynamic_reference_function: &mut bool,
) -> Result<(), ParseError<'i, ()>> {
    loop {
        let token_start = parser.position();
        let token = match parser.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => return Ok(()),
            Err(error) => return Err(error.into()),
        };

        validate_css_component(
            parser,
            token_start,
            token,
            policy,
            depth,
            has_dynamic_reference_function,
        )?;
    }
}

fn function_is_forbidden(name: &str, policy: CssValuePolicy) -> bool {
    let name = name.to_ascii_lowercase();
    matches!(
        name.as_str(),
        "url"
            | "src"
            | "image"
            | "image-set"
            | "-webkit-image-set"
            | "cross-fade"
            | "element"
            | "paint"
            | "expression"
    ) || (policy == CssValuePolicy::PortableTheme
        && matches!(name.as_str(), "var" | "env" | "attr"))
}

fn ensure_source_closed_block<'i, 't>(
    parser: &Parser<'i, 't>,
    token_start: SourcePosition,
    close: char,
) -> Result<(), ParseError<'i, ()>> {
    let raw_block = parser.slice(token_start..parser.position());
    if raw_block.trim_end().ends_with(close) && source_closes_initial_block(raw_block, close) {
        return Ok(());
    }
    Err(parser.new_custom_error(()))
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
    if !matches!(
        (&token, close),
        (Token::Function(_) | Token::ParenthesisBlock, ')') | (Token::SquareBracketBlock, ']')
    ) {
        return false;
    }
    if parser
        .parse_nested_block(|nested| {
            while nested.next_including_whitespace().is_ok() {}
            Ok::<_, ParseError<'_, ()>>(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_safe_style_decl_accepts_mermaid_style_values() {
        assert_eq!(
            parse_safe_style_decl("fill: rgba(232,232,232, 0.8)"),
            Some(("fill", "rgba(232,232,232, 0.8)"))
        );
        assert_eq!(
            parse_safe_style_decl("font-family: \"IBM Plex Sans\", Arial, sans-serif"),
            Some(("font-family", "\"IBM Plex Sans\", Arial, sans-serif"))
        );
        assert_eq!(
            parse_safe_style_decl("stroke-dasharray: 5,5"),
            Some(("stroke-dasharray", "5,5"))
        );
        let parsed =
            parse_style_declaration("FONT-SIZE: 24px !IMPORTANT;").expect("typed declaration");
        assert_eq!(parsed.property(), "font-size");
        assert_eq!(parsed.value(), "24px");
        assert_eq!(parsed.source_value(), "24px !IMPORTANT");
        assert!(parsed.important());
        assert!(parsed.is_single_component_value());
        let custom = parse_style_declaration("--BrandColor: #fff").expect("custom property");
        assert_eq!(custom.property(), "--BrandColor");
        let escaped = parse_style_declaration(r"f\69ll: #fff").expect("escaped property");
        assert_eq!(escaped.property(), "fill");
        let escaped_custom =
            parse_style_declaration(r"--brand\:accent: #fff").expect("escaped custom property");
        assert_eq!(escaped_custom.property(), "--brand:accent");
        assert_eq!(escaped_custom.value(), "#fff");
        assert_eq!(
            parse_safe_style_decl(r"--brand\:accent: #fff"),
            Some((r"--brand\:accent", "#fff"))
        );
        assert_eq!(parse_style_declaration("1fill: #fff"), None);
        assert_eq!(parse_style_declaration("fill: !important"), None);
        assert_eq!(
            parse_style_declaration("fill: red !important trailing"),
            None
        );
        assert_eq!(parse_style_declaration("fill: red !foo"), None);
    }

    #[test]
    fn browser_color_values_accept_host_variables_without_admitting_css_injection() {
        for value in [
            "#123456",
            "currentColor",
            "inherit",
            "var(--message-color)",
            "var(--message-color, rgb(15 23 42))",
        ] {
            assert!(is_safe_browser_css_color_value(value), "value={value}");
        }
        for value in [
            "var(--message-color);background:red",
            "url(https://example.test/paint.svg)",
            "expression(alert(1))",
            "calc(1px + 2px)",
        ] {
            assert!(!is_safe_browser_css_color_value(value), "value={value}");
        }
    }

    #[test]
    fn parsed_declarations_expose_structured_important_priority() {
        for raw in [
            "fill:red!important",
            "fill: red ! IMPORTANT",
            "fill: red !ImPoRtAnT;",
        ] {
            let parsed = parse_style_declaration(raw).expect("important declaration");
            assert!(parsed.important(), "raw={raw}");
            assert_eq!(parsed.value(), "red", "raw={raw}");
        }

        for raw in ["fill: red", "content: \"important!\""] {
            let parsed = parse_style_declaration(raw).expect("ordinary declaration");
            assert!(!parsed.important(), "raw={raw}");
        }
    }

    #[test]
    fn font_family_ownership_follows_css_winner_priority() {
        assert_eq!(
            css_font_family_ownership(["font-family:First;font-family:inherit"]),
            CssFontFamilyOwnership::Inherited
        );
        assert_eq!(
            css_font_family_ownership(["font-family:First !important;font-family:inherit"]),
            CssFontFamilyOwnership::SourceOwned
        );
        assert_eq!(
            css_font_family_ownership([
                "font-family:First !important;font-family:unset !important"
            ]),
            CssFontFamilyOwnership::Inherited
        );
        assert_eq!(
            css_font_family_ownership(["font-family:var(--class-font)"]),
            CssFontFamilyOwnership::Unverified
        );
        assert_eq!(
            css_font_family_ownership(["font-family:env(class-font)"]),
            CssFontFamilyOwnership::Unverified
        );
        assert_eq!(
            css_font_family_ownership([r"font-family:v\61r(--class-font)"]),
            CssFontFamilyOwnership::Unverified
        );
        assert_eq!(
            css_font_family_ownership([r"font-family:e\6ev(class-font)"]),
            CssFontFamilyOwnership::Unverified
        );
        assert_eq!(
            css_font_family_ownership([r"font-family:i\6eherit"]),
            CssFontFamilyOwnership::Inherited
        );
        assert_eq!(
            css_font_family_ownership([r"font-family:u\6eset"]),
            CssFontFamilyOwnership::Inherited
        );
    }

    #[test]
    fn pre_split_font_declarations_reject_embedded_extra_declarations() {
        let malformed = ["font-family:inherit;font-family:Source"];
        assert_eq!(
            css_font_family_ownership(malformed),
            CssFontFamilyOwnership::SourceOwned
        );
        assert_eq!(
            css_font_family_declaration_ownership(malformed),
            CssFontFamilyOwnership::Inherited
        );
    }

    #[test]
    fn css_numeric_parsing_uses_css_tokens_instead_of_suffix_trimming() {
        assert_eq!(parse_svg_number_or_px("12px"), Some(12.0));
        assert_eq!(parse_svg_number_or_px("12PX !important"), Some(12.0));
        assert_eq!(parse_svg_number_or_px("12"), Some(12.0));
        assert_eq!(parse_svg_number_or_px("12pxjunk"), None);
        assert_eq!(parse_svg_number_or_px("calc(10px + 2px)"), None);
        assert_eq!(parse_svg_number_or_px("12px trailing"), None);
        assert_eq!(parse_svg_number_or_px(r"12p\78"), Some(12.0));
        assert_eq!(
            parse_svg_number_or_px("123456789.125px"),
            Some(123456789.125)
        );

        let context = CssFontSizeContext::new(20.0, 16.0);
        assert_eq!(resolve_mermaid_font_size_px("125%", context), Some(25.0));
        assert_eq!(resolve_mermaid_font_size_px("1.5REM", context), Some(24.0));
        assert_eq!(
            resolve_mermaid_font_size_px(r"1.5r\65m", context),
            Some(24.0)
        );
        assert_eq!(
            resolve_mermaid_font_size_px("1.25em !important", context),
            Some(25.0)
        );
        assert_eq!(resolve_mermaid_font_size_px("medium", context), Some(16.0));
        assert_eq!(resolve_mermaid_font_size_px("small", context), Some(14.24));
        assert_eq!(
            resolve_mermaid_font_size_px("medium", context.with_medium_px(18.0),),
            Some(18.0)
        );
        assert_eq!(resolve_mermaid_font_size_px("0px", context), None);
        assert_eq!(strip_css_important("600 !IMPORTANT"), "600");
        assert_eq!(strip_css_important("\"important!\""), "\"important!\"");
        assert_eq!(strip_css_important("red !foo"), "red !foo");
        assert_eq!(strip_css_important("!"), "!");
    }

    #[test]
    fn parsed_values_expose_single_pass_scalar_admission() {
        let valid =
            parse_style_declaration("stroke-width: 2.5px !important").expect("valid stroke width");
        assert_eq!(valid.svg_number_or_px(), Some(2.5));

        let trailing = parse_style_declaration("stroke-width: 2.5px junk")
            .expect("safe but property-invalid value");
        assert!(!trailing.is_single_component_value());
        assert_eq!(trailing.svg_number_or_px(), None);

        let paint = parse_style_declaration("fill: red junk")
            .expect("safe but property-invalid paint value");
        assert!(!paint.is_single_component_value());

        let font = parse_style_declaration("font-size: 125%").expect("valid relative font size");
        assert_eq!(
            font.resolve_font_size_px(CssFontSizeContext::uniform(20.0)),
            Some(25.0)
        );
    }

    #[test]
    fn supported_typography_values_match_the_native_admission_subset() {
        for value in ["normal", "bold", "bolder", "lighter", "1", "700", "1000"] {
            assert!(is_supported_css_font_weight_value(value), "value={value}");
        }
        for value in ["banana", "0", "1001", "700.0"] {
            assert!(!is_supported_css_font_weight_value(value), "value={value}");
        }

        for value in ["normal", "italic", "oblique"] {
            assert!(is_supported_css_font_style_value(value), "value={value}");
        }
        assert!(!is_supported_css_font_style_value("sideways"));

        for value in ["normal", "1.5", "150%", "24px", "1.2em"] {
            assert!(is_supported_css_line_height_value(value), "value={value}");
        }
        for value in ["banana", "0", "-1", "1rem"] {
            assert!(!is_supported_css_line_height_value(value), "value={value}");
        }

        for value in ["normal", "0", "-0.5px", "0.1em"] {
            assert!(is_supported_css_spacing_value(value), "value={value}");
        }
        for value in ["banana", "1", "10%", "1rem"] {
            assert!(!is_supported_css_spacing_value(value), "value={value}");
        }
    }

    #[test]
    fn parse_safe_style_decl_rejects_structural_injection_values() {
        for raw in [
            "fill: red;</style><svg>",
            "fill: red; stroke: blue",
            "fill: red} :not(&){background: green",
            "background: url(javascript:alert(1))",
            r"background: u\72l(//example.test/image.svg)",
            "width: expression(alert(1))",
            "fill: rgba(0, 0, 0, 0.2",
            "fill: red/*",
            "fill: red !important /* hidden */",
            "fill: red !important trailing",
            "fill: red !foo",
            "bad>key: red",
        ] {
            assert_eq!(parse_safe_style_decl(raw), None, "expected reject: {raw}");
        }
    }

    #[test]
    fn portable_theme_values_reject_host_and_resource_functions_after_css_decoding() {
        for raw in [
            r"u\72l(//example.test/theme.svg)",
            r"v\61r(--host-color)",
            "env(safe-area-inset-top)",
            "attr(data-theme color)",
            "src(local-font.woff2)",
            "linear-gradient(red, url(../paint.svg))",
        ] {
            assert!(!is_resource_free_css_value(raw), "expected reject: {raw}");
        }
        for raw in [
            "#123456",
            "rgba(0, 0, 0, 0.25)",
            "drop-shadow(0 1px 2px rgba(0, 0, 0, 0.2))",
        ] {
            assert!(is_resource_free_css_value(raw), "expected accept: {raw}");
        }
    }
}
