//! ER database order and measured rectangular geometry for Mermaid Layered.

use std::fmt;
use std::sync::Arc;

use merman_model::{
    OperationPhase,
    families::er::{ErDiagramModel, ErEntity, ErSubgraph},
};
use merman_render_core::{
    Bounds, Error, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, Result,
    graph_layout::{
        Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode,
        RoutingStyle, Size, SolvedEdge, SolvedGraph, TitlePosition, direction_from_rank,
        mermaid_layered::{self, LayeredError, Options},
    },
    resources::{OperationWorkError, OperationWorkMeter},
    svg_edge_parity::layered_route::Curve,
    text::TextMeasurer,
};
use rustc_hash::FxHashMap;
use serde_json::Value;

use super::{
    ErLayoutSettings, ErPreparedLabels, er_marker_id, geometry::compute_layout_bounds, labels,
};

const GROUP_PADDING: f64 = 20.0;
const TITLE_GAP: f64 = 12.0;
const PAINT_PADDING: f64 = 8.0;

#[derive(Debug)]
pub enum ErLayeredError {
    Provider(LayeredError),
    DuplicateNodeId(String),
    InvalidNesting(String),
    UnroutedRelationship(usize),
    Host(Error),
}

impl ErLayeredError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Provider(error) => error.is_recoverable(),
            Self::DuplicateNodeId(_) | Self::InvalidNesting(_) | Self::UnroutedRelationship(_) => {
                true
            }
            Self::Host(_) => false,
        }
    }
}

impl fmt::Display for ErLayeredError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(error) => error.fmt(f),
            Self::DuplicateNodeId(id) => write!(f, "duplicate ER node id '{id}'"),
            Self::InvalidNesting(id) => write!(f, "ER subgraph '{id}' is nested inside itself"),
            Self::UnroutedRelationship(index) => {
                write!(f, "ER relationship {index} has no projected endpoint")
            }
            Self::Host(error) => error.fmt(f),
        }
    }
}

impl From<Error> for ErLayeredError {
    fn from(error: Error) -> Self {
        Self::Host(error)
    }
}

impl From<OperationWorkError> for ErLayeredError {
    fn from(error: OperationWorkError) -> Self {
        Self::Host(error.into())
    }
}

/// Labels and source slots owned by a single Layered attempt.
pub struct ErLayeredLabels<'a> {
    labels: Arc<ErPreparedLabels>,
    group_slots: FxHashMap<&'a str, u32>,
    titles: Vec<Size>,
    leaves: Vec<(&'a str, &'a ErEntity)>,
}

impl<'a> ErLayeredLabels<'a> {
    pub fn prepare(
        model: &'a ErDiagramModel,
        config: &Value,
        measurer: &dyn TextMeasurer,
        settings: &ErLayoutSettings,
        work: &Arc<OperationWorkMeter>,
    ) -> Result<Self> {
        let mut group_slots = table(model.subgraphs.len(), work)?;
        let mut titles = buffer(model.subgraphs.len(), work)?;
        for (index, group) in model.subgraphs.iter().rev().enumerate() {
            work.charge(group.id.len().max(1))?;
            group_slots.insert(group.id.as_str(), slot(index, work)?);
            work.charge(group.title.len().max(1))?;
            let title = labels::er_layered_title_size(group, config, measurer, settings);
            work.checkpoint(OperationPhase::Layout)?;
            titles.push(title);
        }
        let mut leaves = buffer(model.entities.len(), work)?;
        for (name, entity) in &model.entities {
            work.charge(name.len().max(1))?;
            if !group_slots.contains_key(name.as_str()) {
                leaves.push((name.as_str(), entity));
            }
        }
        let labels = ErPreparedLabels::prepare_for_layered(
            model,
            config,
            measurer,
            settings,
            leaves.iter().map(|&(_, entity)| entity),
            work,
        )?;
        Ok(Self {
            labels: Arc::new(labels),
            group_slots,
            titles,
            leaves,
        })
    }

    pub fn into_labels(self) -> Arc<ErPreparedLabels> {
        self.labels
    }
}

#[derive(Debug)]
pub struct ErLayeredLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
    pub paint: ErLayeredPaint,
}

/// The successful attempt's rectangular routes, or its successful Dagre retry marker.
#[derive(Debug)]
pub struct ErLayeredPaint {
    routes: Option<ErLayeredRoutes>,
}

