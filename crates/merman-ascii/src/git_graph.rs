use crate::Result;
use crate::options::AsciiRenderOptions;
use crate::safe_text::{BudgetedTextDocument, BudgetedTextLine};
use merman_core::diagrams::git_graph::{GitGraphCommitRenderModel, GitGraphRenderModel};

pub fn render_git_graph_diagram(
    model: &GitGraphRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);

    document.push_line_with(|line| {
        line.write_fmt(format_args!(
            "gitGraph direction(bytes={})=",
            model.direction.len()
        ))?;
        line.push_quoted_text(&model.direction)?;
        line.write_fmt(format_args!(
            " current(bytes={})=",
            model.current_branch.len()
        ))?;
        line.push_quoted_text(&model.current_branch)
    })?;
    push_optional_framed_line(&mut document, "title", model.title.as_deref())?;
    push_optional_framed_line(&mut document, "accTitle", model.acc_title.as_deref())?;
    push_optional_framed_line(&mut document, "accDescr", model.acc_descr.as_deref())?;
    if !model.branches.is_empty() {
        document.push_line_with(|line| {
            line.push_str("branches=[")?;
            for (index, branch) in model.branches.iter().enumerate() {
                if index > 0 {
                    line.push_str(", ")?;
                }
                line.write_fmt(format_args!("bytes={} ", branch.name.len()))?;
                line.push_quoted_text(&branch.name)?;
            }
            line.push_str("]")?;
            Ok(())
        })?;
    }

    for commit in &model.commits {
        document.resources_mut().charge_layout_work(1)?;
        document.push_line_with(|line| {
            line.push_str("  - ")?;
            push_commit_text(line, commit)
        })?;
    }

    if !model.warning_facts.is_empty() {
        document.push_line("warnings:")?;
        for warning in &model.warning_facts {
            document.resources_mut().charge_layout_work(1)?;
            document.push_line_with(|line| {
                line.write_fmt(format_args!(
                    "  - message(bytes={})=",
                    warning.message.len()
                ))?;
                line.push_quoted_text(&warning.message)
            })?;
        }
    }

    document.finish(options)
}

fn push_commit_text(
    line: &mut BudgetedTextLine<'_>,
    commit: &GitGraphCommitRenderModel,
) -> Result<()> {
    line.write_fmt(format_args!("seq={}", commit.seq))?;
    push_framed_field(line, "branch", &commit.branch)?;
    push_framed_field(line, "id", &commit.id)?;
    if let Some(kind) = commit_kind(commit.commit_type) {
        line.write_fmt(format_args!(" kind={kind}"))?;
    }
    push_framed_field(line, "message", &commit.message)?;
    if !commit.tags.is_empty() {
        push_framed_list(line, "tags", &commit.tags)?;
    }
    if !commit.parents.is_empty() {
        push_framed_list(line, "parents", &commit.parents)?;
    }
    if let Some(custom_type) = commit.custom_type {
        line.push_str(" typeOverride=")?;
        if let Some(kind) = commit_kind(custom_type) {
            line.push_str(kind)?;
        } else {
            line.write_fmt(format_args!("{custom_type}"))?;
        }
    }
    if commit.custom_id == Some(true) {
        line.push_str(" idSource=explicit")?;
    }
    Ok(())
}

fn push_framed_field(line: &mut BudgetedTextLine<'_>, key: &str, value: &str) -> Result<()> {
    line.write_fmt(format_args!(" {key}(bytes={})=", value.len()))?;
    line.push_quoted_text(value)
}

fn push_framed_list(line: &mut BudgetedTextLine<'_>, key: &str, values: &[String]) -> Result<()> {
    line.write_fmt(format_args!(" {key}=["))?;
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            line.push_str(", ")?;
        }
        line.write_fmt(format_args!("bytes={} ", value.len()))?;
        line.push_quoted_text(value)?;
    }
    line.push_str("]")?;
    Ok(())
}

fn push_optional_framed_line(
    document: &mut BudgetedTextDocument,
    key: &str,
    value: Option<&str>,
) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    document.push_line_with(|line| {
        line.write_fmt(format_args!("{key}(bytes={})=", value.len()))?;
        line.push_quoted_text(value)
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AsciiError;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::diagrams::git_graph::GitGraphBranchRenderModel;
    use merman_core::resources::ResourceProfile;

    #[test]
    fn document_limit_rejects_branches_before_join_or_full_branch_scan() {
        let branch_name = "branch-name-that-must-not-be-preformatted".repeat(128);
        let header = "gitGraph direction(bytes=0)=\"\" current(bytes=0)=\"\"";
        let branch_prefix = format!("branches=[bytes={} \"", branch_name.len());
        let exact_prefix = header.len() + branch_prefix.len();
        let resources = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_prefix)
            .expect("positive document limit");
        let options = AsciiRenderOptions::ascii().with_resource_policy(resources);
        let model = GitGraphRenderModel {
            diagram_type: "gitGraph".to_string(),
            commits: Vec::new(),
            branches: vec![GitGraphBranchRenderModel { name: branch_name }],
            current_branch: String::new(),
            direction: String::new(),
            title: None,
            acc_title: None,
            acc_descr: None,
            warning_facts: Vec::new(),
        };

        let error = render_git_graph_diagram(&model, &options)
            .expect_err("the branch row must fail at its first document cell");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxDocumentCells
                    && details.actual == exact_prefix + 1
                    && details.max == exact_prefix
        ));
    }
}
