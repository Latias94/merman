//! Flowchart cluster renderer.

use std::borrow::Cow;

use merman_model::presentation::PresentationConfig;
use merman_render_core::theme_svg::edge_label_background::{adaptive_text_ink, literal_fill};
use merman_theme_contract::v2::Paint;

use super::super::*;

use crate::cluster_shell::{
    ClusterHandDrawn, FlowchartClusterShape, FlowchartClusterSvg, FlowchartClusterTitle,
};

use crate::graph::FlowchartClusterPass;
use crate::style::compiled::FlowchartCompiledStyles;
use crate::svg::cluster_geometry::painted_cluster_frame;

pub(in crate::svg) fn resolve_canvas_color(
    config: &PresentationConfig,
    services: &FlowchartSvgServices<'_>,
) -> Option<Color> {
    if let Some(color) = services.canvas_background {
        return Some(color);
    }
    if let Some(theme) = services.native_family_theme {
        return match &theme.roles.canvas {
            Paint::Solid(color) => Some(*color),
            Paint::LinearGradient(_) => None,
        };
    }
    config.get_str("themeVariables.background").map_or_else(
        || Some(FlowchartClusterSvg::compat_theme_canvas_fallback(config)),
        literal_fill,
    )
}

pub(in crate::svg) fn lane_color_slot(ctx: &FlowchartRenderCtx<'_>, id: &str) -> Option<usize> {
    crate::cluster_shell::swimlane::swimlane_color_slot(
        ctx.model,
        &ctx.palette,
        ctx.diagram_type,
        id,
        !ctx.subgraphs_by_id.contains_key(id),
    )
}

pub(in crate::svg) fn swimlane_label_ink(
    ctx: &FlowchartRenderCtx<'_>,
    id: &str,
    style: &FlowchartCompiledStyles,
    default_fill: Option<Color>,
    default_ink: &str,
    text_style: &TextStyle,
) -> Result<Option<Cow<'static, str>>> {
    if !ctx.layout.route_contract.is_canonical_axis() {
        return Ok(None);
    }
    if style.has_source_label_foreground() {
        let ink = style
            .effective_edge_label_text_style_with_provenance(text_style)
            .terminal_foreground;
        return Ok(ink.map(|ink| Cow::Owned(ink.value().to_owned())));
    }
    let fill = match style.fill.as_deref() {
        Some(fill) => literal_fill(fill),
        None => default_fill,
    };
    let Some((fill, default_ink)) = fill.zip(literal_fill(default_ink)) else {
        return Ok(None);
    };
    let mut backdrop = fill.composite(Color::TRANSPARENT, style.fill_opacity);
    let mut parent = ctx.parent.get(id).copied();
    while backdrop.channels()[3] != u8::MAX {
        let Some(id) = parent else { break };
        ctx.work_meter.charge_emit_work(1)?;
        let terminal = ctx.prepared_nodes.cluster(id)?;
        let fill = match terminal.compiled.fill.as_deref() {
            Some(fill) => literal_fill(fill),
            None => lane_color_slot(ctx, id)
                .and_then(|slot| ctx.palette.background_color(slot))
                .and_then(literal_fill)
                .or_else(|| ctx.native.map(|native| native.container))
                .or_else(|| literal_fill(&terminal.fill)),
        };
        let Some(fill) = fill else { return Ok(None) };
        let layer = fill.composite(Color::TRANSPARENT, terminal.compiled.fill_opacity);
        backdrop = backdrop.composite(layer, 1.0);
        parent = ctx.parent.get(id).copied();
    }
    let backdrop = ctx
        .canvas
        .map_or(backdrop, |canvas| backdrop.composite(canvas, 1.0));
    Ok(adaptive_text_ink(backdrop, default_ink).map(Cow::Borrowed))
}

pub fn advance_rough_geometry(
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
) -> merman_render_core::Result<()> {
    ctx.work_meter
        .checkpoint(merman_model::OperationPhase::Layout)
        .map_err(merman_render_core::Error::from)?;
    if ctx.compatibility.look.as_str() != "handDrawn"
        || ctx.native.is_some()
        || ctx
            .swimlane_lanes_by_id
            .get(cluster.id.as_str())
            .is_some_and(|lane| lane.parent_id.is_none())
        || (!ctx.subgraph_has_children(cluster.id.as_str())
            && !super::flowchart_renders_empty_subgraph_as_cluster(ctx))
    {
        return Ok(());
    }
    let terminal = ctx.prepared_nodes.cluster(cluster.id.as_str())?;
    let frame = painted_cluster_frame(ctx, cluster).in_root(
        kurbo::Vec2::new(ctx.tx, ctx.ty),
        kurbo::Point::new(origin_x, origin_y),
    );
    ClusterHandDrawn::for_compiled_styles(&terminal.compiled, ctx.work_meter, &ctx.hand_drawn_seed)
        .advance_cluster(frame, ctx.diagram_type == "agentflow");
    ctx.work_meter
        .checkpoint(merman_model::OperationPhase::Layout)
        .map_err(merman_render_core::Error::from)
}

#[cfg(feature = "layout-shoal")]
pub(in crate::svg) fn cluster_base_fill<'a>(
    ctx: &'a FlowchartRenderCtx<'_>,
    terminal: &'a crate::node_inventory::FlowchartPreparedCluster,
    id: &str,
) -> &'a str {
    if ctx.diagram_type == "agentflow" {
        return FlowchartClusterShape::agentflow_fill(
            &terminal.fill,
            &ctx.palette,
            super::super::agentflow::container_color_slot(ctx, id),
        );
    }
    &terminal.fill
}

