use crate::options::AsciiRenderOptions;
use crate::text::{normalize_optional_text, push_wrapped_prefixed_line, trim_trailing_blank_lines};
use merman_core::diagrams::git_graph::{GitGraphCommitRenderModel, GitGraphRenderModel};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_git_graph_diagram(
    model: &GitGraphRenderModel,
    _options: &AsciiRenderOptions,
) -> String {
    let mut lines = Vec::new();

    lines.push(format!(
        "gitGraph direction={} current={}",
        model.direction, model.current_branch
    ));
    if let Some(title) = normalize_optional_text(model.acc_title.as_deref()) {
        lines.push(format!("accTitle: {title}"));
    }
    if let Some(descr) = normalize_optional_text(model.acc_descr.as_deref()) {
        lines.push(format!("accDescr: {descr}"));
    }
    if !model.branches.is_empty() {
        lines.push(format!(
            "branches: {}",
            model
                .branches
                .iter()
                .map(|branch| branch.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    for commit in &model.commits {
        push_wrapped_prefixed_line(
            &mut lines,
            "  - ",
            "    ",
            &render_commit_text(commit),
            SUMMARY_WRAP_WIDTH,
        );
    }

    if !model.warnings.is_empty() {
        lines.push("warnings:".to_string());
        for warning in &model.warnings {
            push_wrapped_prefixed_line(&mut lines, "  - ", "    ", warning, SUMMARY_WRAP_WIDTH);
        }
    }

    trim_trailing_blank_lines(lines).join("\n")
}

fn render_commit_text(commit: &GitGraphCommitRenderModel) -> String {
    let mut parts = vec![format!("{} {} {}", commit.seq, commit.branch, commit.id)];
    if let Some(kind) = commit_kind(commit.commit_type) {
        parts.push(format!("[{kind}]"));
    }
    if !commit.message.is_empty() {
        parts.push(commit.message.clone());
    }
    if !commit.tags.is_empty() {
        parts.push(format!("tags={}", commit.tags.join(", ")));
    }
    if !commit.parents.is_empty() {
        parts.push(format!("parents={}", commit.parents.join(", ")));
    }
    if let Some(custom_type) = commit.custom_type {
        parts.push(format!("customType={custom_type}"));
    }
    if let Some(custom_id) = commit.custom_id {
        parts.push(format!("customId={custom_id}"));
    }
    parts.join(" ")
}

fn commit_kind(commit_type: i64) -> Option<&'static str> {
    match commit_type {
        0 => Some("normal"),
        1 => Some("reverse"),
        2 => Some("highlight"),
        3 => Some("merge"),
        4 => Some("cherry-pick"),
        _ => None,
    }
}
