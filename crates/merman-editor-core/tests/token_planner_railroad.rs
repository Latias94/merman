mod support;

use merman_editor_core::{
    DocumentKind, DocumentSnapshot, PlannedToken, PlannedTokenKind, PlannedTokenModifier,
    SemanticTokenPlan, plan_semantic_tokens_for_snapshot,
};
use support::SnapshotHarness;

#[test]
fn railroad_dialects_plan_parser_lexemes_semantics_and_utf16_exactly() {
    let cases = [
        (
            "ir",
            concat!(
                "railroad-beta\r\n",
                "/* 家族注释 🤓 */\r\n",
                "entry = sequence(terminal(\"值 🤓\"), nonterminal(\"next\")) ;\r\n",
            ),
            "/* 家族注释 🤓 */",
            "=",
        ),
        (
            "ebnf",
            concat!(
                "railroad-ebnf-beta\r\n",
                "(* 家族注释 🤓 *)\r\n",
                "entry ::= \"值 🤓\", [ next ] ;\r\n",
            ),
            "(* 家族注释 🤓 *)",
            "::=",
        ),
        (
            "abnf",
            concat!(
                "railroad-abnf-beta\r\n",
                "; 家族注释 🤓\r\n",
                "entry = 1*2\"值 🤓\" / next ;\r\n",
            ),
            "; 家族注释 🤓",
            "=",
        ),
        (
            "peg",
            concat!(
                "railroad-peg-beta\r\n",
                "# 家族注释 🤓\r\n",
                "entry <- &next / \"值 🤓\"+ ;\r\n",
            ),
            "# 家族注释 🤓",
            "<-",
        ),
    ];

    for (suffix, source, comment, assignment) in cases {
        let (snapshot, plan) = plan(source, suffix);

        assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
        for kind in [
            PlannedTokenKind::Keyword,
            PlannedTokenKind::Comment,
            PlannedTokenKind::Operator,
            PlannedTokenKind::Delimiter,
            PlannedTokenKind::String,
            PlannedTokenKind::Function,
        ] {
            assert_has_kind(&plan, kind);
        }
        assert_sorted_non_overlapping(plan.tokens());

        let comment = exact_token(source, &snapshot, &plan, comment, 0);
        assert_eq!(comment.kind, PlannedTokenKind::Comment);
        assert_eq!(comment.length as usize, comment_text_utf16(source, comment));

        let assignment = exact_token(source, &snapshot, &plan, assignment, 0);
        assert_eq!(assignment.kind, PlannedTokenKind::Operator);

        let definition = exact_token(source, &snapshot, &plan, "entry", 0);
        assert_eq!(definition.kind, PlannedTokenKind::Function);
        assert!(definition.has_modifier(PlannedTokenModifier::Definition));

        let reference = exact_token(source, &snapshot, &plan, "next", 0);
        assert_eq!(
            reference.kind,
            PlannedTokenKind::Function,
            "{suffix} nonterminal reference kind"
        );
        assert!(reference.has_modifier(PlannedTokenModifier::Reference));

        let unicode = exact_token(source, &snapshot, &plan, "值 🤓", 0);
        assert_eq!(unicode.kind, PlannedTokenKind::String);
        assert!(unicode.has_modifier(PlannedTokenModifier::Payload));
        assert_eq!(unicode.length as usize, "值 🤓".encode_utf16().count());
        assert_ne!(unicode.length as usize, "值 🤓".len());
    }
}

#[test]
fn railroad_recovery_plan_keeps_invalid_token_and_later_rule() {
    let source = concat!(
        "railroad-peg-beta\r\n",
        "before <- \"ok\" ;\r\n",
        "@\r\n",
        "after <- \"后来 🤓\" ;\r\n",
    );
    let (snapshot, plan) = plan(source, "recovery");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    assert_sorted_non_overlapping(plan.tokens());
    let invalid = exact_token(source, &snapshot, &plan, "@", 0);
    assert_eq!(invalid.kind, PlannedTokenKind::Literal);

    let later = exact_token(source, &snapshot, &plan, "after", 0);
    assert_eq!(later.kind, PlannedTokenKind::Function);
    assert!(later.has_modifier(PlannedTokenModifier::Definition));

    let payload = exact_token(source, &snapshot, &plan, "后来 🤓", 0);
    assert_eq!(payload.kind, PlannedTokenKind::String);
    assert!(payload.has_modifier(PlannedTokenModifier::Payload));
    assert_eq!(payload.length as usize, "后来 🤓".encode_utf16().count());
}

fn plan(source: &str, suffix: &str) -> (DocumentSnapshot, SemanticTokenPlan) {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            format!("file:///tmp/railroad-{suffix}.mmd"),
            1,
            source.to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("Railroad semantic token plan");
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

fn comment_text_utf16(source: &str, token: PlannedToken) -> usize {
    source
        .lines()
        .nth(token.line as usize)
        .expect("comment line")
        .encode_utf16()
        .count()
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
            "planner emitted overlapping or unsorted Railroad tokens: {pair:?}"
        );
    }
}
