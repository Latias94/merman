//! Flowchart v2 basic geometry shapes.

use crate::svg::parity::flowchart::{OptionalStyleAttr, escape_attr};
use crate::svg::parity::{fmt, fmt_display};

use super::super::geom::path_from_points;
use super::super::roughjs::{
    roughjs_hachure_paths_for_svg_path, roughjs_paths_for_circle,
    roughjs_paths_for_circle_with_roughness,
};

const FLOWCHART_DIAMOND_HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const FLOWCHART_DIAMOND_HAND_DRAWN_FILL_WEIGHT: f32 = 1.5;
const FLOWCHART_DIAMOND_HAND_DRAWN_HACHURE_GAP: f32 = 1.5;

pub(in crate::svg::parity::flowchart::render::node) fn render_diamond(
    out: &mut impl crate::svg::parity::SvgOutput,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) {
    let w = common.layout_node.width.max(1.0);
    let h = common.layout_node.height.max(1.0);
    let tx = -w / 2.0 + if common.centered_diamond { 0.0 } else { 0.5 };
    let ty = h / 2.0;
    if common.look_is_hand_drawn() {
        // RoughJS normalizes this path to a left-edge hachure start. `roughr` is start-point
        // sensitive for this diamond, so feed the same polygon from the left vertex to keep the
        // hand-drawn silhouette centered instead of visually skewing into a parallelogram.
        let path_data = diamond_path(w, h);
        if let Some((fill_d, stroke_d)) =
            super::super::helpers::timed_node_roughjs(common.timing, details, || {
                roughjs_hachure_paths_for_svg_path(
                    &path_data,
                    common.stroke_width,
                    common.stroke_dasharray,
                    FLOWCHART_DIAMOND_HAND_DRAWN_FILL_WEIGHT,
                    FLOWCHART_DIAMOND_HAND_DRAWN_HACHURE_GAP,
                    FLOWCHART_DIAMOND_HAND_DRAWN_ROUGHNESS,
                    common.work_meter,
                    common.hand_drawn_seed,
                )
            })
        {
            let _ = write!(
                out,
                r#"<g transform="translate({},{})" style="{}">"#,
                fmt(tx),
                fmt(ty),
                escape_attr(super::super::helpers::hand_drawn_group_style(common)),
            );
            super::super::helpers::write_hand_drawn_hachure(
                out,
                common,
                &path_data,
                &fill_d,
                f64::from(FLOWCHART_DIAMOND_HAND_DRAWN_FILL_WEIGHT),
            );
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
                escape_attr(&stroke_d),
                escape_attr(common.stroke_color),
                fmt_display(common.stroke_width as f64),
                escape_attr(common.stroke_dasharray),
            );
            return;
        }
    }

    let effect_style;
    let body_style = if common.effect_filter_style.is_empty() {
        common.style
    } else {
        effect_style = format!("{}{}", common.style, common.effect_filter_style);
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

pub(in crate::svg::parity::flowchart::render::node) fn render_circle(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) -> crate::Result<()> {
    let w = common.layout_node.width.max(1.0);
    let h = common.layout_node.height.max(1.0);
    let r = (w.min(h) / 2.0).max(0.5);
    if common.look_is_hand_drawn() {
        let paths = super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_circle(
                r * 2.0,
                common.stroke_width,
                common.stroke_dasharray,
                true,
                common.work_meter,
                common.hand_drawn_seed,
            )
        });
        ctx.checkpoint_emit()?;
        if let Some(paths) = paths {
            let _ = write!(
                out,
                r#"<g class="basic label-container" style="{}">"#,
                escape_attr(&circle_group_style(common)),
            );
            write_rough_circle_paths(out, common, &paths, common.stroke_width, r);
            out.push_str("</g>");
            out.checkpoint()?;
            return Ok(());
        }
    }
    let _ = write!(
        out,
        r#"<circle class="basic label-container" style="{}{}" r="{}" cx="0" cy="0"/>"#,
        escape_attr(common.style),
        common.effect_filter_style,
        fmt(r),
    );
    out.checkpoint()?;

    Ok(())
}

pub(in crate::svg::parity::flowchart::render::node) fn render_double_circle(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) -> crate::Result<()> {
    let geometry = crate::flowchart::DoubleCircleGeometry::from_outer_diameter(
        common.layout_node.width.min(common.layout_node.height),
        common.look_is_neo(),
    );
    if common.look_is_hand_drawn() {
        // Mermaid doubleCircle.ts overrides even user stroke widths for each ring.
        let outer = super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_circle_with_roughness(
                geometry.outer_radius * 2.0,
                2.5,
                common.stroke_dasharray,
                true,
                0.2,
                common.work_meter,
                common.hand_drawn_seed,
            )
        });
        ctx.checkpoint_emit()?;
        let inner = super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_circle_with_roughness(
                geometry.inner_radius * 2.0,
                1.5,
                common.stroke_dasharray,
                true,
                0.2,
                common.work_meter,
                common.hand_drawn_seed,
            )
        });
        ctx.checkpoint_emit()?;
        if let (Some(outer), Some(inner)) = (outer, inner) {
            let classes = std::iter::once("default")
                .chain(common.node_classes.iter().map(String::as_str))
                .collect::<Vec<_>>()
                .join(" ");
            let _ = write!(
                out,
                r#"<g class="{}" style="{}"><g>"#,
                escape_attr(&classes),
                escape_attr(&circle_group_style(common)),
            );
            write_rough_circle_paths(out, common, &outer, 2.5, geometry.outer_radius);
            out.push_str("</g><g>");
            write_rough_circle_paths(out, common, &inner, 1.5, geometry.inner_radius);
            out.push_str("</g></g>");
            out.checkpoint()?;
            return Ok(());
        }
    }
    let child_style = super::super::helpers::grouped_shape_child_style(common);
    let _ = write!(
        out,
        r#"<g class="basic label-container" style="{}{}"><circle class="outer-circle" cx="0" cy="0" r="{}" style="{}"#,
        escape_attr(common.style),
        common.effect_filter_style,
        fmt(geometry.outer_radius),
        escape_attr(&child_style),
    );
    common.write_child_filter_style(out);
    let _ = write!(
        out,
        r#""/><circle class="inner-circle" cx="0" cy="0" r="{}" style="{}"#,
        fmt(geometry.inner_radius),
        escape_attr(&child_style)
    );
    common.write_child_filter_style(out);
    out.push_str(r#""/></g>"#);
    out.checkpoint()?;

    Ok(())
}

