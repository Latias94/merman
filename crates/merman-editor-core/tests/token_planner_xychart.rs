mod support;

use merman_editor_core::{
    DocumentKind, DocumentSnapshot, PlannedToken, PlannedTokenKind, PlannedTokenModifier,
    SemanticTokenPlan, plan_semantic_tokens_for_snapshot,
};
use support::SnapshotHarness;

#[test]
fn xychart_parser_tokens_plan_across_crlf_unicode_repeated_values_and_overlay() {
    let source = concat!(
        "%% 全局注释\r\n",
        "xychart horizontal\r\n",
        "title \"收入 计划 🤓\"; accTitle: 可访问标题\r\n",
        "x-axis \"季度\" [\"重复\", \"重复\"]\r\n",
        "y-axis 金额 -5 --> 10\r\n",
        "line \"系列 🤓\" [5 \"重复\", 5 \"重复\"]\r\n",
        "bar [3, 4] %% 行尾注释\r\n",
    );
    let (snapshot, plan) = plan(source, "valid");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    assert_sorted_non_overlapping(plan.tokens());
    for kind in [
        PlannedTokenKind::Keyword,
        PlannedTokenKind::Literal,
        PlannedTokenKind::String,
        PlannedTokenKind::Delimiter,
        PlannedTokenKind::Number,
        PlannedTokenKind::Operator,
        PlannedTokenKind::Comment,
    ] {
        assert!(
            plan.tokens().iter().any(|token| token.kind == kind),
            "missing {kind:?}: {:?}",
            plan.tokens()
        );
    }

    let title = exact_token(source, &snapshot, &plan, "收入 计划 🤓", 0);
    assert_eq!(title.kind, PlannedTokenKind::String);
    assert!(title.has_modifier(PlannedTokenModifier::Payload));
    assert_eq!(title.length as usize, "收入 计划 🤓".encode_utf16().count());
    assert_ne!(title.length as usize, "收入 计划 🤓".len());

    let plot_title = exact_token(source, &snapshot, &plan, "系列 🤓", 0);
    assert_eq!(plot_title.kind, PlannedTokenKind::String);
    assert!(plot_title.has_modifier(PlannedTokenModifier::Payload));

    for occurrence in 0..4 {
        let repeated = exact_token(source, &snapshot, &plan, "重复", occurrence);
        assert_eq!(repeated.kind, PlannedTokenKind::String);
        assert_eq!(
            repeated.has_modifier(PlannedTokenModifier::Payload),
            occurrence >= 2,
            "only parsed point labels are semantic payloads"
        );
    }

    assert_eq!(
        exact_token(source, &snapshot, &plan, "%% 全局注释", 0).kind,
        PlannedTokenKind::Comment
    );
    assert_eq!(
        exact_token(source, &snapshot, &plan, "%% 行尾注释", 0).kind,
        PlannedTokenKind::Comment
    );
}

#[test]
fn xychart_recovery_plan_keeps_prefix_invalid_token_and_safe_later_statement() {
    let source = concat!(
        "xychart\r\n",
        "title Before\r\n",
        "bar [1, -4aa5]\r\n",
        "line \"After 后来 🤓\" [2 \"后来\"]\r\n",
    );
    let (snapshot, plan) = plan(source, "recovery");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    assert_sorted_non_overlapping(plan.tokens());
    for (text, kind) in [
        ("bar", PlannedTokenKind::Keyword),
        ("1", PlannedTokenKind::Number),
        ("-4aa5", PlannedTokenKind::Literal),
        ("line", PlannedTokenKind::Keyword),
        ("After 后来 🤓", PlannedTokenKind::String),
        ("2", PlannedTokenKind::Number),
    ] {
        assert_eq!(exact_token(source, &snapshot, &plan, text, 0).kind, kind);
    }

    let later = exact_token(source, &snapshot, &plan, "After 后来 🤓", 0);
    assert!(later.has_modifier(PlannedTokenModifier::Payload));
    assert_eq!(
        later.length as usize,
        "After 后来 🤓".encode_utf16().count()
    );
}

#[test]
fn xychart_eof_recovery_plan_keeps_partial_string() {
    let source = "xychart\r\ntitle \"未完成 🤓";
    let (snapshot, plan) = plan(source, "eof");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    assert_sorted_non_overlapping(plan.tokens());
    assert_eq!(
        exact_token(source, &snapshot, &plan, "\"", 0).kind,
        PlannedTokenKind::Delimiter
    );
    let body = exact_token(source, &snapshot, &plan, "未完成 🤓", 0);
    assert_eq!(body.kind, PlannedTokenKind::String);
    assert_eq!(body.length as usize, "未完成 🤓".encode_utf16().count());
}

fn plan(source: &str, suffix: &str) -> (DocumentSnapshot, SemanticTokenPlan) {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            format!("file:///tmp/xychart-{suffix}.mmd"),
            1,
            source.to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("XYChart semantic token plan");
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

fn assert_sorted_non_overlapping(tokens: &[PlannedToken]) {
    for pair in tokens.windows(2) {
        assert!(
            (pair[0].line, pair[0].start + pair[0].length) <= (pair[1].line, pair[1].start),
            "planner emitted overlapping or unsorted tokens: {pair:?}"
        );
    }
}
