//! ELK graph importer scaffold.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/transform/ElkGraphImporter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/LayeredLayoutProvider.java
//! - https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid-layout-elk/src/render.ts

use std::collections::{HashMap, VecDeque};

use crate::graph::{
    CompoundEdgeSegment, EdgeLabelPlacement, HierarchyEdge, LGraph, LLabel, LNode, LPort, LSize,
    LayeredEdge, PortRef, PortSide, PortType, create_external_port_dummy,
};
use crate::options::{
    ElkDirection, ElkPadding, HierarchyHandling, LayerConstraint, LayeredOptions,
    NodeLabelPlacement, PortConstraints, SpacingOptions,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ElkInputGraph {
    pub id: String,
    pub options: LayeredOptions,
    pub nodes: Vec<ElkInputNode>,
    pub edges: Vec<ElkInputEdge>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElkInputNode {
    pub id: String,
    pub width: f64,
    pub height: f64,
    pub parent: Option<String>,
    pub direction: Option<ElkDirection>,
    pub hierarchy_handling: Option<HierarchyHandling>,
    pub layer_constraint: Option<LayerConstraint>,
    pub port_constraints: Option<PortConstraints>,
    pub node_label_placement: NodeLabelPlacement,
    pub nested_spacing_base: Option<f64>,
    pub label: Option<ElkInputLabel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElkInputEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: Option<ElkInputLabel>,
    pub minlen: usize,
    pub priority_direction: i32,
    pub priority_shortness: i32,
    pub priority_straightness: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElkInputLabel {
    pub text: String,
    pub width: f64,
    pub height: f64,
    pub placement: EdgeLabelPlacement,
    pub inline: bool,
}

impl ElkInputLabel {
    pub fn center(text: impl Into<String>, width: f64, height: f64) -> Self {
        Self {
            text: text.into(),
            width,
            height,
            placement: EdgeLabelPlacement::Center,
            inline: true,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ImportError {
    #[error("ELK graph has duplicate node id: {id}")]
    DuplicateNode { id: String },
    #[error("ELK edge `{edge_id}` references missing node `{node_id}`")]
    MissingEndpoint { edge_id: String, node_id: String },
    #[error("ELK node `{node_id}` references missing parent `{parent_id}`")]
    MissingParent { node_id: String, parent_id: String },
    #[error("ELK parent assignment would create a cycle at node `{node_id}`")]
    ParentCycle { node_id: String },
}

pub type ImportResult<T> = Result<T, ImportError>;

/// Imports the adapter graph into an ELK layered `LGraph`.
///
/// This mirrors the front half of `ElkGraphImporter.importGraph(...)`: create the root `LGraph`,
/// transform nodes into `layerless_nodes`, create nested graphs for hierarchy-enabled compound
/// nodes, transform edges with synthetic ports, and mark graph properties discovered during import.
pub fn import_graph(input: &ElkInputGraph) -> ImportResult<LGraph> {
    let index = InputIndex::new(input)?;
    let mut root = LGraph::new(input.id.clone(), input.options.clone());
    root.options.direction = resolve_direction(root.options.direction);
    apply_graph_padding_from_options(&mut root);

    if root.options.hierarchy_handling == HierarchyHandling::IncludeChildren {
        import_hierarchical_graph(input, &index, &mut root)?;
    } else {
        import_flat_graph(input, &index, &mut root, None)?;
    }

    root.sync_graph_properties_to_options();
    Ok(root)
}

fn import_flat_graph(
    input: &ElkInputGraph,
    index: &InputIndex<'_>,
    graph: &mut LGraph,
    parent: Option<&str>,
) -> ImportResult<()> {
    for (model_order, node) in index.children(parent).into_iter().enumerate() {
        transform_node(node, graph, Some(model_order));
    }

    for (model_order, edge) in input.edges.iter().enumerate() {
        let source_parent = index.node_parent(edge.source.as_str());
        let target_parent = index.node_parent(edge.target.as_str());
        if source_parent == parent && target_parent == parent {
            transform_edge(edge, graph, model_order)?;
        }
    }

    Ok(())
}

fn import_hierarchical_graph(
    input: &ElkInputGraph,
    index: &InputIndex<'_>,
    root: &mut LGraph,
) -> ImportResult<()> {
    let mut queue = VecDeque::new();
    queue.extend(index.children(None));

    let mut model_order = 0usize;
    while let Some(node) = queue.pop_front() {
        let parent_graph = graph_for_parent(root, node.parent.as_deref());
        let node_index = transform_node(node, parent_graph, Some(model_order));
        model_order += 1;

        if node_has_nested_graph(input, node) {
            let nested_options = nested_graph_options(input, &parent_graph.options, node);
            let mut nested_graph = LGraph::new(node.id.clone(), nested_options);
            nested_graph.parent_node_id = Some(node.id.clone());
            apply_graph_padding_from_options(&mut nested_graph);
            apply_inside_node_label_padding(
                &mut nested_graph,
                &parent_graph.layerless_nodes[node_index],
            );
            parent_graph.layerless_nodes[node_index].compound = true;
            parent_graph.layerless_nodes[node_index].nested_graph = Some(Box::new(nested_graph));
            queue.extend(index.children(Some(node.id.as_str())));
        }
    }

    for (edge_order, edge) in input.edges.iter().enumerate() {
        let source_path = index.graph_path(edge.source.as_str());
        let target_path = index.graph_path(edge.target.as_str());
        if source_path == target_path {
            let graph = graph_for_path(root, &source_path);
            transform_edge(edge, graph, edge_order)?;
        } else {
            transform_cross_hierarchy_edge(edge, index, root, edge_order)?;
        }
    }

    Ok(())
}

fn nested_graph_options(
    input: &ElkInputGraph,
    parent_options: &LayeredOptions,
    node: &ElkInputNode,
) -> LayeredOptions {
    let mut options = LayeredOptions::default();
    options.direction = node.direction.unwrap_or(parent_options.direction);
    options.hierarchy_handling = node
        .hierarchy_handling
        .unwrap_or(input.options.hierarchy_handling);
    if let Some(spacing_base) = node.nested_spacing_base {
        options.spacing = SpacingOptions::layered_base_value(spacing_base);
    }
    options
}

fn apply_graph_padding_from_options(graph: &mut LGraph) {
    let ElkPadding {
        top,
        right,
        bottom,
        left,
    } = graph.options.padding;
    graph.padding.top += top;
    graph.padding.right += right;
    graph.padding.bottom += bottom;
    graph.padding.left += left;
}

fn apply_inside_node_label_padding(graph: &mut LGraph, parent_node: &LNode) {
    let padding = compute_inside_node_label_padding(&graph.options, parent_node);
    graph.padding.top += padding.top;
    graph.padding.right += padding.right;
    graph.padding.bottom += padding.bottom;
    graph.padding.left += padding.left;
}

fn compute_inside_node_label_padding(options: &LayeredOptions, node: &LNode) -> ElkPadding {
    let mut cells = [LabelCellSize::default(); 9];
    for label in &node.labels {
        let Some((row, col)) = inside_node_label_cell(node.node_label_placement) else {
            continue;
        };
        cells[row * 3 + col].add_label(label, options.spacing.label_label);
    }

    let container_gap = 2.0 * options.spacing.label_label;
    let mut padding = ElkPadding {
        top: max_cell_height(&cells, [0, 1, 2]),
        right: max_cell_width(&cells, [2, 5, 8]),
        bottom: max_cell_height(&cells, [6, 7, 8]),
        left: max_cell_width(&cells, [0, 3, 6]),
    };
    if padding.top > 0.0 {
        padding.top += options.node_labels_padding.top + container_gap;
    }
    if padding.right > 0.0 {
        padding.right += options.node_labels_padding.right + container_gap;
    }
    if padding.bottom > 0.0 {
        padding.bottom += options.node_labels_padding.bottom + container_gap;
    }
    if padding.left > 0.0 {
        padding.left += options.node_labels_padding.left + container_gap;
    }
    padding
}

#[derive(Debug, Clone, Copy, Default)]
struct LabelCellSize {
    min_width: f64,
    min_height: f64,
    label_count: usize,
}

impl LabelCellSize {
    fn add_label(&mut self, label: &LLabel, label_gap: f64) {
        self.min_width = self.min_width.max(label.size.width);
        if self.label_count > 0 {
            self.min_height += label_gap;
        }
        self.min_height += label.size.height;
        self.label_count += 1;
    }
}

fn max_cell_height(cells: &[LabelCellSize; 9], indices: [usize; 3]) -> f64 {
    indices
        .into_iter()
        .map(|index| cells[index].min_height)
        .fold(0.0, f64::max)
}

fn max_cell_width(cells: &[LabelCellSize; 9], indices: [usize; 3]) -> f64 {
    indices
        .into_iter()
        .map(|index| cells[index].min_width)
        .fold(0.0, f64::max)
}

fn inside_node_label_cell(placement: NodeLabelPlacement) -> Option<(usize, usize)> {
    match placement {
        NodeLabelPlacement::InsideTopLeft => Some((0, 0)),
        NodeLabelPlacement::InsideTopCenter => Some((0, 1)),
        NodeLabelPlacement::InsideTopRight => Some((0, 2)),
        NodeLabelPlacement::InsideCenterLeft => Some((1, 0)),
        NodeLabelPlacement::InsideCenter => Some((1, 1)),
        NodeLabelPlacement::InsideCenterRight => Some((1, 2)),
        NodeLabelPlacement::InsideBottomLeft => Some((2, 0)),
        NodeLabelPlacement::InsideBottomCenter => Some((2, 1)),
        NodeLabelPlacement::InsideBottomRight => Some((2, 2)),
        NodeLabelPlacement::Fixed
        | NodeLabelPlacement::OutsideTopLeft
        | NodeLabelPlacement::OutsideTopCenter
        | NodeLabelPlacement::OutsideTopRight
        | NodeLabelPlacement::OutsideBottomLeft
        | NodeLabelPlacement::OutsideBottomCenter
        | NodeLabelPlacement::OutsideBottomRight => None,
    }
}

fn transform_node(node: &ElkInputNode, graph: &mut LGraph, model_order: Option<usize>) -> usize {
    let mut lnode = LNode::new(node.id.clone(), node.width, node.height, model_order);
    lnode.port_constraints = node.port_constraints.unwrap_or(PortConstraints::Free);
    if let Some(layer_constraint) = node.layer_constraint {
        lnode.layer_constraint = layer_constraint;
        lnode.layer_constraint_explicit = true;
    }
    if let Some(label) = node.label.as_ref() {
        lnode.labels.push(label_to_lgraph(label));
    }
    lnode.node_label_placement = node.node_label_placement;
    graph.layerless_nodes.push(lnode);
    graph.layerless_nodes.len() - 1
}

fn transform_edge(
    edge: &ElkInputEdge,
    graph: &mut LGraph,
    model_order: usize,
) -> ImportResult<usize> {
    transform_edge_between(
        edge,
        graph,
        model_order,
        edge.source.as_str(),
        edge.target.as_str(),
        edge.source.as_str(),
        edge.target.as_str(),
        None,
        edge.label.as_ref(),
    )
}

fn transform_edge_between(
    edge: &ElkInputEdge,
    graph: &mut LGraph,
    model_order: usize,
    local_source: &str,
    local_target: &str,
    source_node_id: &str,
    target_node_id: &str,
    compound_segment: Option<CompoundEdgeSegment>,
    label: Option<&ElkInputLabel>,
) -> ImportResult<usize> {
    let source = ensure_port(graph, local_source, PortType::Output).ok_or_else(|| {
        ImportError::MissingEndpoint {
            edge_id: edge.id.clone(),
            node_id: local_source.to_string(),
        }
    })?;
    let target = ensure_port(graph, local_target, PortType::Input).ok_or_else(|| {
        ImportError::MissingEndpoint {
            edge_id: edge.id.clone(),
            node_id: local_target.to_string(),
        }
    })?;

    if source.node == target.node {
        graph.graph_properties.self_loops = true;
    }

    let mut labels = Vec::new();
    if let Some(label) = label {
        match label.placement {
            EdgeLabelPlacement::Center => graph.graph_properties.center_labels = true,
            EdgeLabelPlacement::Head | EdgeLabelPlacement::Tail => {
                graph.graph_properties.end_labels = true;
            }
        }
        labels.push(label_to_lgraph(label));
    }

    let edge_index = graph
        .add_edge(LayeredEdge {
            id: edge.id.clone(),
            source,
            target,
            source_node_id: source_node_id.to_string(),
            target_node_id: target_node_id.to_string(),
            labels,
            minlen: edge.minlen.max(1),
            reversed: false,
            bend_points: Vec::new(),
            model_order: Some(model_order),
            priority_direction: edge.priority_direction,
            priority_shortness: edge.priority_shortness,
            priority_straightness: edge.priority_straightness,
            thickness: 0.0,
            original_opposite_port: None,
            compound_segment,
        })
        .expect("ports were created before adding edge");

    if has_parallel_port_edges(&graph.layerless_nodes[source.node].ports[source.port])
        || has_parallel_port_edges(&graph.layerless_nodes[target.node].ports[target.port])
    {
        graph.graph_properties.hyperedges = true;
    }

    Ok(edge_index)
}

/// Preserve a hierarchy-crossing edge for ELK's compound preprocessor.
///
/// Source:
/// https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/transform/ElkGraphImporter.java
fn transform_cross_hierarchy_edge(
    edge: &ElkInputEdge,
    index: &InputIndex<'_>,
    root: &mut LGraph,
    model_order: usize,
) -> ImportResult<()> {
    let source_path = index.graph_path(edge.source.as_str());
    let target_path = index.graph_path(edge.target.as_str());
    let merge_edges = root.options.merge_edges;
    root.hierarchy_edges.push(HierarchyEdge {
        id: edge.id.clone(),
        source_node_id: edge.source.clone(),
        target_node_id: edge.target.clone(),
        source_port_key: hierarchy_port_key(
            edge.source.as_str(),
            model_order,
            "source",
            merge_edges,
            PortType::Output,
        ),
        target_port_key: hierarchy_port_key(
            edge.target.as_str(),
            model_order,
            "target",
            merge_edges,
            PortType::Input,
        ),
        source_path: source_path.into_iter().map(str::to_string).collect(),
        target_path: target_path.into_iter().map(str::to_string).collect(),
        labels: edge.label.iter().map(label_to_lgraph).collect(),
        minlen: edge.minlen.max(1),
        model_order: Some(model_order),
        priority_direction: edge.priority_direction,
        priority_shortness: edge.priority_shortness,
        priority_straightness: edge.priority_straightness,
    });

    Ok(())
}

fn hierarchy_port_key(
    node_id: &str,
    model_order: usize,
    role: &str,
    merge_edges: bool,
    port_type: PortType,
) -> String {
    if merge_edges {
        format!("{node_id}:collector:{port_type:?}")
    } else {
        format!("{node_id}:{model_order}:{role}")
    }
}

fn ensure_port(graph: &mut LGraph, node_id: &str, port_type: PortType) -> Option<PortRef> {
    if let Some(node) = graph
        .layerless_nodes
        .iter()
        .position(|candidate| candidate.id == node_id)
    {
        if graph.options.merge_edges
            && !graph.layerless_nodes[node].port_constraints.is_side_fixed()
        {
            let default_side = port_side_from_direction(graph.options.direction);
            let side = match port_type {
                PortType::Output => default_side,
                PortType::Input => default_side.opposed(),
            };
            return graph.provide_collector_port(node, port_type, side);
        }

        let port = graph.layerless_nodes[node].ports.len();
        graph.layerless_nodes[node].ports.push(LPort::new(
            format!("{node_id}:{port:?}"),
            node,
            port_type,
        ));
        return Some(PortRef { node, port });
    }

    graph.graph_properties.external_ports = true;
    let mut dummy = create_external_port_dummy(
        format!("external:{node_id}"),
        format!("external:{node_id}:0"),
        port_type,
        PortConstraints::Free,
        PortSide::Undefined,
        match port_type {
            PortType::Input => 1,
            PortType::Output => -1,
        },
        Default::default(),
        LSize::default(),
        LSize::default(),
        0.0,
        graph.options.direction,
    );
    let node = graph.layerless_nodes.len();
    dummy.ports[0].node = node;
    graph.layerless_nodes.push(dummy);
    Some(PortRef { node, port: 0 })
}

fn has_parallel_port_edges(port: &LPort) -> bool {
    port.incoming_edges.len() + port.outgoing_edges.len() > 1
}

fn port_side_from_direction(direction: ElkDirection) -> PortSide {
    match direction {
        ElkDirection::Right | ElkDirection::Undefined => PortSide::East,
        ElkDirection::Left => PortSide::West,
        ElkDirection::Down => PortSide::South,
        ElkDirection::Up => PortSide::North,
    }
}

fn label_to_lgraph(label: &ElkInputLabel) -> LLabel {
    let mut llabel = LLabel::new(label.text.clone(), label.width, label.height);
    llabel.placement = label.placement;
    llabel.inline = label.inline;
    llabel.label_side = None;
    llabel
}

fn node_has_nested_graph(input: &ElkInputGraph, node: &ElkInputNode) -> bool {
    input
        .nodes
        .iter()
        .any(|candidate| candidate.parent.as_deref() == Some(node.id.as_str()))
        && node
            .hierarchy_handling
            .unwrap_or(input.options.hierarchy_handling)
            == HierarchyHandling::IncludeChildren
}

fn graph_for_parent<'a>(graph: &'a mut LGraph, parent: Option<&str>) -> &'a mut LGraph {
    let Some(parent) = parent else {
        return graph;
    };

    let path = graph_path_for_parent(graph, parent);
    match path {
        Some(path) => graph_mut_at_path(graph, &path),
        None => graph,
    }
}

fn graph_for_path<'a>(graph: &'a mut LGraph, path: &[&str]) -> &'a mut LGraph {
    graph_for_parent(graph, path.last().copied())
}

fn graph_path_for_parent(graph: &LGraph, parent: &str) -> Option<Vec<usize>> {
    for (index, node) in graph.layerless_nodes.iter().enumerate() {
        let Some(nested_graph) = node.nested_graph.as_deref() else {
            continue;
        };
        if node.id == parent || nested_graph.id == parent {
            return Some(vec![index]);
        }
        if let Some(mut path) = graph_path_for_parent(nested_graph, parent) {
            path.insert(0, index);
            return Some(path);
        }
    }
    None
}

fn graph_mut_at_path<'a>(mut graph: &'a mut LGraph, path: &[usize]) -> &'a mut LGraph {
    for index in path {
        graph = graph.layerless_nodes[*index]
            .nested_graph
            .as_deref_mut()
            .expect("graph path should only contain nested graph nodes");
    }
    graph
}

