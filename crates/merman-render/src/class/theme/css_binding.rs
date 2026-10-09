use serde_json::Value;

/// Raw browser paint slots bound before Class measurement and SVG emission.
/// Values stay as CSS tokens; binding does not imply native color support.
#[derive(Debug, Clone)]
pub(crate) struct ClassCssThemeBinding {
    pub(crate) class_text: String,
    pub(crate) note_text: String,
    pub(crate) class_group_text: String,
    pub(crate) text_color: String,
    pub(crate) main_bkg: String,
    pub(crate) node_default_fill: String,
    pub(crate) node_default_stroke: String,
    pub(crate) node_border: String,
    pub(crate) edge_label_background: String,
    pub(crate) note_fill: String,
    pub(crate) note_stroke: String,
    pub(crate) palette: Vec<(String, Option<String>)>,
    pub(crate) palette_look: String,
    pub(crate) use_gradient: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_binding_preserves_mermaid_boolean_coercion() {
        for (value, expected) in [
            (serde_json::json!(true), true),
            (serde_json::json!(false), false),
            (serde_json::json!("true"), true),
            (serde_json::json!("false"), false),
            (serde_json::json!(" ON "), true),
            (serde_json::json!(1), true),
            (serde_json::json!(0), false),
            (serde_json::json!(-1), true),
            (serde_json::json!("unknown"), false),
        ] {
            let binding = ClassCssThemeBinding::resolve(&serde_json::json!({
                "themeVariables": {"useGradient": value}
            }));
            assert_eq!(binding.use_gradient, expected);
        }
    }

    #[test]
    fn node_defaults_preserve_primary_fallback_without_changing_css_defaults() {
        let fallback = ClassCssThemeBinding::resolve(&serde_json::json!({
            "themeVariables": {
                "primaryColor": "#112233",
                "primaryBorderColor": "#445566"
            }
        }));
        assert_eq!(fallback.node_default_fill, "#112233");
        assert_eq!(fallback.node_default_stroke, "#445566");
        assert_eq!(fallback.main_bkg, "#ECECFF");
        assert_eq!(fallback.node_border, "#9370DB");

        let explicit = ClassCssThemeBinding::resolve(&serde_json::json!({
            "themeVariables": {
                "mainBkg": "var(--fill)",
                "nodeBorder": "none",
                "primaryColor": "#112233",
                "primaryBorderColor": "#445566"
            }
        }));
        assert_eq!(explicit.node_default_fill, "var(--fill)");
        assert_eq!(explicit.node_default_stroke, "none");
    }

    #[test]
    fn browser_tokens_and_palette_spelling_survive_binding() {
        let config = serde_json::json!({
            "theme": "redux-color",
            "look": "handDrawn",
            "themeVariables": {
                "classText": "var(--class-text)",
                "mainBkg": "color(display-p3 1 0 0)",
                "strokeWidth": "calculated",
                "noteBkgColor": "none",
                "borderColorArray": ["  var(--border)  ", 12],
                "bkgColorArray": ["  transparent  "]
            }
        });
        let binding = ClassCssThemeBinding::resolve(&config);
        assert_eq!(binding.class_text, "var(--class-text)");
        assert_eq!(binding.main_bkg, "color(display-p3 1 0 0)");
        assert_eq!(binding.note_fill, "none");
        assert_eq!(binding.palette_look, "handDrawn");
        assert_eq!(
            binding.palette,
            vec![
                ("var(--border)".into(), Some("transparent".into())),
                ("12".into(), Some("transparent".into()))
            ]
        );
    }
}

impl ClassCssThemeBinding {
    pub(crate) fn resolve(config: &Value) -> Self {
        let token = |key: &str, fallback: &str| {
            crate::config::config_string(config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.to_string())
        };
        let text_color = token("textColor", "#333");
        let class_text = token("classText", &token("primaryTextColor", &text_color));
        let class_group_text =
            crate::config::config_string(config, &["themeVariables", "nodeBorder"])
                .unwrap_or_else(|| class_text.clone());
        let palette_look = config
            .get("look")
            .and_then(|value| match value {
                Value::String(value) => Some(value.clone()),
                Value::Number(value) => Some(value.to_string()),
                _ => None,
            })
            .filter(|look| {
                !look.is_empty()
                    && look
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            })
            .unwrap_or_else(|| "classic".into());
        let palette = if matches!(
            config.get("theme").and_then(Value::as_str),
            Some("redux-color" | "redux-dark-color")
        ) {
            let backgrounds = config
                .pointer("/themeVariables/bkgColorArray")
                .and_then(Value::as_array)
                .filter(|colors| !colors.is_empty());
            let color = |value: &Value| {
                value
                    .as_str()
                    .map(|value| value.trim().to_owned())
                    .unwrap_or_else(|| value.to_string())
            };
            config
                .pointer("/themeVariables/borderColorArray")
                .and_then(Value::as_array)
                .map(|borders| {
                    borders
                        .iter()
                        .enumerate()
                        .map(|(index, border)| {
                            (
                                color(border),
                                backgrounds.map(|backgrounds| {
                                    color(&backgrounds[index % backgrounds.len()])
                                }),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            class_text: class_text.clone(),
            note_text: token("noteTextColor", "#333"),
            class_group_text,
            text_color: token("textColor", &class_text),
            main_bkg: token("mainBkg", "#ECECFF"),
            node_default_fill: token("mainBkg", &token("primaryColor", "#ECECFF")),
            node_default_stroke: token("nodeBorder", &token("primaryBorderColor", "#9370DB")),
            node_border: token("nodeBorder", "#9370DB"),
            edge_label_background: token("edgeLabelBackground", "rgba(232,232,232, 0.8)"),
            note_fill: token("noteBkgColor", "#fff5ad"),
            note_stroke: token("noteBorderColor", "#aaaa33"),
            palette,
            palette_look,
            use_gradient: crate::config::value_at(config, &["themeVariables", "useGradient"])
                .and_then(crate::config::json_bool)
                .unwrap_or(false),
        }
    }
}
