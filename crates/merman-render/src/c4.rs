use crate::text::{TextMeasurer, TextStyle, WrapMode};
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

fn js_round_pos(v: f64) -> f64 {
    if !(v.is_finite() && v >= 0.0) {
        0.0
    } else {
        (v + 0.5).floor()
    }
}

fn c4_normalize_font_key(font_family: &str) -> String {
    font_family
        .chars()
        .filter_map(|ch| {
            if ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ';' {
                None
            } else {
                Some(ch.to_ascii_lowercase())
            }
        })
        .collect()
}

fn c4_font_weight_key(style: &TextStyle) -> String {
    style
        .font_weight
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("normal")
        .to_ascii_lowercase()
}

fn c4_text_width_override_px(style: &TextStyle, text: &str) -> Option<f64> {
    let font_family = style
        .font_family
        .as_deref()
        .unwrap_or(C4_DEFAULT_FONT_FAMILY);
    let font_key = c4_normalize_font_key(font_family);
    let font_size_key = (style.font_size.max(1.0) * 1000.0).round().max(1.0) as usize;
    let font_weight = c4_font_weight_key(style);

    crate::generated::c4_text_overrides_11_12_2::lookup_c4_text_width_px(
        &font_key,
        font_size_key,
        &font_weight,
        text.trim_end(),
    )
}

fn c4_svg_bbox_line_height_px(style: &TextStyle) -> f64 {
    // C4 in Mermaid@11.12.2 uses `calculateTextDimensions(...).height`, which is measured via
    // SVG `getBBox()` and rounded with `Math.round`. Upstream fixtures show stable, integer
    // per-line heights for the default C4 fonts:
    // - 12px -> 14px
    // - 14px -> 16px
    // - 16px -> 17px
    //
    // These do not match our generic deterministic SVG line-height approximation (`1.1em`),
    // so C4 owns the small rule directly instead of keeping it in generated parity data.
    let fs = js_round_pos(style.font_size.max(1.0)) as i64;
    match fs {
        12 => 14.0,
        14 => 16.0,
        16 => 17.0,
        _ => js_round_pos(style.font_size.max(1.0) * 1.1),
    }
}

fn measure_c4_text(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    wrap: bool,
    text_limit_width: f64,
) -> TextMeasure {
    // Mermaid's `calculateTextWidth/Height` (used by C4) draws SVG `<text>` nodes, calls
    // `getBBox()`, and then applies `Math.round(...)` per line. To keep C4 layout + viewport
    // parity with upstream SVG baselines, we mirror that integer rounding behavior here.
    if wrap {
        let m = measurer.measure_wrapped(text, style, Some(text_limit_width), WrapMode::SvgLike);
        return TextMeasure {
            width: text_limit_width,
            height: c4_svg_bbox_line_height_px(style) * m.line_count.max(1) as f64,
            line_count: m.line_count,
        };
    }

    let mut width: f64 = 0.0;
    let lines = crate::text::DeterministicTextMeasurer::normalized_text_lines(text);
    for line in &lines {
        let bbox_width = c4_text_width_override_px(style, line)
            .unwrap_or_else(|| measurer.measure_svg_simple_text_bbox_width_px(line, style));
        width = width.max(js_round_pos(bbox_width));
    }
    let height = c4_svg_bbox_line_height_px(style) * lines.len().max(1) as f64;
    TextMeasure {
        width,
        height,
        line_count: lines.len().max(1),
    }
}

mod layout;
pub(crate) use layout::{layout_c4_diagram, layout_c4_diagram_typed};

#[cfg(test)]
mod tests {
    use super::{TextStyle, c4_svg_bbox_line_height_px, c4_text_width_override_px};

    #[test]
    fn c4_svg_bbox_line_height_uses_owner_rules() {
        fn style(font_size: f64) -> TextStyle {
            TextStyle {
                font_size,
                ..Default::default()
            }
        }

        assert_eq!(c4_svg_bbox_line_height_px(&style(12.0)), 14.0);

        assert_eq!(c4_svg_bbox_line_height_px(&style(14.0)), 16.0);

        assert_eq!(c4_svg_bbox_line_height_px(&style(16.0)), 17.0);

        assert_eq!(c4_svg_bbox_line_height_px(&style(15.0)), 17.0);
    }

    #[test]
    fn c4_text_width_override_uses_headless_shell_metric() {
        let style = TextStyle {
            font_family: Some(r#""Open Sans", sans-serif"#.to_string()),
            font_size: 14.0,
            font_weight: None,
        };

        assert_eq!(
            c4_text_width_override_px(
                &style,
                "Allows customers to view information about their bank accounts, and make payments."
            ),
            Some(532.484375)
        );
    }
}
