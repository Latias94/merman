//! NativeCompact projection of an admitted, ungrouped Flowchart.

use merman_markup::theme_color::ThemeColor;
use merman_model::{
    OperationPhase,
    families::flowchart::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility},
};
#[cfg(feature = "layout-shoal")]
use merman_render_core::route_paint::RouteCommand;
use merman_render_core::{
    Bounds, Error, Result,
    math::contains_delimited_math,
    paint_scene::{
        self, PaintScene, SceneBox, SceneColor, SceneFontFamily, SceneGroup, SceneItem,
        SceneMarkerRef, SceneMetadata, ScenePaint, ScenePath, ScenePathCommand, SceneRect,
        SceneRole, SceneStroke, SceneText, SceneTextAnchor, SceneTextBaseline, SceneTextStyle,
        SceneViewport,
    },
    resources::OperationWorkMeter,
    root_svg::{self, RootViewportContext},
    svg_output::SvgOutputProfile,
    text::svg_label::SVG_SOURCE_WORD_LINE_HEIGHT_EM,
    text::{FontStyle, parse_css_font_stack},
    theme_svg::{elevation::HardShadows, marker::arrow_commands},
};
use merman_theme_contract::v2::{Color, Elevation, MarkerStyle, Paint, Texture};

use crate::{
    bpmn,
    graph::FlowchartHierarchyPlan,
    markers::{
        FlowchartMarkerBase, flowchart_edge_marker_end_base, flowchart_edge_marker_start_base,
    },
    node_geometry::flowchart_node_svg_width_mode,
    projection::{FlowchartRenderEdge, FlowchartRenderEdgeRef},
    shapes::FlowchartShape,
    svg_label::FlowchartSvgLabelRenderPlan,
};

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
use super::edge_geom::LinearPaintedRoute;
#[cfg(feature = "layout-dagre")]
use super::paint::prepare_dagre_linear_edge_route;
use super::{
    FlowchartSvgModelRequest, FlowchartSvgRoute, FlowchartSvgServices,
    defs::compact_classic_point_end,
    paint::with_flowchart_paint_context,
    types::{FlowchartEdgeDataPointsScratch, FlowchartRenderCtx},
    viewbox::{FlowchartRenderedBoundsRequest, prepare_flowchart_rendered_bounds},
};
#[cfg(feature = "layout-elk")]
use super::{edge_geom::elk_straight_painted_route, paint::prepare_elk_clipped_routes};
#[cfg(feature = "layout-shoal")]
use super::{edge_geom::paint_canonical_edge_commands, paint::charged_edge_route_request};

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

fn solid_color(color: Color) -> SceneColor {
    let [red, green, blue, alpha] = color.channels().map(f64::from);
    SceneColor {
        red,
        green,
        blue,
        alpha: alpha / 255.0,
    }
}

fn admit_native_paint<'a>(
    services: &FlowchartSvgServices<'a>,
) -> Result<(Option<SceneColor>, HardShadows<'a>)> {
    let meter = services.work_meter;
    let native = services.native_family_theme;
    let background = services.canvas_background.map(Paint::Solid);
    let paint = background
        .as_ref()
        .or_else(|| native.map(|theme| &theme.roles.canvas));
    let transparent = matches!(paint, Some(Paint::Solid(color)) if color.channels()[3] == 0);
    if !transparent
        && let Some(canvas) = services.native_canvas
        && canvas
            .textures
            .iter()
            .any(|texture| !matches!(texture, Texture::None))
    {
        return Err(unsupported(meter, "flowchart-canvas-texture"));
    }
    if let Some(native) = native
        && [
            &native.roles.surface_node,
            &native.roles.stroke_border,
            &native.roles.stroke_connector,
        ]
        .into_iter()
        .any(|paint| !matches!(paint, Paint::Solid(_)))
    {
        return Err(unsupported(meter, "flowchart-native-material"));
    }
    let elevation = native.map_or(&Elevation::Flat, |theme| &theme.elevation);
    let shadows = HardShadows::resolve(elevation)
        .ok_or_else(|| unsupported(meter, "flowchart-native-material"))?;
    let canvas = match paint {
        None => None,
        Some(Paint::Solid(color)) => (!transparent).then(|| solid_color(*color)),
        Some(_) => return Err(unsupported(meter, "flowchart-canvas-paint")),
    };
    Ok((canvas, shadows))
}

