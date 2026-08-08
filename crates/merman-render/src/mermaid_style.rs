//! Mermaid CSS/style helpers shared by layout and SVG parity code.

use cssparser::{BasicParseErrorKind, ParseError, Parser, ParserInput, SourcePosition, Token};

pub(crate) fn parse_safe_style_decl(s: &str) -> Option<(&str, &str)> {
    let s = s.trim().trim_end_matches(';').trim();
    if s.is_empty() {
        return None;
    }
    let (key, value) = s.split_once(':')?;
    let key = key.trim();
    let value = value.trim();
    if !is_safe_css_property_name(key)
        || !css_value_is_safe(value, CssValuePolicy::MermaidSourceStyle)
    {
        return None;
    }
    Some((key, value))
}

pub(crate) fn is_safe_css_font_family_value(value: &str) -> bool {
    css_value_is_safe(value, CssValuePolicy::MermaidSourceStyle) && !value.contains(':')
}

pub(crate) fn is_resource_free_css_value(value: &str) -> bool {
    css_value_is_safe(value, CssValuePolicy::PortableTheme)
}

pub(crate) fn is_label_style_key(key: &str) -> bool {
    matches!(
        key.trim(),
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

pub(crate) fn parse_css_font_size_px(raw: &str, inherited_px: f64) -> Option<f64> {
    let raw = raw.trim().trim_end_matches(';').trim();
    if raw.is_empty() {
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    let inherited_px = inherited_px.max(1.0);

    if let Some(v) = lower.strip_suffix("px") {
        return parse_positive_f64(v);
    }
    if let Some(v) = lower.strip_suffix('%') {
        return parse_positive_f64(v).map(|pct| inherited_px * pct / 100.0);
    }
    if let Some(v) = lower.strip_suffix("rem") {
        return parse_positive_f64(v).map(|scale| inherited_px * scale);
    }
    if let Some(v) = lower.strip_suffix("em") {
        return parse_positive_f64(v).map(|scale| inherited_px * scale);
    }

    match lower.as_str() {
        "xx-small" => Some(inherited_px * 0.6),
        "x-small" => Some(inherited_px * 0.75),
        "small" => Some(inherited_px * 0.89),
        "medium" => Some(inherited_px),
        "large" => Some(inherited_px * 1.2),
        "x-large" => Some(inherited_px * 1.5),
        "xx-large" => Some(inherited_px * 2.0),
        "smaller" => Some(inherited_px * 0.8),
        "larger" => Some(inherited_px * 1.2),
        _ => parse_positive_f64(raw),
    }
}

fn parse_positive_f64(raw: &str) -> Option<f64> {
    let v = raw.trim().parse::<f64>().ok()?;
    (v.is_finite() && v > 0.0).then_some(v)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CssValuePolicy {
    MermaidSourceStyle,
    PortableTheme,
}

fn is_safe_css_property_name(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn css_value_is_safe(value: &str, policy: CssValuePolicy) -> bool {
    let value = value.trim();
    if value.is_empty()
        || value.contains("/*")
        || value.contains("*/")
        || value.chars().any(|character| {
            character.is_control() || matches!(character, '<' | '>' | '{' | '}' | ';')
        })
    {
        return false;
    }

    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    consume_safe_component_values(&mut parser, policy, 0).is_ok()
}

fn consume_safe_component_values<'i, 't>(
    parser: &mut Parser<'i, 't>,
    policy: CssValuePolicy,
    depth: u8,
) -> Result<(), ParseError<'i, ()>> {
    const MAX_NESTING: u8 = 32;

    loop {
        let token_start = parser.position();
        let token = match parser.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => return Ok(()),
            Err(error) => return Err(error.into()),
        };

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
            | Token::CDC => return Err(parser.new_custom_error(())),
            Token::Function(name) => {
                if function_is_forbidden(&name, policy) || depth >= MAX_NESTING {
                    return Err(parser.new_custom_error(()));
                }
                parser.parse_nested_block(|nested| {
                    consume_safe_component_values(nested, policy, depth + 1)
                })?;
                ensure_source_closed_block(parser, token_start, ')')?;
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
                    consume_safe_component_values(nested, policy, depth + 1)
                })?;
                ensure_source_closed_block(parser, token_start, close)?;
            }
            _ => {}
        }
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
