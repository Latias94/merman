//! Process and rounded-rectangle bodies and their Rough queries.

use kurbo::Rect;
use merman_render_core::{
    config::json_f64,
    number_format::canonical_number as fmt,
    rough_svg::paths::{
        RoughGeometryTarget, roughjs_hachure_paths_for_rect, roughjs_hachure_paths_for_svg_path,
        roughjs_prepare_hachure_paths_for_rect, roughjs_prepare_hachure_paths_for_svg_path,
    },
    svg_output::SvgOutput,
};
use serde_json::Value;

use crate::theme::FlowchartCompatibilityBinding;

use super::{NodeBodyGeometry, NodeBodySvg, escape_attr};

const HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const HAND_DRAWN_FILL_WEIGHT: f32 = 1.5;
const HAND_DRAWN_HACHURE_GAP: f32 = 1.5;

/// The two Mermaid rectangle paths share the same square fallback.
pub enum RectangleBody {
    Process,
    Rounded,
}

/// Resolved source radii, native defaults, and Neo filter suppression.
pub struct RectangleStyle<'a> {
    pub source_radii: [Option<f64>; 2],
    pub native_radius: Option<f64>,
    pub configured_radius: Option<f64>,
    pub effect_filter_style: &'a str,
}

impl RectangleBody {
    pub fn write(
        self,
        out: &mut impl SvgOutput,
        body: NodeBodySvg<'_>,
        rect: Rect,
        style: RectangleStyle<'_>,
        compatibility: &FlowchartCompatibilityBinding,
    ) {
        match self {
            Self::Process => write_process(out, body, rect, style),
            Self::Rounded => write_rounded(out, body, rect, style, compatibility),
        }
    }

    pub fn advance_rough_geometry(
        self,
        geometry: NodeBodyGeometry<'_>,
        compatibility: &FlowchartCompatibilityBinding,
    ) {
        if !geometry.hand_drawn {
            return;
        }
        // The math-only traversal uses the layout box. The writer uses the prepared SVG rect.
        let width = geometry.width.max(1.0);
        let height = geometry.height.max(1.0);
        match self {
            Self::Rounded if compatibility.rounded_rect_radius_truthy => {
                let path =
                    rounded_rect_path_data(width, height, &compatibility.rounded_rect_radius);
                let _ = roughjs_prepare_hachure_paths_for_svg_path(
                    RoughGeometryTarget::Query,
                    &path,
                    geometry.stroke_width,
                    geometry.stroke_dasharray,
                    HAND_DRAWN_FILL_WEIGHT,
                    HAND_DRAWN_HACHURE_GAP,
                    HAND_DRAWN_ROUGHNESS,
                    geometry.work_meter,
                    geometry.randomness,
                );
            }
            Self::Process | Self::Rounded => {
                let _ = roughjs_prepare_hachure_paths_for_rect(
                    RoughGeometryTarget::Query,
                    -width / 2.0,
                    -height / 2.0,
                    width,
                    height,
                    geometry.stroke_width,
                    geometry.stroke_dasharray,
                    HAND_DRAWN_FILL_WEIGHT,
                    HAND_DRAWN_HACHURE_GAP,
                    HAND_DRAWN_ROUGHNESS,
                    geometry.work_meter,
                    geometry.randomness,
                );
            }
        }
    }
}

fn write_process(
    out: &mut impl SvgOutput,
    mut body: NodeBodySvg<'_>,
    rect: Rect,
    style: RectangleStyle<'_>,
) {
    let width = rect.width();
    let height = rect.height();
    let rough_paths = if body.geometry.hand_drawn {
        body.timed_rough(|geometry| {
            roughjs_hachure_paths_for_rect(
                -width / 2.0,
                -height / 2.0,
                width,
                height,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                HAND_DRAWN_FILL_WEIGHT,
                HAND_DRAWN_HACHURE_GAP,
                HAND_DRAWN_ROUGHNESS,
                geometry.work_meter,
                geometry.randomness,
            )
        })
    } else {
        None
    };

    if let Some((fill_d, stroke_d)) = rough_paths {
        let _ = write!(
            out,
            r#"<g class="basic label-container" style="{}"><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
            escape_attr(body.rough_group_style),
            escape_attr(&fill_d),
            escape_attr(body.fill_color),
            fmt(HAND_DRAWN_FILL_WEIGHT as f64),
            escape_attr(&stroke_d),
            escape_attr(body.stroke_color),
            body.geometry.stroke_width,
            escape_attr(body.geometry.stroke_dasharray),
        );
        return;
    }

    // Preserve explicit Neo configuration and the independent native and source radii.
    let radius = style.native_radius.or(style.configured_radius);
    style.write_start(out, rect, body.style);
    style.write_radii(out, radius);
    out.push_str(" />");
}

