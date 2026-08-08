use crate::Result;
use crate::error::AsciiError;
use crate::options::AsciiRenderOptions;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{BudgetedTextDocument, charge_text_layout};
use merman_core::diagrams::mindmap::{
    MindmapDiagramRenderEdge, MindmapDiagramRenderModel, MindmapDiagramRenderNode,
};
use std::collections::{HashMap, HashSet};

const SUMMARY_WRAP_WIDTH: usize = 80;
const BRANCH: &str = "|-- ";
const CONTINUE: &str = "|   ";
const EMPTY: &str = "    ";

pub fn render_mindmap_diagram(
    model: &MindmapDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);
    let nodes_by_id = index_nodes(&model.nodes, document.resources_mut())?;
    let children_by_id = build_children_map(&model.edges, document.resources_mut())?;
    let roots = root_ids(&model.nodes, &model.edges, document.resources_mut())?;

    for (index, root_id) in roots.iter().enumerate() {
        if index > 0 {
            document.push_line("")?;
        }
        let Some(root) = nodes_by_id.get(*root_id) else {
            continue;
        };

        let mut visiting = HashSet::new();
        document
            .resources_mut()
            .charge_layout_work(nodes_by_id.len())?;
        visiting
            .try_reserve(nodes_by_id.len())
            .map_err(|_| layout_allocation_error())?;
        let mut stack = Vec::new();
        push_enter_frame(
            &mut stack,
            MindmapEnterFrame {
                node: root,
                prefix: String::new(),
                is_last: true,
                depth: 1,
                is_root: true,
            },
            document.resources_mut(),
        )?;

        while let Some(frame) = stack.pop() {
            match frame {
                MindmapFrame::Exit(node_id) => {
                    visiting.remove(node_id);
                }
                MindmapFrame::Enter(frame) => {
                    let MindmapEnterFrame {
                        node,
                        prefix,
                        is_last,
                        depth,
                        is_root,
                    } = frame;
                    let is_cycle = visiting.contains(node.id.as_str());
                    let branch = if is_root {
                        String::new()
                    } else {
                        branch_prefix(&prefix, is_last)
                    };
                    push_wrapped_label(&mut document, &branch, |line| {
                        line.push_str(&node.label)?;
                        if is_cycle {
                            line.push_str(" (cycle)")?;
                        }
                        Ok(())
                    })?;

                    if is_cycle {
                        continue;
                    }
                    visiting.insert(node.id.as_str());
                    push_exit_frame(&mut stack, node.id.as_str(), document.resources_mut())?;

                    let Some(children) = children_by_id.get(node.id.as_str()) else {
                        continue;
                    };
                    let child_depth = depth.checked_add(1).ok_or_else(|| {
                        document
                            .resources_mut()
                            .policy()
                            .overflow(AsciiResourceLimitId::MaxNestingDepth)
                    })?;
                    document.resources_mut().check_nesting_depth(child_depth)?;
                    let next_prefix = if is_root {
                        String::new()
                    } else {
                        child_prefix(&prefix, is_last)
                    };

                    for (child_index, child_id) in children.iter().enumerate().rev() {
                        let Some(child) = nodes_by_id.get(*child_id) else {
                            continue;
                        };
                        document
                            .resources_mut()
                            .charge_layout_work(next_prefix.len())?;
                        push_enter_frame(
                            &mut stack,
                            MindmapEnterFrame {
                                node: child,
                                prefix: next_prefix.clone(),
                                is_last: child_index + 1 == children.len(),
                                depth: child_depth,
                                is_root: false,
                            },
                            document.resources_mut(),
                        )?;
                    }
                }
            }
        }
    }

    document.finish(options)
}

