use crate::config::{config_css_number_or_string, config_string};
use serde_json::Value;

/// Operation-local visual compatibility inputs. Raw CSS remains observable in SVG.
#[derive(Debug)]
pub(crate) struct FlowchartCompatibilityBinding {
    pub(crate) text_color: String,
    pub(crate) line_color: String,
    pub(crate) arrowhead_color: String,
    pub(crate) node_border: String,
    pub(crate) main_bkg: String,
    pub(crate) node_text_color: String,
    pub(crate) title_color: String,
    pub(crate) stroke_width: String,
    pub(crate) error_bkg: String,
    pub(crate) error_text: String,
    pub(crate) edge_label_background: String,
    pub(crate) tertiary: String,
    pub(crate) cluster_bkg: String,
    pub(crate) cluster_border: String,
    pub(crate) border2: String,
    pub(crate) drop_shadow: String,
    pub(crate) root_font_family: String,
    pub(crate) agentflow_container_stroke: String,
    pub(crate) neo: crate::svg::PreparedCommonNeoCss,
    pub(crate) palette: Vec<Value>,
    pub(crate) palette_backgrounds: Vec<Value>,
    pub(crate) palette_look: String,
    pub(crate) html_labels: bool,
    pub(crate) node_stroke_width: f32,
    pub(crate) node_corner_radius: f64,
}

impl FlowchartCompatibilityBinding {
    pub(crate) fn resolve(config: &Value) -> Self {
        let theme = |key: &str, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.to_owned())
        };
        let css = |key: &str, fallback: &str| {
            config_css_number_or_string(config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.to_owned())
        };
        let text_color = theme("textColor", "#333");
        let line_color = theme("lineColor", "#333333");
        let cluster_border = theme("clusterBorder", "#aaaa33");
        Self {
            arrowhead_color: theme("arrowheadColor", &line_color),
            node_text_color: theme("nodeTextColor", &text_color),
            title_color: theme("titleColor", &text_color),
            text_color,
            line_color,
            node_border: theme("nodeBorder", "#9370DB"),
            main_bkg: theme("mainBkg", "#ECECFF"),
            stroke_width: css("strokeWidth", "1"),
            error_bkg: theme("errorBkgColor", "#552222"),
            error_text: theme("errorTextColor", "#552222"),
            edge_label_background: theme("edgeLabelBackground", "rgba(232,232,232, 0.8)"),
            tertiary: theme("tertiaryColor", "hsl(80, 100%, 96.2745098039%)"),
            cluster_bkg: theme("clusterBkg", "#ffffde"),
            border2: theme("border2", &cluster_border),
            agentflow_container_stroke: theme(
                "flowContainerStroke",
                &theme("secondaryBorderColor", &cluster_border),
            ),
            cluster_border,
            drop_shadow: theme("dropShadow", "none"),
            root_font_family: crate::config::config_root_font_family_css(config),
            neo: crate::svg::PreparedCommonNeoCss::new(config),
            palette: if matches!(
                config.get("theme").and_then(Value::as_str),
                Some("redux-color" | "redux-dark-color")
            ) {
                config
                    .pointer("/themeVariables/borderColorArray")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            } else {
                Vec::new()
            },
            palette_backgrounds: config
                .pointer("/themeVariables/bkgColorArray")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            palette_look: config
                .get("look")
                .and_then(Value::as_str)
                .filter(|look| {
                    !look.is_empty()
                        && look
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
                })
                .unwrap_or("classic")
                .to_owned(),
            html_labels: super::FlowchartConfigView::new(config).effective_html_labels(),
            node_stroke_width: crate::config::config_f64(
                config,
                &["themeVariables", "strokeWidth"],
            )
            .filter(|value| value.is_finite() && *value >= 0.0 && *value <= f32::MAX as f64)
            .map(|value| value as f32)
            .unwrap_or(1.3),
            node_corner_radius: crate::config::config_f64(config, &["themeVariables", "radius"])
                .unwrap_or(5.0)
                .max(0.0),
        }
    }
}

#[derive(Debug)]
pub(crate) struct FlowchartPreparedTheme {
    pub(crate) compatibility: FlowchartCompatibilityBinding,
    pub(crate) text_surface: super::FlowchartTextSurfacePaintPlan,
}

impl FlowchartPreparedTheme {
    pub(crate) fn resolve(
        theme: Option<&crate::diagram_theme::ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        node_owned: bool,
        work: &crate::resources::OperationWorkMeter,
    ) -> Result<Self, crate::resources::OperationWorkError> {
        Ok(Self {
            compatibility: FlowchartCompatibilityBinding::resolve(config.as_value()),
            text_surface: super::FlowchartTextSurfacePaintPlan::resolve(
                theme, config, node_owned, work,
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_preserves_common_aliases_numeric_css_and_palette_slots() {
        let binding = FlowchartCompatibilityBinding::resolve(&serde_json::json!({
            "theme": "redux-color", "look": "neo",
            "themeVariables": {
                "textColor": "var(--text)", "lineColor": "currentColor",
                "strokeWidth": 3, "clusterBorder": "var(--border)",
                "secondaryBorderColor": "", "flowContainerStroke": null,
                "borderColorArray": [null, " blue "], "bkgColorArray": [12]
            }
        }));
        assert_eq!(binding.node_text_color, "var(--text)");
        assert_eq!(binding.title_color, "var(--text)");
        assert_eq!(binding.arrowhead_color, "currentColor");
        assert_eq!(binding.stroke_width, "3");
        assert_eq!(binding.node_stroke_width, 3.0);
        assert_eq!(binding.border2, "var(--border)");
        assert_eq!(binding.agentflow_container_stroke, "");
        assert_eq!(
            binding.palette,
            [Value::Null, Value::String(" blue ".into())]
        );
        assert_eq!(binding.palette_backgrounds, [serde_json::json!(12)]);
        assert_eq!(binding.palette_look, "neo");
    }
}
