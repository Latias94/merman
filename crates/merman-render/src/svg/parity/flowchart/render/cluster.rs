//! Flowchart cluster renderer.

use super::super::*;
use crate::flowchart::FLOWCHART_FIXED_LABEL_WRAP_WIDTH;
use crate::svg::parity::flowchart::cluster_geometry::painted_cluster_frame;

use merman_render_flow::cluster_shell::{
    ClusterHandDrawn, ClusterShellForm, ClusterShellPaint, FlowchartClusterShape,
    FlowchartClusterSvg, FlowchartClusterTitle,
};

use merman_render_flow::graph::FlowchartClusterPass;

pub(in crate::svg::parity::flowchart) fn advance_rough_geometry(
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
) -> crate::Result<()> {
    ctx.work_meter
        .checkpoint(merman_model::OperationPhase::Layout)
        .map_err(crate::Error::from)?;
    if ctx.compatibility.look.as_str() != "handDrawn"
        || ctx
            .swimlane_lanes_by_id
            .get(cluster.id.as_str())
            .is_some_and(|lane| lane.parent_id.is_none())
        || (!ctx.subgraph_has_children(cluster.id.as_str())
            && !super::flowchart_elk_renders_empty_subgraph_as_cluster(ctx))
    {
        return Ok(());
    }
    let terminal = ctx.prepared_nodes.cluster(cluster.id.as_str())?;
    let frame = painted_cluster_frame(ctx, cluster).in_root(
        kurbo::Vec2::new(ctx.tx, ctx.ty),
        kurbo::Point::new(origin_x, origin_y),
    );
    cluster_hand_drawn(ctx, &terminal.compiled)
        .advance_cluster(frame, ctx.diagram_type == "agentflow");
    ctx.work_meter
        .checkpoint(merman_model::OperationPhase::Layout)
        .map_err(crate::Error::from)
}

fn cluster_hand_drawn<'a>(
    ctx: &'a FlowchartRenderCtx<'_>,
    compiled: &'a FlowchartCompiledStyles,
) -> ClusterHandDrawn<'a> {
    ClusterHandDrawn {
        stroke_width: compiled.stroke_width.as_deref(),
        stroke_dasharray: compiled.stroke_dasharray.as_deref(),
        work_meter: ctx.work_meter,
        randomness: &ctx.hand_drawn_seed,
    }
}

pub(in crate::svg::parity::flowchart) fn cluster_base_fill<'a>(
    ctx: &'a FlowchartRenderCtx<'_>,
    terminal: &'a super::super::node_inventory::FlowchartPreparedCluster,
    id: &str,
) -> &'a str {
    if ctx.diagram_type == "agentflow"
        && let Some(slot) = super::super::agentflow::container_color_slot(ctx, id)
        && let Some(background) = ctx.palette.background_color(slot)
    {
        return background;
    }
    &terminal.fill
}

pub(in crate::svg::parity::flowchart) fn compat_theme_canvas_fallback(
    ctx: &FlowchartRenderCtx<'_>,
) -> crate::theme::Color {
    match ctx.config.get_str("theme") {
        Some("dark" | "redux-dark" | "redux-dark-color" | "neo-dark") => {
            crate::theme::Color::rgb(0x1F, 0x20, 0x20)
        }
        _ => crate::theme::Color::rgb(255, 255, 255),
    }
}

