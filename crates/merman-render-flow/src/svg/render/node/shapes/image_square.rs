//! Flowchart v2 image square shape.

use crate::node_geometry::{ImageSquareGeometry, ImageSquareSizing};
use crate::svg::types::{FlowchartRenderCtx, FlowchartRenderDetails};
use crate::svg::{escape_attr_display, fmt_display};
use merman_model::OperationPhase;

use super::super::roughjs::roughjs_stroke_path_for_svg_path;
use super::super::{FlowchartNodeGeometry, FlowchartNodeLabelState, FlowchartNodeRenderCommon};

pub(in crate::svg::render::node) fn try_render_image_square(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    label: &FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) -> merman_render_core::Result<bool> {
    let Some(href) = common.node_img.filter(|href| !href.trim().is_empty()) else {
        return Ok(false);
    };
    let geometry = image_geometry(ctx, common, label, href, OperationPhase::Emit)?;
    let image = geometry.image;
    let image_x = -image.width / 2.0;
    let image_top = geometry.image_top();
    let shape_y = image_top + image.height / 2.0;
    let fill_path = image_fill_path(image);
    let _ = write!(
        out,
        r#"<g transform="translate(0,{})"><path d="{}" stroke="none" stroke-width="0" fill="{}"/>"#,
        fmt_display(shape_y),
        escape_attr_display(&fill_path),
        escape_attr_display(common.fill_color)
    );
    let stroke_path = close_image_path(fill_path, image);
    if let Some(stroke_d) =
        super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_stroke_path_for_svg_path(
                &stroke_path,
                common.stroke_width,
                common.stroke_dasharray,
                common.work_meter,
                common.hand_drawn_seed,
            )
        })
    {
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
            escape_attr_display(&stroke_d),
            escape_attr_display(common.stroke_color),
            fmt_display(common.stroke_width as f64),
            escape_attr_display(common.stroke_dasharray)
        );
    }
    out.push_str("</g>");

    let label_y = if geometry.top_label {
        -image.height / 2.0 - geometry.label.height / 2.0 - geometry.label_padding / 2.0
    } else {
        image.height / 2.0 - geometry.label.height / 2.0 + geometry.label_padding / 2.0
    };
    common.label_emission.write_special_html_label(
        out,
        ctx,
        common,
        label,
        details,
        -geometry.label.width / 2.0,
        label_y,
        geometry.label.width,
        geometry.label.height,
    );

    let outer = geometry.outer;
    let outer_x = -outer.width / 2.0;
    let outer_y = -outer.height / 2.0;
    let outer_path = format!(
        "M{outer_x} {outer_y} L{} {outer_y} L{} {} L{outer_x} {}",
        outer.width / 2.0,
        outer.width / 2.0,
        outer.height / 2.0,
        outer.height / 2.0,
    );
    let _ = write!(
        out,
        r#"<g><path d="{}" stroke="none" stroke-width="0" fill="none"/></g>"#,
        escape_attr_display(&outer_path)
    );
    let _ = write!(
        out,
        r#"<image href="{}" width="{}" height="{}" preserveAspectRatio="none" transform="translate({},{})"/>"#,
        escape_attr_display(href),
        fmt_display(image.width),
        fmt_display(image.height),
        fmt_display(image_x),
        fmt_display(image_top)
    );

    out.push_str("</g>");
    if common.wrapped_in_a {
        out.push_str("</a>");
    }
    Ok(true)
}

fn image_geometry(
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeGeometry<'_>,
    label: &FlowchartNodeLabelState<'_>,
    href: &str,
    phase: OperationPhase,
) -> merman_render_core::Result<ImageSquareGeometry> {
    if let Some(geometry) = ctx.node_image(common.node_id) {
        return Ok(geometry);
    }
    // Low-level geometry controls can supply a layout without the prepared label sidecar.
    ImageSquareGeometry::resolve(
        ImageSquareSizing {
            href,
            width: common.node_asset_width,
            height: common.node_asset_height,
            constraint_on: common.node_constraint == Some("on"),
        },
        common.label_metrics(ctx, label),
        !label.text.is_empty(),
        ctx.wrapping_width,
        common.node_pos == Some("t"),
        |units| {
            common
                .work_meter
                .charge_work(units, phase)
                .map_err(Into::into)
        },
    )
}

fn image_fill_path(size: kurbo::Size) -> String {
    let x = size.width / 2.0;
    let y = size.height / 2.0;
    format!(
        "M{} {} L{} {} L{} {} L{} {}",
        fmt_display(-x),
        fmt_display(-y),
        fmt_display(x),
        fmt_display(-y),
        fmt_display(x),
        fmt_display(y),
        fmt_display(-x),
        fmt_display(y),
    )
}

fn close_image_path(mut path: String, size: kurbo::Size) -> String {
    use std::fmt::Write as _;

    let _ = write!(
        path,
        " L{} {}",
        fmt_display(-size.width / 2.0),
        fmt_display(-size.height / 2.0)
    );
    path
}

pub(super) fn advance_rough_geometry(
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeGeometry<'_>,
    label: &FlowchartNodeLabelState<'_>,
) -> merman_render_core::Result<()> {
    if let Some(href) = common.node_img.filter(|href| !href.trim().is_empty()) {
        let geometry = image_geometry(ctx, common, label, href, OperationPhase::Layout)?;
        let path = close_image_path(image_fill_path(geometry.image), geometry.image);
        let _ = super::super::roughjs::roughjs_prepare_stroke_path_for_svg_path(
            super::super::roughjs::RoughGeometryTarget::Query,
            &path,
            common.stroke_width,
            common.stroke_dasharray,
            common.work_meter,
            common.hand_drawn_seed,
        );
    }
    Ok(())
}
