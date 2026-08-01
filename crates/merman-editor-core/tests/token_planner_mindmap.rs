use merman_editor_core::{
    DocumentKind, DocumentSnapshot, DocumentWorkspace, PlannedToken, PlannedTokenKind,
    PlannedTokenModifier, SemanticTokenPlan, plan_semantic_tokens_for_snapshot,
};

#[test]
fn mindmap_complete_plan_merges_multiline_parser_lexemes_and_semantics() {
    let source = concat!(
        "mindmap root[\"`根节点 🌳\r\n",
        "第二行`\"]\r\n",
        "  child[\"重复\"]\r\n",
        "  :::hot warm\r\n",
        "  ::icon(lucide:home)\r\n",
        "  %% global comment\r\n",
    );
    let (snapshot, plan) = plan(source, "complete");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    for kind in [
        PlannedTokenKind::Keyword,
        PlannedTokenKind::Delimiter,
        PlannedTokenKind::Namespace,
        PlannedTokenKind::String,
        PlannedTokenKind::Property,
        PlannedTokenKind::Comment,
    ] {
        assert_has_kind(&plan, kind);
    }
    assert_sorted_non_overlapping(plan.tokens());

    let root = exact_token(source, &snapshot, &plan, "root", 0);
    assert_eq!(root.kind, PlannedTokenKind::Namespace);
    assert!(root.has_modifier(PlannedTokenModifier::Definition));
    assert!(root.has_modifier(PlannedTokenModifier::Entity));

    let child = exact_token(source, &snapshot, &plan, "child", 0);
    assert_eq!(child.kind, PlannedTokenKind::Namespace);
    assert!(child.has_modifier(PlannedTokenModifier::Definition));

    let label = exact_token(source, &snapshot, &plan, "重复", 0);
    assert_eq!(label.kind, PlannedTokenKind::String);
    assert_eq!(label.length as usize, "重复".encode_utf16().count());
    assert!(label.has_modifier(PlannedTokenModifier::Payload));

    let class = exact_token(source, &snapshot, &plan, "hot", 0);
    assert_eq!(class.kind, PlannedTokenKind::Property);
    assert!(class.has_modifier(PlannedTokenModifier::Reference));
    assert!(class.has_modifier(PlannedTokenModifier::Payload));

    let icon = exact_token(source, &snapshot, &plan, "lucide:home", 0);
    assert_eq!(icon.kind, PlannedTokenKind::String);
    assert!(icon.has_modifier(PlannedTokenModifier::Reference));
    assert!(icon.has_modifier(PlannedTokenModifier::Payload));

    for text in ["根节点 🌳", "第二行"] {
        assert_token_covers(source, &snapshot, &plan, text);
    }
    let emoji_offset = source.find('🌳').expect("emoji offset");
    let emoji_position = snapshot
        .source_map()
        .utf16_position(emoji_offset)
        .expect("emoji UTF-16 position");
    assert!(plan.tokens().iter().any(|token| {
        token.kind == PlannedTokenKind::String
            && token.line == emoji_position.line as u32
            && token.start <= emoji_position.character as u32
            && token.start + token.length > emoji_position.character as u32
    }));
}

#[test]
fn mindmap_recovery_plan_keeps_confirmed_prefix_and_later_overlay() {
    let source = concat!(
        "mindmap\r\n",
        " root\r\n",
        "  broken[unterminated\r\n",
        "  后续(\"完成\")\r\n",
        "  :::hot\r\n",
    );
    let (snapshot, plan) = plan(source, "recovery");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    assert_sorted_non_overlapping(plan.tokens());

    let broken = exact_token(source, &snapshot, &plan, "broken", 0);
    assert_eq!(broken.kind, PlannedTokenKind::Identifier);
    assert!(broken.has_modifier(PlannedTokenModifier::Definition));
    assert!(!broken.has_modifier(PlannedTokenModifier::Entity));

    let later = exact_token(source, &snapshot, &plan, "后续", 0);
    assert_eq!(later.kind, PlannedTokenKind::Namespace);
    assert_eq!(later.length as usize, "后续".encode_utf16().count());
    assert!(later.has_modifier(PlannedTokenModifier::Definition));
    assert!(later.has_modifier(PlannedTokenModifier::Entity));

    let label = exact_token(source, &snapshot, &plan, "完成", 0);
    assert_eq!(label.kind, PlannedTokenKind::String);
    assert!(label.has_modifier(PlannedTokenModifier::Payload));

    let class = exact_token(source, &snapshot, &plan, "hot", 0);
    assert_eq!(class.kind, PlannedTokenKind::Property);
    assert!(class.has_modifier(PlannedTokenModifier::Reference));
}

fn plan(source: &str, suffix: &str) -> (DocumentSnapshot, SemanticTokenPlan) {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace
        .upsert(
            format!("file:///tmp/mindmap-{suffix}.mmd"),
            1,
            source.to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("mindmap semantic token plan");
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

fn assert_token_covers(
    source: &str,
    snapshot: &DocumentSnapshot,
    plan: &SemanticTokenPlan,
    text: &str,
) {
    let offset = source.find(text).expect("covered text offset");
    let position = snapshot
        .source_map()
        .utf16_position(offset)
        .expect("covered text UTF-16 position");
    assert!(plan.tokens().iter().any(|token| {
        token.line == position.line as u32
            && token.start <= position.character as u32
            && token.start + token.length > position.character as u32
    }));
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
            "planner emitted overlapping or unsorted mindmap tokens: {pair:?}",
        );
    }
}