fn resolve_direction(direction: ElkDirection) -> ElkDirection {
    match direction {
        ElkDirection::Undefined => ElkDirection::Right,
        direction => direction,
    }
}

struct InputIndex<'a> {
    nodes: HashMap<&'a str, &'a ElkInputNode>,
    children_by_parent: HashMap<Option<&'a str>, Vec<&'a ElkInputNode>>,
}

impl<'a> InputIndex<'a> {
    fn new(input: &'a ElkInputGraph) -> ImportResult<Self> {
        let mut nodes = HashMap::new();
        for node in &input.nodes {
            if nodes.insert(node.id.as_str(), node).is_some() {
                return Err(ImportError::DuplicateNode {
                    id: node.id.clone(),
                });
            }
        }

        let mut children_by_parent: HashMap<Option<&str>, Vec<&ElkInputNode>> = HashMap::new();
        for node in &input.nodes {
            if let Some(parent) = node.parent.as_deref() {
                if !nodes.contains_key(parent) {
                    return Err(ImportError::MissingParent {
                        node_id: node.id.clone(),
                        parent_id: parent.to_string(),
                    });
                }
            }
            children_by_parent
                .entry(node.parent.as_deref())
                .or_default()
                .push(node);
        }

        for edge in &input.edges {
            if !nodes.contains_key(edge.source.as_str()) {
                return Err(ImportError::MissingEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: edge.source.clone(),
                });
            }
            if !nodes.contains_key(edge.target.as_str()) {
                return Err(ImportError::MissingEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: edge.target.clone(),
                });
            }
        }

        detect_parent_cycles(input, &nodes)?;
        Ok(Self {
            nodes,
            children_by_parent,
        })
    }

    fn children(&self, parent: Option<&str>) -> Vec<&'a ElkInputNode> {
        self.children_by_parent
            .get(&parent)
            .cloned()
            .unwrap_or_default()
    }

    fn node_parent(&self, id: &str) -> Option<&'a str> {
        self.nodes.get(id).and_then(|node| node.parent.as_deref())
    }

    fn graph_path(&self, node: &str) -> Vec<&'a str> {
        let mut path = Vec::new();
        let mut current = self.node_parent(node);
        while let Some(parent) = current {
            path.push(parent);
            current = self.node_parent(parent);
        }
        path.reverse();
        path
    }
}

