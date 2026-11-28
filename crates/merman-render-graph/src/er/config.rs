use merman_render_core::config::{
    self, ConfigLookup, config_bool, config_diagram_look, config_f64, config_f64_css_px,
};
use merman_render_core::graph_layout::{Direction, direction_from_rank};
use merman_render_core::text::{TextStyle, WrapMode};
use serde_json::Value;

const DEFAULT_NODE_SPACING: f64 = 140.0;
const DEFAULT_RANK_SPACING: f64 = 80.0;
const DEFAULT_DIAGRAM_PADDING: f64 = 20.0;
const DEFAULT_ENTITY_PADDING: f64 = 15.0;
const DEFAULT_MIN_ENTITY_WIDTH: f64 = 100.0;
const DEFAULT_WRAPPING_WIDTH: f64 = 120.0;
const DEFAULT_FONT_SIZE: f64 = 16.0;
const DEFAULT_RELATIONSHIP_FONT_SIZE: f64 = 14.0;
const DEFAULT_TITLE_TOP_MARGIN: f64 = 25.0;

pub struct ErConfigView<'a, C: ConfigLookup + ?Sized = Value> {
    effective_config: &'a C,
    er_config: &'a Value,
    flowchart_config: &'a Value,
}

impl<'a, C: ConfigLookup + ?Sized> ErConfigView<'a, C> {
    pub fn new(effective_config: &'a C) -> Self {
        Self {
            effective_config,
            er_config: effective_config.get("er").unwrap_or(&Value::Null),
            flowchart_config: effective_config.get("flowchart").unwrap_or(&Value::Null),
        }
    }

    #[cfg(test)]
    pub(super) fn layout_settings_for_test(
        &self,
        direction: &str,
        swimlane: bool,
    ) -> ErLayoutSettings {
        self.layout_settings_with_native_style(
            direction,
            None,
            None,
            self.relationship_html_labels(swimlane),
        )
    }

    pub fn layout_settings_with_native_style(
        &self,
        direction: &str,
        font_family_css: Option<&str>,
        native: Option<&super::ErNativeStyle>,
        relationship_html_labels: bool,
    ) -> ErLayoutSettings {
        let mut settings = self.layout_settings_with_resolved_typography(
            direction,
            native
                .and_then(|style| style.body_family.as_deref())
                .or(font_family_css),
            native.and_then(|style| style.font_size),
            relationship_html_labels,
        );
        if let Some(style) = native {
            style.apply_layout(&mut settings);
        }
        settings
    }

    fn layout_settings_with_resolved_typography(
        &self,
        direction: &str,
        font_family_override: Option<&str>,
        font_size_override: Option<f64>,
        relationship_html_labels: bool,
    ) -> ErLayoutSettings {
        let label_style =
            self.text_style_with_resolved_typography(font_family_override, font_size_override);
        let attr_style = TextStyle {
            font_family: label_style.font_family.clone(),
            font_size: label_style.font_size,
            font_weight: None,
            font_style: None,
        };
        let relationship_label_style = TextStyle {
            font_family: label_style.font_family.clone(),
            // Mermaid ER relationship labels stay at a fixed 14px in the emitted stylesheet.
            font_size: DEFAULT_RELATIONSHIP_FONT_SIZE,
            font_weight: None,
            font_style: None,
        };

        ErLayoutSettings {
            direction: direction_from_rank(direction),
            node_spacing: self.er_spacing("nodeSpacing", DEFAULT_NODE_SPACING),
            rank_spacing: self.er_spacing("rankSpacing", DEFAULT_RANK_SPACING),
            label_style,
            attr_style,
            code_family: None,
            calculation_style: self.calculation_text_style(),
            relationship_label_style,
            relationship_html_labels,
            entity_measurement: self.entity_measurement_settings(),
        }
    }