fn admit_request(
    request: FlowchartSvgModelRequest<'_>,
    meter: &OperationWorkMeter,
    standard_debug: bool,
    trusted_theme_css: bool,
) -> Result<()> {
    meter.checkpoint(OperationPhase::Emit)?;
    if !standard_debug {
        return Err(unsupported(meter, "flowchart-diagnostics"));
    }
    let model = request.model;
    let config = request.render_config;
    let route = request.layout.route_contract;
    let (curve, reason) = match route {
        #[cfg(feature = "layout-dagre")]
        FlowchartSvgRoute::DagreParity => (
            "linear",
            "flowchart-requires-classic-dagre-linear-svg-labels",
        ),
        #[cfg(feature = "layout-elk")]
        FlowchartSvgRoute::ElkParity => (
            "rounded",
            "flowchart-requires-classic-elk-rounded-svg-labels",
        ),
        #[cfg(feature = "layout-shoal")]
        FlowchartSvgRoute::CanonicalAxis(_) => (
            "linear",
            "flowchart-requires-classic-shoal-linear-svg-labels",
        ),
        _ => {
            return Err(unsupported(
                meter,
                "flowchart-requires-classic-dagre-linear-svg-labels",
            ));
        }
    };
    if request.prepared_theme.compatibility.look.as_str() != "classic"
        || config.default_edge_interpolate != curve
        || config.node_html_labels
        || config.edge_html_labels
    {
        return Err(unsupported(meter, reason));
    }
    #[cfg(feature = "layout-elk")]
    if route.is_elk_parity() && request.effective_config.get_bool("elk.lineHops") != Some(false) {
        return Err(unsupported(meter, "flowchart-elk-line-hops"));
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
            if !trusted_theme_css {
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
        .copied()
        .ok_or_else(|| invalid("missing Flowchart layout edge"))?;
    let materialized = layout.materialized();
    let canonical_axis = ctx.layout.route_contract.is_canonical_axis();
    if materialized.is_none() && !canonical_axis {
        return Err(invalid("missing Flowchart layout edge"));
    }
    if style.edge_marker_color(false).is_some()
        || ctx
            .edge_style_plan
            .animation_for(render_edge.key)?
            .is_active()
        || (canonical_axis && layout.label().is_some())
        || materialized.is_some_and(|layout| {
            layout.start_label_left.is_some()
                || layout.start_label_right.is_some()
                || layout.end_label_left.is_some()
                || layout.end_label_right.is_some()
                || layout.start_marker.is_some()
                || layout.end_marker.is_some()
                || layout.stroke_dasharray.is_some()
        })
    {
        return Err(unsupported(meter, "flowchart-edge-style"));
    }
    Ok(end.is_some())
}

fn append_plain_node_items<'a>(
    ctx: &'a FlowchartRenderCtx<'_>,
    id: &str,
    font: SceneTextStyle<'a>,
    width: f64,
    items: &mut Vec<SceneItem<'a>>,
    element_count: &mut usize,
) -> Result<SceneRect> {
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
        || contains_delimited_math(label)
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
        Some(bpmn::wrapping_width(Some(shape), ctx.wrapping_width)),
        true,
        flowchart_node_svg_width_mode(label, "text", ctx.node_wrap_mode, shape),
    );
    let wrapped = plan.wrapped_lines();
    meter.checkpoint(OperationPhase::Emit)?;
    // A literal label can lend its row text when the prepared wrapping only moved
    // whole words between rows and the parity writer separates them with one space.
    if wrapped.is_empty()
        || wrapped.iter().any(Vec::is_empty)
        || !wrapped
            .iter()
            .flatten()
            .map(String::as_str)
            .eq(label.split(' '))
    {
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
    let extra_rows = wrapped.len() - 1;
    if extra_rows != 0 {
        *element_count = element_count
            .checked_add(extra_rows)
            .ok_or_else(|| meter.arithmetic_overflow())?;
        meter.preflight_svg_element_count(*element_count, OperationPhase::Emit)?;
        meter.charge_emit_work(extra_rows)?;
    }
    reserve(items, 1 + wrapped.len())?;
    let surface = SceneRect {
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
    };
    items.push(SceneItem::Rect(surface));
    let label_top = y - height / 2.0;
    let line_step = font.size * SVG_SOURCE_WORD_LINE_HEIGHT_EM;
    let mut start = 0;
    for (index, words) in wrapped.iter().enumerate() {
        let end = start + words.iter().map(|word| word.len() + 1).sum::<usize>() - 1;
        let row_y = if index == 0 {
            label_top
        } else {
            label_top + index as f64 * line_step
        };
        items.push(SceneItem::Text(SceneText {
            text: &label[start..end],
            position: [x, row_y],
            dy: font.size,
            anchor: SceneTextAnchor::Middle,
            baseline: SceneTextBaseline::Alphabetic,
            style: SceneTextStyle {
                fill: text_fill,
                ..font
            },
        }));
        start = end + 1;
    }
    Ok(surface)
}

fn reserve(items: &mut Vec<impl Sized>, additional: usize) -> Result<()> {
    items
        .try_reserve(additional)
        .map_err(|_| invalid("Flowchart compact scene capacity"))
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn append_linear_route(
    route: LinearPaintedRoute,
    commands: &mut Vec<ScenePathCommand>,
    meter: &OperationWorkMeter,
) -> Result<Bounds> {
    meter.charge_emit_work(route.points.len())?;
    reserve(commands, route.points.len())?;
    for (index, point) in route.points.into_iter().enumerate() {
        let point = [point.x, point.y];
        commands.push(if index == 0 {
            ScenePathCommand::MoveTo(point)
        } else {
            ScenePathCommand::LineTo(point)
        });
    }
    Ok(Bounds {
        min_x: route.bounds.min_x,
        min_y: route.bounds.min_y,
        max_x: route.bounds.max_x,
        max_y: route.bounds.max_y,
    })
}

#[cfg(feature = "layout-shoal")]
fn append_shoal_command(
    command: RouteCommand,
    commands: &mut Vec<ScenePathCommand>,
    meter: &OperationWorkMeter,
) -> Result<()> {
    let command = match command {
        RouteCommand::MoveTo(point) => ScenePathCommand::MoveTo([point.x, point.y]),
        RouteCommand::LineTo(point) => ScenePathCommand::LineTo([point.x, point.y]),
        RouteCommand::CubicTo { .. } | RouteCommand::QuarterArcTo { .. } => {
            return Err(unsupported(meter, "flowchart-shoal-linear-painted-route"));
        }
    };
    meter.charge_emit_work(1)?;
    reserve(commands, 1)?;
    commands.push(command);
    Ok(())
}

fn render_scene(
    request: FlowchartSvgModelRequest<'_>,
    ctx: &FlowchartRenderCtx<'_>,
    edges: &[FlowchartRenderEdge<'_>],
    hierarchy: &FlowchartHierarchyPlan<'_>,
    (canvas, shadows): (Option<SceneColor>, HardShadows<'_>),
    root_viewport: &RootViewportContext<'_>,
) -> Result<String> {
    let meter = ctx.work_meter;
    let root = hierarchy.root(None)?;
    let nodes = root.dom_order();
    if nodes.len() != request.layout.nodes.len() || root.edges().len() != edges.len() {
        return Err(unsupported(meter, "flowchart-nested-or-hidden-geometry"));
    }
    let mut marked = false;
    let theme_sketch = super::paint::uses_theme_sketch(ctx);
    for (edge, ordered) in edges.iter().zip(root.edges()) {
        if edge.key != ordered.key {
            return Err(invalid(
                "Flowchart compact painter order differs from ungrouped routes",
            ));
        }
        marked |= admit_edge(ctx, edge.as_ref())?;
        if theme_sketch && !super::paint::has_authored_curve(ctx, edge.key) {
            return Err(unsupported(meter, "flowchart-shoal-sketch-painted-route"));
        }
    }
    let foreground_count = nodes
        .len()
        .checked_mul(2)
        .and_then(|count| count.checked_add(edges.len()))
        .ok_or_else(|| meter.arithmetic_overflow())?;
    let shadow_count = nodes
        .len()
        .checked_mul(shadows.layer_count())
        .ok_or_else(|| meter.arithmetic_overflow())?;
    let item_count = foreground_count
        .checked_add(shadow_count)
        .ok_or_else(|| meter.arithmetic_overflow())?;
    let mut element_count = item_count
        .checked_add(
            2 + usize::from(!edges.is_empty())
                + usize::from(canvas.is_some())
                + usize::from(shadow_count != 0)
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
    reserve(&mut items, foreground_count)?;
    let mut depth_items = Vec::new();
    reserve(&mut depth_items, shadow_count)?;
    let mut depth_bounds: Option<kurbo::Rect> = None;
    for id in nodes {
        let rect = append_plain_node_items(
            ctx,
            id,
            text_style,
            node_width,
            &mut items,
            &mut element_count,
        )?;
        if shadow_count != 0 {
            depth_items.extend(
                shadows
                    .rectangles(rect.bounds, rect.radius)
                    .map(SceneItem::Rect),
            );
            let shadow_bounds = shadows.bounds(rect.bounds);
            depth_bounds =
                Some(depth_bounds.map_or(shadow_bounds, |bounds| bounds.union(shadow_bounds)));
        }
    }
    let edge_layer_start = items.len();
    let bounds = prepare_flowchart_rendered_bounds(
        FlowchartRenderedBoundsRequest {
            ctx,
            subgraph_title_y_shift: 0.0,
        },
        hierarchy,
    )?;
    let mut min_x = bounds.min_x + ctx.tx;
    let mut min_y = bounds.min_y + ctx.ty;
    let mut max_x = bounds.max_x + ctx.tx;
    let mut max_y = bounds.max_y + ctx.ty;
    if let Some(bounds) = depth_bounds {
        min_x = min_x.min(bounds.x0);
        min_y = min_y.min(bounds.y0);
        max_x = max_x.max(bounds.x1);
        max_y = max_y.max(bounds.y1);
    }
    let mut commands = Vec::new();
    let mut ranges = Vec::new();
    meter.charge(edges.len())?;
    reserve(&mut ranges, edges.len())?;
    let mut scratch = FlowchartEdgeDataPointsScratch::default();
    #[cfg(feature = "layout-elk")]
    let mut elk_routes = if ctx.uses_elk_adapter_dom {
        Some(prepare_elk_clipped_routes(
            ctx,
            edges,
            hierarchy,
            &mut scratch,
        )?)
    } else {
        None
    };
    for edge in edges {
        let edge = edge.as_ref();
        let start = commands.len();
        let route_bounds: Bounds = match ctx.layout.route_contract {
            #[cfg(feature = "layout-dagre")]
            FlowchartSvgRoute::DagreParity => {
                let route = prepare_dagre_linear_edge_route(ctx, edge, hierarchy, &mut scratch)?
                    .ok_or_else(|| unsupported(meter, "flowchart-linear-painted-route"))?;
                append_linear_route(route, &mut commands, meter)?
            }
            #[cfg(feature = "layout-elk")]
            FlowchartSvgRoute::ElkParity => {
                let route = elk_routes
                    .as_mut()
                    .and_then(Iterator::next)
                    .filter(|(request, _, _)| request.key == edge.key)
                    .and_then(|(request, _, route)| {
                        elk_straight_painted_route(request, route, &mut scratch)
                    })
                    .ok_or_else(|| unsupported(meter, "flowchart-elk-straight-painted-route"))?;
                append_linear_route(route, &mut commands, meter)?
            }
            #[cfg(feature = "layout-shoal")]
            FlowchartSvgRoute::CanonicalAxis(axis) => {
                let (request, _) = charged_edge_route_request(ctx, edge, hierarchy)?;
                let bounds =
                    paint_canonical_edge_commands(request, axis, &mut scratch, |command| {
                        append_shoal_command(command, &mut commands, meter)
                    })?
                    .ok_or_else(|| invalid("missing Flowchart canonical route"))?;
                if commands.len() - start < 2 {
                    return Err(unsupported(meter, "flowchart-shoal-linear-painted-route"));
                }
                bounds
            }
            _ => return Err(invalid("unadmitted Flowchart compact route contract")),
        };
        min_x = min_x.min(route_bounds.min_x);
        min_y = min_y.min(route_bounds.min_y);
        max_x = max_x.max(route_bounds.max_x);
        max_y = max_y.max(route_bounds.max_y);
        ranges.push((
            start..commands.len(),
            flowchart_edge_marker_end_base(edge.edge).is_some(),
        ));
    }
    meter.checkpoint(OperationPhase::Layout)?;

    let root_bounds =
        root_svg::DiagramBounds::from_extents(min_x, min_y, max_x, max_y, config.diagram_padding);
    let view = root_viewport
        .plan(
            root_svg::RootViewportSpec::mermaid(root_bounds, config.use_max_width)
                .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(root_bounds.width)),
        )?
        .view_box()
        .ok_or_else(|| invalid("Flowchart compact viewport has no viewBox"))?;
    reserve(&mut items, ranges.len())?;
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
            role: SceneRole::Depth,
            items: &depth_items,
        },
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
    let marker_style = ctx
        .native
        .map_or(MarkerStyle::Classic, |native| native.form.marker_style);
    let mut marker = compact_classic_point_end(line_color);
    if let Some(commands) = arrow_commands(marker_style) {
        marker.commands = commands;
    }
    if marker_style == MarkerStyle::Open {
        marker.paint.fill = None;
        marker.round_stroke = true;
    }
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
        groups: &groups[usize::from(depth_items.is_empty())..],
    };
    paint_scene::write_svg(&scene, ctx.diagram_id, meter)
}

/// Render an admitted Flowchart with the native compact SVG profile.
pub fn render_native_compact(
    request: FlowchartSvgModelRequest<'_>,
    services: &FlowchartSvgServices<'_>,
    standard_debug: bool,
    trusted_theme_css: bool,
) -> Result<String> {
    admit_request(
        request,
        services.work_meter,
        standard_debug,
        trusted_theme_css,
    )?;
    let paint = admit_native_paint(services)?;
    with_flowchart_paint_context(request, services, |ctx, edges, hierarchy| {
        render_scene(
            request,
            ctx,
            edges,
            hierarchy,
            paint,
            &services.root_viewport,
        )
    })
}

#[cfg(all(test, feature = "layout-shoal"))]
mod tests {
    use super::*;
    use merman_model::OperationLedgerError;
    use merman_render_core::{
        resources::{RenderResourcePolicy, ResourceLimitId},
        route_paint::{CurveTerminals, RouteCommandError, RoutePaintRequest, RouteStyle},
        route_types::{Axis, RoutePoint, Side},
    };

    #[test]
    fn shoal_compact_command_sink_keeps_the_admitted_work_prefix_and_phase() {
        let points = [
            RoutePoint::new(0.0, 0.0, Axis::Horizontal),
            RoutePoint::new(64.0, 0.0, Axis::Horizontal),
        ];
        let route = || RoutePaintRequest {
            points: &points,
            sides: (Side::Right, Side::Left),
            style: RouteStyle::Linear,
            markers: Default::default(),
            curve_terminals: CurveTerminals::Smooth,
            stitches: &[],
            clearance: None,
        };
        let render = |meter: &OperationWorkMeter, cancel: bool| {
            let mut commands = Vec::new();
            let result = route().paint_commands(&mut Default::default(), |command| {
                append_shoal_command(command, &mut commands, meter)?;
                if cancel {
                    meter.control().cancel();
                }
                Ok::<_, Error>(())
            });
            (result, commands.len())
        };
        let meter = |maximum| {
            OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, maximum)
                    .unwrap(),
            )
        };

        let exact = meter(2);
        let (result, commands) = render(&exact, false);
        assert!(result.is_ok());
        assert_eq!((commands, exact.used()), (2, 2));

        let limited = meter(1);
        let (result, commands) = render(&limited, false);
        assert!(matches!(
            result,
            Err(RouteCommandError::Sink(Error::ResourceLimitExceeded(limit)))
                if limit.limit == "max_layout_work_units" && limit.actual == 2 && limit.max == 1
        ));
        assert_eq!((commands, limited.used()), (1, 1));
        assert!(matches!(
            limited.control().terminal_checkpoint_at(OperationPhase::Layout),
            Err(OperationLedgerError::ResourceLimitExceeded(error))
                if error.phase == OperationPhase::Emit
        ));

        let cancelled = meter(2);
        let (result, commands) = render(&cancelled, true);
        assert!(matches!(
            result,
            Err(RouteCommandError::Sink(Error::Cancelled(error))) if error.phase == OperationPhase::Emit
        ));
        assert_eq!((commands, cancelled.used()), (1, 1));
        assert!(matches!(
            cancelled.control().terminal_checkpoint_at(OperationPhase::Layout),
            Err(OperationLedgerError::Cancelled(error)) if error.phase == OperationPhase::Emit
        ));
    }
}
