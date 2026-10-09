use serde_json::Value;

/// Raw browser paint slots bound before Class measurement and SVG emission.
/// Values stay as CSS tokens; binding does not imply native color support.
#[derive(Debug, Clone)]
pub(crate) struct ClassCssThemeBinding {
    pub(crate) class_text: String,
    pub(crate) note_text: String,
    pub(crate) class_group_text: String,
    pub(crate) title_color: String,
    pub(crate) text_color: String,
    pub(crate) line_color: String,
    pub(crate) main_bkg: String,
    pub(crate) node_border: String,
    pub(crate) cluster_bkg: String,
    pub(crate) cluster_border: String,
    pub(crate) stroke_width: String,
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
        assert_eq!(binding.stroke_width, "calculated");
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
            title_color: token("titleColor", "#333"),
            text_color: token("textColor", &class_text),
            line_color: token("lineColor", "#333333"),
            main_bkg: token("mainBkg", "#ECECFF"),
            node_border: token("nodeBorder", "#9370DB"),
            cluster_bkg: token("clusterBkg", "#ffffde"),
            cluster_border: token("clusterBorder", "#aaaa33"),
            stroke_width: crate::config::config_css_number_or_string(
                config,
                &["themeVariables", "strokeWidth"],
            )
            .unwrap_or_else(|| "1".into()),
            edge_label_background: token("edgeLabelBackground", "rgba(232,232,232, 0.8)"),
            note_fill: token("noteBkgColor", "#fff5ad"),
            note_stroke: token("noteBorderColor", "#aaaa33"),
            palette,
            palette_look,
            use_gradient: crate::config::config_bool(config, &["themeVariables", "useGradient"])
                .unwrap_or(false),
        }
    }
}
