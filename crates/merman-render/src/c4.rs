use crate::text::{TextMeasurer, TextStyle, WrapMode, measure_mermaid_text_dimensions};
use merman_core::diagrams::c4::C4DiagramRenderModel;

mod config;

pub(crate) use config::{
    C4_DEFAULT_FONT_FAMILY, C4ConfigView, C4LayoutSettings, default_use_max_width,
};

type C4Model = C4DiagramRenderModel;
type C4Conf = C4LayoutSettings;

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

mod layout;
pub use layout::layout_c4_diagram_typed;

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
