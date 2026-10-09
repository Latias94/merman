use crate::config::{
    config_bool, config_diagram_look, config_f64, config_f64_css_px, config_f64_explicit_css_px,
    config_string,
};
use crate::text::{TextStyle, WrapMode};
use serde_json::Value;

const DEFAULT_CLASS_FONT_FAMILY: &str = "\"trebuchet ms\", verdana, arial, sans-serif";
const DEFAULT_CLASS_HTML_CALC_FONT_FAMILY: &str = "\"trebuchet ms\", verdana, arial, sans-serif;";

pub(super) struct ClassConfigView<'a> {
    effective_config: &'a Value,
    flowchart_config: &'a Value,
    class_config: &'a Value,
    state_config: &'a Value,
    theme_variables: &'a Value,
}

impl<'a> ClassConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            effective_config,
            flowchart_config: effective_config.get("flowchart").unwrap_or(&Value::Null),
            class_config: effective_config.get("class").unwrap_or(&Value::Null),
            state_config: effective_config.get("state").unwrap_or(&Value::Null),
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

    fn state_compat_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.state_config, &[key])
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

    fn layout_settings(&self) -> ClassLayoutSettings {
        let nodesep = self.layout_spacing("nodeSpacing");
        let ranksep = self.layout_spacing("rankSpacing");

        let node_html_labels = self.render_diagram_html_labels();
        let edge_html_labels = self.render_edge_html_labels();
        let wrap_mode_node = class_wrap_mode(node_html_labels);
        let wrap_mode_label = class_wrap_mode(edge_html_labels);
        let text_style = self.render_text_style(self.render_font_size());
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

    fn layout_spacing(&self, key: &str) -> f64 {
        let renderer_spacing = self
            .state_compat_f64(key)
            .filter(|value| *value != 0.0)
            .unwrap_or(50.0);
        [
            self.root_compat_f64(key),
            Some(renderer_spacing),
            self.flowchart_compat_f64(key),
        ]
        .into_iter()
        .flatten()
        .find(|value| *value != 0.0)
        .unwrap_or(50.0)
    }

    fn html_calculate_text_style(&self) -> TextStyle {
        TextStyle {
            font_family: config_string(self.effective_config, &["fontFamily"])
                .or_else(|| Some(DEFAULT_CLASS_HTML_CALC_FONT_FAMILY.to_string())),
            font_size: config_f64_css_px(self.effective_config, &["fontSize"])
                .unwrap_or(16.0)
                .max(1.0),
            font_weight: None,
            font_style: None,
        }
    }

    fn interface_wrapping_width(&self) -> f64 {
        self.flowchart_compat_f64("wrappingWidth")
            .filter(|width| width.is_finite() && *width > 0.0)
            .unwrap_or(crate::text::MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX)
    }

    fn render_diagram_html_labels(&self) -> bool {
        self.root_bool("htmlLabels").unwrap_or(true)
    }

    fn render_edge_html_labels(&self) -> bool {
        self.root_bool("htmlLabels")
            .or_else(|| self.flowchart_bool("htmlLabels"))
            .unwrap_or(true)
    }

    fn render_font_size(&self) -> f64 {
        // HTML labels inherit the same root SVG theme font size as SVG labels.
        config_f64_explicit_css_px(self.effective_config, &["themeVariables", "fontSize"])
            .unwrap_or(16.0)
            .max(1.0)
    }

    fn wrap_probe_font_size(&self) -> f64 {
        self.root_compat_f64("fontSize").unwrap_or(16.0).max(1.0)
    }

    fn render_class_padding(&self) -> f64 {
        self.class_json_number("padding").unwrap_or(12.0).max(0.0)
    }

    fn render_text_style(&self, font_size: f64) -> TextStyle {
        TextStyle {
            font_family: self.text_font_family(),
            font_size,
            font_weight: None,
            font_style: None,
        }
    }

    fn render_viewport_padding(&self) -> f64 {
        config_f64(self.diagram_config(), &["diagramPadding"])
            .unwrap_or(8.0)
            .max(0.0)
    }

    fn diagram_look(&self) -> String {
        config_diagram_look(self.effective_config)
            .as_str()
            .to_string()
    }

    fn text_font_family(&self) -> Option<String> {
        config_string(self.effective_config, &["fontFamily"])
            .or_else(|| self.theme_string("fontFamily"))
            .or_else(|| Some(DEFAULT_CLASS_FONT_FAMILY.to_string()))
    }

    pub(super) fn layout_font_family_css(&self) -> String {
        let font_family = crate::config::normalize_css_font_family(
            self.text_font_family().unwrap_or_default().as_str(),
        );
        if font_family.is_empty() {
            DEFAULT_CLASS_FONT_FAMILY.to_string()
        } else {
            font_family
        }
    }
}

