use serde_json::Value;

pub(crate) const MERMAID_DEFAULT_FONT_FAMILY_CSS: &str =
    r#""trebuchet ms",verdana,arial,sans-serif"#;
const MERMAID_DEFAULT_FONT_FAMILY_CONFIG: &str = r#""trebuchet ms", verdana, arial, sans-serif"#;
const MERMAID_DEFAULT_ROOT_FONT_FAMILY_CONFIG: &str =
    r#""trebuchet ms", verdana, arial, sans-serif;"#;
pub(crate) const DEFAULT_DIAGRAM_LOOK: &str = "classic";

pub(crate) fn value_at<'a>(cfg: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    Some(cur)
}

pub(crate) fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    value_at(cfg, path).and_then(|v| v.as_str().map(str::to_string))
}

/// Selects a string and its actual origin together; empty strings remain CSS tokens.
pub(crate) fn selected_config_string<'a>(
    cfg: &'a Value,
    paths: &[&'static str],
) -> Option<(&'static str, &'a str)> {
    paths.iter().find_map(|&path| {
        let value = path.split('.').try_fold(cfg, |value, key| value.get(key))?;
        Some((path, value.as_str()?))
    })
}

pub(crate) fn json_string_or_first_array(value: &Value) -> Option<String> {
    value.as_str().map(str::to_string).or_else(|| {
        value
            .as_array()
            .and_then(|values| values.first()?.as_str())
            .map(str::to_string)
    })
}

pub(crate) fn config_string_or_first_array(cfg: &Value, path: &[&str]) -> Option<String> {
    value_at(cfg, path).and_then(json_string_or_first_array)
}

pub(crate) fn json_string_vec(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn config_string_vec(cfg: &Value, path: &[&str]) -> Vec<String> {
    value_at(cfg, path).map_or_else(Vec::new, json_string_vec)
}

pub(crate) fn config_bool(cfg: &Value, path: &[&str]) -> Option<bool> {
    value_at(cfg, path).and_then(Value::as_bool)
}

pub(crate) fn json_bool(v: &Value) -> Option<bool> {
    v.as_bool()
        .or_else(|| v.as_i64().map(|n| n != 0))
        .or_else(|| v.as_u64().map(|n| n != 0))
        .or_else(|| {
            v.as_str()
                .and_then(|s| match s.trim().to_ascii_lowercase().as_str() {
                    "true" | "yes" | "on" | "1" => Some(true),
                    "false" | "no" | "off" | "0" => Some(false),
                    _ => None,
                })
        })
}

// Theme overrides retain their JSON types; source drawing guards use JavaScript truthiness.
pub(crate) fn json_value_is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

pub(crate) fn config_effective_html_labels(cfg: &Value) -> bool {
    config_bool(cfg, &["htmlLabels"])
        .or_else(|| config_bool(cfg, &["flowchart", "htmlLabels"]))
        .unwrap_or(true)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct DiagramLook<'a> {
    /// The effective look used by layout and renderer behavior.
    value: &'a str,
    /// The allow-listed token preserved when a renderer serializes `data-look`.
    ///
    /// Mermaid's State v2 fixtures can contain the historical `default` token. It behaves like
    /// `classic`, but the token is still observable in the generated DOM. Keep that compatibility
    /// spelling separate from the effective behavior so accepting it does not open an arbitrary
    /// selector-valued input path.
    serialized_value: &'a str,
}

impl<'a> DiagramLook<'a> {
    pub(crate) fn from_raw(raw: Option<&'a str>) -> Self {
        let serialized_value = raw
            .map(str::trim)
            .filter(|look| matches!(*look, "classic" | "handDrawn" | "neo" | "default"))
            .unwrap_or(DEFAULT_DIAGRAM_LOOK);
        let value = match serialized_value {
            "default" => DEFAULT_DIAGRAM_LOOK,
            value => value,
        };
        Self {
            value,
            serialized_value,
        }
    }

    pub(crate) fn into_static(self) -> DiagramLook<'static> {
        let serialized_value = match self.serialized_value {
            "handDrawn" => "handDrawn",
            "neo" => "neo",
            "default" => "default",
            _ => DEFAULT_DIAGRAM_LOOK,
        };
        DiagramLook::from_raw(Some(serialized_value))
    }

    pub(crate) fn as_str(&self) -> &'a str {
        self.value
    }

    pub(crate) fn serialized(&self) -> &'a str {
        self.serialized_value
    }

    pub(crate) fn is_neo(&self) -> bool {
        self.value == "neo"
    }

    pub(crate) fn is_hand_drawn(&self) -> bool {
        self.value == "handDrawn"
    }
}

impl std::fmt::Display for DiagramLook<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.value)
    }
}

