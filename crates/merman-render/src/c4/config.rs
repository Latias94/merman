use crate::config::{
    DiagramLook, config_bool, config_css_number_or_string, config_diagram_look, config_f64,
    config_string,
};
use crate::text::TextStyle;
use serde_json::Value;

pub(crate) const C4_DEFAULT_FONT_FAMILY: &str = r#""Open Sans", sans-serif"#;

/// Mermaid emits one CSS rule for every C4 element type, including external variants.
pub(crate) const C4_ELEMENT_TYPES: &[&str] = &[
    "person",
    "external_person",
    "system",
    "external_system",
    "system_db",
    "external_system_db",
    "system_queue",
    "external_system_queue",
    "container",
    "external_container",
    "container_db",
    "external_container_db",
    "container_queue",
    "external_container_queue",
    "component",
    "external_component",
    "component_db",
    "external_component_db",
    "component_queue",
    "external_component_queue",
];

const DEFAULT_DIAGRAM_MARGIN_X: f64 = 50.0;
const DEFAULT_DIAGRAM_MARGIN_Y: f64 = 10.0;
const DEFAULT_C4_SHAPE_MARGIN: f64 = 50.0;
const DEFAULT_C4_SHAPE_PADDING: f64 = 20.0;
const DEFAULT_WIDTH: f64 = 216.0;
const DEFAULT_HEIGHT: f64 = 60.0;
const DEFAULT_WRAP: bool = true;
const DEFAULT_NEXT_LINE_PADDING_X: f64 = 0.0;
const DEFAULT_USE_MAX_WIDTH: bool = true;
const DEFAULT_BOUNDARY_FONT_SIZE: f64 = 14.0;
const DEFAULT_MESSAGE_FONT_SIZE: f64 = 12.0;

/// The C4 renderer supports the same global look vocabulary as Mermaid, while keeping the
/// implementation-owned behavior explicit at the family boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum C4Look {
    Classic,
    Neo,
    HandDrawn,
}

impl C4Look {
    fn from_diagram_look(look: DiagramLook<'_>) -> Self {
        match look.as_str() {
            "neo" => Self::Neo,
            "handDrawn" => Self::HandDrawn,
            _ => Self::Classic,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Neo => "neo",
            Self::HandDrawn => "handDrawn",
        }
    }

    pub(crate) const fn is_neo(self) -> bool {
        matches!(self, Self::Neo)
    }

    pub(crate) const fn is_hand_drawn(self) -> bool {
        matches!(self, Self::HandDrawn)
    }
}

pub(crate) struct C4ConfigView<'a> {
    effective_config: &'a Value,
    c4_config: &'a Value,
}

