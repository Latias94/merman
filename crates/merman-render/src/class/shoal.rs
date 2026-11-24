use super::{
    ClassDiagramModel, ClassLayoutSettings, ClassPreparedLabels, ClassRenderConfig,
    ClassTextThemePlan, MeasuredClassDiagram, class_namespace_label, measure_class_diagram,
};
use crate::graph_layout::canonical::CanonicalRoutes;
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, RoutingStyle,
    Side, Size, TitlePosition,
};
use crate::graph_layout::shoal_bridge::{self, LayoutScratch};
use crate::layout_work::OperationLayoutWorkControl;
use crate::math::MathRenderer;
use crate::model::{
    Bounds, ClassDiagramLayout, ClassRenderItem, ClassRenderRoot, ClassRenderRootId,
    ClassRenderTree, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use kurbo::Insets;
use merman_model::presentation::PresentationConfig;
use rustc_hash::FxHashMap;

pub(crate) fn layout(
    model: &ClassDiagramModel,
    config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    typography: &ClassTextThemePlan,
    render: &ClassRenderConfig,
    work: &mut OperationLayoutWorkControl,
) -> Result<(ClassDiagramLayout, ClassPreparedLabels)> {
    let settings = render.layout_settings();
    let mut measured = measure_class_diagram(
        model,
        config,
        measurer,
        math_renderer,
        render,
        false,
        typography,
        Some(work),
    )?;
    let (graph, profiles) =
        canonical_input(model, &measured, settings, render.look == "neo", work)?;
    let solved = shoal_bridge::solve(
        &graph,
        Default::default(),
        &mut LayoutScratch::default(),
        work,
    )?;
    let source = &measured.graph;
    measured.prepared_labels.canonical_routes = Some(CanonicalRoutes::from_solved(
        &graph,
        &solved,
        &profiles,
        |_| 0.0,
        source.edges.keys().map(|(_, _, id)| id.as_str()),
        work,
    )?);
    work.charge_adapter(work.checked_add(
        source.nodes.len(),
        work.checked_add(source.edges.len(), solved.groups.len())?,
    )?)?;
    let nodes: Vec<_> = source
        .nodes
        .iter()
        .filter(|(_, node)| !node.is_namespace)
        .zip(solved.nodes)
        .map(|((id, _), rect)| {
            let center = rect.center();
            let label = measured.node_label_metrics_by_id.get(id);
            LayoutNode {
                id: id.clone(),
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
    let clusters: Vec<_> = source
        .nodes
        .iter()
        .filter(|(_, node)| node.is_namespace)
        .zip(solved.groups)
        .map(|((id, _), group)| {
            let center = group.rect.center();
            let title_label = group.title.unwrap_or(LayoutLabel {
                x: center.x,
                y: group.rect.y,
                width: 0.0,
                height: 0.0,
            });
            LayoutCluster {
                id: id.clone(),
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
                effective_dir: source.direction.clone(),
                requested_dir: None,
                padding: settings.namespace_padding,
            }
        })
        .collect();
    let mut edges = Vec::with_capacity(source.edges.len());
    for ((from, to, id), edge) in source.edges.keys().zip(solved.edges) {
        work.charge_adapter(edge.points.len())?;
        edges.push(LayoutEdge {
            id: id.clone(),
            from: from.clone(),
            to: to.clone(),
            from_cluster: source.nodes[from].is_namespace.then(|| from.clone()),
            to_cluster: source.nodes[to].is_namespace.then(|| to.clone()),
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
            class_label_plans_by_id: measured.class_label_plans_by_id,
            render_tree,
        },
        measured.prepared_labels,
    ))
}

fn canonical_input(
    model: &ClassDiagramModel,
    measured: &MeasuredClassDiagram,
    settings: &ClassLayoutSettings,
    neo: bool,
    work: &mut OperationLayoutWorkControl,
) -> Result<(MeasuredGraph, Vec<ShapePortProfile>)> {
    let source = &measured.graph;
    let direction = match source.direction.as_str() {
        "LR" => Direction::Right,
        "RL" => Direction::Left,
        "BT" => Direction::Up,
        _ => Direction::Down,
    };
    let (face, perpendicular) = match direction {
        Direction::Down | Direction::Up => (Side::Top, Side::Left),
        Direction::Left | Direction::Right => (Side::Left, Side::Top),
    };
    let mut graph = MeasuredGraph {
        direction,
        routing: RoutingStyle::Curved,
        node_spacing: source.nodesep,
        rank_spacing: source.ranksep,
        component_spacing: source.nodesep.max(source.ranksep),
        inherit_crossed_group_direction: true,
        nodes: Vec::new(),
        groups: Vec::new(),
        edges: Vec::new(),
    };
    work.charge_adapter(work.checked_mul(source.nodes.len(), 2)?)?;
    work.charge_adapter(work.checked_add(source.edges.len(), model.relations().len())?)?;
    let mut profiles = Vec::new();
    let mut endpoints = FxHashMap::with_capacity_and_hasher(source.nodes.len(), Default::default());
    let index = |index: usize, work: &OperationLayoutWorkControl| {
        u32::try_from(index).map_err(|_| Error::from(work.record_arithmetic_overflow()))
    };
    for (sequence, (id, node)) in source.nodes.iter().enumerate() {
        let seq = index(sequence, work)?;
        let endpoint = if node.is_namespace {
            let endpoint = Endpoint::Group(index(graph.groups.len(), work)?);
            let pad = settings.namespace_padding.max(0.0);
            let title = label_size(measured.namespace_label_metrics_by_id.get(id).copied());
            let title_band = title.map_or(pad, |size| {
                2.0 * pad
                    + size.height
                    + settings.title_margin_top.max(0.0)
                    + settings.title_margin_bottom.max(0.0)
            });
            graph.groups.push(MeasuredGroup {
                seq,
                parent: None,
                direction: None,
                padding: Insets::new(pad, title_band, pad, pad),
                min_size: Size {
                    width: 2.0 * pad + title.map_or(0.0, |size| size.width),
                    height: title_band + pad,
                },
                title,
                title_position: TitlePosition::TopCenter,
            });
            endpoint
        } else {
            let endpoint = Endpoint::Node(index(graph.nodes.len(), work)?);
            let size = Size {
                width: node.width.max(1.0),
                height: node.height.max(1.0),
            };
            let profile = ShapePortProfile::rectangle(size.width, size.height);
            graph.nodes.push(MeasuredNode {
                size,
                seq,
                parent: None,
                port_span: Some(profile.port_span(face)),
                perpendicular_port_span: Some(profile.port_span(perpendicular)),
                anchor_y: None,
            });
            profiles.push(profile);
            endpoint
        };
        endpoints.insert(id.as_str(), endpoint);
    }
    for (child, parent) in &source.parents {
        let Endpoint::Group(parent) = endpoints[parent.as_str()] else {
            return Err(Error::InvalidModel {
                message: format!("Class parent `{parent}` is not a namespace"),
            });
        };
        match endpoints[child.as_str()] {
            Endpoint::Node(index) => graph.nodes[index as usize].parent = Some(parent),
            Endpoint::Group(index) => graph.groups[index as usize].parent = Some(parent),
        }
    }
    let relations: FxHashMap<_, _> = model
        .relations()
        .map(|rel| {
            (
                rel.source.id.as_str(),
                (rel.relation.type1, rel.relation.type2),
            )
        })
        .collect();
    for ((from, to, id), edge) in &source.edges {
        work.charge_adapter(0)?;
        let (start_marker, end_marker) =
            relations
                .get(id.as_str())
                .map_or_else(Default::default, |&(start, end)| {
                    (
                        crate::svg::class_canonical_marker(start, true, neo),
                        crate::svg::class_canonical_marker(end, false, neo),
                    )
                });
        graph.edges.push(MeasuredEdge {
            source: endpoints[from.as_str()],
            target: endpoints[to.as_str()],
            min_rank_span: 1,
            weight: 1.0,
            label: label_size(Some((edge.width, edge.height))),
            start_label: label_size(
                edge.terminals
                    .as_ref()
                    .and_then(|metrics| metrics.start_right),
            ),
            end_label: label_size(edge.terminals.as_ref().and_then(|metrics| metrics.end_left)),
            start_marker,
            end_marker,
        });
    }
    Ok((graph, profiles))
}

fn label_size(metrics: Option<(f64, f64)>) -> Option<Size> {
    let (width, height) = metrics?;
    (width > 0.0 && height > 0.0).then_some(Size { width, height })
}
