use std::{borrow::Cow, fmt::Write as _};

use merman_render_core::{
    mermaid_style::visit_parsed_style_declarations, number_format::canonical_number as fmt,
    svg_output::SvgOutput,
};

use super::escape_attr;

/// Keeps the group's opacity off children that need their own inline paint.
pub fn grouped_shape_child_style(style: &str) -> Cow<'_, str> {
    let mut grouped_opacity = false;
    let mut child_style = String::new();
    visit_parsed_style_declarations(style, |decl| {
        if decl.property() == "opacity" {
            grouped_opacity = true;
        } else {
            if !child_style.is_empty() {
                child_style.push(';');
            }
            let _ = write!(
                child_style,
                "{}:{}",
                decl.property_css(),
                decl.source_value()
            );
        }
    });
    if !grouped_opacity {
        return Cow::Borrowed(style);
    }
    // In SVG label mode a classDef can also select the child primitive directly.
    if !child_style.is_empty() {
        child_style.push(';');
    }
    child_style.push_str("opacity:1 !important");
    Cow::Owned(child_style)
}

pub fn write_hand_drawn_hachure(
    out: &mut impl SvgOutput,
    outline: &str,
    hatch: &str,
    width: f64,
    colors: (&str, &str),
    fill_opacity: Option<f64>,
) {
    if let Some(fill_opacity) = fill_opacity {
        write_backed_hachure(out, outline, hatch, width, colors, fill_opacity);
    } else {
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"/>"#,
            escape_attr(hatch),
            escape_attr(colors.0),
            fmt(width),
        );
    }
}

pub fn write_backed_hachure(
    out: &mut impl SvgOutput,
    outline: &str,
    hatch: &str,
    width: f64,
    (fill_color, stroke_color): (&str, &str),
    fill_opacity: f64,
) {
    // The silhouette covers connectors and container paint in the gaps between hatch lines.
    let fill = escape_attr(fill_color);
    let opacity = fmt(fill_opacity);
    let _ = write!(
        out,
        r#"<path class="merman-rough-backing" d="{}" fill="{fill}" stroke="none" style="fill:{fill} !important;fill-opacity:{opacity} !important;stroke:none !important"/>"#,
        escape_attr(outline),
    );
    let alpha = merman_model::css_color::parse(fill_color.trim()).map_or(0.0, |color| {
        f64::from(color.rgba.to_rgba8()[3]) / f64::from(u8::MAX)
    });
    if alpha > 0.0 && fill_opacity > 0.0 {
        let stroke = escape_attr(stroke_color);
        let _ = write!(
            out,
            r#"<path d="{}" fill="none" stroke="{stroke}" stroke-width="{}" stroke-dasharray="0 0" style="fill:none !important;stroke:{stroke} !important;stroke-opacity:{} !important"/>"#,
            escape_attr(hatch),
            fmt(width),
            fmt(0.15 * fill_opacity * alpha),
        );
    }
}
