//! NativeCompact projection of an admitted, ungrouped Dagre Flowchart.

use merman_markup::theme_color::ThemeColor;
use merman_model::{
    OperationPhase,
    families::flowchart::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility},
    presentation::PresentationMetadata,
};
use merman_render_core::{
    paint_scene::{
        self, PaintScene, SceneBox, SceneColor, SceneFontFamily, SceneGroup, SceneItem,
        SceneMarkerRef, SceneMetadata, ScenePaint, ScenePath, ScenePathCommand, SceneRect,
        SceneRole, SceneStroke, SceneText, SceneTextAnchor, SceneTextBaseline, SceneTextStyle,
        SceneViewport,
    },
    text::{FontStyle, parse_css_font_stack},
};
use merman_theme_contract::v2::{Elevation, MarkerStyle, Paint, Texture};

use super::{
    FlowchartEdgeDataPointsScratch, FlowchartHierarchyPlan, FlowchartMarkerBase,
    FlowchartRenderCtx,
    defs::compact_classic_point_end,
    flowchart_edge_marker_end_base, flowchart_edge_marker_start_base,
    paint::{prepare_dagre_linear_edge_route, with_flowchart_paint_context},
    render_input::{FlowchartRenderEdge, FlowchartRenderEdgeRef},
    svg_emit::{FlowchartSvgModelRequest, flowchart_request},
    viewbox::{FlowchartRenderedBoundsRequest, prepare_flowchart_rendered_bounds},
};
use crate::{
    DiagramFamilyId, Error, Result,
    family::FlowchartFamilyArtifact,
    flowchart::{FlowchartShape, FlowchartSvgLabelRenderPlan, flowchart_node_svg_width_mode},
    graph_layout::RouteContract,
    model::FlowchartLayout,
    resources::OperationWorkMeter,
    svg::{
        SvgOutputProfile,
        parity::{SvgExecution, root_svg},
    },
};

const MAX_TOKEN_BYTES: usize = 8 * 1024;

fn unsupported(meter: &OperationWorkMeter, reason: &'static str) -> Error {
    meter.checkpoint(OperationPhase::Emit).err().map_or(
        Error::UnsupportedSvgOutputProfile {
            profile: SvgOutputProfile::NativeCompact,
            reason,
        },
        Into::into,
    )
}

fn invalid(message: &'static str) -> Error {
    Error::InvalidModel {
        message: message.to_owned(),
    }
}

fn admit_token(text: &str, meter: &OperationWorkMeter) -> Result<()> {
    if text.len() > MAX_TOKEN_BYTES {
        return Err(unsupported(meter, "flowchart-token-bytes"));
    }
    meter.charge_emit_work(1 + text.len().div_ceil(32))?;
    Ok(())
}

fn resolved_color(text: &str, meter: &OperationWorkMeter) -> Result<SceneColor> {
    admit_token(text, meter)?;
    let color = ThemeColor::parse(text.trim())
        .map_err(|_| unsupported(meter, "flowchart-paint"))?
        .rgba_channels();
    meter.checkpoint(OperationPhase::Emit)?;
    Ok(color)
}

fn css_width(text: &str, meter: &OperationWorkMeter) -> Result<f64> {
    admit_token(text, meter)?;
    text.parse::<f64>()
        .ok()
        .filter(|width| width.is_finite() && *width > 0.0)
        .ok_or_else(|| unsupported(meter, "flowchart-stroke-width"))
}

fn solid_color(color: crate::theme::Color) -> SceneColor {
    let [red, green, blue, alpha] = color.channels().map(f64::from);
    SceneColor {
        red,
        green,
        blue,
        alpha: alpha / 255.0,
    }
}

fn admit_canvas(options: &SvgExecution<'_>) -> Result<Option<SceneColor>> {
    let meter = options.work_meter();
    let session = options.family.session();
    if let Some(theme) = session.native_theme()
        && theme
            .resolved()
            .canvas()
            .textures
            .iter()
            .any(|texture| !matches!(texture, Texture::None))
    {
        return Err(unsupported(meter, "flowchart-canvas-texture"));
    }
    let native = options.native_family_theme();
    if let Some(native) = native
        && (!matches!(native.elevation, Elevation::Flat)
            || native.form.marker_style != MarkerStyle::Classic
            || [
                &native.roles.surface_node,
                &native.roles.stroke_border,
                &native.roles.stroke_connector,
            ]
            .into_iter()
            .any(|paint| !matches!(paint, Paint::Solid(_))))
    {
        return Err(unsupported(meter, "flowchart-native-material"));
    }
    let canvas = match (session.canvas_background(), native) {
        (Some(color), _) => Some(solid_color(color)),
        (None, Some(native)) => match &native.roles.canvas {
            Paint::Solid(color) => Some(solid_color(*color)),
            _ => return Err(unsupported(meter, "flowchart-canvas-paint")),
        },
        (None, None) => None,
    };
    Ok(canvas.filter(|color| color.alpha != 0.0))
}

