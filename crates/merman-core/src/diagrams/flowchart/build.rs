use super::{Edge, FlowNodeProvenance, FlowNodeSyntax, Node, Stmt, TitleKind};
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
        statements: &[Stmt],
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        // Keep Mermaid's preorder statement handling without using the Rust call stack for
        // deeply nested subgraphs.
        let mut stack = vec![statements.iter()];
        let mut visited = 0usize;
        while let Some(iter) = stack.last_mut() {
            let Some(stmt) = iter.next() else {
                stack.pop();
                continue;
            };
            if visited.is_multiple_of(128) {
                control.checkpoint()?;
            }
            visited = visited.saturating_add(1);

            match stmt {
                Stmt::Chain {
                    node_groups,
                    edge_groups,
                } => {
                    let has_edges = !edge_groups.is_empty();
                    if let Some(first_group) = node_groups.first() {
                        self.upsert_group(first_group, has_edges, control)?;
                    }
                    for (segment_index, edges) in edge_groups.iter().enumerate() {
                        if let Some(next_group) = node_groups.get(segment_index + 1) {
                            self.upsert_group(next_group, true, control)?;
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
                    let mut n = n.as_ref().clone();
                    if self.used_edge_ids.contains(&n.id) {
                        continue;
                    }
                    n.provenance = FlowNodeProvenance::Authored;
                    self.upsert_node(n);
                }
                Stmt::ShapeData {
                    target,
                    target_span,
                    ..
                } => {
                    // Reserve the structural node slot in source order. The semantic replay is
                    // the sole owner of parsing and applying shapeData values.
                    if !self.used_edge_ids.contains(target) {
                        self.upsert_node(Node {
                            id: target.clone(),
                            provenance: FlowNodeProvenance::Authored,
                            syntax: FlowNodeSyntax::ExplicitDefinition,
                            id_span: *target_span,
                            label: None,
                            label_type: TitleKind::Text,
                            label_span: None,
                            label_selection: None,
                            shape: None,
                            shape_data: None,
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
                }
                Stmt::Style(_) => {}
                Stmt::Subgraph(sg) => stack.push(sg.statements.iter()),
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

    fn upsert_group(
        &mut self,
        nodes: &[Node],
        has_edges: bool,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        for (index, node) in nodes.iter().cloned().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if self.used_edge_ids.contains(&node.id) {
                continue;
            }
            let mut node = node;
            if has_edges
                && node.syntax == FlowNodeSyntax::BareReference
                && self.subgraph_ids.contains(&node.id)
            {
                node.provenance = FlowNodeProvenance::SubgraphAnchor;
            }
            self.upsert_node(node);
        }
        Ok(())
    }

    fn upsert_node(&mut self, n: Node) {
        if let Some(&idx) = self.node_index.get(&n.id) {
            if matches!(n.provenance, FlowNodeProvenance::Authored) {
                self.nodes[idx].provenance = FlowNodeProvenance::Authored;
            }
            if n.label.is_some() {
                self.nodes[idx].label = n.label;
                self.nodes[idx].label_type = n.label_type;
                self.nodes[idx].label_span = n.label_span;
                self.nodes[idx].label_selection = n.label_selection;
            }
            if n.shape.is_some() {
                self.nodes[idx].shape = n.shape;
            }
            if n.icon.is_some() {
                self.nodes[idx].icon = n.icon;
            }
            if n.form.is_some() {
                self.nodes[idx].form = n.form;
            }
            if n.pos.is_some() {
                self.nodes[idx].pos = n.pos;
            }
            if n.img.is_some() {
                self.nodes[idx].img = n.img;
            }
            if n.constraint.is_some() {
                self.nodes[idx].constraint = n.constraint;
            }
            if n.asset_width.is_some() {
                self.nodes[idx].asset_width = n.asset_width;
            }
            if n.asset_height.is_some() {
                self.nodes[idx].asset_height = n.asset_height;
            }
            self.nodes[idx].styles.extend(n.styles);
            self.nodes[idx].classes.extend(n.classes);
            return;
        }
        let idx = self.nodes.len();
        self.node_index.insert(n.id.clone(), idx);
        self.nodes.push(n);
    }

    fn push_edge(&mut self, mut e: Edge) {
        let key = (e.from.clone(), e.to.clone());
        let existing = *self.edge_pair_counts.get(&key).unwrap_or(&0);

        let mut final_id = e.id.clone();
        let mut is_user_defined_id = false;
        if let Some(user_id) = e.id.clone() {
            if !self.used_edge_ids.contains(&user_id) {
                is_user_defined_id = true;
                self.used_edge_ids.insert(user_id);
            } else {
                final_id = None;
            }
        }

        if final_id.is_none() {
            let counter = if existing == 0 { 0 } else { existing + 1 };
            final_id = Some(format!("L_{}_{}_{}", e.from, e.to, counter));
            if let Some(id) = final_id.clone() {
                self.used_edge_ids.insert(id);
            }
        }

        self.edge_pair_counts.insert(key, existing + 1);

        e.id = final_id;
        e.is_user_defined_id = is_user_defined_id;
        e.link.length = e.link.length.min(10);
        self.edges.push(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagrams::flowchart::TitleKind;

    #[test]
    fn single_large_chain_observes_cancellation_inside_node_projection() {
        let nodes = (0..256)
            .map(|index| Node {
                id: format!("n{index}"),
                provenance: FlowNodeProvenance::Authored,
                syntax: FlowNodeSyntax::ExplicitDefinition,
                id_span: None,
                label: None,
                label_type: TitleKind::Text,
                label_span: None,
                label_selection: None,
                shape: None,
                shape_data: None,
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
            })
            .collect();
        let statements = [Stmt::Chain {
            node_groups: vec![nodes],
            edge_groups: Vec::new(),
        }];
        let mut build = FlowchartBuildState::new(HashSet::new());
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            build.add_statements(&statements, &control),
            Err(crate::OperationCancelled { .. })
        ));
        assert!(build.nodes.len() < 256);
    }
}