    pub fn render_settings_with_resolved_typography(
        &self,
        font_family_override: Option<&str>,
        font_size_override: Option<f64>,
    ) -> ErRenderSettings {
        let font_family = font_family_override
            .map(str::to_owned)
            .unwrap_or_else(|| self.font_family_css());
        let font_size = font_size_override.unwrap_or_else(|| self.font_size());
        ErRenderSettings {
            diagram_look: config_diagram_look(self.effective_config)
                .as_str()
                .to_string(),
            font_family: font_family.clone(),
            font_size,
            title_top_margin: self.title_top_margin_with_root_fallback().max(0.0),
            insert_title_top_margin: self.title_top_margin_without_root_fallback(),
            use_max_width: self.er_bool("useMaxWidth").unwrap_or(true),
            label_style: TextStyle {
                font_family: Some(font_family),
                font_size,
                font_weight: None,
                font_style: None,
            },
            entity_html_label_wrap_mode: self.entity_html_label_wrap_mode(),
            entity_measurement: self.entity_measurement_settings(),
            hand_drawn_seed: self
                .effective_config
                .get("handDrawnSeed")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
        }
    }

    pub fn entity_measurement_settings(&self) -> ErEntityMeasurementSettings {
        ErEntityMeasurementSettings {
            html_labels_raw: self.root_bool("htmlLabels").unwrap_or(false),
            label_wrap_mode: self.entity_html_label_wrap_mode(),
            diagram_padding: self
                .er_f64("diagramPadding")
                .unwrap_or(DEFAULT_DIAGRAM_PADDING),
            entity_padding: self
                .er_f64("entityPadding")
                .unwrap_or(DEFAULT_ENTITY_PADDING),
            min_entity_width: self
                .er_f64("minEntityWidth")
                .unwrap_or(DEFAULT_MIN_ENTITY_WIDTH),
            wrapping_width_px: self
                .flowchart_f64("wrappingWidth")
                .unwrap_or(DEFAULT_WRAPPING_WIDTH)
                .round()
                .max(0.0) as i64,
        }
    }

    fn text_style_with_resolved_typography(
        &self,
        font_family_override: Option<&str>,
        font_size_override: Option<f64>,
    ) -> TextStyle {
        TextStyle {
            font_family: Some(
                font_family_override
                    .map(str::to_owned)
                    .unwrap_or_else(|| self.font_family_css()),
            ),
            font_size: font_size_override.unwrap_or_else(|| self.font_size()),
            font_weight: None,
            font_style: None,
        }
    }

    pub fn relationship_html_labels(&self, swimlane: bool) -> bool {
        if swimlane {
            return Self::evaluate_html_labels(self.effective_config.get("htmlLabels"));
        }
        // Mermaid ER relationship labels follow `getEffectiveHtmlLabels(config)` from
        // `rendering-elements/edges.js`:
        // - root `htmlLabels` wins when explicitly set
        // - otherwise `flowchart.htmlLabels` is used
        // - otherwise the default is `true`
        //
        // This intentionally differs from the entity padding quirk, which keys off raw
        // `!config.htmlLabels` in `erBox.ts`.
        self.root_bool("htmlLabels")
            .or_else(|| self.flowchart_bool("htmlLabels"))
            .unwrap_or(true)
    }

    pub fn entity_html_label_wrap_mode(&self) -> WrapMode {
        // Mermaid's ER box painter reads the root config directly. This deliberately does
        // not use `getEffectiveHtmlLabels`: `flowchart.htmlLabels` controls relationship
        // labels, but does not override ER entity, attribute, or subgraph labels.
        if self.root_bool("htmlLabels").unwrap_or(true) {
            WrapMode::HtmlLike
        } else {
            WrapMode::SvgLike
        }
    }

    fn font_family_css(&self) -> String {
        config::config_font_family_css(self.effective_config)
    }

