//! RequirementDB projection onto Mermaid's measured Grid layout.

use super::{
    RequirementDiagramLayout, RequirementEdgeIdentity, RequirementEdgeLabelPlan,
    RequirementEdgeLabels, RequirementNodeRenderPlan, RequirementPreparedArtifact,
    bounds_for_nodes_edges, config::RequirementLayoutSettings, measure_requirement,
    missing_relationship_node, requirement_layout_node_from_center,
};
use crate::label_rect::LabelRect;
use merman_model::{MermaidConfig, OperationPhase, families::requirement::RequirementDiagramModel};
use merman_render_core::{
    Error, LayoutLabel, LayoutPoint, Result,
    config::json_f64,
    graph_layout::{
        Size,
        mermaid_grid::{
            GridEdgeLabel, GridItem, GridItemKind, GridMeasuredEdge, GridMeasuredItem, GridPaint,
            prepare_grid,
        },
    },
    resources::{OperationWorkMeter, PreparedTextRetainedReservation},
    text::TextMeasurer,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug)]
pub(super) struct RequirementGridPaint {
    pub(super) settings: GridPaint,
    pub(super) labels: Vec<RequirementGridLabel>,
    _storage: PreparedTextRetainedReservation,
}

#[derive(Debug)]
pub(super) struct RequirementGridLabel {
    pub(super) terminal: LabelRect,
    _retained: PreparedTextRetainedReservation,
}

pub fn layout(
    model: &RequirementDiagramModel,
    config: &MermaidConfig,
    settings: &RequirementLayoutSettings,
    measurer: &dyn TextMeasurer,
    work: &Arc<OperationWorkMeter>,
) -> Result<RequirementPreparedArtifact> {
    work.checkpoint(OperationPhase::Layout)?;
    let measured = measure_requirement(model, config, measurer, settings, work)?;
    work.charge(measured.nodes.len())?;
    let mut items = buffer(measured.nodes.len())?;
    for (ordinal, (&id, node)) in measured.nodes.iter().enumerate() {
        work.charge(id.len().max(1))?;
        items.push(GridMeasuredItem {
            item: GridItem {
                id,
                authored_id: None,
                source_ordinal: Some(ordinal),
                inline: Default::default(),
            },
            parent_id: None,
            kind: GridItemKind::Leaf(Size {
                width: node.width,
                height: node.height,
            }),
        });
    }

    let edge_count = model.relationships.len();
    work.charge(edge_count)?;
    let label_storage = edge_count
        .checked_mul(std::mem::size_of::<RequirementGridLabel>())
        .ok_or_else(|| work.arithmetic_overflow())?;
    let storage = work.reserve_prepared_text_retained_bytes(label_storage)?;
    let mut relations = buffer(edge_count)?;
    let mut labels = buffer(edge_count)?;
    let wrapping_width = config
        .get_path(&["flowchart", "wrappingWidth"])
        .and_then(json_f64)
        .unwrap_or(120.0);
    for (index, relationship) in model.relationships.iter().enumerate() {
        work.charge(relationship.src.len())?;
        work.charge(relationship.dst.len())?;
        let from = measured
            .nodes
            .get_index_of(relationship.src.as_str())
            .ok_or_else(|| missing_relationship_node("src", &relationship.src))?;
        let to = measured
            .nodes
            .get_index_of(relationship.dst.as_str())
            .ok_or_else(|| missing_relationship_node("dst", &relationship.dst))?;
        work.charge(relationship.rel_type.len().max(1))?;
        let plan = RequirementEdgeLabelPlan::for_relationship(
            relationship,
            index,
            format!("&lt;&lt;{}&gt;&gt;", relationship.rel_type),
        );
        let terminal = LabelRect::prepare(
            &plan.display_text,
            measurer,
            &measured.styles.html_regular,
            settings.html_labels,
            config,
            wrapping_width,
            true,
        );
        work.checkpoint(OperationPhase::Layout)?;
        labels.push(RequirementGridLabel {
            _retained: work
                .reserve_prepared_text_retained_bytes(terminal.retained_buffer_bytes())?,
            terminal,
        });
        relations.push((from, to, plan));
    }
    let mut measured_edges = buffer(edge_count)?;
    for ((from, to, plan), label) in relations.iter().zip(&labels) {
        work.charge(plan.rendered_id.len().max(1))?;
        let metrics = label.terminal.metrics();
        measured_edges.push(GridMeasuredEdge {
            id: &plan.rendered_id,
            source: *from,
            target: *to,
            label: Some(GridEdgeLabel {
                size: Size {
                    width: metrics.width.max(0.0),
                    height: metrics.height.max(0.0),
                },
                start_marker: plan.marker_start,
                end_marker: plan.marker_end,
            }),
        });
    }

    let grid = prepare_grid(&items, &measured_edges, config.get("grid"), work)?;
    let routes = grid.route_edges(work)?;
    let settings = grid.paint();
    let mut nodes = buffer(items.len())?;
    let mut node_plans = HashMap::new();
    node_plans
        .try_reserve(items.len())
        .map_err(|_| allocation_error())?;
    for (slot, (id, measured)) in measured.nodes.into_iter().enumerate() {
        work.charge(id.len().max(1))?;
        let Some(rect) = grid.item_bounds(slot) else {
            return Err(Error::InvalidModel {
                message: format!("missing Requirement Grid bounds for `{id}`"),
            });
        };
        let center = rect.center();
        nodes.push(requirement_layout_node_from_center(
            id.to_owned(),
            center.x,
            center.y,
            measured.width,
            measured.height,
        ));
        node_plans.insert(
            id.to_owned(),
            RequirementNodeRenderPlan::Semantic(measured.label),
        );
    }
    drop(grid);
    if routes.len() != edge_count {
        return Err(Error::InvalidModel {
            message: "Requirement Grid route count differs from its source".to_owned(),
        });
    }
    let mut edges = buffer(edge_count)?;
    let mut edge_plans = RequirementEdgeLabels::new();
    edge_plans
        .try_reserve(edge_count)
        .map_err(|_| allocation_error())?;
    for (((source, target, plan), route), label) in relations.into_iter().zip(routes).zip(&labels) {
        let id = RequirementEdgeIdentity {
            from: items[source].item.id,
            to: items[target].item.id,
            name: plan.rendered_id.as_str(),
        };
        work.charge(route.points.len())?;
        let mut points = buffer(route.points.len())?;
        points.extend(route.points.into_iter().map(|point| LayoutPoint {
            x: point.x,
            y: point.y,
        }));
        let center = route.label_center.ok_or_else(|| Error::InvalidModel {
            message: format!("missing Requirement Grid label center for `{}`", id.name),
        })?;
        let metrics = label.terminal.metrics();
        edges.push(id.layout_edge(
            points,
            Some(LayoutLabel {
                x: center.x,
                y: center.y,
                width: metrics.width.max(0.0),
                height: metrics.height.max(0.0),
            }),
        ));
        edge_plans.insert(id.into_owned(), plan);
    }
    let bounds = bounds_for_nodes_edges(&nodes, &edges);
    work.checkpoint(OperationPhase::Layout)?;
    Ok(RequirementPreparedArtifact {
        layout: RequirementDiagramLayout {
            nodes,
            edges,
            bounds,
        },
        render_layout: None,
        nodes: node_plans,
        edges: edge_plans,
        measurement_binding: measured.binding,
        grid: Some(RequirementGridPaint {
            settings,
            labels,
            _storage: storage,
        }),
        #[cfg(feature = "layout-layered")]
        layered: None,
    })
}

fn buffer<T>(len: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| allocation_error())?;
    Ok(values)
}

fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}
