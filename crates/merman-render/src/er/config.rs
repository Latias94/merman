use crate::config::{config_bool, config_diagram_look, config_f64, config_f64_css_px};
use crate::layout_backend::{GraphLayoutBackend, resolve_graph_layout};
use crate::text::{TextStyle, WrapMode};
use dugong::{GraphLabel, RankDir};
use serde_json::Value;

const DEFAULT_NODE_SPACING: f64 = 140.0;
const DEFAULT_RANK_SPACING: f64 = 80.0;
const DEFAULT_DIAGRAM_PADDING: f64 = 20.0;
const DEFAULT_ENTITY_PADDING: f64 = 15.0;
const DEFAULT_MIN_ENTITY_WIDTH: f64 = 100.0;
const DEFAULT_WRAPPING_WIDTH: f64 = 200.0;
const DEFAULT_FONT_SIZE: f64 = 16.0;
const DEFAULT_RELATIONSHIP_FONT_SIZE: f64 = 14.0;
const DEFAULT_TITLE_TOP_MARGIN: f64 = 25.0;

pub(crate) struct ErConfigView<'a> {
    effective_config: &'a Value,
    er_config: &'a Value,
    flowchart_config: &'a Value,
}

impl<'a> ErConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            effective_config,
            er_config: effective_config.get("er").unwrap_or(&Value::Null),
            flowchart_config: effective_config.get("flowchart").unwrap_or(&Value::Null),
        }
    }

    #[cfg(test)]
    pub(super) fn layout_settings_with_font_family(
        &self,
        direction: &str,
        font_family_override: Option<&str>,
    ) -> ErLayoutSettings {
        self.layout_settings_with_resolved_typography(direction, font_family_override, None)
    }

    pub(super) fn layout_settings_with_resolved_typography(
        &self,
        direction: &str,
        font_family_override: Option<&str>,
        font_size_override: Option<f64>,
    ) -> ErLayoutSettings {
        let label_style =
            self.text_style_with_resolved_typography(font_family_override, font_size_override);
        let attr_style = TextStyle {
            font_family: label_style.font_family.clone(),
            font_size: label_style.font_size.max(1.0),
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
            graph: GraphLabel {
                rankdir: rank_dir_from(direction),
                nodesep: self.er_f64("nodeSpacing").unwrap_or(DEFAULT_NODE_SPACING),
                ranksep: self.er_f64("rankSpacing").unwrap_or(DEFAULT_RANK_SPACING),
                // Mermaid's unified Dagre adapter lays out inside an 8px graph margin, then its
                // root viewport adds the same 8px around the emitted bbox.
                marginx: 8.0,
                marginy: 8.0,
                ..Default::default()
            },
            label_style,
            attr_style,
            relationship_label_style,
            relationship_html_labels: self.relationship_html_labels(),
            entity_measurement: self.entity_measurement_settings(),
        }
    }

    #[cfg(test)]
    pub(crate) fn render_settings_with_font_family(
        &self,
        font_family_override: Option<&str>,
    ) -> ErRenderSettings {
        self.render_settings_with_resolved_typography(font_family_override, None)
    }

    pub(crate) fn render_settings_with_resolved_typography(
        &self,
        font_family_override: Option<&str>,
        font_size_override: Option<f64>,
    ) -> ErRenderSettings {
        let font_family = font_family_override
            .map(str::to_owned)
            .unwrap_or_else(|| self.font_family_css());
        let font_size = font_size_override
            .unwrap_or_else(|| self.font_size())
            .max(1.0);
        ErRenderSettings {
            is_elk_layout: self.is_elk_layout(),
            diagram_look: config_diagram_look(self.effective_config)
                .as_str()
                .to_string(),
            font_family: font_family.clone(),
            font_size,
            title_top_margin: self.title_top_margin_with_root_fallback().max(0.0),
            insert_title_top_margin: self.title_top_margin_without_root_fallback(),
            use_max_width: self.er_bool("useMaxWidth").unwrap_or(true),
            label_style: TextStyle {
                font_family: Some(font_family.clone()),
                font_size,
                font_weight: None,
                font_style: None,
            },
            attr_style: TextStyle {
                font_family: Some(font_family),
                font_size,
                font_weight: None,
                font_style: None,
            },
            relationship_html_labels: self.relationship_html_labels(),
            entity_html_label_wrap_mode: self.entity_html_label_wrap_mode(),
            entity_measurement: self.entity_measurement_settings(),
            hand_drawn_seed: self
                .effective_config
                .get("handDrawnSeed")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
        }
    }

    pub(crate) fn entity_measurement_settings(&self) -> ErEntityMeasurementSettings {
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

    #[cfg(test)]
    pub(crate) fn text_style_with_font_family(
        &self,
        font_family_override: Option<&str>,
    ) -> TextStyle {
        self.text_style_with_resolved_typography(font_family_override, None)
    }

    pub(crate) fn text_style_with_resolved_typography(
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
            font_size: font_size_override
                .unwrap_or_else(|| self.font_size())
                .max(1.0),
            font_weight: None,
            font_style: None,
        }
    }

    pub(crate) fn relationship_html_labels(&self) -> bool {
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

    pub(crate) fn entity_html_label_wrap_mode(&self) -> WrapMode {
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
        crate::config::config_font_family_css(self.effective_config)
    }

    fn font_size(&self) -> f64 {
        // Mermaid ER unified renderer inherits the root SVG font-size, so
        // `themeVariables.fontSize` wins when present (including Mermaid's common `"NNpx"` form).
        crate::config::config_theme_or_root_font_size_px_opt(self.effective_config)
            .or_else(|| config_f64_css_px(self.er_config, &["fontSize"]))
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

    pub(crate) fn is_elk_layout(&self) -> bool {
        resolve_graph_layout(self.effective_config).backend == GraphLayoutBackend::Elk
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

    fn er_raw_f64(&self, key: &str) -> Option<f64> {
        self.er_config.get(key).and_then(Value::as_f64)
    }

    fn flowchart_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.flowchart_config, &[key])
    }
}

