use crate::Result;
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{
    BudgetedTextDocument, charge_text_layout, push_wrapped_field, try_clone_layout_text,
    try_concat_layout_text, try_repeat_layout_char,
};
use crate::text::display_width_with_profile;
#[cfg(test)]
use merman_model::families::mindmap::{
    MindmapDiagramRenderEdge, MindmapDiagramRenderModel, MindmapDiagramRenderNode,
};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

mod view;
pub(super) use view::MindmapView;
use view::{NodeId, NodeView, charge_node_id};

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

pub(super) fn render_mindmap_diagram<'a>(
    model: impl Into<MindmapView<'a>>,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    render_mindmap_with_resources(
        model.into(),
        options,
        execution.new_resource_context(merman_model::OperationPhase::Layout),
        execution,
    )
}

fn render_mindmap_with_resources(
    model: MindmapView<'_>,
    options: &AsciiRenderOptions,
    resources: ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let layout_resources =
        execution.resource_context(&resources, merman_model::OperationPhase::Layout);
    let mut document = BudgetedTextDocument::from_resources(layout_resources, options);
    let chars = MindmapChars::from_options(options);
    let nodes_by_id = index_nodes(model, document.resources_mut(), execution)?;
    let edge_count = model.edge_count(document.resources_mut(), execution)?;
    let children_by_id = build_children_map(
        model,
        edge_count,
        &nodes_by_id,
        document.resources_mut(),
        execution,
    )?;
    let mut roots = root_ids(model, edge_count, document.resources_mut(), execution)?;
    append_disconnected_component_roots(
        model,
        &nodes_by_id,
        &children_by_id,
        &mut roots,
        document.resources_mut(),
        execution,
    )?;

    // Reuse traversal storage across roots. Each traversal removes its own entries on the
    // matching exit frames, so retaining the capacity avoids charging and allocating O(N) for
    // every disconnected component.
    let mut visiting = HashSet::new();
    document
        .resources_mut()
        .charge_layout_work(nodes_by_id.len())?;
    visiting
        .try_reserve(nodes_by_id.len())
        .map_err(|_| layout_allocation_error())?;
    let mut stack = Vec::new();
    execution.rebind_resource_context(document.resources_mut(), merman_model::OperationPhase::Emit);

    for (index, root_id) in roots.iter().enumerate() {
        if index > 0 {
            document.push_line("")?;
        }
        let Some(root_index) = nodes_by_id.get(root_id) else {
            continue;
        };

        debug_assert!(visiting.is_empty());
        debug_assert!(stack.is_empty());
        push_enter_frame(
            &mut stack,
            MindmapEnterFrame {
                node_index: *root_index,
                prefix: String::new(),
                is_last: true,
                depth: 1,
                is_root: true,
                inherited_section: None,
            },
            document.resources_mut(),
        )?;

        while let Some(frame) = stack.pop() {
            match frame {
                MindmapFrame::Exit(node_id) => {
                    visiting.remove(&node_id);
                }
                MindmapFrame::Enter(frame) => {
                    execution.checkpoint(merman_model::OperationPhase::Emit)?;
                    let MindmapEnterFrame {
                        node_index,
                        prefix,
                        is_last,
                        depth,
                        is_root,
                        inherited_section,
                    } = frame;
                    let node = model.node(node_index)?;
                    let is_cycle = visiting.contains(&node.id);
                    let branch = if is_root {
                        String::new()
                    } else {
                        branch_prefix(&prefix, is_last, chars, document.resources_mut())?
                    };
                    push_wrapped_label(&mut document, &branch, options, |line| {
                        push_node_text(
                            line,
                            node,
                            model.section(node, inherited_section, depth - 1),
                            is_cycle,
                        )
                    })?;

                    if is_cycle {
                        continue;
                    }
                    visiting.insert(node.id);
                    push_exit_frame(&mut stack, node.id, document.resources_mut())?;

                    let Some(children) = children_by_id.get(&node.id) else {
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
                        execution.checkpoint(merman_model::OperationPhase::Emit)?;
                        let Some(child_index_in_model) = nodes_by_id.get(child_id) else {
                            continue;
                        };
                        push_enter_frame(
                            &mut stack,
                            MindmapEnterFrame {
                                node_index: *child_index_in_model,
                                prefix: try_clone_layout_text(
                                    &next_prefix,
                                    document.resources_mut(),
                                )?,
                                is_last: child_index + 1 == children.len(),
                                depth: child_depth,
                                is_root: false,
                                inherited_section: model.child_section(
                                    node,
                                    child_index,
                                    inherited_section,
                                ),
                            },
                            document.resources_mut(),
                        )?;
                    }
                }
            }
        }
    }

    document.finish_with_execution(execution)
}