impl ErLayeredPaint {
    pub const fn dagre_fallback() -> Self {
        Self { routes: None }
    }

    pub(super) const fn routes(&self) -> Option<&ErLayeredRoutes> {
        self.routes.as_ref()
    }
}

#[derive(Debug)]
pub(super) struct ErLayeredRoutes {
    // The compact route slots match the source-ordered LayoutEdge slice; IDs carry the original
    // relationship occurrence even if a relationship has no projected endpoint.
    pub(super) edges: Vec<SolvedEdge>,
    pub(super) curve: Curve,
    pub(super) label_y_offset: f64,
}

pub fn layout(
    model: &ErDiagramModel,
    config: &Value,
    settings: &ErLayoutSettings,
    labels: &ErLayeredLabels<'_>,
    work: &OperationWorkMeter,
) -> std::result::Result<ErLayeredLayout, ErLayeredError> {
    let input = Input::new(model, config, settings, labels, work)?;
    input.solve(work)
}

struct Input<'a> {
    model: &'a ErDiagramModel,
    graph: MeasuredGraph,
    leaves: Vec<&'a ErEntity>,
    groups: Vec<(&'a ErSubgraph, Size)>,
    // Source occurrence and measured dimensions with no solver padding.
    relationships: Vec<(usize, Option<Size>)>,
    first_unrouted: Option<usize>,
    curve: Curve,
    margins: [f64; 2],
}

impl<'a> Input<'a> {
    fn new(
        model: &'a ErDiagramModel,
        config: &Value,
        settings: &ErLayoutSettings,
        labels: &ErLayeredLabels<'a>,
        work: &OperationWorkMeter,
    ) -> std::result::Result<Self, ErLayeredError> {
        let group_count = model.subgraphs.len();
        let node_count = labels.leaves.len();
        let edge_count = model.relationships.len();
        let total = group_count
            .checked_add(node_count)
            .ok_or_else(|| work.arithmetic_overflow())?;
        let mut endpoints = table(total, work)?;
        let mut parents = FxHashMap::default();
        for group in model.subgraphs.iter().rev() {
            for member in &group.nodes {
                work.charge(member.len().max(1))?;
                parents.try_reserve(1).map_err(|_| allocation_error())?;
                parents.insert(member.as_str(), group.id.as_str());
            }
        }
        let parent = |id: &str| {
            parents
                .get(id)
                .and_then(|parent| labels.group_slots.get(parent))
                .copied()
        };
        let (routing, curve) = routing(config);
        let margins = [title_margin(config, "top"), title_margin(config, "bottom")];
        let mut graph = MeasuredGraph {
            direction: settings.direction,
            routing,
            node_spacing: spacing(config, "nodeSpacing", 140.0),
            rank_spacing: spacing(config, "rankSpacing", 80.0),
            component_spacing: 0.0,
            inherit_crossed_group_direction: true,
            nodes: buffer(node_count, work)?,
            groups: buffer(group_count, work)?,
            edges: buffer(edge_count, work)?,
        };
        let mut groups = buffer(group_count, work)?;
        let mut leaves = buffer(node_count, work)?;
        let mut relationships = buffer(edge_count, work)?;
        let mut first_unrouted = None;
        // ErDB.getData inserts groups in reverse DB order, then leaves by entity map order.
        for (index, (group, &size)) in model.subgraphs.iter().rev().zip(&labels.titles).enumerate()
        {
            let index = slot(index, work)?;
            insert_endpoint(&mut endpoints, &group.id, Endpoint::Group(index), work)?;
            let title = (size.width > 0.0 && size.height > 0.0).then_some(size);
            let top = if title.is_some() {
                positive(margins[0]) + positive(margins[1]) + TITLE_GAP
            } else {
                GROUP_PADDING
            };
            graph.groups.push(MeasuredGroup {
                seq: index,
                parent: parent(&group.id),
                direction: group.dir.as_deref().and_then(group_direction),
                padding: kurbo::Insets::new(GROUP_PADDING, top, GROUP_PADDING, GROUP_PADDING),
                min_size: Size::default(),
                title,
                title_position: TitlePosition::TopCenter,
            });
            groups.push((group, size));
        }
        for &(name, entity) in &labels.leaves {
            work.charge(name.len().max(1))?;
            let index = slot(leaves.len(), work)?;
            insert_endpoint(&mut endpoints, &entity.id, Endpoint::Node(index), work)?;
            let measured = labels
                .labels
                .entity(&entity.id)
                .ok_or_else(|| invalid_projection("missing entity measurement"))?;
            graph.nodes.push(MeasuredNode::new(
                Size {
                    width: positive(measured.width),
                    height: positive(measured.height),
                },
                index,
                parent(name),
            ));
            leaves.push(entity);
        }
        for (index, relationship) in model.relationships.iter().enumerate() {
            for id in [&relationship.entity_a, &relationship.entity_b] {
                work.charge(id.len().max(1))?;
            }
            let (Some(&source), Some(&target)) = (
                endpoints.get(relationship.entity_a.as_str()),
                endpoints.get(relationship.entity_b.as_str()),
            ) else {
                first_unrouted.get_or_insert(index);
                continue;
            };
            work.charge(relationship.role_a.len().max(1))?;
            let (width, height) = labels
                .labels
                .relationship(index)
                .ok_or_else(|| invalid_projection("missing relationship measurement"))?
                .dimensions();
            let label = edge_label_size(&relationship.role_a, width, height);
            graph.edges.push(MeasuredEdge {
                source,
                target,
                min_rank_span: 1,
                weight: 1.0,
                label: label.map(|size| Size {
                    width: size.width + 12.0,
                    height: size.height + 4.0,
                }),
                start_label: None,
                end_label: None,
                start_marker: Default::default(),
                end_marker: Default::default(),
            });
            relationships.push((index, label));
        }
        Ok(Self {
            model,
            graph,
            leaves,
            groups,
            relationships,
            first_unrouted,
            curve,
            margins,
        })
    }