pub(crate) fn config_diagram_look(cfg: &Value) -> DiagramLook<'_> {
    DiagramLook::from_raw(value_at(cfg, &["look"]).and_then(Value::as_str))
}

pub(crate) fn mermaid_config_diagram_look(
    cfg: &merman_model::presentation::PresentationConfig,
) -> DiagramLook<'_> {
    DiagramLook::from_raw(cfg.get_str("look"))
}

pub(crate) fn normalize_css_font_family(font_family: &str) -> String {
    let Some(parsed) = crate::mermaid_style::parse_css_font_family_value(font_family) else {
        return String::new();
    };
    let s = parsed.value;

    // Mermaid serializes generated CSS with comma-separated font-family lists and no extra
    // whitespace around commas. Keep that spelling stable for SVG parity.
    let mut parts = Vec::new();
    let mut parser = cssparser::Parser::new(s);
    let mut start = 0;
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(cssparser::Token::Comma) => {
                // Advancing past a function skips its block before returning this comma token.
                // Use the returned token's end, rather than the position before that advance.
                let end = parser.position().byte_index();
                let part = s[start..end - 1].trim();
                if !part.is_empty() {
                    parts.push(part);
                }
                start = end;
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let part = s[start..parser.position().byte_index()].trim();
    if !part.is_empty() {
        parts.push(part);
    }

    let mut normalized = parts.join(",");
    if crate::mermaid_style::is_safe_css_font_family_value(&normalized) {
        if parsed.important {
            normalized.push_str(" !important");
        }
        normalized
    } else {
        String::new()
    }
}

pub(crate) fn configured_font_family(cfg: &Value) -> Option<(String, &'static str)> {
    configured_font_family_with(cfg, Value::as_str)
}

pub(crate) fn configured_font_family_or_first_array(cfg: &Value) -> Option<(String, &'static str)> {
    configured_font_family_with(cfg, json_str_or_first_array_ref)
}

fn configured_font_family_with(
    cfg: &Value,
    read: fn(&Value) -> Option<&str>,
) -> Option<(String, &'static str)> {
    value_at(cfg, &["themeVariables", "fontFamily"])
        .and_then(read)
        .and_then(normalized_config_font_family)
        .map(|font| (font, "themeVariables.fontFamily"))
        .or_else(|| {
            value_at(cfg, &["fontFamily"])
                .and_then(read)
                .and_then(normalized_config_font_family)
                .map(|font| (font, "fontFamily"))
        })
}

pub(crate) fn config_font_family_css(cfg: &Value) -> String {
    configured_font_family(cfg)
        .map(|(font, _)| font)
        .unwrap_or_else(|| MERMAID_DEFAULT_FONT_FAMILY_CSS.to_owned())
}

/// Mermaid's generated :root custom property reads config.fontFamily, independently
/// of the theme font used by diagram styles (mermaidAPI.createCssStyles).
pub(crate) fn config_root_font_family_css(cfg: &Value) -> String {
    let Some(font_family) = value_at(cfg, &["fontFamily"]).and_then(Value::as_str) else {
        return MERMAID_DEFAULT_FONT_FAMILY_CSS.to_string();
    };
    // An explicitly blank value produces no declaration in the browser CSSOM.
    if font_family.trim().is_empty() {
        return String::new();
    }
    normalized_config_font_family(font_family)
        .unwrap_or_else(|| MERMAID_DEFAULT_FONT_FAMILY_CSS.to_owned())
}

pub(crate) fn config_font_family_or_first_array_css(cfg: &Value) -> String {
    configured_font_family_or_first_array(cfg)
        .map(|(font, _)| font)
        .unwrap_or_else(|| MERMAID_DEFAULT_FONT_FAMILY_CSS.to_owned())
}

fn json_str_or_first_array_ref(value: &Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.as_array().and_then(|values| values.first()?.as_str()))
}

pub(crate) fn normalized_config_font_family(font_family: &str) -> Option<String> {
    if matches!(
        font_family,
        MERMAID_DEFAULT_FONT_FAMILY_CONFIG | MERMAID_DEFAULT_ROOT_FONT_FAMILY_CONFIG
    ) {
        return Some(MERMAID_DEFAULT_FONT_FAMILY_CSS.to_owned());
    }

    let font_family = normalize_css_font_family(font_family);
    (!font_family.is_empty()).then_some(font_family)
}

pub(crate) fn config_theme_or_root_font_size_px_opt(cfg: &Value) -> Option<f64> {
    configured_theme_or_root_font_size_px(cfg).map(|(size, _)| size)
}