#[cfg(feature = "test-support")]
pub fn render_flowchart_cluster(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
) -> merman_render_core::Result<()> {
    render_flowchart_cluster_pass(
        out,
        ctx,
        cluster,
        origin_x,
        origin_y,
        FlowchartClusterPass::Complete,
    )
}

pub(in crate::svg) fn render_flowchart_cluster_pass(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
    pass: FlowchartClusterPass,
) -> merman_render_core::Result<()> {
    if let Some(lane) = ctx.swimlane_lanes_by_id.get(cluster.id.as_str())
        && super::super::swimlane::is_lane_or_pool_cluster(ctx, cluster.id.as_str())
    {
        return super::super::swimlane::render_swimlane_cluster(
            out, ctx, cluster, lane, origin_x, origin_y, pass,
        );
    }

    let Some(sg) = ctx.subgraphs_by_id.get(cluster.id.as_str()) else {
        return Err(merman_render_core::Error::InvalidModel {
            message: format!(
                "Flowchart layout cluster `{}` has no semantic subgraph owner",
                cluster.id
            ),
        });
    };
    let Some(subgraph_index) = ctx.subgraph_indices_by_id.get(cluster.id.as_str()).copied() else {
        return Err(merman_render_core::Error::InvalidModel {
            message: format!(
                "missing semantic Flowchart subgraph index for cluster `{}`",
                cluster.id
            ),
        });
    };
    if !ctx.subgraph_has_children(cluster.id.as_str())
        && !super::flowchart_renders_empty_subgraph_as_cluster(ctx)
    {
        return Ok(());
    }

    let terminal = ctx.prepared_nodes.cluster(cluster.id.as_str())?;
    let classes = ctx.model.effective_subgraph_css(subgraph_index, sg).0;

    let elk_geometry = ctx.elk_cluster_geometry.get(cluster.id.as_str());
    let translation = kurbo::Vec2::new(ctx.tx, ctx.ty);
    let group_origin = kurbo::Point::new(origin_x, origin_y);
    let frame = painted_cluster_frame(ctx, cluster).in_root(translation, group_origin);
    let Some(cluster_dom_id) = ctx.document_ids().cluster(cluster.id.as_str()) else {
        return Err(merman_render_core::Error::InvalidModel {
            message: format!(
                "missing prepared Flowchart DOM id for cluster `{}`",
                cluster.id
            ),
        });
    };
    ctx.checkpoint_emit()?;

    let label_type = sg.label_type.as_deref().unwrap_or("text");
    let render_title = ctx.model.subgraph_title_for_render(subgraph_index, sg);
    let title_owner = ctx
        .svg_label_sidecar
        .and_then(|sidecar| sidecar.subgraph_title_owner(cluster.id.as_str()));
    let title_text_style = terminal.title_text_style.as_ref();
    let markdown_wrap_width =
        FlowchartClusterTitle::markdown_wrap_width(cluster, ctx.uses_elk_adapter_dom);
    let painted_markdown = elk_geometry.map(|geometry| geometry.label).or_else(|| {
        (ctx.uses_elk_adapter_dom && label_type == "markdown").then(|| {
            crate::svg::label::flowchart_label_metrics_for_layout(
                merman_render_core::text::create_text_html::FlowchartLabelMetricsRequest {
                    measurer: ctx.measurer,
                    raw_label: render_title,
                    label_type,
                    style: title_text_style,
                    max_width_px: Some(markdown_wrap_width),
                    wrap_mode: ctx.edge_wrap_mode,
                    config: ctx.config,
                    math_renderer: ctx.math_renderer,
                },
            )
        })
    });
    let struck = ctx
        .layout
        .route_contract
        .struck_group_titles()
        .is_some_and(|titles| titles.contains(cluster.id.as_str()));
    let svg = FlowchartClusterSvg {
        semantic_id: &cluster.id,
        document_id: cluster_dom_id,
        classes,
        model: ctx.model,
        diagram_type: ctx.diagram_type,
        compatibility: ctx.compatibility,
        native: ctx.native,
        palette: &ctx.palette,
        canonical_axis: ctx.layout.route_contract.is_canonical_axis(),
        struck,
        frame,
        prepared_nodes: ctx.prepared_nodes,
        parent: &ctx.parent,
    };
    let shape = FlowchartClusterShape {
        terminal,
        work_meter: ctx.work_meter,
        randomness: &ctx.hand_drawn_seed,
    };
    let title = FlowchartClusterTitle {
        layout: cluster,
        translation,
        group_origin,
        canonical_group_title: ctx.layout.route_contract.group_parents().is_some(),
        painted_markdown,
        markdown_wrap_width,
        label_type,
        source: render_title,
        html_labels: ctx.edge_html_labels,
        svg_font_fallback: ctx.svg_label_font_fallback(),
        uses_elk_adapter_dom: ctx.uses_elk_adapter_dom,
        owner: title_owner,
        sidecar: ctx.svg_label_sidecar,
        measurer: ctx.measurer,
        config: ctx.config,
        math_renderer: ctx.math_renderer,
    };
    let solid_backing =
        svg.canonical_axis && (ctx.diagram_type == "agentflow" || ctx.swimlane_direction.is_none());
    svg.write(out, pass, shape, title, ctx.canvas, solid_backing)
}