    fn calculation_text_style(&self) -> TextStyle {
        // erBox passes the root config to calculateTextWidth, independently of label CSS.
        TextStyle {
            font_family: Some(config::normalize_css_font_family(
                &config::config_string(self.effective_config, &["fontFamily"])
                    .unwrap_or_else(|| config::MERMAID_DEFAULT_FONT_FAMILY_CSS.to_owned()),
            )),
            font_size: config_f64_css_px(self.effective_config, &["fontSize"])
                .unwrap_or(DEFAULT_FONT_SIZE),
            font_weight: config::config_css_number_or_string(
                self.effective_config,
                &["fontWeight"],
            ),
            font_style: None,
        }
    }

    fn font_size(&self) -> f64 {
        // Mermaid ER unified renderer inherits the root SVG font-size, so
        // `themeVariables.fontSize` wins when present (including Mermaid's common `"NNpx"` form).
        config::config_theme_or_root_font_size_px_opt(self.effective_config)
            .or_else(|| {
                config_f64_css_px(self.er_config, &["fontSize"]).filter(|size| *size >= 0.0)
            })
            .unwrap_or(DEFAULT_FONT_SIZE)
    }

    fn title_top_margin_with_root_fallback(&self) -> f64 {
        self.er_raw_f64("titleTopMargin")
            .or_else(|| {
                self.effective_config
                    .get("titleTopMargin")
                    .and_then(Value::as_f64)
            })
            .unwrap_or(DEFAULT_TITLE_TOP_MARGIN)
    }

    fn title_top_margin_without_root_fallback(&self) -> f64 {
        self.er_raw_f64("titleTopMargin")
            .unwrap_or(DEFAULT_TITLE_TOP_MARGIN)
    }

    pub fn swimlane_relationship_wrapping_width(&self) -> f64 {
        self.flowchart_f64("wrappingWidth")
            .unwrap_or(DEFAULT_WRAPPING_WIDTH)
    }

    pub fn swimlane_title_html_labels(&self) -> bool {
        // The Swimlane cluster shape reads evaluate(flowchart.htmlLabels) directly.
        Self::evaluate_html_labels(self.flowchart_config.get("htmlLabels"))
    }

    pub fn subgraph_html_labels(
        &self,
        is_swimlane_lane: bool,
        relationship_html_labels: bool,
    ) -> bool {
        if is_swimlane_lane {
            self.swimlane_title_html_labels()
        } else {
            relationship_html_labels
        }
    }

    fn evaluate_html_labels(value: Option<&Value>) -> bool {
        match value {
            None => true,
            Some(Value::Bool(false) | Value::Null) => false,
            Some(Value::Number(value)) => value.as_f64() != Some(0.0),
            Some(Value::String(value)) => !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "false" | "null" | "0"
            ),
            Some(_) => true,
        }
    }

    fn root_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.effective_config, &[key])
    }

    fn er_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.er_config, &[key])
    }

    fn flowchart_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.flowchart_config, &[key])
    }

    fn er_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.er_config, &[key])
    }

    fn er_spacing(&self, key: &str, fallback: f64) -> f64 {
        // Mermaid uses `conf[key] || fallback` before Dagre converts strings to numbers.
        self.er_f64(key)
            .filter(|_| self.er_raw_f64(key) != Some(0.0))
            .unwrap_or(fallback)
    }

    fn er_raw_f64(&self, key: &str) -> Option<f64> {
        self.er_config.get(key).and_then(Value::as_f64)
    }

    fn flowchart_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.flowchart_config, &[key])
    }
}

pub struct ErLayoutSettings {
    pub direction: Direction,
    pub node_spacing: f64,
    pub rank_spacing: f64,
    pub label_style: TextStyle,
    pub attr_style: TextStyle,
    pub code_family: Option<String>,
    pub calculation_style: TextStyle,
    pub relationship_label_style: TextStyle,
    pub relationship_html_labels: bool,
    pub entity_measurement: ErEntityMeasurementSettings,
}

#[derive(Debug, Clone, Copy)]
pub struct ErEntityMeasurementSettings {
    pub html_labels_raw: bool,
    pub label_wrap_mode: WrapMode,
    pub diagram_padding: f64,
    pub entity_padding: f64,
    pub min_entity_width: f64,
    pub wrapping_width_px: i64,
}

