use merman_render_core::config::{
    DiagramLook, MERMAID_DEFAULT_FONT_FAMILY_CSS, config_bool, config_diagram_look,
    config_effective_html_labels, config_f64, config_f64_css_px,
    config_font_family_or_first_array_css, config_string_or_first_array,
    config_theme_or_root_font_size_px, normalize_css_font_family,
};
use serde_json::Value;

const DEFAULT_NODE_SPACING: f64 = 50.0;
const DEFAULT_RANK_SPACING: f64 = 50.0;
const DEFAULT_FONT_SIZE: f64 = 16.0;
const DEFAULT_VIEWPORT_PADDING: f64 = 8.0;
const DEFAULT_USE_MAX_WIDTH: bool = true;
const DEFAULT_TITLE_TOP_MARGIN: f64 = 25.0;

pub struct RequirementConfigView<'a> {
    effective_config: &'a Value,
    state_config: &'a Value,
}

impl<'a> RequirementConfigView<'a> {
    pub fn new(effective_config: &'a Value) -> Self {
        Self {
            effective_config,
            state_config: effective_config.get("state").unwrap_or(&Value::Null),
        }
    }

    pub fn hand_drawn_seed(&self) -> f64 {
        self.effective_config
            .get("handDrawnSeed")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    }

    pub fn layout_settings(&self) -> RequirementLayoutSettings {
        RequirementLayoutSettings {
            html_labels: config_bool(self.effective_config, &["htmlLabels"]).unwrap_or(true),
            edge_html_labels: config_effective_html_labels(self.effective_config),
            nodesep: self.layout_spacing("nodeSpacing", DEFAULT_NODE_SPACING),
            ranksep: self.layout_spacing("rankSpacing", DEFAULT_RANK_SPACING),
            font_family: self.font_family(),
            font_size: self.font_size(),
            calculation_font_family: self.calculation_font_family(),
            calculation_font_size: self.calculation_font_size(),
        }
    }

    pub(crate) fn layout_settings_with_resolved_typography(
        &self,
        resolved_font_family: Option<&str>,
        resolved_font_size_px: Option<f64>,
    ) -> RequirementLayoutSettings {
        let mut settings = self.layout_settings();
        if let Some(font_family) = resolved_font_family {
            settings.font_family = font_family.to_owned();
            settings.calculation_font_family = font_family.to_owned();
        }
        if let Some(font_size_px) = resolved_font_size_px {
            settings.font_size = font_size_px;
        }
        settings
    }

