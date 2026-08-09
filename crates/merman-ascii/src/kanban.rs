use crate::Result;
use crate::error::AsciiError;
use crate::options::AsciiRenderOptions;
use crate::resource::AsciiResourceLimitPhase;
use crate::safe_text::BudgetedTextDocument;
use merman_core::diagrams::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};
use std::collections::{HashMap, HashSet};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_kanban_diagram(
    model: &KanbanDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);
    let mut group_ids = HashSet::new();
    let mut node_ids = HashSet::new();
    let mut children_by_parent: HashMap<&str, Vec<&KanbanRenderNode>> = HashMap::new();
    document
        .resources_mut()
        .charge_layout_work(model.nodes.len())?;
    node_ids
        .try_reserve(model.nodes.len())
        .map_err(|_| layout_allocation_error())?;
    for node in &model.nodes {
        document.preflight_text_work(&node.id)?;
        if node.id.is_empty() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "kanban",
                feature: "empty node ids",
            });
        }
        if !node_ids.insert(node.id.as_str()) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "kanban",
                feature: "duplicate node ids",
            });
        }
    }
    group_ids
        .try_reserve(model.nodes.len())
        .map_err(|_| layout_allocation_error())?;
    for group in model.nodes.iter().filter(|node| node.is_group) {
        document.preflight_text_work(&group.id)?;
        group_ids.insert(group.id.as_str());
    }

    document
        .resources_mut()
        .charge_layout_work(model.nodes.len())?;
    children_by_parent
        .try_reserve(model.nodes.len())
        .map_err(|_| layout_allocation_error())?;
    for node in model.nodes.iter().filter(|node| !node.is_group) {
        document.resources_mut().charge_layout_work(1)?;
        if let Some(parent_id) = node.parent_id.as_deref() {
            document.preflight_text_work(parent_id)?;
            if group_ids.contains(parent_id) {
                let children = children_by_parent.entry(parent_id).or_default();
                children
                    .try_reserve(1)
                    .map_err(|_| layout_allocation_error())?;
                children.push(node);
            }
        }
    }

    let has_groups = !group_ids.is_empty();
    for group in model.nodes.iter().filter(|node| node.is_group) {
        document.resources_mut().charge_layout_work(1)?;
        document.push_wrapped_prefixed_line_with("", "", SUMMARY_WRAP_WIDTH, |line| {
            push_node_text(line, group, None)
        })?;
        if let Some(children) = children_by_parent.get(group.id.as_str()) {
            for child in children {
                document.resources_mut().charge_layout_work(1)?;
                document.push_wrapped_prefixed_line_with(
                    "  - ",
                    "    ",
                    SUMMARY_WRAP_WIDTH,
                    |line| push_node_text(line, child, None),
                )?;
            }
        }
    }

    if has_groups {
        let mut emitted_unassigned_heading = false;
        for node in model.nodes.iter().filter(|node| !node.is_group) {
            let parent_id = node.parent_id.as_deref();
            if parent_id.is_some_and(|parent_id| group_ids.contains(parent_id)) {
                continue;
            }
            if !emitted_unassigned_heading {
                document.push_line("Unassigned")?;
                emitted_unassigned_heading = true;
            }
            document.resources_mut().charge_layout_work(1)?;
            document.push_wrapped_prefixed_line_with(
                "  - ",
                "    ",
                SUMMARY_WRAP_WIDTH,
                |line| push_node_text(line, node, parent_id),
            )?;
        }
    } else {
        for node in &model.nodes {
            if !node.is_group {
                document.resources_mut().charge_layout_work(1)?;
                document.push_wrapped_prefixed_line_with(
                    "- ",
                    "  ",
                    SUMMARY_WRAP_WIDTH,
                    |line| push_node_text(line, node, node.parent_id.as_deref()),
                )?;
            }
        }
    }

    document.finish(options)
}

fn push_node_text(
    line: &mut crate::safe_text::BudgetedWrappedText<'_, '_>,
    node: &KanbanRenderNode,
    disclosed_parent: Option<&str>,
) -> Result<()> {
    line.push_str(&node.label)?;
    let metadata = [
        ("id=", Some(node.id.as_str())),
        ("parent=", disclosed_parent),
        ("ticket=", node.ticket.as_deref()),
        ("priority=", node.priority.as_deref()),
        ("assigned=", node.assigned.as_deref()),
        ("icon=", node.icon.as_deref()),
    ];
    let mut emitted = false;
    for (key, value) in metadata
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
    {
        line.push_str(if emitted { ", " } else { " [" })?;
        line.push_str(key)?;
        line.push_str(value)?;
        emitted = true;
    }
    if emitted {
        line.push_str("]")?;
    }
    Ok(())
}

fn layout_allocation_error() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}
