//! Mermaid's ER database projection onto the shared measured Grid provider.

use merman_model::{OperationPhase, families::er::ErDiagramModel};
use merman_render_core::{
    Bounds, Error, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, Result,
    config::config_f64,
    graph_layout::{
        Size,
        mermaid_grid::{
            GridEdgeLabel, GridGroupMeasure, GridItem, GridItemKind, GridMeasuredEdge,
            GridMeasuredItem, GridPaint, PreparedGrid, prepare_grid,
        },
    },
    resources::OperationWorkMeter,
    text::TextMeasurer,
};
use rustc_hash::FxHashMap;
use serde_json::Value;

use super::{
    ErLayoutSettings, ErPreparedLabels, er_layout_bounds, er_marker_id, er_subgraph_title_metrics,
    geometry::er_relationship_dom_id,
};

const GROUP_PADDING: f64 = 8.0;

/// Solved ER geometry and the Grid settings resolved in the same layout operation.
#[derive(Debug)]
pub struct ErGridLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
    pub paint: GridPaint,
}

pub fn layout(
    model: &ErDiagramModel,
    config: &Value,
    measurer: &dyn TextMeasurer,
    settings: &ErLayoutSettings,
    prepared_labels: &ErPreparedLabels,
    work: &OperationWorkMeter,
) -> Result<ErGridLayout> {
    let item_count = model
        .subgraphs
        .len()
        .checked_add(model.entities.len())
        .ok_or_else(|| work.arithmetic_overflow())?;
    work.charge(item_count)?;
    let mut items = buffer(item_count)?;
    let mut slots = FxHashMap::default();
    slots
        .try_reserve(item_count)
        .map_err(|_| allocation_error())?;
    let mut parents = FxHashMap::default();

    // ErDB.getData fills parentDB and emits groups in reverse DB order. Membership is keyed by
    // the authored name, while the Grid forest uses generated entity IDs and original group IDs.
    for group in model.subgraphs.iter().rev() {
        work.charge(group.id.len().max(1))?;
        for member in &group.nodes {
            work.charge(member.len().max(1))?;
            parents.try_reserve(1).map_err(|_| allocation_error())?;
            parents.insert(member.as_str(), group.id.as_str());
        }
    }
    for group in model.subgraphs.iter().rev() {
        work.charge(group.title.len().max(1))?;
        let metrics = er_subgraph_title_metrics(
            group,
            measurer,
            &settings.label_style,
            settings.relationship_html_labels,
        );
        work.checkpoint(OperationPhase::Layout)?;
        let title = (!group.title.is_empty()).then_some(Size {
            width: metrics.width.max(0.0),
            height: metrics.height.max(0.0),
        });
        let slot = items.len();
        slots.insert(group.id.as_str(), slot);
        items.push(GridMeasuredItem {
            item: GridItem {
                id: &group.id,
                authored_id: None,
                source_ordinal: Some(slot),
                inline: Default::default(),
            },
            parent_id: parents.get(group.id.as_str()).copied(),
            kind: GridItemKind::Group(GridGroupMeasure {
                padding: GROUP_PADDING,
                title,
            }),
        });
    }
    for (name, entity) in &model.entities {
        work.charge(name.len().max(1))?;
        if slots
            .get(name.as_str())
            .is_some_and(|&slot| slot < model.subgraphs.len())
        {
            // ErDB omits entities whose source names coincide with subgraph IDs.
            continue;
        }
        work.charge(entity.id.len().max(1))?;
        let measured = prepared_labels
            .entity(&entity.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("ER prepared labels are missing entity {}", entity.id),
            })?;
        let slot = items.len();
        slots.entry(entity.id.as_str()).or_insert(slot);
        items.push(GridMeasuredItem {
            item: GridItem {
                id: &entity.id,
                // Mermaid 12.4 ErDB exposes placementId separately from label, alias, and id.
                authored_id: Some(name),
                source_ordinal: Some(slot),
                inline: Default::default(),
            },
            parent_id: parents.get(name.as_str()).copied(),
            kind: GridItemKind::Leaf(Size {
                width: measured.width,
                height: measured.height,
            }),
        });
    }

    work.charge(model.relationships.len())?;
    let mut ids = buffer(model.relationships.len())?;
    for (index, relationship) in model.relationships.iter().enumerate() {
        work.charge(relationship.entity_a.len())?;
        work.charge(relationship.entity_b.len())?;
        ids.push(er_relationship_dom_id(relationship, index));
    }
    let mut edges = buffer(ids.len())?;
    for (index, (relationship, id)) in model.relationships.iter().zip(&ids).enumerate() {
        work.charge(id.len())?;
        let (Some(&source), Some(&target)) = (
            slots.get(relationship.entity_a.as_str()),
            slots.get(relationship.entity_b.as_str()),
        ) else {
            return Err(Error::InvalidModel {
                message: format!("GRID_MISSING_ENDPOINT: Missing endpoint for edge \"{id}\""),
            });
        };
        let label = if relationship.role_a.is_empty() {
            None
        } else {
            let (width, height) = prepared_labels
                .relationship(index)
                .ok_or_else(|| invalid_projection("missing relationship measurement"))?
                .dimensions();
            (width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0).then(|| {
                GridEdgeLabel {
                    size: Size { width, height },
                    start_marker: super::markers::has_er_marker(&relationship.rel_spec.card_b),
                    end_marker: super::markers::has_er_marker(&relationship.rel_spec.card_a),
                }
            })
        };
        edges.push(GridMeasuredEdge {
            id,
            source,
            target,
            label,
        });
    }
    let prepared = prepare_grid(&items, &edges, config.get("grid"), work)?;
    project(model, &items, &edges, &prepared, config, work)
}

