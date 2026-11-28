use merman_render_core::config::{
    ConfigLookup, config_bool, config_f64, config_string, config_string_vec,
};
use merman_render_core::mermaid_style::font_family_css;
use merman_render_core::text::TextStyle;
use merman_theme_contract::v2::FamilyTheme;
use serde_json::Value;

use super::JourneyNativeStyle;

const DEFAULT_LEFT_MARGIN: f64 = 150.0;
const DEFAULT_MAX_LABEL_WIDTH: f64 = 360.0;
const DEFAULT_BOX_TEXT_MARGIN: f64 = 5.0;
const DEFAULT_DIAGRAM_MARGIN_X: f64 = 50.0;
const DEFAULT_DIAGRAM_MARGIN_Y: f64 = 10.0;
const DEFAULT_TASK_MARGIN: f64 = 50.0;
const DEFAULT_CELL_WIDTH: f64 = 150.0;
const DEFAULT_CELL_HEIGHT: f64 = 50.0;
const DEFAULT_TASK_FONT_SIZE: f64 = 14.0;
const DEFAULT_TASK_FONT_FAMILY: &str = "\"Open Sans\", sans-serif";
const DEFAULT_TITLE_FONT_FAMILY: &str = "\"trebuchet ms\", verdana, arial, sans-serif";
const DEFAULT_TITLE_FONT_SIZE: &str = "4ex";
const DEFAULT_TITLE_COLOR: &str = "";
const DEFAULT_USE_MAX_WIDTH: bool = true;
const DEFAULT_ACTOR_COLOURS: [&str; 6] = [
    "#8FBC8F", "#7CFC00", "#00FFFF", "#20B2AA", "#B0E0E6", "#FFFFE0",
];
const DEFAULT_SECTION_FILLS: [&str; 7] = [
    "#191970", "#8B008B", "#4B0082", "#2F4F4F", "#800000", "#8B4513", "#00008B",
];

pub(crate) struct JourneyConfigView<'a> {
    journey_config: &'a Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JourneyTextPlacement {
    Fo,
    Old,
    Tspan,
}

impl<'a> JourneyConfigView<'a> {
    pub(crate) fn new(effective_config: &'a (impl ConfigLookup + ?Sized)) -> Self {
        Self {
            journey_config: effective_config.get("journey").unwrap_or(&Value::Null),
        }
    }

    pub(super) fn layout_settings(
        &self,
        native: Option<&JourneyNativeStyle>,
    ) -> JourneyLayoutSettings {
        let (section_fills, section_fill_count) =
            if native.is_none_or(|style| style.section_fills_owned) {
                let fills = self.section_fills();
                let count = fills.len();
                (fills, count)
            } else {
                // The configured palette still determines Mermaid's section class and fillType slot.
                let count = self
                    .journey_config
                    .get("sectionFills")
                    .and_then(Value::as_array)
                    .map_or(0, |fills| {
                        fills.iter().filter(|fill| fill.is_string()).count()
                    });
                (
                    Vec::new(),
                    if count == 0 {
                        DEFAULT_SECTION_FILLS.len()
                    } else {
                        count
                    },
                )
            };
        JourneyLayoutSettings {
            left_margin_base: self
                .journey_f64("leftMargin")
                .unwrap_or(DEFAULT_LEFT_MARGIN)
                .max(0.0),
            max_label_width: self
                .journey_f64("maxLabelWidth")
                .unwrap_or(DEFAULT_MAX_LABEL_WIDTH)
                .max(1.0),
            box_text_margin: self
                .journey_f64("boxTextMargin")
                .unwrap_or(DEFAULT_BOX_TEXT_MARGIN)
                .max(0.0),
            diagram_margin_x: self
                .journey_f64("diagramMarginX")
                .unwrap_or(DEFAULT_DIAGRAM_MARGIN_X)
                .max(0.0),
            diagram_margin_y: self
                .journey_f64("diagramMarginY")
                .unwrap_or(DEFAULT_DIAGRAM_MARGIN_Y)
                .max(0.0),
            task_margin: self
                .journey_f64("taskMargin")
                .unwrap_or(DEFAULT_TASK_MARGIN)
                .max(0.0),
            cell_width: self.cell_dimension("width", DEFAULT_CELL_WIDTH),
            cell_height: self.cell_dimension("height", DEFAULT_CELL_HEIGHT),
            actor_colours: if native.is_none_or(|style| style.actor_colors_owned) {
                self.actor_colours()
            } else {
                Vec::new()
            },
            section_fills,
            section_fill_count,
            use_max_width: self.use_max_width(),
        }
    }