fn index_nodes<'a>(
    model: MindmapView<'a>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<HashMap<NodeId<'a>, usize>> {
    resources.charge_layout_work(model.node_count())?;
    let mut out = HashMap::new();
    let mut authored_ids = HashSet::new();
    try_reserve_hash_map(&mut out, model.node_count())?;
    authored_ids
        .try_reserve(model.node_count())
        .map_err(|_| layout_allocation_error())?;
    for (index, node) in model.nodes().enumerate() {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        charge_node_id(node.id, resources)?;
        charge_text_layout(resources, node.node_id)?;
        if out.insert(node.id, index).is_some() {
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
        if !authored_ids.insert(node.node_id) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "duplicate authored node ids",
            });
        }
    }
    Ok(out)
}

fn build_children_map<'a>(
    model: MindmapView<'a>,
    edge_count: usize,
    nodes_by_id: &HashMap<NodeId<'a>, usize>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<HashMap<NodeId<'a>, Vec<NodeId<'a>>>> {
    resources.charge_layout_work_product(edge_count, 2)?;
    let mut children = HashMap::new();
    try_reserve_hash_map(&mut children, edge_count)?;
    let mut parent_by_child = HashMap::new();
    try_reserve_hash_map(&mut parent_by_child, edge_count)?;
    let mut edge_ids = HashSet::new();
    let mut endpoint_pairs = HashSet::new();
    edge_ids
        .try_reserve(edge_count)
        .map_err(|_| layout_allocation_error())?;
    endpoint_pairs
        .try_reserve(edge_count)
        .map_err(|_| layout_allocation_error())?;
    model.visit_edges(resources, execution, |edge| {
        charge_node_id(edge.start, resources)?;
        charge_node_id(edge.end, resources)?;
        if !nodes_by_id.contains_key(&edge.start) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "edge with missing start node",
            });
        }
        if !nodes_by_id.contains_key(&edge.end) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "edge with missing end node",
            });
        }
        if !edge.id.is_empty() && !edge_ids.insert(edge.id) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "duplicate edge ids",
            });
        }
        if !endpoint_pairs.insert((edge.start, edge.end)) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "parallel edges",
            });
        }
        match parent_by_child.entry(edge.end) {
            Entry::Vacant(entry) => {
                entry.insert(edge.start);
            }
            Entry::Occupied(entry) if *entry.get() != edge.start => {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "mindmap",
                    feature: "nodes with multiple parents",
                });
            }
            Entry::Occupied(_) => {}
        }
        let siblings: &mut Vec<NodeId<'a>> = children.entry(edge.start).or_default();
        siblings
            .try_reserve(1)
            .map_err(|_| layout_allocation_error())?;
        siblings.push(edge.end);
        Ok(())
    })?;
    Ok(children)
}

fn push_node_text(
    line: &mut crate::safe_text::BudgetedWrappedText<'_>,
    node: NodeView<'_>,
    section: Option<i32>,
    is_cycle: bool,
) -> Result<()> {
    push_wrapped_field(line, "", "node", node.label)?;
    push_wrapped_field(line, " ", "id", node.node_id)?;
    if !node.shape.is_empty() && node.shape != "defaultMindmapNode" {
        push_wrapped_field(line, " ", "shape", node.shape)?;
    }
    if let Some(icon) = node.icon {
        push_wrapped_field(line, " ", "icon", icon)?;
    }
    if let Some(section) = section {
        line.write_fmt(format_args!(" section={section}"))?;
    }
    if is_cycle {
        line.push_str(" (cycle)")?;
    }
    Ok(())
}