fn admit_request(request: FlowchartSvgModelRequest<'_>, options: &SvgExecution<'_>) -> Result<()> {
    let meter = options.work_meter();
    meter.checkpoint(OperationPhase::Emit)?;
    let debug = options.debug;
    if !debug.include_edges
        || !debug.include_nodes
        || !debug.include_clusters
        || debug.include_cluster_debug_markers
        || debug.include_edge_id_labels
        || debug.include_timing_diagnostics
        || debug.flowchart_edge_trace.is_some()
    {
        return Err(unsupported(meter, "flowchart-diagnostics"));
    }
    let model = request.model;
    let config = request.render_config;
    if !matches!(request.layout.route_contract, RouteContract::DagreParity)
        || request.prepared_theme.compatibility.look.as_str() != "classic"
        || config.default_edge_interpolate != "linear"
        || config.node_html_labels
        || config.edge_html_labels
    {
        return Err(unsupported(
            meter,
            "flowchart-requires-classic-dagre-linear-svg-labels",
        ));
    }
    if request.swimlane_layout.is_some()
        || !model.subgraphs.is_empty()
        || !request.layout.clusters.is_empty()
        || model.nodes.is_empty()
        || request.diagram_title.is_some_and(|title| !title.is_empty())
        || model.acc_title.is_some()
        || model.acc_descr.is_some()
    {
        return Err(unsupported(meter, "flowchart-groups-title-or-empty"));
    }
    if !model.class_defs.is_empty()
        || !model.tooltips.is_empty()
        || !config.default_edge_style.is_empty()
    {
        return Err(unsupported(meter, "flowchart-styles-or-interaction"));
    }
    if let Some(css) = request.effective_config.get_str("themeCSS") {
        admit_token(css, meter)?;
        let css = css.trim();
        if !css.is_empty() && css != "{ /* ERROR: Unbalanced CSS */ }" {
            if !options.family.session().allows_trusted_theme_css() {
                return Err(Error::UntrustedThemeCss);
            }
            return Err(unsupported(meter, "flowchart-authored-css"));
        }
    }
    Ok(())
}

fn admit_edge(
    ctx: &FlowchartRenderCtx<'_>,
    render_edge: FlowchartRenderEdgeRef<'_>,
) -> Result<bool> {
    let meter = ctx.work_meter;
    meter.checkpoint(OperationPhase::Emit)?;
    let edge = render_edge.edge;
    let end = flowchart_edge_marker_end_base(edge);
    let valid_end = matches!(
        (edge.end_marker, end),
        (FlowEdgeMarker::None, None) | (FlowEdgeMarker::Point, Some(FlowchartMarkerBase::PointEnd))
    );
    if !valid_end
        || flowchart_edge_marker_start_base(edge).is_some()
        || edge.start_marker != FlowEdgeMarker::None
        || !matches!(
            edge.edge_type.as_deref(),
            Some("arrow_point" | "arrow_open")
        )
        || edge.stroke_kind != FlowEdgeStroke::Normal
        || edge.visibility != FlowEdgeVisibility::Visible
        || edge.from == edge.to
        || render_edge.key.self_loop_segment_order().is_some()
        || edge.is_user_defined_id
        || !edge.style.is_empty()
        || !edge.classes.is_empty()
        || edge.animate.is_some()
        || edge.animation.is_some()
        || edge.step.is_some()
        || edge
            .interpolate
            .as_deref()
            .is_some_and(|curve| curve != "linear")
        || ctx
            .model
            .edge_label_for_occurrence(render_edge.key, edge)
            .is_some_and(|label| !label.is_empty())
    {
        return Err(unsupported(meter, "flowchart-edge-shape-style-or-label"));
    }
    let style = ctx.edge_style_plan.edge_for(render_edge.key)?;
    let layout = ctx
        .layout_edges_by_key
        .get(&render_edge.key)
        .and_then(|edge| edge.materialized())
        .ok_or_else(|| invalid("missing Flowchart layout edge"))?;
    if style.edge_marker_color(false).is_some()
        || ctx
            .edge_style_plan
            .animation_for(render_edge.key)?
            .is_active()
        || layout.start_label_left.is_some()
        || layout.start_label_right.is_some()
        || layout.end_label_left.is_some()
        || layout.end_label_right.is_some()
        || layout.start_marker.is_some()
        || layout.end_marker.is_some()
        || layout.stroke_dasharray.is_some()
    {
        return Err(unsupported(meter, "flowchart-edge-style"));
    }
    Ok(end.is_some())
}

