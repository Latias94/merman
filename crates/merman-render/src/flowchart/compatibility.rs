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
    pub(crate) rounded_rect_radius: Value,
    pub(crate) rounded_rect_radius_truthy: bool,
    pub(crate) rounded_rect_radius_numeric: Option<f64>,
    pub(crate) note_fill: String,
    pub(crate) note_stroke: String,
    pub(crate) state_inner_fill: String,
    pub(crate) small_state_shadow: bool,
    pub(crate) collapsed_agentflow_stroke: String,
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
        let node_border = theme("nodeBorder", "#9370DB");
        let rounded_rect_radius = config
            .get("themeVariables")
            .and_then(|theme| theme.get("radius"))
            .filter(|radius| !radius.is_null())
            .cloned()
            .unwrap_or_else(|| serde_json::json!(5));
        let rounded_rect_radius_truthy = crate::config::json_value_is_truthy(&rounded_rect_radius);
        let rounded_rect_radius_numeric = rounded_rect_radius_truthy
            .then(|| crate::config::json_f64(&rounded_rect_radius))
            .flatten();
        Self {
            rounded_rect_radius,
            rounded_rect_radius_truthy,
            rounded_rect_radius_numeric,
            note_fill: theme("noteBkgColor", "#fff5ad"),
            note_stroke: theme("noteBorderColor", "#aaaa33"),
            state_inner_fill: theme("stateBorder", &node_border),
            small_state_shadow: crate::config::value_at(config, &["themeVariables", "nodeShadow"])
                .is_some_and(crate::config::json_value_is_truthy),
            collapsed_agentflow_stroke: theme(
                "flowContainerStroke",
                &theme("secondaryBorderColor", "#aaaa33"),
            ),
            arrowhead_color: theme("arrowheadColor", &line_color),
            node_text_color: theme("nodeTextColor", &text_color),
            title_color: theme("titleColor", &text_color),
            text_color,
            line_color,
            node_border,
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
    pub(crate) effects: FlowchartEffectEligibility,
}

#[derive(Debug, Default)]
pub(crate) struct FlowchartEffectEligibility {
    pub(crate) edge: bool,
    pub(crate) label: bool,
    pub(crate) marker: bool,
    pub(crate) marker_route_work: usize,
}

impl FlowchartEffectEligibility {
    fn resolve(theme: Option<&crate::diagram_theme::ResolvedDiagramTheme>) -> Self {
        use crate::diagram_theme::{FamilyThemeMechanism, FamilyThemeRuleFacet, ThemeTarget};
        let Some(theme) = theme else {
            return Self::default();
        };
        let mut eligibility = Self {
            marker_route_work: theme.family_mechanism_routes().len(),
            ..Self::default()
        };
        for route in theme.family_mechanism_routes() {
            match route.mechanism() {
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Edge,
                    ..
                }
                | FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Edge,
                    facet: FamilyThemeRuleFacet::Effect,
                    ..
                } => eligibility.edge = true,
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel,
                    ..
                }
                | FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel,
                    facet: FamilyThemeRuleFacet::Effect,
                    ..
                } => eligibility.label = true,
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Marker,
                    ..
                }
                | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Marker,
                }
                | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Marker,
                    ..
                } => eligibility.marker = true,
                _ => {}
            }
        }
        eligibility
    }
}

impl FlowchartPreparedTheme {
    pub(crate) fn resolve(
        theme: Option<&crate::diagram_theme::ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        node_owned: bool,
        work: &crate::resources::OperationWorkMeter,
    ) -> Result<Self, crate::resources::OperationWorkError> {
        Ok(Self {
            effects: FlowchartEffectEligibility::resolve(theme),
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
    fn rounded_radius_keeps_numeric_zero_distinct_from_string_zero_and_null() {
        for (raw, expected_raw, truthy, numeric) in [
            (
                serde_json::json!(null),
                serde_json::json!(5),
                true,
                Some(5.0),
            ),
            (serde_json::json!(0), serde_json::json!(0), false, None),
            (
                serde_json::json!("0"),
                serde_json::json!("0"),
                true,
                Some(0.0),
            ),
            (
                serde_json::json!("var(--radius)"),
                serde_json::json!("var(--radius)"),
                true,
                None,
            ),
        ] {
            let binding = FlowchartCompatibilityBinding::resolve(&serde_json::json!({
                "themeVariables": { "radius": raw }
            }));
            assert_eq!(binding.rounded_rect_radius, expected_raw);
            assert_eq!(binding.rounded_rect_radius_truthy, truthy);
            assert_eq!(binding.rounded_rect_radius_numeric, numeric);
        }
    }

    #[test]
    fn special_shapes_preserve_raw_roles_and_distinct_collapsed_alias_fallback() {
        let binding = FlowchartCompatibilityBinding::resolve(&serde_json::json!({
            "themeVariables": {
                "noteBkgColor": "", "noteBorderColor": "var(--note)",
                "nodeBorder": "currentColor", "clusterBorder": "var(--cluster)",
                "nodeShadow": {}
            }
        }));
        assert_eq!(binding.note_fill, "");
        assert_eq!(binding.note_stroke, "var(--note)");
        assert_eq!(binding.state_inner_fill, "currentColor");
        assert!(binding.small_state_shadow);
        assert_eq!(binding.agentflow_container_stroke, "var(--cluster)");
        assert_eq!(binding.collapsed_agentflow_stroke, "#aaaa33");
    }

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
