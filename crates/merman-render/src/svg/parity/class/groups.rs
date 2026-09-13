use super::super::SvgOutput;
use super::super::timing::RenderTiming;
use super::ClassSvgRelation;
use super::context::{ClassEmitCheckpoint, ClassRenderDetails};
use super::edge::{
    ClassEdgeGroupsRenderContext, ClassEdgeLabelCenters, render_class_edge_labels,
    render_class_edge_paths,
};
use crate::model::{Bounds, LayoutEdge};
use crate::svg::parity::SvgDiagramId;
use crate::text::{TextMeasurer, TextStyle};
use rustc_hash::FxHashMap;

pub(super) struct ClassSplitEdgeGroupsRenderState<'a> {
    pub(super) content_bounds: &'a mut Option<Bounds>,
    pub(super) detail: &'a mut ClassRenderDetails,
    pub(super) theme_receipt: &'a mut crate::class::ClassRelationThemeReceipt,
    pub(super) typography_receipt: &'a mut Option<crate::class::ClassTextThemeReceipt>,
}

pub(super) struct ClassSplitEdgeGroupsRenderContext<'a> {
    pub(super) edges: &'a [LayoutEdge],
    pub(super) relations_by_id: &'a FxHashMap<&'a str, &'a ClassSvgRelation>,
    pub(super) relation_index_by_id: &'a FxHashMap<&'a str, usize>,
    pub(super) diagram_marker_class: &'a str,
    pub(super) diagram_id: SvgDiagramId<'a>,
    pub(super) content_tx: f64,
    pub(super) content_ty: f64,
    pub(super) edge_use_html_labels: bool,
    pub(super) text_measurer: &'a dyn TextMeasurer,
    pub(super) terminal_text_style: &'a TextStyle,
    pub(super) mermaid_config: Option<&'a merman_core::MermaidConfig>,
    pub(super) math_renderer: Option<&'a (dyn crate::math::MathRenderer + Send + Sync)>,
    pub(super) look: &'a str,
    pub(super) hand_drawn_seed: roughr::core::RoughRandomness,
    pub(super) timing: RenderTiming,
    pub(super) edge_paths_class: &'static str,
    pub(super) relation_theme: &'a crate::class::ClassRelationThemePlan,
    pub(super) emit: ClassEmitCheckpoint<'a>,
}

pub(super) fn render_class_split_edge_groups<O: SvgOutput>(
    out: &mut O,
    state: ClassSplitEdgeGroupsRenderState<'_>,
    ctx: &ClassSplitEdgeGroupsRenderContext<'_>,
    bounds_dx: f64,
    bounds_dy: f64,
) -> crate::Result<()> {
    let ClassSplitEdgeGroupsRenderState {
        content_bounds,
        detail,
        theme_receipt,
        typography_receipt,
    } = state;
    let edge_label_centers = render_class_split_edge_paths(
        out,
        content_bounds,
        detail,
        theme_receipt,
        ctx,
        bounds_dx,
        bounds_dy,
    )?;
    render_class_split_edge_labels(
        out,
        content_bounds,
        detail,
        theme_receipt,
        typography_receipt,
        ctx,
        bounds_dx,
        bounds_dy,
        &edge_label_centers,
    )
}

pub(super) fn render_class_split_edge_paths<O: SvgOutput>(
    out: &mut O,
    content_bounds: &mut Option<Bounds>,
    detail: &mut ClassRenderDetails,
    theme_receipt: &mut crate::class::ClassRelationThemeReceipt,
    ctx: &ClassSplitEdgeGroupsRenderContext<'_>,
    bounds_dx: f64,
    bounds_dy: f64,
) -> crate::Result<ClassEdgeLabelCenters> {
    let local_ctx = local_edge_context(ctx, bounds_dx, bounds_dy);
    render_class_edge_paths(out, content_bounds, detail, theme_receipt, &local_ctx)
}

pub(super) fn render_class_split_edge_labels<O: SvgOutput>(
    out: &mut O,
    content_bounds: &mut Option<Bounds>,
    detail: &mut ClassRenderDetails,
    theme_receipt: &mut crate::class::ClassRelationThemeReceipt,
    typography_receipt: &mut Option<crate::class::ClassTextThemeReceipt>,
    ctx: &ClassSplitEdgeGroupsRenderContext<'_>,
    bounds_dx: f64,
    bounds_dy: f64,
    edge_label_centers: &ClassEdgeLabelCenters,
) -> crate::Result<()> {
    let local_ctx = local_edge_context(ctx, bounds_dx, bounds_dy);
    render_class_edge_labels(
        out,
        content_bounds,
        detail,
        theme_receipt,
        typography_receipt,
        &local_ctx,
        edge_label_centers,
    )
}

fn local_edge_context<'a>(
    ctx: &ClassSplitEdgeGroupsRenderContext<'a>,
    bounds_dx: f64,
    bounds_dy: f64,
) -> ClassEdgeGroupsRenderContext<'a> {
    ClassEdgeGroupsRenderContext {
        edges: ctx.edges,
        relations_by_id: ctx.relations_by_id,
        relation_index_by_id: ctx.relation_index_by_id,
        diagram_marker_class: ctx.diagram_marker_class,
        diagram_id: ctx.diagram_id,
        content_tx: ctx.content_tx,
        content_ty: ctx.content_ty,
        bounds_dx,
        bounds_dy,
        edge_use_html_labels: ctx.edge_use_html_labels,
        text_measurer: ctx.text_measurer,
        terminal_text_style: ctx.terminal_text_style,
        mermaid_config: ctx.mermaid_config,
        math_renderer: ctx.math_renderer,
        look: ctx.look,
        hand_drawn_seed: ctx.hand_drawn_seed.clone(),
        timing: ctx.timing,
        edge_paths_class: ctx.edge_paths_class,
        relation_theme: ctx.relation_theme,
        emit: ctx.emit,
    }
}