    pub(crate) fn render_settings(&self) -> JourneyRenderSettings {
        JourneyRenderSettings {
            text_placement: self.text_placement(),
            task_text_style: self.task_text_style(),
            section_colours: self.section_colours(),
            title_font_size: self.journey_string("titleFontSize", DEFAULT_TITLE_FONT_SIZE),
            title_font_family: self.journey_string("titleFontFamily", DEFAULT_TITLE_FONT_FAMILY),
            title_color: self.journey_string("titleColor", DEFAULT_TITLE_COLOR),
        }
    }

    pub(super) fn native_render_settings(
        &self,
        theme: &FamilyTheme,
        body_font: &str,
        font_size_px: f64,
    ) -> JourneyRenderSettings {
        let typography = &theme.typography;
        JourneyRenderSettings {
            text_placement: self.text_placement(),
            task_text_style: TextStyle {
                font_family: Some(body_font.to_owned()),
                font_size: font_size_px,
                font_weight: None,
                font_style: None,
            },
            section_colours: self.section_colours(),
            title_font_family: font_family_css(typography.stack(typography.title_family)),
            title_font_size: format!("{}px", font_size_px * 1.9375),
            title_color: theme.roles.text_title.to_string(),
        }
    }

    fn section_colours(&self) -> Vec<String> {
        if self.journey_config.get("sectionColours").is_none() {
            vec!["#fff".to_owned()]
        } else {
            config_string_vec(self.journey_config, &["sectionColours"])
        }
    }

    fn journey_string(&self, key: &str, fallback: &str) -> String {
        config_string(self.journey_config, &[key]).unwrap_or_else(|| fallback.to_owned())
    }

    fn text_placement(&self) -> JourneyTextPlacement {
        // Mermaid's schema defaults to `fo`; svgDraw's selector uses exact string equality.
        match self.journey_config.get("textPlacement") {
            None => JourneyTextPlacement::Fo,
            Some(Value::String(value)) if value == "fo" => JourneyTextPlacement::Fo,
            Some(Value::String(value)) if value == "old" => JourneyTextPlacement::Old,
            Some(_) => JourneyTextPlacement::Tspan,
        }
    }

