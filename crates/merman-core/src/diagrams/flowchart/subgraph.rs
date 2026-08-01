use super::{FlowSubGraph, Stmt, SubgraphBlock, TitleKind, strip_wrapping_backticks, unquote};
use crate::{ParseControl, ParseControlResult};
use std::collections::HashSet;

#[derive(Debug, Clone)]
enum StatementItem {
    Id(String),
    Dir(String),
}

struct EvalFrame<'a> {
    statements: &'a [Stmt],
    index: usize,
    subgraph: Option<&'a SubgraphBlock>,
    items: Vec<StatementItem>,
}

pub(super) struct SubgraphBuilder {
    sub_count: usize,
    pub(super) subgraphs: Vec<FlowSubGraph>,
    inherit_dir: bool,
    global_dir: Option<String>,
}

impl SubgraphBuilder {
    pub(super) fn new(inherit_dir: bool, global_dir: Option<String>) -> Self {
        Self {
            sub_count: 0,
            subgraphs: Vec::new(),
            inherit_dir,
            global_dir,
        }
    }

    pub(super) fn visit_statements(
        &mut self,
        statements: &[Stmt],
        control: &ParseControl,
    ) -> ParseControlResult<()> {
        let _ = self.eval_statements(statements, control)?;
        Ok(())
    }

    fn eval_statements(
        &mut self,
        statements: &[Stmt],
        control: &ParseControl,
    ) -> ParseControlResult<Vec<StatementItem>> {
        enum EvalStep<'a> {
            Statement(&'a Stmt),
            Finish,
        }

        let mut stack = vec![EvalFrame {
            statements,
            index: 0,
            subgraph: None,
            items: Vec::new(),
        }];
        let mut root_items = Vec::new();
        let mut visited = 0usize;

        while !stack.is_empty() {
            if visited.is_multiple_of(128) {
                control.checkpoint()?;
            }
            visited = visited.saturating_add(1);
            let step = {
                let Some(frame) = stack.last_mut() else {
                    return Ok(root_items);
                };
                if frame.index >= frame.statements.len() {
                    EvalStep::Finish
                } else {
                    let stmt = &frame.statements[frame.index];
                    frame.index += 1;
                    EvalStep::Statement(stmt)
                }
            };

            match step {
                EvalStep::Statement(Stmt::Subgraph(sg)) => stack.push(EvalFrame {
                    statements: &sg.statements,
                    index: 0,
                    subgraph: Some(sg),
                    items: Vec::new(),
                }),
                EvalStep::Statement(stmt) => {
                    if let Some(frame) = stack.last_mut()
                        && frame.subgraph.is_some()
                    {
                        push_statement_items(&mut frame.items, stmt, control)?;
                    }
                }
                EvalStep::Finish => {
                    let Some(frame) = stack.pop() else {
                        return Ok(root_items);
                    };
                    if let Some(sg) = frame.subgraph {
                        let id = self.eval_subgraph_from_items(sg, frame.items, control)?;
                        if let Some(parent) = stack.last_mut() {
                            parent.items.push(StatementItem::Id(id));
                        }
                    } else {
                        root_items = frame.items;
                    }
                }
            }
        }

        control.checkpoint()?;
        Ok(root_items)
    }

    fn eval_subgraph_from_items(
        &mut self,
        sg: &SubgraphBlock,
        items: Vec<StatementItem>,
        control: &ParseControl,
    ) -> ParseControlResult<String> {
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
                    if id.trim().is_empty() {
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

        let raw_id = sg.header.raw_id.trim();
        let (title_raw, title_kind) =
            parse_subgraph_title(&sg.header.raw_title, sg.header.id_equals_title);
        let id_raw = if raw_id.starts_with('"') && raw_id.ends_with('"') {
            // Only a double-quoted header enters Mermaid's string state. Markdown backticks are
            // meaningful after that quote has been removed; bare backticks stay in the id.
            let unquoted = unquote(raw_id);
            strip_wrapping_backticks(unquoted.trim()).0
        } else {
            raw_id.to_string()
        };

        let mut id: Option<String> = {
            let trimmed = id_raw.trim();
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
        if sg.header.id_equals_title && sg.header.raw_title.chars().any(|c| c.is_whitespace()) {
            id = None;
        }

        let id = id.unwrap_or_else(|| format!("subGraph{}", self.sub_count));
        let title = title_raw.trim().to_string();
        let label_type = match title_kind {
            TitleKind::Text => "text",
            TitleKind::String => "string",
            TitleKind::Markdown => "markdown",
        }
        .to_string();

        self.sub_count += 1;

        let mut nested_members = HashSet::new();
        for (subgraph_index, subgraph) in self.subgraphs.iter().enumerate() {
            if subgraph_index % 128 == 0 {
                control.checkpoint()?;
            }
            for (member_index, member) in subgraph.nodes.iter().enumerate() {
                if member_index % 128 == 0 {
                    control.checkpoint()?;
                }
                nested_members.insert(member.as_str());
            }
        }
        let mut retained_members = Vec::with_capacity(members.len());
        for (index, member) in members.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if !nested_members.contains(member.as_str()) {
                retained_members.push(member);
            }
        }

        self.subgraphs.push(FlowSubGraph {
            id: id.clone(),
            nodes: retained_members,
            title,
            classes: Vec::new(),
            styles: Vec::new(),
            dir,
            has_explicit_dir,
            label_type,
        });

        Ok(id)
    }
}

fn push_statement_items(
    out: &mut Vec<StatementItem>,
    stmt: &Stmt,
    control: &ParseControl,
) -> ParseControlResult<()> {
    match stmt {
        Stmt::Chain { nodes, edges } => {
            // Mermaid FlowDB's subgraph membership list is based on the Jison
            // `vertexStatement.nodes` shape, which prepends the last node in a chain first
            // (e.g. `a-->b` yields `[b, a]`).
            //
            // For node-only group statements (e.g. `A & B`), there are no edges and the list
            // preserves the input order.
            if edges.is_empty() {
                for (index, n) in nodes.iter().enumerate() {
                    if index % 128 == 0 {
                        control.checkpoint()?;
                    }
                    out.push(StatementItem::Id(n.id.clone()));
                }
            } else {
                for (index, n) in nodes.iter().rev().enumerate() {
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

fn parse_subgraph_title(raw_title: &str, id_equals_title: bool) -> (String, TitleKind) {
    let trimmed = raw_title.trim();
    let quoted = trimmed.starts_with('"') && trimmed.ends_with('"');
    let unquoted = if quoted {
        // Keep flowchart subgraph titles raw (strip only surrounding quotes).
        // This matches upstream and avoids mangling backslash-heavy labels.
        unquote(trimmed)
    } else {
        trimmed.to_string()
    };

    if quoted {
        let (no_backticks, is_markdown) = strip_wrapping_backticks(unquoted.trim());
        if is_markdown {
            return (no_backticks, TitleKind::Markdown);
        }
    }

    if !id_equals_title && quoted {
        return (unquoted, TitleKind::String);
    }

    (unquoted, TitleKind::Text)
}
