//! The measured Usecase DB projection for Mermaid Layered.

use super::{
    UsecaseDiagramLayout, UsecaseEdgePlan, UsecaseMeasured, UsecasePrepared, layout, measure,
    resolve_css,
};
use merman_model::{
    OperationPhase, families::usecase::UsecaseDiagramModel, presentation::PresentationConfig,
};
use merman_render_core::{
    Bounds, Error, LayoutLabel, LayoutPoint, Result,
    graph_layout::{
        Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, Size, SolvedEdge,
        TitlePosition, direction_from_rank,
        mermaid_layered::{self, EdgeRouting, LayeredError, Options},
    },
    layout_work::OperationLayoutWorkControl,
    math::MathRenderer,
    resources::{OperationWorkError, OperationWorkMeter},
    svg_edge_parity::layered_route::Curve,
    text::{TextMeasurer, is_ecmascript_whitespace},
};
use merman_theme_contract::v2::{Color, FamilyTheme};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::Arc,
};

const GROUP_PADDING: f64 = 20.0;
const TITLE_GAP: f64 = 12.0;

#[derive(Debug)]
pub enum UsecaseLayeredError {
    Provider(LayeredError),
    DuplicateEdgeId(String),
    Host(Error),
}

impl UsecaseLayeredError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Provider(error) => error.is_recoverable(),
            Self::DuplicateEdgeId(_) => true,
            Self::Host(_) => false,
        }
    }
}

impl fmt::Display for UsecaseLayeredError {
    fn fmt(&self, sink: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(error) => error.fmt(sink),
            Self::DuplicateEdgeId(id) => write!(sink, "layout: duplicate edge id '{id}'"),
            Self::Host(error) => error.fmt(sink),
        }
    }
}

impl From<Error> for UsecaseLayeredError {
    fn from(error: Error) -> Self {
        Self::Host(error)
    }
}

impl From<OperationWorkError> for UsecaseLayeredError {
    fn from(error: OperationWorkError) -> Self {
        Self::Host(error.into())
    }
}

#[derive(Debug)]
pub(super) enum Paint {
    Axes(Routes),
    #[cfg(feature = "layout-dagre")]
    DagreFallback,
}

#[derive(Debug)]
pub(super) struct Routes {
    /// Slots follow DB edges, including the note connectors added after the relationships.
    pub(super) edges: Vec<SolvedEdge>,
    pub(super) curve: Curve,
    pub(super) label_y_offset: f64,
}

pub fn prepare(
    model: &UsecaseDiagramModel,
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    canvas: Option<Color>,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    work: &Arc<OperationWorkMeter>,
) -> std::result::Result<UsecasePrepared, UsecaseLayeredError> {
    work.checkpoint(OperationPhase::Layout)?;
    let css = resolve_css(config, native, canvas);
    let mut control = OperationLayoutWorkControl::new(Arc::clone(work));
    let (nodes, edges) =
        measure::measure_layered(model, config, &css, measurer, math_renderer, &mut control)?;
    let mut prepared =
        prepare_measured(model, config, UsecaseMeasured { css, nodes, edges }, work)?;
    measure::prepare_layered_boundary_labels(
        model,
        config,
        &prepared.measured.css,
        measurer,
        math_renderer,
        &mut prepared.measured.nodes,
        &mut control,
    )?;
    Ok(prepared)
}

