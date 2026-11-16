use crate::Result;
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use crate::safe_text::{BudgetedTextDocument, BudgetedTextLine};
use merman_model::families::git_graph::{GitGraphCommit, GitGraphDiagramModel};

mod history;
use history::{History, HistoryLanes};

pub(super) fn render_git_graph_diagram(
    model: &GitGraphDiagramModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let layout_resources = execution.new_resource_context(merman_model::OperationPhase::Layout);
    validate_commit_types(&model.commits, &layout_resources)?;
    let mut document = BudgetedTextDocument::from_resources(layout_resources, options);
    execution.rebind_resource_context(document.resources_mut(), merman_model::OperationPhase::Emit);

    push_named_text(&mut document, "Current branch: ", &model.current_branch)?;
    push_named_text(&mut document, "Source direction: ", &model.direction)?;
    for (key, value) in [
        ("Title: ", model.title.as_deref()),
        ("Accessible title: ", model.acc_title.as_deref()),
        ("Accessible description: ", model.acc_descr.as_deref()),
    ] {
        if let Some(value) = value {
            push_named_text(&mut document, key, value)?;
        }
    }
    if !model.branches.is_empty() {
        for _ in &model.branches {
            execution.checkpoint(merman_model::OperationPhase::Emit)?;
        }
        document.push_line_with(|line| {
            line.push_str("Branches: ")?;
            push_quoted_list(
                line,
                model.branches.iter().map(|branch| branch.name.as_str()),
            )
        })?;
    }

    execution.rebind_resource_context(
        document.resources_mut(),
        merman_model::OperationPhase::Layout,
    );
    let history = History::plan(&model.commits, document.resources_mut())?;
    execution.rebind_resource_context(document.resources_mut(), merman_model::OperationPhase::Emit);
    document.push_line("")?;
    if let Some(reason) = history.relation_reason() {
        document.push_line("Git history (model order; explicit parent records):")?;
        document.push_line(reason)?;
        for (index, commit) in model.commits.iter().enumerate() {
            execution.checkpoint(merman_model::OperationPhase::Emit)?;
            document.push_line_with(|line| {
                line.write_fmt(format_args!("* #{} ", index + 1))?;
                push_commit_text(line, commit)
            })?;
            document.push_line_with(|line| {
                line.push_str("    ")?;
                history.push_parents(line, commit, true)
            })?;
        }
    } else {
        document.push_line("Git history (newest first):")?;
        let mut lanes = HistoryLanes::default();
        for (index, commit) in model.commits.iter().enumerate().rev() {
            execution.checkpoint(merman_model::OperationPhase::Emit)?;
            let step = history.advance(&mut lanes, index, document.resources_mut())?;
            document.push_line_with(|line| {
                step.push_commit_prefix(line)?;
                push_commit_text(line, commit)
            })?;
            if !commit.parents.is_empty() {
                document.push_line_with(|line| {
                    step.push_parent_prefix(line)?;
                    history.push_parents(line, commit, false)
                })?;
            }
        }
    }
    if model.commits.is_empty() {
        document.push_line("  (no commits)")?;
    }

    let mut has_details = false;
    for (index, commit) in model.commits.iter().enumerate() {
        execution.checkpoint(merman_model::OperationPhase::Emit)?;
        document.resources_mut().charge_layout_work(1)?;
        if i64::try_from(index).ok() == Some(commit.seq)
            && commit.custom_type.is_none()
            && commit.custom_id.is_none()
        {
            continue;
        }
        if !has_details {
            document.push_line("")?;
            document.push_line("Details (commit occurrences in model order):")?;
            has_details = true;
        }
        document.push_line_with(|line| {
            line.write_fmt(format_args!("  #{} ", index + 1))?;
            line.push_quoted_text(&commit.id)?;
            line.write_fmt(format_args!(": sequence {}", commit.seq))?;
            if let Some(custom_type) = commit.custom_type {
                line.push_str("; type override ")?;
                if let Some(kind) = commit_kind(custom_type) {
                    line.push_str(kind)?;
                } else {
                    line.write_fmt(format_args!("{custom_type}"))?;
                }
            }
            if let Some(custom_id) = commit.custom_id {
                line.push_str(if custom_id {
                    "; ID source explicit"
                } else {
                    "; ID source generated"
                })?;
            }
            Ok(())
        })?;
    }

    if !model.warning_facts.is_empty() {
        document.push_line("")?;
        document.push_line("Warnings:")?;
        for warning in &model.warning_facts {
            execution.checkpoint(merman_model::OperationPhase::Emit)?;
            document.resources_mut().charge_layout_work(1)?;
            document.push_line_with(|line| {
                line.push_str("  - ")?;
                line.push_quoted_text(&warning.message)
            })?;
        }
    }

    document.finish_with_execution(execution)
}

