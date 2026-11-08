use super::{
    FlowSubGraph, FlowchartAst, StatementEvent, Stmt, SubgraphBlock, TitleKind,
    is_ecmascript_trim_char, trim_flowdb_label_text,
};
use crate::{OperationControl, OperationControlResult};
use merman_model::families::flowchart::FlowDroppedSubgraphMember;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
enum StatementItem {
    Id(String),
    Dir(String),
}

pub(super) struct SubgraphBuilder {
    sub_count: usize,
    pub(super) subgraphs: Vec<FlowSubGraph>,
    pub(super) declaration_owners: Vec<usize>,
    pub(super) dropped_subgraph_members: Vec<FlowDroppedSubgraphMember>,
    subgraph_index: HashMap<String, usize>,
    claimed_members: HashMap<String, usize>,
    inherit_dir: bool,
    global_dir: Option<String>,
}

impl SubgraphBuilder {
    pub(super) fn new(inherit_dir: bool, global_dir: Option<String>) -> Self {
        Self {
            sub_count: 0,
            subgraphs: Vec::new(),
            declaration_owners: Vec::new(),
            dropped_subgraph_members: Vec::new(),
            subgraph_index: HashMap::new(),
            claimed_members: HashMap::new(),
            inherit_dir,
            global_dir,
        }
    }

    pub(super) fn visit_statements(
        &mut self,
        ast: &FlowchartAst,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let mut stack: Vec<Vec<StatementItem>> = Vec::new();
        for (visited, event) in ast.walk().enumerate() {
            if visited.is_multiple_of(128) {
                control.checkpoint()?;
            }
            match event {
                StatementEvent::Enter(Stmt::Subgraph(_)) => stack.push(Vec::new()),
                StatementEvent::Enter(stmt) => {
                    if let Some(items) = stack.last_mut() {
                        push_statement_items(items, stmt, control)?;
                    }
                }
                StatementEvent::Exit(subgraph) => {
                    let items = stack.pop().unwrap_or_default();
                    let id = self.eval_subgraph_from_items(subgraph, items, control)?;
                    if let Some(parent) = stack.last_mut() {
                        parent.push(StatementItem::Id(id));
                    }
                }
            }
        }
        control.checkpoint()?;
        Ok(())
    }

    fn eval_subgraph_from_items(
        &mut self,
        sg: &SubgraphBlock,
        items: Vec<StatementItem>,
        control: &OperationControl,
    ) -> OperationControlResult<String> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut members: Vec<String> = Vec::new();
        let mut dir: Option<String> = None;

        for (index, item) in items.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            match item {
                StatementItem::Dir(d) => dir = Some(d),
                StatementItem::Id(id) => {
                    if trim_flowdb_label_text(&id).is_empty() {
                        continue;
                    }
                    if seen.insert(id.clone()) {
                        members.push(id);
                    }
                }
            }
        }

        let has_explicit_dir = dir.is_some();
        let dir = dir.or_else(|| {
            if self.inherit_dir {
                self.global_dir.clone()
            } else {
                None
            }
        });

        let mut id: Option<String> = {
            let trimmed = trim_flowdb_label_text(&sg.header.raw_id);
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        };

        // Mirror Mermaid `FlowDB.addSubGraph(...)`:
        // `if (_id === _title && /\\s/.exec(_title.text)) id = undefined;`
        //
        // The important nuance is that this checks the untrimmed title token (including any
        // extra whitespace that may have been captured into the header).
        if sg.header.id_equals_title && sg.header.raw_title.chars().any(is_ecmascript_trim_char) {
            id = None;
        }

        let id = id.unwrap_or_else(|| format!("subGraph{}", self.sub_count));
        let title = trim_flowdb_label_text(&sg.header.raw_title).to_string();
        let label_type = match sg.header.title_kind {
            TitleKind::Text => "text",
            TitleKind::String => "string",
            TitleKind::Markdown => "markdown",
        }
        .to_string();

        self.sub_count += 1;

        // Mermaid 12.1 keeps the first completed declaration as the single group owner.
        // Replay still visits every declaration, so retain its canonical owner separately.
        let owner = if let Some(&index) = self.subgraph_index.get(&id) {
            index
        } else {
            let index = self.subgraphs.len();
            self.subgraph_index.insert(id.clone(), index);
            self.subgraphs.push(FlowSubGraph {
                id: id.clone(),
                nodes: Vec::new(),
                title,
                classes: Vec::new(),
                styles: Vec::new(),
                dir,
                has_explicit_dir,
                label_type,
                metadata: None,
            });
            index
        };
        for (index, member) in members.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if member == id {
                continue;
            }
            if let Some(&previous_owner) = self.claimed_members.get(member.as_str()) {
                if previous_owner != owner && !self.is_nested_in(previous_owner, &seen, control)? {
                    self.dropped_subgraph_members
                        .push(FlowDroppedSubgraphMember {
                            node: member,
                            kept_by: self.subgraphs[previous_owner].id.clone(),
                            dropped_from: id.clone(),
                        });
                }
                continue;
            }
            self.subgraphs[owner].nodes.push(member.clone());
            self.claimed_members.insert(member, owner);
        }
        self.declaration_owners.push(owner);

        Ok(id)
    }

    fn is_nested_in(
        &self,
        owner: usize,
        ancestor_members: &HashSet<String>,
        control: &OperationControl,
    ) -> OperationControlResult<bool> {
        let mut current = Some(owner);
        let mut visited = HashSet::new();
        while let Some(index) = current {
            if visited.len().is_multiple_of(128) {
                control.checkpoint()?;
            }
            if !visited.insert(index) {
                break;
            }
            let id = self.subgraphs[index].id.as_str();
            if ancestor_members.contains(id) {
                return Ok(true);
            }
            current = self.claimed_members.get(id).copied();
        }
        Ok(false)
    }
}