fn prepare_flowchart_cluster_shape<'a>(
    ctx: &'a FlowchartRenderCtx<'_>,
    terminal: &'a super::super::node_inventory::FlowchartPreparedCluster,
    id: &str,
    frame: super::super::cluster_geometry::PaintedClusterFrame,
) -> FlowchartClusterShape<'a> {
    let compiled = &terminal.compiled;
    let fill = cluster_base_fill(ctx, terminal, id);
    let canonical = ctx.layout.route_contract.is_canonical_axis();
    let form = if let Some(native) = ctx.native {
        ClusterShellForm::Native {
            radius: super::super::cluster_geometry::cluster_container_radius(
                ctx,
                frame.size.width,
                frame.size.height,
            ),
            stroke_width: native.form.stroke_width,
        }
    } else if ctx.diagram_type == "agentflow" {
        ClusterShellForm::Agentflow {
            stroke: &ctx.compatibility.agentflow_container_stroke,
            canonical_fill: canonical.then(|| {
                (
                    compiled.fill.as_deref().unwrap_or(fill),
                    compiled.fill_opacity,
                )
            }),
        }
    } else {
        ClusterShellForm::Flowchart {
            inline_fill: canonical && ctx.palette.color_count() > 0,
        }
    };
    FlowchartClusterShape {
        frame,
        paint: ClusterShellPaint {
            style: &terminal.rect_style,
            fill,
            stroke: &terminal.stroke,
        },
        form,
        rough: ctx
            .compatibility
            .look
            .is_hand_drawn()
            .then(|| cluster_hand_drawn(ctx, compiled)),
    }
}

fn title_halo_color(
    ctx: &FlowchartRenderCtx<'_>,
    id: &str,
) -> crate::Result<Option<crate::theme::Color>> {
    use crate::theme::{Color, edge_label_bg::literal_fill};

    let canvas = ctx.native.map(|native| native.canvas).or_else(|| {
        ctx.config
            .get_str("themeVariables.background")
            .and_then(literal_fill)
    });
    let mut surface = Color::TRANSPARENT;
    let mut current = Some(id);
    while let Some(id) = current {
        ctx.work_meter.charge(1)?;
        let terminal = ctx.prepared_nodes.cluster(id)?;
        let fill = terminal
            .compiled
            .fill
            .as_deref()
            .and_then(literal_fill)
            .or_else(|| literal_fill(cluster_base_fill(ctx, terminal, id)))
            .unwrap_or(Color::TRANSPARENT);
        let layer = fill.composite(Color::TRANSPARENT, terminal.compiled.fill_opacity);
        surface = surface.composite(layer, 1.0);
        if surface.channels()[3] == u8::MAX {
            return Ok(Some(surface));
        }
        current = ctx.parent.get(id).copied();
    }
    match canvas {
        Some(canvas) if canvas.channels()[3] == u8::MAX => Ok(Some(surface.composite(canvas, 1.0))),
        Some(_) => Ok(None),
        None => Ok(Some(
            surface.composite(compat_theme_canvas_fallback(ctx), 1.0),
        )),
    }
}

#[cfg(all(test, feature = "diagram-flowchart"))]
pub(in crate::svg::parity) fn render_flowchart_cluster(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
) -> crate::Result<()> {
    render_flowchart_cluster_pass(
        out,
        ctx,
        cluster,
        origin_x,
        origin_y,
        FlowchartClusterPass::Complete,
    )
}

