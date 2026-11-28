//! Project ClassDB's ordered, measured graph into the shared Grid provider.

mod label;

pub(super) use label::{ClassGridLabel, ClassGridLabelBody, prepare_label_rect};

use std::borrow::Cow;

use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    Error, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, Result,
    graph_layout::{
        Size,
        mermaid_grid::{
            GridEdgeLabel, GridGroupMeasure, GridItem, GridItemKind, GridMeasuredEdge,
            GridMeasuredItem, GridPaint, prepare_grid,
        },
    },
    layout_work::OperationLayoutWorkControl,
    math::MathRenderer,
    resources::OperationWorkMeter,
    text::TextMeasurer,
};
use rustc_hash::FxHashMap;

use super::measured::{MeasureInterfaceLabel, MeasurementTarget, RelationTerminals, measure_into};
use super::{
    ClassDiagramLayout, ClassDiagramModel, ClassPreparedLabels, ClassRenderConfig, ClassRenderItem,
    ClassRenderRoot, ClassRenderRootId, ClassRenderTree, EdgeTerminalMetrics,
    apply_class_terminal_labels, class_arrow_type_for_relation_end, class_namespace_label,
    compute_bounds, normalize_dir, write_class_relation_dom_id,
};

#[allow(
    clippy::too_many_arguments,
    reason = "Class Grid shares the operation's existing measurement callback and work control."
)]
pub fn layout(
    model: &ClassDiagramModel,
    config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    render: &ClassRenderConfig,
    measure_interface_label: MeasureInterfaceLabel,
    work_control: &OperationLayoutWorkControl,
    work: &OperationWorkMeter,
) -> Result<(ClassDiagramLayout, ClassPreparedLabels, GridPaint)> {
    let namespace_count = model.namespaces().len();
    let item_count = work_control.checked_add(namespace_count, model.classes().len())?;
    let item_count = work_control.checked_add(item_count, model.notes().len())?;
    let item_count = work_control.checked_add(item_count, model.interfaces().len())?;
    let edge_count = work_control.checked_add(model.notes().len(), model.relations().len())?;
    work.charge(work_control.checked_add(item_count, edge_count)?)?;
    let mut input = GridMeasurement {
        items: buffer(item_count)?,
        edges: buffer(edge_count)?,
        padding: render.layout_settings().namespace_padding,
        relation_count: 0,
        work,
        work_control,
    };
    let labels = measure_into(
        model,
        config,
        measurer,
        math_renderer,
        render,
        false,
        measure_interface_label,
        Some(work_control),
        &mut input,
    )?;
    work_control.checkpoint()?;

    let mut slots = FxHashMap::default();
    slots
        .try_reserve(item_count)
        .map_err(|_| allocation_error())?;
    for (slot, item) in input.items.iter().enumerate() {
        work.charge(item.item.id.len().max(1))?;
        slots.insert(item.item.id, slot);
    }
    let mut edges = buffer(input.edges.len())?;
    for edge in &input.edges {
        work.charge(edge.from.len().max(1))?;
        work.charge(edge.to.len().max(1))?;
        let (Some(&source), Some(&target)) = (slots.get(edge.from), slots.get(edge.to)) else {
            return Err(Error::InvalidModel {
                message: format!(
                    "GRID_MISSING_ENDPOINT: Missing endpoint for edge \"{}\"",
                    edge.dom_id
                ),
            });
        };
        edges.push(GridMeasuredEdge {
            id: &edge.dom_id,
            source,
            target,
            label: edge.label,
        });
    }
    let prepared = prepare_grid(&input.items, &edges, config.as_value().get("grid"), work)?;
    let routes = prepared.route_edges(work)?;
    if routes.len() != edges.len() {
        return Err(invalid_projection("route count differs from edge count"));
    }

    let settings = render.layout_settings();
    let direction = normalize_dir(&model.model().direction);
    let mut nodes = buffer(input.items.len())?;
    let mut clusters = buffer(namespace_count)?;
    let mut root = ClassRenderRoot {
        cluster_ids: buffer(namespace_count)?,
        edge_ids: buffer(routes.len())?,
        items: buffer(input.items.len().saturating_sub(namespace_count))?,
        ..Default::default()
    };
    for (slot, item) in input.items.iter().enumerate() {
        let id = item.item.id;
        work.charge(id.len().max(1))?;
        let bounds = prepared
            .item_bounds(slot)
            .ok_or_else(|| invalid_projection("missing item geometry"))?;
        let center = bounds.center();
        let is_cluster = matches!(item.kind, GridItemKind::Group(_));
        let metrics = labels.node_label_metrics_by_id.get(id);
        nodes.push(LayoutNode {
            id: id.to_owned(),
            x: center.x,
            y: center.y,
            width: bounds.width,
            height: bounds.height,
            is_cluster,
            label_width: metrics.map(|m| m.0),
            label_height: metrics.map(|m| m.1),
        });
        if let GridItemKind::Group(measured) = item.kind {
            let title = class_namespace_label(model, id);
            work.charge(title.len().max(1))?;
            let size = measured.title.unwrap_or_default();
            clusters.push(LayoutCluster {
                id: id.to_owned(),
                x: center.x,
                y: center.y,
                width: bounds.width,
                height: bounds.height,
                diff: -settings.namespace_padding,
                offset_y: size.height - settings.namespace_padding / 2.0,
                title: title.to_owned(),
                title_label: LayoutLabel {
                    x: center.x,
                    y: bounds.y + settings.title_margin_top + size.height / 2.0,
                    width: size.width,
                    height: size.height,
                },
                requested_dir: None,
                effective_dir: direction.clone(),
                padding: settings.namespace_padding,
                title_margin_top: settings.title_margin_top,
                title_margin_bottom: settings.title_margin_bottom,
            });
            root.cluster_ids.push(id.to_owned());
        } else {
            root.items.push(ClassRenderItem::Node(id.to_owned()));
        }
    }

    let mut projected_edges = buffer(routes.len())?;
    for ((edge, source), route) in edges.iter().zip(&input.edges).zip(routes) {
        work.charge(source.id.len().max(1))?;
        // One visit to project the points and another for the final bounds scan.
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
                    "label geometry differs from its measured relation",
                ));
            }
        };
        root.edge_ids.push(source.id.to_string());
        let mut projected = LayoutEdge {
            id: source.id.to_string(),
            from: source.from.to_owned(),
            to: source.to.to_owned(),
            from_cluster: nodes[edge.source]
                .is_cluster
                .then(|| source.from.to_owned()),
            to_cluster: nodes[edge.target].is_cluster.then(|| source.to.to_owned()),
            points: Vec::new(),
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        };
        if let Some(terminals) = &source.terminals
            && (terminals.start_right.is_some() || terminals.end_left.is_some())
        {
            // The common renderer positions Class cardinalities on the untrimmed route.
            // ClassDB supplies a truthy arrow name even for a missing marker (`none`).
            work.charge(points.len())?;
            work.charge(points.len())?;
            apply_class_terminal_labels(&mut projected, terminals, &points)?;
        }
        projected.points = points;
        projected_edges.push(projected);
    }
    let bounds = compute_bounds(&nodes, &projected_edges, &clusters);
    work_control.checkpoint()?;
    Ok((
        ClassDiagramLayout {
            nodes,
            edges: projected_edges,
            clusters,
            bounds,
            uses_elk_adapter_dom: false,
            class_label_plans_by_id: labels.class_label_plans_by_id,
            render_tree: ClassRenderTree {
                roots: vec![root],
                top: ClassRenderRootId(0),
            },
        },
        labels.prepared_labels,
        prepared.paint(),
    ))
}

