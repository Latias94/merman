//! Flowchart root renderer.

use super::super::defs::FlowchartMarkerEmissionPlan;
use super::super::*;
use super::edge_label::render_swimlane_edge_label_node;
use crate::svg::parity::timing::RenderTiming;

pub(in crate::svg::parity::flowchart) fn flowchart_elk_renders_empty_subgraph_as_cluster(
    ctx: &FlowchartRenderCtx<'_>,
) -> bool {
    ctx.uses_elk_adapter_dom
}

pub(in crate::svg::parity::flowchart) struct FlowchartRootRenderSession<
    'details,
    'cache,
    'marker,
    'data,
> {
    pub(in crate::svg::parity::flowchart) timing: RenderTiming,
    pub(in crate::svg::parity::flowchart) details: &'details mut FlowchartRenderDetails,
    pub(in crate::svg::parity::flowchart) edge_cache:
        &'cache mut FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    pub(in crate::svg::parity::flowchart) hierarchy_plan: &'marker FlowchartHierarchyPlan<'data>,
    pub(in crate::svg::parity::flowchart) marker_plan: &'marker FlowchartMarkerEmissionPlan,
}

struct FlowchartRootFrame<'data, 'plan> {
    cluster_id: Option<&'data str>,
    parent_origin_x: f64,
    parent_origin_y: f64,
    origin_x: f64,
    origin_y: f64,
    content_origin_y: f64,
    dom_order: &'plan [&'data str],
    next_dom_index: usize,
    initialized: bool,
    nested_start: Option<merman_core::runtime::OperationTimer>,
}

impl<'data, 'plan> FlowchartRootFrame<'data, 'plan> {
    fn new(
        cluster_id: Option<&'data str>,
        parent_origin_x: f64,
        parent_origin_y: f64,
        nested_start: Option<merman_core::runtime::OperationTimer>,
    ) -> Self {
        Self {
            cluster_id,
            parent_origin_x,
            parent_origin_y,
            origin_x: parent_origin_x,
            origin_y: parent_origin_y,
            content_origin_y: parent_origin_y,
            dom_order: &[],
            next_dom_index: 0,
            initialized: false,
            nested_start,
        }
    }
}

pub(in crate::svg::parity::flowchart) fn render_flowchart_root<'data, 'plan>(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &'data FlowchartRenderCtx<'data>,
    cluster_id: Option<&'data str>,
    parent_origin_x: f64,
    parent_origin_y: f64,
    session: &mut FlowchartRootRenderSession<'_, '_, 'plan, 'data>,
) -> crate::Result<()> {
    let mut stack = vec![FlowchartRootFrame::new(
        cluster_id,
        parent_origin_x,
        parent_origin_y,
        None,
    )];

    while let Some(frame) = stack.pop() {
        let mut frame = Some(frame);
        if !frame.as_ref().is_some_and(|frame| frame.initialized)
            && let Some(frame) = frame.as_mut()
        {
            initialize_flowchart_root_frame(out, ctx, session, frame)?;
        }

        let mut pushed_nested = false;
        while frame
            .as_ref()
            .is_some_and(|frame| frame.next_dom_index < frame.dom_order.len())
        {
            let id = {
                let Some(frame) = frame.as_mut() else {
                    break;
                };
                let id = frame.dom_order[frame.next_dom_index];
                frame.next_dom_index += 1;
                id
            };

            if let Some(edge) = ctx.swimlane_edge_label_edges_by_node_id.get(id).copied() {
                let Some(current) = frame.as_ref() else {
                    break;
                };
                render_swimlane_edge_label_node(
                    out,
                    ctx,
                    id,
                    edge,
                    current.origin_x,
                    current.content_origin_y,
                    &*session.edge_cache,
                )?;
                out.checkpoint()?;
                continue;
            }

            if ctx.subgraphs_by_id.contains_key(id) && ctx.subgraph_has_children(id) {
                // Non-recursive clusters render as cluster boxes (in `.clusters`) and do not emit a
                // node DOM element. Recursive clusters render as nested `.root` groups.
                if ctx.recursive_clusters.contains(id) {
                    let nested_start = session.timing.start();
                    if let Some(parent) = frame.take() {
                        let child = FlowchartRootFrame::new(
                            Some(id),
                            parent.origin_x,
                            parent.origin_y,
                            nested_start,
                        );
                        stack.push(parent);
                        stack.push(child);
                        pushed_nested = true;
                    }
                    break;
                }
                continue;
            }

            let node_start = session.timing.start();
            let Some(current) = frame.as_ref() else {
                break;
            };
            render_flowchart_node(
                out,
                ctx,
                id,
                current.origin_x,
                current.content_origin_y,
                session.timing,
                &mut *session.details,
            )?;
            out.checkpoint()?;
            if let Some(s) = node_start {
                session.details.nodes += s.elapsed();
            }
        }

        if pushed_nested {
            continue;
        }

        if let Some(frame) = frame.take() {
            out.push_str("</g></g>");
            out.checkpoint()?;
            if let Some(start) = frame.nested_start {
                session.details.nested_roots += start.elapsed();
            }
        }
    }
    Ok(())
}

