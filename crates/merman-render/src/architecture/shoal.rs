mod align;
mod measure;

use super::ArchitectureGroupThemePlan;
use crate::architecture_metrics::{ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX, extend_bounds};
use crate::config::config_f64;
use crate::graph_layout::Marker;
use crate::graph_layout::canonical::CanonicalRoutes;
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, Point, Rect,
    RoutingStyle, Side, Size, SolvedGroup, TitlePosition,
};
use crate::graph_layout::placement;
use crate::layout_frame::GraphPlacement;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{
    ArchitectureDiagramLayout, Bounds, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::resources::PreparedTextRetainedReservation;
use crate::{Error, LayoutExecution, Result};
use kurbo::Insets;
use merman_layout_shoal::PortSideHint;
use merman_model::families::architecture::{
    ArchitectureDiagramModel, ArchitectureLayoutDirection, ArchitectureNodeKind,
};
use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::MarkerStyle;
use rustc_hash::FxHashMap;

pub(crate) use measure::ArchitectureShoalText;
use measure::TEXT_CLEARANCE;

#[derive(Debug)]
pub(crate) struct ArchitectureShoalGeometry {
    pub(crate) routes: CanonicalRoutes,
    pub(crate) groups: Vec<ArchitectureShoalGroup>,
    pub(crate) service_titles: Vec<Option<ArchitectureShoalTitle>>,
    pub(crate) edge_labels: Vec<Option<ArchitectureShoalText>>,
    pub(crate) edge_stroke_outset: f64,
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

fn facing(sides: (Side, Side)) -> Option<usize> {
    match sides {
        (Side::Right, Side::Left) => Some(0),
        (Side::Bottom, Side::Top) => Some(1),
        (Side::Left, Side::Right) => Some(2),
        (Side::Top, Side::Bottom) => Some(3),
        _ => None,
    }
}

const DIRECTIONS: [Direction; 4] = [
    Direction::Right,
    Direction::Down,
    Direction::Left,
    Direction::Up,
];

#[derive(Default)]
struct DirectionVotes {
    ports: [usize; 4],
    declared: [usize; 2],
}

impl DirectionVotes {
    fn choose(&self) -> Option<Direction> {
        (0..DIRECTIONS.len())
            .max_by_key(|&i| {
                (
                    self.declared.get(i).copied().unwrap_or_default(),
                    self.ports[i],
                    std::cmp::Reverse(i),
                )
            })
            .filter(|&i| self.ports[i] > 0 || self.declared.get(i).copied().unwrap_or_default() > 0)
            .map(|i| DIRECTIONS[i])
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

fn containing_group(
    endpoint: Endpoint,
    nodes: &[Option<u32>],
    groups: &[MeasuredGroup],
) -> Option<u32> {
    match endpoint {
        Endpoint::Node(index) => nodes[index as usize],
        Endpoint::Group(index) => groups[index as usize].parent,
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
    let edge_stroke_outset = binding
        .edge_width
        .trim()
        .trim_end_matches("px")
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|width| width.is_finite() && *width >= 0.0)
        .unwrap_or(3.0)
        / 2.0;
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
            title_position: TitlePosition::TopLeft,
        });
        headers.push(header);
    }

    let mut votes: Vec<DirectionVotes> = (0..=g).map(|_| DirectionVotes::default()).collect();
    for hint in &model.layout_hints {
        let axis = usize::from(hint.direction == ArchitectureLayoutDirection::Column);
        let mut previous = None;
        for member in &hint.members {
            work.charge_adapter(work.checked_add(member.len(), 1)?)?;
            let node = node_index(&node_indices, member)?;
            if let Some(before) = previous {
                let parent = parents[node as usize]
                    .filter(|parent| parents[before as usize] == Some(*parent));
                votes[parent.map_or(g, |id| id as usize)].declared[axis] += 1;
            }
            previous = Some(node);
        }
    }

    let mut edges = Vec::with_capacity(m);
    let mut sides = Vec::with_capacity(m);
    let mut edge_labels = Vec::with_capacity(m);
    let mut bottom_degree = vec![0usize; n];
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
        for (end, port) in [(source, pair.0), (target, pair.1)] {
            if let Endpoint::Node(id) = end
                && port == Side::Bottom
            {
                bottom_degree[id as usize] += 1;
            }
        }
        if let Some(vote) = facing(pair)
            && source != target
        {
            let parent = containing_group(source, &parents, &groups)
                .filter(|&parent| Some(parent) == containing_group(target, &parents, &groups));
            votes[parent.map_or(g, |id| id as usize)].ports[vote] += 1;
        }
        sides.push(pair);
        let text = ArchitectureShoalText::centered(
            edge.title.as_deref(),
            wrap,
            measurer,
            &style,
            &mut work,
        )?;
        let label = text.as_ref().map(|text| text.edge_space(label_clearance));
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
        // A junction's transparent SVG box positions it. Its visible connections meet at its center.
        let profile = if junction {
            ShapePortProfile::rectangle(0.0, 0.0)
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
        let half = icon / 2.0;
        let mut w = half;
        let mut h = half;
        let title = title.map(|text| {
            let mut x = 0.0;
            if bottom_degree[index] > 0 {
                let intervals = bottom_degree[index].saturating_sub(1) as f64;
                let pitch = (arrow + TEXT_CLEARANCE).min(span / intervals.max(1.0));
                bottom_pitch[index] = Some(pitch);
                x = (pitch * intervals / 2.0 + arrow / 2.0 + TEXT_CLEARANCE - text.bounds.min_x)
                    .max(0.0);
            }
            let anchor = Point { x, y: half };
            let ink = text.bounds_at(anchor);
            w = w.max(ink.min_x.abs().max(ink.max_x.abs()) + TEXT_CLEARANCE);
            h = h.max(ink.min_y.abs().max(ink.max_y.abs()) + TEXT_CLEARANCE);
            ArchitectureShoalTitle { anchor, text }
        });
        titles.push(title);
        profiles.push(profile);
        nodes.push(MeasuredNode {
            size: Size {
                width: 2.0 * w,
                height: 2.0 * h,
            },
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
                    return PortSideHint::FixedOffset { side, offset: 0.0 };
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
    for (group, hints) in groups.iter_mut().zip(&votes) {
        group.direction = hints.choose();
    }
    let graph = MeasuredGraph {
        direction: votes[g].choose().unwrap_or(Direction::Right),
        routing: RoutingStyle::Orthogonal,
        node_spacing,
        rank_spacing,
        component_spacing: node_spacing,
        inherit_crossed_group_direction: false,
        nodes,
        groups,
        edges,
    };
    let (mut solved, mut placement) = placement::solve(
        &graph,
        model.nodes.iter().map(|node| node.id.as_str()),
        model.groups.iter().map(|group| group.id.as_str()),
        Some(execution),
        placement::Constraints {
            edge_ports: &ports,
            ..Default::default()
        },
        &mut work,
    )?;
    align::apply(
        model,
        &node_indices,
        &graph,
        &mut solved,
        &mut placement,
        &mut work,
    )?;
    for (edge, label) in solved.edges.iter_mut().zip(&edge_labels) {
        if let Some(label) = label {
            label.place_edge(edge, label_clearance, &mut work)?;
        }
    }

    let mut layout = ArchitectureDiagramLayout {
        nodes: Vec::with_capacity(n + g),
        edges: Vec::with_capacity(m),
        cytoscape_service_bounds: Vec::new(),
        fcose_compound_bounds: Vec::new(),
        bounds: None,
    };
    let mut retained = 0usize;
    for ((node, &rect), title) in model.nodes.iter().zip(&solved.nodes).zip(&mut titles) {
        work.charge_adapter(node.id.len())?;
        let center = rect.center();
        if let Some(title) = title {
            title.anchor.x += center.x;
            title.anchor.y += center.y;
            retained = work.checked_add(retained, title.text.checked_retained_bytes(&work)?)?;
        }
        layout.nodes.push(LayoutNode {
            id: node.id.clone(),
            x: center.x - icon / 2.0,
            y: center.y - icon / 2.0,
            width: icon,
            height: icon,
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
        extend_bounds(
            &mut layout.bounds,
            Bounds::from_points(points.iter().map(|p| (p.x, p.y)))
                .ok_or_else(|| invalid(format!("Architecture route edge-{i} has no points")))?,
        );
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
            label: solved_edge.label.clone(),
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
    Ok((
        layout,
        ArchitectureShoalGeometry {
            routes,
            groups,
            service_titles: titles,
            edge_labels,
            edge_stroke_outset,
            _label_storage: work.reserve_prepared_text_retained_bytes(retained)?,
        },
    ))
}
