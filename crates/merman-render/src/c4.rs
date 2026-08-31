use crate::text::{TextMeasurer, TextStyle, WrapMode, measure_mermaid_text_dimensions};
use merman_core::diagrams::c4::{C4DiagramRenderModel, C4ShapeRenderModel};
use serde_json::Value;

mod config;

pub(crate) use config::{
    C4_DEFAULT_FONT_FAMILY, C4_ELEMENT_TYPES, C4ConfigView, C4LayoutSettings, default_use_max_width,
};

type C4Model = C4DiagramRenderModel;
type C4Conf = C4LayoutSettings;

/// The unified Mermaid shape used to draw a C4 element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum C4NodeShape {
    Rounded,
    Framed,
    Person,
    Cylinder,
    HorizontalCylinder,
}

fn value_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn keyword_shape(value: Option<&str>) -> Option<C4NodeShape> {
    match value?.to_ascii_lowercase().as_str() {
        "person" => Some(C4NodeShape::Person),
        "box" | "rounded" => Some(C4NodeShape::Rounded),
        "component" => Some(C4NodeShape::Framed),
        "cylinder" | "database" | "db" => Some(C4NodeShape::Cylinder),
        "queue" | "pipe" => Some(C4NodeShape::HorizontalCylinder),
        _ => None,
    }
}

/// Resolves `$shape`, `$sprite`, `$tags`, and the legacy C4 element type in the
/// same order as Mermaid's `c4ShapeAdapter`.
pub(crate) fn c4_node_shape(shape: &C4ShapeRenderModel) -> C4NodeShape {
    keyword_shape(value_string(shape.shape.as_ref()))
        .or_else(|| keyword_shape(value_string(shape.sprite.as_ref())))
        .or_else(|| {
            value_string(shape.tags.as_ref()).and_then(|tags| {
                tags.split(',')
                    .find_map(|tag| keyword_shape(Some(tag.trim())))
            })
        })
        .unwrap_or_else(|| match shape.type_c4_shape.as_str() {
            "person" | "external_person" => C4NodeShape::Person,
            value if value.ends_with("_db") => C4NodeShape::Cylinder,
            value if value.ends_with("_queue") => C4NodeShape::HorizontalCylinder,
            _ => C4NodeShape::Rounded,
        })
}

fn c4_stereotype_name(type_c4_shape: &str) -> String {
    let base = type_c4_shape
        .strip_prefix("external_")
        .unwrap_or(type_c4_shape);
    let base = base
        .strip_suffix("_db")
        .or_else(|| base.strip_suffix("_queue"))
        .unwrap_or(base);
    match base {
        "person" => "Person".to_string(),
        "system" => "Software System".to_string(),
        "container" => "Container".to_string(),
        "component" => "Component".to_string(),
        other => other.replace('_', " "),
    }
}

/// Returns the structured stereotype line used by the unified C4 label helper.
pub(crate) fn c4_stereotype_text(shape: &C4ShapeRenderModel) -> String {
    let stereotype = c4_stereotype_name(shape.type_c4_shape.as_str());
    match shape
        .techn
        .as_ref()
        .map(|text| text.as_str())
        .filter(|s| !s.is_empty())
    {
        Some(technology) => format!("[{stereotype}: {technology}]"),
        None => format!("[{stereotype}]"),
    }
}

#[derive(Debug, Clone, Copy)]
struct TextMeasure {
    width: f64,
    height: f64,
    line_count: usize,
}

fn measure_c4_text(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    wrap: bool,
    text_limit_width: f64,
) -> TextMeasure {
    let dimensions = measure_mermaid_text_dimensions(measurer, text, style);
    if wrap {
        let m = measurer.measure_wrapped(text, style, Some(text_limit_width), WrapMode::SvgLike);
        let line_count = m.line_count.max(1);
        return TextMeasure {
            width: text_limit_width,
            height: dimensions.line_height.max(0) as f64 * line_count as f64,
            line_count,
        };
    }

    TextMeasure {
        width: dimensions.width.max(0) as f64,
        height: dimensions.height.max(0) as f64,
        line_count: crate::text::split_html_br_lines(text).len().max(1),
    }
}

/// Measures one section of the unified C4 label.
///
/// Unlike the legacy C4 path, the unified label helper sizes from the rendered
/// section's actual wrapped bounding box; the configured wrap width is only a
/// line-breaking constraint. Keep the legacy helper above unchanged for
/// boundaries and relationships, which still use Mermaid's fixed-width path.
fn measure_c4_unified_text(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    wrap: bool,
    text_limit_width: f64,
) -> TextMeasure {
    if wrap {
        let metrics =
            measurer.measure_wrapped(text, style, Some(text_limit_width), WrapMode::SvgLike);
        return TextMeasure {
            width: metrics.width.max(0.0) as f64,
            height: metrics.height.max(0.0) as f64,
            line_count: metrics.line_count.max(1),
        };
    }

    measure_c4_text(measurer, text, style, false, text_limit_width)
}

mod layout;
pub(crate) use layout::layout_c4_diagram_typed;

#[cfg(test)]
mod tests {
    use crate::text::{TextMeasurer, TextMetrics};

    use super::{TextStyle, measure_c4_text};

    struct C4ProbeMeasurer;

    impl TextMeasurer for C4ProbeMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 3,
            }
        }

        fn measure_svg_simple_text_bbox_width_px(&self, _text: &str, style: &TextStyle) -> f64 {
            if style.font_family.as_deref() == Some("sans-serif") {
                80.0
            } else {
                120.0
            }
        }

        fn measure_svg_simple_text_bbox_height_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            19.0
        }
    }

    #[test]
    fn c4_unwrapped_text_consumes_shared_mermaid_dimensions() {
        let measured = measure_c4_text(
            &C4ProbeMeasurer,
            "configured text",
            &TextStyle::default(),
            false,
            500.0,
        );

        assert_eq!(measured.width, 120.0);
        assert_eq!(measured.height, 19.0);
        assert_eq!(measured.line_count, 1);
    }

    #[test]
    fn c4_wrapped_text_uses_shared_bbox_line_height() {
        let measured = measure_c4_text(
            &C4ProbeMeasurer,
            "Allows customers to view information about their bank accounts, and make payments.",
            &TextStyle::default(),
            true,
            200.0,
        );

        assert_eq!(measured.width, 200.0);
        assert_eq!(measured.height, 57.0);
        assert_eq!(measured.line_count, 3);
    }
}
