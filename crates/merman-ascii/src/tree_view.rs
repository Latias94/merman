use crate::options::AsciiRenderOptions;
use crate::text::{
    display_width, normalize_optional_text, trim_trailing_blank_lines, wrap_display_lines,
};
use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};

const TREE_BRANCH: &str = "|-- ";
const TREE_CHILD_CONTINUE: &str = "|   ";
const TREE_CHILD_EMPTY: &str = "    ";

pub fn render_tree_view_diagram(
    model: &TreeViewDiagramRenderModel,
    _options: &AsciiRenderOptions,
) -> String {
    let mut lines = Vec::new();
    if let Some(title) = normalize_optional_text(model.title.as_deref()) {
        lines.push(title);
    }
    if let Some(acc_title) = normalize_optional_text(model.acc_title.as_deref()) {
        lines.push(format!("accTitle: {acc_title}"));
    }
    if let Some(acc_descr) = normalize_optional_text(model.acc_descr.as_deref()) {
        lines.push(format!("accDescr: {acc_descr}"));
    }
    for (index, child) in model.root.children.iter().enumerate() {
        render_node(
            child,
            "",
            index + 1 == model.root.children.len(),
            &mut lines,
        );
    }
    trim_trailing_blank_lines(lines).join("\n")
}

fn render_node(
    node: &TreeViewNodeRenderModel,
    prefix: &str,
    is_last: bool,
    lines: &mut Vec<String>,
) {
    let branch = if prefix.is_empty() {
        if is_last {
            format!("\\-- ")
        } else {
            format!("{TREE_BRANCH}")
        }
    } else if is_last {
        format!("{prefix}\\-- ")
    } else {
        format!("{prefix}{TREE_BRANCH}")
    };
    push_wrapped_label(lines, &branch, &node.name);

    let next_prefix = if prefix.is_empty() {
        if is_last {
            TREE_CHILD_EMPTY.to_string()
        } else {
            TREE_CHILD_CONTINUE.to_string()
        }
    } else if is_last {
        format!("{prefix}{TREE_CHILD_EMPTY}")
    } else {
        format!("{prefix}{TREE_CHILD_CONTINUE}")
    };

    for (index, child) in node.children.iter().enumerate() {
        render_node(child, &next_prefix, index + 1 == node.children.len(), lines);
    }
}

fn push_wrapped_label(lines: &mut Vec<String>, prefix: &str, label: &str) {
    let available = 80usize.saturating_sub(display_width(prefix));
    let wrapped = wrap_display_lines(label, available.max(1));
    if wrapped.is_empty() {
        lines.push(prefix.to_string());
        return;
    }

    for (index, line) in wrapped.iter().enumerate() {
        if index == 0 {
            lines.push(format!("{prefix}{line}"));
        } else {
            lines.push(format!("{}{}", " ".repeat(display_width(prefix)), line));
        }
    }
}
