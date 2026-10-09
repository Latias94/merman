use serde_json::Value;

#[derive(Debug, Clone)]
pub(crate) struct BlockClassCssDeclarations {
    pub(crate) id: String,
    pub(crate) shape: Vec<(String, String)>,
    pub(crate) text: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub(crate) struct BlockCssThemeBinding {
    pub(crate) class_definitions: Vec<BlockClassCssDeclarations>,
    pub(crate) look_defs: crate::svg::PreparedLookDefs,
    pub(crate) text_color: String,
    pub(crate) node_text_color: String,
    pub(crate) title_color: String,
    pub(crate) main_bkg: String,
    pub(crate) node_border: String,
    pub(crate) line_color: String,
    pub(crate) arrowhead_color: String,
    pub(crate) stroke_width: String,
    pub(crate) edge_label_background: String,
    pub(crate) cluster_bkg: String,
    pub(crate) cluster_border: String,
    pub(crate) html_labels: bool,
    pub(crate) palette_look: String,
    pub(crate) palette: Vec<(String, Option<String>)>,
    pub(crate) root_font_family: String,
    pub(crate) edge_stroke_width_px: Option<f64>,
    pub(crate) look: String,
    pub(crate) hand_drawn_seed: Option<f64>,
}

impl BlockCssThemeBinding {
    pub(crate) fn prepare_class_definitions(
        &mut self,
        definitions: &indexmap::IndexMap<
            String,
            merman_core::diagrams::block::BlockClassDefRenderModel,
        >,
    ) {
        let declarations = |styles: &[String]| {
            styles
                .iter()
                .filter_map(|style| crate::mermaid_style::parse_safe_style_decl(style))
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect()
        };
        self.class_definitions = definitions
            .values()
            .map(|definition| BlockClassCssDeclarations {
                id: definition.id.clone(),
                shape: declarations(&definition.styles),
                text: declarations(&definition.text_styles),
            })
            .collect();
    }

