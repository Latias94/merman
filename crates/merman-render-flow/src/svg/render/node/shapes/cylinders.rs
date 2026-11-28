//! Facade label preparation for the Flowchart cylinder bodies.

use crate::node_body::CylinderBody;

use crate::svg::SvgOutput;
use crate::svg::types::{FlowchartRenderCtx, FlowchartRenderDetails};

use super::super::{FlowchartNodeLabelState, FlowchartNodeRenderCommon, helpers};
use super::body_svg;

pub(in crate::svg::render::node) fn render_cylinder(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) {
    // Mermaid applies an extra downward label shift of `node.padding / 1.5`.
    label.dy = ctx.node_padding / 1.5;
    let cylinder = CylinderBody::vertical(common.layout_node.width, common.layout_node.height);
    write_cylinder_body(out, ctx, common, details, cylinder);
}

pub(in crate::svg::render::node) fn render_horizontal_cylinder(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) {
    // Mermaid lays out the tilted cylinder's painted path from the label bbox.
    let metrics = helpers::compute_node_label_metrics(
        ctx,
        common.node_id,
        Some(common.layout_node),
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    );
    let (cylinder, dx) = CylinderBody::horizontal(
        metrics.width,
        metrics.height,
        ctx.node_padding,
        common.look_is_neo(),
    );
    label.dx = dx;
    if !ctx.node_html_labels {
        let node_text_style = helpers::node_source_text_style(
            ctx,
            Some(common.node_id),
            common.node_classes,
            common.node_styles,
        );
        label.dy -= ctx
            .measurer
            .measure_svg_create_text_bbox_y_offset_px(label.text, &node_text_style);
    }

    write_cylinder_body(out, ctx, common, details, cylinder);
}

pub(in crate::svg::render::node) fn render_lined_cylinder(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) {
    let (cylinder, dy) = CylinderBody::lined(common.layout_node.width, common.layout_node.height);
    label.dy = dy;
    write_cylinder_body(out, ctx, common, details, cylinder);
}

fn write_cylinder_body(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    details: &mut FlowchartRenderDetails,
    cylinder: CylinderBody,
) {
    // The canonical route renders edges before node bodies. Rough cylinders painted here
    // cannot affect later edges' randomness in the geometry query.
    cylinder.write(
        out,
        body_svg(common, details),
        ctx.layout
            .route_contract
            .is_canonical_axis()
            .then(|| ctx.document_ids().node(common.node_id))
            .flatten(),
        ctx.security_level_loose,
    );
}
