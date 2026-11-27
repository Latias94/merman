use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    config::{config_bool, value_at},
    text::{
        is_ecmascript_whitespace, trim_ecmascript_whitespace, trim_start_ecmascript_whitespace,
    },
};
use merman_theme_contract::v2::FamilyTheme;

mod native_theme;

/// Presentation settings shared by Railroad layout and SVG emission.
#[derive(Debug, Clone)]
pub struct RailroadStyle {
    pub(super) padding: f64,
    pub(super) vertical_separation: f64,
    pub(super) horizontal_separation: f64,
    pub(super) arc_radius: f64,
    pub(super) font_size: f64,
    pub(super) font_family: String,
    native_typography: Option<native_theme::RailroadNativeTypography>,
    pub(super) terminal_fill: String,
    pub(super) terminal_stroke: String,
    pub(super) terminal_text_color: String,
    pub(super) terminal_radius: f64,
    pub(super) non_terminal_fill: String,
    pub(super) non_terminal_stroke: String,
    pub(super) non_terminal_text_color: String,
    pub(super) line_color: String,
    pub(super) stroke_width: f64,
    pub(super) connector_width: Option<f64>,
    pub(super) marker_fill: String,
    pub(super) comment_fill: String,
    pub(super) comment_stroke: String,
    pub(super) comment_text_color: String,
    pub(super) special_fill: String,
    pub(super) special_stroke: String,
    pub(super) rule_name_color: String,
    pub(super) marker_radius: f64,
    pub(super) use_max_width: bool,
}

impl RailroadStyle {
    pub(super) fn terminal_font_family(&self) -> &str {
        self.native_typography
            .as_ref()
            .and_then(|typography| typography.terminal_font_family.as_deref())
            .unwrap_or(&self.font_family)
    }

    pub(super) fn rule_name_font_family(&self) -> &str {
        self.native_typography
            .as_ref()
            .and_then(|typography| typography.rule_name_font_family.as_deref())
            .unwrap_or(&self.font_family)
    }

    pub(super) fn rule_name_weight(&self) -> Option<u16> {
        self.native_typography
            .as_ref()
            .map(|typography| typography.title_weight)
    }
}

fn railroad_config_value<'a>(
    effective_config: &'a serde_json::Value,
    key: &str,
) -> Option<&'a serde_json::Value> {
    value_at(effective_config, &["railroad", key])
}

fn theme_config_value<'a>(
    effective_config: &'a serde_json::Value,
    primary_key: &str,
    secondary_key: Option<&str>,
) -> Option<&'a serde_json::Value> {
    let primary = value_at(effective_config, &["themeVariables", primary_key]);
    match primary {
        Some(value) if !value.is_null() => Some(value),
        _ => secondary_key.and_then(|key| value_at(effective_config, &["themeVariables", key])),
    }
}

fn sanitize_color_value(value: Option<&serde_json::Value>, fallback: &str) -> String {
    value
        .and_then(serde_json::Value::as_str)
        .map(trim_ecmascript_whitespace)
        .filter(|value| is_valid_color_value(value))
        .unwrap_or(fallback)
        .to_string()
}

fn is_valid_color_value(value: &str) -> bool {
    if let Some(hex) = value.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8)
            && hex.bytes().all(|byte| byte.is_ascii_hexdigit());
    }

    let Some(open) = value.find('(') else {
        return !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphabetic());
    };
    if !value.ends_with(')') {
        return false;
    }

    let function = &value[..open];
    let parameters = &value[open + 1..value.len() - 1];
    let valid_function = [
        "rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch", "oklab", "oklch",
    ]
    .iter()
    .any(|candidate| function.eq_ignore_ascii_case(candidate));

    valid_function
        && !parameters.is_empty()
        && parameters.chars().all(|character| {
            character.is_ascii_digit()
                || is_ecmascript_whitespace(character)
                || matches!(character, '%' | '+' | ',' | '.' | '/' | '-')
        })
}

fn sanitize_font_family_value(value: Option<&serde_json::Value>, fallback: &str) -> String {
    value
        .and_then(serde_json::Value::as_str)
        .map(trim_ecmascript_whitespace)
        .filter(|value| {
            !value.is_empty()
                && value.chars().all(|character| {
                    character.is_ascii_alphanumeric()
                        || character == '_'
                        || matches!(character, ' ' | '"' | '\'' | ',' | '.' | '-')
                })
        })
        .unwrap_or(fallback)
        .to_string()
}

fn sanitize_number_value(value: Option<&serde_json::Value>, fallback: f64) -> f64 {
    parse_number_value(value)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(fallback)
}

fn parse_theme_font_size(value: Option<&serde_json::Value>) -> Option<f64> {
    parse_number_value(value).filter(|value| value.is_finite() && *value > 0.0)
}

fn parse_number_value(value: Option<&serde_json::Value>) -> Option<f64> {
    match value? {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => parse_js_float_prefix(text),
        _ => None,
    }
}