    #[cfg(test)]
    pub(crate) fn render_settings(&self) -> RequirementRenderSettings<'a> {
        RequirementRenderSettings {
            html_labels: config_bool(self.effective_config, &["htmlLabels"]).unwrap_or(true),
            edge_html_labels: config_effective_html_labels(self.effective_config),
            body_text_start: self.effective_config.get("layout").and_then(Value::as_str)
                == Some("elk"),
            look: self.look(),
            viewport_padding: DEFAULT_VIEWPORT_PADDING,
            use_max_width: config_bool(self.state_config, &["useMaxWidth"])
                .unwrap_or(DEFAULT_USE_MAX_WIDTH),
            title_top_margin: self
                .config_f64(&["state", "titleTopMargin"])
                .unwrap_or(DEFAULT_TITLE_TOP_MARGIN),
            font_family: self.font_family(),
            font_size: self.font_size(),
        }
    }

    pub(crate) fn render_settings_with_binding(
        &self,
        binding: &super::RequirementCssBinding,
    ) -> RequirementRenderSettings<'a> {
        RequirementRenderSettings {
            html_labels: config_bool(self.effective_config, &["htmlLabels"]).unwrap_or(true),
            edge_html_labels: config_effective_html_labels(self.effective_config),
            body_text_start: self.effective_config.get("layout").and_then(Value::as_str)
                == Some("elk"),
            look: self.look(),
            viewport_padding: DEFAULT_VIEWPORT_PADDING,
            use_max_width: config_bool(self.state_config, &["useMaxWidth"])
                .unwrap_or(DEFAULT_USE_MAX_WIDTH),
            title_top_margin: self
                .config_f64(&["state", "titleTopMargin"])
                .unwrap_or(DEFAULT_TITLE_TOP_MARGIN),
            font_family: binding.layout.font_family.clone(),
            font_size: binding.layout.font_size,
        }
    }

    pub(crate) fn look(&self) -> DiagramLook<'a> {
        config_diagram_look(self.effective_config)
    }

    #[cfg(feature = "layout-layered")]
    pub(super) fn flowchart_curve(&self) -> &str {
        self.effective_config
            .pointer("/flowchart/curve")
            .and_then(Value::as_str)
            .unwrap_or("basis")
    }

    #[cfg(feature = "layout-layered")]
    pub(super) fn layered_edge_label_y_offset(&self) -> f64 {
        let margins = self
            .effective_config
            .pointer("/flowchart/subGraphTitleMargin");
        let [top, bottom] = ["top", "bottom"].map(|side| {
            margins
                .and_then(|margin| margin.get(side))
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
        });
        (top + bottom) / 2.0
    }

    pub fn canonical_stroke_width(&self) -> f64 {
        if self.look().is_neo() {
            merman_render_core::config::config_f64_css_absolute_px(
                self.effective_config,
                &["themeVariables", "strokeWidth"],
            )
            .filter(|width| *width >= 0.0)
            .unwrap_or(1.0)
        } else {
            1.0
        }
    }

    fn config_f64(&self, path: &[&str]) -> Option<f64> {
        config_f64(self.effective_config, path)
    }

    fn root_f64(&self, key: &str) -> Option<f64> {
        self.config_f64(&[key])
    }

    fn state_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.state_config, &[key])
    }

    fn layout_spacing(&self, key: &str, default: f64) -> f64 {
        let renderer_spacing = self.state_f64(key).unwrap_or(default);
        [
            self.root_f64(key),
            Some(renderer_spacing),
            self.config_f64(&["flowchart", key]),
        ]
        .into_iter()
        .flatten()
        .find(|value| *value != 0.0)
        .unwrap_or(default)
    }

    fn font_family(&self) -> String {
        config_font_family_or_first_array_css(self.effective_config)
    }

    fn font_size(&self) -> f64 {
        config_theme_or_root_font_size_px(self.effective_config, DEFAULT_FONT_SIZE)
    }

    fn calculation_font_family(&self) -> String {
        let raw = config_string_or_first_array(self.effective_config, &["fontFamily"])
            .unwrap_or_else(|| MERMAID_DEFAULT_FONT_FAMILY_CSS.to_string());
        let normalized = normalize_css_font_family(&raw);
        if normalized.is_empty() {
            MERMAID_DEFAULT_FONT_FAMILY_CSS.to_string()
        } else {
            normalized
        }
    }

    fn calculation_font_size(&self) -> f64 {
        config_f64_css_px(self.effective_config, &["fontSize"]).unwrap_or(DEFAULT_FONT_SIZE)
    }
}

#[derive(Debug, Clone)]
pub struct RequirementLayoutSettings {
    pub html_labels: bool,
    pub(crate) edge_html_labels: bool,
    pub nodesep: f64,
    pub ranksep: f64,
    pub(crate) font_family: String,
    pub(crate) font_size: f64,
    pub(crate) calculation_font_family: String,
    pub(crate) calculation_font_size: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct RequirementRenderSettings<'a> {
    pub html_labels: bool,
    pub(crate) edge_html_labels: bool,
    pub(crate) body_text_start: bool,
    pub(crate) look: DiagramLook<'a>,
    pub(crate) viewport_padding: f64,
    pub(crate) use_max_width: bool,
    pub(crate) title_top_margin: f64,
    pub(crate) font_family: String,
    pub(crate) font_size: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn requirement_layout_settings_preserve_defaults() {
        let cfg = json!({});
        let settings = RequirementConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.nodesep, DEFAULT_NODE_SPACING);
        assert_eq!(settings.ranksep, DEFAULT_RANK_SPACING);
        assert_eq!(
            settings.font_family,
            merman_render_core::config::MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
        assert_eq!(settings.font_size, DEFAULT_FONT_SIZE);
        assert_eq!(
            settings.calculation_font_family,
            merman_render_core::config::MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
        assert_eq!(settings.calculation_font_size, DEFAULT_FONT_SIZE);
        assert!(settings.html_labels);
        assert!(settings.edge_html_labels);
    }

    #[test]
    fn requirement_label_modes_follow_node_and_shared_edge_ownership() {
        for (config, node_html, edge_html, body_start) in [
            (json!({"htmlLabels": false}), false, false, false),
            (
                json!({"flowchart": {"htmlLabels": false}}),
                true,
                false,
                false,
            ),
            (
                json!({"htmlLabels": true, "flowchart": {"htmlLabels": false}, "layout": "elk"}),
                true,
                true,
                true,
            ),
        ] {
            let view = RequirementConfigView::new(&config);
            let layout = view.layout_settings();
            let render = view.render_settings();
            assert_eq!(layout.html_labels, node_html);
            assert_eq!(layout.edge_html_labels, edge_html);
            assert_eq!(render.html_labels, node_html);
            assert_eq!(render.edge_html_labels, edge_html);
            assert_eq!(render.body_text_start, body_start);
        }
    }

    #[test]
    fn requirement_layout_settings_keep_root_spacing_precedence() {
        let cfg = json!({
            "nodeSpacing": "0",
            "rankSpacing": 70,
            "state": {
                "nodeSpacing": 0,
                "rankSpacing": 60
            },
            "flowchart": {
                "nodeSpacing": 11,
                "rankSpacing": 12
            },
            "themeVariables": {
                "fontFamily": ["Courier New", "serif"],
                "fontSize": "18px"
            }
        });
        let settings = RequirementConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.nodesep, 11.0);
        assert_eq!(settings.ranksep, 70.0);
        assert_eq!(settings.font_family, "Courier New");
        assert_eq!(settings.font_size, 18.0);
        assert_eq!(
            settings.calculation_font_family,
            merman_render_core::config::MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
        assert_eq!(settings.calculation_font_size, DEFAULT_FONT_SIZE);
    }

    #[test]
    fn requirement_layout_settings_default_renderer_spacing_precedes_flowchart_spacing() {
        let cfg = json!({
            "flowchart": {
                "nodeSpacing": "33",
                "rankSpacing": 44
            }
        });
        let settings = RequirementConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.nodesep, DEFAULT_NODE_SPACING);
        assert_eq!(settings.ranksep, DEFAULT_RANK_SPACING);
    }

