//! Flowchart v2 basic geometry shapes.

use std::borrow::Cow;

use merman_model::OperationPhase;
use merman_render_core::{
    Result,
    mermaid_style::parse_safe_style_decl,
    rough_svg::{
        closed_path_d_from_points as path_from_points,
        paths::{
            RoughGeometryTarget, roughjs_hachure_paths_for_svg_path, roughjs_paths_for_circle,
            roughjs_paths_for_circle_with_roughness, roughjs_prepare_hachure_paths_for_svg_path,
            roughjs_prepare_paths_for_circle_with_roughness,
        },
    },
    svg_output::SvgOutput,
};

use crate::{
    node_geometry::DoubleCircleGeometry, shapes::FlowchartShape,
    style::flowchart_split_mermaid_style_decls,
};

use super::{NodeBodyGeometry, NodeBodySvg, OptionalStyleAttr, circle_backing, escape_attr, fmt};

mod hexagon;
mod odd;
mod stadium;

pub use hexagon::{advance_hexagon_rough_geometry, write_hexagon};
pub use odd::{advance_odd_rough_geometry, write_odd};
pub use stadium::{advance_stadium_rough_geometry, write_stadium};

const FLOWCHART_DIAMOND_HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const FLOWCHART_DIAMOND_HAND_DRAWN_FILL_WEIGHT: f32 = 1.5;
const FLOWCHART_DIAMOND_HAND_DRAWN_HACHURE_GAP: f32 = 1.5;
const SUPPRESS_NEO_FILTER_STYLE: &str = ";filter:none !important";

pub fn write_diamond(
    out: &mut impl SvgOutput,
    mut body: NodeBodySvg<'_>,
    centered_diamond: bool,
    suppress_neo_filter: bool,
) {
    let w = body.geometry.width.max(1.0);
    let h = body.geometry.height.max(1.0);
    let tx = -w / 2.0 + if centered_diamond { 0.0 } else { 0.5 };
    let ty = h / 2.0;

    if body.geometry.hand_drawn {
        // RoughJS normalizes this path to a left-edge hachure start. `roughr` is start-point
        // sensitive for this diamond, so feed the same polygon from the left vertex to keep the
        // hand-drawn silhouette centered instead of visually skewing into a parallelogram.
        let path_data = diamond_path(w, h);
        if let Some((fill_d, stroke_d)) = body.timed_rough(|geometry| {
            roughjs_hachure_paths_for_svg_path(
                &path_data,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                FLOWCHART_DIAMOND_HAND_DRAWN_FILL_WEIGHT,
                FLOWCHART_DIAMOND_HAND_DRAWN_HACHURE_GAP,
                FLOWCHART_DIAMOND_HAND_DRAWN_ROUGHNESS,
                geometry.work_meter,
                geometry.randomness,
            )
        }) {
            let _ = write!(
                out,
                r#"<g transform="translate({},{})" style="{}">"#,
                fmt(tx),
                fmt(ty),
                escape_attr(body.hand_drawn_group_style()),
            );
            body.write_hand_drawn_path_pair(
                out,
                &path_data,
                &fill_d,
                &stroke_d,
                f64::from(FLOWCHART_DIAMOND_HAND_DRAWN_FILL_WEIGHT),
            );
            out.push_str("</g>");
            return;
        }
    }

    let effect_style;
    let body_style = if !suppress_neo_filter {
        body.style
    } else {
        effect_style = format!("{}{SUPPRESS_NEO_FILTER_STYLE}", body.style);
        &effect_style
    };
    let _ = write!(
        out,
        r#"<polygon points="{},0 {},{} {},{} 0,{}" class="label-container" transform="translate({},{})"{} />"#,
        fmt(w / 2.0),
        fmt(w),
        fmt(-h / 2.0),
        fmt(w / 2.0),
        fmt(-h),
        fmt(-h / 2.0),
        fmt(tx),
        fmt(ty),
        OptionalStyleAttr(body_style),
    );
}

