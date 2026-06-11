use crate::config::{
    config_bool, config_css_number_or_string, config_diagram_look, config_f64, config_f64_css_px,
    config_f64_explicit_css_px, config_string,
};
use crate::text::{TextStyle, WrapMode};
use serde_json::Value;

const DEFAULT_CLASS_FONT_FAMILY: &str = "\"trebuchet ms\", verdana, arial, sans-serif";
const DEFAULT_CLASS_HTML_CALC_FONT_FAMILY: &str = "\"trebuchet ms\", verdana, arial, sans-serif;";

pub(crate) struct ClassConfigView<'a> {
    effective_config: &'a Value,
    flowchart_config: &'a Value,
    class_config: &'a Value,
    theme_variables: &'a Value,
}

impl<'a> ClassConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            effective_config,
            flowchart_config: effective_config.get("flowchart").unwrap_or(&Value::Null),
            class_config: effective_config.get("class").unwrap_or(&Value::Null),
            theme_variables: effective_config
                .get("themeVariables")
                .unwrap_or(&Value::Null),
        }
    }

    fn diagram_config(&self) -> &'a Value {
        self.effective_config
            .get("flowchart")
            .or_else(|| self.effective_config.get("class"))
            .unwrap_or(self.effective_config)
    }

    fn root_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.effective_config, &[key])
    }

    fn flowchart_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.flowchart_config, &[key])
    }

    fn class_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.class_config, &[key])
    }

    fn root_compat_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.effective_config, &[key])
    }

    fn flowchart_compat_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.flowchart_config, &[key])
    }

    fn class_compat_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.class_config, &[key])
    }

    fn class_json_number(&self, key: &str) -> Option<f64> {
        self.class_config.get(key).and_then(Value::as_f64)
    }

    fn theme_string(&self, key: &str) -> Option<String> {
        config_string(self.theme_variables, &[key])
    }

    pub(super) fn layout_settings(&self) -> ClassLayoutSettings {
        let conf = self.diagram_config();
        let nodesep = config_f64(conf, &["nodeSpacing"]).unwrap_or(50.0);
        let ranksep = config_f64(conf, &["rankSpacing"]).unwrap_or(50.0);

        let node_html_labels = self.root_bool("htmlLabels").unwrap_or(true);
        let edge_html_labels = self
            .flowchart_bool("htmlLabels")
            .or_else(|| self.root_bool("htmlLabels"))
            .unwrap_or(true);
        let wrap_mode_node = class_wrap_mode(node_html_labels);
        let wrap_mode_label = class_wrap_mode(edge_html_labels);
        let text_style = self.text_style_for_wrap_mode(wrap_mode_node);
        let html_calc_text_style = self.html_calculate_text_style();

        ClassLayoutSettings {
            nodesep,
            ranksep,
            wrap_mode_node,
            wrap_mode_label,
            wrap_mode_note: wrap_mode_node,
            class_padding: self.class_compat_f64("padding").unwrap_or(12.0),
            namespace_padding: self.flowchart_compat_f64("padding").unwrap_or(15.0),
            hide_empty_members_box: self.class_bool("hideEmptyMembersBox").unwrap_or(false),
            text_style,
            html_calc_text_style,
            wrap_probe_font_size: self.wrap_probe_font_size(),
            title_margin_top: config_f64(
                self.effective_config,
                &["flowchart", "subGraphTitleMargin", "top"],
            )
            .unwrap_or(0.0),
            title_margin_bottom: config_f64(
                self.effective_config,
                &["flowchart", "subGraphTitleMargin", "bottom"],
            )
            .unwrap_or(0.0),
        }
    }

    fn text_style_for_wrap_mode(&self, wrap_mode: WrapMode) -> TextStyle {
        TextStyle {
            font_family: self.text_font_family(),
            font_size: self.font_size_for_wrap_mode(wrap_mode),
            font_weight: None,
        }
    }

    pub(crate) fn html_calculate_text_style(&self) -> TextStyle {
        TextStyle {
            font_family: config_string(self.effective_config, &["fontFamily"])
                .or_else(|| Some(DEFAULT_CLASS_HTML_CALC_FONT_FAMILY.to_string())),
            font_size: config_f64_css_px(self.effective_config, &["fontSize"])
                .unwrap_or(16.0)
                .max(1.0),
            font_weight: None,
        }
    }

    pub(crate) fn render_diagram_html_labels(&self) -> bool {
        self.root_bool("htmlLabels").unwrap_or(true)
    }

    pub(crate) fn render_edge_html_labels(&self) -> bool {
        self.root_bool("htmlLabels")
            .or_else(|| self.flowchart_bool("htmlLabels"))
            .unwrap_or(true)
    }

    pub(crate) fn render_font_size(&self, diagram_use_html_labels: bool) -> f64 {
        if diagram_use_html_labels {
            return 16.0;
        }

        config_f64_explicit_css_px(self.effective_config, &["themeVariables", "fontSize"])
            .unwrap_or(16.0)
            .max(1.0)
    }

    pub(crate) fn render_font_size_css(&self) -> String {
        config_css_number_or_string(self.effective_config, &["themeVariables", "fontSize"])
            .unwrap_or_else(|| "16px".to_string())
    }

    pub(crate) fn wrap_probe_font_size(&self) -> f64 {
        self.root_compat_f64("fontSize").unwrap_or(16.0).max(1.0)
    }

    pub(crate) fn render_class_padding(&self) -> f64 {
        self.class_json_number("padding").unwrap_or(12.0).max(0.0)
    }

    pub(crate) fn render_text_style(&self, font_size: f64) -> TextStyle {
        TextStyle {
            font_family: self.text_font_family(),
            font_size,
            font_weight: None,
        }
    }

    pub(crate) fn render_viewport_padding(&self) -> f64 {
        config_f64(self.diagram_config(), &["diagramPadding"])
            .unwrap_or(8.0)
            .max(0.0)
    }

    pub(crate) fn hide_empty_members_box(&self) -> bool {
        self.class_bool("hideEmptyMembersBox").unwrap_or(false)
    }

    pub(crate) fn default_node_fill(&self) -> String {
        self.theme_string("mainBkg")
            .or_else(|| self.theme_string("primaryColor"))
            .unwrap_or_else(|| "#ECECFF".to_string())
    }

    pub(crate) fn default_node_stroke(&self) -> String {
        self.theme_string("nodeBorder")
            .or_else(|| self.theme_string("primaryBorderColor"))
            .unwrap_or_else(|| "#9370DB".to_string())
    }

    pub(crate) fn diagram_look(&self) -> String {
        config_diagram_look(self.effective_config)
            .as_str()
            .to_string()
    }

    fn text_font_family(&self) -> Option<String> {
        config_string(self.effective_config, &["fontFamily"])
            .or_else(|| self.theme_string("fontFamily"))
            .or_else(|| Some(DEFAULT_CLASS_FONT_FAMILY.to_string()))
    }

    fn font_size_for_wrap_mode(&self, wrap_mode: WrapMode) -> f64 {
        match wrap_mode {
            WrapMode::HtmlLike => 16.0,
            WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => {
                config_f64_explicit_css_px(self.effective_config, &["themeVariables", "fontSize"])
                    .unwrap_or(16.0)
            }
        }
    }
}

