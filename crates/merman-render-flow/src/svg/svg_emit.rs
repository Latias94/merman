use crate::document::{FlowchartSvgDocumentRequest, prepare_flowchart_svg_document};
use crate::graph::FlowchartGraphTimings;
use crate::stylesheet::FlowchartStylesheetRequest;

use super::defs::{bind_marker_path_sources_for, prepare_flowchart_markers};
use super::edge_fill::FlowchartEdgeFillPlan;
use super::render_config::FlowchartRenderConfig;
use super::viewbox::{
    FlowchartRenderedBoundsRequest, FlowchartViewboxBounds, FlowchartViewboxBoundsRequest,
    prepare_flowchart_rendered_bounds, prepare_flowchart_viewbox_bounds,
};
use super::*;

#[derive(Clone, Copy)]
pub struct FlowchartSvgModelRequest<'a> {
    pub render_config: &'a FlowchartRenderConfig,
    pub prepared_nodes: &'a crate::node_inventory::FlowchartPreparedNodes,
    pub prepared_theme: &'a crate::theme::FlowchartPreparedTheme,
    pub layout: FlowchartSvgLayout<'a>,
    pub swimlane_layout: Option<SwimlaneSvgLayout<'a>>,
    pub model: &'a merman_model::families::flowchart::FlowchartModel,
    pub render_context: &'a merman_model::families::flowchart::FlowchartRenderContext,
    pub effective_config: &'a merman_model::presentation::PresentationConfig,
    pub diagram_type: &'a str,
    pub diagram_title: Option<&'a str>,
    pub svg_label_sidecar: &'a crate::svg_label::FlowchartSvgLabelSidecar,

    pub edge_style_plan: &'a FlowchartEdgeStylePlan,
}