#[derive(Debug)]
pub struct ErRenderSettings {
    pub diagram_look: String,
    pub font_family: String,
    pub font_size: f64,
    pub title_top_margin: f64,
    pub insert_title_top_margin: f64,
    pub use_max_width: bool,
    pub label_style: TextStyle,
    pub entity_html_label_wrap_mode: WrapMode,
    pub entity_measurement: ErEntityMeasurementSettings,
    pub hand_drawn_seed: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn er_font_family_css_uses_mermaid_default_fallback_for_raw_config() {
        assert_eq!(
            ErConfigView::new(&json!({}))
                .text_style_with_resolved_typography(None, None)
                .font_family
                .as_deref(),
            Some(config::MERMAID_DEFAULT_FONT_FAMILY_CSS)
        );
    }

    #[test]
    fn er_font_family_css_prefers_theme_variables() {
        assert_eq!(
            ErConfigView::new(&json!({
                "fontFamily": "Courier, monospace",
                "themeVariables": {
                    "fontFamily": "\"IBM Plex Sans\", Arial, sans-serif"
                }
            }))
            .text_style_with_resolved_typography(None, None)
            .font_family
            .as_deref(),
            Some(r#""IBM Plex Sans",Arial,sans-serif"#)
        );
    }

    #[test]
    fn er_label_probe_keeps_root_typography_and_flowchart_default_width() {
        let config = json!({
            "fontFamily": "BudgetFont", "fontSize": 10, "fontWeight": 500,
            "themeVariables": {"fontFamily": "VisibleFont", "fontSize": "32px"}
        });
        let settings = ErConfigView::new(&config).layout_settings_for_test("TB", false);
        assert_eq!(
            settings.label_style.font_family.as_deref(),
            Some("VisibleFont")
        );
        assert_eq!(settings.label_style.font_size, 32.0);
        assert_eq!(
            settings.calculation_style.font_family.as_deref(),
            Some("BudgetFont")
        );
        assert_eq!(settings.calculation_style.font_size, 10.0);
        assert_eq!(
            settings.calculation_style.font_weight.as_deref(),
            Some("500")
        );
        assert_eq!(settings.entity_measurement.wrapping_width_px, 120);
    }

    #[test]
    fn er_layout_settings_project_direction_spacing_and_label_styles() {
        let cfg = json!({
            "fontFamily": "Root Sans",
            "themeVariables": {
                "fontSize": "22px"
            },
            "flowchart": {
                "htmlLabels": false
            },
            "er": {
                "nodeSpacing": "160",
                "rankSpacing": 90
            }
        });

        let settings = ErConfigView::new(&cfg).layout_settings_for_test("LR", false);

        assert_eq!(settings.direction, Direction::Right);
        assert_eq!(settings.node_spacing, 160.0);
        assert_eq!(settings.rank_spacing, 90.0);
        assert_eq!(
            settings.label_style.font_family.as_deref(),
            Some("Root Sans")
        );
        assert_eq!(settings.label_style.font_size, 22.0);
        assert_eq!(settings.attr_style.font_size, 22.0);
        assert_eq!(settings.relationship_label_style.font_size, 14.0);
        assert!(!settings.relationship_html_labels);
    }

    #[test]
    fn er_resolved_font_size_preserves_root_fallback_and_relationship_role_owner() {
        let config = json!({
            "fontSize": 10,
            "er": {
                "fontSize": 30
            }
        });
        let view = ErConfigView::new(&config);

        let configured = view.layout_settings_with_resolved_typography("TB", None, None, true);
        assert_eq!(configured.label_style.font_size, 10.0);
        assert_eq!(configured.attr_style.font_size, 10.0);
        assert_eq!(configured.relationship_label_style.font_size, 14.0);

        let typed = view.layout_settings_with_resolved_typography("TB", None, Some(24.0), true);
        assert_eq!(typed.label_style.font_size, 24.0);
        assert_eq!(typed.attr_style.font_size, 24.0);
        assert_eq!(typed.relationship_label_style.font_size, 14.0);

        let er_fallback = ErConfigView::new(&json!({
            "er": {
                "fontSize": 30
            }
        }))
        .layout_settings_with_resolved_typography("TB", None, None, true);
        assert_eq!(er_fallback.label_style.font_size, 30.0);
        assert_eq!(er_fallback.attr_style.font_size, 30.0);
        assert_eq!(er_fallback.relationship_label_style.font_size, 14.0);
    }

    #[test]
    fn er_relationship_html_labels_keep_root_precedence() {
        assert!(ErConfigView::new(&json!({})).relationship_html_labels(false));
        assert!(
            ErConfigView::new(&json!({
                "flowchart": { "htmlLabels": true }
            }))
            .relationship_html_labels(false)
        );
        assert!(
            !ErConfigView::new(&json!({
                "flowchart": { "htmlLabels": false }
            }))
            .relationship_html_labels(false)
        );
        assert!(
            ErConfigView::new(&json!({
                "htmlLabels": true,
                "flowchart": { "htmlLabels": false }
            }))
            .relationship_html_labels(false)
        );
        assert!(
            !ErConfigView::new(&json!({
                "htmlLabels": false,
                "flowchart": { "htmlLabels": true }
            }))
            .relationship_html_labels(false)
        );

        let config = json!({"layout": "swimlane", "flowchart": {"htmlLabels": false}});
        let view = ErConfigView::new(&config);
        assert!(!view.relationship_html_labels(false));
        assert!(view.relationship_html_labels(true));
    }

    #[test]
    fn er_entity_measurement_preserves_padding_quirk_inputs() {
        let cfg = json!({
            "flowchart": {
                "wrappingWidth": "240"
            },
            "er": {
                "diagramPadding": "21",
                "entityPadding": 17,
                "minEntityWidth": "120"
            }
        });

        let settings = ErConfigView::new(&cfg).entity_measurement_settings();

        assert!(!settings.html_labels_raw);
        assert_eq!(settings.label_wrap_mode, WrapMode::HtmlLike);
        assert_eq!(settings.diagram_padding, 21.0);
        assert_eq!(settings.entity_padding, 17.0);
        assert_eq!(settings.min_entity_width, 120.0);
        assert_eq!(settings.wrapping_width_px, 240);
    }

    #[test]
    fn er_render_settings_preserve_svg_numeric_boundaries() {
        let cfg = json!({
            "layout": "elk",
            "look": "handDrawn",
            "handDrawnSeed": 7,
            "htmlLabels": false,
            "titleTopMargin": 33,
            "er": {
                "fontSize": "0px",
                "titleTopMargin": 12,
                "useMaxWidth": false
            }
        });

        let settings = ErConfigView::new(&cfg).render_settings_with_resolved_typography(None, None);

        assert_eq!(settings.diagram_look, "handDrawn");
        assert_eq!(settings.hand_drawn_seed, 7.0);
        assert_eq!(settings.font_size, 0.0);
        assert_eq!(settings.title_top_margin, 12.0);
        assert_eq!(settings.insert_title_top_margin, 12.0);
        assert!(!settings.use_max_width);

        let flowchart_fallback = ErConfigView::new(&json!({
            "flowchart": { "htmlLabels": false }
        }))
        .render_settings_with_resolved_typography(None, None);
        assert_eq!(
            flowchart_fallback.entity_html_label_wrap_mode,
            WrapMode::HtmlLike
        );
        assert_eq!(settings.entity_html_label_wrap_mode, WrapMode::SvgLike);
        assert_eq!(
            settings.entity_measurement.label_wrap_mode,
            WrapMode::SvgLike
        );

        let root_fallback = ErConfigView::new(&json!({
            "titleTopMargin": 33
        }))
        .render_settings_with_resolved_typography(None, None);
        assert_eq!(root_fallback.title_top_margin, 33.0);
        assert_eq!(
            root_fallback.insert_title_top_margin,
            DEFAULT_TITLE_TOP_MARGIN
        );
    }
}