fn index_nodes<'a>(
    nodes: &'a [MindmapDiagramRenderNode],
    resources: &mut ResourceContext,
) -> Result<HashMap<&'a str, &'a MindmapDiagramRenderNode>> {
    resources.charge_layout_work(nodes.len())?;
    let mut out = HashMap::new();
    try_reserve_hash_map(&mut out, nodes.len())?;
    for node in nodes {
        charge_text_layout(resources, &node.id)?;
        out.insert(node.id.as_str(), node);
    }
    Ok(out)
}

fn build_children_map<'a>(
    edges: &'a [MindmapDiagramRenderEdge],
    resources: &mut ResourceContext,
) -> Result<HashMap<&'a str, Vec<&'a str>>> {
    resources.charge_layout_work(edges.len())?;
    let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
    try_reserve_hash_map(&mut children, edges.len())?;
    for edge in edges {
        charge_text_layout(resources, &edge.start)?;
        charge_text_layout(resources, &edge.end)?;
        let siblings = children.entry(edge.start.as_str()).or_default();
        resources.charge_layout_work(siblings.len())?;
        if !siblings.contains(&edge.end.as_str()) {
            siblings
                .try_reserve(1)
                .map_err(|_| layout_allocation_error())?;
            siblings.push(edge.end.as_str());
        }
    }
    Ok(children)
}

fn root_ids<'a>(
    nodes: &'a [MindmapDiagramRenderNode],
    edges: &'a [MindmapDiagramRenderEdge],
    resources: &mut ResourceContext,
) -> Result<Vec<&'a str>> {
    resources.charge_layout_work(edges.len())?;
    let mut incoming = HashSet::new();
    incoming
        .try_reserve(edges.len())
        .map_err(|_| layout_allocation_error())?;
    for edge in edges {
        charge_text_layout(resources, &edge.end)?;
        incoming.insert(edge.end.as_str());
    }

    let mut roots = Vec::new();
    for node in nodes {
        resources.charge_layout_work(1)?;
        charge_text_layout(resources, &node.id)?;
        if !incoming.contains(node.id.as_str()) {
            roots
                .try_reserve(1)
                .map_err(|_| layout_allocation_error())?;
            roots.push(node.id.as_str());
        }
    }

    if roots.is_empty()
        && let Some(node) = nodes.first()
    {
        roots
            .try_reserve(1)
            .map_err(|_| layout_allocation_error())?;
        roots.push(node.id.as_str());
    }

    Ok(roots)
}

enum MindmapFrame<'a> {
    Enter(MindmapEnterFrame<'a>),
    Exit(&'a str),
}

struct MindmapEnterFrame<'a> {
    node: &'a MindmapDiagramRenderNode,
    prefix: String,
    is_last: bool,
    depth: usize,
    is_root: bool,
}

fn push_enter_frame<'a>(
    stack: &mut Vec<MindmapFrame<'a>>,
    frame: MindmapEnterFrame<'a>,
    resources: &mut ResourceContext,
) -> Result<()> {
    resources.check_nesting_depth(frame.depth)?;
    resources.charge_layout_work(1)?;
    stack
        .try_reserve(1)
        .map_err(|_| layout_allocation_error())?;
    stack.push(MindmapFrame::Enter(frame));
    Ok(())
}

fn push_exit_frame<'a>(
    stack: &mut Vec<MindmapFrame<'a>>,
    node_id: &'a str,
    resources: &mut ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(1)?;
    stack
        .try_reserve(1)
        .map_err(|_| layout_allocation_error())?;
    stack.push(MindmapFrame::Exit(node_id));
    Ok(())
}

fn branch_prefix(prefix: &str, is_last: bool) -> String {
    if prefix.is_empty() {
        if is_last {
            "\\-- ".to_string()
        } else {
            BRANCH.to_string()
        }
    } else if is_last {
        format!("{prefix}\\-- ")
    } else {
        format!("{prefix}{BRANCH}")
    }
}

