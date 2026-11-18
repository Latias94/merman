//! Fixed branch tracks over canonical commit/parent facts, with complete reading records.

use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use crate::safe_text::BudgetedTextDocument;
use crate::{
    AsciiColorMode, AsciiDiagram, AsciiProjection, AsciiRenderOptions, AsciiSourceContext, Result,
};
use merman_model::OperationPhase;
use merman_model::families::git_graph::{GitGraphCommit, GitGraphDiagramModel};

mod complexity;
mod tracks;
pub(crate) use complexity::measure as complexity;

pub(super) fn render_git_graph_diagram(
    model: &GitGraphDiagramModel,
    options: &AsciiRenderOptions,
    source: AsciiSourceContext<'_>,
    execution: AsciiExecution<'_>,
) -> Result<crate::output::RenderedPrimary> {
    let resources = execution.new_resource_context(OperationPhase::Layout);
    validate_commit_types(&model.commits, &resources)?;
    crate::output::preflight_semantic_model(&AsciiDiagram::GitGraph(model), source, execution)?;
    let admission = tracks::plan(model, source.semantic_config, &resources)?;
    let projection = match &admission {
        tracks::Admission::Spatial(_) => AsciiProjection::Diagrammatic,
        tracks::Admission::Records(_) => AsciiProjection::StructuredText,
    };
    let mut document = BudgetedTextDocument::from_resources(
        resources.scoped(),
        &options.with_color_mode(AsciiColorMode::Plain),
    );
    execution.rebind_resource_context(document.resources_mut(), OperationPhase::Emit);
    match admission {
        tracks::Admission::Spatial(tracks) => tracks.write(
            model,
            options,
            execution.max_width().unwrap_or(80),
            &mut document,
        )?,
        tracks::Admission::Records(reason) => {
            let width = execution.max_width().unwrap_or(80);
            document.push_wrapped_prefixed_line_with("", "  ", width, |line| {
                line.push_str("Git branch tracks unavailable: ")?;
                line.push_str(reason)
            })?;
        }
    }
    let topology = document.finish()?;
    crate::output::render_reading_primary(
        &AsciiDiagram::GitGraph(model),
        source,
        options,
        execution,
        Some(&topology),
    )
    .map(|text| crate::output::RenderedPrimary::records(text, projection))
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

pub(crate) fn commit_kind(commit_type: i64) -> Option<&'static str> {
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
    use crate::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_model::{OperationControl, OperationPhase};

    fn commit(commit_type: i64) -> GitGraphCommit {
        GitGraphCommit {
            id: "root".to_string(),
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

    #[test]
    fn commit_type_validation_admits_scan_before_touching_commits() {
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let resources = ResourceContext::new(policy);
        assert!(matches!(
            validate_commit_types(&[commit(0), commit(98)], &resources),
            Err(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == 2 && details.max == 1
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn commit_type_validation_rolls_back_semantic_failure() {
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        resources.charge_usage(3, 2).unwrap();
        assert!(matches!(
            validate_commit_types(&[commit(0), commit(98)], &resources),
            Err(AsciiError::UnsupportedFeature {
                diagram_type: "gitGraph",
                feature: "unknown commit types",
            })
        ));
        assert_eq!(resources.layout_work_used(), 3);
        assert_eq!(resources.document_cells_used(), 2);
    }

    #[test]
    fn cancellation_precedes_commit_type_work_limit() {
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let control = OperationControl::new();
        control.cancel();
        let resources = ResourceContext::new(policy).controlled(control, OperationPhase::Layout);
        assert!(matches!(
            validate_commit_types(&[commit(0), commit(98)], &resources),
            Err(AsciiError::Cancelled(_))
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }
}