pub(in crate::svg::parity::flowchart) fn render_flowchart_cluster_pass(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
    pass: FlowchartClusterPass,
) -> crate::Result<()> {
    if let Some(lane) = ctx.swimlane_lanes_by_id.get(cluster.id.as_str())
        && super::super::swimlane::is_lane_or_pool_cluster(ctx, cluster.id.as_str())
    {
        if pass == FlowchartClusterPass::Label {
            return Ok(());
        }
        super::super::swimlane::render_swimlane_cluster(
            out, ctx, cluster, lane, origin_x, origin_y,
        )?;
        return Ok(());
    }

    let Some(sg) = ctx.subgraphs_by_id.get(cluster.id.as_str()) else {
        return Err(crate::Error::InvalidModel {
            message: format!(
                "Flowchart layout cluster `{}` has no semantic subgraph owner",
                cluster.id
            ),
        });
    };
    let Some(subgraph_index) = ctx.subgraph_indices_by_id.get(cluster.id.as_str()).copied() else {
        return Err(crate::Error::InvalidModel {
            message: format!(
                "missing semantic Flowchart subgraph index for cluster `{}`",
                cluster.id
            ),
        });
    };
    if !ctx.subgraph_has_children(cluster.id.as_str())
        && !super::flowchart_elk_renders_empty_subgraph_as_cluster(ctx)
    {
        return Ok(());
    }

    let terminal = ctx.prepared_nodes.cluster(cluster.id.as_str())?;
    let compiled_styles = &terminal.compiled;
    let label_style = compiled_styles.label_style.trim();
    let classes = ctx.model.effective_subgraph_css(subgraph_index, sg).0;

    let elk_geometry = ctx.elk_cluster_geometry.get(cluster.id.as_str());
    let frame = painted_cluster_frame(ctx, cluster).in_root(
        kurbo::Vec2::new(ctx.tx, ctx.ty),
        kurbo::Point::new(origin_x, origin_y),
    );
    let Some(cluster_dom_id) = ctx.document_ids().cluster(cluster.id.as_str()) else {
        return Err(crate::Error::InvalidModel {
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
    // ELK paints Markdown after the final frame is known. clusters.js passes node.width to
    // createText, so these paint metrics differ from the wrapped pre-layout placeholder.
    let markdown_wrap_width = if ctx.uses_elk_adapter_dom {
        cluster.width.max(1.0)
    } else {
        FLOWCHART_FIXED_LABEL_WRAP_WIDTH
    };
    let painted_markdown = elk_geometry.map(|geometry| geometry.label).or_else(|| {
        (ctx.uses_elk_adapter_dom && label_type == "markdown").then(|| {
            crate::flowchart::flowchart_label_metrics_for_layout(
                crate::flowchart::FlowchartLabelMetricsRequest {
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
    let label_w = painted_markdown
        .map_or(cluster.title_label.width, |metrics| metrics.width)
        .max(0.0);
    let label_h = painted_markdown
        .map_or(cluster.title_label.height, |metrics| metrics.height)
        .max(0.0);
    let (label_left, label_top) = if ctx.layout.route_contract.group_parents().is_some() {
        (
            cluster.title_label.x + ctx.tx - origin_x - label_w / 2.0,
            cluster.title_label.y + ctx.ty - origin_y - label_h / 2.0,
        )
    } else {
        (
            cluster.x + ctx.tx - origin_x - label_w / 2.0,
            frame.origin.y + cluster.title_margin_top.max(0.0),
        )
    };

    let struck = ctx
        .layout
        .route_contract
        .struck_group_titles()
        .is_some_and(|titles| titles.contains(cluster.id.as_str()));
    let svg = FlowchartClusterSvg {
        semantic_id: &cluster.id,
        document_id: cluster_dom_id,
        classes,
        agentflow: ctx.diagram_type == "agentflow",
        look: ctx.compatibility.look.as_str(),
        color_slot: super::super::agentflow::container_color_slot(ctx, &cluster.id),
        struck,
    };
    if pass == FlowchartClusterPass::Background {
        return svg.write_background(
            out,
            prepare_flowchart_cluster_shape(ctx, terminal, &cluster.id, frame),
        );
    }

    let halo = struck
        .then(|| title_halo_color(ctx, &cluster.id))
        .transpose()?
        .flatten();
    let overlay = struck && pass == FlowchartClusterPass::Label;
    let shape = (pass == FlowchartClusterPass::Complete)
        .then(|| prepare_flowchart_cluster_shape(ctx, terminal, &cluster.id, frame));
    let title = FlowchartClusterTitle {
        position: kurbo::Point::new(label_left, label_top),
        size: kurbo::Size::new(label_w, label_h),
        markdown_wrap_width,
        measured_title_width: ctx
            .layout
            .route_contract
            .group_parents()
            .map(|_| cluster.title_label.width),
        label_type,
        source: render_title,
        label_style,
        text_style: title_text_style,
        html_labels: ctx.edge_html_labels,
        uses_elk_adapter_dom: ctx.uses_elk_adapter_dom,
        owner: title_owner,
        sidecar: ctx.svg_label_sidecar,
        measurer: ctx.measurer,
        config: ctx.config,
        math_renderer: ctx.math_renderer,
    };
    svg.write_foreground(out, shape, title, halo, overlay);
    Ok(())
}