    fn solve(
        self,
        work: &OperationWorkMeter,
    ) -> std::result::Result<ErLayeredLayout, ErLayeredError> {
        if !(self.margins[0] + self.margins[1]).is_finite() {
            return Err(ErLayeredError::Provider(LayeredError::NonFiniteGeometry));
        }
        let group_order = self.group_paint_order(work)?;
        let solved = {
            #[cfg(feature = "benchmark-stages")]
            let _stage = merman_model::benchmark_stages::enter(
                merman_model::benchmark_stages::Stage::LayoutSolve,
            );
            mermaid_layered::layout(&self.graph, Options::default(), work)
        }
        .map_err(|error| match error {
            LayeredError::Work(error) => error.into(),
            LayeredError::ArithmeticOverflow => work.arithmetic_overflow().into(),
            LayeredError::AllocationFailed => allocation_error().into(),
            error => ErLayeredError::Provider(error),
        })?;
        // The upstream solver ignores unbound ends. The common painter still encounters that
        // occurrence and retries. Reject before writing to the SVG sink.
        if let Some(index) = self.first_unrouted {
            return Err(ErLayeredError::UnroutedRelationship(index));
        }
        Ok(self.project(solved.graph, group_order, work)?)
    }

    fn group_paint_order(
        &self,
        work: &OperationWorkMeter,
    ) -> std::result::Result<Vec<usize>, ErLayeredError> {
        let groups = &self.graph.groups;
        let count = groups.len();
        let mut depths = buffer(count, work)?;
        depths.resize(count, None);
        let mut visiting = buffer(count, work)?;
        // Cache resolved depths so a long nesting chain only needs one parent walk.
        for start in 0..count {
            let mut next = Some(start);
            let mut depth = 0usize;
            while let Some(index) = next {
                work.charge(1)?;
                match depths[index] {
                    Some(usize::MAX) => {
                        return Err(ErLayeredError::InvalidNesting(owned(
                            &self.groups[index].0.id,
                            work,
                        )?));
                    }
                    Some(resolved_depth) => {
                        depth = resolved_depth + 1;
                        break;
                    }
                    None => {
                        depths[index] = Some(usize::MAX);
                        visiting.push(index);
                        next = groups[index].parent.map(|parent| parent as usize);
                    }
                }
            }
            while let Some(index) = visiting.pop() {
                work.charge(1)?;
                depths[index] = Some(depth);
                depth += 1;
            }
        }
        let mut order = buffer(count, work)?;
        order.extend(0..count);
        let comparisons = (usize::BITS - count.saturating_sub(1).leading_zeros()) as usize;
        work.charge(
            count
                .checked_mul(comparisons)
                .ok_or_else(|| work.arithmetic_overflow())?,
        )?;
        order.sort_unstable_by_key(|&index| (depths[index], index));
        Ok(order)
    }