fn plain_node_items<'a>(
    ctx: &'a FlowchartRenderCtx<'_>,
    id: &str,
    font: SceneTextStyle<'a>,
    width: f64,
) -> Result<[SceneItem<'a>; 2]> {
    let meter = ctx.work_meter;
    meter.checkpoint(OperationPhase::Emit)?;
    let node = ctx
        .nodes_by_id
        .get(id)
        .ok_or_else(|| invalid("missing Flowchart node"))?;
    let shape = node.layout_shape.as_deref().unwrap_or("squareRect");
    if !matches!(
        FlowchartShape::resolve(shape),
        Some(FlowchartShape::Process)
    ) || node.is_subgraph_anchor()
        || node.link.is_some()
        || node.link_target.is_some()
        || node.have_callback
        || node.icon.is_some()
        || node.img.is_some()
        || !node.classes.is_empty()
        || !node.styles.is_empty()
        || node
            .label_type
            .as_deref()
            .is_some_and(|kind| kind != "text")
    {
        return Err(unsupported(
            meter,
            "flowchart-node-shape-style-or-interaction",
        ));
    }
    let label = ctx.model.node_label_for_render(node).unwrap_or(&node.id);
    admit_token(label, meter)?;
    if label.is_empty()
        || label.chars().any(|ch| {
            (ch.is_whitespace() && ch != ' ') || ch.is_control() || matches!(ch, '&' | '<' | '>')
        })
        || crate::math::contains_delimited_math(label)
    {
        return Err(unsupported(meter, "flowchart-literal-label"));
    }
    let layout = ctx
        .layout_nodes_by_id
        .get(id)
        .ok_or_else(|| invalid("missing Flowchart layout node"))?;
    let terminal = ctx
        .prepared_nodes
        .node(id)
        .ok_or_else(|| invalid("missing Flowchart terminal node style"))?;
    let geometry = ctx.rectangle_geometry(layout);
    let rect = geometry.shape;
    let height = layout
        .label_height
        .filter(|height| height.is_finite() && *height > 0.0)
        .ok_or_else(|| unsupported(meter, "flowchart-label-metrics"))?;
    let source_style = terminal.source_text_style.as_ref();
    let owner = ctx
        .svg_label_sidecar
        .and_then(|sidecar| sidecar.node_owner(id, false));
    let plan = FlowchartSvgLabelRenderPlan::new(
        ctx.svg_label_sidecar,
        owner,
        label,
        ctx.measurer,
        source_style,
        Some(crate::flowchart::bpmn::wrapping_width(
            Some(shape),
            ctx.wrapping_width,
        )),
        true,
        flowchart_node_svg_width_mode(label, "text", ctx.node_wrap_mode, shape),
    );
    let wrapped = plan.wrapped_lines();
    meter.checkpoint(OperationPhase::Emit)?;
    let [line] = wrapped.as_ref() else {
        return Err(unsupported(meter, "flowchart-single-row-label"));
    };
    // The parity writer emits one space between the measured words. Borrow the
    // authored label if its sole row has exactly that representation.
    if !line.iter().map(String::as_str).eq(label.split(' ')) {
        return Err(unsupported(meter, "flowchart-literal-label"));
    }
    let coordinates = [
        layout.x,
        layout.y,
        rect.x0,
        rect.y0,
        rect.width(),
        rect.height(),
        geometry.group.x0,
        geometry.group.y0,
        geometry.group.x1,
        geometry.group.y1,
    ];
    if layout.is_cluster
        || !coordinates.into_iter().all(f64::is_finite)
        || rect.width() <= 0.0
        || rect.height() <= 0.0
    {
        return Err(unsupported(meter, "flowchart-rectangle-geometry"));
    }
    let x = layout.x + ctx.tx;
    let y = layout.y + ctx.ty;
    let radius = terminal
        .selection
        .native_radius
        .map(f64::from)
        .or(terminal.selection.configured_radius)
        .unwrap_or_default();
    if !radius.is_finite() || radius < 0.0 {
        return Err(unsupported(meter, "flowchart-rectangle-radius"));
    }
    let fill = resolved_color(terminal.fill_color(ctx.compatibility), meter)?;
    let border = resolved_color(terminal.stroke_color(ctx.compatibility), meter)?;
    let text_fill = match terminal.selection.native_label_color.as_deref() {
        Some(color) => resolved_color(color, meter)?,
        None => font.fill,
    };
    Ok([
        SceneItem::Rect(SceneRect {
            bounds: SceneBox {
                x: x + rect.x0,
                y: y + rect.y0,
                width: rect.width(),
                height: rect.height(),
            },
            radius,
            paint: ScenePaint {
                fill: Some(fill),
                stroke: Some(SceneStroke {
                    color: border,
                    width,
                    dash: None,
                }),
            },
        }),
        SceneItem::Text(SceneText {
            text: label,
            position: [x, y - height / 2.0],
            dy: font.size,
            anchor: SceneTextAnchor::Middle,
            baseline: SceneTextBaseline::Alphabetic,
            style: SceneTextStyle {
                fill: text_fill,
                ..font
            },
        }),
    ])
}

