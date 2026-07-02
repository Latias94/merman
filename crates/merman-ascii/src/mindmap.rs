use crate::options::AsciiRenderOptions;
use crate::text::{push_wrapped_prefixed_line, trim_trailing_blank_lines};
use merman_core::diagrams::mindmap::{
    MindmapDiagramRenderEdge, MindmapDiagramRenderModel, MindmapDiagramRenderNode,
};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

const SUMMARY_WRAP_WIDTH: usize = 80;
const BRANCH: &str = "|-- ";
const CONTINUE: &str = "|   ";
const EMPTY: &str = "    ";

pub fn render_mindmap_diagram(
    model: &MindmapDiagramRenderModel,
    _options: &AsciiRenderOptions,
) -> String {
    let mut lines = Vec::new();
    let nodes_by_id = index_nodes(&model.nodes);
    let children_by_id = build_children_map(&model.edges);
    let roots = root_ids(&model.nodes, &model.edges);

    for (index, root_id) in roots.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        if let Some(root) = nodes_by_id.get(root_id.as_str()) {
            push_wrapped_label(&mut lines, "", &root.label);
            let mut visiting = HashSet::from([root.id.as_str()]);
            render_children(
                root,
                "",
                &children_by_id,
                &nodes_by_id,
                &mut visiting,
                &mut lines,
            );
        }
    }

    trim_trailing_blank_lines(lines).join("\n")
}

fn index_nodes(nodes: &[MindmapDiagramRenderNode]) -> HashMap<&str, &MindmapDiagramRenderNode> {
    let mut out = HashMap::with_capacity(nodes.len());
    for node in nodes {
        out.insert(node.id.as_str(), node);
    }
    out
}

fn build_children_map(edges: &[MindmapDiagramRenderEdge]) -> HashMap<String, Vec<String>> {
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for edge in edges {
        let siblings = children.entry(edge.start.clone()).or_default();
        if !siblings.iter().any(|child| child == &edge.end) {
            siblings.push(edge.end.clone());
        }
    }
    children
}

fn root_ids(nodes: &[MindmapDiagramRenderNode], edges: &[MindmapDiagramRenderEdge]) -> Vec<String> {
    let mut incoming = HashSet::new();
    for edge in edges {
        incoming.insert(edge.end.as_str());
    }

    let mut roots = Vec::new();
    for node in nodes {
        if !incoming.contains(node.id.as_str()) {
            roots.push(node.id.clone());
        }
    }

    if roots.is_empty()
        && let Some(node) = nodes.first()
    {
        roots.push(node.id.clone());
    }

    roots
}

fn render_children<'a>(
    node: &'a MindmapDiagramRenderNode,
    prefix: &str,
    children_by_id: &HashMap<String, Vec<String>>,
    nodes_by_id: &HashMap<&'a str, &'a MindmapDiagramRenderNode>,
    visiting: &mut HashSet<&'a str>,
    lines: &mut Vec<String>,
) {
    let Some(children) = children_by_id.get(node.id.as_str()) else {
        return;
    };

    for (index, child_id) in children.iter().enumerate() {
        let Some(child) = nodes_by_id.get(child_id.as_str()) else {
            continue;
        };
        let is_last = index + 1 == children.len();
        let branch = if prefix.is_empty() {
            if is_last {
                "\\-- ".to_string()
            } else {
                BRANCH.to_string()
            }
        } else if is_last {
            format!("{prefix}\\-- ")
        } else {
            format!("{prefix}{BRANCH}")
        };
        let is_cycle = visiting.contains(child.id.as_str());
        let label = if is_cycle {
            Cow::Owned(format!("{} (cycle)", child.label))
        } else {
            Cow::Borrowed(child.label.as_str())
        };
        push_wrapped_label(lines, &branch, &label);

        let next_prefix = if prefix.is_empty() {
            if is_last {
                EMPTY.to_string()
            } else {
                CONTINUE.to_string()
            }
        } else if is_last {
            format!("{prefix}{EMPTY}")
        } else {
            format!("{prefix}{CONTINUE}")
        };

        if is_cycle {
            continue;
        }

        visiting.insert(child.id.as_str());
        render_children(
            child,
            &next_prefix,
            children_by_id,
            nodes_by_id,
            visiting,
            lines,
        );
        visiting.remove(child.id.as_str());
    }
}

fn push_wrapped_label(lines: &mut Vec<String>, prefix: &str, label: &str) {
    let continuation_prefix = " ".repeat(crate::text::display_width(prefix));
    push_wrapped_prefixed_line(
        lines,
        prefix,
        &continuation_prefix,
        label,
        SUMMARY_WRAP_WIDTH,
    );
}