fn write_rounded(
    out: &mut impl SvgOutput,
    mut body: NodeBodySvg<'_>,
    rect: Rect,
    style: RectangleStyle<'_>,
    compatibility: &FlowchartCompatibilityBinding,
) {
    let w = rect.width();
    let h = rect.height();
    // drawRect applies truthy overrides. Numeric zero leaves rx/ry absent and "0" applies.
    let radius = compatibility
        .rounded_rect_radius_truthy
        .then_some(&compatibility.rounded_rect_radius);
    if body.geometry.hand_drawn && radius.is_none() {
        // drawRect uses rc.rectangle when neither resolved radius is truthy.
        return write_process(out, body, rect, style);
    }

    let rough_paths = if body.geometry.hand_drawn {
        // Preserve the source SVG arcs, including radii larger than half the box.
        let path_data = rounded_rect_path_data(w, h, &compatibility.rounded_rect_radius);
        body.timed_rough(|geometry| {
            roughjs_hachure_paths_for_svg_path(
                &path_data,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                HAND_DRAWN_FILL_WEIGHT,
                HAND_DRAWN_HACHURE_GAP,
                HAND_DRAWN_ROUGHNESS,
                geometry.work_meter,
                geometry.randomness,
            )
        })
    } else {
        None
    };

    if let Some((fill_d, stroke_d)) = rough_paths {
        let _ = write!(
            out,
            r#"<g class="basic label-container" style="{}">"#,
            escape_attr(body.rough_group_style)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"/>"#,
            escape_attr(&fill_d),
            escape_attr(body.fill_color),
            fmt(HAND_DRAWN_FILL_WEIGHT as f64),
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
            escape_attr(&stroke_d),
            escape_attr(body.stroke_color),
            body.geometry.stroke_width,
            escape_attr(body.geometry.stroke_dasharray),
        );
        out.push_str("</g>");
    } else {
        let numeric_radius = style
            .native_radius
            .or(compatibility.rounded_rect_radius_numeric);
        style.write_start(out, rect, body.style);
        if style.native_radius.is_some() || style.source_radii.iter().any(Option::is_some) {
            style.write_radii(out, numeric_radius);
        } else if let Some(radius) = radius {
            let radius = radius_text(radius);
            let _ = write!(
                out,
                r#" rx="{}" ry="{}""#,
                escape_attr(&radius),
                escape_attr(&radius)
            );
        }
        out.push_str("/>");
    }
}

impl RectangleStyle<'_> {
    fn write_radii(&self, out: &mut impl SvgOutput, fallback: Option<f64>) {
        for (axis, source) in ["rx", "ry"].into_iter().zip(self.source_radii) {
            if let Some(radius) = source.or(fallback) {
                let _ = write!(out, " {axis}=\"{}\"", fmt(radius));
            }
        }
    }

    fn write_start(&self, out: &mut impl SvgOutput, rect: Rect, style: &str) {
        let _ = write!(
            out,
            r#"<rect class="basic label-container" style="{}"#,
            escape_attr(style),
        );
        out.push_str(self.effect_filter_style);

        // Keep admitted source winners and typed geometry consistent with native attributes.
        // Neo defaults and differently cased source spellings must not override them in CSS.
        for (axis, source) in ["rx", "ry"].into_iter().zip(self.source_radii) {
            if let Some(radius) = source.or(self.native_radius) {
                let radius = fmt(radius);
                let _ = write!(out, ";{axis}:{radius}px !important");
            }
        }
        let _ = write!(
            out,
            r#"" x="{}" y="{}" width="{}" height="{}""#,
            fmt(rect.x0),
            fmt(rect.y0),
            fmt(rect.width()),
            fmt(rect.height()),
        );
    }
}

fn radius_text(radius: &Value) -> String {
    match radius {
        Value::String(text) => text.clone(),
        Value::Number(number) => fmt(number.as_f64().unwrap_or(0.0)).to_string(),
        _ => radius.to_string(),
    }
}

fn rounded_rect_path_data(width: f64, height: f64, radius: &Value) -> String {
    let x = -width / 2.0;
    let y = -height / 2.0;
    let numeric_radius = match radius {
        Value::Bool(value) => f64::from(u8::from(*value)),
        Value::String(value) if value.trim().is_empty() => 0.0,
        _ => json_f64(radius).unwrap_or(f64::NAN),
    };
    let add_radius = |coordinate: f64| match radius.as_str() {
        Some(text) => format!("{}{text}", fmt(coordinate)),
        None => fmt(coordinate + numeric_radius).to_string(),
    };
    let radius = radius_text(radius);
    format!(
        "M {} {} H {} A {} {} 0 0 1 {} {} V {} A {} {} 0 0 1 {} {} H {} A {} {} 0 0 1 {} {} V {} A {} {} 0 0 1 {} {} Z",
        add_radius(x),
        fmt(y),
        fmt(x + width - numeric_radius),
        radius,
        radius,
        fmt(x + width),
        add_radius(y),
        fmt(y + height - numeric_radius),
        radius,
        radius,
        fmt(x + width - numeric_radius),
        fmt(y + height),
        add_radius(x),
        radius,
        radius,
        fmt(x),
        fmt(y + height - numeric_radius),
        add_radius(y),
        radius,
        radius,
        add_radius(x),
        fmt(y),
    )
}