    fn project(
        self,
        solved: SolvedGraph,
        group_order: Vec<usize>,
        work: &OperationWorkMeter,
    ) -> Result<ErLayeredLayout> {
        if solved.nodes.len() != self.leaves.len()
            || solved.groups.len() != self.groups.len()
            || solved.edges.len() != self.relationships.len()
        {
            return Err(invalid_projection(
                "solver geometry does not match measured slots",
            ));
        }
        let count = solved
            .nodes
            .len()
            .checked_add(solved.groups.len())
            .ok_or_else(|| work.arithmetic_overflow())?;
        let mut nodes = buffer(count, work)?;
        let mut clusters = buffer(solved.groups.len(), work)?;
        let mut edges = buffer(solved.edges.len(), work)?;
        let [top, bottom] = self.margins;
        for index in group_order {
            let (group, title) = self.groups[index];
            let placed = &solved.groups[index];
            let rect = placed.rect;
            let center = rect.center();
            nodes.push(layout_node(&group.id, rect, true, work)?);
            clusters.push(LayoutCluster {
                id: owned(&group.id, work)?,
                x: center.x,
                y: center.y,
                width: rect.width,
                height: rect.height,
                diff: -PAINT_PADDING,
                offset_y: title.height - PAINT_PADDING / 2.0,
                title: owned(&group.title, work)?,
                title_label: LayoutLabel {
                    x: center.x,
                    y: rect.y + top + title.height / 2.0,
                    width: title.width,
                    height: title.height,
                },
                requested_dir: group
                    .dir
                    .as_deref()
                    .map(|dir| owned(dir, work))
                    .transpose()?,
                effective_dir: rank(placed.direction).into(),
                padding: PAINT_PADDING,
                title_margin_top: top,
                title_margin_bottom: bottom,
            });
        }
        for (entity, rect) in self.leaves.iter().zip(solved.nodes) {
            nodes.push(layout_node(&entity.id, rect, false, work)?);
        }
        let label_y_offset = (top + bottom) / 2.0;
        for (occurrence, ((index, size), route)) in self
            .relationships
            .into_iter()
            .zip(&solved.edges)
            .enumerate()
        {
            let relation = &self.model.relationships[index];
            let measured_edge = &self.graph.edges[occurrence];
            let mut points = buffer(route.points.len(), work)?;
            points.extend(route.points.iter().map(|point| LayoutPoint {
                x: point.x,
                y: point.y,
            }));
            work.charge(points.len())?; // Bounds scan.
            let label = match (size, &route.label) {
                (Some(size), Some(anchor)) => Some(LayoutLabel {
                    x: anchor.x,
                    y: anchor.y + label_y_offset,
                    width: size.width,
                    height: size.height,
                }),
                (None, None) => None,
                _ => {
                    return Err(invalid_projection(
                        "route label does not match its measurement",
                    ));
                }
            };
            edges.push(LayoutEdge {
                id: format!("er-rel-{index}"),
                from: owned(&relation.entity_a, work)?,
                to: owned(&relation.entity_b, work)?,
                from_cluster: matches!(measured_edge.source, Endpoint::Group(_))
                    .then(|| owned(&relation.entity_a, work))
                    .transpose()?,
                to_cluster: matches!(measured_edge.target, Endpoint::Group(_))
                    .then(|| owned(&relation.entity_b, work))
                    .transpose()?,
                points,
                label,
                start_label_left: None,
                start_label_right: None,
                end_label_left: None,
                end_label_right: None,
                start_marker: er_marker_id(&relation.rel_spec.card_b, "START"),
                end_marker: er_marker_id(&relation.rel_spec.card_a, "END"),
                stroke_dasharray: (relation.rel_spec.rel_type != "IDENTIFYING")
                    .then(|| "8,8".to_owned()),
            });
        }
        work.charge(nodes.len())?;
        work.charge(edges.len())?;
        work.charge(clusters.len())?;
        let bounds = compute_layout_bounds(&clusters, &nodes, &edges);
        work.checkpoint(OperationPhase::Layout)?;
        Ok(ErLayeredLayout {
            nodes,
            edges,
            clusters,
            bounds,
            paint: ErLayeredPaint {
                routes: Some(ErLayeredRoutes {
                    edges: solved.edges,
                    curve: self.curve,
                    label_y_offset,
                }),
            },
        })
    }
}

