//! Project solved Grid positions into Flow's owned layout DTOs.

use std::collections::HashMap;

use merman_render_core::graph_layout::mermaid_grid::{GridPaint, GridRoutedEdge, PreparedGrid};
use merman_render_core::{Bounds, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};

use super::{
    Error, FlowGridInput, FlowchartEdgeKey, FlowchartRenderModelRef, OperationWorkMeter, Result,
    allocation_error,
};
use crate::layout_config::FlowchartLayoutSettings;

pub struct FlowGridLayout {
    pub nodes: Vec<LayoutNode>,
    pub clusters: Vec<LayoutCluster>,
    pub edges: Vec<LayoutEdge>,
    pub edge_keys: Vec<FlowchartEdgeKey>,
    pub bounds: Option<Bounds>,
    pub dom_node_order_by_root: HashMap<String, Vec<String>>,
    pub paint: GridPaint,
}

impl FlowGridInput<'_> {
    /// Binds geometry to original paint occurrences, including redirected or duplicate raw IDs.
    pub(super) fn project_layout(
        &self,
        view: FlowchartRenderModelRef<'_>,
        prepared: &PreparedGrid<'_>,
        routes: Vec<GridRoutedEdge>,
        settings: &FlowchartLayoutSettings,
        work: &OperationWorkMeter,
    ) -> Result<FlowGridLayout> {
        work.charge(0)?;
        if routes.len() != self.edges.len() {
            return Err(invalid_projection("route count does not match edge owners"));
        }
        work.charge(self.items.len())?;
        work.charge(routes.len())?;
        let mut nodes = Vec::new();
        let mut clusters = Vec::new();
        let mut order = Vec::new();
        let mut edges = Vec::new();
        reserve(&mut nodes, self.items.len())?;
        reserve(&mut order, self.items.len())?;
        reserve(&mut edges, routes.len())?;

        for (slot, (item, paint)) in self.items.iter().zip(&self.item_paint).enumerate() {
            let id = item.item.id;
            work.charge(id.len().max(1))?;
            let rect = prepared
                .item_bounds(slot)
                .ok_or_else(|| invalid_projection("missing measured item geometry"))?;
            let center = rect.center();
            let is_cluster = paint.group.is_some();
            nodes.push(LayoutNode {
                id: id.to_owned(),
                x: center.x,
                y: center.y,
                width: rect.width,
                height: rect.height,
                is_cluster,
                label_width: Some(paint.label.width),
                label_height: Some(paint.label.height),
            });
            order.push(id.to_owned());
            if let Some(group) = paint.group {
                work.charge(group.title.len().max(1))?;
                clusters.try_reserve(1).map_err(|_| allocation_error())?;
                let padding = settings.cluster_padding;
                let title = &paint.label;
                let paint_width = rect.width.max(title.width + padding);
                clusters.push(LayoutCluster {
                    id: id.to_owned(),
                    x: center.x,
                    y: center.y,
                    width: rect.width,
                    height: rect.height,
                    diff: (paint_width - rect.width) / 2.0 - padding,
                    offset_y: title.height - padding / 2.0,
                    title: group.title.clone(),
                    title_label: LayoutLabel {
                        x: center.x,
                        y: rect.y + settings.title_margin_top + title.height / 2.0,
                        width: title.width,
                        height: title.height,
                    },
                    requested_dir: group.dir.clone(),
                    effective_dir: paint.direction.to_owned(),
                    padding,
                    title_margin_top: settings.title_margin_top,
                    title_margin_bottom: settings.title_margin_bottom,
                });
            }
        }

        for ((edge, key), route) in self.edges.iter().zip(&self.edge_keys).zip(routes) {
            let semantic = &view.semantic.edges[key.semantic_index()];
            let source = &nodes[edge.source];
            let target = &nodes[edge.target];
            work.charge(semantic.id.len().max(1))?;
            work.charge(source.id.len().max(1))?;
            work.charge(target.id.len().max(1))?;
            // Both materializing and bounding the points run before the layout can be published.
            work.charge(route.points.len())?;
            work.charge(route.points.len())?;
            let mut points = Vec::new();
            reserve(&mut points, route.points.len())?;
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
                _ => return Err(invalid_projection("label geometry does not match its edge")),
            };
            edges.push(LayoutEdge {
                id: semantic.id.clone(),
                from: source.id.clone(),
                to: target.id.clone(),
                from_cluster: source.is_cluster.then(|| source.id.clone()),
                to_cluster: target.is_cluster.then(|| target.id.clone()),
                points,
                label,
                start_label_left: None,
                start_label_right: None,
                end_label_left: None,
                end_label_right: None,
                start_marker: None,
                end_marker: None,
                stroke_dasharray: None,
            });
        }

        let bounds = bounds(&nodes, &edges);
        let mut edge_keys = Vec::new();
        reserve(&mut edge_keys, self.edge_keys.len())?;
        edge_keys.extend_from_slice(&self.edge_keys);
        work.charge(0)?;
        Ok(FlowGridLayout {
            nodes,
            clusters,
            edges,
            edge_keys,
            bounds,
            dom_node_order_by_root: HashMap::from([(String::new(), order)]),
            paint: prepared.paint(),
        })
    }
}

fn bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let corners = |x, y, width, height| {
        let half_width = width / 2.0;
        let half_height = height / 2.0;
        [
            (x - half_width, y - half_height),
            (x + half_width, y + half_height),
        ]
    };
    let node_corners = nodes
        .iter()
        .flat_map(|node| corners(node.x, node.y, node.width, node.height));
    let edge_points = edges.iter().flat_map(|edge| {
        edge.points.iter().map(|point| (point.x, point.y)).chain(
            edge.label
                .iter()
                .flat_map(|label| corners(label.x, label.y, label.width, label.height)),
        )
    });
    Bounds::from_points(node_corners.chain(edge_points))
}

fn reserve<T>(buffer: &mut Vec<T>, count: usize) -> Result<()> {
    buffer
        .try_reserve_exact(count)
        .map_err(|_| allocation_error())
}

fn invalid_projection(message: &str) -> Error {
    Error::InvalidModel {
        message: format!("Invalid Grid layout projection: {message}"),
    }
}