fn root_ids<'a>(
    model: MindmapView<'a>,
    edge_count: usize,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<NodeId<'a>>> {
    resources.charge_layout_work(edge_count)?;
    let mut incoming = HashSet::new();
    incoming
        .try_reserve(edge_count)
        .map_err(|_| layout_allocation_error())?;
    model.visit_edges(resources, execution, |edge| {
        charge_node_id(edge.end, resources)?;
        incoming.insert(edge.end);
        Ok(())
    })?;

    let mut roots = Vec::new();
    for node in model.nodes() {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        charge_node_id(node.id, resources)?;
        if !incoming.contains(&node.id) {
            roots
                .try_reserve(1)
                .map_err(|_| layout_allocation_error())?;
            roots.push(node.id);
        }
    }

    if roots.is_empty()
        && let Some(node) = model.nodes().next()
    {
        roots
            .try_reserve(1)
            .map_err(|_| layout_allocation_error())?;
        roots.push(node.id);
    }

    Ok(roots)
}

fn append_disconnected_component_roots<'a>(
    model: MindmapView<'a>,
    nodes_by_id: &HashMap<NodeId<'a>, usize>,
    children_by_id: &HashMap<NodeId<'a>, Vec<NodeId<'a>>>,
    roots: &mut Vec<NodeId<'a>>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
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
            execution,
        )?;
    }

    for node in model.nodes() {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        if reachable.contains(&node.id) {
            continue;
        }
        roots
            .try_reserve(1)
            .map_err(|_| layout_allocation_error())?;
        roots.push(node.id);
        mark_reachable_component(
            node.id,
            nodes_by_id,
            children_by_id,
            &mut reachable,
            &mut stack,
            resources,
            execution,
        )?;
    }

    Ok(())
}

fn mark_reachable_component<'a>(
    root_id: NodeId<'a>,
    nodes_by_id: &HashMap<NodeId<'a>, usize>,
    children_by_id: &HashMap<NodeId<'a>, Vec<NodeId<'a>>>,
    reachable: &mut HashSet<NodeId<'a>>,
    stack: &mut Vec<NodeId<'a>>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    if !nodes_by_id.contains_key(&root_id) || !reachable.insert(root_id) {
        return Ok(());
    }
    resources.charge_layout_work(1)?;
    stack.push(root_id);

    while let Some(node_id) = stack.pop() {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        let Some(children) = children_by_id.get(&node_id) else {
            continue;
        };
        for child_id in children.iter().rev().copied() {
            execution.checkpoint(merman_model::OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            if nodes_by_id.contains_key(&child_id) && reachable.insert(child_id) {
                stack.push(child_id);
            }
        }
    }

    Ok(())
}

enum MindmapFrame<'a> {
    Enter(MindmapEnterFrame),
    Exit(NodeId<'a>),
}

struct MindmapEnterFrame {
    node_index: usize,
    prefix: String,
    is_last: bool,
    depth: usize,
    is_root: bool,
    inherited_section: Option<i32>,
}