fn validate_commit_types(commits: &[GitGraphCommit], resources: &ResourceContext) -> Result<()> {
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

fn push_commit_text(line: &mut BudgetedTextLine<'_>, commit: &GitGraphCommit) -> Result<()> {
    line.push_quoted_text(&commit.id)?;
    line.push_str(" [")?;
    line.push_quoted_text(&commit.branch)?;
    if commit.commit_type != 0
        && let Some(kind) = commit_kind(commit.commit_type)
    {
        line.write_fmt(format_args!("; {kind}"))?;
    }
    line.push_str("]")?;
    if commit.parents.is_empty() {
        line.push_str(" (root)")?;
    }
    if !commit.message.is_empty() {
        line.push_str(" - ")?;
        line.push_quoted_text(&commit.message)?;
    }
    if !commit.tags.is_empty() {
        line.push_str("; tags: ")?;
        push_quoted_list(line, commit.tags.iter().map(String::as_str))?;
    }
    Ok(())
}

fn push_named_text(document: &mut BudgetedTextDocument, key: &str, value: &str) -> Result<()> {
    document.push_line_with(|line| {
        line.push_str(key)?;
        line.push_quoted_text(value)
    })
}

fn push_quoted_list<'a>(
    line: &mut BudgetedTextLine<'_>,
    values: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    for (index, value) in values.into_iter().enumerate() {
        if index > 0 {
            line.push_str(", ")?;
        }
        line.push_quoted_text(value)?;
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
    use merman_model::families::git_graph::GitGraphBranch;
    use merman_model::resources::ResourceProfile;
    use merman_model::{CancelReason, OperationControl, OperationPhase};

    fn commit(commit_type: i64) -> GitGraphCommit {
        GitGraphCommit {
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

    fn model_with_commits(commits: Vec<GitGraphCommit>) -> GitGraphDiagramModel {
        GitGraphDiagramModel {
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
    fn history_index_admits_work_before_reserving_or_scanning_commit_ids() {
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("positive index-work limit");
        let resources = ResourceContext::new(policy);
        let mut first = commit(0);
        first.id = "unscanned".repeat(128);

        let error = History::plan(&[first, commit(0)], &resources)
            .expect_err("index admission must reject before touching the first key");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == 2 && details.max == 1
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn history_planning_observes_layout_cancellation_before_allocating_the_index() {
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let control = OperationControl::new();
        control.cancel();
        let controlled = resources.controlled(control, OperationPhase::Layout);

        let error = History::plan(&[commit(0)], &controlled)
            .expect_err("cancelled history must not allocate or scan its index");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn readable_history_output_honors_exact_encoded_byte_limits() {
        let model = model_with_commits(vec![commit(0)]);
        let options = AsciiRenderOptions::ascii();
        let unbounded = AsciiResourcePolicy::unbounded();
        let baseline =
            render_git_graph_diagram(&model, &options, AsciiExecution::for_test(&unbounded))
                .expect("unbounded history");
        let exact = unbounded
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, baseline.len())
            .expect("exact output limit");
        assert_eq!(
            render_git_graph_diagram(&model, &options, AsciiExecution::for_test(&exact),)
                .expect("exact output limit should fit"),
            baseline
        );

        let below = unbounded
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, baseline.len() - 1)
            .expect("N-1 output limit");
        let error = render_git_graph_diagram(&model, &options, AsciiExecution::for_test(&below))
            .expect_err("N-1 must not emit a partial history");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxOutputBytes
                    && details.actual == baseline.len() && details.max == baseline.len() - 1
        ));
    }

    #[test]
    fn document_limit_rejects_branches_before_join_or_full_branch_scan() {
        let branch_name = "branch-name-that-must-not-be-preformatted".repeat(128);
        let header = "Current branch: \"\"Source direction: \"\"";
        let branch_prefix = "Branches: \"";
        let exact_prefix = header.len() + branch_prefix.len();
        let resources = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_prefix)
            .expect("positive document limit");
        let options = AsciiRenderOptions::ascii();
        let model = GitGraphDiagramModel {
            diagram_type: "gitGraph".to_string(),
            commits: Vec::new(),
            branches: vec![GitGraphBranch { name: branch_name }],
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
