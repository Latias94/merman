use super::{
    Edge, EdgeAction, FlowNodeProvenance, FlowchartAst, Node, NodeAction, StatementEvent, Stmt,
    TitleKind,
};
use crate::{OperationControl, OperationControlResult};
use std::collections::{HashMap, HashSet};

pub(super) struct FlowchartBuildState {
    pub(super) nodes: Vec<Node>,
    pub(super) node_index: HashMap<String, usize>,
    pub(super) edges: Vec<Edge>,
    pub(super) used_edge_ids: HashSet<String>,
    pub(super) subgraph_ids: HashSet<String>,
    pub(super) edge_pair_counts: HashMap<(String, String), usize>,
}

impl FlowchartBuildState {
    pub(super) fn new(subgraph_ids: HashSet<String>) -> Self {
        Self {
            nodes: Vec::new(),
            node_index: HashMap::new(),
            edges: Vec::new(),
            used_edge_ids: HashSet::new(),
            subgraph_ids,
            edge_pair_counts: HashMap::new(),
        }
    }

    pub(super) fn add_statements(
        &mut self,
        ast: &FlowchartAst,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        for (visited, event) in ast.walk().enumerate() {
            if visited.is_multiple_of(128) {
                control.checkpoint()?;
            }
            let StatementEvent::Enter(stmt) = event else {
                continue;
            };

            match stmt {
                Stmt::Chain {
                    node_groups,
                    edge_groups,
                } => {
                    let has_edges = !edge_groups.is_empty();
                    if let Some(first_group) = node_groups.first() {
                        self.reserve_group(first_group, has_edges, control)?;
                    }
                    for (segment_index, edges) in edge_groups.iter().enumerate() {
                        if let Some(next_group) = node_groups.get(segment_index + 1) {
                            self.reserve_group(next_group, true, control)?;
                        }
                        for (edge_index, edge) in edges.iter().cloned().enumerate() {
                            if edge_index % 128 == 0 {
                                control.checkpoint()?;
                            }
                            self.push_edge(edge);
                        }
                    }
                }
                Stmt::Node(n) => {
                    if self.used_edge_ids.contains(&n.id) {
                        continue;
                    }
                    self.reserve_node(&n.id, FlowNodeProvenance::Authored);
                }
                Stmt::ShapeData { target, .. } => {
                    if !self.used_edge_ids.contains(target) {
                        self.reserve_node(target, FlowNodeProvenance::Authored);
                    }
                }
                Stmt::Style(style) => {
                    if !self.used_edge_ids.contains(&style.target) {
                        self.reserve_node(&style.target, FlowNodeProvenance::Authored);
                    }
                }
                Stmt::Subgraph(_) | Stmt::AccessibilityValue(_) => {}
                Stmt::Direction(_)
                | Stmt::ClassDef(_)
                | Stmt::ClassAssign(_)
                | Stmt::Click(_)
                | Stmt::LinkStyle(_) => {}
            }
        }
        control.checkpoint()?;
        Ok(())
    }

    fn reserve_group(
        &mut self,
        nodes: &[NodeAction],
        has_edges: bool,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        for (index, node) in nodes.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if self.used_edge_ids.contains(&node.id) {
                continue;
            }
            let provenance =
                if has_edges && !node.is_explicit() && self.subgraph_ids.contains(&node.id) {
                    FlowNodeProvenance::SubgraphAnchor
                } else {
                    FlowNodeProvenance::Authored
                };
            self.reserve_node(&node.id, provenance);
        }
        Ok(())
    }

    fn reserve_node(&mut self, id: &str, provenance: FlowNodeProvenance) {
        if let Some(&idx) = self.node_index.get(id) {
            if matches!(provenance, FlowNodeProvenance::Authored) {
                self.nodes[idx].provenance = FlowNodeProvenance::Authored;
            }
            return;
        }
        // Reserve identity and provenance only. The existing semantic traversal owns every
        // mutable definition, so an early metadata call cannot see a later label or shape.
        let idx = self.nodes.len();
        self.node_index.insert(id.to_string(), idx);
        self.nodes.push(Node {
            id: id.to_string(),
            provenance,
            label: None,
            label_type: TitleKind::Text,
            shape: None,
            icon: None,
            form: None,
            pos: None,
            img: None,
            constraint: None,
            asset_width: None,
            asset_height: None,
            styles: Vec::new(),
            classes: Vec::new(),
            link: None,
            link_target: None,
            have_callback: false,
        });
    }

    fn push_edge(&mut self, mut edge: EdgeAction) {
        let key = (edge.from.clone(), edge.to.clone());
        let existing = *self.edge_pair_counts.get(&key).unwrap_or(&0);

        let mut final_id = edge.id.clone();
        let mut is_user_defined_id = false;
        if let Some(user_id) = edge.id.clone() {
            if !self.used_edge_ids.contains(&user_id) {
                is_user_defined_id = true;
                self.used_edge_ids.insert(user_id);
            } else {
                final_id = None;
            }
        }

        if final_id.is_none() {
            let counter = if existing == 0 { 0 } else { existing + 1 };
            final_id = Some(format!("L_{}_{}_{}", edge.from, edge.to, counter));
            if let Some(id) = final_id.clone() {
                self.used_edge_ids.insert(id);
            }
        }

        self.edge_pair_counts.insert(key, existing + 1);

        edge.link.length = edge.link.length.min(10);
        let (label, label_type) = edge.label.map_or((None, TitleKind::Text), |label| {
            (Some(label.text), label.kind)
        });
        self.edges.push(Edge {
            from: edge.from,
            to: edge.to,
            id: final_id,
            link: edge.link,
            label,
            label_type,
            style: Vec::new(),
            classes: Vec::new(),
            interpolate: None,
            is_user_defined_id,
            animate: None,
            animation: None,
            step: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse_flowchart_ast;
    use super::*;

    #[test]
    fn single_large_chain_observes_cancellation_inside_node_projection() {
        let nodes = (0..256)
            .map(|index| format!("n{index}"))
            .collect::<Vec<_>>()
            .join(" & ");
        let source = format!("flowchart TB\n{nodes}\n");
        let meta = crate::SemanticMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::empty_object(),
            title: None,
        };
        let ast = parse_flowchart_ast(&source, &meta).expect("large node group should parse");
        let mut build = FlowchartBuildState::new(HashSet::new());
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            build.add_statements(&ast, &control),
            Err(crate::OperationCancelled { .. })
        ));
        assert!(build.nodes.len() < 256);
    }
}