fn parse_js_float_prefix(text: &str) -> Option<f64> {
    let text = trim_start_ecmascript_whitespace(text);
    let bytes = text.as_bytes();
    let mut index = 0;

    if matches!(bytes.first(), Some(b'+' | b'-')) {
        index += 1;
    }

    let integer_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    let mut has_digits = index > integer_start;

    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fraction_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        has_digits |= index > fraction_start;
    }

    if !has_digits {
        return None;
    }

    let mut end = index;
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        let mut exponent_index = index + 1;
        if matches!(bytes.get(exponent_index), Some(b'+' | b'-')) {
            exponent_index += 1;
        }
        let exponent_start = exponent_index;
        while bytes.get(exponent_index).is_some_and(u8::is_ascii_digit) {
            exponent_index += 1;
        }
        if exponent_index > exponent_start {
            end = exponent_index;
        }
    }

    text[..end].parse::<f64>().ok()
}

pub(super) fn resolve_railroad_style(
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
) -> RailroadStyle {
    let mut style = railroad_style(config.as_value());
    if let Some(theme) = native {
        native_theme::apply(&mut style, theme, config);
    }
    style
}

pub(super) fn railroad_style(effective_config: &serde_json::Value) -> RailroadStyle {
    let theme_font_family = sanitize_font_family_value(
        theme_config_value(effective_config, "fontFamily", None),
        "monospace",
    );
    let theme_font_size =
        parse_theme_font_size(theme_config_value(effective_config, "fontSize", None))
            .unwrap_or(14.0);
    let theme_terminal_fill = sanitize_color_value(
        theme_config_value(effective_config, "secondBkg", Some("secondaryColor")),
        "#FFFFC0",
    );
    let theme_terminal_stroke = sanitize_color_value(
        theme_config_value(effective_config, "secondaryBorderColor", Some("lineColor")),
        "#000000",
    );
    let theme_terminal_text_color = sanitize_color_value(
        theme_config_value(effective_config, "secondaryTextColor", Some("textColor")),
        "#000000",
    );
    let theme_non_terminal_fill = sanitize_color_value(
        theme_config_value(effective_config, "mainBkg", Some("background")),
        "#FFFFFF",
    );
    let theme_non_terminal_stroke = sanitize_color_value(
        theme_config_value(effective_config, "primaryBorderColor", Some("lineColor")),
        "#000000",
    );
    let theme_non_terminal_text_color = sanitize_color_value(
        theme_config_value(effective_config, "primaryTextColor", Some("textColor")),
        "#000000",
    );
    let theme_line_color = sanitize_color_value(
        theme_config_value(effective_config, "lineColor", None),
        "#000000",
    );
    let theme_comment_fill = sanitize_color_value(
        theme_config_value(effective_config, "labelBackground", Some("tertiaryColor")),
        "#E8E8E8",
    );
    let theme_comment_stroke = sanitize_color_value(
        theme_config_value(effective_config, "tertiaryBorderColor", Some("lineColor")),
        "#888888",
    );
    let theme_comment_text_color = sanitize_color_value(
        theme_config_value(effective_config, "tertiaryTextColor", Some("textColor")),
        "#666666",
    );
    let theme_special_fill = sanitize_color_value(
        theme_config_value(effective_config, "tertiaryColor", Some("secondaryColor")),
        "#F0E0FF",
    );
    let theme_special_stroke = sanitize_color_value(
        theme_config_value(
            effective_config,
            "tertiaryBorderColor",
            Some("secondaryBorderColor"),
        ),
        "#8800CC",
    );
    let theme_rule_name_color = sanitize_color_value(
        theme_config_value(effective_config, "titleColor", Some("textColor")),
        "#000066",
    );

    RailroadStyle {
        padding: sanitize_number_value(railroad_config_value(effective_config, "padding"), 10.0),
        vertical_separation: sanitize_number_value(
            railroad_config_value(effective_config, "verticalSeparation"),
            8.0,
        ),
        horizontal_separation: sanitize_number_value(
            railroad_config_value(effective_config, "horizontalSeparation"),
            10.0,
        ),
        arc_radius: sanitize_number_value(
            railroad_config_value(effective_config, "arcRadius"),
            10.0,
        ),
        font_size: sanitize_number_value(
            railroad_config_value(effective_config, "fontSize"),
            theme_font_size,
        ),
        font_family: sanitize_font_family_value(
            railroad_config_value(effective_config, "fontFamily"),
            &theme_font_family,
        ),
        native_typography: None,
        terminal_fill: sanitize_color_value(
            railroad_config_value(effective_config, "terminalFill"),
            &theme_terminal_fill,
        ),
        terminal_stroke: sanitize_color_value(
            railroad_config_value(effective_config, "terminalStroke"),
            &theme_terminal_stroke,
        ),
        terminal_text_color: sanitize_color_value(
            railroad_config_value(effective_config, "terminalTextColor"),
            &theme_terminal_text_color,
        ),
        terminal_radius: 10.0,
        non_terminal_fill: sanitize_color_value(
            railroad_config_value(effective_config, "nonTerminalFill"),
            &theme_non_terminal_fill,
        ),
        non_terminal_stroke: sanitize_color_value(
            railroad_config_value(effective_config, "nonTerminalStroke"),
            &theme_non_terminal_stroke,
        ),
        non_terminal_text_color: sanitize_color_value(
            railroad_config_value(effective_config, "nonTerminalTextColor"),
            &theme_non_terminal_text_color,
        ),
        line_color: sanitize_color_value(
            railroad_config_value(effective_config, "lineColor"),
            &theme_line_color,
        ),
        stroke_width: sanitize_number_value(
            railroad_config_value(effective_config, "strokeWidth"),
            2.0,
        ),
        connector_width: None,
        marker_fill: sanitize_color_value(
            railroad_config_value(effective_config, "markerFill"),
            &theme_line_color,
        ),
        comment_fill: sanitize_color_value(
            railroad_config_value(effective_config, "commentFill"),
            &theme_comment_fill,
        ),
        comment_stroke: sanitize_color_value(
            railroad_config_value(effective_config, "commentStroke"),
            &theme_comment_stroke,
        ),
        comment_text_color: sanitize_color_value(
            railroad_config_value(effective_config, "commentTextColor"),
            &theme_comment_text_color,
        ),
        special_fill: sanitize_color_value(
            railroad_config_value(effective_config, "specialFill"),
            &theme_special_fill,
        ),
        special_stroke: sanitize_color_value(
            railroad_config_value(effective_config, "specialStroke"),
            &theme_special_stroke,
        ),
        rule_name_color: sanitize_color_value(
            railroad_config_value(effective_config, "ruleNameColor"),
            &theme_rule_name_color,
        ),
        marker_radius: sanitize_number_value(
            railroad_config_value(effective_config, "markerRadius"),
            5.0,
        ),
        use_max_width: config_bool(effective_config, &["railroad", "useMaxWidth"]).unwrap_or(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn railroad_style_matches_upstream_css_value_whitelists() {
        for color in [
            "#abc",
            "#abcd",
            "#abcdef",
            "#abcdef12",
            "rgb(10 20 30 / 50%)",
            "hsl(120, 40%, 50%)",
            "oklch(70% 0.1 200)",
            "currentColor",
        ] {
            assert!(is_valid_color_value(color), "expected valid color: {color}");
        }
        for color in [
            "#abcde",
            "var(--railroad-color)",
            "url(javascript:alert(1))",
            "#fff;stroke:red",
            "red}",
        ] {
            assert!(
                !is_valid_color_value(color),
                "expected invalid color: {color}"
            );
        }

        let valid_font = serde_json::json!("\"Fira Code\", monospace");
        let invalid_font = serde_json::json!("safe\"} body { display: none; } /*");
        assert_eq!(
            sanitize_font_family_value(Some(&valid_font), "fallback"),
            "\"Fira Code\", monospace"
        );
        assert_eq!(
            sanitize_font_family_value(Some(&invalid_font), "fallback"),
            "fallback"
        );
    }

    #[test]
    fn railroad_style_numbers_follow_number_parse_float_semantics() {
        for (input, expected) in [
            ("12px", 12.0),
            ("1e2junk", 100.0),
            ("0x10", 0.0),
            (".5rem", 0.5),
        ] {
            assert_eq!(parse_js_float_prefix(input), Some(expected), "{input}");
        }
        for input in ["", "NaN", "Infinity", "-Infinity"] {
            assert_eq!(
                sanitize_number_value(Some(&serde_json::json!(input)), 7.0),
                7.0,
                "{input}"
            );
        }
        assert_eq!(
            sanitize_number_value(Some(&serde_json::json!("-1px")), 7.0),
            7.0
        );
        assert_eq!(sanitize_number_value(Some(&serde_json::json!(0)), 7.0), 0.0);
        assert_eq!(parse_theme_font_size(Some(&serde_json::json!(0))), None);
    }

    #[test]
    fn railroad_style_uses_ecmascript_whitespace_semantics() {
        let bom_color = serde_json::json!("\u{FEFF}rgb(1\u{FEFF}2 3)\u{FEFF}");
        let nel_color = serde_json::json!("rgb(1\u{0085}2 3)");
        assert_eq!(
            sanitize_color_value(Some(&bom_color), "fallback"),
            "rgb(1\u{FEFF}2 3)"
        );
        assert_eq!(
            sanitize_color_value(Some(&nel_color), "fallback"),
            "fallback"
        );
        assert_eq!(parse_js_float_prefix("\u{FEFF}18px"), Some(18.0));
        assert_eq!(parse_js_float_prefix("\u{0085}18px"), None);
    }

    #[test]
    fn invalid_primary_theme_color_uses_hard_fallback() {
        let style = railroad_style(&serde_json::json!({
            "themeVariables": {
                "secondBkg": "#fff; stroke: red",
                "secondaryColor": "#123456"
            }
        }));

        assert_eq!(style.terminal_fill, "#FFFFC0");
    }
}
