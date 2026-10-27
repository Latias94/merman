//! CSS font values shared by family measurement on the maintenance line.

use super::TextStyle;
use cssparser::{Delimiter, Parser, Token};
use std::collections::BTreeMap;

#[cfg(any(feature = "diagram-git-graph", feature = "diagram-timeline"))]
pub(crate) fn css_font_value_inherits(value: &str) -> bool {
    let mut parser = Parser::new(value);
    if !matches!(parser.next(), Ok(Token::Ident(value))
        if value.eq_ignore_ascii_case("inherit") || value.eq_ignore_ascii_case("unset"))
    {
        return false;
    }
    let _ = parser.try_parse(cssparser::parse_important);
    parser.expect_exhausted().is_ok()
}

pub(crate) fn is_static_css_font_family_list(value: &str) -> bool {
    if value
        .chars()
        .any(|ch| ch.is_control() || matches!(ch, '<' | '>'))
    {
        return false;
    }
    let mut parser = Parser::new(value);
    let mut families = 0;
    while !parser.is_exhausted() {
        if parser
            .try_parse(|input| input.expect_string_cloned())
            .is_err()
        {
            let Ok(first) = parser.expect_ident_cloned() else {
                return false;
            };
            if matches!(
                first.to_ascii_lowercase().as_str(),
                "inherit" | "initial" | "unset" | "revert" | "revert-layer"
            ) {
                return false;
            }
            while let Ok(component) = parser.try_parse(|input| input.expect_ident_cloned()) {
                if matches!(
                    component.to_ascii_lowercase().as_str(),
                    "inherit" | "initial" | "unset" | "revert" | "revert-layer"
                ) {
                    return false;
                }
            }
        }
        families += 1;
        if parser.is_exhausted() {
            break;
        }
        if parser.expect_comma().is_err() || parser.is_exhausted() {
            return false;
        }
    }
    families > 0
}

fn resolve_css_font_weight(value: &str, inherited: u16) -> Option<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(400),
        "bold" => Some(700),
        "bolder" => Some(match inherited {
            1..=349 => 400,
            350..=549 => 700,
            550..=899 => 900,
            _ => inherited,
        }),
        "lighter" => Some(match inherited {
            1..=99 => inherited,
            100..=549 => 100,
            550..=749 => 400,
            _ => 700,
        }),
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight)),
    }
}

pub(crate) fn apply_css_font_declarations(base: &TextStyle, css: &str) -> TextStyle {
    let mut parser = Parser::new(css);
    let mut winners: BTreeMap<String, (bool, Option<&str>)> = BTreeMap::new();
    while !parser.is_exhausted() {
        let declaration = parser.parse_until_after(Delimiter::Semicolon, |input| {
            let property = input.expect_ident_cloned()?.to_ascii_lowercase();
            input.expect_colon()?;
            let start = input.position();
            let mut end = start;
            let mut important = false;
            while !input.is_exhausted() {
                let before = input.position();
                if matches!(
                    input.next_including_whitespace_and_comments()?,
                    Token::Delim('!')
                ) {
                    input.expect_ident_matching("important")?;
                    input.expect_exhausted()?;
                    end = before;
                    important = true;
                    break;
                }
                end = input.position();
            }
            Ok::<_, cssparser::ParseError<()>>((
                property,
                input.slice(start..end).trim(),
                important,
            ))
        });
        let Ok((property, value, important)) = declaration else {
            continue;
        };
        let inherits = matches!(value.to_ascii_lowercase().as_str(), "inherit" | "unset");
        let valid = match property.as_str() {
            "font-family" => inherits || is_static_css_font_family_list(value),
            "font-size" => {
                inherits
                    || crate::mermaid_style::parse_css_font_size_px(value, base.font_size).is_some()
            }
            "font-style" => {
                inherits
                    || matches!(
                        value.to_ascii_lowercase().as_str(),
                        "normal" | "italic" | "oblique"
                    )
            }
            "font-weight" => {
                inherits
                    || resolve_css_font_weight(value, 400).is_some()
                    || value.parse::<f64>().ok().is_some_and(|weight| {
                        weight.is_finite() && (1.0..=1000.0).contains(&weight)
                    })
            }
            _ => false,
        };
        if valid
            && winners
                .get(&property)
                .is_none_or(|(current_important, _)| important || !*current_important)
        {
            winners.insert(property, (important, (!inherits).then_some(value)));
        }
    }
    let mut style = base.clone();
    for (property, (_, value)) in winners {
        let Some(value) = value else { continue };
        match property.as_str() {
            "font-family" => {
                style.font_family = Some(crate::config::normalize_css_font_family(value))
            }
            "font-size" => {
                if let Some(size) =
                    crate::mermaid_style::parse_css_font_size_px(value, base.font_size)
                {
                    style.font_size = size;
                }
            }
            "font-style" => style.font_style = Some(value.to_ascii_lowercase()),
            "font-weight" => {
                let inherited = base
                    .font_weight
                    .as_deref()
                    .and_then(|weight| resolve_css_font_weight(weight, 400))
                    .unwrap_or(400);
                style.font_weight = Some(
                    resolve_css_font_weight(value, inherited)
                        .map_or_else(|| value.to_owned(), |weight| weight.to_string()),
                );
            }
            _ => {}
        }
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_font_values_preserve_quoted_separators_and_declaration_priority() {
        let base = TextStyle {
            font_size: 24.0,
            font_weight: Some("700".to_owned()),
            ..TextStyle::default()
        };
        let style = apply_css_font_declarations(
            &base,
            r#"font-family:"a;b",serif;font-size:2em!important;font-size:3em;font-weight:lighter"#,
        );
        assert_eq!(style.font_family.as_deref(), Some(r#""a;b",serif"#));
        assert_eq!(style.font_size, 48.0);
        assert_eq!(style.font_weight.as_deref(), Some("400"));
        for malformed in ["", "serif,", "url(x)", "inherit junk", "serif; color:red"] {
            assert!(!is_static_css_font_family_list(malformed), "{malformed}");
        }
    }
}