/// Selects a valid nonnegative font size before assigning configuration ownership.
pub(crate) fn configured_theme_or_root_font_size_px(cfg: &Value) -> Option<(f64, &'static str)> {
    config_f64_css_absolute_px(cfg, &["themeVariables", "fontSize"])
        .filter(|size| size.is_finite() && *size >= 0.0)
        .map(|size| (size, "themeVariables.fontSize"))
        .or_else(|| {
            config_f64_css_px(cfg, &["fontSize"])
                .filter(|size| size.is_finite() && *size >= 0.0)
                .map(|size| (size, "fontSize"))
        })
}

pub(crate) fn config_theme_or_root_font_size_px(cfg: &Value, default: f64) -> f64 {
    config_theme_or_root_font_size_px_opt(cfg).unwrap_or(default)
}

pub(crate) fn config_theme_font_size_css_or_root_number_px_opt(cfg: &Value) -> Option<f64> {
    configured_theme_font_size_css_or_root_number_px(cfg).map(|(size, _)| size)
}

pub(crate) fn configured_theme_font_size_css_or_root_number_px(
    cfg: &Value,
) -> Option<(f64, &'static str)> {
    config_f64_css_absolute_px(cfg, &["themeVariables", "fontSize"])
        .filter(|size| size.is_finite() && *size >= 0.0)
        .map(|size| (size, "themeVariables.fontSize"))
        .or_else(|| {
            config_f64(cfg, &["fontSize"])
                .filter(|size| size.is_finite() && *size >= 0.0)
                .map(|size| (size, "fontSize"))
        })
}

pub(crate) fn config_theme_font_size_css_or_root_number_px(cfg: &Value, default: f64) -> f64 {
    config_theme_font_size_css_or_root_number_px_opt(cfg).unwrap_or(default)
}

pub(crate) fn json_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|n| n as f64))
        .or_else(|| value.as_u64().map(|n| n as f64))
        .or_else(|| {
            let n = value.as_str()?.trim().parse::<f64>().ok()?;
            n.is_finite().then_some(n)
        })
}

pub(crate) fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    value_at(cfg, path).and_then(json_f64)
}

pub(crate) fn config_f64_or(cfg: &Value, path: &[&str], default: f64) -> f64 {
    config_f64(cfg, path).unwrap_or(default)
}

