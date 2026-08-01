use merman_editor_core::{
    DocumentKind, DocumentSnapshot, DocumentWorkspace, PlannedToken, PlannedTokenKind,
    PlannedTokenModifier, SemanticTokenPlan, plan_semantic_tokens_for_snapshot,
};

#[test]
fn eventmodeling_plan_projects_parser_lexemes_semantics_and_multiline_utf16() {
    let source = concat!(
        "eventmodeling // inline 🤓\r\n",
        "title 订单流程\r\n",
        "/* 块注释\r\n",
        "   第二行 🤓 */\r\n",
        "entity CartUpdated\r\n",
        "tf 001 cmd Cart.Update\r\n",
        "rf 002 evt Cart.Updated ->> 001 [[Payload]] `json`{\"ok\": true}\r\n",
        "data Payload `json` {\r\n",
        "  \"🤓 数量\": 7\r\n",
        "}\r\n",
    );
    let (snapshot, plan) = plan(source, "complete");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    for kind in [
        PlannedTokenKind::Keyword,
        PlannedTokenKind::Operator,
        PlannedTokenKind::Delimiter,
        PlannedTokenKind::String,
        PlannedTokenKind::Comment,
    ] {
        assert!(
            plan.tokens().iter().any(|token| token.kind == kind),
            "missing {kind:?}: {:?}",
            plan.tokens()
        );
    }
    assert_sorted_non_overlapping(plan.tokens());

    let inline_comment = exact_token(source, &snapshot, &plan, "// inline 🤓", 0);
    assert_eq!(inline_comment.kind, PlannedTokenKind::Comment);
    assert_eq!(
        inline_comment.length as usize,
        "// inline 🤓".encode_utf16().count()
    );

    let block_comment_line = "   第二行 🤓 */";
    let block_comment_start = source.find(block_comment_line).unwrap();
    let block_comment_position = snapshot
        .source_map()
        .utf16_position(block_comment_start)
        .expect("block comment UTF-16 position");
    assert!(plan.tokens().iter().any(|token| {
        token.line == block_comment_position.line as u32
            && token.start == 0
            && token.length as usize == block_comment_line.encode_utf16().count()
            && token.kind == PlannedTokenKind::Comment
    }));

    let entity = exact_token(source, &snapshot, &plan, "CartUpdated", 0);
    assert_eq!(entity.kind, PlannedTokenKind::Variable);
    assert!(entity.has_modifier(PlannedTokenModifier::Definition));
    assert!(entity.has_modifier(PlannedTokenModifier::Entity));

    let frame = exact_token(source, &snapshot, &plan, "001", 0);
    assert_eq!(frame.kind, PlannedTokenKind::Namespace);
    assert!(frame.has_modifier(PlannedTokenModifier::Definition));
    assert!(frame.has_modifier(PlannedTokenModifier::Entity));

    let source_frame = exact_token(source, &snapshot, &plan, "001", 1);
    assert_eq!(source_frame.kind, PlannedTokenKind::Namespace);
    assert!(source_frame.has_modifier(PlannedTokenModifier::Reference));
    assert!(source_frame.has_modifier(PlannedTokenModifier::Entity));

    let data_reference = exact_token(source, &snapshot, &plan, "Payload", 0);
    assert_eq!(data_reference.kind, PlannedTokenKind::Namespace);
    assert!(data_reference.has_modifier(PlannedTokenModifier::Reference));

    let data_definition = exact_token(source, &snapshot, &plan, "Payload", 1);
    assert_eq!(data_definition.kind, PlannedTokenKind::Namespace);
    assert!(data_definition.has_modifier(PlannedTokenModifier::Definition));

    let body_line = "  \"🤓 数量\": 7";
    let body_start = source.find(body_line).expect("multiline data body");
    let position = snapshot
        .source_map()
        .utf16_position(body_start)
        .expect("body UTF-16 position");
    let body = plan
        .tokens()
        .iter()
        .copied()
        .find(|token| {
            token.line == position.line as u32
                && token.start == 0
                && token.kind == PlannedTokenKind::String
        })
        .expect("multiline data block must split to a String token on its body line");
    assert_eq!(body.length as usize, body_line.encode_utf16().count());
    assert_ne!(body.length as usize, body_line.len());
}

#[test]
fn eventmodeling_recovery_plan_keeps_number_literal_and_tokens_after_error() {
    let source = concat!(
        "eventmodeling\r\n",
        "entity Before\r\n",
        "tf 01 invalid Broken\r\n",
        "/* 恢复注释 🤓 */\r\n",
        "entity After\r\n",
        "tf 02 evt Done {\"🤓 后来\": true}\r\n",
    );
    let (snapshot, plan) = plan(source, "recovery");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    assert_sorted_non_overlapping(plan.tokens());
    for kind in [
        PlannedTokenKind::Keyword,
        PlannedTokenKind::Number,
        PlannedTokenKind::Literal,
        PlannedTokenKind::Comment,
    ] {
        assert!(
            plan.tokens().iter().any(|token| token.kind == kind),
            "missing recovery {kind:?}: {:?}",
            plan.tokens()
        );
    }

    let partial_frame = exact_token(source, &snapshot, &plan, "01", 0);
    assert_eq!(partial_frame.kind, PlannedTokenKind::Number);
    assert!(partial_frame.has_modifier(PlannedTokenModifier::Definition));
    assert!(!partial_frame.has_modifier(PlannedTokenModifier::Entity));

    let invalid = exact_token(source, &snapshot, &plan, "invalid", 0);
    assert_eq!(invalid.kind, PlannedTokenKind::Literal);

    let recovery_comment = exact_token(source, &snapshot, &plan, "/* 恢复注释 🤓 */", 0);
    assert_eq!(recovery_comment.kind, PlannedTokenKind::Comment);

    let after = exact_token(source, &snapshot, &plan, "After", 0);
    assert_eq!(after.kind, PlannedTokenKind::Variable);
    assert!(after.has_modifier(PlannedTokenModifier::Definition));

    for text in ["After", "后来"] {
        let offset = source.find(text).expect("recovery token offset");
        let position = snapshot
            .source_map()
            .utf16_position(offset)
            .expect("recovery UTF-16 position");
        assert!(
            plan.tokens().iter().any(|token| {
                token.line == position.line as u32
                    && token.start <= position.character as u32
                    && token.start + token.length > position.character as u32
            }),
            "recovery plan does not cover {text:?}: {:?}",
            plan.tokens()
        );
    }
}

fn plan(source: &str, suffix: &str) -> (DocumentSnapshot, SemanticTokenPlan) {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace
        .upsert(
            format!("file:///tmp/eventmodeling-{suffix}.mmd"),
            1,
            source.to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let plan =
        plan_semantic_tokens_for_snapshot(&snapshot).expect("eventmodeling semantic token plan");
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