pub(super) struct ErLayoutSettings {
    pub(super) graph: GraphLabel,
    pub(super) label_style: TextStyle,
    pub(super) attr_style: TextStyle,
    pub(super) relationship_label_style: TextStyle,
    pub(super) relationship_html_labels: bool,
    pub(super) entity_measurement: ErEntityMeasurementSettings,
}

#[derive(Clone, Copy)]
pub(crate) struct ErEntityMeasurementSettings {
    pub(crate) html_labels_raw: bool,
    pub(crate) label_wrap_mode: WrapMode,
    pub(crate) diagram_padding: f64,
    pub(crate) entity_padding: f64,
    pub(crate) min_entity_width: f64,
    pub(crate) wrapping_width_px: i64,
}

pub(crate) struct ErRenderSettings {
    pub(crate) is_elk_layout: bool,
    pub(crate) diagram_look: String,
    pub(crate) font_family: String,
    pub(crate) font_size: f64,
    pub(crate) title_top_margin: f64,
    pub(crate) insert_title_top_margin: f64,
    pub(crate) use_max_width: bool,
    pub(crate) label_style: TextStyle,
    pub(crate) attr_style: TextStyle,
    pub(crate) relationship_html_labels: bool,
    pub(crate) entity_html_label_wrap_mode: WrapMode,
    pub(crate) entity_measurement: ErEntityMeasurementSettings,
    pub(crate) hand_drawn_seed: f64,
}

pub(super) fn rank_dir_from(direction: &str) -> RankDir {
    match normalize_dir(direction).as_str() {
        "TB" => RankDir::TB,
        "BT" => RankDir::BT,
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        _ => RankDir::TB,
    }
}

fn normalize_dir(direction: &str) -> String {
    match direction.trim().to_uppercase().as_str() {
        "TB" | "TD" => "TB".to_string(),
        "BT" => "BT".to_string(),
        "LR" => "LR".to_string(),
        "RL" => "RL".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn er_layout_settings_project_dagre_text_and_relationship_labels() {
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

        let settings = ErConfigView::new(&cfg).layout_settings_with_font_family("LR", None);

        assert!(!ErConfigView::new(&cfg).is_elk_layout());
        assert_eq!(settings.graph.rankdir, RankDir::LR);
        assert_eq!(settings.graph.nodesep, 160.0);
        assert_eq!(settings.graph.ranksep, 90.0);
        assert_eq!(settings.graph.marginx, 8.0);
        assert_eq!(settings.graph.marginy, 8.0);
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

        let configured = view.layout_settings_with_resolved_typography("TB", None, None);
        assert_eq!(configured.label_style.font_size, 10.0);
        assert_eq!(configured.attr_style.font_size, 10.0);
        assert_eq!(configured.relationship_label_style.font_size, 14.0);

        let typed = view.layout_settings_with_resolved_typography("TB", None, Some(24.0));
        assert_eq!(typed.label_style.font_size, 24.0);
        assert_eq!(typed.attr_style.font_size, 24.0);
        assert_eq!(typed.relationship_label_style.font_size, 14.0);

        let er_fallback = ErConfigView::new(&json!({
            "er": {
                "fontSize": 30
            }
        }))
        .layout_settings_with_resolved_typography("TB", None, None);
        assert_eq!(er_fallback.label_style.font_size, 30.0);
        assert_eq!(er_fallback.attr_style.font_size, 30.0);
        assert_eq!(er_fallback.relationship_label_style.font_size, 14.0);
    }

    #[test]
    fn er_relationship_html_labels_keep_root_precedence() {
        assert!(ErConfigView::new(&json!({})).relationship_html_labels());
        assert!(
            ErConfigView::new(&json!({
                "flowchart": { "htmlLabels": true }
            }))
            .relationship_html_labels()
        );
        assert!(
            !ErConfigView::new(&json!({
                "flowchart": { "htmlLabels": false }
            }))
            .relationship_html_labels()
        );
        assert!(
            ErConfigView::new(&json!({
                "htmlLabels": true,
                "flowchart": { "htmlLabels": false }
            }))
            .relationship_html_labels()
        );
        assert!(
            !ErConfigView::new(&json!({
                "htmlLabels": false,
                "flowchart": { "htmlLabels": true }
            }))
            .relationship_html_labels()
        );
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

        let settings = ErConfigView::new(&cfg).render_settings_with_font_family(None);

        assert_eq!(settings.is_elk_layout, cfg!(feature = "layout-elk"));
        assert_eq!(settings.diagram_look, "handDrawn");
        assert_eq!(settings.hand_drawn_seed, 7.0);
        assert_eq!(settings.font_size, 1.0);
        assert_eq!(settings.title_top_margin, 12.0);
        assert_eq!(settings.insert_title_top_margin, 12.0);
        assert!(!settings.use_max_width);
        assert!(!settings.relationship_html_labels);

        let flowchart_fallback = ErConfigView::new(&json!({
            "flowchart": { "htmlLabels": false }
        }))
        .render_settings_with_font_family(None);
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
        .render_settings_with_font_family(None);
        assert_eq!(root_fallback.title_top_margin, 33.0);
        assert_eq!(
            root_fallback.insert_title_top_margin,
            DEFAULT_TITLE_TOP_MARGIN
        );
    }
}
