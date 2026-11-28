use super::{
    ClassDiagramModel, ClassPreparedLabels, ClassRenderConfig, class_namespace_label,
    measured::measure_for_shoal,
};
use crate::graph_layout::canonical::{CanonicalRoutes, CaptionClearance};
use crate::graph_layout::ir::Endpoint;
use crate::graph_layout::placement;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{
    Bounds, ClassDiagramLayout, ClassRenderItem, ClassRenderRoot, ClassRenderRootId,
    ClassRenderTree, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::{LayoutExecution, Result};
use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::EdgeStyle;

mod ports;
mod terminals;

pub(crate) fn layout(
    model: &ClassDiagramModel,
    config: &PresentationConfig,
    execution: &LayoutExecution<'_>,
    render: &ClassRenderConfig,
    work: &mut OperationLayoutWorkControl,
) -> Result<(ClassDiagramLayout, ClassPreparedLabels, CanonicalRoutes)> {
    let settings = render.layout_settings();
    let theme = execution.native_family_theme();
    let (measured, labels) = measure_for_shoal(
        model,
        config,
        execution.text_measurer(),
        execution.math_renderer(),
        render,
        theme.map(|theme| theme.form.edge_style),
        work,
    )?;
    let graph = &measured.graph;
    let solve = |edge_ports, work: &mut OperationLayoutWorkControl| {
        placement::solve(
            graph,
            measured.node_ids.iter().copied(),
            measured.group_ids.iter().copied(),
            Some(execution),
            placement::Constraints {
                edge_ports,
                ..Default::default()
            },
            work,
        )
    };
    let (mut solved, mut placement) = solve(&[], work)?;
    let edge_ports = ports::parallel_ports(graph, &measured.profiles, &solved, work)?;
    if !edge_ports.is_empty() {
        (solved, placement) = solve(&edge_ports, work)?;
    }
    let rough = crate::config::config_diagram_look(config).is_hand_drawn()
        || theme.is_some_and(|theme| theme.form.edge_style == EdgeStyle::Sketch);
    let shaft_outset = theme.map_or(0.5, |theme| theme.form.connector_width / 2.0)
        + if rough { 0.75 } else { 0.0 };
    terminals::seat(graph, &mut solved, shaft_outset, work)?;
    let mut routes = CanonicalRoutes::from_solved(
        graph,
        &solved,
        &measured.profiles,
        |_| 0.0,
        measured.edge_ids.iter().map(|id| id.as_ref()),
        CaptionClearance::Avoid,
        work,
    )?;
    routes.placement = placement;
    work.charge_adapter(work.checked_add(
        work.checked_add(graph.nodes.len(), graph.groups.len())?,
        work.checked_add(graph.edges.len(), solved.groups.len())?,
    )?)?;
    let nodes: Vec<_> = measured
        .node_ids
        .iter()
        .copied()
        .zip(solved.nodes)
        .map(|(id, rect)| {
            let center = rect.center();
            let label = labels.node_label_metrics_by_id.get(id);
            LayoutNode {
                id: id.to_owned(),
                x: center.x,
                y: center.y,
                width: rect.width,
                height: rect.height,
                is_cluster: false,
                label_width: label.map(|metrics| metrics.0),
                label_height: label.map(|metrics| metrics.1),
            }
        })
        .collect();
    let clusters: Vec<_> = measured
        .group_ids
        .iter()
        .copied()
        .zip(solved.groups)
        .map(|(id, group)| {
            let center = group.rect.center();
            let title_label = group.title.unwrap_or(LayoutLabel {
                x: center.x,
                y: group.rect.y,
                width: 0.0,
                height: 0.0,
            });
            LayoutCluster {
                id: id.to_owned(),
                x: center.x,
                y: center.y,
                width: group.rect.width,
                height: group.rect.height,
                diff: 0.0,
                offset_y: 0.0,
                title: class_namespace_label(model, id).to_owned(),
                title_margin_top: title_label.y - title_label.height / 2.0 - group.rect.y,
                title_margin_bottom: settings.title_margin_bottom,
                title_label,
                effective_dir: measured.direction_name.clone(),
                requested_dir: None,
                padding: settings.namespace_padding,
            }
        })
        .collect();
    let mut edges = Vec::with_capacity(graph.edges.len());
    for ((id, relation), edge) in measured.edge_ids.iter().zip(&graph.edges).zip(solved.edges) {
        work.charge_adapter(edge.points.len())?;
        let from = measured.endpoint_id(relation.source);
        let to = measured.endpoint_id(relation.target);
        edges.push(LayoutEdge {
            id: id.to_string(),
            from: from.to_owned(),
            to: to.to_owned(),
            from_cluster: matches!(relation.source, Endpoint::Group(_)).then(|| from.to_owned()),
            to_cluster: matches!(relation.target, Endpoint::Group(_)).then(|| to.to_owned()),
            points: edge
                .points
                .into_iter()
                .map(|p| LayoutPoint { x: p.x, y: p.y })
                .collect(),
            label: edge.label,
            start_label_left: None,
            start_label_right: edge.start_label,
            end_label_left: edge.end_label,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
    }
    let render_tree = ClassRenderTree {
        roots: vec![ClassRenderRoot {
            namespace_id: None,
            cluster_ids: clusters.iter().map(|group| group.id.clone()).collect(),
            edge_ids: edges.iter().map(|edge| edge.id.clone()).collect(),
            items: nodes
                .iter()
                .map(|node| ClassRenderItem::Node(node.id.clone()))
                .collect(),
        }],
        top: ClassRenderRootId(0),
    };
    let bounds = (!nodes.is_empty() || !clusters.is_empty()).then_some(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: solved.bounds.width,
        max_y: solved.bounds.height,
    });
    Ok((
        ClassDiagramLayout {
            nodes,
            edges,
            clusters,
            bounds,
            uses_elk_adapter_dom: false,
            class_label_plans_by_id: labels.class_label_plans_by_id,
            render_tree,
        },
        labels.prepared_labels,
        routes,
    ))
}