pub fn write_circle(
    out: &mut impl SvgOutput,
    mut body: NodeBodySvg<'_>,
    source_styles: &[String],
    suppress_neo_filter: bool,
) -> Result<()> {
    let w = body.geometry.width.max(1.0);
    let h = body.geometry.height.max(1.0);
    let r = (w.min(h) / 2.0).max(0.5);
    if body.geometry.hand_drawn {
        let paths = body.timed_rough(|geometry| {
            roughjs_paths_for_circle(
                r * 2.0,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                true,
                geometry.work_meter,
                geometry.randomness,
            )
        });
        body.geometry.work_meter.checkpoint(OperationPhase::Emit)?;
        if let Some(paths) = paths {
            let _ = write!(
                out,
                r#"<g class="basic label-container" style="{}">"#,
                escape_attr(&circle_group_style(&body, source_styles)),
            );
            write_rough_circle_paths(out, &body, &paths, body.geometry.stroke_width, r);
            out.push_str("</g>");
            out.checkpoint()?;
            return Ok(());
        }
    }
    let effect_filter_style = if suppress_neo_filter {
        SUPPRESS_NEO_FILTER_STYLE
    } else {
        ""
    };
    let _ = write!(
        out,
        r#"<circle class="basic label-container" style="{}{}" r="{}" cx="0" cy="0"/>"#,
        escape_attr(body.style),
        effect_filter_style,
        fmt(r),
    );
    out.checkpoint()?;

    Ok(())
}

pub fn write_double_circle(
    out: &mut impl SvgOutput,
    mut body: NodeBodySvg<'_>,
    neo: bool,
    node_classes: &[String],
    source_styles: &[String],
    suppress_neo_filter: bool,
) -> Result<()> {
    let rings = DoubleCircleGeometry::from_outer_diameter(
        body.geometry.width.min(body.geometry.height),
        neo,
    );
    if body.geometry.hand_drawn {
        // Mermaid doubleCircle.ts overrides even user stroke widths for each ring.
        let outer = body.timed_rough(|geometry| {
            roughjs_paths_for_circle_with_roughness(
                rings.outer_radius * 2.0,
                2.5,
                geometry.stroke_dasharray,
                true,
                0.2,
                geometry.work_meter,
                geometry.randomness,
            )
        });
        body.geometry.work_meter.checkpoint(OperationPhase::Emit)?;
        let inner = body.timed_rough(|geometry| {
            roughjs_paths_for_circle_with_roughness(
                rings.inner_radius * 2.0,
                1.5,
                geometry.stroke_dasharray,
                true,
                0.2,
                geometry.work_meter,
                geometry.randomness,
            )
        });
        body.geometry.work_meter.checkpoint(OperationPhase::Emit)?;
        if let (Some(outer), Some(inner)) = (outer, inner) {
            let classes = std::iter::once("default")
                .chain(node_classes.iter().map(String::as_str))
                .collect::<Vec<_>>()
                .join(" ");
            let _ = write!(
                out,
                r#"<g class="{}" style="{}"><g>"#,
                escape_attr(&classes),
                escape_attr(&circle_group_style(&body, source_styles)),
            );
            write_rough_circle_paths(out, &body, &outer, 2.5, rings.outer_radius);
            out.push_str("</g><g>");
            write_rough_circle_paths(out, &body, &inner, 1.5, rings.inner_radius);
            out.push_str("</g></g>");
            out.checkpoint()?;
            return Ok(());
        }
    }
    let effect_filter_style = if suppress_neo_filter {
        SUPPRESS_NEO_FILTER_STYLE
    } else {
        ""
    };
    let child_style = body.grouped_child_style();
    let _ = write!(
        out,
        r#"<g class="basic label-container" style="{}{}"><circle class="outer-circle" cx="0" cy="0" r="{}" style="{}"#,
        escape_attr(body.style),
        effect_filter_style,
        fmt(rings.outer_radius),
        escape_attr(&child_style),
    );
    if suppress_neo_filter {
        out.push_str(SUPPRESS_NEO_FILTER_STYLE);
    }
    let _ = write!(
        out,
        r#""/><circle class="inner-circle" cx="0" cy="0" r="{}" style="{}"#,
        fmt(rings.inner_radius),
        escape_attr(&child_style)
    );
    if suppress_neo_filter {
        out.push_str(SUPPRESS_NEO_FILTER_STYLE);
    }
    out.push_str(r#""/></g>"#);
    out.checkpoint()?;

    Ok(())
}

fn hand_drawn_circle_style(styles: &[String]) -> String {
    // circle.ts and doubleCircle.ts use cssStyles, rather than the compiled nodeStyles.
    // Preserve that distinction while applying the renderer's usual safe-declaration filter.
    styles
        .iter()
        .flat_map(|raw| flowchart_split_mermaid_style_decls(raw))
        .filter_map(|decl| parse_safe_style_decl(decl).map(|(key, value)| format!("{key}:{value}")))
        .collect::<Vec<_>>()
        .join(",")
}

fn circle_group_style<'a>(body: &NodeBodySvg<'a>, source_styles: &[String]) -> Cow<'a, str> {
    if body.rough_fill_opacity.is_some() {
        Cow::Borrowed(body.style)
    } else {
        Cow::Owned(hand_drawn_circle_style(source_styles))
    }
}