pub(super) struct ClassLayoutSettings {
    pub(super) nodesep: f64,
    pub(super) ranksep: f64,
    pub(super) wrap_mode_node: WrapMode,
    pub(super) wrap_mode_label: WrapMode,
    pub(super) wrap_mode_note: WrapMode,
    pub(super) class_padding: f64,
    pub(super) namespace_padding: f64,
    pub(super) hide_empty_members_box: bool,
    pub(super) text_style: TextStyle,
    pub(super) html_calc_text_style: TextStyle,
    pub(super) wrap_probe_font_size: f64,
    pub(super) title_margin_top: f64,
    pub(super) title_margin_bottom: f64,
}

fn class_wrap_mode(html_labels: bool) -> WrapMode {
    if html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn class_layout_settings_preserve_layout_numeric_string_config() {
        let cfg = json!({
            "htmlLabels": false,
            "fontFamily": "Root Sans",
            "fontSize": "18",
            "themeVariables": {
                "fontFamily": "Theme Sans",
                "fontSize": "24px"
            },
            "flowchart": {
                "htmlLabels": true,
                "nodeSpacing": "70",
                "rankSpacing": "80",
                "padding": "17",
                "subGraphTitleMargin": {
                    "top": "3",
                    "bottom": "4"
                }
            },
            "class": {
                "padding": "30",
                "hideEmptyMembersBox": true
            }
        });

        let settings = ClassConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.nodesep, 70.0);
        assert_eq!(settings.ranksep, 80.0);
        assert_eq!(settings.wrap_mode_node, WrapMode::SvgLike);
        assert_eq!(settings.wrap_mode_label, WrapMode::HtmlLike);
        assert_eq!(settings.wrap_mode_note, WrapMode::SvgLike);
        assert_eq!(settings.class_padding, 30.0);
        assert_eq!(settings.namespace_padding, 17.0);
        assert!(settings.hide_empty_members_box);
        assert_eq!(
            settings.text_style.font_family.as_deref(),
            Some("Root Sans")
        );
        assert_eq!(settings.text_style.font_size, 24.0);
        assert_eq!(
            settings.html_calc_text_style.font_family.as_deref(),
            Some("Root Sans")
        );
        assert_eq!(settings.html_calc_text_style.font_size, 18.0);
        assert_eq!(settings.wrap_probe_font_size, 18.0);
        assert_eq!(settings.title_margin_top, 3.0);
        assert_eq!(settings.title_margin_bottom, 4.0);
    }

    #[test]
    fn class_render_settings_preserve_svg_numeric_boundaries() {
        let cfg = json!({
            "htmlLabels": false,
            "fontSize": "18",
            "themeVariables": {
                "fontFamily": "Theme Sans",
                "fontSize": "24px",
                "primaryColor": "#112233",
                "primaryBorderColor": "#445566"
            },
            "flowchart": {
                "htmlLabels": true,
                "diagramPadding": "20"
            },
            "class": {
                "padding": "30"
            }
        });
        let config = ClassConfigView::new(&cfg);

        assert!(!config.render_diagram_html_labels());
        assert!(!config.render_edge_html_labels());
        assert_eq!(config.render_font_size(false), 24.0);
        assert_eq!(config.render_font_size_css(), "24px");
        assert_eq!(config.wrap_probe_font_size(), 18.0);
        assert_eq!(config.render_class_padding(), 12.0);
        assert_eq!(config.render_viewport_padding(), 20.0);
        assert_eq!(config.default_node_fill(), "#112233");
        assert_eq!(config.default_node_stroke(), "#445566");
        assert_eq!(
            config.render_text_style(24.0).font_family.as_deref(),
            Some("Theme Sans")
        );
    }

    #[test]
    fn class_viewport_padding_preserves_flowchart_nullish_precedence() {
        let flowchart_present = json!({
            "flowchart": {},
            "class": {
                "diagramPadding": "31"
            }
        });
        let flowchart_absent = json!({
            "class": {
                "diagramPadding": "31"
            }
        });

        assert_eq!(
            ClassConfigView::new(&flowchart_present).render_viewport_padding(),
            8.0
        );
        assert_eq!(
            ClassConfigView::new(&flowchart_absent).render_viewport_padding(),
            31.0
        );
    }
}