/// Mermaid `parseFontSize` preserves numeric fractions and reads an integer prefix from strings.
/// This is a compatibility number, not a conversion from CSS lengths to pixels.
pub(crate) fn json_mermaid_font_size_number(value: &Value) -> Option<f64> {
    if let Some(number) = value.as_f64() {
        return Some(number);
    }
    let text = crate::text::trim_start_ecmascript_whitespace(value.as_str()?);
    let digits = text
        .strip_prefix('-')
        .or_else(|| text.strip_prefix('+'))
        .unwrap_or(text);
    let count = digits.bytes().take_while(u8::is_ascii_digit).count();
    if count == 0 {
        return None;
    }
    text[..text.len() - digits.len() + count]
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

pub(crate) fn json_f64_css_px(value: &Value) -> Option<f64> {
    json_f64(value).or_else(|| value.as_str().and_then(parse_css_px_to_f64))
}

pub(crate) fn config_f64_css_px(cfg: &Value, path: &[&str]) -> Option<f64> {
    value_at(cfg, path).and_then(json_f64_css_px)
}

/// Extends the legacy numeric/px configuration subset with context-free CSS length units.
/// Literal zero lengths need no host context. Other relative lengths, keywords, and expressions
/// retain their existing family-specific owners.
pub(crate) fn config_f64_css_absolute_px(cfg: &Value, path: &[&str]) -> Option<f64> {
    value_at(cfg, path).and_then(|value| {
        json_f64_css_px(value)
            .filter(|number| {
                *number != 0.0
                    || value.as_str().is_none_or(|raw| {
                        // The legacy parser already admitted this spelling. Inspect its
                        // coefficient without imposing stricter CSS syntax on genuine zeros.
                        !raw.trim()
                            .bytes()
                            .take_while(|byte| {
                                byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.')
                            })
                            .any(|byte| byte.is_ascii_digit() && byte != b'0')
                    })
            })
            .or_else(|| {
                let css = value.as_str()?;
                crate::mermaid_style::resolve_mermaid_absolute_font_size_px(css)
                    .or_else(|| crate::mermaid_style::css_font_size_is_zero(css).then_some(0.0))
            })
    })
}

pub(crate) fn json_f64_explicit_css_absolute_px(value: &Value) -> Option<f64> {
    crate::mermaid_style::resolve_explicit_css_absolute_font_size_px(value.as_str()?)
}

pub(crate) fn config_f64_explicit_css_absolute_px(cfg: &Value, path: &[&str]) -> Option<f64> {
    value_at(cfg, path).and_then(json_f64_explicit_css_absolute_px)
}

pub(crate) fn json_css_number_or_string(value: &Value) -> Option<String> {
    if let Some(raw) = value.as_str() {
        let text = normalize_css_raw_value_text(raw);
        return (!text.is_empty()).then(|| text.to_string());
    }

    let value = json_f64(value)?;
    value.is_finite().then(|| {
        let text = value.to_string();
        if text == "-0" { "0".to_string() } else { text }
    })
}

pub(crate) fn config_css_number_or_string(cfg: &Value, path: &[&str]) -> Option<String> {
    value_at(cfg, path).and_then(json_css_number_or_string)
}

fn parse_css_px_to_f64(text: &str) -> Option<f64> {
    let text = normalize_css_value_text(text);
    let text = text.strip_suffix("px").unwrap_or(text).trim();
    let value = text.parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}

fn normalize_css_value_text(text: &str) -> &str {
    let text = text.trim().trim_end_matches(';').trim();
    text.trim_end_matches("!important").trim()
}

fn normalize_css_raw_value_text(text: &str) -> &str {
    text.trim().trim_end_matches(';').trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_f64_accepts_json_numbers_and_plain_numeric_strings() {
        assert_eq!(json_f64(&json!(100)), Some(100.0));
        assert_eq!(json_f64(&json!(70.5)), Some(70.5));
        assert_eq!(json_f64(&json!("100")), Some(100.0));
        assert_eq!(json_f64(&json!(" 70.5 ")), Some(70.5));
    }

    #[test]
    fn json_f64_rejects_css_or_nonfinite_strings() {
        assert_eq!(json_f64(&json!("100px")), None);
        assert_eq!(json_f64(&json!("NaN")), None);
        assert_eq!(json_f64(&json!("inf")), None);
    }

    #[test]
    fn config_f64_walks_paths_and_accepts_yaml_string_numbers() {
        let cfg = json!({
            "flowchart": {
                "rankSpacing": "100",
                "nodeSpacing": "70.5"
            }
        });

        assert_eq!(config_f64(&cfg, &["flowchart", "rankSpacing"]), Some(100.0));
        assert_eq!(config_f64(&cfg, &["flowchart", "nodeSpacing"]), Some(70.5));
        assert_eq!(config_f64(&cfg, &["missing", "rankSpacing"]), None);
    }

    #[test]
    fn config_string_or_first_array_accepts_string_and_array_first_item() {
        let cfg = json!({
            "themeVariables": {
                "fontFamily": ["Courier", "Ignored"],
                "textColor": "#333"
            }
        });

        assert_eq!(
            config_string_or_first_array(&cfg, &["themeVariables", "fontFamily"]),
            Some("Courier".to_string())
        );
        assert_eq!(
            config_string_or_first_array(&cfg, &["themeVariables", "textColor"]),
            Some("#333".to_string())
        );
        assert_eq!(
            config_string_or_first_array(&cfg, &["themeVariables", "missing"]),
            None
        );
    }

    #[test]
    fn config_string_vec_accepts_string_arrays_and_ignores_non_arrays() {
        let cfg = json!({
            "journey": {
                "actorColours": ["#111", 2, "#333"],
                "sectionFills": "not-an-array"
            }
        });

        assert_eq!(
            config_string_vec(&cfg, &["journey", "actorColours"]),
            vec!["#111".to_string(), "#333".to_string()]
        );
        assert_eq!(
            config_string_vec(&cfg, &["journey", "sectionFills"]),
            Vec::<String>::new()
        );
        assert_eq!(
            config_string_vec(&cfg, &["journey", "missing"]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn config_bool_accepts_only_json_bool() {
        let cfg = json!({
            "a": true,
            "b": "true"
        });

        assert_eq!(config_bool(&cfg, &["a"]), Some(true));
        assert_eq!(config_bool(&cfg, &["b"]), None);
    }

    #[test]
    fn config_diagram_look_trims_and_defaults_to_classic() {
        assert_eq!(
            config_diagram_look(&json!({ "look": " neo " })).as_str(),
            "neo"
        );
        assert!(config_diagram_look(&json!({ "look": "neo" })).is_neo());
        assert_eq!(
            config_diagram_look(&json!({ "look": "handDrawn" })).as_str(),
            "handDrawn"
        );
        assert_eq!(
            config_diagram_look(&json!({})).as_str(),
            DEFAULT_DIAGRAM_LOOK
        );
        assert_eq!(
            config_diagram_look(&json!({ "look": "" })).as_str(),
            DEFAULT_DIAGRAM_LOOK
        );
        assert_eq!(
            config_diagram_look(&json!({ "look": "neo\"]{color:red}" })).as_str(),
            DEFAULT_DIAGRAM_LOOK
        );
        let default_cfg = json!({ "look": " default " });
        let default = config_diagram_look(&default_cfg);
        assert_eq!(default.as_str(), DEFAULT_DIAGRAM_LOOK);
        assert_eq!(default.serialized(), "default");
        assert_eq!(default.into_static().serialized(), "default");
        assert_eq!(default.into_static().as_str(), DEFAULT_DIAGRAM_LOOK);
        let missing_cfg = json!({});
        let missing = config_diagram_look(&missing_cfg);
        assert_eq!(missing.serialized(), DEFAULT_DIAGRAM_LOOK);
    }

    #[test]
    fn json_f64_css_px_accepts_plain_and_css_numeric_strings() {
        assert_eq!(json_f64_css_px(&json!("24")), Some(24.0));
        assert_eq!(json_f64_css_px(&json!("24px")), Some(24.0));
        assert_eq!(json_f64_css_px(&json!("24px !important;")), Some(24.0));
        assert_eq!(json_f64_css_px(&json!("24pt")), None);
        assert_eq!(json_f64_css_px(&json!("NaNpx")), None);
    }

    #[test]
    fn configured_theme_font_sizes_resolve_absolute_css_lengths() {
        for (value, expected) in [
            ("24pt", 32.0),
            ("2pc", 32.0),
            ("1in", 96.0),
            ("2.54cm", 96.0),
            ("25.4mm", 96.0),
            ("101.6Q", 96.0),
            (r"24p\74", 32.0),
        ] {
            let config = json!({"themeVariables": {"fontSize": value}});
            let (size, owner) = configured_theme_or_root_font_size_px(&config).unwrap();
            assert!((size - expected).abs() < 1e-9, "{value}: {size}");
            assert_eq!(owner, "themeVariables.fontSize");
            assert!(
                (config_theme_font_size_css_or_root_number_px_opt(&config).unwrap() - expected)
                    .abs()
                    < 1e-9,
                "{value}"
            );
        }
        for value in ["1rem", "125%", "large", "calc(1px + 2px)", "24pt trailing"] {
            assert_eq!(
                config_f64_css_absolute_px(&json!({"size": value}), &["size"]),
                None,
                "{value}"
            );
        }
        assert_eq!(
            config_f64_css_absolute_px(&json!({"size": "-2px"}), &["size"]),
            Some(-2.0),
            "legacy numeric/px clamping remains owned by the consuming family"
        );
    }

    #[test]
    fn configured_theme_font_sizes_preserve_context_free_relative_zero() {
        for value in [
            "0em",
            "0rem",
            "0%",
            "-0ex",
            "0vw",
            r"0e\6d",
            "0em !important",
        ] {
            let config = json!({"fontSize": 29, "themeVariables": {"fontSize": value}});
            assert_eq!(
                configured_theme_or_root_font_size_px(&config),
                Some((0.0, "themeVariables.fontSize")),
                "{value}"
            );
            assert_eq!(
                configured_theme_font_size_css_or_root_number_px(&config),
                Some((0.0, "themeVariables.fontSize")),
                "{value}"
            );
        }
        for value in [
            "1em",
            "1rem",
            "1e-999em",
            "1e-999px",
            "-1e-999px",
            "1e-999",
            "calc(0px)",
            "var(--size, 0px)",
            "0unknown",
        ] {
            assert_eq!(
                config_f64_css_absolute_px(&json!({"size": value}), &["size"]),
                None,
                "{value} must retain its family-local context"
            );
        }
        for value in [
            json!(0),
            json!("0e-999px"),
            json!("-0e-999px"),
            json!("0e-999"),
            json!("0."),
            json!("0px;;"),
        ] {
            assert_eq!(
                config_f64_css_absolute_px(&json!({"size":value}), &["size"]),
                Some(0.0)
            );
            assert_eq!(
                configured_theme_or_root_font_size_px(
                    &json!({"themeVariables":{"fontSize":value},"fontSize":29})
                ),
                Some((0.0, "themeVariables.fontSize"))
            );
        }
    }

    #[test]
    fn json_f64_explicit_css_absolute_px_accepts_only_absolute_length_strings() {
        assert_eq!(json_f64_explicit_css_absolute_px(&json!(24)), None);
        assert_eq!(json_f64_explicit_css_absolute_px(&json!("24")), None);
        assert_eq!(
            json_f64_explicit_css_absolute_px(&json!("24px")),
            Some(24.0)
        );
        assert_eq!(
            json_f64_explicit_css_absolute_px(&json!("24px !important;")),
            Some(24.0)
        );
        assert_eq!(
            json_f64_explicit_css_absolute_px(&json!("24pt")),
            Some(32.0)
        );
        assert_eq!(json_f64_explicit_css_absolute_px(&json!("1em")), None);
        assert_eq!(json_f64_explicit_css_absolute_px(&json!("medium")), None);
        for raw in [
            "32 !IMPORTANT",
            "32 ! important",
            r"32 !\69mportant",
            "-2px",
        ] {
            assert_eq!(
                json_f64_explicit_css_absolute_px(&json!(raw)),
                None,
                "{raw}"
            );
        }
        for raw in [
            "24pt !IMPORTANT",
            "24pt ! important",
            r"24p\74 !\69mportant",
        ] {
            assert_eq!(
                json_f64_explicit_css_absolute_px(&json!(raw)),
                Some(32.0),
                "{raw}"
            );
        }
    }

    #[test]
    fn json_css_number_or_string_keeps_mermaid_style_interpolation_spelling() {
        assert_eq!(
            json_css_number_or_string(&json!(24)),
            Some("24".to_string())
        );
        assert_eq!(
            json_css_number_or_string(&json!(24.5)),
            Some("24.5".to_string())
        );
        assert_eq!(
            json_css_number_or_string(&json!(" 24px; ")),
            Some("24px".to_string())
        );
        assert_eq!(
            json_css_number_or_string(&json!("24px !important;")),
            Some("24px !important".to_string())
        );
        assert_eq!(json_css_number_or_string(&json!(true)), None);
    }

    #[test]
    fn normalize_css_font_family_matches_mermaid_spacing() {
        assert_eq!(
            normalize_css_font_family(r#" "trebuchet ms", verdana, arial, sans-serif; "#),
            r#""trebuchet ms",verdana,arial,sans-serif"#
        );
        assert_eq!(
            normalize_css_font_family(r#"'Open Sans', "IBM Plex Sans", sans-serif"#),
            r#"'Open Sans',"IBM Plex Sans",sans-serif"#
        );
    }

    #[test]
    fn normalize_css_font_family_preserves_quoted_metacharacters_and_escaped_commas() {
        for (authored, expected) in [
            (r#""A<B", monospace"#, r#""A<B",monospace"#),
            (r#""A>B", serif"#, r#""A>B",serif"#),
            (r#""A&B", monospace"#, r#""A&B",monospace"#),
            (r#""A: B", serif"#, r#""A: B",serif"#),
            (r#""A&amp;B", serif"#, r#""A&amp;B",serif"#),
            (
                r#""Comma, \"Quoted, Font\" Glyph", monospace"#,
                r#""Comma, \"Quoted, Font\" Glyph",monospace"#,
            ),
            (r"Open\, Sans, monospace", r"Open\, Sans,monospace"),
            (r#""字體 Ω", sans-serif"#, r#""字體 Ω",sans-serif"#),
            (
                "var(--font, serif), monospace",
                "var(--font, serif),monospace",
            ),
            (
                "env(host-font, monospace), serif",
                "env(host-font, monospace),serif",
            ),
        ] {
            assert_eq!(normalize_css_font_family(authored), expected, "{authored}");
        }
    }

    #[test]
    fn config_font_family_css_uses_theme_then_legacy_then_default() {
        assert_eq!(
            config_font_family_css(&json!({
                "fontFamily": "Courier, monospace",
                "themeVariables": {
                    "fontFamily": "\"IBM Plex Sans\", Arial, sans-serif"
                }
            })),
            r#""IBM Plex Sans",Arial,sans-serif"#
        );
        assert_eq!(
            config_font_family_css(&json!({
                "fontFamily": "Courier, monospace"
            })),
            "Courier,monospace"
        );
        assert_eq!(
            config_font_family_css(&json!({
                "themeVariables": {
                    "fontFamily": " ; "
                }
            })),
            MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
    }

    #[test]
    fn configured_font_family_skips_css_rejected_candidates_before_normalizing() {
        for invalid in [
            "24px",
            "serif,,monospace",
            ",serif",
            "serif,",
            "default",
            "serif inherit",
            "var(--font),,serif",
            "var(--font),",
            ",var(--font)",
            "serif !important !important",
            "serif !important)",
            "",
            "   ",
        ] {
            let config = json!({
                "fontFamily": "Cousine",
                "themeVariables": {"fontFamily": invalid}
            });
            assert_eq!(
                configured_font_family(&config),
                Some(("Cousine".to_owned(), "fontFamily")),
                "{invalid:?} must not hide the accepted legacy candidate"
            );
            assert_eq!(
                configured_font_family(&json!({"themeVariables": {"fontFamily": invalid}})),
                None,
                "{invalid:?} must not claim an initialized or typed font"
            );
            assert_eq!(
                configured_font_family_or_first_array(&json!({
                    "fontFamily": ["Cousine"],
                    "themeVariables": {"fontFamily": [invalid]}
                })),
                Some(("Cousine".to_owned(), "fontFamily")),
                "array candidate {invalid:?}"
            );
        }
    }

    #[test]
    fn configured_font_family_retains_literal_names_and_host_values() {
        for (authored, expected) in [
            (r#""", serif"#, r#""",serif"#),
            (r#""A<B", serif"#, r#""A<B",serif"#),
            (r#""A: B", serif"#, r#""A: B",serif"#),
            (r#""A;B", serif"#, r#""A;B",serif"#),
            (r#""A<B", serif !important"#, r#""A<B",serif !important"#),
            (r#""A: B", serif !important"#, r#""A: B",serif !important"#),
            (r#""A;B", serif !important"#, r#""A;B",serif !important"#),
            (r"Open\, Sans, monospace", r"Open\, Sans,monospace"),
            (r"Open\;", r"Open\;"),
            (r"Open\!important", r"Open\!important"),
            ("Cousine !important", "Cousine !important"),
            ("Cousine ! IMPORTANT;;", "Cousine !important"),
            ("inherit", "inherit"),
            ("initial", "initial"),
            ("var(--font, serif)", "var(--font, serif)"),
            ("var(--font)!important", "var(--font) !important"),
            ("var(--font);", "var(--font)"),
            ("env(host-font)!important", "env(host-font) !important"),
            ("var(--font) !important", "var(--font) !important"),
            ("env(host-font, serif)", "env(host-font, serif)"),
            ("future-host-font(--font)", "future-host-font(--font)"),
        ] {
            assert_eq!(
                configured_font_family(&json!({"themeVariables": {"fontFamily": authored}})),
                Some((expected.to_owned(), "themeVariables.fontFamily")),
                "{authored}"
            );
        }
    }

    #[test]
    fn config_font_family_helpers_accept_exact_builtin_default_spellings() {
        assert_eq!(
            config_font_family_css(&json!({
                "themeVariables": { "fontFamily": MERMAID_DEFAULT_FONT_FAMILY_CONFIG }
            })),
            MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
        assert_eq!(
            config_font_family_css(&json!({
                "fontFamily": MERMAID_DEFAULT_ROOT_FONT_FAMILY_CONFIG
            })),
            MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
        assert_eq!(
            config_root_font_family_css(&json!({
                "fontFamily": MERMAID_DEFAULT_ROOT_FONT_FAMILY_CONFIG
            })),
            MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
        assert_eq!(
            config_font_family_or_first_array_css(&json!({
                "themeVariables": {
                    "fontFamily": [MERMAID_DEFAULT_FONT_FAMILY_CONFIG]
                }
            })),
            MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
        assert_eq!(
            config_font_family_css(&json!({
                "themeVariables": { "fontFamily": "serif; color: red" }
            })),
            MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
    }

    #[test]
    fn root_font_family_is_independent_of_the_theme_font() {
        for empty in ["", "   "] {
            assert_eq!(
                config_root_font_family_css(&json!({"fontFamily": empty})),
                ""
            );
        }
        assert_eq!(
            config_root_font_family_css(&json!({
                "fontFamily": "Courier, monospace",
                "themeVariables": { "fontFamily": "Arial, sans-serif" }
            })),
            "Courier,monospace"
        );
        assert_eq!(
            config_root_font_family_css(&json!({
                "themeVariables": { "fontFamily": "Arial, sans-serif" }
            })),
            MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
        assert_eq!(
            config_root_font_family_css(&json!({
                "fontFamily": "serif; color: red"
            })),
            MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
    }

    #[test]
    fn config_font_family_or_first_array_css_uses_theme_then_legacy_then_default() {
        assert_eq!(
            config_font_family_or_first_array_css(&json!({
                "fontFamily": ["Courier, monospace", "Ignored Sans"],
                "themeVariables": {
                    "fontFamily": ["\"IBM Plex Sans\", Arial, sans-serif", "Ignored Sans"]
                }
            })),
            r#""IBM Plex Sans",Arial,sans-serif"#
        );
        assert_eq!(
            config_font_family_or_first_array_css(&json!({
                "fontFamily": ["Courier, monospace", "Ignored Sans"]
            })),
            "Courier,monospace"
        );
        assert_eq!(
            config_font_family_or_first_array_css(&json!({
                "fontFamily": []
            })),
            MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
    }

    #[test]
    fn configured_font_candidates_keep_the_first_accepted_normalized_source() {
        for (config, expected) in [
            (
                json!({"themeVariables":{"fontFamily":["Georgia"]},"fontFamily":"Arial"}),
                Some(("Georgia", "themeVariables.fontFamily")),
            ),
            (
                json!({"themeVariables":{"fontFamily":[42,"Georgia"]},"fontFamily":"Arial"}),
                Some(("Arial", "fontFamily")),
            ),
            (
                json!({"themeVariables":{"fontFamily":[""]},"fontFamily":"Arial"}),
                Some(("Arial", "fontFamily")),
            ),
            (
                json!({"themeVariables":{"fontFamily":[]},"fontFamily":"Arial"}),
                Some(("Arial", "fontFamily")),
            ),
            (json!({"themeVariables":{"fontFamily":false}}), None),
        ] {
            let expected = expected.map(|(font, source)| (font.to_owned(), source));
            assert_eq!(
                configured_font_family_or_first_array(&config),
                expected,
                "{config}"
            );
        }
    }

    #[test]
    fn configured_font_size_owner_follows_the_first_numeric_consumer() {
        for (config, expected) in [
            (
                json!({"themeVariables":{"fontSize":"bad"},"fontSize":"18px"}),
                Some((18.0, "fontSize")),
            ),
            (json!({"themeVariables":{"fontSize":false}}), None),
            (
                json!({"themeVariables":{"fontSize":"-1px"},"fontSize":"18px"}),
                Some((18.0, "fontSize")),
            ),
            (
                json!({"themeVariables":{"fontSize":-1},"fontSize":18}),
                Some((18.0, "fontSize")),
            ),
            (
                json!({"themeVariables":{"fontSize":-1},"fontSize":-2}),
                None,
            ),
            (
                json!({"themeVariables":{"fontSize":"23px"},"fontSize":18}),
                Some((23.0, "themeVariables.fontSize")),
            ),
            (
                json!({"themeVariables":{"fontSize":0},"fontSize":18}),
                Some((0.0, "themeVariables.fontSize")),
            ),
        ] {
            assert_eq!(
                configured_theme_or_root_font_size_px(&config),
                expected,
                "{config}"
            );
        }
    }

    #[test]
    fn configured_font_size_owner_preserves_number_only_root_fallback() {
        for (config, expected) in [
            (
                json!({"themeVariables":{"fontSize":"-1px"},"fontSize":"18"}),
                Some((18.0, "fontSize")),
            ),
            (
                json!({"themeVariables":{"fontSize":-1},"fontSize":-2}),
                None,
            ),
            (
                json!({"themeVariables":{"fontSize":0},"fontSize":18}),
                Some((0.0, "themeVariables.fontSize")),
            ),
            (
                json!({"themeVariables":{"fontSize":"bad"},"fontSize":"18px"}),
                None,
            ),
            (
                json!({"themeVariables":{"fontSize":"bad"},"fontSize":"18"}),
                Some((18.0, "fontSize")),
            ),
            (
                json!({"themeVariables":{"fontSize":"23px"},"fontSize":18}),
                Some((23.0, "themeVariables.fontSize")),
            ),
            (
                json!({"themeVariables":{"fontSize":false},"fontSize":18}),
                Some((18.0, "fontSize")),
            ),
        ] {
            assert_eq!(
                configured_theme_font_size_css_or_root_number_px(&config),
                expected,
                "{config}"
            );
        }
    }

    #[test]
    fn config_theme_or_root_font_size_px_uses_theme_then_legacy_then_default() {
        assert_eq!(
            config_theme_or_root_font_size_px(
                &json!({
                    "fontSize": "18px",
                    "themeVariables": {
                        "fontSize": "24px"
                    }
                }),
                16.0,
            ),
            24.0
        );
        assert_eq!(
            config_theme_or_root_font_size_px(
                &json!({
                    "fontSize": "18px"
                }),
                16.0,
            ),
            18.0
        );
        assert_eq!(config_theme_or_root_font_size_px(&json!({}), 16.0), 16.0);
    }

    #[test]
    fn config_theme_font_size_css_or_root_number_px_keeps_root_number_semantics() {
        assert_eq!(
            config_theme_font_size_css_or_root_number_px(
                &json!({
                    "fontSize": 18,
                    "themeVariables": {
                        "fontSize": "24px"
                    }
                }),
                16.0,
            ),
            24.0
        );
        assert_eq!(
            config_theme_font_size_css_or_root_number_px(
                &json!({
                    "fontSize": "18"
                }),
                16.0,
            ),
            18.0
        );
        assert_eq!(
            config_theme_font_size_css_or_root_number_px(
                &json!({
                    "fontSize": "18px"
                }),
                16.0,
            ),
            16.0
        );
    }
}