pub fn render_parity(
    request: FlowchartSvgModelRequest<'_>,
    services: &FlowchartSvgServices<'_>,
) -> Result<root_svg::RootedSvg> {
    let FlowchartSvgModelRequest {
        render_config,
        prepared_nodes: _,
        prepared_theme,
        layout: _,
        swimlane_layout,
        model,
        render_context: _,
        effective_config,
        diagram_type,
        diagram_title,
        svg_label_sidecar: _,
        edge_style_plan: _,
    } = request;
    super::paint::with_flowchart_paint_context(
        request,
        services,
        |ctx, render_edges, hierarchy_plan| {
            let render_timing = services.timing;
            let mut timings = timing::RenderTimings::default();
            let total_timer = render_timing.start();
            let document_ids = ctx.document_ids();
            let document_scope = document_ids.scope();
            let diagram_id = ctx.diagram_id;
            let font_family = &render_config.font_family;
            let font_size = render_config.font_size;
            let diagram_padding = render_config.diagram_padding;
            let use_max_width = render_config.use_max_width;
            let title_top_margin = render_config.title_top_margin;
            let mut edge_fills = FlowchartEdgeFillPlan::default();
            let adaptive_edges = FlowchartEdgeFillPlan::applies(ctx);
            let marker_plan = if adaptive_edges {
                // Route painting consumes stroke widths before fill-aware marker colors are known.
                for edge in hierarchy_plan.ordered_edges() {
                    bind_marker_path_sources_for(ctx, hierarchy_plan, edge.key)?;
                }
                None
            } else {
                Some(prepare_flowchart_markers(ctx, hierarchy_plan, &edge_fills)?)
            };

            let subgraph_title_y_shift =
                crate::layout_config::FlowchartConfigView::new(effective_config)
                    .render_subgraph_title_y_shift();

            let mut detail = FlowchartRenderDetails::default();
            let mut graph_timings = FlowchartGraphTimings::default();
            let mut viewbox_edge_curve_bounds = std::time::Duration::ZERO;
            let _g_viewbox = render_timing.section(&mut timings.viewbox);

            let bounds = prepare_flowchart_rendered_bounds(
                FlowchartRenderedBoundsRequest {
                    ctx,
                    subgraph_title_y_shift,
                },
                hierarchy_plan,
            )?;
            let FlowchartViewboxBounds {
                diagram_title,
                title_anchor_x,
                source_paint_bounds,
                mut bbox_min_x,
                mut bbox_min_y,
                mut bbox_max_x,
                mut bbox_max_y,
                mut edge_path_cache,
            } = prepare_flowchart_viewbox_bounds(
                FlowchartViewboxBoundsRequest {
                    ctx,
                    render_edges,
                    base_bounds: bounds,
                    diagram_title,
                    font_family,
                    title_top_margin,
                    timing: render_timing,
                    viewbox_edge_curve_bounds: &mut viewbox_edge_curve_bounds,
                    detail: &mut detail,
                },
                hierarchy_plan,
            )?;

            let cutout_labels = super::edge_label_cutout::applies(ctx, services);
            if super::paint::paints_rough_edges_first(ctx, adaptive_edges, cutout_labels) {
                super::paint::prepare_rough_edge_paths(
                    ctx,
                    hierarchy_plan.ordered_edges(),
                    &mut edge_path_cache,
                )?;
            }
            #[cfg(feature = "layout-shoal")]
            let source_paint_bounds =
                super::viewbox::include_sketch_shafts(ctx, source_paint_bounds, &edge_path_cache)?;

            let marker_plan = match marker_plan {
                Some(plan) => plan,
                None => {
                    edge_fills = FlowchartEdgeFillPlan::prepare(
                        ctx,
                        hierarchy_plan,
                        &edge_path_cache,
                        services,
                    )?;
                    prepare_flowchart_markers(ctx, hierarchy_plan, &edge_fills)?
                }
            };

            let mut edge_labels =
                super::paint::FlowchartEdgeLabels::prepare(ctx, hierarchy_plan, &edge_path_cache)?;
            let step_badges = super::step::prepare_step_badges(
                ctx,
                hierarchy_plan,
                &edge_path_cache,
                &edge_labels,
            )?;
            step_badges.include_bounds(
                &mut bbox_min_x,
                &mut bbox_min_y,
                &mut bbox_max_x,
                &mut bbox_max_y,
            );
            let document = prepare_flowchart_svg_document(FlowchartSvgDocumentRequest {
                root_viewport: services.root_viewport.clone(),
                document_scope,
                uses_elk_adapter_dom: ctx.uses_elk_adapter_dom,
                use_max_width,
                diagram_padding,
                source_paint_bounds,
                bbox_min_x,
                bbox_min_y,
                bbox_max_x,
                bbox_max_y,
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
                diagram_title: diagram_title.as_deref(),
                title_anchor_x,
                title_top_margin,
            });

            drop(_g_viewbox);
            let _g_render_svg = render_timing.section(&mut timings.render_svg);

            let swimlane_styles = swimlane_layout.is_some();
            #[cfg(feature = "layout-grid")]
            let swimlane_styles = swimlane_styles
                || diagram_type == "swimlane" && ctx.layout.route_contract.grid_paint().is_some();
            let document = document.write(
                services.work_meter,
                FlowchartStylesheetRequest {
                    diagram_id,
                    diagram_type,
                    drop_shadow_id: document_scope.drop_shadow(),
                    drop_shadow_small_id: document_scope.drop_shadow_small(),
                    gradient_id: document_scope.root_gradient(),
                    theme: prepared_theme,
                    palette: &ctx.palette,
                    font_family,
                    font_size,
                    class_defs: &model.class_defs,
                    swimlane: swimlane_styles,
                    allow_deferred_palette: services.defer_palette_stylesheet,
                },
                |out| {
                    edge_fills.write_clips(out, ctx)?;
                    let label_cutouts = if cutout_labels {
                        super::edge_label_cutout::write_definitions(
                            out,
                            ctx,
                            &edge_path_cache,
                            &edge_labels,
                            kurbo::Rect::new(bbox_min_x, bbox_min_y, bbox_max_x, bbox_max_y)
                                .inflate(diagram_padding, diagram_padding),
                        )?
                    } else {
                        FxHashSet::default()
                    };
                    let mut paint = render::FlowchartRootPaint {
                        ctx,
                        timing: render_timing,
                        details: &mut detail,
                        node_label_div_style: String::new(),
                        edge_cache: &mut edge_path_cache,
                        hierarchy_plan,
                        marker_plan: &marker_plan,
                        edge_fills: &edge_fills,
                        label_cutouts: &label_cutouts,
                        edge_labels: &mut edge_labels,
                        step_badges: &step_badges,
                    };
                    paint.write_graph(out, &mut graph_timings)?;
                    Ok(ctx.steps.emitted())
                },
            )?;

            drop(_g_render_svg);
            timings.total = total_timer
                .map(merman_model::runtime::OperationTimer::elapsed)
                .unwrap_or_default();
            if render_timing.is_enabled() {
                eprintln!(
                    "[render-timing] diagram=flowchart-v2 total={:?} deserialize={:?} build_ctx={:?} viewbox={:?} viewbox_edge_curve_bounds={:?} viewbox_edge_root_lookup={:?} viewbox_edge_curve_offsets={:?} viewbox_edge_curve_geom={:?} viewbox_edge_curve_bbox_union={:?} viewbox_edge_curve_geom_calls={} viewbox_edge_curve_geom_skipped_bounds={} render_svg={:?} finalize={:?} root_calls={} clusters={:?} edges_select={:?} edge_paths={:?} edge_labels={:?} dom_order={:?} nodes={:?} node_style_compile={:?} node_roughjs={:?} node_roughjs_calls={} node_label_html={:?} node_label_html_calls={} nested_roots={:?}",
                    timings.total,
                    timings.deserialize_model,
                    timings.build_ctx,
                    timings.viewbox,
                    viewbox_edge_curve_bounds,
                    detail.viewbox_edge_root_lookup,
                    detail.viewbox_edge_curve_offsets,
                    detail.viewbox_edge_curve_geom,
                    detail.viewbox_edge_curve_bbox_union,
                    detail.viewbox_edge_curve_geom_calls,
                    detail.viewbox_edge_curve_geom_skipped_bounds,
                    timings.render_svg,
                    timings.finalize_svg,
                    graph_timings.root_calls,
                    graph_timings.clusters,
                    graph_timings.edges_select,
                    graph_timings.edge_paths,
                    graph_timings.edge_labels,
                    graph_timings.dom_order,
                    graph_timings.nodes,
                    detail.node_style_compile,
                    detail.node_roughjs,
                    detail.node_roughjs_calls,
                    detail.node_label_html,
                    detail.node_label_html_calls,
                    graph_timings.nested_roots,
                );
            }
            document.finish()
        },
    )
}
