//! Facade label measurement and placement for the Flowchart document shapes.

use merman_render_flow::node_body::document_shapes::{
    DocumentLabelOffset, LinedWaveDocumentBody, StackedDocumentBody, TaggedWaveDocumentBody,
    WaveDocumentBody,
};

use crate::svg::parity::SvgOutput;
use crate::svg::parity::flowchart::types::{FlowchartRenderCtx, FlowchartRenderDetails};

use super::super::{FlowchartNodeGeometry, FlowchartNodeLabelState, FlowchartNodeRenderCommon};
use super::{body_geometry, body_svg, shape_label_metrics};

pub(in super::super) fn render_wave_document(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) {
    let document = prepare_wave_document(ctx, common, label);
    let offset = document.write(out, body_svg(common, details));
    place_label(label, offset);
}

pub(super) fn advance_wave_document_rough_geometry(
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeGeometry<'_>,
    label: &FlowchartNodeLabelState<'_>,
) {
    prepare_wave_document(ctx, common, label).advance_rough_geometry(body_geometry(common));
}

fn prepare_wave_document(
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeGeometry<'_>,
    label: &FlowchartNodeLabelState<'_>,
) -> WaveDocumentBody {
    let metrics = shape_label_metrics(ctx, common, label, Some(common.layout_node));
    WaveDocumentBody::from_label(
        metrics.width,
        metrics.height,
        ctx.node_padding,
        common.look_is_neo(),
    )
}

pub(in super::super) fn render_lined_wave_document(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) {
    let document = prepare_lined_document(ctx, common, label);
    document.write(out, body_svg(common, details));
}

pub(super) fn advance_lined_wave_document_rough_geometry(
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeGeometry<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
) {
    prepare_lined_document(ctx, common, label).advance_rough_geometry(body_geometry(common));
}

fn prepare_lined_document(
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeGeometry<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
) -> LinedWaveDocumentBody {
    let metrics = shape_label_metrics(ctx, common, label, Some(common.layout_node));
    let document = LinedWaveDocumentBody::from_label(
        metrics.width,
        metrics.height,
        ctx.node_padding,
        common.look_is_neo(),
    );
    place_label(label, document.label);
    document
}

pub(in super::super) fn render_tagged_wave_document(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) {
    let document = prepare_tagged_document(ctx, common, label);
    document.write(out, body_svg(common, details));
}

pub(super) fn advance_tagged_wave_document_rough_geometry(
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeGeometry<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
) {
    prepare_tagged_document(ctx, common, label).advance_rough_geometry(body_geometry(common));
}

fn prepare_tagged_document(
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeGeometry<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
) -> TaggedWaveDocumentBody {
    let metrics = shape_label_metrics(ctx, common, label, Some(common.layout_node));
    let document =
        TaggedWaveDocumentBody::from_label(metrics.width, metrics.height, ctx.node_padding);
    place_label(label, document.label);
    document
}

pub(in super::super) fn render_stacked_document(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) {
    let document = prepare_stacked_document(ctx, common, label);
    document.write(out, body_svg(common, details));
}

pub(super) fn advance_stacked_document_rough_geometry(
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeGeometry<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
) {
    prepare_stacked_document(ctx, common, label).advance_rough_geometry(body_geometry(common));
}

fn prepare_stacked_document(
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeGeometry<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
) -> StackedDocumentBody {
    let metrics = shape_label_metrics(ctx, common, label, Some(common.layout_node));
    let geometry = crate::flowchart::flowchart_stacked_document_geometry(
        metrics.width,
        metrics.height,
        ctx.node_padding,
        common.look_is_neo(),
    );
    label.dx = geometry.label_dx;
    label.dy = geometry.label_dy;
    StackedDocumentBody::from_points(
        geometry.group_dy,
        &geometry.outer_points,
        &geometry.inner_points,
    )
}

fn place_label(label: &mut FlowchartNodeLabelState<'_>, offset: DocumentLabelOffset) {
    label.dx = offset.dx;
    label.dy = offset.dy;
}
