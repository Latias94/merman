use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::safe_text::encode_text_lines;
use crate::text::{
    display_width_with_profile, normalize_optional_text, push_wrapped_prefixed_line_with_profile,
    trim_trailing_blank_lines,
};
use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};

const SUMMARY_WRAP_WIDTH: usize = 80;
const TREE_BRANCH: &str = "|-- ";
const TREE_CHILD_CONTINUE: &str = "|   ";
const TREE_CHILD_EMPTY: &str = "    ";

pub fn render_tree_view_diagram(
    model: &TreeViewDiagramRenderModel,
    options: &AsciiRenderOptions,
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
            options.terminal_width_profile,
        );
    }
    encode_text_lines(trim_trailing_blank_lines(lines), options)
}

fn render_node(
    node: &TreeViewNodeRenderModel,
    prefix: &str,
    is_last: bool,
    lines: &mut Vec<String>,
    width_profile: TerminalWidthProfile,
) {
    let branch = if prefix.is_empty() {
        if is_last {
            "\\-- ".to_string()
        } else {
            TREE_BRANCH.to_string()
        }
    } else if is_last {
        format!("{prefix}\\-- ")
    } else {
        format!("{prefix}{TREE_BRANCH}")
    };
    push_wrapped_label(lines, &branch, &node.name, width_profile);

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
        render_node(
            child,
            &next_prefix,
            index + 1 == node.children.len(),
            lines,
            width_profile,
        );
    }
}

fn push_wrapped_label(
    lines: &mut Vec<String>,
    prefix: &str,
    label: &str,
    width_profile: TerminalWidthProfile,
) {
    let continuation_prefix = " ".repeat(display_width_with_profile(prefix, width_profile));
    push_wrapped_prefixed_line_with_profile(
        lines,
        prefix,
        &continuation_prefix,
        label,
        SUMMARY_WRAP_WIDTH,
        width_profile,
    );
}