fn layout_node(
    id: &str,
    rect: merman_render_core::graph_layout::Rect,
    is_cluster: bool,
    work: &OperationWorkMeter,
) -> Result<LayoutNode> {
    let center = rect.center();
    Ok(LayoutNode {
        id: owned(id, work)?,
        x: center.x,
        y: center.y,
        width: rect.width,
        height: rect.height,
        is_cluster,
        label_width: None,
        label_height: None,
    })
}

fn insert_endpoint<'a>(
    endpoints: &mut FxHashMap<&'a str, Endpoint>,
    id: &'a str,
    endpoint: Endpoint,
    work: &OperationWorkMeter,
) -> std::result::Result<(), ErLayeredError> {
    work.charge(id.len().max(1))?;
    if endpoints.insert(id, endpoint).is_some() {
        return Err(ErLayeredError::DuplicateNodeId(owned(id, work)?));
    }
    Ok(())
}

fn group_direction(value: &str) -> Option<Direction> {
    if ["TB", "TD", "BT", "LR", "RL"]
        .iter()
        .any(|direction| value.eq_ignore_ascii_case(direction))
    {
        Some(direction_from_rank(value))
    } else {
        None
    }
}

fn rank(direction: Direction) -> &'static str {
    match direction {
        Direction::Down => "TB",
        Direction::Up => "BT",
        Direction::Right => "LR",
        Direction::Left => "RL",
    }
}

fn routing(config: &Value) -> (RoutingStyle, Curve) {
    match config
        .pointer("/layered/edgeRouting")
        .and_then(Value::as_str)
    {
        Some("curved") => (RoutingStyle::Curved, Curve::Curved),
        Some("stepped") => (RoutingStyle::Orthogonal, Curve::Step),
        Some("straight") => (RoutingStyle::Polyline, Curve::Linear),
        _ => (RoutingStyle::Orthogonal, Curve::Rounded),
    }
}

fn spacing(config: &Value, key: &str, er_default: f64) -> f64 {
    // The ER renderer overwrites Flowchart spacing with `er[key] || erDefault`.
    // Layered then accepts only finite positive numbers and otherwise uses 50.
    match config.get("er").and_then(|er| er.get(key)) {
        None | Some(Value::Null | Value::Bool(false)) => er_default,
        Some(Value::Number(value)) if value.as_f64() == Some(0.0) => er_default,
        Some(Value::String(value)) if value.is_empty() => er_default,
        Some(value) => value
            .as_f64()
            .filter(|value| *value > 0.0 && value.is_finite())
            .unwrap_or(50.0),
    }
}

fn title_margin(config: &Value, key: &str) -> f64 {
    config
        .pointer("/flowchart/subGraphTitleMargin")
        .and_then(|margins| margins.get(key))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

fn edge_label_size(source: &str, width: f64, height: f64) -> Option<Size> {
    let width = positive(width);
    let has_text = !source
        .trim_matches(merman_model::text::is_ecmascript_whitespace)
        .is_empty();
    (has_text && width > 0.0).then(|| Size {
        width,
        height: positive(height),
    })
}

pub(super) fn positive(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

fn slot(index: usize, work: &OperationWorkMeter) -> Result<u32> {
    u32::try_from(index).map_err(|_| work.arithmetic_overflow().into())
}

fn table<'a, T>(count: usize, work: &OperationWorkMeter) -> Result<FxHashMap<&'a str, T>> {
    work.charge(count)?;
    let mut map = FxHashMap::default();
    map.try_reserve(count).map_err(|_| allocation_error())?;
    Ok(map)
}

fn buffer<T>(count: usize, work: &OperationWorkMeter) -> Result<Vec<T>> {
    work.charge(count)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| allocation_error())?;
    Ok(output)
}

fn owned(text: &str, work: &OperationWorkMeter) -> Result<String> {
    work.charge(text.len().max(1))?;
    let mut copy = String::new();
    copy.try_reserve_exact(text.len())
        .map_err(|_| allocation_error())?;
    copy.push_str(text);
    Ok(copy)
}

pub(super) fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "layered" }
}

pub(super) fn invalid_projection(detail: &str) -> Error {
    Error::InvalidModel {
        message: format!("Invalid ER Layered projection: {detail}"),
    }
}

#[cfg(test)]
mod tests;
