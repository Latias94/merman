use crate::Result;
use crate::error::AsciiError;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{
    BudgetedTextDocument, charge_text_layout, push_wrapped_field, try_clone_layout_text,
    try_concat_layout_text, try_repeat_layout_char,
};
use crate::text::display_width_with_profile;
use merman_core::diagrams::mindmap::{
    MindmapDiagramRenderEdge, MindmapDiagramRenderModel, MindmapDiagramRenderNode,
};
use std::collections::{HashMap, HashSet};

const SUMMARY_WRAP_WIDTH: usize = 80;

#[derive(Clone, Copy)]
struct MindmapChars {
    branch: &'static str,
    last_branch: &'static str,
    child_continue: &'static str,
    child_empty: &'static str,
}

impl MindmapChars {
    fn from_options(options: &AsciiRenderOptions) -> Self {
        match options.structural_charset() {
            AsciiCharset::Ascii => Self {
                branch: "|-- ",
                last_branch: "\\-- ",
                child_continue: "|   ",
                child_empty: "    ",
            },
            AsciiCharset::Unicode => Self {
                branch: "├── ",
                last_branch: "└── ",
                child_continue: "│   ",
                child_empty: "    ",
            },
        }
    }
}

pub fn render_mindmap_diagram(
    model: &MindmapDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);
    let chars = MindmapChars::from_options(options);
    let nodes_by_id = index_nodes(&model.nodes, document.resources_mut())?;
    let children_by_id = build_children_map(&model.edges, &nodes_by_id, document.resources_mut())?;
    let mut roots = root_ids(&model.nodes, &model.edges, document.resources_mut())?;
    append_disconnected_component_roots(
        &model.nodes,
        &nodes_by_id,
        &children_by_id,
        &mut roots,
        document.resources_mut(),
    )?;

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
                        branch_prefix(&prefix, is_last, chars, document.resources_mut())?
                    };
                    push_wrapped_label(&mut document, &branch, options, |line| {
                        push_node_text(line, node, is_cycle)
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
                        child_prefix(&prefix, is_last, chars, document.resources_mut())?
                    };

                    for (child_index, child_id) in children.iter().enumerate().rev() {
                        let Some(child) = nodes_by_id.get(*child_id) else {
                            continue;
                        };
                        push_enter_frame(
                            &mut stack,
                            MindmapEnterFrame {
                                node: child,
                                prefix: try_clone_layout_text(
                                    &next_prefix,
                                    document.resources_mut(),
                                )?,
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
    let mut authored_ids = HashSet::new();
    try_reserve_hash_map(&mut out, nodes.len())?;
    authored_ids
        .try_reserve(nodes.len())
        .map_err(|_| layout_allocation_error())?;
    for node in nodes {
        charge_text_layout(resources, &node.id)?;
        charge_text_layout(resources, &node.node_id)?;
        if out.insert(node.id.as_str(), node).is_some() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "duplicate node ids",
            });
        }
        if node.node_id.is_empty() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "missing authored node ids",
            });
        }
        if !authored_ids.insert(node.node_id.as_str()) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "duplicate authored node ids",
            });
        }
    }
    Ok(out)
}

fn build_children_map<'a>(
    edges: &'a [MindmapDiagramRenderEdge],
    nodes_by_id: &HashMap<&'a str, &'a MindmapDiagramRenderNode>,
    resources: &mut ResourceContext,
) -> Result<HashMap<&'a str, Vec<&'a str>>> {
    resources.charge_layout_work(edges.len())?;
    let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
    try_reserve_hash_map(&mut children, edges.len())?;
    let mut edge_ids = HashSet::new();
    let mut endpoint_pairs = HashSet::new();
    edge_ids
        .try_reserve(edges.len())
        .map_err(|_| layout_allocation_error())?;
    endpoint_pairs
        .try_reserve(edges.len())
        .map_err(|_| layout_allocation_error())?;
    for edge in edges {
        charge_text_layout(resources, &edge.start)?;
        charge_text_layout(resources, &edge.end)?;
        if !nodes_by_id.contains_key(edge.start.as_str()) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "edge with missing start node",
            });
        }
        if !nodes_by_id.contains_key(edge.end.as_str()) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "edge with missing end node",
            });
        }
        if !edge.id.is_empty() && !edge_ids.insert(edge.id.as_str()) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "duplicate edge ids",
            });
        }
        if !endpoint_pairs.insert((edge.start.as_str(), edge.end.as_str())) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "parallel edges",
            });
        }
        let siblings = children.entry(edge.start.as_str()).or_default();
        resources.charge_layout_work(siblings.len())?;
        siblings
            .try_reserve(1)
            .map_err(|_| layout_allocation_error())?;
        siblings.push(edge.end.as_str());
    }
    Ok(children)
}

