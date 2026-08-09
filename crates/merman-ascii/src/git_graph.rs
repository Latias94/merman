use crate::Result;
use crate::options::AsciiRenderOptions;
use crate::safe_text::BudgetedTextDocument;
use merman_core::diagrams::git_graph::{GitGraphCommitRenderModel, GitGraphRenderModel};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_git_graph_diagram(
    model: &GitGraphRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);

    document.push_line_with(|line| {
        line.push_str("gitGraph direction=")?;
        line.push_str(&model.direction)?;
        line.push_str(" current=")?;
        line.push_str(&model.current_branch)
    })?;
    document.push_optional_line(model.title.as_deref())?;
    document.push_optional_prefixed_line("accTitle: ", model.acc_title.as_deref())?;
    document.push_optional_prefixed_line("accDescr: ", model.acc_descr.as_deref())?;
    if !model.branches.is_empty() {
        document.push_line_with(|line| {
            line.push_str("branches: ")?;
            for (index, branch) in model.branches.iter().enumerate() {
                if index > 0 {
                    line.push_str(", ")?;
                }
                line.push_str(&branch.name)?;
            }
            Ok(())
        })?;
    }

    for commit in &model.commits {
        document.resources_mut().charge_layout_work(1)?;
        document.push_wrapped_prefixed_line_with("  - ", "    ", SUMMARY_WRAP_WIDTH, |line| {
            push_commit_text(line, commit)
        })?;
    }

    if !model.warning_facts.is_empty() {
        document.push_line("warnings:")?;
        for warning in &model.warning_facts {
            document.resources_mut().charge_layout_work(1)?;
            document.push_wrapped_prefixed_line(
                "  - ",
                "    ",
                &warning.message,
                SUMMARY_WRAP_WIDTH,
            )?;
        }
    }

    document.finish(options)
}

fn push_commit_text(
    line: &mut crate::safe_text::BudgetedWrappedText<'_, '_>,
    commit: &GitGraphCommitRenderModel,
) -> Result<()> {
    line.write_fmt(format_args!("{} ", commit.seq))?;
    line.push_str(&commit.branch)?;
    line.push_str(" ")?;
    line.push_str(&commit.id)?;
    if let Some(kind) = commit_kind(commit.commit_type) {
        line.write_fmt(format_args!(" [{kind}]"))?;
    }
    if !commit.message.is_empty() {
        line.push_str(" ")?;
        line.push_str(&commit.message)?;
    }
    if !commit.tags.is_empty() {
        line.push_str(" tags=")?;
        push_joined(line, &commit.tags)?;
    }
    if !commit.parents.is_empty() {
        line.push_str(" parents=")?;
        push_joined(line, &commit.parents)?;
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

fn push_joined(
    line: &mut crate::safe_text::BudgetedWrappedText<'_, '_>,
    values: &[String],
) -> Result<()> {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            line.push_str(", ")?;
        }
        line.push_str(value)?;
    }
    Ok(())
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
        let resources = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, 28)
            .expect("positive document limit")
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 31)
            .expect("positive layout-work limit");
        let options = AsciiRenderOptions::ascii().with_resource_policy(resources);
        let model = GitGraphRenderModel {
            diagram_type: "gitGraph".to_string(),
            commits: Vec::new(),
            branches: vec![GitGraphBranchRenderModel {
                name: "branch-name-that-must-not-be-preformatted".repeat(128),
            }],
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
                    && details.actual == 29
                    && details.max == 28
        ));
    }
}
