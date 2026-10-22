use crate::Result;
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::resource::AsciiResourceLimitPhase;
use crate::safe_text::{BudgetedTextDocument, push_wrapped_field};
use merman_core::diagrams::kanban::{KanbanTerminalNode, KanbanTerminalView};
use std::collections::{HashMap, HashSet};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub(super) fn render_kanban_diagram(
    model: KanbanTerminalView<'_>,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let layout_resources = execution.new_resource_context(merman_core::OperationPhase::Layout);
    let mut document = BudgetedTextDocument::from_resources(layout_resources, options);
    let mut group_ids = HashSet::new();
    let mut node_ids = HashSet::new();
    let mut children_by_parent: HashMap<&str, Vec<usize>> = HashMap::new();
    document
        .resources_mut()
        .charge_layout_work(model.nodes().len())?;
    node_ids
        .try_reserve(model.nodes().len())
        .map_err(|_| layout_allocation_error())?;
    for node in model.nodes() {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
        document.preflight_text_work(node.id)?;
        if node.id.is_empty() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "kanban",
                feature: "empty node ids",
            });
        }
        if !node_ids.insert(node.id) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "kanban",
                feature: "duplicate node ids",
            });
        }
    }
    group_ids
        .try_reserve(model.nodes().len())
        .map_err(|_| layout_allocation_error())?;
    for group in model.nodes().filter(|node| node.is_group) {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
        document.preflight_text_work(group.id)?;
        if let Some(parent_id) = group.parent_id {
            document.preflight_text_work(parent_id)?;
        }
        group_ids.insert(group.id);
    }

    document
        .resources_mut()
        .charge_layout_work(model.nodes().len())?;
    children_by_parent
        .try_reserve(model.nodes().len())
        .map_err(|_| layout_allocation_error())?;
    for (node_index, node) in model.nodes().enumerate().filter(|(_, node)| !node.is_group) {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
        document.resources_mut().charge_layout_work(1)?;
        if let Some(parent_id) = node.parent_id {
            document.preflight_text_work(parent_id)?;
            if group_ids.contains(parent_id) {
                let children = children_by_parent.entry(parent_id).or_default();
                children
                    .try_reserve(1)
                    .map_err(|_| layout_allocation_error())?;
                children.push(node_index);
            }
        }
    }

    execution.rebind_resource_context(document.resources_mut(), merman_core::OperationPhase::Emit);

    let has_groups = !group_ids.is_empty();
    for group in model.nodes().filter(|node| node.is_group) {
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
        document.resources_mut().charge_layout_work(1)?;
        document.push_wrapped_prefixed_line_with("", "", SUMMARY_WRAP_WIDTH, |line| {
            push_node_text(line, group, group.parent_id)
        })?;
        if let Some(children) = children_by_parent.get(group.id) {
            for child in children {
                let child = model.node(*child).ok_or_else(layout_allocation_error)?;
                execution.checkpoint(merman_core::OperationPhase::Emit)?;
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
        for node in model.nodes().filter(|node| !node.is_group) {
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            let parent_id = node.parent_id;
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
        for node in model.nodes() {
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            if !node.is_group {
                document.resources_mut().charge_layout_work(1)?;
                document.push_wrapped_prefixed_line_with(
                    "- ",
                    "  ",
                    SUMMARY_WRAP_WIDTH,
                    |line| push_node_text(line, node, node.parent_id),
                )?;
            }
        }
    }

    document.finish_with_execution(execution)
}

fn push_node_text(
    line: &mut crate::safe_text::BudgetedWrappedText<'_>,
    node: KanbanTerminalNode<'_>,
    disclosed_parent: Option<&str>,
) -> Result<()> {
    push_wrapped_field(
        line,
        "",
        if node.is_group { "group" } else { "card" },
        node.label,
    )?;
    let metadata = [
        ("id", Some(node.id)),
        ("parent", disclosed_parent),
        ("ticket", node.ticket),
        ("priority", node.priority),
        ("assigned", node.assigned),
        ("icon", node.icon),
    ];
    let mut emitted = false;
    for (key, value) in metadata
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
    {
        push_wrapped_field(line, if emitted { ", " } else { " [" }, key, value)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::AsciiResourcePolicy;

    #[test]
    fn canonical_kanban_retains_authored_metadata_and_matches_full_terminal_output() {
        use merman_core::{
            MermaidEngine, ParseOptions, RenderSemanticModel, SemanticDiagramModel, SemanticParser,
        };

        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default();
        for source in [
            "kanban\n  todo[Todo]\n    card[Caption]@{ ticket: MC-1, priority: high, assigned: owner, custom: [1, 2] }\n    ::icon(star)\n    :::highlight\n  done[Done]\n",
            "kanban\n  empty[Empty]\n",
            "kanban\n",
        ] {
            let canonical = SemanticParser::new().parse_diagram_sync(source).unwrap();
            let SemanticDiagramModel::Kanban(model) = &canonical.model else {
                panic!("Kanban model")
            };
            let view = KanbanTerminalView::from_model(model);
            assert_eq!(view.nodes().len(), model.nodes.len());
            for (node, canonical) in view.nodes().zip(&model.nodes) {
                assert!(std::ptr::eq(node.authored().unwrap(), canonical));
                assert_eq!(node.id, canonical.id);
            }
            assert!(view.node(model.nodes.len()).is_none());
            let full = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let RenderSemanticModel::Kanban(full) = full.model() else {
                panic!("Kanban model")
            };
            let actual =
                render_kanban_diagram(view, &options, AsciiExecution::for_test(&policy)).unwrap();
            let expected = render_kanban_diagram(
                KanbanTerminalView::from_render(full),
                &options,
                AsciiExecution::for_test(&policy),
            )
            .unwrap();
            assert_eq!(actual, expected, "{source}");
        }
    }
}
