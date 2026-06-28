use crate::options::AsciiRenderOptions;
use merman_core::diagrams::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};
use std::collections::HashMap;

pub fn render_kanban_diagram(
    model: &KanbanDiagramRenderModel,
    _options: &AsciiRenderOptions,
) -> String {
    let mut lines = Vec::new();
    let groups: Vec<&KanbanRenderNode> = model.nodes.iter().filter(|node| node.is_group).collect();
    let mut children_by_parent: HashMap<&str, Vec<&KanbanRenderNode>> = HashMap::new();
    for node in model.nodes.iter().filter(|node| !node.is_group) {
        if let Some(parent_id) = node.parent_id.as_deref() {
            children_by_parent.entry(parent_id).or_default().push(node);
        }
    }

    for group in groups {
        lines.push(group.label.clone());
        if let Some(children) = children_by_parent.get(group.id.as_str()) {
            for child in children {
                lines.push(format!("  - {}{}", child.label, render_metadata(child)));
            }
        }
    }

    if lines.is_empty() {
        for node in &model.nodes {
            if !node.is_group {
                lines.push(format!("- {}{}", node.label, render_metadata(node)));
            }
        }
    }

    trim_trailing_blank_lines(lines).join("\n")
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

fn trim_trailing_blank_lines(mut lines: Vec<String>) -> Vec<String> {
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines
}