pub(super) fn write_rough_circle_paths(
    out: &mut impl SvgOutput,
    body: &NodeBodySvg<'_>,
    paths: &(String, String),
    stroke_width: f32,
    radius: f64,
) {
    body.write_hachure(out, &circle_backing(body, radius), &paths.0, 1.5);
    // RoughGenerator.ellipse still generates the outline before the fill for PRNG ordering,
    // but excludes that opset from the drawable when the requested stroke is "none".
    if body.stroke_color != "none" {
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
            escape_attr(&paths.1),
            escape_attr(body.stroke_color),
            fmt(stroke_width as f64),
            escape_attr(body.geometry.stroke_dasharray),
        );
    }
}

fn diamond_path(w: f64, h: f64) -> String {
    let rough_points = [
        (0.0, -h / 2.0),
        (w / 2.0, 0.0),
        (w, -h / 2.0),
        (w / 2.0, -h),
    ];
    path_from_points(&rough_points)
}

pub fn advance_basic_rough_geometry(
    shape: FlowchartShape,
    geometry: NodeBodyGeometry<'_>,
    neo: bool,
) -> Result<()> {
    if !geometry.hand_drawn {
        return Ok(());
    }
    match shape {
        FlowchartShape::Diamond => {
            let path = diamond_path(geometry.width.max(1.0), geometry.height.max(1.0));
            let _ = roughjs_prepare_hachure_paths_for_svg_path(
                RoughGeometryTarget::Query,
                &path,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                FLOWCHART_DIAMOND_HAND_DRAWN_FILL_WEIGHT,
                FLOWCHART_DIAMOND_HAND_DRAWN_HACHURE_GAP,
                FLOWCHART_DIAMOND_HAND_DRAWN_ROUGHNESS,
                geometry.work_meter,
                geometry.randomness,
            );
        }
        FlowchartShape::Circle => {
            let diameter = geometry
                .width
                .max(1.0)
                .min(geometry.height.max(1.0))
                .max(1.0);
            let _ = roughjs_prepare_paths_for_circle_with_roughness(
                RoughGeometryTarget::Query,
                diameter,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                true,
                0.7,
                geometry.work_meter,
                geometry.randomness,
            );
        }
        FlowchartShape::DoubleCircle => {
            let rings =
                DoubleCircleGeometry::from_outer_diameter(geometry.width.min(geometry.height), neo);
            let _ = roughjs_prepare_paths_for_circle_with_roughness(
                RoughGeometryTarget::Query,
                rings.outer_radius * 2.0,
                2.5,
                geometry.stroke_dasharray,
                true,
                0.2,
                geometry.work_meter,
                geometry.randomness,
            );
            geometry.work_meter.checkpoint(OperationPhase::Layout)?;
            let _ = roughjs_prepare_paths_for_circle_with_roughness(
                RoughGeometryTarget::Query,
                rings.inner_radius * 2.0,
                1.5,
                geometry.stroke_dasharray,
                true,
                0.2,
                geometry.work_meter,
                geometry.randomness,
            );
            geometry.work_meter.checkpoint(OperationPhase::Layout)?;
        }
        _ => {}
    }
    Ok(())
}