struct GridMeasurement<'a, 'work> {
    items: Vec<GridMeasuredItem<'a>>,
    edges: Vec<MeasuredRelation<'a>>,
    padding: f64,
    relation_count: usize,
    work: &'work OperationWorkMeter,
    work_control: &'work OperationLayoutWorkControl,
}

struct MeasuredRelation<'a> {
    id: Cow<'a, str>,
    dom_id: String,
    from: &'a str,
    to: &'a str,
    label: Option<GridEdgeLabel>,
    terminals: Option<EdgeTerminalMetrics>,
}

impl<'a> GridMeasurement<'a, '_> {
    fn push_item(
        &mut self,
        id: &'a str,
        parent_id: Option<&'a str>,
        kind: GridItemKind,
    ) -> Result<()> {
        self.work.charge(id.len().max(1))?;
        self.items.push(GridMeasuredItem {
            // ClassDB uses the same semantic id for the Grid key and connectivity.
            item: GridItem {
                id,
                authored_id: None,
                source_ordinal: Some(self.items.len()),
                inline: Default::default(),
            },
            parent_id,
            kind,
        });
        Ok(())
    }
}

impl<'a> MeasurementTarget<'a> for GridMeasurement<'a, '_> {
    fn grid_label_work(&self) -> Option<(&OperationWorkMeter, &OperationLayoutWorkControl)> {
        Some((self.work, self.work_control))
    }

    fn namespace(
        &mut self,
        id: &'a str,
        parent: Option<&'a str>,
        (width, height): (f64, f64),
    ) -> Result<()> {
        self.push_item(
            id,
            parent,
            GridItemKind::Group(GridGroupMeasure {
                padding: self.padding,
                title: (width > 0.0 && height > 0.0).then_some(Size { width, height }),
            }),
        )
    }

    fn node(
        &mut self,
        id: &'a str,
        parent: Option<&'a str>,
        (width, height): (f64, f64),
    ) -> Result<()> {
        self.push_item(id, parent, GridItemKind::Leaf(Size { width, height }))
    }

    fn edge(
        &mut self,
        from: &'a str,
        to: &'a str,
        id: Cow<'a, str>,
        (width, height): (f64, f64),
        terminals: Option<RelationTerminals<'a>>,
    ) -> Result<()> {
        self.work.charge(from.len())?;
        self.work.charge(to.len())?;
        self.work.charge(id.len().max(1))?;
        let mut dom_id = String::new();
        let (terminals, label) = if let Some(terminals) = terminals {
            self.relation_count += 1;
            // Use the registered edge id for the router's deterministic sibling-port ties.
            write_class_relation_dom_id(&mut dom_id, from, to, self.relation_count);
            let start = class_arrow_type_for_relation_end(terminals.relation.relation.type1);
            let end = class_arrow_type_for_relation_end(terminals.relation.relation.type2);
            let label = (width > 0.0 && height > 0.0).then_some(GridEdgeLabel {
                size: Size { width, height },
                start_marker: start.is_some(),
                end_marker: end.is_some(),
            });
            let metrics = EdgeTerminalMetrics {
                start_left: None,
                start_right: terminals.start_right,
                end_left: terminals.end_left,
                end_right: None,
                start_marker: 10.0,
                end_marker: 10.0,
                #[cfg(feature = "layout-elk")]
                start_arrow_type: start,
                #[cfg(feature = "layout-elk")]
                end_arrow_type: end,
            };
            (Some(metrics), label)
        } else {
            dom_id.push_str(&id);
            (None, None)
        };
        self.edges.push(MeasuredRelation {
            id,
            dom_id,
            from,
            to,
            label,
            terminals,
        });
        Ok(())
    }
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
        message: format!("Invalid Class Grid projection: {detail}"),
    }
}
