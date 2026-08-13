use crate::Result;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::text::{push_wrapped_prefixed_line, trim_trailing_blank_lines};
use merman_core::diagrams::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};
use std::collections::HashMap;

const SUMMARY_WRAP_WIDTH: usize = 80;

pub(super) fn render_kanban_diagram(
    model: &KanbanDiagramRenderModel,
    _options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let mut lines = Vec::new();
    let mut groups = Vec::new();
    let mut children_by_parent: HashMap<&str, Vec<&KanbanRenderNode>> = HashMap::new();
    for node in &model.nodes {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
        if node.is_group {
            groups.push(node);
        } else if let Some(parent_id) = node.parent_id.as_deref() {
            children_by_parent.entry(parent_id).or_default().push(node);
        }
    }

    for group in groups {
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
        lines.push(group.label.clone());
        if let Some(children) = children_by_parent.get(group.id.as_str()) {
            for child in children {
                execution.checkpoint(merman_core::OperationPhase::Emit)?;
                push_wrapped_prefixed_line(
                    &mut lines,
                    "  - ",
                    "    ",
                    &format!("{}{}", child.label, render_metadata(child)),
                    SUMMARY_WRAP_WIDTH,
                );
            }
        }
    }

    if lines.is_empty() {
        for node in &model.nodes {
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            if !node.is_group {
                push_wrapped_prefixed_line(
                    &mut lines,
                    "- ",
                    "  ",
                    &format!("{}{}", node.label, render_metadata(node)),
                    SUMMARY_WRAP_WIDTH,
                );
            }
        }
    }

    Ok(trim_trailing_blank_lines(lines).join("\n"))
}

fn render_metadata(node: &KanbanRenderNode) -> String {
    let mut parts = Vec::new();
    if let Some(ticket) = &node.ticket {
        parts.push(format!("ticket={ticket}"));
    }
    if let Some(priority) = &node.priority {
        parts.push(format!("priority={priority}"));
    }
    if let Some(assigned) = &node.assigned {
        parts.push(format!("assigned={assigned}"));
    }
    if let Some(icon) = &node.icon {
        parts.push(format!("icon={icon}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", parts.join(", "))
    }
}