impl<'a> C4ConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            effective_config,
            c4_config: effective_config.get("c4").unwrap_or(&Value::Null),
        }
    }

    pub(crate) fn look(&self) -> C4Look {
        C4Look::from_diagram_look(config_diagram_look(self.effective_config))
    }

    #[cfg(test)]
    pub(crate) fn layout_settings(&self) -> C4LayoutSettings {
        self.layout_settings_with_prepared_fonts(&self.boundary_font(), &self.message_font())
    }

    pub(crate) fn shape_padding(&self) -> f64 {
        self.c4_f64("c4ShapePadding")
            .unwrap_or(DEFAULT_C4_SHAPE_PADDING)
            .max(0.0)
    }

    pub(crate) fn layout_settings_with_prepared_fonts(
        &self,
        boundary: &TextStyle,
        message: &TextStyle,
    ) -> C4LayoutSettings {
        C4LayoutSettings {
            look: self.look(),
            diagram_margin_x: self
                .c4_f64("diagramMarginX")
                .unwrap_or(DEFAULT_DIAGRAM_MARGIN_X)
                .max(0.0),
            diagram_margin_y: self
                .c4_f64("diagramMarginY")
                .unwrap_or(DEFAULT_DIAGRAM_MARGIN_Y)
                .max(0.0),
            c4_shape_margin: self
                .c4_f64("c4ShapeMargin")
                .unwrap_or(DEFAULT_C4_SHAPE_MARGIN)
                .max(0.0),
            c4_shape_padding: self
                .c4_f64("c4ShapePadding")
                .unwrap_or(DEFAULT_C4_SHAPE_PADDING)
                .max(0.0),
            width: self.c4_f64("width").unwrap_or(DEFAULT_WIDTH).max(1.0),
            height: self.c4_f64("height").unwrap_or(DEFAULT_HEIGHT).max(1.0),
            wrap: self.c4_bool("wrap").unwrap_or(DEFAULT_WRAP),
            next_line_padding_x: self
                .c4_f64("nextLinePaddingX")
                .unwrap_or(DEFAULT_NEXT_LINE_PADDING_X),
            boundary_font_family: boundary.font_family.clone(),
            boundary_font_size: boundary.font_size,
            boundary_font_weight: boundary.font_weight.clone(),
            message_font_family: message.font_family.clone(),
            message_font_size: message.font_size,
            message_font_weight: message.font_weight.clone(),
            use_max_width: self.use_max_width(),
        }
    }

    pub(crate) fn color(&self, key: &str, fallback: &str) -> String {
        self.c4_string(key).unwrap_or_else(|| fallback.to_string())
    }

    pub(crate) fn font_family(&self, key: &str) -> String {
        config_string(self.c4_config, &[key])
            .map(|s| s.trim().trim_end_matches(';').trim().to_string())
            .unwrap_or_else(|| C4_DEFAULT_FONT_FAMILY.to_string())
    }

    pub(crate) fn font_size(&self, key: &str, fallback: f64) -> f64 {
        self.c4_f64(key).unwrap_or(fallback)
    }

    pub(crate) fn font_weight(&self, key: &str) -> Option<String> {
        config_css_number_or_string(self.c4_config, &[key])
    }

    pub(crate) fn boundary_font(&self) -> TextStyle {
        TextStyle {
            font_family: Some(self.font_family("boundaryFontFamily")),
            font_size: self.font_size("boundaryFontSize", DEFAULT_BOUNDARY_FONT_SIZE),
            font_weight: self.font_weight("boundaryFontWeight"),
            font_style: None,
        }
    }

    pub(crate) fn message_font(&self) -> TextStyle {
        TextStyle {
            font_family: Some(self.font_family("messageFontFamily")),
            font_size: self.font_size("messageFontSize", DEFAULT_MESSAGE_FONT_SIZE),
            font_weight: self.font_weight("messageFontWeight"),
            font_style: None,
        }
    }

    pub(crate) fn shape_font(&self, type_c4_shape: &str) -> TextStyle {
        let key_family = format!("{type_c4_shape}FontFamily");
        let key_size = format!("{type_c4_shape}FontSize");
        let key_weight = format!("{type_c4_shape}FontWeight");

        TextStyle {
            font_family: Some(self.font_family(&key_family)),
            font_size: self.font_size(&key_size, 14.0),
            font_weight: self.font_weight(&key_weight),
            font_style: None,
        }
    }

    pub(crate) fn use_max_width(&self) -> bool {
        self.c4_bool("useMaxWidth").unwrap_or(DEFAULT_USE_MAX_WIDTH)
    }

    pub(crate) fn diagram_margin_x(&self) -> f64 {
        self.c4_f64("diagramMarginX")
            .unwrap_or(DEFAULT_DIAGRAM_MARGIN_X)
    }

    pub(crate) fn diagram_margin_y(&self) -> f64 {
        self.c4_f64("diagramMarginY")
            .unwrap_or(DEFAULT_DIAGRAM_MARGIN_Y)
    }

    fn c4_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.c4_config, &[key])
    }

    fn c4_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.c4_config, &[key])
    }

    fn c4_string(&self, key: &str) -> Option<String> {
        config_string(self.c4_config, &[key])
    }
}

#[derive(Debug, Clone)]
pub(crate) struct C4LayoutSettings {
    pub(crate) look: C4Look,
    pub(crate) diagram_margin_x: f64,
    pub(crate) diagram_margin_y: f64,
    pub(crate) c4_shape_margin: f64,
    pub(crate) c4_shape_padding: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) wrap: bool,
    pub(crate) next_line_padding_x: f64,
    pub(crate) boundary_font_family: Option<String>,
    pub(crate) boundary_font_size: f64,
    pub(crate) boundary_font_weight: Option<String>,
    pub(crate) message_font_family: Option<String>,
    pub(crate) message_font_size: f64,
    pub(crate) message_font_weight: Option<String>,
    pub(crate) use_max_width: bool,
}

impl C4LayoutSettings {
    pub(crate) fn boundary_font(&self) -> TextStyle {
        TextStyle {
            font_family: self.boundary_font_family.clone(),
            font_size: self.boundary_font_size,
            font_weight: self.boundary_font_weight.clone(),
            font_style: None,
        }
    }

    pub(crate) fn message_font(&self) -> TextStyle {
        TextStyle {
            font_family: self.message_font_family.clone(),
            font_size: self.message_font_size,
            font_weight: self.message_font_weight.clone(),
            font_style: None,
        }
    }
}

