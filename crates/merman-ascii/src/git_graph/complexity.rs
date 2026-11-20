//! Controlled counts for the shallow GitGraph schema. Collection lengths are admitted before
//! walking their elements; generic serde complexity has no cancellation inside those walks.

use crate::Result;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use merman_model::families::git_graph::GitGraphDiagramModel;
use merman_model::resources::ModelComplexity;

pub(crate) fn measure(
    model: &GitGraphDiagramModel,
    resources: &ResourceContext,
) -> Result<ModelComplexity> {
    let mut items = resources.checked_work_add(model.commits.len(), model.branches.len())?;
    items = resources.checked_work_add(items, model.warning_facts.len())?;
    resources.charge_layout_work(items)?;
    let mut text_bytes = 0usize;
    let mut add_bytes = |bytes: usize| -> Result<()> {
        text_bytes = text_bytes
            .checked_add(bytes)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        resources.check(AsciiResourceLimitId::MaxOutputBytes, text_bytes)
    };
    for text in [
        Some(model.diagram_type.as_str()),
        Some(&model.current_branch),
        Some(&model.direction),
        model.title.as_deref(),
        model.acc_title.as_deref(),
        model.acc_descr.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        add_bytes(text.len())?;
    }
    let mut nesting = if model.branches.is_empty() { 1 } else { 2 };
    for branch in &model.branches {
        add_bytes(branch.name.len())?;
    }
    for commit in &model.commits {
        let entries = resources.checked_work_add(commit.tags.len(), commit.parents.len())?;
        resources.charge_layout_work(entries)?;
        items = resources.checked_work_add(items, entries)?;
        nesting = nesting.max(3);
        for text in [&commit.id, &commit.message, &commit.branch]
            .into_iter()
            .chain(&commit.tags)
            .chain(&commit.parents)
        {
            add_bytes(text.len())?;
        }
    }
    for warning in &model.warning_facts {
        resources.checkpoint()?;
        // Warning facts have only scalar strings and optional fixed-size SourceSpan structs.
        let warning = ModelComplexity::from_serializable(warning);
        nesting = nesting.max(warning.nesting_depth + 2);
        add_bytes(warning.text_bytes)?;
    }
    Ok(ModelComplexity::new(items.max(1), text_bytes, nesting))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AsciiError, AsciiResourcePolicy};
    use merman_model::{OperationControl, OperationPhase};

    fn model() -> GitGraphDiagramModel {
        serde_json::from_value(serde_json::json!({
            "type":"gitGraph", "currentBranch":"main", "direction":"LR", "title":"History", "accTitle":null, "accDescr":"Plan",
            "branches":[{"name":"main"}],
            "commits":[{"id":"root","message":"Start","seq":17,"type":0,"tags":["v1","tag"],"parents":[],"branch":"main","customId":false}],
            "warningFacts":[{"ruleId":"example","message":"warning","span":{"start":0,"end":1}}]
        })).unwrap()
    }

    #[test]
    fn controlled_counts_match_the_canonical_serialized_schema() {
        let mut data = model();
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        for step in 0..4 {
            assert_eq!(
                measure(&data, &resources).unwrap(),
                ModelComplexity::from_serializable(&data)
            );
            match step {
                0 => data.warning_facts.clear(),
                1 => data.commits.clear(),
                _ => data.branches.clear(),
            }
        }
    }

    #[test]
    fn nested_collection_work_is_admitted_before_scanning_its_entries() {
        let mut data = model();
        data.warning_facts.clear();
        data.commits[0].tags = vec![String::new(); 50_000];
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 2)
            .unwrap();
        let resources = ResourceContext::new(policy);
        assert!(
            matches!(measure(&data, &resources), Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits && details.actual == 50_002)
        );
        assert_eq!(resources.layout_work_used(), 2);
    }

    #[test]
    fn cancellation_interrupts_a_large_admitted_collection() {
        let mut data = model();
        data.commits[0].tags = vec![String::new(); 50_000];
        let control = OperationControl::new();
        control.cancel_after_checkpoints(32);
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded())
            .controlled(control, OperationPhase::Semantic);
        assert!(
            matches!(measure(&data, &resources), Err(AsciiError::Cancelled(cancelled)) if cancelled.phase == OperationPhase::Semantic)
        );
    }
}