fn reserve(items: &mut Vec<impl Sized>, additional: usize) -> Result<()> {
    items
        .try_reserve(additional)
        .map_err(|_| invalid("Flowchart compact scene capacity"))
}

fn render_scene(
    request: FlowchartSvgModelRequest<'_>,
    ctx: &FlowchartRenderCtx<'_>,
    edges: &[FlowchartRenderEdge<'_>],
    hierarchy: &FlowchartHierarchyPlan<'_>,
    canvas: Option<SceneColor>,
    options: &SvgExecution<'_>,
) -> Result<String> {
    let meter = ctx.work_meter;
    let root = hierarchy.root(None)?;
    let nodes = root.dom_order();
    if nodes.len() != request.layout.nodes.len() || root.edges().len() != edges.len() {
        return Err(unsupported(meter, "flowchart-nested-or-hidden-geometry"));
    }
    let mut marked = false;
    for (edge, ordered) in edges.iter().zip(root.edges()) {
        if edge.key != ordered.key {
            return Err(invalid(
                "Flowchart compact painter order differs from ungrouped routes",
            ));
        }
        marked |= admit_edge(ctx, edge.as_ref())?;
    }
    let item_count = nodes
        .len()
        .checked_mul(2)
        .and_then(|count| count.checked_add(edges.len()))
        .ok_or_else(|| meter.arithmetic_overflow())?;
    let element_count = item_count
        .checked_add(
            2 + usize::from(!edges.is_empty())
                + usize::from(canvas.is_some())
                + 3 * usize::from(marked),
        )
        .ok_or_else(|| meter.arithmetic_overflow())?;
    meter.preflight_svg_element_count(element_count, OperationPhase::Emit)?;
    meter.charge_emit_work(item_count)?;

    let config = request.render_config;
    admit_token(&config.font_family, meter)?;
    let font = parse_css_font_stack(&config.font_family)
        .ok_or_else(|| unsupported(meter, "flowchart-font-stack"))?;
    let families: Vec<_> = font
        .resolved_families()
        .map(|(name, generic)| {
            generic.map_or(SceneFontFamily::Named(name), SceneFontFamily::Generic)
        })
        .collect();
    let text_style = SceneTextStyle {
        families: &families,
        size: config.font_size,
        weight: Some(ctx.compatibility.root_font_weight.unwrap_or(400)),
        style: FontStyle::Normal,
        fill: resolved_color(&ctx.compatibility.node_text_color, meter)?,
    };
    let node_width = css_width(&ctx.compatibility.stroke_width, meter)?;
    let line_color = resolved_color(&ctx.compatibility.line_color, meter)?;
    let connector = SceneStroke {
        color: line_color,
        width: css_width(&ctx.compatibility.normal_edge_css_width(), meter)?,
        dash: None,
    };

    let mut items = Vec::new();
    reserve(&mut items, item_count)?;
    for id in nodes {
        items.extend(plain_node_items(ctx, id, text_style, node_width)?);
    }
    let edge_layer_start = items.len();
    let bounds = prepare_flowchart_rendered_bounds(
        FlowchartRenderedBoundsRequest {
            ctx,
            layout: request.layout,
            subgraph_title_y_shift: 0.0,
        },
        hierarchy,
    )?;
    let mut min_x = bounds.min_x + ctx.tx;
    let mut min_y = bounds.min_y + ctx.ty;
    let mut max_x = bounds.max_x + ctx.tx;
    let mut max_y = bounds.max_y + ctx.ty;
    let mut commands = Vec::new();
    let mut ranges = Vec::new();
    meter.charge(edges.len())?;
    reserve(&mut ranges, edges.len())?;
    let mut scratch = FlowchartEdgeDataPointsScratch::default();
    for edge in edges {
        let edge = edge.as_ref();
        let route = prepare_dagre_linear_edge_route(ctx, edge, hierarchy, &mut scratch)?
            .ok_or_else(|| unsupported(meter, "flowchart-linear-painted-route"))?;
        min_x = min_x.min(route.bounds.min_x);
        min_y = min_y.min(route.bounds.min_y);
        max_x = max_x.max(route.bounds.max_x);
        max_y = max_y.max(route.bounds.max_y);
        meter.charge_emit_work(route.points.len())?;
        reserve(&mut commands, route.points.len())?;
        let start = commands.len();
        for (index, point) in route.points.into_iter().enumerate() {
            let point = [point.x, point.y];
            commands.push(if index == 0 {
                ScenePathCommand::MoveTo(point)
            } else {
                ScenePathCommand::LineTo(point)
            });
        }
        ranges.push((
            start..commands.len(),
            flowchart_edge_marker_end_base(edge.edge).is_some(),
        ));
    }
    meter.checkpoint(OperationPhase::Layout)?;

    let root_bounds =
        root_svg::DiagramBounds::from_extents(min_x, min_y, max_x, max_y, config.diagram_padding);
    let view = options
        .root_viewport(DiagramFamilyId::FLOWCHART, ctx.diagram_id)
        .plan(
            root_svg::RootViewportSpec::mermaid(root_bounds, config.use_max_width)
                .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(root_bounds.width)),
        )?
        .view_box()
        .ok_or_else(|| invalid("Flowchart compact viewport has no viewBox"))?;
    for (span, marker) in ranges {
        items.push(SceneItem::Path(ScenePath {
            commands: &commands[span],
            paint: ScenePaint {
                fill: None,
                stroke: Some(connector),
            },
            start_marker: None,
            end_marker: marker.then_some(SceneMarkerRef(0)),
        }));
    }
    let groups = [
        SceneGroup {
            id: None,
            role: SceneRole::Edge,
            items: &items[edge_layer_start..],
        },
        SceneGroup {
            id: None,
            role: SceneRole::Node,
            items: &items[..edge_layer_start],
        },
    ];
    let marker = compact_classic_point_end(line_color);
    let markers = if marked {
        std::slice::from_ref(&marker)
    } else {
        &[]
    };
    let scene = PaintScene {
        viewport: SceneViewport {
            view_box: SceneBox {
                x: view.min_x,
                y: view.min_y,
                width: view.width,
                height: view.height,
            },
            width: view.width,
            height: view.height,
            canvas,
        },
        metadata: SceneMetadata::default(),
        markers,
        groups: &groups,
    };
    paint_scene::write_svg(&scene, ctx.diagram_id, meter)
}

pub(in crate::svg::parity) fn render_native_compact_flowchart_artifact(
    artifact: &FlowchartFamilyArtifact<FlowchartLayout>,
    metadata: &PresentationMetadata,
    execution: &SvgExecution<'_>,
) -> Result<String> {
    let request = flowchart_request(artifact, metadata)?;
    admit_request(request, execution)?;
    let canvas = admit_canvas(execution)?;
    with_flowchart_paint_context(
        request,
        &super::svg_services(execution),
        |ctx, edges, hierarchy| render_scene(request, ctx, edges, hierarchy, canvas, execution),
    )
}