fn child_prefix(prefix: &str, is_last: bool) -> String {
    if prefix.is_empty() {
        if is_last {
            EMPTY.to_string()
        } else {
            CONTINUE.to_string()
        }
    } else if is_last {
        format!("{prefix}{EMPTY}")
    } else {
        format!("{prefix}{CONTINUE}")
    }
}

fn push_wrapped_label(
    document: &mut BudgetedTextDocument,
    prefix: &str,
    render: impl FnOnce(&mut crate::safe_text::BudgetedWrappedText<'_, '_>) -> Result<()>,
) -> Result<()> {
    let continuation_width = prefix.len();
    document
        .resources_mut()
        .charge_layout_work(continuation_width)?;
    let continuation_prefix = " ".repeat(continuation_width);
    document.push_wrapped_prefixed_line_with(
        prefix,
        &continuation_prefix,
        SUMMARY_WRAP_WIDTH,
        render,
    )
}

fn try_reserve_hash_map<K, V>(map: &mut HashMap<K, V>, additional: usize) -> Result<()>
where
    K: Eq + std::hash::Hash,
{
    map.try_reserve(additional)
        .map_err(|_| layout_allocation_error())
}

fn layout_allocation_error() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;

    const DEEP_NESTING: usize = 256;

    fn node(id: &str, label: &str, level: i64) -> MindmapDiagramRenderNode {
        MindmapDiagramRenderNode {
            id: id.to_string(),
            dom_id: format!("node_{id}"),
            label: label.to_string(),
            label_type: String::new(),
            is_group: false,
            shape: "defaultMindmapNode".to_string(),
            width: 40.0,
            height: 24.0,
            padding: 10.0,
            css_classes: String::new(),
            css_styles: Vec::new(),
            look: "classic".to_string(),
            icon: None,
            x: None,
            y: None,
            level,
            node_id: id.to_string(),
            node_type: 0,
            section: None,
        }
    }

    fn edge(id: &str, start: &str, end: &str) -> MindmapDiagramRenderEdge {
        MindmapDiagramRenderEdge {
            id: id.to_string(),
            start: start.to_string(),
            end: end.to_string(),
            edge_type: String::new(),
            curve: String::new(),
            thickness: String::new(),
            look: String::new(),
            classes: String::new(),
            depth: 0,
            section: None,
        }
    }

    fn chain(depth: usize) -> MindmapDiagramRenderModel {
        let mut nodes = Vec::with_capacity(depth);
        let mut edges = Vec::with_capacity(depth.saturating_sub(1));
        for index in 0..depth {
            let id = format!("node-{index}");
            let label = if index + 1 == depth {
                "Leaf".to_string()
            } else {
                format!("Level {index}")
            };
            nodes.push(node(&id, &label, index as i64));
            if index > 0 {
                let parent = format!("node-{}", index - 1);
                edges.push(edge(&format!("{parent}-{id}"), &parent, &id));
            }
        }
        MindmapDiagramRenderModel { nodes, edges }
    }

    fn options_with_nesting_limit(limit: usize) -> AsciiRenderOptions {
        let resources = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxNestingDepth, limit)
            .expect("positive nesting limit");
        AsciiRenderOptions::ascii().with_resource_policy(resources)
    }

    #[test]
    fn mindmap_accepts_exact_nesting_limit() {
        let rendered = render_mindmap_diagram(
            &chain(DEEP_NESTING),
            &options_with_nesting_limit(DEEP_NESTING),
        )
        .expect("deep nesting equal to the limit should render iteratively");

        assert!(rendered.contains("Leaf"));
    }

    #[test]
    fn mindmap_rejects_limit_minus_one_before_descending() {
        let error = render_mindmap_diagram(
            &chain(DEEP_NESTING),
            &options_with_nesting_limit(DEEP_NESTING - 1),
        )
        .expect_err("deep nesting above the limit should fail before the final descent");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxNestingDepth
                    && details.actual == DEEP_NESTING
                    && details.max == DEEP_NESTING - 1
        ));
    }
}
