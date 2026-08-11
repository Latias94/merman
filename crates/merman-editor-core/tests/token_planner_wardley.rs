mod support;

use merman_editor_core::{
    DocumentKind, DocumentSnapshot, PlannedToken, PlannedTokenKind, PlannedTokenModifier,
    SemanticTokenPlan, plan_semantic_tokens_for_snapshot,
};
use support::SnapshotHarness;

#[test]
fn wardley_complete_plan_projects_parser_lexemes_semantics_and_utf16() {
    let source = concat!(
        "wardley-beta\r\n",
        "title 重复 🤓\r\n",
        "%% global comment 🤓\r\n",
        "component \"重复 🤓\" [0.50, 0.60] label [-20, 10] (build) (inertia)\r\n",
        "component Target [0.40, 0.70]\r\n",
        "\"重复 🤓\" +'同步'> Target+> ; 重复 🤓\r\n",
        "note \"说明 🤓\" [0.20, 0.30]\r\n",
    );
    let (snapshot, plan) = plan(source, "complete");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    assert_sorted_non_overlapping(plan.tokens());
    for kind in [
        PlannedTokenKind::Keyword,
        PlannedTokenKind::Comment,
        PlannedTokenKind::Operator,
        PlannedTokenKind::Delimiter,
        PlannedTokenKind::Variable,
        PlannedTokenKind::Property,
        PlannedTokenKind::String,
    ] {
        assert_has_kind(&plan, kind);
    }

    let definition = exact_token(source, &snapshot, &plan, "重复 🤓", 1);
    assert_eq!(definition.kind, PlannedTokenKind::Variable);
    assert!(definition.has_modifier(PlannedTokenModifier::Definition));
    assert!(definition.has_modifier(PlannedTokenModifier::Entity));
    assert_eq!(definition.length as usize, "重复 🤓".encode_utf16().count());
    assert_ne!(definition.length as usize, "重复 🤓".len());

    let reference = exact_token(source, &snapshot, &plan, "重复 🤓", 2);
    assert_eq!(reference.kind, PlannedTokenKind::Variable);
    assert!(reference.has_modifier(PlannedTokenModifier::Reference));
    assert!(reference.has_modifier(PlannedTokenModifier::Entity));

    assert_eq!(
        exact_token(source, &snapshot, &plan, "+'同步'>", 0).kind,
        PlannedTokenKind::Operator
    );
    let note = exact_token(source, &snapshot, &plan, "说明 🤓", 0);
    assert_eq!(note.kind, PlannedTokenKind::String);
    assert!(note.has_modifier(PlannedTokenModifier::Payload));
}

#[test]
fn wardley_recovery_plan_keeps_pipeline_suffix_later_definition_and_eof_string() {
    let source = concat!(
        "wardley-beta\r\n",
        "component Before [0.10, 0.20]\r\n",
        "component Broken [0.30, 1]\r\n",
        "pipeline Before {\r\n",
        "  component Child [0.40]\r\n",
        "  invalid pipeline syntax\r\n",
        "  component \"后续 子项\" [0.50]\r\n",
        "}\r\n",
        "component \"后来 🤓\" [0.60, 0.70]\r\n",
        "note \"未结束 🤓",
    );
    let (snapshot, plan) = plan(source, "recovery");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    assert_sorted_non_overlapping(plan.tokens());
    for kind in [
        PlannedTokenKind::Keyword,
        PlannedTokenKind::Delimiter,
        PlannedTokenKind::Number,
        PlannedTokenKind::Literal,
        PlannedTokenKind::String,
        PlannedTokenKind::Variable,
    ] {
        assert_has_kind(&plan, kind);
    }

    let pipeline_parent = exact_token(source, &snapshot, &plan, "Before", 1);
    assert_eq!(pipeline_parent.kind, PlannedTokenKind::Variable);
    assert!(pipeline_parent.has_modifier(PlannedTokenModifier::Reference));

    let pipeline_suffix = exact_token(source, &snapshot, &plan, "后续 子项", 0);
    assert_eq!(pipeline_suffix.kind, PlannedTokenKind::Variable);
    assert!(pipeline_suffix.has_modifier(PlannedTokenModifier::Definition));
    assert!(pipeline_suffix.has_modifier(PlannedTokenModifier::Entity));

    assert_eq!(
        exact_token(source, &snapshot, &plan, "invalid pipeline syntax", 0).kind,
        PlannedTokenKind::Literal
    );
    let later = exact_token(source, &snapshot, &plan, "后来 🤓", 0);
    assert_eq!(later.kind, PlannedTokenKind::Variable);
    assert!(later.has_modifier(PlannedTokenModifier::Definition));
    assert_eq!(later.length as usize, "后来 🤓".encode_utf16().count());

    let partial = exact_token(source, &snapshot, &plan, "未结束 🤓", 0);
    assert_eq!(partial.kind, PlannedTokenKind::String);
    assert_eq!(partial.length as usize, "未结束 🤓".encode_utf16().count());
}

fn plan(source: &str, suffix: &str) -> (DocumentSnapshot, SemanticTokenPlan) {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            format!("file:///tmp/wardley-{suffix}.mmd"),
            1,
            source.to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("Wardley semantic token plan");
    (snapshot, plan)
}

fn exact_token(
    source: &str,
    snapshot: &DocumentSnapshot,
    plan: &SemanticTokenPlan,
    needle: &str,
    occurrence: usize,
) -> PlannedToken {
    let start = source
        .match_indices(needle)
        .nth(occurrence)
        .map(|(start, _)| start)
        .unwrap_or_else(|| panic!("missing occurrence {occurrence} of {needle:?}"));
    let end = start + needle.len();
    let start = snapshot
        .source_map()
        .utf16_position(start)
        .expect("token start UTF-16 position");
    let end = snapshot
        .source_map()
        .utf16_position(end)
        .expect("token end UTF-16 position");
    assert_eq!(start.line, end.line, "test token must stay on one line");
    plan.tokens()
        .iter()
        .copied()
        .find(|token| {
            token.line == start.line as u32
                && token.start == start.character as u32
                && token.length == (end.character - start.character) as u32
        })
        .unwrap_or_else(|| {
            panic!(
                "missing exact planned token for {needle:?} occurrence {occurrence}: {:?}",
                plan.tokens()
            )
        })
}

fn assert_has_kind(plan: &SemanticTokenPlan, kind: PlannedTokenKind) {
    assert!(
        plan.tokens().iter().any(|token| token.kind == kind),
        "missing {kind:?}: {:?}",
        plan.tokens()
    );
}

fn assert_sorted_non_overlapping(tokens: &[PlannedToken]) {
    for pair in tokens.windows(2) {
        assert!(
            (pair[0].line, pair[0].start + pair[0].length) <= (pair[1].line, pair[1].start),
            "planner emitted overlapping or unsorted Wardley tokens: {pair:?}"
        );
    }
}