fn prepare_measured(
    model: &UsecaseDiagramModel,
    config: &PresentationConfig,
    measured: UsecaseMeasured,
    work: &OperationWorkMeter,
) -> std::result::Result<UsecasePrepared, UsecaseLayeredError> {
    let (graph, routing, label_y_offset) = measured_graph(model, config, &measured, work)?;
    let solved = {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::LayoutSolve,
        );
        mermaid_layered::layout(&graph, Options::default(), work)
    }
    .map_err(|error| match error {
        LayeredError::Work(error) => error.into(),
        LayeredError::ArithmeticOverflow => work.arithmetic_overflow().into(),
        LayeredError::AllocationFailed => allocation_error().into(),
        error => UsecaseLayeredError::Provider(error),
    })?
    .graph;
    if solved.nodes.len() != graph.nodes.len()
        || solved.groups.len() != graph.groups.len()
        || solved.edges.len() != measured.edges.len()
    {
        return Err(invalid_projection("solver slots differ from measured slots").into());
    }

    work.charge(measured.nodes.len())?;
    let mut nodes = buffer(measured.nodes.len())?;
    let mut leaf_slots = solved.nodes.into_iter();
    let mut group_slots = solved.groups.into_iter();
    for plan in &measured.nodes {
        work.charge(plan.id.len().max(1))?;
        let rect = if plan.is_boundary {
            group_slots.next().map(|group| group.rect)
        } else {
            leaf_slots.next()
        }
        .ok_or_else(|| invalid_projection("no box for measured node"))?;
        let center = rect.center();
        nodes.push(layout::node(
            plan,
            center.x,
            center.y,
            rect.width,
            rect.height,
        ));
    }

    work.charge(measured.edges.len())?;
    let mut edges = buffer(measured.edges.len())?;
    for (plan, route) in measured.edges.iter().zip(&solved.edges) {
        work.charge(plan.id.len().max(1))?;
        work.charge(route.points.len())?;
        let mut points = buffer(route.points.len())?;
        points.extend(route.points.iter().map(|p| LayoutPoint { x: p.x, y: p.y }));
        let label = match (&plan.label, &route.label) {
            (Some(label), Some(anchor)) => Some(LayoutLabel {
                x: anchor.x,
                y: anchor.y,
                width: label.metrics.width,
                height: label.metrics.height,
            }),
            (_, None) => None,
            (None, Some(_)) => {
                return Err(invalid_projection("unmeasured relationship label").into());
            }
        };
        edges.push(layout::edge(plan, points, label));
    }
    work.charge(nodes.len())?;
    work.charge(edges.len())?;
    let bounds = Bounds::from_points(
        nodes
            .iter()
            .flat_map(|node| {
                [
                    (node.x - node.width / 2.0, node.y - node.height / 2.0),
                    (node.x + node.width / 2.0, node.y + node.height / 2.0),
                ]
            })
            .chain(
                edges
                    .iter()
                    .flat_map(|edge| edge.points.iter().map(|p| (p.x, p.y))),
            )
            .chain(
                edges
                    .iter()
                    .filter_map(|edge| edge.label.as_ref())
                    .flat_map(|label| {
                        [
                            (label.x - label.width / 2.0, label.y - label.height / 2.0),
                            (label.x + label.width / 2.0, label.y + label.height / 2.0),
                        ]
                    }),
            ),
    );

    let authored_curve = config
        .get_path(&["flowchart", "curve"])
        .and_then(Value::as_str)
        .unwrap_or("basis");
    let curve = measured.css.native_curve().map_or_else(
        || Curve::for_edge_routing(routing, authored_curve),
        Curve::from_mermaid,
    );
    work.checkpoint(OperationPhase::Layout)?;
    let mut prepared = UsecasePrepared::new(
        measured,
        UsecaseDiagramLayout {
            nodes,
            edges,
            bounds,
        },
        HashMap::new(),
        false,
    );
    prepared.layered = Some(Paint::Axes(Routes {
        edges: solved.edges,
        curve,
        label_y_offset,
    }));
    Ok(prepared)
}

