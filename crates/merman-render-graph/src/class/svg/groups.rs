#[cfg(feature = "layout-shoal")]
use super::ClassCanonicalRoutes;
use super::ClassSvgRelation;
use super::SvgOutput;
use super::context::{ClassEmitCheckpoint, ClassRenderDetails};
use super::edge::{
    ClassEdgeGroupsRenderContext, ClassEdgePathEmission, render_class_edge_labels,
    render_class_edge_paths,
};
use crate::class::model::{Bounds, LayoutEdge, LayoutPoint};
use merman_render_core::svg_css::SvgId as SvgDiagramId;
use merman_render_core::svg_output::timing::RenderTiming;
use merman_render_core::text::{TextMeasurer, TextStyle};
use rustc_hash::FxHashMap;

pub(super) struct ClassSplitEdgeGroupsRenderState<'a> {
    pub(super) content_bounds: &'a mut Option<Bounds>,
    pub(super) detail: &'a mut ClassRenderDetails,
}

pub(super) struct ClassSplitEdgeGroupsRenderContext<'a> {
    pub(super) edges: &'a [LayoutEdge],
    pub(super) clusters: &'a [merman_render_core::LayoutCluster],
    pub(super) native_style: Option<&'a crate::class::ClassNativeStyle>,
    pub(super) missing_section_points: &'a FxHashMap<&'a str, Vec<LayoutPoint>>,
    pub(super) work_meter: &'a merman_render_core::resources::OperationWorkMeter,
    pub(super) line_hop_paths: &'a std::collections::HashMap<&'a str, String>,
    pub(super) relations_by_id: &'a FxHashMap<&'a str, ClassSvgRelation<'a>>,
    pub(super) relation_index_by_id: &'a FxHashMap<&'a str, usize>,
    pub(super) prepared_svg_edge_labels:
        &'a FxHashMap<String, crate::class::ClassPreparedSvgEdgeLabel>,
    #[cfg(feature = "layout-shoal")]
    pub(super) canonical_routes: Option<&'a ClassCanonicalRoutes<'a>>,
    pub(super) diagram_marker_class: &'a str,
    pub(super) diagram_id: SvgDiagramId<'a>,
    pub(super) content_tx: f64,
    pub(super) content_ty: f64,
    pub(super) edge_use_html_labels: bool,
    pub(super) text_measurer: &'a dyn TextMeasurer,
    pub(super) terminal_text_style: &'a TextStyle,
    pub(super) mermaid_config: Option<&'a merman_model::presentation::PresentationConfig>,
    pub(super) math_renderer:
        Option<&'a (dyn merman_render_core::math::MathRenderer + Send + Sync)>,
    pub(super) look: &'a str,
    pub(super) hand_drawn_seed: roughr::core::RoughRandomness,
    pub(super) timing: RenderTiming,
    pub(super) uses_elk_adapter_dom: bool,
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
) -> merman_render_core::Result<()> {
    let ClassSplitEdgeGroupsRenderState {
        content_bounds,
        detail,
    } = state;
    let path_emission =
        render_class_split_edge_paths(out, content_bounds, detail, ctx, bounds_dx, bounds_dy)?;
    render_class_split_edge_labels(
        out,
        content_bounds,
        detail,
        ctx,
        bounds_dx,
        bounds_dy,
        &path_emission,
    )
}

pub(super) fn render_class_split_edge_paths<'edges, O: SvgOutput>(
    out: &mut O,
    content_bounds: &mut Option<Bounds>,
    detail: &mut ClassRenderDetails,
    ctx: &ClassSplitEdgeGroupsRenderContext<'edges>,
    bounds_dx: f64,
    bounds_dy: f64,
) -> merman_render_core::Result<ClassEdgePathEmission<'edges>> {
    let local_ctx = local_edge_context(ctx, bounds_dx, bounds_dy);
    render_class_edge_paths(out, content_bounds, detail, &local_ctx)
}

pub(super) fn render_class_split_edge_labels<O: SvgOutput>(
    out: &mut O,
    content_bounds: &mut Option<Bounds>,
    detail: &mut ClassRenderDetails,
    ctx: &ClassSplitEdgeGroupsRenderContext<'_>,
    bounds_dx: f64,
    bounds_dy: f64,
    path_emission: &ClassEdgePathEmission<'_>,
) -> merman_render_core::Result<()> {
    let local_ctx = local_edge_context(ctx, bounds_dx, bounds_dy);
    render_class_edge_labels(out, content_bounds, detail, &local_ctx, path_emission)
}

fn local_edge_context<'a>(
    ctx: &ClassSplitEdgeGroupsRenderContext<'a>,
    bounds_dx: f64,
    bounds_dy: f64,
) -> ClassEdgeGroupsRenderContext<'a> {
    ClassEdgeGroupsRenderContext {
        edges: ctx.edges,
        clusters: ctx.clusters,
        native_style: ctx.native_style,
        missing_section_points: ctx.missing_section_points,
        work_meter: ctx.work_meter,
        line_hop_paths: ctx.line_hop_paths,
        relations_by_id: ctx.relations_by_id,
        relation_index_by_id: ctx.relation_index_by_id,
        prepared_svg_edge_labels: ctx.prepared_svg_edge_labels,
        #[cfg(feature = "layout-shoal")]
        canonical_routes: ctx.canonical_routes,
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
        uses_elk_adapter_dom: ctx.uses_elk_adapter_dom,
        edge_paths_class: ctx.edge_paths_class,
        relation_theme: ctx.relation_theme,
        emit: ctx.emit,
    }
}