fn edge_label_is_empty(
    ctx: &FlowchartRenderCtx<'_>,
    edge: super::super::render_input::FlowchartRenderEdgeRef<'_>,
) -> bool {
    let label_text = ctx
        .model
        .edge_label_for_render(edge.key.semantic_index(), edge.edge)
        .unwrap_or_default();
    crate::flowchart::flowchart_label_is_empty_for_render(label_text)
}

pub(in crate::svg::parity::flowchart) fn render_flowchart_elk_root_groups(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    session: &mut FlowchartRootRenderSession<'_, '_, '_, '_>,
) -> crate::Result<()> {
    session.details.root_calls += 1;

    render_flowchart_elk_subgraphs(out, ctx, session)?;
    render_flowchart_elk_nodes(out, ctx, session)?;

    let _g_edges_select = detail_guard(session.timing, &mut session.details.edges_select);
    let edges = session.hierarchy_plan.ordered_edges();
    drop(_g_edges_select);

    render_flowchart_elk_edge_paths(out, ctx, session, edges)?;
    render_flowchart_elk_edge_labels(out, ctx, session, edges)?;
    Ok(())
}

fn render_flowchart_elk_subgraphs(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    session: &mut FlowchartRootRenderSession<'_, '_, '_, '_>,
) -> crate::Result<()> {
    let _g_clusters = detail_guard(session.timing, &mut session.details.clusters);
    let clusters_to_draw = session.hierarchy_plan.root(None)?.clusters();

    if clusters_to_draw.is_empty() {
        out.push_str(r#"<g class="subgraphs"/>"#);
        return out.checkpoint();
    }

    out.push_str(r#"<g class="subgraphs">"#);
    for &cluster in clusters_to_draw {
        out.push_str(r#"<g class="subgraph">"#);
        render_flowchart_cluster(out, ctx, cluster, 0.0, 0.0)?;
        out.push_str("</g>");
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn render_flowchart_elk_nodes(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    session: &mut FlowchartRootRenderSession<'_, '_, '_, '_>,
) -> crate::Result<()> {
    out.push_str(r#"<g class="nodes">"#);

    let _g_dom_order = detail_guard(session.timing, &mut session.details.dom_order);
    let dom_order = session.hierarchy_plan.root(None)?.dom_order();
    drop(_g_dom_order);

    for &id in dom_order {
        if ctx.subgraphs_by_id.contains_key(id)
            && (ctx.subgraph_has_children(id)
                || flowchart_elk_renders_empty_subgraph_as_cluster(ctx))
        {
            continue;
        }

        let node_start = session.timing.start();
        render_flowchart_node(
            out,
            ctx,
            id,
            0.0,
            0.0,
            session.timing,
            &mut *session.details,
        )?;
        out.checkpoint()?;
        if let Some(s) = node_start {
            session.details.nodes += s.elapsed();
        }
    }

    out.push_str("</g>");
    out.checkpoint()
}

fn render_flowchart_elk_edge_paths(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    session: &mut FlowchartRootRenderSession<'_, '_, '_, '_>,
    edges: &[super::super::render_input::FlowchartRenderEdgeRef<'_>],
) -> crate::Result<()> {
    let _g_edge_paths = detail_guard(session.timing, &mut session.details.edge_paths);
    if edges.is_empty() {
        out.push_str(r#"<g class="edges edgePaths"/>"#);
        return out.checkpoint();
    }

    out.push_str(r#"<g class="edges edgePaths">"#);
    let mut scratch = FlowchartEdgeDataPointsScratch::default();
    for &e in edges {
        render_flowchart_edge_path(
            out,
            ctx,
            session.hierarchy_plan,
            session.marker_plan,
            e,
            0.0,
            0.0,
            &mut scratch,
            &mut *session.edge_cache,
        )?;
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn render_flowchart_elk_edge_labels(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    session: &mut FlowchartRootRenderSession<'_, '_, '_, '_>,
    edges: &[super::super::render_input::FlowchartRenderEdgeRef<'_>],
) -> crate::Result<()> {
    let _g_edge_labels = detail_guard(session.timing, &mut session.details.edge_labels);
    if edges.is_empty() {
        out.push_str(r#"<g class="edgeLabels"/>"#);
        return out.checkpoint();
    }

    out.push_str(r#"<g class="edgeLabels">"#);
    if !ctx.edge_html_labels {
        for &e in edges {
            if edge_label_is_empty(ctx, e) {
                out.push_str(r#"<g><rect class="background" style="stroke: none"/></g>"#);
                out.checkpoint()?;
            }
        }
    }
    for &e in edges {
        render_flowchart_edge_label(out, ctx, e, 0.0, 0.0, &*session.edge_cache)?;
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn initialize_flowchart_root_frame<'data, 'plan>(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &'data FlowchartRenderCtx<'data>,
    session: &mut FlowchartRootRenderSession<'_, '_, 'plan, 'data>,
    frame: &mut FlowchartRootFrame<'data, 'plan>,
) -> crate::Result<()> {
    session.details.root_calls += 1;
    let root_emission = session.hierarchy_plan.root(frame.cluster_id)?;

    let (origin_x, origin_y, transform_attr) = if let Some(cid) = frame.cluster_id {
        if let Some(off) = session.hierarchy_plan.root_offsets(cid) {
            let rel_x = off.origin_x - frame.parent_origin_x;
            let rel_y = off.abs_top_transform - frame.parent_origin_y;
            (
                off.origin_x,
                off.origin_y,
                format!(
                    r#" transform="translate({},{})""#,
                    fmt_display(rel_x),
                    fmt_display(rel_y)
                ),
            )
        } else {
            // Fallback: keep the group in the parent's coordinate space.
            (
                frame.parent_origin_x,
                frame.parent_origin_y,
                r#" transform="translate(0,0)""#.to_string(),
            )
        }
    } else {
        (0.0, 0.0, String::new())
    };

    frame.origin_x = origin_x;
    frame.origin_y = origin_y;
    frame.content_origin_y = origin_y;

    let _ = write!(out, r#"<g class="root"{}>"#, transform_attr);

    let _g_clusters = detail_guard(session.timing, &mut session.details.clusters);
    let clusters_to_draw = root_emission.clusters();
    if clusters_to_draw.is_empty() {
        out.push_str(r#"<g class="clusters"/>"#);
    } else {
        out.push_str(r#"<g class="clusters">"#);
        for &cluster in clusters_to_draw {
            render_flowchart_cluster(out, ctx, cluster, origin_x, frame.content_origin_y)?;
            out.checkpoint()?;
        }
        out.push_str("</g>");
    }
    drop(_g_clusters);

    let _g_edges_select = detail_guard(session.timing, &mut session.details.edges_select);
    let edges = root_emission.edges();
    drop(_g_edges_select);

    let _g_edge_paths = detail_guard(session.timing, &mut session.details.edge_paths);
    let edge_group_class = if ctx.swimlane_direction.is_some() {
        "edges edgePath"
    } else {
        "edgePaths"
    };
    if edges.is_empty() {
        let _ = write!(out, r#"<g class="{}"/>"#, edge_group_class);
    } else {
        let _ = write!(out, r#"<g class="{}">"#, edge_group_class);
        let mut scratch = FlowchartEdgeDataPointsScratch::default();
        for &e in edges {
            render_flowchart_edge_path(
                out,
                ctx,
                session.hierarchy_plan,
                session.marker_plan,
                e,
                origin_x,
                frame.content_origin_y,
                &mut scratch,
                &mut *session.edge_cache,
            )?;
            out.checkpoint()?;
        }
        out.push_str("</g>");
    }
    drop(_g_edge_paths);

    let _g_edge_labels = detail_guard(session.timing, &mut session.details.edge_labels);
    if ctx.swimlane_direction.is_some() || edges.is_empty() {
        out.push_str(r#"<g class="edgeLabels"/>"#);
    } else {
        out.push_str(r#"<g class="edgeLabels">"#);
        if !ctx.edge_html_labels {
            // Mermaid's `createText(..., useHtmlLabels=false)` always creates a background `<rect>`,
            // but for empty labels it returns the `<text>` element instead of the wrapper `<g>`.
            // The unused wrapper `<g>` (with the `background` rect) remains as a direct child
            // under `.edgeLabels`. Mirror this by emitting one rect-group per empty label.
            for &e in edges {
                if edge_label_is_empty(ctx, e) {
                    out.push_str(r#"<g><rect class="background" style="stroke: none"/></g>"#);
                    out.checkpoint()?;
                }
            }
            for &e in edges {
                render_flowchart_edge_label(
                    out,
                    ctx,
                    e,
                    origin_x,
                    frame.content_origin_y,
                    &*session.edge_cache,
                )?;
                out.checkpoint()?;
            }
        } else {
            // Mermaid emits HTML edge-label wrappers in graph edge order. Empty labels stay in
            // place as zero-sized foreignObjects instead of being partitioned ahead of labels.
            for &e in edges {
                render_flowchart_edge_label(
                    out,
                    ctx,
                    e,
                    origin_x,
                    frame.content_origin_y,
                    &*session.edge_cache,
                )?;
                out.checkpoint()?;
            }
        }
        out.push_str("</g>");
    }
    drop(_g_edge_labels);

    out.push_str(r#"<g class="nodes">"#);

    let _g_dom_order = detail_guard(session.timing, &mut session.details.dom_order);
    let dom_order = root_emission.dom_order();
    drop(_g_dom_order);

    frame.dom_order = dom_order;
    frame.initialized = true;
    out.checkpoint()
}