    #[test]
    fn requirement_layout_settings_use_flowchart_spacing_when_renderer_spacing_is_zero() {
        let cfg = json!({
            "state": {
                "nodeSpacing": 0,
                "rankSpacing": 0
            },
            "flowchart": {
                "nodeSpacing": "33",
                "rankSpacing": 44
            }
        });
        let settings = RequirementConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.nodesep, 33.0);
        assert_eq!(settings.ranksep, 44.0);
    }

    #[test]
    fn requirement_render_settings_project_values() {
        let cfg = json!({
            "look": "neo",
            "handDrawnSeed": 7,
            "fontFamily": "Inter, sans-serif",
            "fontSize": "20px",
            "state": {
                "useMaxWidth": false
            }
        });
        let settings = RequirementConfigView::new(&cfg).render_settings();

        assert_eq!(settings.look.as_str(), "neo");
        assert_eq!(settings.viewport_padding, DEFAULT_VIEWPORT_PADDING);
        assert!(!settings.use_max_width);
        assert_eq!(RequirementConfigView::new(&cfg).hand_drawn_seed(), 7.0);
        assert_eq!(settings.title_top_margin, DEFAULT_TITLE_TOP_MARGIN);
        assert_eq!(settings.font_family, "Inter,sans-serif");
        assert_eq!(settings.font_size, 20.0);
    }

    #[test]
    fn requirement_title_margin_uses_the_state_config_namespace() {
        let cfg = json!({
            "titleTopMargin": 91,
            "requirement": { "titleTopMargin": 72 },
            "state": { "titleTopMargin": 33 }
        });

        let settings = RequirementConfigView::new(&cfg).render_settings();

        assert_eq!(settings.title_top_margin, 33.0);
    }

    #[test]
    fn requirement_text_calculation_keeps_root_font_size_separate_from_theme_font_size() {
        let cfg = json!({
            "fontFamily": "Trebuchet MS, sans-serif",
            "fontSize": 10,
            "themeVariables": {
                "fontFamily": "Courier New, monospace",
                "fontSize": "24px"
            }
        });
        let settings = RequirementConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.font_family, "Courier New,monospace");
        assert_eq!(settings.font_size, 24.0);
        assert_eq!(settings.calculation_font_family, "Trebuchet MS,sans-serif");
        assert_eq!(settings.calculation_font_size, 10.0);
    }
    #[test]
    fn requirement_html_label_modes_follow_node_and_edge_source_precedence() {
        for (config, node_html, edge_html) in [
            (
                json!({"htmlLabels": false, "flowchart": {"htmlLabels": true}}),
                false,
                false,
            ),
            (
                json!({"htmlLabels": true, "flowchart": {"htmlLabels": false}}),
                true,
                true,
            ),
            (json!({"flowchart": {"htmlLabels": false}}), true, false),
        ] {
            let view = RequirementConfigView::new(&config);
            let layout = view.layout_settings();
            let render = view.render_settings();
            assert_eq!(layout.html_labels, node_html);
            assert_eq!(render.html_labels, node_html);
            assert_eq!(layout.edge_html_labels, edge_html);
            assert_eq!(render.edge_html_labels, edge_html);
        }
    }
}
