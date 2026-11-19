use super::adapter::project;
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy, ResourceContext};
use merman_model::families::usecase::{
    UsecaseActorType, UsecaseDiagramModel, UsecaseJsonNode, UsecaseLabelType, UsecaseNode,
    UsecaseNodeKind,
};
use merman_model::{OperationControl, OperationPhase};

fn labels(count: usize, label: &str) -> UsecaseDiagramModel {
    UsecaseDiagramModel {
        direction: "LR".into(),
        nodes: (0..count)
            .map(|index| UsecaseNode {
                id: format!("node-{index}"),
                label: label.into(),
                label_type: UsecaseLabelType::Text,
                kind: UsecaseNodeKind::UseCaseEllipse,
                actor_type: UsecaseActorType::Normal,
                icon: None,
                parent_id: None,
                business: false,
                stereotype: None,
                classes: vec![],
                styles: vec![],
            })
            .collect(),
        boundaries: vec![],
        relationships: vec![],
        notes: vec![],
        json_nodes: vec![],
        class_defs: vec![],
        title: None,
        acc_title: None,
        acc_description: None,
    }
}

fn json_table(value: serde_json::Value) -> UsecaseDiagramModel {
    let mut model = labels(0, "");
    model.json_nodes.push(UsecaseJsonNode {
        id: "Payload".into(),
        value,
        string_encoding: None,
        property_order: Default::default(),
        non_finite_numbers: Default::default(),
        classes: vec![],
        styles: vec![],
    });
    model
}

#[test]
fn json_table_projection_has_exact_work_admission_and_transactional_rollback() {
    let model =
        json_table(serde_json::json!({"items": ["客户👩‍💻", {"enabled": true}], "empty": {}}));
    let policy = AsciiResourcePolicy::unbounded();
    let resources = ResourceContext::new(policy);
    project(
        &model,
        TerminalWidthProfile::Cjk,
        &resources,
        AsciiExecution::for_test(&policy),
    )
    .unwrap();
    let exact = resources.layout_work_used();
    assert!(exact > 1);
    for max in [exact, exact - 1] {
        let policy = policy
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, max)
            .unwrap();
        let resources = ResourceContext::new(policy);
        let result = resources.transaction(|resources| {
            project(
                &model,
                TerminalWidthProfile::Cjk,
                resources,
                AsciiExecution::for_test(&policy),
            )
        });
        if max == exact {
            result.expect("the complete table projection must fit its measured allowance");
            assert_eq!(resources.layout_work_used(), exact);
        } else {
            assert!(
                matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits)
            );
            assert_eq!(resources.layout_work_used(), 0);
        }
        assert_eq!(
            resources.document_cells_used(),
            0,
            "the prepared graph owns retained node cells"
        );
    }
}

#[test]
fn json_table_scanning_is_cancellable_and_preserves_prior_ledger_usage() {
    let model = json_table(serde_json::json!({"value": "客".repeat(32 * 1024)}));
    let policy = AsciiResourcePolicy::unbounded();
    let control = OperationControl::new();
    control.cancel_after_checkpoints(64);
    let resources = ResourceContext::new(policy);
    resources.charge_usage(7, 3).unwrap();
    let resources = resources.controlled(control.clone(), OperationPhase::Semantic);
    let execution = AsciiExecution::new(&control, &policy).with_render_ledger(&resources);
    let error = resources
        .transaction(|resources| {
            project(&model, TerminalWidthProfile::Unicode, resources, execution)
        })
        .unwrap_err();
    assert!(
        matches!(error, AsciiError::Cancelled(cancelled) if cancelled.phase == OperationPhase::Semantic)
    );
    assert_eq!(resources.layout_work_used(), 7);
    assert_eq!(resources.document_cells_used(), 3);
}

#[test]
fn json_table_depth_and_authored_graphemes_keep_their_resource_limits() {
    let model = json_table(serde_json::json!({"value": [["👩‍💻"]]}));
    for (limit, exact) in [
        (AsciiResourceLimitId::MaxNestingDepth, 4),
        (AsciiResourceLimitId::MaxGraphemeBytes, "👩‍💻".len()),
    ] {
        for max in [exact, exact - 1] {
            let policy = AsciiResourcePolicy::unbounded()
                .with_limit(limit, max)
                .unwrap();
            let resources = ResourceContext::new(policy);
            let result = resources.transaction(|resources| {
                project(
                    &model,
                    TerminalWidthProfile::Unicode,
                    resources,
                    AsciiExecution::for_test(&policy),
                )
            });
            if max == exact {
                result.expect("the exact authored text/depth bound must pass");
            } else {
                assert!(
                    matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == limit)
                );
                assert_eq!(resources.layout_work_used(), 0);
                assert_eq!(resources.document_cells_used(), 0);
            }
        }
    }
}

#[test]
fn wide_json_containers_admit_minimum_rows_before_reserving_the_queue() {
    const COUNT: usize = 4096;
    let model = json_table(serde_json::json!({"items": vec![serde_json::Value::Null; COUNT]}));
    for limit in [
        AsciiResourceLimitId::MaxDocumentCells,
        AsciiResourceLimitId::MaxOutputBytes,
    ] {
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(limit, 128)
            .unwrap();
        let resources = ResourceContext::new(policy);
        let error = resources
            .transaction(|resources| {
                project(
                    &model,
                    TerminalWidthProfile::Unicode,
                    resources,
                    AsciiExecution::for_test(&policy),
                )
            })
            .unwrap_err();
        assert!(matches!(error, AsciiError::ResourceLimitExceeded(details)
            if details.limit == limit && details.actual >= COUNT));
        assert_eq!(resources.layout_work_used(), 0);
        assert_eq!(resources.document_cells_used(), 0);
    }
}