fn detect_parent_cycles<'a>(
    input: &'a ElkInputGraph,
    nodes: &HashMap<&'a str, &'a ElkInputNode>,
) -> ImportResult<()> {
    for node in &input.nodes {
        let mut seen = Vec::new();
        let mut current = node.parent.as_deref();
        while let Some(parent) = current {
            if seen.contains(&parent) {
                return Err(ImportError::ParentCycle {
                    node_id: node.id.clone(),
                });
            }
            seen.push(parent);
            current = nodes.get(parent).and_then(|node| node.parent.as_deref());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compound::preprocess_source_ported_compound_graph;
    use crate::graph::LNodeKind;
    use crate::options::OrderingStrategy;

    fn node(id: &str) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            port_constraints: None,
            node_label_placement: NodeLabelPlacement::Fixed,
            nested_spacing_base: None,
            label: None,
        }
    }

    fn edge(id: &str, source: &str, target: &str) -> ElkInputEdge {
        ElkInputEdge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            label: None,
            minlen: 1,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        }
    }

    fn graph(nodes: Vec<ElkInputNode>, edges: Vec<ElkInputEdge>) -> ElkInputGraph {
        ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes,
            edges,
        }
    }

    #[test]
    fn imports_mermaid_flowchart_nodes_edges_labels_and_model_order() {
        let mut a = node("A");
        a.label = Some(ElkInputLabel::center("Alpha", 42.0, 18.0));
        let mut ab = edge("A-B", "A", "B");
        ab.label = Some(ElkInputLabel::center("go", 20.0, 12.0));

        let lgraph = import_graph(&graph(vec![a, node("B")], vec![ab])).unwrap();

        assert_eq!(lgraph.layerless_nodes.len(), 2);
        assert_eq!(lgraph.layerless_nodes[0].id, "A");
        assert_eq!(lgraph.layerless_nodes[0].model_order, Some(0));
        assert_eq!(lgraph.layerless_nodes[0].labels[0].text, "Alpha");
        assert_eq!(lgraph.edges.len(), 1);
        assert_eq!(lgraph.edges[0].model_order, Some(0));
        assert_eq!(
            lgraph.edges[0].labels[0].placement,
            EdgeLabelPlacement::Center
        );
        assert!(lgraph.graph_properties.center_labels);
        assert!(lgraph.options.graph_has_center_labels);
    }

    #[test]
    fn importer_applies_layered_padding_option_to_lgraph_padding() {
        let mut input = graph(vec![node("A")], vec![]);
        input.options.padding = ElkPadding {
            top: 7.0,
            right: 8.0,
            bottom: 9.0,
            left: 10.0,
        };

        let lgraph = import_graph(&input).unwrap();

        assert_eq!(lgraph.padding.top, 7.0);
        assert_eq!(lgraph.padding.right, 8.0);
        assert_eq!(lgraph.padding.bottom, 9.0);
        assert_eq!(lgraph.padding.left, 10.0);
    }

    #[test]
    fn importer_applies_layered_padding_option_to_nested_graphs() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let lgraph = import_graph(&graph(vec![cluster, child], vec![])).unwrap();
        let nested = lgraph.layerless_nodes[0].nested_graph.as_ref().unwrap();

        assert_eq!(lgraph.padding.top, 12.0);
        assert_eq!(lgraph.padding.left, 12.0);
        assert_eq!(nested.padding.top, 12.0);
        assert_eq!(nested.padding.left, 12.0);
    }

    #[test]
    fn importer_adds_inside_top_node_label_padding_to_nested_graphs() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        cluster.node_label_placement = NodeLabelPlacement::InsideTopCenter;
        cluster.label = Some(ElkInputLabel::center("Cluster", 64.0, 22.0));
        cluster.nested_spacing_base = Some(30.0);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let lgraph = import_graph(&graph(vec![cluster, child], vec![])).unwrap();
        let nested = lgraph.layerless_nodes[0].nested_graph.as_ref().unwrap();

        assert_eq!(nested.options.spacing.node_node, 30.0);
        assert_eq!(nested.padding.top, 39.0);
        assert_eq!(nested.padding.right, 12.0);
        assert_eq!(nested.padding.bottom, 12.0);
        assert_eq!(nested.padding.left, 12.0);
    }

    #[test]
    fn imports_include_children_hierarchy_into_nested_graphs() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let lgraph = import_graph(&graph(vec![cluster, child, node("B")], vec![])).unwrap();

        let cluster = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        let nested = cluster.nested_graph.as_ref().unwrap();
        assert_eq!(nested.parent_node_id.as_deref(), Some("cluster"));
        assert_eq!(nested.layerless_nodes[0].id, "A");
    }

    #[test]
    fn importer_preserves_descendant_edge_for_compound_preprocessor() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let lgraph = import_graph(&graph(
            vec![cluster, child],
            vec![edge("cluster-A", "cluster", "A")],
        ))
        .unwrap();
        let cluster = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        let nested = cluster.nested_graph.as_ref().unwrap();
        assert!(nested.edges.is_empty());
        assert_eq!(lgraph.hierarchy_edges.len(), 1);
        assert_eq!(lgraph.hierarchy_edges[0].id, "cluster-A");
        assert_eq!(lgraph.hierarchy_edges[0].source_node_id, "cluster");
        assert_eq!(lgraph.hierarchy_edges[0].target_node_id, "A");
        assert_eq!(lgraph.hierarchy_edges[0].source_path, Vec::<String>::new());
        assert_eq!(lgraph.hierarchy_edges[0].target_path, vec!["cluster"]);
    }

    #[test]
    fn source_ported_compound_metadata_links_parent_port_and_external_dummy() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let mut lgraph = import_graph(&graph(
            vec![cluster, child, node("B")],
            vec![edge("A-B", "A", "B")],
        ))
        .unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let cluster_index = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "cluster")
            .unwrap();
        let cluster = &lgraph.layerless_nodes[cluster_index];
        let parent_port = &cluster.ports[0];
        let port_dummy = parent_port
            .port_dummy
            .as_ref()
            .expect("compound port should point to nested external dummy");
        assert!(parent_port.inside_connections);
        assert_eq!(port_dummy.graph_id, "cluster");

        let nested = cluster.nested_graph.as_ref().unwrap();
        let external = &nested.layerless_nodes[port_dummy.node];
        assert_eq!(external.external_port_side, PortSide::South);
        let origin = external
            .origin_port
            .as_ref()
            .expect("external dummy should point back to parent port");
        assert_eq!(origin.graph_id, "root");
        assert_eq!(origin.port.node, cluster_index);
        assert_eq!(origin.port.port, 0);
    }

    #[test]
    fn source_ported_compound_metadata_links_parent_to_child_external_dummy() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let mut lgraph = import_graph(&graph(
            vec![cluster, child],
            vec![edge("cluster-A", "cluster", "A")],
        ))
        .unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let cluster_index = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "cluster")
            .unwrap();
        let cluster = &lgraph.layerless_nodes[cluster_index];
        let parent_port = cluster
            .ports
            .iter()
            .find(|port| port.port_dummy.is_some())
            .expect("parent-to-child edge should create a parent external port");
        let port_dummy = parent_port.port_dummy.as_ref().unwrap();
        assert!(parent_port.inside_connections);
        assert_eq!(port_dummy.graph_id, "cluster");

        let nested = cluster.nested_graph.as_ref().unwrap();
        let external = &nested.layerless_nodes[port_dummy.node];
        let origin = external
            .origin_port
            .as_ref()
            .expect("external dummy should point back to parent port");
        assert_eq!(origin.graph_id, "root");
        assert_eq!(origin.port.node, cluster_index);
    }

    #[test]
    fn source_ported_compound_parent_boundary_segments_use_external_port_dummies() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let mut lgraph = import_graph(&graph(
            vec![cluster, child],
            vec![edge("cluster-A", "cluster", "A")],
        ))
        .unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);
        let cluster = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        let nested = cluster.nested_graph.as_ref().unwrap();
        let segment = nested
            .edges
            .iter()
            .find(|edge| edge.id == "cluster-A")
            .unwrap();

        assert_eq!(
            nested.layerless_nodes[segment.source.node].kind,
            LNodeKind::ExternalPort
        );
        assert_eq!(nested.layerless_nodes[segment.target.node].id, "A");
    }

    #[test]
    fn source_ported_compound_import_records_cross_hierarchy_segments() {
        let mut outer = node("outer");
        outer.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut inner = node("inner");
        inner.parent = Some("outer".to_string());
        inner.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("inner".to_string());

        let mut lgraph = import_graph(&graph(
            vec![outer, inner, child, node("B")],
            vec![edge("A-B", "A", "B")],
        ))
        .unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let outer = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "outer")
            .unwrap();
        let inner_graph = outer
            .nested_graph
            .as_ref()
            .unwrap()
            .layerless_nodes
            .iter()
            .find(|node| node.id == "inner")
            .unwrap()
            .nested_graph
            .as_ref()
            .unwrap();
        let outer_graph = outer.nested_graph.as_ref().unwrap();
        assert_eq!(
            inner_graph.cross_hierarchy_edges[0].segment,
            CompoundEdgeSegment::Output { depth: 2 }
        );
        assert_eq!(
            outer_graph.cross_hierarchy_edges[0].segment,
            CompoundEdgeSegment::Output { depth: 1 }
        );
        assert_eq!(
            lgraph.cross_hierarchy_edges[0].segment,
            CompoundEdgeSegment::Output { depth: 0 }
        );
    }

    #[test]
    fn source_ported_compound_reuses_exported_external_port_when_hierarchy_edges_merge() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let mut first = edge("A-B", "A", "B");
        let mut first_label = ElkInputLabel::center("first", 12.0, 6.0);
        first_label.placement = EdgeLabelPlacement::Tail;
        first.label = Some(first_label);
        let mut second = edge("A-C", "A", "C");
        let mut second_label = ElkInputLabel::center("second", 18.0, 6.0);
        second_label.placement = EdgeLabelPlacement::Tail;
        second.label = Some(second_label);
        let mut input = graph(
            vec![cluster, child, node("B"), node("C")],
            vec![first, second],
        );
        input.options.merge_edges = true;
        input.options.merge_hierarchy_edges = true;

        let mut lgraph = import_graph(&input).unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let nested = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap()
            .nested_graph
            .as_ref()
            .unwrap();
        assert_eq!(nested.edges.len(), 1);
        assert_eq!(nested.cross_hierarchy_edges.len(), 2);
        assert!(
            nested
                .cross_hierarchy_edges
                .iter()
                .all(|segment| segment.edge == 0)
        );
        assert_eq!(nested.edges[0].labels.len(), 2);
        assert_eq!(
            nested.edges[0]
                .labels
                .iter()
                .filter_map(|label| label.original_label_edge.as_deref())
                .collect::<Vec<_>>(),
            vec!["A-B", "A-C"]
        );
        assert!(nested.graph_properties.end_labels);
    }

    #[test]
    fn source_ported_compound_keeps_external_ports_distinct_when_hierarchy_merge_is_disabled() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let mut input = graph(
            vec![cluster, child, node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
        );
        input.options.merge_edges = true;
        input.options.merge_hierarchy_edges = false;

        let mut lgraph = import_graph(&input).unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let nested = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap()
            .nested_graph
            .as_ref()
            .unwrap();
        assert_eq!(nested.edges.len(), 2);
        assert_eq!(
            nested
                .cross_hierarchy_edges
                .iter()
                .map(|segment| segment.edge)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn source_ported_compound_parent_end_segments_are_not_reused() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let mut input = graph(
            vec![cluster, child],
            vec![
                edge("cluster-A-1", "cluster", "A"),
                edge("cluster-A-2", "cluster", "A"),
            ],
        );
        input.options.merge_edges = true;
        input.options.merge_hierarchy_edges = true;

        let mut lgraph = import_graph(&input).unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let nested = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap()
            .nested_graph
            .as_ref()
            .unwrap();
        assert_eq!(nested.edges.len(), 2);
        assert_eq!(
            nested
                .cross_hierarchy_edges
                .iter()
                .map(|segment| segment.edge)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert!(
            nested.edges.iter().all(
                |edge| nested.layerless_nodes[edge.source.node].kind == LNodeKind::ExternalPort
            )
        );
    }

    #[test]
    fn importer_reuses_collector_ports_when_edges_are_merged() {
        let mut input = graph(
            vec![node("A"), node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
        );
        input.options.merge_edges = true;

        let lgraph = import_graph(&input).unwrap();

        let a = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        assert_eq!(lgraph.layerless_nodes[a].ports.len(), 1);
        assert_eq!(
            lgraph.layerless_nodes[a].ports[0].collector_type,
            Some(PortType::Output)
        );
        assert_eq!(
            lgraph.layerless_nodes[a].ports[0].outgoing_edges,
            vec![0, 1]
        );
        assert!(lgraph.graph_properties.hyperedges);
        assert!(lgraph.options.graph_has_hyperedges);
    }

    #[test]
    fn importer_keeps_dedicated_ports_when_edge_merge_is_disabled() {
        let lgraph = import_graph(&graph(
            vec![node("A"), node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
        ))
        .unwrap();

        let a = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        assert_eq!(lgraph.layerless_nodes[a].ports.len(), 2);
        assert!(
            lgraph.layerless_nodes[a]
                .ports
                .iter()
                .all(|port| port.collector_type.is_none())
        );
        assert!(!lgraph.graph_properties.hyperedges);
    }

    #[test]
    fn importer_keeps_dedicated_ports_when_node_port_constraints_are_side_fixed() {
        let mut a = node("A");
        a.port_constraints = Some(PortConstraints::FixedSide);
        let mut input = graph(
            vec![a, node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
        );
        input.options.merge_edges = true;

        let lgraph = import_graph(&input).unwrap();

        let a = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        assert_eq!(lgraph.layerless_nodes[a].ports.len(), 2);
        assert!(
            lgraph.layerless_nodes[a]
                .ports
                .iter()
                .all(|port| port.collector_type.is_none())
        );
    }

    #[test]
    fn import_rejects_invalid_parent_and_endpoints() {
        let mut child = node("A");
        child.parent = Some("missing".to_string());
        assert!(matches!(
            import_graph(&graph(vec![child], vec![])),
            Err(ImportError::MissingParent { .. })
        ));

        assert!(matches!(
            import_graph(&graph(vec![node("A")], vec![edge("A-B", "A", "B")])),
            Err(ImportError::MissingEndpoint { .. })
        ));
    }

    #[test]
    fn import_preserves_model_order_strategy_without_enabling_wrapping() {
        let mut input = graph(vec![node("A"), node("B")], vec![edge("A-B", "A", "B")]);
        input.options.consider_model_order_strategy = OrderingStrategy::NodesAndEdges;

        let lgraph = import_graph(&input).unwrap();

        assert_eq!(
            lgraph.options.consider_model_order_strategy,
            OrderingStrategy::NodesAndEdges
        );
        assert!(!lgraph.options.graph_has_hyperedges);
    }
}
