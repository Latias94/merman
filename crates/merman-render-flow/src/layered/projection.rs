use merman_render_core::graph_layout::{Endpoint, Rect, SolvedGraph};
use merman_render_core::{Bounds, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};

use super::input::{FlowLayeredInput, rank};
use super::{
    FlowLayeredError, FlowLayeredLayout, FlowLayeredRoutes, FlowLayeredRouting,
    FlowchartEdgeOwners, HashMap, LayeredError, OperationWorkMeter, allocation_error, owned,
    reserve,
};
use crate::graph::flowchart_checked_sort_work;
use crate::measured::MeasuredLabel;

impl FlowLayeredInput<'_> {
    pub(super) fn project(
        self,
        solved: SolvedGraph,
        routing: FlowLayeredRouting,
        cluster_padding: f64,
        work: &OperationWorkMeter,
    ) -> Result<FlowLayeredLayout, FlowLayeredError> {
        if solved.nodes.len() != self.leaves.len()
            || solved.groups.len() != self.groups.len()
            || solved.edges.len() != self.edges.len()
        {
            return Err(incompatible("geometry count does not match measured slots"));
        }

        let node_count = self
            .leaves
            .len()
            .checked_add(self.groups.len())
            .ok_or_else(|| work.arithmetic_overflow())?;
        let mut nodes = reserve(node_count, work)?;
        let mut clusters = reserve(self.groups.len(), work)?;
        let mut group_parents = reserve(self.groups.len(), work)?;
        let mut paint_groups = reserve(self.groups.len(), work)?;
        let mut order = reserve(node_count, work)?;
        let mut edges = reserve(self.edges.len(), work)?;
        let mut edge_keys = reserve(self.edges.len(), work)?;

        for (leaf, rect) in self.leaves.iter().zip(solved.nodes) {
            nodes.push(layout_node(leaf.id, rect, &leaf.label, false, work)?);
        }

        // Common paints the group shells in depth order, with the input order settling ties.
        for (index, (measured, group)) in self.groups.iter().zip(solved.groups).enumerate() {
            work.charge(1)?;
            let shape = group.rect;
            let center = shape.center();
            let label = &measured.label;
            let id = measured.group.id.as_str();
            let [top, bottom] = self.margins;
            clusters.push(LayoutCluster {
                id: owned(id, work)?,
                x: center.x,
                y: center.y,
                width: shape.width,
                height: shape.height,
                diff: -cluster_padding,
                offset_y: label.height - cluster_padding / 2.0,
                title: owned(&measured.group.title, work)?,
                title_label: LayoutLabel {
                    x: center.x,
                    y: shape.y + top + label.height / 2.0,
                    width: label.width,
                    height: label.height,
                },
                requested_dir: measured
                    .group
                    .dir
                    .as_deref()
                    .map(|dir| owned(dir, work))
                    .transpose()?,
                effective_dir: owned(rank(group.direction), work)?,
                padding: cluster_padding,
                title_margin_top: top,
                title_margin_bottom: bottom,
            });
            nodes.push(layout_node(id, shape, label, true, work)?);
            group_parents.push(group.parent);
            paint_groups.push((measured.depth, index));
        }
        work.charge(flowchart_checked_sort_work(paint_groups.len(), work)?)?;
        paint_groups.sort_unstable();
        for (_, index) in paint_groups {
            order.push(owned(&self.groups[index].group.id, work)?);
        }
        for leaf in self.leaves {
            order.push(owned(leaf.id, work)?);
        }

        // `index.ts` defers geometry for some group-member edges. `applyLayoutModel` and
        // the common SVG renderer still visit the original LayoutData occurrences in order.
        for (measured, route) in self.edges.iter().zip(&solved.edges) {
            let edge = &measured.edge;
            let mut points = reserve(route.points.len(), work)?;
            for point in &route.points {
                work.charge(1)?;
                points.push(LayoutPoint {
                    x: point.x,
                    y: point.y,
                });
            }
            let label = match (&measured.label, &route.label) {
                (Some(size), Some(anchor)) => Some(LayoutLabel {
                    x: anchor.x,
                    y: anchor.y,
                    width: size.width,
                    height: size.height,
                }),
                (None, None) => None,
                _ => {
                    return Err(incompatible(
                        "edge label center does not match measured label",
                    ));
                }
            };
            let owner = &self.graph.edges[edges.len()];
            edges.push(LayoutEdge {
                id: owned(&edge.semantic.id, work)?,
                from: owned(edge.from, work)?,
                to: owned(edge.to, work)?,
                from_cluster: matches!(owner.source, Endpoint::Group(_))
                    .then(|| owned(edge.from, work))
                    .transpose()?,
                to_cluster: matches!(owner.target, Endpoint::Group(_))
                    .then(|| owned(edge.to, work))
                    .transpose()?,
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
            edge_keys.push(measured.key);
        }

        let bounds = occupied_bounds(&nodes, &edges, work)?;
        let mut dom_node_order_by_root = HashMap::new();
        dom_node_order_by_root
            .try_reserve(1)
            .map_err(|_| allocation_error())?;
        dom_node_order_by_root.insert(String::new(), order);
        Ok(FlowLayeredLayout {
            nodes,
            edges,
            edge_owners: FlowchartEdgeOwners::new(edge_keys),
            clusters,
            bounds,
            dom_node_order_by_root,
            routes: FlowLayeredRoutes {
                routing,
                group_parents,
                edges: solved.edges,
                border_stitches: solved.border_stitches,
            },
        })
    }
}

fn layout_node(
    id: &str,
    rect: Rect,
    label: &MeasuredLabel,
    is_cluster: bool,
    work: &OperationWorkMeter,
) -> Result<LayoutNode, FlowLayeredError> {
    let center = rect.center();
    Ok(LayoutNode {
        id: owned(id, work)?,
        x: center.x,
        y: center.y,
        width: rect.width,
        height: rect.height,
        is_cluster,
        label_width: Some(label.width),
        label_height: Some(label.height),
    })
}

fn occupied_bounds(
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    work: &OperationWorkMeter,
) -> Result<Option<Bounds>, FlowLayeredError> {
    let corners = |x, y, w: f64, h: f64| [(x - w / 2.0, y - h / 2.0), (x + w / 2.0, y + h / 2.0)];
    let mut bounds = None::<Bounds>;
    let mut extend = |(x, y)| {
        let bounds = bounds.get_or_insert(Bounds {
            min_x: x,
            min_y: y,
            max_x: x,
            max_y: y,
        });
        bounds.min_x = bounds.min_x.min(x);
        bounds.min_y = bounds.min_y.min(y);
        bounds.max_x = bounds.max_x.max(x);
        bounds.max_y = bounds.max_y.max(y);
    };
    for node in nodes {
        work.charge(1)?;
        corners(node.x, node.y, node.width, node.height)
            .into_iter()
            .for_each(&mut extend);
    }
    for edge in edges {
        work.charge(1)?;
        for point in &edge.points {
            work.charge(1)?;
            extend((point.x, point.y));
        }
        if let Some(label) = &edge.label {
            corners(label.x, label.y, label.width, label.height)
                .into_iter()
                .for_each(&mut extend);
        }
    }
    Ok(bounds)
}

fn incompatible(reason: &'static str) -> FlowLayeredError {
    FlowLayeredError::Provider(LayeredError::InconsistentLevel(reason))
}
