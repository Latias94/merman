use super::{MindmapView, NodeId, NodeView, layout_allocation_error, push_wrapped_label};
use crate::Result;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::{
    BudgetedTextDocument, BudgetedWrappedText, charge_text_layout, push_wrapped_field,
    terminal_single_line_text_requires_normalization,
};
use crate::text::display_width_with_profile;
use merman_model::families::mindmap::MindmapDisplayPolicy;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(super) struct NodePresentation {
    quoted_label: bool,
    label_width: usize,
    section: Option<i32>,
    visited: bool,
}

impl NodePresentation {
    fn has_details(self, node: NodeView<'_>) -> bool {
        node.node_id != node.label
            || has_shape(node)
            || node.icon.is_some()
            || self.section.is_some()
    }
}

struct SectionFrame {
    node_index: usize,
    depth: usize,
    inherited: Option<i32>,
    expected_inherited: Option<i32>,
}

pub(super) fn plan_node_presentations<'a>(
    model: MindmapView<'a>,
    roots: &[NodeId<'a>],
    nodes_by_id: &HashMap<NodeId<'a>, usize>,
    children_by_id: &HashMap<NodeId<'a>, Vec<NodeId<'a>>>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<NodePresentation>> {
    resources.charge_layout_work(model.node_count())?;
    let mut presentations = Vec::new();
    presentations
        .try_reserve_exact(model.node_count())
        .map_err(|_| layout_allocation_error())?;
    for node in model.nodes() {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        charge_text_layout(resources, node.label)?;
        presentations.push(NodePresentation {
            quoted_label: label_needs_quoting(node.label, resources)?,
            label_width: display_width_with_profile(node.label, options.terminal_width_profile),
            section: None,
            visited: false,
        });
    }

    let default_sections = MindmapDisplayPolicy::from_config(&Default::default());
    let mut sections_follow_tree = true;
    let mut stack = Vec::new();
    for root in roots {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        let Some(&node_index) = nodes_by_id.get(root) else {
            continue;
        };
        push_section_frame(
            &mut stack,
            SectionFrame {
                node_index,
                depth: 1,
                inherited: None,
                expected_inherited: None,
            },
            resources,
        )?;
        while let Some(frame) = stack.pop() {
            execution.checkpoint(merman_model::OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            let presentation = &mut presentations[frame.node_index];
            if presentation.visited {
                continue;
            }
            presentation.visited = true;
            let node = model.node(frame.node_index)?;
            let depth = frame.depth - 1;
            let level = i32::from(depth != 0);
            presentation.section = model.section(node, frame.inherited, depth);
            sections_follow_tree &= presentation.section
                == default_sections.section(level, frame.expected_inherited, depth);
            let Some(children) = children_by_id.get(&node.id) else {
                continue;
            };
            let child_depth = frame.depth.checked_add(1).ok_or_else(|| {
                resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxNestingDepth)
            })?;
            for (child_index, child_id) in children.iter().enumerate().rev() {
                execution.checkpoint(merman_model::OperationPhase::Layout)?;
                resources.charge_layout_work(1)?;
                let Some(&node_index) = nodes_by_id.get(child_id) else {
                    continue;
                };
                push_section_frame(
                    &mut stack,
                    SectionFrame {
                        node_index,
                        depth: child_depth,
                        inherited: model.child_section(node, child_index, frame.inherited),
                        expected_inherited: default_sections.child_section(
                            level,
                            child_index,
                            frame.expected_inherited,
                        ),
                    },
                    resources,
                )?;
            }
        }
    }

    // Standard sections repeat the branch grouping already expressed by the outline.
    // Keep every value instead when any part of the graph differs from that rule.
    if sections_follow_tree {
        for presentation in &mut presentations {
            resources.charge_layout_work(1)?;
            presentation.section = None;
        }
    }
    Ok(presentations)
}

fn push_section_frame(
    stack: &mut Vec<SectionFrame>,
    frame: SectionFrame,
    resources: &ResourceContext,
) -> Result<()> {
    resources.check_nesting_depth(frame.depth)?;
    resources.charge_layout_work(1)?;
    stack
        .try_reserve(1)
        .map_err(|_| layout_allocation_error())?;
    stack.push(frame);
    Ok(())
}

fn label_needs_quoting(label: &str, resources: &ResourceContext) -> Result<bool> {
    let mut quote = label.is_empty()
        || label == "Details:"
        || label.starts_with("|-- ")
        || label.starts_with("├── ")
        || label.starts_with("└── ");
    let mut previous_space = true;
    for ch in label.chars() {
        resources.charge_layout_work(1)?;
        quote |= matches!(ch, '"' | '\\' | '[' | ']' | '(' | ')')
            || (ch.is_whitespace() && (ch != ' ' || previous_space));
        previous_space = ch == ' ';
    }
    quote |= previous_space;
    Ok(quote || terminal_single_line_text_requires_normalization(label, resources)?)
}

pub(super) fn push_node_text(
    line: &mut BudgetedWrappedText<'_>,
    node: NodeView<'_>,
    presentation: NodePresentation,
    available: usize,
    number: usize,
    is_cycle: bool,
) -> Result<()> {
    // Word wrapping can discard authored spaces or put a label's connector at a row start.
    // Quoted output preserves the exact text across physical continuation rows.
    if presentation.quoted_label || presentation.label_width > available {
        line.push_quoted_text(node.label)?;
    } else {
        line.push_str(node.label)?;
    }
    if presentation.has_details(node) {
        line.write_fmt(format_args!(" [{number}]"))?;
    }
    if is_cycle {
        line.push_str(" (cycle)")?;
    }
    Ok(())
}

pub(super) fn push_details(
    document: &mut BudgetedTextDocument,
    model: MindmapView<'_>,
    presentations: &[NodePresentation],
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    let mut started = false;
    for (index, node) in model.nodes().enumerate() {
        execution.checkpoint(merman_model::OperationPhase::Emit)?;
        document.resources_mut().charge_layout_work(1)?;
        let presentation = presentations[index];
        if !presentation.has_details(node) {
            continue;
        }
        if !started {
            document.push_line("")?;
            document.push_line("Details:")?;
            started = true;
        }
        push_wrapped_label(document, "  ", options, |line, _| {
            line.write_fmt(format_args!("[{}]", index + 1))?;
            if node.node_id != node.label {
                push_wrapped_field(line, " ", "id", node.node_id)?;
            }
            if has_shape(node) {
                push_wrapped_field(line, " ", "shape", node.shape)?;
            }
            if let Some(icon) = node.icon {
                push_wrapped_field(line, " ", "icon", icon)?;
            }
            if let Some(section) = presentation.section {
                line.write_fmt(format_args!(" section={section}"))?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn has_shape(node: NodeView<'_>) -> bool {
    !node.shape.is_empty() && node.shape != "defaultMindmapNode"
}