fn measured_graph(
    model: &UsecaseDiagramModel,
    config: &PresentationConfig,
    measured: &UsecaseMeasured,
    work: &OperationWorkMeter,
) -> std::result::Result<(MeasuredGraph, EdgeRouting, f64), UsecaseLayeredError> {
    work.charge(measured.nodes.len())?;
    let mut endpoints = HashMap::new();
    endpoints
        .try_reserve(measured.nodes.len())
        .map_err(|_| allocation_error())?;
    let (mut node_count, mut group_count) = (0, 0);
    for plan in &measured.nodes {
        work.charge(plan.id.len().max(1))?;
        let endpoint = if plan.is_boundary {
            let endpoint = Endpoint::Group(slot(group_count, work)?);
            group_count += 1;
            endpoint
        } else {
            let endpoint = Endpoint::Node(slot(node_count, work)?);
            node_count += 1;
            endpoint
        };
        endpoints.insert(plan.id.as_str(), endpoint);
    }
    let margin = |side| {
        config
            .get_path(&["flowchart", "subGraphTitleMargin", side])
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .unwrap_or(0.0)
    };
    let (margin_top, margin_bottom) = (margin("top"), margin("bottom"));
    let routing = EdgeRouting::from_name(
        config
            .get_path(&["layered", "edgeRouting"])
            .and_then(Value::as_str),
    );
    let mut graph = MeasuredGraph {
        direction: direction_from_rank(&model.direction),
        routing: routing.geometry(),
        node_spacing: spacing(config, "nodeSpacing"),
        rank_spacing: spacing(config, "rankSpacing"),
        component_spacing: 0.0,
        inherit_crossed_group_direction: true,
        nodes: buffer(node_count)?,
        groups: buffer(group_count)?,
        edges: buffer(measured.edges.len())?,
    };
    work.charge(measured.nodes.len())?;
    for plan in &measured.nodes {
        if plan.is_boundary {
            let width = positive(plan.label.metrics.width);
            let height = positive(plan.label.metrics.height);
            let title = (!plan.label.text.is_empty() && width > 0.0 && height > 0.0)
                .then_some(Size { width, height });
            let top = if title.is_some() {
                positive(margin_top) + positive(margin_bottom) + TITLE_GAP
            } else {
                GROUP_PADDING
            };
            graph.groups.push(MeasuredGroup {
                seq: slot(graph.groups.len(), work)?,
                parent: None,
                direction: None,
                padding: kurbo::Insets::new(GROUP_PADDING, top, GROUP_PADDING, GROUP_PADDING),
                min_size: Size::default(),
                title,
                title_position: TitlePosition::TopCenter,
            });
        } else {
            let parent = plan
                .parent
                .as_deref()
                .and_then(|id| match endpoints.get(id) {
                    Some(Endpoint::Group(slot)) => Some(*slot),
                    _ => None,
                });
            // The browser marks these ovals spreadPorts for other engines. Layered's shape-span
            // table does not include usecaseEllipse or usecaseBusiness.
            graph.nodes.push(MeasuredNode::new(
                Size {
                    width: positive(plan.width),
                    height: positive(plan.height),
                },
                slot(graph.nodes.len(), work)?,
                parent,
            ));
        }
    }

    work.charge(measured.edges.len())?;
    let mut edge_ids = HashSet::new();
    edge_ids
        .try_reserve(measured.edges.len())
        .map_err(|_| allocation_error())?;
    for plan in &measured.edges {
        work.charge(plan.id.len().max(1))?;
        work.charge(plan.source.len())?;
        work.charge(plan.target.len())?;
        let source = *endpoints
            .get(plan.source.as_str())
            .ok_or_else(|| invalid_projection("missing relationship source"))?;
        let target = *endpoints
            .get(plan.target.as_str())
            .ok_or_else(|| invalid_projection("missing relationship target"))?;
        if !edge_ids.insert(plan.id.as_str()) {
            let mut id = String::new();
            id.try_reserve_exact(plan.id.len())
                .map_err(|_| allocation_error())?;
            id.push_str(&plan.id);
            return Err(UsecaseLayeredError::DuplicateEdgeId(id));
        }
        let label = solver_label_size(plan, work)?;
        graph.edges.push(MeasuredEdge {
            source,
            target,
            min_rank_span: u32::try_from(plan.minlen.max(1)).unwrap_or(u32::MAX),
            weight: 1.0,
            label,
            start_label: None,
            end_label: None,
            start_marker: Default::default(),
            end_marker: Default::default(),
        });
    }
    Ok((graph, routing, (margin_top + margin_bottom) / 2.0))
}

fn solver_label_size(plan: &UsecaseEdgePlan, work: &OperationWorkMeter) -> Result<Option<Size>> {
    let Some(label) = &plan.label else {
        return Ok(None);
    };
    work.charge(label.text.len().max(1))?;
    let width = positive(label.metrics.width);
    Ok(
        (width > 0.0 && !label.text.trim_matches(is_ecmascript_whitespace).is_empty()).then_some(
            Size {
                width: width + 12.0,
                height: positive(label.metrics.height) + 4.0,
            },
        ),
    )
}

fn spacing(config: &PresentationConfig, key: &str) -> f64 {
    let Some(family) = config.get_path(&["usecase", key]) else {
        return 50.0;
    };
    [Some(family), config.get_path(&["flowchart", key])]
        .into_iter()
        .flatten()
        .filter_map(Value::as_f64)
        .find(|&value| positive(value) > 0.0)
        .unwrap_or(50.0)
}

fn positive(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

fn slot(value: usize, work: &OperationWorkMeter) -> Result<u32> {
    u32::try_from(value).map_err(|_| work.arithmetic_overflow().into())
}

pub(super) fn buffer<T>(len: usize) -> Result<Vec<T>> {
    let mut items = Vec::new();
    items
        .try_reserve_exact(len)
        .map_err(|_| allocation_error())?;
    Ok(items)
}

pub(super) fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "layered" }
}

pub(super) fn invalid_projection(detail: &str) -> Error {
    Error::InvalidModel {
        message: format!("Invalid Usecase Layered projection: {detail}"),
    }
}

#[cfg(test)]
mod tests;