/// Operation-owned configuration shared by layout and terminal SVG emission.
#[derive(Debug)]
pub(crate) struct ClassRenderConfig {
    layout: ClassLayoutSettings,
    pub(crate) diagram_use_html_labels: bool,
    pub(crate) edge_use_html_labels: bool,
    pub(crate) line_height: f64,
    pub(crate) class_padding: f64,
    pub(crate) viewport_padding: f64,
    pub(crate) security_level_loose: bool,
    pub(crate) look: String,
    pub(crate) interface_wrapping_width: f64,
    pub(crate) hand_drawn_seed: Option<f64>,
}

impl ClassRenderConfig {
    pub(crate) fn resolve(
        effective_config: &merman_core::MermaidConfig,
        typography: &super::ClassTextThemePlan,
    ) -> Self {
        let effective_config = effective_config.as_value();
        let config = ClassConfigView::new(effective_config);
        let mut layout = config.layout_settings();
        typography
            .apply_layout_text_styles(&mut layout.text_style, &mut layout.html_calc_text_style);
        let line_height = layout.text_style.font_size * 1.5;
        let diagram_use_html_labels = matches!(layout.wrap_mode_node, WrapMode::HtmlLike);
        let edge_use_html_labels = matches!(layout.wrap_mode_label, WrapMode::HtmlLike);
        Self {
            layout,
            diagram_use_html_labels,
            edge_use_html_labels,
            line_height,
            class_padding: config.render_class_padding(),
            viewport_padding: config.render_viewport_padding(),
            security_level_loose: effective_config
                .get("securityLevel")
                .and_then(Value::as_str)
                == Some("loose"),
            look: config.diagram_look(),
            interface_wrapping_width: config.interface_wrapping_width(),
            hand_drawn_seed: effective_config
                .get("handDrawnSeed")
                .and_then(Value::as_f64),
        }
    }

    pub(super) const fn layout_settings(&self) -> &ClassLayoutSettings {
        &self.layout
    }

    pub(crate) const fn text_style(&self) -> &TextStyle {
        &self.layout.text_style
    }

    pub(crate) const fn html_calc_text_style(&self) -> &TextStyle {
        &self.layout.html_calc_text_style
    }

    pub(crate) const fn wrap_probe_font_size(&self) -> f64 {
        self.layout.wrap_probe_font_size
    }

    pub(crate) const fn hide_empty_members_box(&self) -> bool {
        self.layout.hide_empty_members_box
    }
}