#[test]
fn projection_work_accepts_exact_n_rejects_n_minus_one_and_rolls_back() {
    let model = labels(8, "客户👩‍💻");
    let policy = AsciiResourcePolicy::unbounded();
    let resources = ResourceContext::new(policy);
    project(
        &model,
        TerminalWidthProfile::Unicode,
        &resources,
        AsciiExecution::for_test(&policy),
    )
    .unwrap();
    let exact = resources.layout_work_used();
    assert!(exact > 1);
    for max in [exact, exact - 1] {
        let policy = policy
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, max)
            .unwrap();
        let resources = ResourceContext::new(policy);
        let result = resources.transaction(|resources| {
            project(
                &model,
                TerminalWidthProfile::Unicode,
                resources,
                AsciiExecution::for_test(&policy),
            )
        });
        if max == exact {
            result.unwrap();
            assert_eq!(resources.layout_work_used(), exact);
        } else {
            assert!(
                matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits && details.actual == exact && details.max == max)
            );
            assert_eq!(resources.layout_work_used(), 0);
            assert_eq!(resources.document_cells_used(), 0);
        }
    }
}

#[test]
fn aggregate_raw_label_bytes_are_admitted_before_whitespace_is_cloned() {
    const COUNT: usize = 4;
    let label = format!("{}x", " ".repeat(16 * 1024));
    let model = labels(COUNT, &label);
    let source_per_node = label.len() + 1 + "[use case]".len();
    let total = COUNT * (source_per_node + "n0".len());
    for max in [total, total - 1] {
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, max)
            .unwrap();
        let resources = ResourceContext::new(policy);
        let result = resources.transaction(|resources| {
            project(
                &model,
                TerminalWidthProfile::Unicode,
                resources,
                AsciiExecution::for_test(&policy),
            )
        });
        if max == total {
            result.expect("exact retained source bytes must pass");
        } else {
            assert!(
                matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxOutputBytes && details.actual == total && details.max == max)
            );
            assert_eq!(resources.layout_work_used(), 0);
            assert_eq!(resources.document_cells_used(), 0);
        }
    }
}

#[test]
fn aggregate_node_cells_are_checked_before_deferred_sources_materialize() {
    const COUNT: usize = 8;
    let model = labels(COUNT, "客户");
    let total = COUNT * (4 + "[use case]".len());
    for max in [total, total - 1] {
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, max)
            .unwrap();
        let resources = ResourceContext::new(policy);
        let result = resources.transaction(|resources| {
            project(
                &model,
                TerminalWidthProfile::Unicode,
                resources,
                AsciiExecution::for_test(&policy),
            )
        });
        if max == total {
            result.expect("the exact sum of deferred cells must pass");
            assert_eq!(
                resources.document_cells_used(),
                0,
                "the graph owns node cell admission"
            );
        } else {
            assert!(
                matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxDocumentCells && details.actual == total && details.max == max)
            );
            assert_eq!(resources.layout_work_used(), 0);
        }
    }
}

#[test]
fn long_label_planning_is_cancellable_and_preserves_prior_ledger_usage() {
    let model = labels(1, &"客".repeat(32 * 1024));
    let policy = AsciiResourcePolicy::unbounded();
    let control = OperationControl::new();
    control.cancel_after_checkpoints(64);
    let resources = ResourceContext::new(policy);
    resources.charge_usage(7, 3).unwrap();
    let resources = resources.controlled(control.clone(), OperationPhase::Semantic);
    let execution = AsciiExecution::new(&control, &policy).with_render_ledger(&resources);
    let error = resources
        .transaction(|resources| {
            project(&model, TerminalWidthProfile::Unicode, resources, execution)
        })
        .unwrap_err();
    assert!(
        matches!(error, AsciiError::Cancelled(cancelled) if cancelled.phase == OperationPhase::Semantic)
    );
    assert_eq!(resources.layout_work_used(), 7);
    assert_eq!(resources.document_cells_used(), 3);
}

#[test]
fn cancellation_precedes_competing_quota_at_the_family_boundary() {
    let model = labels(1, "label");
    let policy = AsciiResourcePolicy::unbounded()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .unwrap();
    let resources = ResourceContext::new(policy);
    let control = OperationControl::new();
    control.cancel();
    let execution = AsciiExecution::new(&control, &policy);
    assert!(matches!(
        project(&model, TerminalWidthProfile::Unicode, &resources, execution),
        Err(AsciiError::Cancelled(_))
    ));
    assert_eq!(resources.layout_work_used(), 0);
}

#[test]
fn deferred_labels_preserve_exact_grapheme_limits() {
    let label = "客户👩‍💻";
    let model = labels(1, label);
    let exact = "👩‍💻".len();
    for max in [exact, exact - 1] {
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, max)
            .unwrap();
        let resources = ResourceContext::new(policy);
        let result = resources.transaction(|resources| {
            project(
                &model,
                TerminalWidthProfile::Cjk,
                resources,
                AsciiExecution::for_test(&policy),
            )
        });
        if max == exact {
            result.expect("the complete emoji grapheme should fit exactly");
        } else {
            assert!(
                matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxGraphemeBytes
                    && details.actual == exact && details.max == max)
            );
            assert_eq!(resources.layout_work_used(), 0);
        }
    }
}