fn push_statement_items(
    out: &mut Vec<StatementItem>,
    stmt: &Stmt,
    control: &OperationControl,
) -> OperationControlResult<()> {
    match stmt {
        Stmt::Chain {
            node_groups,
            edge_groups,
        } => {
            // Mermaid FlowDB's subgraph membership list is based on the Jison
            // `vertexStatement.nodes` shape, which prepends each later group while
            // preserving the order within each group (`a & b --> c & d` yields `[c, d, a, b]`).
            //
            // For node-only group statements (e.g. `A & B`), there are no edges and the list
            // preserves the input order.
            if edge_groups.is_empty() {
                for (index, n) in node_groups.iter().flatten().enumerate() {
                    if index % 128 == 0 {
                        control.checkpoint()?;
                    }
                    out.push(StatementItem::Id(n.id.clone()));
                }
            } else {
                for (index, n) in node_groups.iter().rev().flatten().enumerate() {
                    if index % 128 == 0 {
                        control.checkpoint()?;
                    }
                    out.push(StatementItem::Id(n.id.clone()));
                }
            }
        }
        Stmt::Node(n) => out.push(StatementItem::Id(n.id.clone())),
        Stmt::Direction(d) => out.push(StatementItem::Dir(d.clone())),
        Stmt::ShapeData { target, .. } => out.push(StatementItem::Id(target.clone())),
        Stmt::Subgraph(_)
        | Stmt::Style(_)
        | Stmt::ClassDef(_)
        | Stmt::ClassAssign(_)
        | Stmt::Click(_)
        | Stmt::LinkStyle(_) => {}
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn subgraphs_exist(subgraphs: &[FlowSubGraph], node_id: &str) -> bool {
    subgraphs
        .iter()
        .any(|sg| sg.nodes.iter().any(|n| n == node_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MermaidConfig, SemanticMetadata};

    #[test]
    fn flowchart_claimed_members_equal_completed_canonical_membership() {
        let meta = SemanticMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        };
        let control = OperationControl::new();
        let mut builder = SubgraphBuilder::new(false, Some("TB".to_string()));
        for body in [
            "subgraph X\nX\nend\n",
            "subgraph owner\nX & A\nend\n",
            "subgraph X\nB\nend\n",
            "subgraph sibling\nX & A & B & C\nend\n",
        ] {
            let ast =
                super::super::parse_flowchart_ast(&format!("flowchart TB\n{body}"), &meta).unwrap();
            builder.visit_statements(&ast, &control).unwrap();
            let retained = builder
                .subgraphs
                .iter()
                .enumerate()
                .flat_map(|(index, subgraph)| {
                    subgraph.nodes.iter().cloned().map(move |id| (id, index))
                })
                .collect::<HashMap<_, _>>();
            assert_eq!(builder.claimed_members, retained);
        }
        assert_eq!(builder.subgraphs.len(), 3);
        assert_eq!(builder.declaration_owners, [0, 1, 0, 2]);
        assert_eq!(builder.subgraphs[0].nodes, ["B"]);
        assert_eq!(builder.subgraphs[1].nodes, ["X", "A"]);
        assert_eq!(builder.subgraphs[2].nodes, ["C"]);
    }

    #[test]
    fn flowchart_interrupted_membership_completion_keeps_only_retained_claims() {
        let meta = SemanticMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        };
        let nodes = (0..256)
            .map(|index| format!("n{index}"))
            .collect::<Vec<_>>()
            .join(" & ");
        let source = format!("flowchart TB\nsubgraph group\n{nodes}\nend\n");
        let ast = super::super::parse_flowchart_ast(&source, &meta).unwrap();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(6);
        let mut builder = SubgraphBuilder::new(false, Some("TB".to_string()));
        assert!(matches!(
            builder.visit_statements(&ast, &control),
            Err(crate::OperationCancelled { .. })
        ));
        assert_eq!(builder.subgraphs.len(), 1);
        assert_eq!(builder.subgraphs[0].nodes.len(), 128);
        assert!(builder.declaration_owners.is_empty());
        assert_eq!(
            builder.claimed_members,
            builder.subgraphs[0]
                .nodes
                .iter()
                .cloned()
                .map(|id| (id, 0))
                .collect::<HashMap<_, _>>()
        );
    }
}