fn push_node_text(
    line: &mut crate::safe_text::BudgetedWrappedText<'_, '_>,
    node: &MindmapDiagramRenderNode,
    is_cycle: bool,
) -> Result<()> {
    push_wrapped_field(line, "", "node", &node.label)?;
    push_wrapped_field(line, " ", "id", &node.node_id)?;
    if !node.shape.is_empty() && node.shape != "defaultMindmapNode" {
        push_wrapped_field(line, " ", "shape", &node.shape)?;
    }
    if let Some(icon) = node.icon.as_deref() {
        push_wrapped_field(line, " ", "icon", icon)?;
    }
    if let Some(section) = node.section {
        line.write_fmt(format_args!(" section={section}"))?;
    }
    if is_cycle {
        line.push_str(" (cycle)")?;
    }
    Ok(())
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

fn append_disconnected_component_roots<'a>(
    nodes: &'a [MindmapDiagramRenderNode],
    nodes_by_id: &HashMap<&'a str, &'a MindmapDiagramRenderNode>,
    children_by_id: &HashMap<&'a str, Vec<&'a str>>,
    roots: &mut Vec<&'a str>,
    resources: &mut ResourceContext,
) -> Result<()> {
    let mut reachable = HashSet::new();
    reachable
        .try_reserve(nodes_by_id.len())
        .map_err(|_| layout_allocation_error())?;
    let mut stack = Vec::new();
    stack
        .try_reserve(nodes_by_id.len())
        .map_err(|_| layout_allocation_error())?;

    let initial_root_count = roots.len();
    for root_id in roots.iter().take(initial_root_count).copied() {
        mark_reachable_component(
            root_id,
            nodes_by_id,
            children_by_id,
            &mut reachable,
            &mut stack,
            resources,
        )?;
    }

    for node in nodes {
        resources.charge_layout_work(1)?;
        if reachable.contains(node.id.as_str()) {
            continue;
        }
        roots
            .try_reserve(1)
            .map_err(|_| layout_allocation_error())?;
        roots.push(node.id.as_str());
        mark_reachable_component(
            node.id.as_str(),
            nodes_by_id,
            children_by_id,
            &mut reachable,
            &mut stack,
            resources,
        )?;
    }

    Ok(())
}

fn mark_reachable_component<'a>(
    root_id: &'a str,
    nodes_by_id: &HashMap<&'a str, &'a MindmapDiagramRenderNode>,
    children_by_id: &HashMap<&'a str, Vec<&'a str>>,
    reachable: &mut HashSet<&'a str>,
    stack: &mut Vec<&'a str>,
    resources: &mut ResourceContext,
) -> Result<()> {
    if !nodes_by_id.contains_key(root_id) || !reachable.insert(root_id) {
        return Ok(());
    }
    resources.charge_layout_work(1)?;
    stack.push(root_id);

    while let Some(node_id) = stack.pop() {
        resources.charge_layout_work(1)?;
        let Some(children) = children_by_id.get(node_id) else {
            continue;
        };
        for child_id in children.iter().rev().copied() {
            resources.charge_layout_work(1)?;
            if nodes_by_id.contains_key(child_id) && reachable.insert(child_id) {
                stack.push(child_id);
            }
        }
    }

    Ok(())
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

fn branch_prefix(
    prefix: &str,
    is_last: bool,
    chars: MindmapChars,
    resources: &ResourceContext,
) -> Result<String> {
    try_concat_layout_text(
        prefix,
        if is_last {
            chars.last_branch
        } else {
            chars.branch
        },
        resources,
    )
}

fn child_prefix(
    prefix: &str,
    is_last: bool,
    chars: MindmapChars,
    resources: &ResourceContext,
) -> Result<String> {
    try_concat_layout_text(
        prefix,
        if is_last {
            chars.child_empty
        } else {
            chars.child_continue
        },
        resources,
    )
}

fn push_wrapped_label(
    document: &mut BudgetedTextDocument,
    prefix: &str,
    options: &AsciiRenderOptions,
    render: impl FnOnce(&mut crate::safe_text::BudgetedWrappedText<'_, '_>) -> Result<()>,
) -> Result<()> {
    let continuation_width = display_width_with_profile(prefix, options.terminal_width_profile);
    let continuation_prefix =
        try_repeat_layout_char(' ', continuation_width, document.resources_mut())?;
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
