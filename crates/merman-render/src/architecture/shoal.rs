mod geometry;
mod measure;

use super::ArchitectureGroupThemePlan;
use crate::architecture_metrics::{
    ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX, ARCHITECTURE_SHOAL_CAPTION_PADDING,
    extend_bounds,
};
use crate::config::config_f64;
use crate::graph_layout::Marker;
use crate::graph_layout::canonical::{CanonicalRoutes, CaptionClearance};
use crate::graph_layout::contour::{ShapePortProfile, shoal_contour};
use crate::graph_layout::ir::{
    Axis, Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, Point,
    Rect, Side, Size, SolvedGroup, TitlePosition,
};
use crate::graph_layout::placement;
use crate::graph_layout::route_paint::RouteStyle;
use crate::layout_frame::GraphPlacement;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{
    ArchitectureDiagramLayout, Bounds, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::resources::PreparedTextRetainedReservation;
use crate::{Error, LayoutExecution, Result};
use kurbo::Insets;
use merman_layout_shoal::{NodeId, PortSideHint, grid};
use merman_model::families::architecture::{
    ArchitectureDiagramModel, ArchitectureLayoutDirection, ArchitectureNodeKind,
};
use merman_model::presentation::PresentationConfig;
use merman_render_core::graph_layout::shoal::{
    Constraints, Placement, project_input, project_output,
};
use merman_theme_contract::v2::MarkerStyle;
use rustc_hash::FxHashMap;

pub(crate) use measure::ArchitectureShoalText;
use measure::TEXT_CLEARANCE;

#[derive(Debug)]
pub(crate) struct ArchitectureShoalGeometry {
    pub(crate) routes: CanonicalRoutes,
    pub(crate) groups: Vec<ArchitectureShoalGroup>,
    pub(crate) service_titles: Vec<Option<ArchitectureShoalTitle>>,
    pub(crate) junction_diameters: Vec<f64>,
    pub(crate) edge_labels: Vec<Option<ArchitectureShoalText>>,
    pub(crate) edge_stroke_outset: f64,
    pub(crate) route_style: RouteStyle,
    _label_storage: PreparedTextRetainedReservation,
}

impl ArchitectureShoalGeometry {
    pub(crate) fn placement(&self) -> &GraphPlacement {
        &self.routes.placement
    }
}

#[derive(Debug)]
pub(crate) struct ArchitectureShoalGroup {
    pub(crate) rect: Rect,
    pub(crate) parent: Option<u32>,
    pub(crate) icon_origin: Option<Point>,
    pub(crate) title: Option<ArchitectureShoalTitle>,
}

#[derive(Debug)]
pub(crate) struct ArchitectureShoalTitle {
    pub(crate) anchor: Point,
    pub(crate) text: ArchitectureShoalText,
}

struct Header {
    title: Option<ArchitectureShoalText>,
    icon_size: Option<f64>,
    size: Option<Size>,
}

impl Header {
    fn new(title: Option<ArchitectureShoalText>, icon_size: Option<f64>) -> Self {
        let size = (title.is_some() || icon_size.is_some()).then(|| {
            let text = title
                .as_ref()
                .map_or(Size::default(), ArchitectureShoalText::size);
            let icon = icon_size.unwrap_or(0.0);
            let gap = if title.is_some() && icon_size.is_some() {
                TEXT_CLEARANCE
            } else {
                0.0
            };
            Size {
                width: text.width + icon + gap,
                height: text.height.max(icon),
            }
        });
        Self {
            title,
            icon_size,
            size,
        }
    }

    fn place(self, group: &SolvedGroup) -> ArchitectureShoalGroup {
        let mut icon_origin = None;
        let mut title = None;
        if let Some(heading) = &group.title {
            let left = heading.x - heading.width / 2.0;
            icon_origin = self.icon_size.map(|size| Point {
                x: left,
                y: heading.y - size / 2.0,
            });
            title = self.title.map(|text| {
                let label = LayoutLabel {
                    x: left + heading.width - text.size().width / 2.0,
                    y: heading.y,
                    width: text.size().width,
                    height: text.size().height,
                };
                ArchitectureShoalTitle {
                    anchor: text.centered_at(&label),
                    text,
                }
            });
        }
        ArchitectureShoalGroup {
            rect: group.rect,
            parent: group.parent,
            icon_origin,
            title,
        }
    }
}

fn invalid(message: String) -> Error {
    Error::InvalidModel { message }
}

fn sequence(index: usize, work: &OperationLayoutWorkControl) -> Result<u32> {
    u32::try_from(index).map_err(|_| work.record_arithmetic_overflow().into())
}

fn side(direction: char) -> Result<Side> {
    match direction {
        'T' => Ok(Side::Top),
        'R' => Ok(Side::Right),
        'B' => Ok(Side::Bottom),
        'L' => Ok(Side::Left),
        _ => Err(invalid(format!(
            "Unknown Architecture port side `{direction}`"
        ))),
    }
}

fn group_id(groups: &FxHashMap<&str, u32>, parent: Option<&str>) -> Result<Option<u32>> {
    parent
        .map(|id| {
            groups
                .get(id)
                .copied()
                .ok_or_else(|| invalid(format!("Unknown Architecture group `{id}`")))
        })
        .transpose()
}

fn endpoint(parents: &[Option<u32>], node: u32, is_group: Option<bool>) -> Endpoint {
    match (is_group, parents[node as usize]) {
        (Some(true), Some(group)) => Endpoint::Group(group),
        // The parser also accepts a group annotation on an ungrouped service.
        _ => Endpoint::Node(node),
    }
}

fn marker(arrow: Option<bool>, size: f64, style: MarkerStyle) -> Marker {
    if arrow == Some(true) {
        let trim = match style {
            MarkerStyle::Classic | MarkerStyle::Filled => size,
            MarkerStyle::Stealth => size * 0.72,
            MarkerStyle::Open => 0.0,
        };
        Marker {
            trim,
            depth: size - trim,
            width: size,
        }
    } else {
        Marker::default()
    }
}

fn translate_bounds(rect: Rect) -> Bounds {
    Bounds {
        min_x: rect.x,
        min_y: rect.y,
        max_x: rect.x + rect.width,
        max_y: rect.y + rect.height,
    }
}

fn spacing(config: &PresentationConfig, key: &str, default: f64) -> f64 {
    config_f64(config.as_value(), &["architecture", key])
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(default)
}

fn node_index(indices: &FxHashMap<&str, u32>, id: &str) -> Result<u32> {
    indices
        .get(id)
        .copied()
        .ok_or_else(|| invalid(format!("Unknown Architecture node `{id}`")))
}

pub(crate) fn layout_architecture_shoal(
    model: &ArchitectureDiagramModel,
    config: &PresentationConfig,
    group_theme: &ArchitectureGroupThemePlan,
    execution: &LayoutExecution<'_>,
) -> Result<(ArchitectureDiagramLayout, ArchitectureShoalGeometry)> {
    let mut work = OperationLayoutWorkControl::new(execution.work_meter());
    let n = model.nodes.len();
    let g = model.groups.len();
    let m = model.edges.len();
    work.charge_adapter(work.checked_add(work.checked_add(n, g)?, m)?)?;
    let typography = group_theme.typography_theme();
    let native = group_theme.native_style();
    let binding = typography.css_binding();
    let style = typography.svg_text_style();
    let measurer = execution.text_measurer();
    let icon = binding.icon_size_px;
    let arrow = icon / 6.0;
    let marker_style = native.map_or(MarkerStyle::Classic, |style| style.marker_style);
    let route_style = native.map_or(RouteStyle::Step, |style| {
        RouteStyle::from_theme(style.edge_style())
    });
    let edge_stroke_outset = binding.edge_stroke_outset();
    let marker_port_width = arrow + 2.0 * edge_stroke_outset.max(1.0);
    // The deterministic measurer can supply a neutral middle baseline. Leave half an em for
    // differences in the browser's x-height and keep the visible shaft outside that allowance.
    let label_clearance = (style.font_size / 2.0 + edge_stroke_outset).max(TEXT_CLEARANCE);
    let group_padding = binding.padding_px.max(2.0 * TEXT_CLEARANCE);
    let rank_spacing = (spacing(config, "idealEdgeLengthMultiplier", 1.5) * icon)
        .max(2.0 * arrow + 4.0 * TEXT_CLEARANCE);
    let node_spacing = spacing(config, "nodeSeparation", 75.0);
    let wrap = ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX;

    let mut node_indices = FxHashMap::default();
    let mut group_indices = FxHashMap::default();
    for (index, node) in model.nodes.iter().enumerate() {
        work.charge_adapter(node.id.len())?;
        if node_indices
            .insert(node.id.as_str(), sequence(index, &work)?)
            .is_some()
        {
            return Err(invalid(format!(
                "Duplicate Architecture node `{}`",
                node.id
            )));
        }
    }
    for (index, group) in model.groups.iter().enumerate() {
        work.charge_adapter(group.id.len())?;
        if node_indices.contains_key(group.id.as_str())
            || group_indices
                .insert(group.id.as_str(), sequence(index, &work)?)
                .is_some()
        {
            return Err(invalid(format!(
                "Duplicate Architecture group `{}`",
                group.id
            )));
        }
    }
    let parents = model
        .nodes
        .iter()
        .map(|node| {
            work.charge_adapter(node.in_group.as_deref().map_or(0, str::len))?;
            group_id(&group_indices, node.in_group.as_deref())
        })
        .collect::<Result<Vec<_>>>()?;

    let mut headers = Vec::with_capacity(g);
    let mut groups = Vec::with_capacity(g);
    for (index, group) in model.groups.iter().enumerate() {
        work.charge_adapter(group.in_group.as_deref().map_or(0, str::len))?;
        let title = ArchitectureShoalText::start(
            group.title.as_deref(),
            wrap,
            measurer,
            &style,
            &mut work,
        )?;
        let icon_size = group
            .icon
            .as_deref()
            .is_some_and(|name| !name.trim().is_empty() && binding.padding_px > 0.0)
            .then_some(binding.padding_px * 0.75);
        let header = Header::new(title, icon_size);
        groups.push(MeasuredGroup {
            seq: sequence(index, &work)?,
            parent: group_id(&group_indices, group.in_group.as_deref())?,
            direction: None,
            padding: Insets::new(
                group_padding,
                group_padding + header.size.map_or(0.0, |s| s.height + TEXT_CLEARANCE),
                group_padding,
                group_padding,
            ),
            min_size: Size {
                width: icon,
                height: icon,
            },
            title: header.size,
            title_position: TitlePosition::TopAuto,
        });
        headers.push(header);
    }

    let mut alignments = Vec::new();
    for hint in &model.layout_hints {
        let axis = match hint.direction {
            ArchitectureLayoutDirection::Row => Axis::Horizontal,
            ArchitectureLayoutDirection::Column => Axis::Vertical,
        };
        let mut previous = None;
        for member in &hint.members {
            work.charge_adapter(work.checked_add(member.len(), 1)?)?;
            let next = NodeId(node_index(&node_indices, member)?);
            if let Some(first) = previous {
                alignments.push(grid::Alignment { first, next, axis });
            }
            previous = Some(next);
        }
    }

    let mut edges = Vec::with_capacity(m);
    let mut sides = Vec::with_capacity(m);
    let mut edge_labels = Vec::with_capacity(m);
    let mut bottom_degree = vec![0usize; n];
    let mut has_top_port = vec![false; n];
    let mut has_bottom_marker = vec![false; n];
    let face_index = |side: Side| side as usize;
    let mut junction_faces = vec![[(0usize, false); 4]; n];
    for edge in &model.edges {
        work.charge_adapter(work.checked_add(edge.lhs_id.len(), edge.rhs_id.len())?)?;
        let source = endpoint(
            &parents,
            node_index(&node_indices, &edge.lhs_id)?,
            edge.lhs_group,
        );
        let target = endpoint(
            &parents,
            node_index(&node_indices, &edge.rhs_id)?,
            edge.rhs_group,
        );
        let pair = (side(edge.lhs_dir)?, side(edge.rhs_dir)?);
        for (end, port, arrow) in [
            (source, pair.0, edge.lhs_into),
            (target, pair.1, edge.rhs_into),
        ] {
            if let Endpoint::Node(id) = end {
                let index = id as usize;
                match port {
                    Side::Top => has_top_port[index] = true,
                    Side::Bottom => {
                        bottom_degree[index] += 1;
                        has_bottom_marker[index] |= arrow == Some(true);
                    }
                    Side::Left | Side::Right => {}
                }
                if model.nodes[index].node_type == ArchitectureNodeKind::Junction {
                    let (degree, marked) = &mut junction_faces[index][face_index(port)];
                    *degree += 1;
                    *marked |= arrow == Some(true);
                }
            }
        }
        sides.push(pair);
        let text = ArchitectureShoalText::centered(
            edge.title.as_deref(),
            wrap,
            measurer,
            &style,
            &mut work,
        )?;
        let label = text.as_ref().map(|text| {
            let size = text.size();
            let (pad_x, pad_y) = ARCHITECTURE_SHOAL_CAPTION_PADDING;
            // Grid seating can use the entire slot beside a fork. Reserve the backplate and
            // empty space around it so the painted box has a gap even in a tight slot.
            Size {
                width: size.width + 2.0 * (pad_x + TEXT_CLEARANCE),
                height: size.height + 2.0 * (pad_y + TEXT_CLEARANCE),
            }
        });
        edge_labels.push(text);
        edges.push(MeasuredEdge {
            source,
            target,
            min_rank_span: 1,
            weight: 1.0,
            label,
            start_label: None,
            end_label: None,
            start_marker: marker(edge.lhs_into, arrow, marker_style),
            end_marker: marker(edge.rhs_into, arrow, marker_style),
        });
    }

    let mut nodes = Vec::with_capacity(n);
    let mut profiles = Vec::with_capacity(n);
    let mut titles = Vec::with_capacity(n);
    let mut bottom_pitch = vec![None; n];
    let mut bottom_ordinal = vec![0; n];
    let junction_pitch = merman_layout_shoal::Spacing::default()
        .port_pitch
        .max(marker_port_width);
    let junction_diameters: Vec<_> = junction_faces
        .iter()
        .map(|faces| {
            let shared = faces
                .iter()
                .filter_map(|&(degree, marked)| marked.then_some(degree))
                .max()
                .unwrap_or_default()
                .saturating_sub(1);
            let degree = faces.iter().map(|face| face.0).sum::<usize>();
            if degree >= 3 || faces.iter().any(|face| face.1) {
                (2.0 * (arrow * 0.3).min(4.0)).max(
                    shared as f64 * junction_pitch
                        / merman_render_core::graph_layout::contour::ROUNDED_PORT_FRACTION,
                )
            } else {
                0.0
            }
        })
        .collect();
    for (index, node) in model.nodes.iter().enumerate() {
        work.charge_adapter(1)?;
        let junction = node.node_type == ArchitectureNodeKind::Junction;
        let radius = native.map_or_else(
            || {
                if node.icon.is_none() && node.icon_text.as_deref().is_none_or(str::is_empty) {
                    5.0
                } else {
                    0.0
                }
            },
            |style| style.radius,
        );
        // Keep the measured routing box for junction leads and clip visible junction contacts
        // to the dot.
        let diameter = junction_diameters[index];
        let profile = if junction {
            if diameter == 0.0 {
                ShapePortProfile::rectangle(0.0, 0.0)
            } else {
                ShapePortProfile::ellipse(diameter, diameter)
            }
        } else {
            ShapePortProfile::rounded_rectangle(icon, icon, radius, radius)
        };
        let span = profile.port_span(Side::Top);
        let title = if junction {
            None
        } else {
            ArchitectureShoalText::centered(
                node.title.as_deref(),
                icon * 1.5,
                measurer,
                &style,
                &mut work,
            )?
        };
        let half = icon.max(diameter) / 2.0;
        let mut w = half;
        let mut h = half;
        let title = title.map(|text| {
            let anchor = if bottom_degree[index] == 0 {
                Point { x: 0.0, y: half }
            } else {
                let intervals = (bottom_degree[index] - 1) as f64;
                let pitch = (arrow + TEXT_CLEARANCE).min(span / intervals.max(1.0));
                bottom_pitch[index] = Some(pitch);
                if bottom_degree[index] == 1 && !has_top_port[index] {
                    // A single bottom stem can leave the title centered above the icon.
                    Point {
                        x: 0.0,
                        y: -half - TEXT_CLEARANCE - text.bounds.max_y,
                    }
                } else {
                    let half_terminal = if has_bottom_marker[index] || bottom_degree[index] > 1 {
                        (arrow / 2.0).max(edge_stroke_outset)
                    } else {
                        edge_stroke_outset
                    };
                    let left = pitch * intervals / 2.0 + half_terminal + TEXT_CLEARANCE;
                    Point {
                        x: (left - text.bounds.min_x).max(0.0),
                        y: half,
                    }
                }
            };
            let ink = text.bounds_at(anchor);
            w = w.max(ink.min_x.abs().max(ink.max_x.abs()) + TEXT_CLEARANCE);
            h = h.max(ink.min_y.abs().max(ink.max_y.abs()) + TEXT_CLEARANCE);
            ArchitectureShoalTitle { anchor, text }
        });
        titles.push(title);
        let size = Size {
            width: 2.0 * w,
            height: 2.0 * h,
        };
        let contour = shoal_contour(&profile, size);
        profiles.push(profile);
        nodes.push(MeasuredNode {
            size,
            contour,
            seq: sequence(index, &work)?,
            parent: parents[index],
            port_span: Some(span),
            perpendicular_port_span: Some(span),
            anchor_y: Some(h),
        });
    }
    let mut ports = Vec::with_capacity(m);
    for (edge, (lhs, rhs)) in edges.iter().zip(sides) {
        let mut port = |endpoint, side| {
            if let Endpoint::Node(id) = endpoint {
                let index = id as usize;
                if model.nodes[index].node_type == ArchitectureNodeKind::Junction {
                    let (degree, marked) = junction_faces[index][face_index(side)];
                    return if marked && degree > 1 {
                        PortSideHint::Side(side)
                    } else {
                        PortSideHint::FixedOffset { side, offset: 0.0 }
                    };
                }
                if side == Side::Bottom
                    && let Some(pitch) = bottom_pitch[index]
                {
                    let offset = pitch
                        * (bottom_ordinal[index] as f64 - (bottom_degree[index] - 1) as f64 / 2.0);
                    bottom_ordinal[index] += 1;
                    return PortSideHint::FixedOffset { side, offset };
                }
            }
            PortSideHint::Side(side)
        };
        ports.push((port(edge.source, lhs), port(edge.target, rhs)));
    }
    let graph = MeasuredGraph {
        direction: Direction::Right,
        routing: route_style.routing_style(),
        node_spacing,
        rank_spacing,
        component_spacing: node_spacing,
        inherit_crossed_group_direction: false,
        nodes,
        groups,
        edges,
    };
    let node_ids = || model.nodes.iter().map(|node| node.id.as_str());
    let group_ids = || model.groups.iter().map(|group| group.id.as_str());
    let hints =
        placement::previous_order_hints(&graph, node_ids(), group_ids(), execution, &mut work)?;
    let input = Placement {
        width: if execution.adaptation == crate::LayoutAdaptation::FitWidth {
            merman_layout_shoal::WidthPolicy::FitWidth {
                max_width: execution.container_width,
                allow_turn: false,
                turn_breakpoint: 0.0,
                fold_wide_ranks: false,
            }
        } else {
            merman_layout_shoal::WidthPolicy::Natural
        },
        capture_ranks: execution.capture_shoal_frame,
        hints: &hints,
        constraints: Constraints {
            edge_ports: &ports,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut options = input.options_for(&graph);
    options.spacing.edge = options.spacing.edge.max(2.0 * edge_stroke_outset);
    options.spacing.end_label_across = label_clearance;
    let mut measured = project_input(&graph, &input, &mut work)?;
    for edge in &mut measured.edges {
        work.charge_adapter(1)?;
        for marker in [&mut edge.start_marker, &mut edge.end_marker] {
            if marker.width > 0.0 {
                marker.width = marker_port_width;
            }
        }
    }
    let geometry = crate::layout_work::measure_layout_solve!(grid::layout_with_control(
        &measured,
        &alignments,
        &options,
        &mut work,
    ))
    .map_err(|error| work.map_shoal_error(error))?;
    let mut solved = project_output(&graph, geometry, &mut work)?;
    work.charge_adapter(solved.nodes.len())?;
    let painted_nodes: Vec<_> = model
        .nodes
        .iter()
        .zip(&solved.nodes)
        .zip(&titles)
        .enumerate()
        .map(|(index, ((node, &body), title))| {
            let size = if node.node_type == ArchitectureNodeKind::Junction {
                junction_diameters[index]
            } else {
                icon
            };
            geometry::node_ink(body, title.as_ref(), size)
        })
        .collect();
    geometry::flatten_top_corridors(
        &graph,
        &mut solved,
        &painted_nodes,
        (&profiles, native.map_or(0.0, |style| style.group_radius)),
        style.font_size,
        edge_stroke_outset,
        &mut work,
    )?;
    // Reserve enough space for a shaft, its marker, and the painted corners at a scope border.
    let route_outset = edge_stroke_outset
        + arrow
            .max(10.0)
            .max(native.map_or(0.0, |style| style.group_radius));
    geometry::compact_empty_groups(&graph, &mut solved, node_spacing, route_outset, &mut work)?;
    let placement = placement::placement_for_solved(
        &graph,
        &solved,
        node_ids(),
        group_ids(),
        execution,
        &mut work,
    )?;

    let mut layout = ArchitectureDiagramLayout {
        nodes: Vec::with_capacity(n + g),
        edges: Vec::with_capacity(m),
        cytoscape_service_bounds: Vec::new(),
        fcose_compound_bounds: Vec::new(),
        bounds: None,
    };
    let mut retained = 0usize;
    for (index, ((node, &rect), title)) in model
        .nodes
        .iter()
        .zip(&solved.nodes)
        .zip(&mut titles)
        .enumerate()
    {
        work.charge_adapter(node.id.len())?;
        let center = rect.center();
        if let Some(title) = title {
            title.anchor.x += center.x;
            title.anchor.y += center.y;
            retained = work.checked_add(retained, title.text.checked_retained_bytes(&work)?)?;
        }
        let size = if node.node_type == ArchitectureNodeKind::Junction
            && junction_diameters[index] > 0.0
        {
            junction_diameters[index]
        } else {
            icon
        };
        layout.nodes.push(LayoutNode {
            id: node.id.clone(),
            x: center.x - size / 2.0,
            y: center.y - size / 2.0,
            width: size,
            height: size,
            is_cluster: false,
            label_width: title.as_ref().map(|title| title.text.size().width),
            label_height: title.as_ref().map(|title| title.text.size().height),
        });
        extend_bounds(&mut layout.bounds, translate_bounds(rect));
    }
    let mut groups = Vec::with_capacity(g);
    for ((model, header), solved) in model.groups.iter().zip(headers).zip(&solved.groups) {
        work.charge_adapter(model.id.len())?;
        let group = header.place(solved);
        if let Some(title) = &group.title {
            retained = work.checked_add(retained, title.text.checked_retained_bytes(&work)?)?;
        }
        let rect = group.rect;
        layout.nodes.push(LayoutNode {
            id: model.id.clone(),
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            is_cluster: true,
            label_width: group.title.as_ref().map(|title| title.text.size().width),
            label_height: group.title.as_ref().map(|title| title.text.size().height),
        });
        extend_bounds(&mut layout.bounds, translate_bounds(rect));
        groups.push(group);
    }

    for (i, ((edge, solved_edge), label)) in model
        .edges
        .iter()
        .zip(&solved.edges)
        .zip(&edge_labels)
        .enumerate()
    {
        work.charge_adapter(work.checked_add(
            solved_edge.points.len(),
            work.checked_add(edge.lhs_id.len(), edge.rhs_id.len())?,
        )?)?;
        if let Some(label) = label {
            retained = work.checked_add(retained, label.checked_retained_bytes(&work)?)?;
        }
        let group_name = |end| match end {
            Endpoint::Group(id) => Some(model.groups[id as usize].id.clone()),
            _ => None,
        };
        let mut points = solved_edge.points.clone();
        if let [start, .., end] = points.as_mut_slice() {
            for (point, endpoint, side) in [
                (start, solved_edge.source, solved_edge.ports.0),
                (end, solved_edge.target, solved_edge.ports.1),
            ] {
                if let Endpoint::Node(id) = endpoint {
                    let index = id as usize;
                    let center = solved.nodes[index].center();
                    *point = profiles[index].contact(
                        &LayoutPoint {
                            x: center.x,
                            y: center.y,
                        },
                        side,
                        *point,
                    );
                }
            }
        }
        let mut edge_bounds = Bounds::from_points(points.iter().map(|p| (p.x, p.y)))
            .ok_or_else(|| invalid(format!("Architecture route edge-{i} has no points")))?;
        edge_bounds.min_x -= edge_stroke_outset;
        edge_bounds.min_y -= edge_stroke_outset;
        edge_bounds.max_x += edge_stroke_outset;
        edge_bounds.max_y += edge_stroke_outset;
        extend_bounds(&mut layout.bounds, edge_bounds);
        layout.edges.push(LayoutEdge {
            id: format!("edge-{i}"),
            from: edge.lhs_id.clone(),
            to: edge.rhs_id.clone(),
            from_cluster: group_name(solved_edge.source),
            to_cluster: group_name(solved_edge.target),
            points: points
                .into_iter()
                .map(|p| LayoutPoint { x: p.x, y: p.y })
                .collect(),
            label: solved_edge.label.clone().map(|mut placed| {
                if let Some(text) = label {
                    let size = text.size();
                    (placed.width, placed.height) = (size.width, size.height);
                }
                placed
            }),
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
    }
    if let Some(bounds) = &mut layout.bounds {
        bounds.min_x = bounds.min_x.min(0.0);
        bounds.min_y = bounds.min_y.min(0.0);
        bounds.max_x = bounds.max_x.max(solved.bounds.width);
        bounds.max_y = bounds.max_y.max(solved.bounds.height);
    }
    let mut routes = CanonicalRoutes::from_solved(
        &graph,
        &solved,
        &profiles,
        |_| native.map_or(0.0, |style| style.group_radius),
        layout.edges.iter().map(|edge| edge.id.as_str()),
        CaptionClearance::AvoidWithPaintedNodes(&painted_nodes),
        &mut work,
    )?;
    routes.placement = placement;
    let retained = work.checked_add(
        retained,
        work.checked_mul(
            titles.capacity(),
            size_of::<Option<ArchitectureShoalTitle>>(),
        )?,
    )?;
    let retained = work.checked_add(
        retained,
        work.checked_mul(
            edge_labels.capacity(),
            size_of::<Option<ArchitectureShoalText>>(),
        )?,
    )?;
    let retained = work.checked_add(
        retained,
        work.checked_mul(groups.capacity(), size_of::<ArchitectureShoalGroup>())?,
    )?;
    let retained = work.checked_add(
        retained,
        work.checked_mul(junction_diameters.capacity(), size_of::<f64>())?,
    )?;
    Ok((
        layout,
        ArchitectureShoalGeometry {
            routes,
            groups,
            service_titles: titles,
            junction_diameters,
            edge_labels,
            edge_stroke_outset,
            route_style,
            _label_storage: work.reserve_prepared_text_retained_bytes(retained)?,
        },
    ))
}
