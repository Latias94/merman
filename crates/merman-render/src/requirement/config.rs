use crate::config::{
    DiagramLook, config_bool, config_diagram_look, config_f64,
    config_font_family_or_first_array_css, config_theme_or_root_font_size_px,
};
use serde_json::Value;

const DEFAULT_NODE_SPACING: f64 = 50.0;
const DEFAULT_RANK_SPACING: f64 = 50.0;
const DEFAULT_FONT_SIZE: f64 = 16.0;
const DEFAULT_VIEWPORT_PADDING: f64 = 8.0;
const DEFAULT_USE_MAX_WIDTH: bool = true;

pub(crate) struct RequirementConfigView<'a> {
    effective_config: &'a Value,
    requirement_config: &'a Value,
}

impl<'a> RequirementConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            effective_config,
            requirement_config: effective_config.get("requirement").unwrap_or(&Value::Null),
        }
    }

    pub(crate) fn layout_settings(&self) -> RequirementLayoutSettings {
        RequirementLayoutSettings {
            nodesep: self
                .root_f64("nodeSpacing")
                .or_else(|| self.config_f64(&["flowchart", "nodeSpacing"]))
                .unwrap_or(DEFAULT_NODE_SPACING),
            ranksep: self
                .root_f64("rankSpacing")
                .or_else(|| self.config_f64(&["flowchart", "rankSpacing"]))
                .unwrap_or(DEFAULT_RANK_SPACING),
            font_family: self.font_family(),
            font_size: self.font_size(),
        }
    }

    pub(crate) fn render_settings(&self) -> RequirementRenderSettings<'a> {
        RequirementRenderSettings {
            look: config_diagram_look(self.effective_config),
            viewport_padding: DEFAULT_VIEWPORT_PADDING,
            use_max_width: self
                .requirement_bool("useMaxWidth")
                .unwrap_or(DEFAULT_USE_MAX_WIDTH),
            hand_drawn_seed: self
                .effective_config
                .get("handDrawnSeed")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            font_family: self.font_family(),
            font_size: self.font_size(),
        }
    }

    fn config_f64(&self, path: &[&str]) -> Option<f64> {
        config_f64(self.effective_config, path)
    }

    fn root_f64(&self, key: &str) -> Option<f64> {
        self.config_f64(&[key])
    }

    fn requirement_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.requirement_config, &[key])
    }

    fn font_family(&self) -> String {
        config_font_family_or_first_array_css(self.effective_config)
    }

    fn font_size(&self) -> f64 {
        config_theme_or_root_font_size_px(self.effective_config, DEFAULT_FONT_SIZE)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RequirementLayoutSettings {
    pub(crate) nodesep: f64,
    pub(crate) ranksep: f64,
    pub(crate) font_family: String,
    pub(crate) font_size: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct RequirementRenderSettings<'a> {
    pub(crate) look: DiagramLook<'a>,
    pub(crate) viewport_padding: f64,
    pub(crate) use_max_width: bool,
    pub(crate) hand_drawn_seed: u64,
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
            crate::config::MERMAID_DEFAULT_FONT_FAMILY_CSS
        );
        assert_eq!(settings.font_size, DEFAULT_FONT_SIZE);
    }

    #[test]
    fn requirement_layout_settings_keep_root_spacing_precedence() {
        let cfg = json!({
            "nodeSpacing": "0",
            "rankSpacing": 70,
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

        assert_eq!(settings.nodesep, 0.0);
        assert_eq!(settings.ranksep, 70.0);
        assert_eq!(settings.font_family, "Courier New");
        assert_eq!(settings.font_size, 18.0);
    }

    #[test]
    fn requirement_layout_settings_fall_back_to_flowchart_spacing() {
        let cfg = json!({
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
            "requirement": {
                "useMaxWidth": false
            }
        });
        let settings = RequirementConfigView::new(&cfg).render_settings();

        assert_eq!(settings.look.as_str(), "neo");
        assert_eq!(settings.viewport_padding, DEFAULT_VIEWPORT_PADDING);
        assert!(!settings.use_max_width);
        assert_eq!(settings.hand_drawn_seed, 7);
        assert_eq!(settings.font_family, "Inter,sans-serif");
        assert_eq!(settings.font_size, 20.0);
    }
}