    fn journey_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.journey_config, &[key])
    }

    fn cell_dimension(&self, key: &str, default: f64) -> f64 {
        let value = self.journey_f64(key).unwrap_or(default);
        if value == 0.0 { 0.0 } else { value.max(1.0) }
    }

    fn journey_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.journey_config, &[key])
    }

    fn actor_colours(&self) -> Vec<String> {
        let actor_colours = config_string_vec(self.journey_config, &["actorColours"]);
        if actor_colours.is_empty() {
            DEFAULT_ACTOR_COLOURS
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            actor_colours
        }
    }

    fn section_fills(&self) -> Vec<String> {
        let section_fills = config_string_vec(self.journey_config, &["sectionFills"]);
        if section_fills.is_empty() {
            DEFAULT_SECTION_FILLS
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            section_fills
        }
    }

    fn task_text_style(&self) -> TextStyle {
        TextStyle {
            font_family: Some(self.journey_string("taskFontFamily", DEFAULT_TASK_FONT_FAMILY)),
            font_size: self
                .journey_f64("taskFontSize")
                .unwrap_or(DEFAULT_TASK_FONT_SIZE),
            font_weight: None,
            font_style: None,
        }
    }

    fn use_max_width(&self) -> bool {
        self.journey_bool("useMaxWidth")
            .unwrap_or(DEFAULT_USE_MAX_WIDTH)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct JourneyLayoutSettings {
    pub(crate) left_margin_base: f64,
    pub(crate) max_label_width: f64,
    pub(crate) box_text_margin: f64,
    pub(crate) diagram_margin_x: f64,
    pub(crate) diagram_margin_y: f64,
    pub(crate) task_margin: f64,
    pub(crate) cell_width: f64,
    pub(crate) cell_height: f64,
    pub(crate) actor_colours: Vec<String>,
    section_fills: Vec<String>,
    section_fill_count: usize,
    pub(crate) use_max_width: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct JourneyRenderSettings {
    pub(crate) text_placement: JourneyTextPlacement,
    pub(crate) task_text_style: TextStyle,
    pub(crate) section_colours: Vec<String>,
    pub(crate) title_font_size: String,
    pub(crate) title_font_family: String,
    pub(crate) title_color: String,
}

impl JourneyLayoutSettings {
    pub(super) fn section_paint(&self, ordinal: i64) -> (i64, &str) {
        let slot = ordinal % self.section_fill_count as i64;
        let fill = self
            .section_fills
            .get(slot as usize)
            .map_or("#CCC", String::as_str);
        (slot, fill)
    }
}

pub(super) const fn default_use_max_width() -> bool {
    DEFAULT_USE_MAX_WIDTH
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::presentation::PresentationConfig;
    use serde_json::json;

    #[test]
    fn native_journey_preserves_default_section_slot_wrap() {
        let config = PresentationConfig::empty_object();
        let theme = merman_theme_contract::v2::ThemeCompiler::new()
            .compile_preset(merman_theme_contract::v2::BuiltinPreset::Studio, None)
            .unwrap();
        let style = JourneyNativeStyle::new(theme.family("journey"), &config);
        let view = JourneyConfigView::new(&config);
        let mermaid = view.layout_settings(None);
        let native = view.layout_settings(Some(&style));

        assert!(native.actor_colours.is_empty());
        assert!(native.section_fills.is_empty());
        for ordinal in [0, 6, 7, 8] {
            assert_eq!(
                native.section_paint(ordinal).0,
                mermaid.section_paint(ordinal).0
            );
        }
    }

    #[test]
    fn journey_text_placement_matches_the_upstream_selector() {
        assert_eq!(
            JourneyConfigView::new(&json!({}))
                .render_settings()
                .text_placement,
            JourneyTextPlacement::Fo
        );
        for (value, expected) in [
            (json!("fo"), JourneyTextPlacement::Fo),
            (json!("old"), JourneyTextPlacement::Old),
            (json!("tspan"), JourneyTextPlacement::Tspan),
            (json!("FO"), JourneyTextPlacement::Tspan),
            (json!(""), JourneyTextPlacement::Tspan),
            (Value::Null, JourneyTextPlacement::Tspan),
            (json!(1), JourneyTextPlacement::Tspan),
        ] {
            let config = json!({"journey": {"textPlacement": value}});
            assert_eq!(
                JourneyConfigView::new(&config)
                    .render_settings()
                    .text_placement,
                expected,
                "{config}"
            );
        }
    }

    #[test]
    fn journey_layout_settings_preserve_defaults() {
        let cfg = json!({});
        let settings = JourneyConfigView::new(&cfg).layout_settings(None);

        assert_eq!(settings.left_margin_base, DEFAULT_LEFT_MARGIN);
        assert_eq!(settings.max_label_width, DEFAULT_MAX_LABEL_WIDTH);
        assert_eq!(settings.box_text_margin, DEFAULT_BOX_TEXT_MARGIN);
        assert_eq!(settings.diagram_margin_x, DEFAULT_DIAGRAM_MARGIN_X);
        assert_eq!(settings.diagram_margin_y, DEFAULT_DIAGRAM_MARGIN_Y);
        assert_eq!(settings.task_margin, DEFAULT_TASK_MARGIN);
        assert_eq!(settings.cell_width, DEFAULT_CELL_WIDTH);
        assert_eq!(settings.cell_height, DEFAULT_CELL_HEIGHT);
        assert!(settings.use_max_width);
        assert_eq!(settings.actor_colours.len(), DEFAULT_ACTOR_COLOURS.len());
        assert_eq!(settings.section_fills.len(), DEFAULT_SECTION_FILLS.len());
    }

    #[test]
    fn journey_cell_dimensions_preserve_zero_and_existing_invalid_value_bounds() {
        for (value, expected_width, expected_height) in [
            (json!(0), 0.0, 0.0),
            (json!(-0.0), 0.0, 0.0),
            (json!("0"), 0.0, 0.0),
            (json!(1), 1.0, 1.0),
            (json!(-2), 1.0, 1.0),
            (json!(0.5), 1.0, 1.0),
            (json!("NaN"), DEFAULT_CELL_WIDTH, DEFAULT_CELL_HEIGHT),
            (json!("Infinity"), DEFAULT_CELL_WIDTH, DEFAULT_CELL_HEIGHT),
            (json!("-Infinity"), DEFAULT_CELL_WIDTH, DEFAULT_CELL_HEIGHT),
            (json!("1e999"), DEFAULT_CELL_WIDTH, DEFAULT_CELL_HEIGHT),
        ] {
            let config = json!({"journey": {"width": value, "height": value}});
            let settings = JourneyConfigView::new(&config).layout_settings(None);
            assert_eq!(settings.cell_width, expected_width, "{config}");
            assert_eq!(settings.cell_height, expected_height, "{config}");
        }
    }

    #[test]
    fn journey_layout_settings_project_configured_values() {
        let cfg = json!({
            "journey": {
                "leftMargin": 180,
                "maxLabelWidth": 250,
                "boxTextMargin": 8,
                "diagramMarginX": 60,
                "diagramMarginY": 12,
                "taskMargin": 90,
                "width": 170,
                "height": 70,
                "taskFontSize": 20,
                "taskFontFamily": "Inter, sans-serif",
                "actorColours": ["#111", "#222"],
                "sectionFills": ["#333", "#444"],
                "useMaxWidth": false
            }
        });
        let settings = JourneyConfigView::new(&cfg).layout_settings(None);

        assert_eq!(settings.left_margin_base, 180.0);
        assert_eq!(settings.max_label_width, 250.0);
        assert_eq!(settings.box_text_margin, 8.0);
        assert_eq!(settings.diagram_margin_x, 60.0);
        assert_eq!(settings.diagram_margin_y, 12.0);
        assert_eq!(settings.task_margin, 90.0);
        assert_eq!(settings.cell_width, 170.0);
        assert_eq!(settings.cell_height, 70.0);
        assert_eq!(settings.actor_colours, vec!["#111", "#222"]);
        assert_eq!(settings.section_fills, vec!["#333", "#444"]);
        assert!(!settings.use_max_width);
    }

    #[test]
    fn journey_render_settings_project_values() {
        let cfg = json!({
            "journey": {
                "taskFontSize": 18,
                "taskFontFamily": "Inter, sans-serif",
                "titleFontSize": "3.5ex",
                "titleFontFamily": "Georgia, serif",
                "titleColor": "#123456"
            }
        });
        let settings = JourneyConfigView::new(&cfg).render_settings();

        assert_eq!(settings.task_text_style.font_size, 18.0);
        assert_eq!(
            settings.task_text_style.font_family.as_deref(),
            Some("Inter, sans-serif")
        );
        assert_eq!(settings.title_font_size, "3.5ex");
        assert_eq!(settings.title_font_family, "Georgia, serif");
        assert_eq!(settings.title_color, "#123456");
    }
}
