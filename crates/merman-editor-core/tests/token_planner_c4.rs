mod support;

use merman_editor_core::{
    DocumentKind, DocumentSnapshot, PlannedToken, PlannedTokenKind, PlannedTokenModifier,
    SemanticTokenPlan, plan_semantic_tokens_for_snapshot,
};
use support::SnapshotHarness;

#[test]
fn c4_complete_plan_merges_parser_lexemes_semantics_and_utf16_exactly() {
    let source = concat!(
        "C4Deployment\r\n",
        "title 系统部署\r\n",
        "Node(root, \"🤓 根节点\", \"EC2\", \"描述\") {\r\n",
        "  Person(用户, \"客户\", \"使用系统\")\r\n",
        "}\r\n",
        "RelIndex(12, 用户, root, \"调用\", \"HTTPS\")\r\n",
        "UpdateElementStyle(root, $bgColor=\"#ffaa00\", $shadowing=\"true\")\r\n",
        "UpdateRelStyle(用户, root, $lineColor=\"blue\", $offsetX=\"12\")\r\n",
        "UpdateLayoutConfig($c4ShapeInRow=\"3\", 2)\r\n",
        "direction LR\r\n",
    );
    let (snapshot, plan) = plan(source, "complete");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    for kind in [
        PlannedTokenKind::Keyword,
        PlannedTokenKind::Delimiter,
        PlannedTokenKind::Operator,
        PlannedTokenKind::Style,
        PlannedTokenKind::Variable,
    ] {
        assert_has_kind(&plan, kind);
    }
    assert_sorted_non_overlapping(plan.tokens());

    let root_definition = exact_token(source, &snapshot, &plan, "root", 0);
    assert_eq!(root_definition.kind, PlannedTokenKind::Variable);
    assert!(root_definition.has_modifier(PlannedTokenModifier::Definition));
    assert!(root_definition.has_modifier(PlannedTokenModifier::Entity));
    assert!(!root_definition.has_modifier(PlannedTokenModifier::Reference));

    let root_reference = exact_token(source, &snapshot, &plan, "root", 1);
    assert_eq!(root_reference.kind, PlannedTokenKind::Variable);
    assert!(root_reference.has_modifier(PlannedTokenModifier::Reference));
    assert!(root_reference.has_modifier(PlannedTokenModifier::Entity));
    assert!(!root_reference.has_modifier(PlannedTokenModifier::Definition));

    let unicode_definition = exact_token(source, &snapshot, &plan, "用户", 0);
    assert_eq!(unicode_definition.kind, PlannedTokenKind::Variable);
    assert_eq!(
        unicode_definition.length as usize,
        "用户".encode_utf16().count()
    );
    assert_ne!(unicode_definition.length as usize, "用户".len());
    assert!(unicode_definition.has_modifier(PlannedTokenModifier::Definition));

    let unicode_reference = exact_token(source, &snapshot, &plan, "用户", 1);
    assert_eq!(unicode_reference.kind, PlannedTokenKind::Variable);
    assert!(unicode_reference.has_modifier(PlannedTokenModifier::Reference));

    let label = exact_token(source, &snapshot, &plan, "🤓 根节点", 0);
    assert_eq!(label.kind, PlannedTokenKind::String);
    assert_eq!(label.length as usize, "🤓 根节点".encode_utf16().count());
    assert!(label.has_modifier(PlannedTokenModifier::Payload));

    let description = exact_token(source, &snapshot, &plan, "描述", 0);
    let description_byte_start = source.find("描述").expect("description byte offset");
    let line_start = source[..description_byte_start]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    let expected_utf16_start = source[line_start..description_byte_start]
        .encode_utf16()
        .count();
    assert_eq!(description.start as usize, expected_utf16_start);
    assert_ne!(
        description.start as usize,
        description_byte_start - line_start
    );
}

#[test]
fn c4_recovery_plan_keeps_partial_color_number_and_later_semantics() {
    let source = concat!(
        "C4Context\r\n",
        "Rel(known, )\r\n",
        "UpdateElementStyle(known, $bgColor=\"#f00\") trailing\r\n",
        "UpdateLayoutConfig(3, 2) trailing\r\n",
        "Person(后续, \"🤓 完成\")\r\n",
    );
    let (snapshot, plan) = plan(source, "recovery");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    for kind in [
        PlannedTokenKind::Keyword,
        PlannedTokenKind::Delimiter,
        PlannedTokenKind::Operator,
        PlannedTokenKind::Color,
        PlannedTokenKind::Number,
        PlannedTokenKind::Literal,
    ] {
        assert_has_kind(&plan, kind);
    }
    assert_sorted_non_overlapping(plan.tokens());

    let partial_relation = exact_token(source, &snapshot, &plan, "known", 0);
    assert_eq!(partial_relation.kind, PlannedTokenKind::Identifier);
    assert!(partial_relation.has_modifier(PlannedTokenModifier::Reference));
    assert!(!partial_relation.has_modifier(PlannedTokenModifier::Entity));

    let color = exact_token(source, &snapshot, &plan, "#f00", 0);
    assert_eq!(color.kind, PlannedTokenKind::Color);
    let number = exact_token(source, &snapshot, &plan, "3", 0);
    assert_eq!(number.kind, PlannedTokenKind::Number);

    let later = exact_token(source, &snapshot, &plan, "后续", 0);
    assert_eq!(later.kind, PlannedTokenKind::Variable);
    assert_eq!(later.length as usize, "后续".encode_utf16().count());
    assert!(later.has_modifier(PlannedTokenModifier::Definition));
    assert!(later.has_modifier(PlannedTokenModifier::Entity));

    let later_label = exact_token(source, &snapshot, &plan, "🤓 完成", 0);
    assert_eq!(later_label.kind, PlannedTokenKind::String);
    assert_eq!(
        later_label.length as usize,
        "🤓 完成".encode_utf16().count()
    );
}

fn plan(source: &str, suffix: &str) -> (DocumentSnapshot, SemanticTokenPlan) {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            format!("file:///tmp/c4-{suffix}.mmd"),
            1,
            source.to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("C4 semantic token plan");
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
            "planner emitted overlapping or unsorted C4 tokens: {pair:?}"
        );
    }
}