fn push_enter_frame<'a>(
    stack: &mut Vec<MindmapFrame<'a>>,
    frame: MindmapEnterFrame,
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
    node_id: NodeId<'a>,
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
    render: impl Fn(&mut crate::safe_text::BudgetedWrappedText<'_>) -> Result<()>,
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
    use merman_model::OperationControl;
    use merman_model::resources::ResourceProfile;

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

    fn policy_with_nesting_limit(limit: usize) -> AsciiResourcePolicy {
        AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxNestingDepth, limit)
            .expect("positive nesting limit")
    }

    fn isolated_nodes(count: usize) -> MindmapDiagramRenderModel {
        MindmapDiagramRenderModel {
            nodes: (0..count)
                .map(|index| {
                    let id = format!("n{index:03}");
                    node(&id, "Node", 0)
                })
                .collect(),
            edges: Vec::new(),
        }
    }

    fn star(children: usize) -> MindmapDiagramRenderModel {
        let mut nodes = vec![node("n000", "Node", 0)];
        let mut edges = Vec::with_capacity(children);
        for index in 1..=children {
            let child_id = format!("n{index:03}");
            nodes.push(node(&child_id, "Node", 1));
            edges.push(edge(&format!("edge-{index}"), "n000", &child_id));
        }
        MindmapDiagramRenderModel { nodes, edges }
    }

    fn chain_with_width(edge_count: usize) -> MindmapDiagramRenderModel {
        let mut nodes = Vec::with_capacity(edge_count + 1);
        let mut edges = Vec::with_capacity(edge_count);
        for index in 0..=edge_count {
            let id = format!("n{index:03}");
            nodes.push(node(&id, "Node", index as i64));
            if index > 0 {
                let parent = format!("n{index_minus_one:03}", index_minus_one = index - 1);
                edges.push(edge(&format!("edge-{index}"), &parent, &id));
            }
        }
        MindmapDiagramRenderModel { nodes, edges }
    }

    fn children_map_work(model: &MindmapDiagramRenderModel) -> usize {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        let execution = AsciiExecution::new(&control, &policy);
        let model = MindmapView::from(model);
        let nodes_by_id =
            index_nodes(model, &mut resources, execution).expect("node indexing should pass");
        let edge_count = model.edge_count(&resources, execution).unwrap();
        build_children_map(model, edge_count, &nodes_by_id, &mut resources, execution)
            .expect("child indexing should pass");
        resources.layout_work_used()
    }

    fn measured_work(model: &MindmapDiagramRenderModel) -> usize {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let options = AsciiRenderOptions::ascii();
        let resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        render_mindmap_with_resources(
            model.into(),
            &options,
            resources.clone(),
            AsciiExecution::new(&control, &policy),
        )
        .expect("unbounded mindmap render should succeed");
        resources.layout_work_used()
    }

    #[test]
    fn mindmap_accepts_exact_nesting_limit() {
        let options = AsciiRenderOptions::ascii();
        let resources = policy_with_nesting_limit(DEEP_NESTING);
        let control = OperationControl::new();
        let rendered = render_mindmap_diagram(
            &chain(DEEP_NESTING),
            &options,
            AsciiExecution::new(&control, &resources),
        )
        .expect("deep nesting equal to the limit should render iteratively");

        assert!(rendered.contains("Leaf"));
    }

    #[test]
    fn mindmap_rejects_limit_minus_one_before_descending() {
        let options = AsciiRenderOptions::ascii();
        let resources = policy_with_nesting_limit(DEEP_NESTING - 1);
        let control = OperationControl::new();
        let error = render_mindmap_diagram(
            &chain(DEEP_NESTING),
            &options,
            AsciiExecution::new(&control, &resources),
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

    #[test]
    fn disconnected_mindmap_roots_do_not_repeat_full_node_scan_work() {
        let work_2 = measured_work(&isolated_nodes(2));
        let work_3 = measured_work(&isolated_nodes(3));
        let work_4 = measured_work(&isolated_nodes(4));

        assert_eq!(
            work_4 - work_3,
            work_3 - work_2,
            "disconnected-root work should have a constant per-root increment"
        );
    }

    #[test]
    fn mindmap_children_map_work_is_independent_of_parent_fanout() {
        let star_work = children_map_work(&star(8));
        let chain_work = children_map_work(&chain_with_width(8));

        assert_eq!(
            star_work, chain_work,
            "child-map work should depend on edge count and text, not sibling fanout"
        );
    }

    #[test]
    fn canonical_mindmap_matches_full_shapes_and_section_traversal() {
        use merman_core::{MermaidEngine, ParseOptions, SemanticParser};
        use merman_model::SemanticDiagramModel;
        use merman_model::presentation::RenderSemanticModel;

        let branches = (0..13)
            .map(|index| format!("    branch{index}\n      leaf{index}\n"))
            .collect::<String>();
        for (config, expected_section) in [
            ("{}", "section=10"),
            (
                r#"{"theme":"redux","mindmap":{"colorBy":"depth"}}"#,
                "section=1",
            ),
            (
                r#"{"theme":"redux-dark","mindmap":{"colorBy":"branch"}}"#,
                "section=10",
            ),
        ] {
            let source = format!("%%{{init: {config}}}%%\nmindmap\n  root((Root λ))\n{branches}");
            let canonical = SemanticParser::new().parse_diagram_sync(&source).unwrap();
            let SemanticDiagramModel::Mindmap(model) = &canonical.model else {
                panic!("expected canonical Mindmap");
            };
            let full = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let RenderSemanticModel::Mindmap(full) = full.model() else {
                panic!("expected full Mindmap");
            };
            for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
                let policy =
                    AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
                let control = OperationControl::new();
                let execution = AsciiExecution::new(&control, &policy);
                let lean = render_mindmap_diagram(
                    (model, &canonical.metadata.semantic_config),
                    &options,
                    execution,
                )
                .unwrap();
                let full = render_mindmap_diagram(full, &options, execution).unwrap();
                assert_eq!(lean, full, "config={config}");
                assert!(lean.contains(expected_section));
            }
        }
    }

    #[test]
    fn canonical_mindmap_retains_graph_validation_and_disconnected_cycles() {
        use merman_core::SemanticParser;
        use merman_model::SemanticDiagramModel;

        let parsed = SemanticParser::new()
            .parse_diagram_sync("mindmap\n root\n  left\n  right\n")
            .unwrap();
        let SemanticDiagramModel::Mindmap(model) = parsed.model else {
            panic!("expected canonical Mindmap");
        };
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let options = AsciiRenderOptions::ascii();
        for (case, expected) in [
            (0, "duplicate node ids"),
            (1, "duplicate authored node ids"),
            (2, "edge with missing end node"),
            (3, "parallel edges"),
            (4, "nodes with multiple parents"),
            (5, "missing authored node ids"),
        ] {
            let mut invalid = model.clone();
            match case {
                0 => invalid.nodes[1].id = invalid.nodes[0].id,
                1 => invalid.nodes[1].node_id = invalid.nodes[0].node_id.clone(),
                2 => invalid.nodes[0].children.push(42),
                3 => invalid.nodes[0].children.push(1),
                4 => invalid.nodes[1].children.push(2),
                5 => invalid.nodes[1].node_id.clear(),
                _ => unreachable!(),
            }
            let error = render_mindmap_diagram(
                (&invalid, &parsed.metadata.semantic_config),
                &options,
                AsciiExecution::for_test(&policy),
            )
            .unwrap_err();
            assert!(
                matches!(error, AsciiError::UnsupportedFeature { feature, .. } if feature == expected)
            );
        }

        let mut disconnected = model;
        disconnected.nodes[0].children.clear();
        disconnected.nodes[1].children.push(2);
        disconnected.nodes[2].children.push(1);
        let output = render_mindmap_diagram(
            (&disconnected, &parsed.metadata.semantic_config),
            &options,
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        assert!(output.contains("(cycle)"));
        for authored in ["root", "left", "right"] {
            assert!(output.contains(authored));
        }
    }

    #[test]
    fn canonical_mindmap_observes_exact_work_limits_and_cancellation() {
        use merman_core::SemanticParser;
        use merman_model::SemanticDiagramModel;

        let parsed = SemanticParser::new()
            .parse_diagram_sync("mindmap\n root\n  left\n   leaf\n  right\n")
            .unwrap();
        let SemanticDiagramModel::Mindmap(model) = &parsed.model else {
            panic!("expected canonical Mindmap");
        };
        let view = MindmapView::from((model, &parsed.metadata.semantic_config));
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let options = AsciiRenderOptions::ascii();
        let resources = ResourceContext::new(policy);
        render_mindmap_with_resources(
            view,
            &options,
            resources.clone(),
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        let exact = resources.layout_work_used();
        for limit in [exact, exact - 1] {
            let limited = policy
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap();
            let result = render_mindmap_diagram(view, &options, AsciiExecution::for_test(&limited));
            if limit == exact {
                result.expect("exact canonical work limit should pass");
            } else {
                assert!(
                    matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits)
                );
            }
        }
        let control = OperationControl::new();
        control.cancel();
        let result = render_mindmap_diagram(view, &options, AsciiExecution::new(&control, &policy));
        assert!(matches!(result, Err(AsciiError::Cancelled(_))));
    }
}