    pub(crate) fn resolve(config: &Value) -> crate::Result<Self> {
        let token = |key: &str, fallback: &str| {
            crate::config::config_string(config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.to_owned())
        };
        let text_color = token("textColor", "#333");
        let node_text_color = token("nodeTextColor", &text_color);
        let main_bkg = token("mainBkg", "#ECECFF");
        let node_border = token("nodeBorder", "#9370DB");
        let line_color = token("lineColor", "#333333");
        let title_color = token("titleColor", &text_color);
        let arrowhead_color = token("arrowheadColor", &line_color);
        let fade = |raw: String, opacity| -> crate::Result<String> {
            use merman_core::theme_color::{ColorChannel, ThemeColor, rgba};
            let color = ThemeColor::parse(raw.trim())?;
            Ok(rgba(
                color.channel(ColorChannel::Red),
                color.channel(ColorChannel::Green),
                color.channel(ColorChannel::Blue),
                opacity,
            )?)
        };
        let cluster_bkg = fade(token("clusterBkg", "#ffffde"), 0.5)?;
        let cluster_border = fade(token("clusterBorder", "#aaaa33"), 0.2)?;
        let stroke_width =
            crate::config::config_css_number_or_string(config, &["themeVariables", "strokeWidth"])
                .unwrap_or_else(|| "1".to_owned());
        let edge_stroke_width_px = stroke_width
            .trim()
            .trim_end_matches(';')
            .trim()
            .trim_end_matches("!important")
            .trim();
        let edge_stroke_width_px = edge_stroke_width_px
            .strip_suffix("px")
            .unwrap_or(edge_stroke_width_px)
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite());
        let palette_look = config
            .get("look")
            .and_then(|look| match look {
                Value::String(value) => Some(value.clone()),
                Value::Number(value) => Some(value.to_string()),
                _ => None,
            })
            .filter(|look: &String| {
                !look.is_empty()
                    && look
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
            })
            .unwrap_or_else(|| "classic".to_owned());
        let palette = if matches!(
            config.get("theme").and_then(Value::as_str),
            Some("redux-color" | "redux-dark-color")
        ) {
            let backgrounds = config
                .pointer("/themeVariables/bkgColorArray")
                .and_then(Value::as_array)
                .filter(|v| !v.is_empty());
            config
                .pointer("/themeVariables/borderColorArray")
                .and_then(Value::as_array)
                .map(|borders| {
                    borders
                        .iter()
                        .enumerate()
                        .map(|(i, border)| {
                            let color = |value: &Value| {
                                value
                                    .as_str()
                                    .map(|value| value.trim().to_owned())
                                    .unwrap_or_else(|| value.to_string())
                            };
                            let border = color(border);
                            let background =
                                backgrounds.map(|values| color(&values[i % values.len()]));
                            (border, background)
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Ok(Self {
            class_definitions: Vec::new(),
            look_defs: crate::svg::PreparedLookDefs::new(config),
            text_color,
            node_text_color,
            title_color,
            main_bkg,
            node_border,
            line_color,
            arrowhead_color,
            stroke_width,
            edge_stroke_width_px,
            edge_label_background: token("edgeLabelBackground", "rgba(232,232,232, 0.8)"),
            cluster_bkg,
            cluster_border,
            html_labels: crate::config::config_effective_html_labels(config),
            palette_look,
            palette,
            root_font_family: crate::config::config_root_font_family_css(config),
            look: config
                .get("look")
                .and_then(Value::as_str)
                .unwrap_or("classic")
                .to_owned(),
            hand_drawn_seed: config.get("handDrawnSeed").and_then(Value::as_f64),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_declarations_preserve_duplicates_order_and_safe_source_spelling() {
        let mut definition = merman_core::diagrams::block::BlockClassDefRenderModel::default();
        definition.id = "branded".into();
        definition.styles = vec![
            "fill:red".into(),
            "stroke:var(--border)".into(),
            "fill:blue !important".into(),
            "fill:red;stroke:black".into(),
        ];
        definition.text_styles = vec!["COLOR:currentColor".into()];
        let definitions = indexmap::IndexMap::from([("branded".into(), definition)]);
        let mut binding = BlockCssThemeBinding::resolve(&serde_json::json!({})).unwrap();
        binding.prepare_class_definitions(&definitions);
        let class = &binding.class_definitions[0];
        assert_eq!(class.id, "branded");
        assert_eq!(
            class.shape,
            vec![
                ("fill".into(), "red".into()),
                ("stroke".into(), "var(--border)".into()),
                ("fill".into(), "blue !important".into()),
            ]
        );
        assert_eq!(class.text, vec![("COLOR".into(), "currentColor".into())]);
    }

    #[test]
    fn raw_defaults_and_nonfinite_width_do_not_fabricate_native_values() {
        let binding = BlockCssThemeBinding::resolve(&serde_json::json!({
            "themeVariables": {"textColor":"var(--text)", "lineColor":"currentColor", "strokeWidth":"NaNpx", "mainBkg":"var(--node)"}
        })).unwrap();
        assert_eq!(binding.title_color, "var(--text)");
        assert_eq!(binding.node_text_color, "var(--text)");
        assert_eq!(binding.arrowhead_color, "currentColor");
        assert_eq!(binding.main_bkg, "var(--node)");
        assert_eq!(binding.stroke_width, "NaNpx");
        assert_eq!(binding.edge_stroke_width_px, None);
        assert!(
            BlockCssThemeBinding::resolve(
                &serde_json::json!({"themeVariables":{"clusterBkg":"var(--cluster)"}})
            )
            .is_err()
        );
    }

    #[test]
    fn palette_keeps_numeric_values_cycles_backgrounds_and_sanitizes_look() {
        let binding = BlockCssThemeBinding::resolve(&serde_json::json!({
            "theme":"redux-color", "look":7,
            "themeVariables":{"borderColorArray":[" red ",12],"bkgColorArray":[" transparent "]}
        }))
        .unwrap();
        assert_eq!(binding.palette_look, "7");
        assert_eq!(
            binding.palette,
            [
                ("red".to_owned(), Some("transparent".to_owned())),
                ("12".to_owned(), Some("transparent".to_owned()))
            ]
        );
        let unsafe_look = BlockCssThemeBinding::resolve(&serde_json::json!({"theme":"redux-color","look":"x\"] .node","themeVariables":{"borderColorArray":["red"]}})).unwrap();
        assert_eq!(unsafe_look.palette_look, "classic");
        assert_eq!(unsafe_look.palette[0].1, None);
    }
}
