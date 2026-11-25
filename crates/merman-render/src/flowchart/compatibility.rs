use crate::config::{config_css_number_or_string, config_string};
use crate::theme::{Color, FamilyTheme};
use serde_json::Value;

use super::native_theme::FlowchartNativeStyle;

/// Operation-local visual compatibility inputs. Raw CSS remains observable in SVG.
#[derive(Debug)]
pub(crate) struct FlowchartCompatibilityBinding {
    pub(crate) look: crate::config::DiagramLook<'static>,
    pub(crate) hand_drawn_seed: Option<f64>,
    pub(crate) look_defs: crate::svg::PreparedLookDefs,
    pub(crate) text_color: String,
    pub(crate) line_color: String,
    pub(crate) arrowhead_color: String,
    pub(crate) node_border: String,
    pub(crate) main_bkg: String,
    pub(crate) node_text_color: String,
    pub(crate) title_color: String,
    pub(crate) stroke_width: String,
    pub(crate) connector_stroke_width: Option<f32>,
    pub(crate) stroke_width_configured: bool,
    pub(crate) error_bkg: String,
    pub(crate) error_text: String,
    pub(crate) edge_label_background: String,
    pub(crate) tertiary: String,
    pub(crate) cluster_bkg: String,
    pub(crate) cluster_border: String,
    pub(crate) border2: String,
    pub(crate) drop_shadow: String,
    pub(crate) root_font_family: String,
    pub(crate) root_font_weight: Option<u16>,
    pub(crate) agentflow_container_stroke: String,
    pub(crate) neo: crate::svg::PreparedCommonNeoCss,
    pub(crate) html_labels: bool,
    pub(crate) node_stroke_width: f32,
    pub(crate) node_corner_radius: f64,
    pub(crate) node_corner_radius_configured: bool,
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
    pub(crate) fn title_color_source(
        config: &(impl crate::config::ConfigLookup + ?Sized),
    ) -> (Option<&'static str>, &str) {
        crate::config::selected_config_string(
            config,
            &["themeVariables.titleColor", "themeVariables.textColor"],
        )
        .map_or((None, "#333"), |(path, color)| (Some(path), color))
    }

    pub(crate) fn resolve(config: &(impl crate::config::ConfigLookup + ?Sized)) -> Self {
        let theme = |key: &str, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.to_owned())
        };
        let text_color = theme("textColor", "#333");
        let line_color = theme("lineColor", "#333333");
        let cluster_border = theme("clusterBorder", "#aaaa33");
        let node_border = theme("nodeBorder", "#9370DB");
        let stroke_width = config_css_number_or_string(config, &["themeVariables", "strokeWidth"]);
        let node_corner_radius = crate::config::config_f64(config, &["themeVariables", "radius"]);
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
            look: crate::config::config_diagram_look(config).into_static(),
            hand_drawn_seed: config.get("handDrawnSeed").and_then(Value::as_f64),
            look_defs: crate::svg::PreparedLookDefs::new(config),
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
            title_color: Self::title_color_source(config).1.to_owned(),
            text_color,
            line_color,
            node_border,
            main_bkg: theme("mainBkg", "#ECECFF"),
            stroke_width_configured: stroke_width.is_some(),
            stroke_width: stroke_width.unwrap_or_else(|| "1".to_owned()),
            connector_stroke_width: None,
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
            root_font_weight: None,
            neo: crate::svg::PreparedCommonNeoCss::new(config),
            html_labels: super::FlowchartConfigView::new(config).effective_html_labels(),
            node_stroke_width: crate::config::config_f64(
                config,
                &["themeVariables", "strokeWidth"],
            )
            .filter(|value| value.is_finite() && *value >= 0.0 && *value <= f32::MAX as f64)
            .map(|value| value as f32)
            .unwrap_or(1.3),
            node_corner_radius: node_corner_radius.unwrap_or(5.0).max(0.0),
            node_corner_radius_configured: node_corner_radius.is_some(),
        }
    }

    fn native(
        config: &merman_model::presentation::PresentationConfig,
        theme: &FamilyTheme,
        style: &FlowchartNativeStyle,
    ) -> Self {
        let roles = &theme.roles;
        let text_color = roles.text_primary.to_string();
        let line_color = roles.stroke_connector.first_color().to_string();
        let border = style.border.to_string();
        let width = style.form.stroke_width.to_string();
        let radius = style.form.radius;
        Self {
            look: crate::config::config_diagram_look(config).into_static(),
            hand_drawn_seed: config.get("handDrawnSeed").and_then(Value::as_f64),
            look_defs: crate::svg::PreparedLookDefs::new(config),
            node_text_color: text_color.clone(),
            text_color,
            arrowhead_color: line_color.clone(),
            state_inner_fill: line_color.clone(),
            line_color,
            node_border: border.clone(),
            main_bkg: style.node.to_string(),
            title_color: roles.text_title.to_string(),
            neo: crate::svg::PreparedCommonNeoCss::native_node_stroke(&border, &width),
            stroke_width: width,
            connector_stroke_width: Some(style.form.connector_width as f32),
            stroke_width_configured: config_css_number_or_string(
                config,
                &["themeVariables", "strokeWidth"],
            )
            .is_some(),
            error_bkg: roles.status_danger.to_string(),
            error_text: roles.text_callout.to_string(),
            edge_label_background: "#00000000".to_owned(),
            tertiary: style.emphasis.to_string(),
            cluster_bkg: style.container.to_string(),
            cluster_border: border.clone(),
            border2: border.clone(),
            drop_shadow: "none".to_owned(),
            root_font_family: crate::theme::font_family_css(
                theme.typography.stack(theme.typography.body_family),
            ),
            root_font_weight: Some(theme.typography.label_weight),
            agentflow_container_stroke: border.clone(),
            html_labels: super::FlowchartConfigView::new(config).effective_html_labels(),
            node_stroke_width: style.form.stroke_width as f32,
            node_corner_radius: radius,
            node_corner_radius_configured: crate::config::config_f64(
                config,
                &["themeVariables", "radius"],
            )
            .is_some(),
            rounded_rect_radius: serde_json::json!(radius),
            rounded_rect_radius_truthy: radius > 0.0,
            rounded_rect_radius_numeric: Some(radius),
            note_fill: style.callout.to_string(),
            note_stroke: border.clone(),
            small_state_shadow: crate::config::value_at(config, &["themeVariables", "nodeShadow"])
                .is_some_and(crate::config::json_value_is_truthy),
            collapsed_agentflow_stroke: border,
        }
    }
}