fn project(
    model: &ErDiagramModel,
    items: &[GridMeasuredItem<'_>],
    edges: &[GridMeasuredEdge<'_>],
    prepared: &PreparedGrid<'_>,
    config: &Value,
    work: &OperationWorkMeter,
) -> Result<ErGridLayout> {
    let routes = prepared.route_edges(work)?;
    if routes.len() != edges.len() {
        return Err(invalid_projection(
            "route count differs from relationship count",
        ));
    }
    work.charge(items.len())?;
    work.charge(routes.len())?;
    let mut nodes = buffer(items.len())?;
    let mut clusters = buffer(model.subgraphs.len())?;
    let mut projected_edges = buffer(routes.len())?;
    let mut groups = model.subgraphs.iter().rev();
    let title_top_margin =
        config_f64(config, &["flowchart", "subGraphTitleMargin", "top"]).unwrap_or(0.0);
    let title_bottom_margin =
        config_f64(config, &["flowchart", "subGraphTitleMargin", "bottom"]).unwrap_or(0.0);

    for (slot, item) in items.iter().enumerate() {
        let id = item.item.id;
        work.charge(id.len().max(1))?;
        let bounds = prepared
            .item_bounds(slot)
            .ok_or_else(|| invalid_projection("missing item geometry"))?;
        let center = bounds.center();
        nodes.push(LayoutNode {
            id: id.to_owned(),
            x: center.x,
            y: center.y,
            width: bounds.width,
            height: bounds.height,
            is_cluster: matches!(item.kind, GridItemKind::Group(_)),
            label_width: None,
            label_height: None,
        });
        if let GridItemKind::Group(measured) = item.kind {
            let group = groups
                .next()
                .ok_or_else(|| invalid_projection("missing subgraph owner"))?;
            work.charge(group.title.len().max(1))?;
            let title = measured.title.unwrap_or_default();
            let paint_width = bounds.width.max(title.width + GROUP_PADDING);
            clusters.push(LayoutCluster {
                id: group.id.clone(),
                x: center.x,
                y: center.y,
                width: bounds.width,
                height: bounds.height,
                diff: (paint_width - bounds.width) / 2.0 - GROUP_PADDING,
                offset_y: title.height - GROUP_PADDING / 2.0,
                title: group.title.clone(),
                title_label: LayoutLabel {
                    x: center.x,
                    y: bounds.y + title_top_margin + title.height / 2.0,
                    width: title.width,
                    height: title.height,
                },
                requested_dir: group.dir.clone(),
                effective_dir: group.dir.clone().unwrap_or_else(|| model.direction.clone()),
                padding: GROUP_PADDING,
                title_margin_top: title_top_margin,
                title_margin_bottom: title_bottom_margin,
            });
        }
    }

    for (index, (edge, route)) in edges.iter().zip(routes).enumerate() {
        let relationship = &model.relationships[index];
        work.charge(relationship.entity_a.len())?;
        work.charge(relationship.entity_b.len())?;
        // Account for both materializing the points and the later bounds scan.
        work.charge(route.points.len())?;
        work.charge(route.points.len())?;
        let mut points = buffer(route.points.len())?;
        points.extend(route.points.into_iter().map(|point| LayoutPoint {
            x: point.x,
            y: point.y,
        }));
        let label = match (edge.label, route.label_center) {
            (Some(label), Some(center)) => Some(LayoutLabel {
                x: center.x,
                y: center.y,
                width: label.size.width,
                height: label.size.height,
            }),
            (None, None) => None,
            _ => {
                return Err(invalid_projection(
                    "label geometry does not match its relationship",
                ));
            }
        };
        let source = &nodes[edge.source];
        let target = &nodes[edge.target];
        projected_edges.push(LayoutEdge {
            id: format!("er-rel-{index}"),
            from: relationship.entity_a.clone(),
            to: relationship.entity_b.clone(),
            from_cluster: source.is_cluster.then(|| source.id.clone()),
            to_cluster: target.is_cluster.then(|| target.id.clone()),
            points,
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: er_marker_id(&relationship.rel_spec.card_b, "START"),
            end_marker: er_marker_id(&relationship.rel_spec.card_a, "END"),
            stroke_dasharray: (relationship.rel_spec.rel_type == "NON_IDENTIFYING")
                .then(|| "8,8".to_owned()),
        });
    }
    let bounds = er_layout_bounds(&nodes, &projected_edges);
    work.checkpoint(OperationPhase::Layout)?;
    Ok(ErGridLayout {
        nodes,
        edges: projected_edges,
        clusters,
        bounds,
        paint: prepared.paint(),
    })
}

fn buffer<T>(capacity: usize) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_error())?;
    Ok(output)
}

fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}

fn invalid_projection(detail: &str) -> Error {
    Error::InvalidModel {
        message: format!("Invalid ER Grid projection: {detail}"),
    }
}
