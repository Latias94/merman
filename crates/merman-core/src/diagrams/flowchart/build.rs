use super::{
    Edge, FlowNodeProvenance, FlowNodeSyntax, FlowchartAst, Node, StatementEvent, Stmt, TitleKind,
};
use crate::{OperationControl, OperationControlResult, SourceSpan};
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
                    self.reserve_node(&n.id, FlowNodeProvenance::Authored, n.syntax, n.id_span);
                }
                Stmt::ShapeData {
                    target,
                    target_span,
                    ..
                } => {
                    if !self.used_edge_ids.contains(target) {
                        self.reserve_node(
                            target,
                            FlowNodeProvenance::Authored,
                            FlowNodeSyntax::ExplicitDefinition,
                            *target_span,
                        );
                    }
                }
                Stmt::Style(style) => {
                    if !self.used_edge_ids.contains(&style.target) {
                        self.reserve_node(
                            &style.target,
                            FlowNodeProvenance::Authored,
                            FlowNodeSyntax::ExplicitDefinition,
                            style.target_span,
                        );
                    }
                }
                Stmt::Subgraph(_) => {}
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
        nodes: &[Node],
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
            let provenance = if has_edges
                && node.syntax == FlowNodeSyntax::BareReference
                && self.subgraph_ids.contains(&node.id)
            {
                FlowNodeProvenance::SubgraphAnchor
            } else {
                node.provenance
            };
            self.reserve_node(&node.id, provenance, node.syntax, node.id_span);
        }
        Ok(())
    }

    fn reserve_node(
        &mut self,
        id: &str,
        provenance: FlowNodeProvenance,
        syntax: FlowNodeSyntax,
        id_span: Option<SourceSpan>,
    ) {
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
            syntax,
            id_span,
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
        let mut statements = super::super::StatementArena::default();
        let construction_control = OperationControl::new();
        let id = statements
            .push(
                Stmt::Chain {
                    node_groups: vec![nodes],
                    edge_groups: Vec::new(),
                },
                &construction_control,
            )
            .unwrap();
        let root = statements
            .prepend(
                id,
                super::super::StatementList::default(),
                &construction_control,
            )
            .unwrap();
        let ast = FlowchartAst {
            keyword: "flowchart".to_string(),
            direction: Some("TB".to_string()),
            header_span: crate::SourceSpan::new(0, 0),
            statements,
            root,
        };
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