#[derive(Debug)]
pub(crate) struct FlowchartPreparedTheme {
    pub(crate) compatibility: FlowchartCompatibilityBinding,
    pub(crate) native: Option<FlowchartNativeStyle>,
}

impl FlowchartPreparedTheme {
    pub(crate) fn resolve(
        config: &merman_model::presentation::PresentationConfig,
        theme: Option<&FamilyTheme>,
        background: Option<Color>,
    ) -> Self {
        if let Some(theme) = theme {
            let native = FlowchartNativeStyle::resolve(theme, background);
            return Self {
                compatibility: FlowchartCompatibilityBinding::native(config, theme, &native),
                native: Some(native),
            };
        }
        Self {
            native: None,
            compatibility: FlowchartCompatibilityBinding::resolve(config),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_look_and_seed_preserve_normalized_tokens_and_numeric_only_seed() {
        for raw in [
            serde_json::json!(null),
            serde_json::json!(" neo "),
            serde_json::json!("default"),
            serde_json::json!("handDrawn"),
            serde_json::json!("unknown"),
            serde_json::json!(false),
        ] {
            for seed in [
                serde_json::json!(17.25),
                serde_json::json!("17.25"),
                serde_json::json!(null),
            ] {
                let config = serde_json::json!({"look": raw, "handDrawnSeed": seed});
                let binding = FlowchartCompatibilityBinding::resolve(&config);
                let expected = crate::config::config_diagram_look(&config);
                assert_eq!(binding.look.as_str(), expected.as_str());
                assert_eq!(binding.look.serialized(), expected.serialized());
                assert_eq!(binding.hand_drawn_seed, seed.as_f64());
            }
        }
    }

    #[test]
    fn prepared_defs_use_known_dark_themes_and_keep_empty_gradient_stops() {
        for (theme, expected_flood) in [
            ("neo-dark", "#FFFFFF"),
            ("custom-dark", "#000000"),
            ("Dark", "#000000"),
        ] {
            let binding = FlowchartCompatibilityBinding::resolve(&serde_json::json!({
                "theme": theme, "themeVariables": {"useGradient": " ON ",
                    "gradientStart": "", "secondaryBorderColor": "var(--stop)"}
            }));
            assert_eq!(binding.look_defs.flood_color(), expected_flood);
            assert_eq!(binding.look_defs.gradient(), Some(("", "var(--stop)")));
        }
        let binding = FlowchartCompatibilityBinding::resolve(&serde_json::json!({
            "themeVariables": {"useGradient": -1, "gradientStart": ""}
        }));
        assert_eq!(binding.look_defs.gradient(), Some(("", "")));
        let binding = FlowchartCompatibilityBinding::resolve(&serde_json::json!({
            "themeVariables": {"useGradient": "unknown"}
        }));
        assert_eq!(binding.look_defs.gradient(), None);
    }

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
    fn compatibility_preserves_common_aliases_and_numeric_css() {
        let binding = FlowchartCompatibilityBinding::resolve(&serde_json::json!({
            "themeVariables": {
                "textColor": "var(--text)", "lineColor": "currentColor",
                "strokeWidth": 3, "clusterBorder": "var(--border)",
                "secondaryBorderColor": "", "flowContainerStroke": null
            }
        }));
        assert_eq!(binding.node_text_color, "var(--text)");
        assert_eq!(binding.title_color, "var(--text)");
        assert_eq!(binding.arrowhead_color, "currentColor");
        assert_eq!(binding.stroke_width, "3");
        assert_eq!(binding.node_stroke_width, 3.0);
        assert_eq!(binding.border2, "var(--border)");
        assert_eq!(binding.agentflow_container_stroke, "");
    }
}
