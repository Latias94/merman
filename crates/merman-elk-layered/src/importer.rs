//! ELK graph importer scaffold.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/transform/ElkGraphImporter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/LayeredLayoutProvider.java
//! - https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid-layout-elk/src/render.ts

use std::collections::{HashMap, VecDeque};

use crate::graph::{
    EdgeLabelPlacement, LGraph, LLabel, LNode, LNodeKind, LPort, LayeredEdge, PortRef, PortType,
};
use crate::options::{ElkDirection, HierarchyHandling, LayeredOptions};

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
            let mut nested_options = parent_graph.options.clone();
            if let Some(direction) = node.direction {
                nested_options.direction = direction;
            }
            if let Some(hierarchy_handling) = node.hierarchy_handling {
                nested_options.hierarchy_handling = hierarchy_handling;
            }
            let mut nested_graph = LGraph::new(node.id.clone(), nested_options);
            nested_graph.parent_node_id = Some(node.id.clone());
            parent_graph.layerless_nodes[node_index].compound = true;
            parent_graph.layerless_nodes[node_index].nested_graph = Some(Box::new(nested_graph));
            queue.extend(index.children(Some(node.id.as_str())));
        }
    }

    for (edge_order, edge) in input.edges.iter().enumerate() {
        let edge_parent = edge_parent_graph_id(index, edge.source.as_str(), edge.target.as_str());
        let graph = graph_for_parent(root, edge_parent.as_deref());
        transform_edge(edge, graph, edge_order)?;
    }

    Ok(())
}

fn transform_node(node: &ElkInputNode, graph: &mut LGraph, model_order: Option<usize>) -> usize {
    let mut lnode = LNode::new(node.id.clone(), node.width, node.height, model_order);
    if let Some(label) = node.label.as_ref() {
        lnode.labels.push(label_to_lgraph(label));
    }
    graph.layerless_nodes.push(lnode);
    graph.layerless_nodes.len() - 1
}

fn transform_edge(
    edge: &ElkInputEdge,
    graph: &mut LGraph,
    model_order: usize,
) -> ImportResult<usize> {
    let source = ensure_port(graph, edge.source.as_str(), PortType::Output).ok_or_else(|| {
        ImportError::MissingEndpoint {
            edge_id: edge.id.clone(),
            node_id: edge.source.clone(),
        }
    })?;
    let target = ensure_port(graph, edge.target.as_str(), PortType::Input).ok_or_else(|| {
        ImportError::MissingEndpoint {
            edge_id: edge.id.clone(),
            node_id: edge.target.clone(),
        }
    })?;

    let edge_index = graph.edges.len();
    graph.layerless_nodes[source.node].ports[source.port]
        .outgoing_edges
        .push(edge_index);
    graph.layerless_nodes[target.node].ports[target.port]
        .incoming_edges
        .push(edge_index);

    if source.node == target.node {
        graph.graph_properties.self_loops = true;
    }

    if has_parallel_port_edges(&graph.layerless_nodes[source.node].ports[source.port])
        || has_parallel_port_edges(&graph.layerless_nodes[target.node].ports[target.port])
    {
        graph.graph_properties.hyperedges = true;
    }

    let mut labels = Vec::new();
    if let Some(label) = edge.label.as_ref() {
        match label.placement {
            EdgeLabelPlacement::Center => graph.graph_properties.center_labels = true,
            EdgeLabelPlacement::Head | EdgeLabelPlacement::Tail => {
                graph.graph_properties.end_labels = true;
            }
        }
        labels.push(label_to_lgraph(label));
    }

    graph.edges.push(LayeredEdge {
        id: edge.id.clone(),
        source,
        target,
        source_node_id: edge.source.clone(),
        target_node_id: edge.target.clone(),
        labels,
        minlen: edge.minlen.max(1),
        reversed: false,
        bend_points: Vec::new(),
        model_order: Some(model_order),
        priority_direction: edge.priority_direction,
        priority_shortness: edge.priority_shortness,
        thickness: 0.0,
    });

    Ok(edge_index)
}

fn ensure_port(graph: &mut LGraph, node_id: &str, port_type: PortType) -> Option<PortRef> {
    let node = match graph
        .layerless_nodes
        .iter()
        .position(|candidate| candidate.id == node_id)
    {
        Some(node) => node,
        None => {
            graph.graph_properties.external_ports = true;
            let mut dummy = LNode::new(format!("external:{node_id}"), 0.0, 0.0, None);
            dummy.kind = LNodeKind::ExternalPort;
            graph.layerless_nodes.push(dummy);
            graph.layerless_nodes.len() - 1
        }
    };
    let port = graph.layerless_nodes[node].ports.len();
    graph.layerless_nodes[node].ports.push(LPort::new(
        format!("{node_id}:{port:?}"),
        node,
        port_type,
    ));
    Some(PortRef { node, port })
}

fn has_parallel_port_edges(port: &LPort) -> bool {
    port.incoming_edges.len() + port.outgoing_edges.len() > 1
}

fn label_to_lgraph(label: &ElkInputLabel) -> LLabel {
    let mut llabel = LLabel::new(label.text.clone(), label.width, label.height);
    llabel.placement = label.placement;
    llabel.inline = label.inline;
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

fn edge_parent_graph_id(index: &InputIndex<'_>, source: &str, target: &str) -> Option<String> {
    if index.is_descendant(target, source) {
        return Some(source.to_string());
    }
    if index.is_descendant(source, target) {
        return Some(target.to_string());
    }
    index.common_parent(source, target)
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

    fn is_descendant(&self, node: &str, ancestor: &str) -> bool {
        let mut current = self.node_parent(node);
        while let Some(parent) = current {
            if parent == ancestor {
                return true;
            }
            current = self.node_parent(parent);
        }
        false
    }

    fn common_parent(&self, source: &str, target: &str) -> Option<String> {
        let mut source_ancestors = Vec::new();
        let mut current = self.node_parent(source);
        while let Some(parent) = current {
            source_ancestors.push(parent);
            current = self.node_parent(parent);
        }

        let mut current = self.node_parent(target);
        while let Some(parent) = current {
            if source_ancestors.contains(&parent) {
                return Some(parent.to_string());
            }
            current = self.node_parent(parent);
        }

        None
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
    use crate::options::OrderingStrategy;

    fn node(id: &str) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
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
    fn imports_descendant_edge_into_ancestor_nested_graph() {
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
        assert_eq!(nested.edges[0].id, "cluster-A");
        assert_eq!(nested.edges[0].source_node_id, "cluster");
        assert_eq!(nested.edges[0].target_node_id, "A");
        assert!(nested.graph_properties.external_ports);
        assert!(nested.options.graph_has_external_ports);
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
