use crate::Result;
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use crate::safe_text::{
    BudgetedTextDocument, BudgetedTextLine, push_line_field, push_line_list,
    push_optional_document_field,
};
use merman_core::diagrams::git_graph::{GitGraphCommitRenderModel, GitGraphRenderModel};

pub(super) fn render_git_graph_diagram(
    model: &GitGraphRenderModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let layout_resources = execution.new_resource_context(merman_core::OperationPhase::Layout);
    validate_commit_types(&model.commits, &layout_resources)?;
    let mut document = BudgetedTextDocument::from_resources(layout_resources, options);
    execution.rebind_resource_context(document.resources_mut(), merman_core::OperationPhase::Emit);

    document.push_line_with(|line| {
        push_line_field(line, "gitGraph ", "direction", &model.direction)?;
        push_line_field(line, " ", "current", &model.current_branch)
    })?;
    push_optional_document_field(&mut document, "title", model.title.as_deref())?;
    push_optional_document_field(&mut document, "accTitle", model.acc_title.as_deref())?;
    push_optional_document_field(&mut document, "accDescr", model.acc_descr.as_deref())?;
    if !model.branches.is_empty() {
        for _ in &model.branches {
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
        }
        document.push_line_with(|line| {
            push_line_list(
                line,
                "",
                "branches",
                model.branches.iter().map(|branch| branch.name.as_str()),
            )
        })?;
    }

    for commit in &model.commits {
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
        document.resources_mut().charge_layout_work(1)?;
        document.push_line_with(|line| {
            line.push_str("  - ")?;
            push_commit_text(line, commit)
        })?;
    }

    if !model.warning_facts.is_empty() {
        document.push_line("warnings:")?;
        for warning in &model.warning_facts {
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            document.resources_mut().charge_layout_work(1)?;
            document.push_line_with(|line| {
                push_line_field(line, "  - ", "message", &warning.message)
            })?;
        }
    }

    document.finish_with_execution(execution)
}

fn validate_commit_types(
    commits: &[GitGraphCommitRenderModel],
    resources: &ResourceContext,
) -> Result<()> {
    resources.transaction(|resources| {
        resources.charge_layout_work(commits.len())?;
        for commit in commits {
            resources.checkpoint()?;
            if commit_kind(commit.commit_type).is_none() {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "gitGraph",
                    feature: "unknown commit types",
                });
            }
        }
        Ok(())
    })
}

fn push_commit_text(
    line: &mut BudgetedTextLine<'_>,
    commit: &GitGraphCommitRenderModel,
) -> Result<()> {
    line.write_fmt(format_args!("seq={}", commit.seq))?;
    push_line_field(line, " ", "branch", &commit.branch)?;
    push_line_field(line, " ", "id", &commit.id)?;
    if let Some(kind) = commit_kind(commit.commit_type) {
        line.write_fmt(format_args!(" kind={kind}"))?;
    }
    push_line_field(line, " ", "message", &commit.message)?;
    if !commit.tags.is_empty() {
        push_line_list(line, " ", "tags", commit.tags.iter().map(String::as_str))?;
    }
    if !commit.parents.is_empty() {
        push_line_list(
            line,
            " ",
            "parents",
            commit.parents.iter().map(String::as_str),
        )?;
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
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy, ResourceContext};
    use merman_core::diagrams::git_graph::GitGraphBranchRenderModel;
    use merman_core::resources::ResourceProfile;
    use merman_core::{CancelReason, OperationControl, OperationPhase};

    fn commit(commit_type: i64) -> GitGraphCommitRenderModel {
        GitGraphCommitRenderModel {
            id: "c0".to_string(),
            message: String::new(),
            seq: 0,
            commit_type,
            tags: Vec::new(),
            parents: Vec::new(),
            branch: "main".to_string(),
            custom_type: None,
            custom_id: None,
        }
    }

    fn model_with_commits(commits: Vec<GitGraphCommitRenderModel>) -> GitGraphRenderModel {
        GitGraphRenderModel {
            diagram_type: "gitGraph".to_string(),
            commits,
            branches: Vec::new(),
            current_branch: "main".to_string(),
            direction: "TB".to_string(),
            title: None,
            acc_title: None,
            acc_descr: None,
            warning_facts: Vec::new(),
        }
    }

    #[test]
    fn commit_type_admission_charges_the_complete_validation_scan() {
        const VALIDATION_WORK: usize = 2;
        let model = model_with_commits(vec![commit(0), commit(98)]);
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let exact = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, VALIDATION_WORK)
            .expect("exact validation-work limit should be valid");
        let exact_control = OperationControl::new();

        assert_eq!(
            render_git_graph_diagram(
                &model,
                &AsciiRenderOptions::ascii(),
                AsciiExecution::new(&exact_control, &exact),
            )
            .expect_err("exact validation work should reach the unknown-type boundary"),
            AsciiError::UnsupportedFeature {
                diagram_type: "gitGraph",
                feature: "unknown commit types",
            }
        );

        let below = unbounded
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                VALIDATION_WORK - 1,
            )
            .expect("N-1 validation-work limit should be valid");
        let below_control = OperationControl::new();
        let error = render_git_graph_diagram(
            &model,
            &AsciiRenderOptions::ascii(),
            AsciiExecution::new(&below_control, &below),
        )
        .expect_err("N-1 validation work should reject before scanning commit types");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == VALIDATION_WORK
                    && details.max == VALIDATION_WORK - 1
        ));
    }

    #[test]
    fn commit_type_validation_rolls_back_semantic_failure() {
        const PRIOR_WORK: usize = 3;
        const PRIOR_CELLS: usize = 2;
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        resources
            .charge_usage(PRIOR_WORK, PRIOR_CELLS)
            .expect("prior ledger usage should fit");

        let error = validate_commit_types(&[commit(0), commit(98)], &resources)
            .expect_err("unknown commit types should reject the validation batch");

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "gitGraph",
                feature: "unknown commit types",
            }
        );
        assert_eq!(resources.layout_work_used(), PRIOR_WORK);
        assert_eq!(resources.document_cells_used(), PRIOR_CELLS);
    }

    #[test]
    fn commit_type_validation_prioritizes_cancellation_over_work_ceiling() {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("positive validation-work limit should be valid");
        let resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel();
        let controlled = resources.controlled(control, OperationPhase::Layout);

        let error = validate_commit_types(&[commit(0), commit(1)], &controlled)
            .expect_err("cancellation should win before the validation-work charge");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn document_limit_rejects_branches_before_join_or_full_branch_scan() {
        let branch_name = "branch-name-that-must-not-be-preformatted".repeat(128);
        let header = "gitGraph direction(bytes=0)=\"\" current(bytes=0)=\"\"";
        let branch_prefix = format!("branches=[bytes={} \"", branch_name.len());
        let exact_prefix = header.len() + branch_prefix.len();
        let resources = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_prefix)
            .expect("positive document limit");
        let options = AsciiRenderOptions::ascii();
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

        let control = OperationControl::new();
        let error =
            render_git_graph_diagram(&model, &options, AsciiExecution::new(&control, &resources))
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