pub(crate) const fn default_use_max_width() -> bool {
    DEFAULT_USE_MAX_WIDTH
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn c4_layout_settings_preserve_defaults() {
        let cfg = json!({});
        let settings = C4ConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.diagram_margin_x, DEFAULT_DIAGRAM_MARGIN_X);
        assert_eq!(settings.diagram_margin_y, DEFAULT_DIAGRAM_MARGIN_Y);
        assert_eq!(settings.look, C4Look::Classic);
        assert_eq!(settings.c4_shape_margin, DEFAULT_C4_SHAPE_MARGIN);
        assert_eq!(settings.c4_shape_padding, DEFAULT_C4_SHAPE_PADDING);
        assert_eq!(settings.width, DEFAULT_WIDTH);
        assert_eq!(settings.height, DEFAULT_HEIGHT);
        assert_eq!(settings.wrap, DEFAULT_WRAP);
        assert_eq!(settings.next_line_padding_x, DEFAULT_NEXT_LINE_PADDING_X);
        assert!(settings.use_max_width);
        assert_eq!(
            settings.boundary_font_family.as_deref(),
            Some(C4_DEFAULT_FONT_FAMILY)
        );
        assert_eq!(settings.boundary_font_size, DEFAULT_BOUNDARY_FONT_SIZE);
        assert_eq!(
            settings.message_font_family.as_deref(),
            Some(C4_DEFAULT_FONT_FAMILY)
        );
        assert_eq!(settings.message_font_size, DEFAULT_MESSAGE_FONT_SIZE);
    }

    #[test]
    fn c4_config_view_projects_configured_values() {
        let cfg = json!({
            "c4": {
                "diagramMarginX": 80,
                "diagramMarginY": 22,
                "c4ShapeMargin": 60,
                "c4ShapePadding": 24,
                "width": 260,
                "height": 72,
                "wrap": false,
                "nextLinePaddingX": 12,
                "boundaryFontFamily": "Inter, sans-serif;",
                "boundaryFontSize": 18,
                "boundaryFontWeight": "bold",
                "messageFontFamily": "Georgia, serif",
                "messageFontSize": 15,
                "messageFontWeight": 600,
                "useMaxWidth": false,
            },
            "look": "neo"
        });
        let settings = C4ConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.diagram_margin_x, 80.0);
        assert_eq!(settings.diagram_margin_y, 22.0);
        assert_eq!(settings.c4_shape_margin, 60.0);
        assert_eq!(settings.c4_shape_padding, 24.0);
        assert_eq!(settings.look, C4Look::Neo);
        assert_eq!(settings.width, 260.0);
        assert_eq!(settings.height, 72.0);
        assert!(!settings.wrap);
        assert_eq!(settings.next_line_padding_x, 12.0);
        assert_eq!(
            settings.boundary_font_family.as_deref(),
            Some("Inter, sans-serif")
        );
        assert_eq!(settings.boundary_font_weight.as_deref(), Some("bold"));
        assert_eq!(
            settings.message_font_family.as_deref(),
            Some("Georgia, serif")
        );
        assert_eq!(settings.message_font_weight.as_deref(), Some("600"));
        assert!(!settings.use_max_width);
    }

    #[test]
    fn c4_font_accessors_trim_and_fallback() {
        let cfg = json!({
            "c4": {
                "personFontFamily": "  Inter, sans-serif; ",
                "personFontSize": 18,
                "personFontWeight": "700"
            }
        });
        let view = C4ConfigView::new(&cfg);
        let font = view.shape_font("person");

        assert_eq!(view.font_family("personFontFamily"), "Inter, sans-serif");
        assert_eq!(view.font_weight("personFontWeight").as_deref(), Some("700"));
        assert_eq!(font.font_family.as_deref(), Some("Inter, sans-serif"));
        assert_eq!(font.font_size, 18.0);
        assert_eq!(font.font_weight.as_deref(), Some("700"));
    }

    #[test]
    fn c4_look_reads_the_global_allow_listed_value() {
        let neo = C4ConfigView::new(&json!({ "look": "neo" })).layout_settings();
        assert_eq!(neo.look, C4Look::Neo);

        let hand_drawn = C4ConfigView::new(&json!({ "look": "handDrawn" })).layout_settings();
        assert_eq!(hand_drawn.look, C4Look::HandDrawn);

        let invalid = C4ConfigView::new(&json!({ "look": "not-a-look" })).layout_settings();
        assert_eq!(invalid.look, C4Look::Classic);
    }
}