#[derive(Debug)]
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
    fn prepared_config_preserves_font_probe_and_padding_domains() {
        let source = merman_core::MermaidConfig::from_value(json!({
            "fontFamily": "Root Sans, monospace",
            "fontSize": "18",
            "htmlLabels": false,
            "themeVariables": { "fontFamily": "Theme Sans, sans-serif", "fontSize": "24px" },
            "flowchart": { "htmlLabels": true, "wrappingWidth": "210", "diagramPadding": "20" },
            "class": { "padding": "30", "hideEmptyMembersBox": true },
            "look": "neo"
        }));
        let typography = super::super::ClassTextThemePlan::resolve(None, &source);
        let config = ClassRenderConfig::resolve(&source, &typography);
        assert_ne!(
            typography.layout_font_family_css(),
            typography.stylesheet_font_family_css()
        );
        assert_eq!(
            config.text_style().font_family.as_deref(),
            Some(typography.layout_font_family_css())
        );
        assert!(std::ptr::eq(
            config.text_style(),
            &config.layout_settings().text_style
        ));
        assert!(std::ptr::eq(
            config.html_calc_text_style(),
            &config.layout_settings().html_calc_text_style
        ));
        assert_eq!(config.text_style().font_size, 24.0);
        assert_eq!(config.html_calc_text_style().font_size, 18.0);
        assert_eq!(config.wrap_probe_font_size(), 18.0);
        assert_eq!(config.layout_settings().class_padding, 30.0);
        assert_eq!(config.class_padding, 12.0);
        assert_eq!(config.viewport_padding, 20.0);
        assert!(!config.diagram_use_html_labels);
        assert!(!config.edge_use_html_labels);
        assert_eq!(config.interface_wrapping_width, 210.0);
        assert_eq!(config.look, "neo");
        assert!(config.hide_empty_members_box());
    }

    #[test]
    fn typed_font_converges_main_styles_without_changing_html_probe_size() {
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
        };
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_typography(
                    TypographySpec::default().with_family_style(
                        crate::DiagramFamilyId::CLASS,
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("monospace").unwrap())
                            .with_font_size_px(26.0)
                            .unwrap(),
                    ),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::CLASS);
        let source = merman_core::MermaidConfig::from_value(json!({ "fontSize": "18" }));
        let typography = super::super::ClassTextThemePlan::resolve(Some(&theme), &source);
        let config = ClassRenderConfig::resolve(&source, &typography);
        assert!(typography.typed_font_size_active());
        assert_eq!(
            typography.layout_font_family_css(),
            typography.stylesheet_font_family_css()
        );
        assert_eq!(
            config.text_style().font_family.as_deref(),
            Some("monospace")
        );
        assert_eq!(
            config.html_calc_text_style().font_family,
            config.text_style().font_family
        );
        assert_eq!(config.text_style().font_size, 26.0);
        assert_eq!(config.line_height, 39.0);
        assert_eq!(config.html_calc_text_style().font_size, 18.0);
        assert_eq!(config.wrap_probe_font_size(), 18.0);
    }

    #[test]
    fn prepared_interface_width_and_seed_keep_existing_coercion_boundaries() {
        for (width, expected) in [
            (json!("200"), 200.0),
            (json!(0), crate::text::MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX),
            (json!(-2), crate::text::MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX),
            (
                json!("Infinity"),
                crate::text::MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX,
            ),
        ] {
            let source = merman_core::MermaidConfig::from_value(
                json!({ "flowchart": { "wrappingWidth": width } }),
            );
            let typography = super::super::ClassTextThemePlan::resolve(None, &source);
            let config = ClassRenderConfig::resolve(&source, &typography);
            assert_eq!(config.interface_wrapping_width, expected);
            assert_eq!(config.hand_drawn_seed, None);
        }
        for (seed, expected) in [
            (json!(-7.5), Some(-7.5)),
            (json!("17"), None),
            (json!(null), None),
        ] {
            let source = merman_core::MermaidConfig::from_value(json!({ "handDrawnSeed": seed }));
            let typography = super::super::ClassTextThemePlan::resolve(None, &source);
            let config = ClassRenderConfig::resolve(&source, &typography);
            assert_eq!(config.hand_drawn_seed, expected);
        }
    }

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
                "nodeSpacing": "11",
                "rankSpacing": "12",
                "padding": "17",
                "subGraphTitleMargin": {
                    "top": "3",
                    "bottom": "4"
                }
            },
            "state": {
                "nodeSpacing": "70",
                "rankSpacing": "80"
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
        assert_eq!(settings.wrap_mode_label, WrapMode::SvgLike);
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
    fn class_edge_html_labels_use_root_config_before_deprecated_flowchart_fallback() {
        for (cfg, expected) in [
            (
                json!({
                    "htmlLabels": false,
                    "flowchart": { "htmlLabels": true }
                }),
                false,
            ),
            (
                json!({
                    "htmlLabels": true,
                    "flowchart": { "htmlLabels": false }
                }),
                true,
            ),
            (json!({ "flowchart": { "htmlLabels": false } }), false),
        ] {
            let config = ClassConfigView::new(&cfg);
            let expected_wrap_mode = class_wrap_mode(expected);

            assert_eq!(config.layout_settings().wrap_mode_label, expected_wrap_mode);
            assert_eq!(config.render_edge_html_labels(), expected);
        }
    }

    #[test]
    fn class_layout_font_family_rejects_empty_or_unsafe_config_values() {
        for font_family in ["", " ; ", "url(javascript:alert(1))"] {
            let cfg = json!({ "fontFamily": font_family });
            assert_eq!(
                ClassConfigView::new(&cfg).layout_font_family_css(),
                DEFAULT_CLASS_FONT_FAMILY,
                "fontFamily={font_family:?}"
            );
        }
    }

    #[test]
    fn class_layout_settings_use_root_spacing_before_renderer_spacing() {
        let cfg = json!({
            "nodeSpacing": 90,
            "rankSpacing": "91",
            "state": {
                "nodeSpacing": 70,
                "rankSpacing": 80
            },
            "flowchart": {
                "nodeSpacing": 11,
                "rankSpacing": 12
            }
        });
        let settings = ClassConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.nodesep, 90.0);
        assert_eq!(settings.ranksep, 91.0);
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
        assert_eq!(config.render_font_size(), 24.0);
        let typography = crate::class::theme::ClassTextThemePlan::resolve(
            None,
            &merman_core::MermaidConfig::from_value(cfg.clone()),
        );
        assert_eq!(typography.font_size_css(), "24px");
        assert_eq!(config.wrap_probe_font_size(), 18.0);
        assert_eq!(config.render_class_padding(), 12.0);
        assert_eq!(config.render_viewport_padding(), 20.0);
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