fn hand_drawn_circle_style(styles: &[String]) -> String {
    // circle.ts and doubleCircle.ts use cssStyles, rather than the compiled nodeStyles.
    // Preserve that distinction while applying the renderer's usual safe-declaration filter.
    styles
        .iter()
        .flat_map(|raw| crate::flowchart::flowchart_split_mermaid_style_decls(raw))
        .filter_map(|decl| {
            crate::mermaid_style::parse_safe_style_decl(decl)
                .map(|(key, value)| format!("{key}:{value}"))
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn circle_group_style<'a>(
    common: &super::super::FlowchartNodeRenderCommon<'a>,
) -> std::borrow::Cow<'a, str> {
    if common.solid_rough_fill {
        std::borrow::Cow::Borrowed(common.style)
    } else {
        std::borrow::Cow::Owned(hand_drawn_circle_style(common.node_styles))
    }
}

fn write_rough_circle_paths(
    out: &mut impl crate::svg::parity::SvgOutput,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    paths: &(String, String),
    stroke_width: f32,
    radius: f64,
) {
    let outline = if common.solid_rough_fill {
        super::super::helpers::circle_outline(radius)
    } else {
        String::new()
    };
    super::super::helpers::write_hand_drawn_hachure(out, common, &outline, &paths.0, 1.5);
    // RoughGenerator.ellipse still generates the outline before the fill for PRNG ordering,
    // but excludes that opset from the drawable when the requested stroke is "none".
    if common.stroke_color != "none" {
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
            escape_attr(&paths.1),
            escape_attr(common.stroke_color),
            fmt_display(stroke_width as f64),
            escape_attr(common.stroke_dasharray),
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

pub(super) fn advance_rough_geometry(
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeGeometry<'_>,
) -> crate::Result<()> {
    use super::super::roughjs::{
        RoughGeometryTarget, roughjs_prepare_hachure_paths_for_svg_path,
        roughjs_prepare_paths_for_circle_with_roughness,
    };
    use crate::flowchart::{FlowchartShape, resolve_flowchart_shape};
    if !common.look_is_hand_drawn() {
        return Ok(());
    }
    match resolve_flowchart_shape(common.shape)? {
        FlowchartShape::Diamond => {
            let path = diamond_path(
                common.layout_node.width.max(1.0),
                common.layout_node.height.max(1.0),
            );
            let _ = roughjs_prepare_hachure_paths_for_svg_path(
                RoughGeometryTarget::Query,
                &path,
                common.stroke_width,
                common.stroke_dasharray,
                FLOWCHART_DIAMOND_HAND_DRAWN_FILL_WEIGHT,
                FLOWCHART_DIAMOND_HAND_DRAWN_HACHURE_GAP,
                FLOWCHART_DIAMOND_HAND_DRAWN_ROUGHNESS,
                common.work_meter,
                common.hand_drawn_seed,
            );
        }
        FlowchartShape::Circle => {
            let diameter = common
                .layout_node
                .width
                .max(1.0)
                .min(common.layout_node.height.max(1.0))
                .max(1.0);
            let _ = roughjs_prepare_paths_for_circle_with_roughness(
                RoughGeometryTarget::Query,
                diameter,
                common.stroke_width,
                common.stroke_dasharray,
                true,
                0.7,
                common.work_meter,
                common.hand_drawn_seed,
            );
        }
        FlowchartShape::DoubleCircle => {
            let geometry = crate::flowchart::DoubleCircleGeometry::from_outer_diameter(
                common.layout_node.width.min(common.layout_node.height),
                common.look_is_neo(),
            );
            let _ = roughjs_prepare_paths_for_circle_with_roughness(
                RoughGeometryTarget::Query,
                geometry.outer_radius * 2.0,
                2.5,
                common.stroke_dasharray,
                true,
                0.2,
                common.work_meter,
                common.hand_drawn_seed,
            );
            ctx.work_meter
                .checkpoint(merman_model::OperationPhase::Layout)
                .map_err(crate::Error::from)?;
            let _ = roughjs_prepare_paths_for_circle_with_roughness(
                RoughGeometryTarget::Query,
                geometry.inner_radius * 2.0,
                1.5,
                common.stroke_dasharray,
                true,
                0.2,
                common.work_meter,
                common.hand_drawn_seed,
            );
            ctx.work_meter
                .checkpoint(merman_model::OperationPhase::Layout)
                .map_err(crate::Error::from)?;
        }
        _ => {}
    }
    Ok(())
}
