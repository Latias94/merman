use merman_editor_core::{
    DocumentKind, DocumentSnapshot, DocumentWorkspace, PlannedToken, PlannedTokenKind,
    PlannedTokenModifier, SemanticTokenPlan, plan_semantic_tokens_for_snapshot,
};

#[test]
fn kanban_complete_plan_projects_parser_lexemes_semantics_and_utf16() {
    let source = concat!(
        "kanban\r\n",
        "%% global comment 🤓\r\n",
        "  todo[\"重复\"]@{\r\n",
        "    ticket: 2038\r\n",
        "    assigned: \"重复\"\r\n",
        "    priority: 'High'\r\n",
        "    active: true\r\n",
        "  }\r\n",
        "    task((\"🤓 后续\"))\r\n",
        "    ::ICON(star)\r\n",
        "    :::urgent\r\n",
    );
    let (snapshot, plan) = plan(source, "complete");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    assert_sorted_non_overlapping(plan.tokens());
    for kind in [
        PlannedTokenKind::Keyword,
        PlannedTokenKind::Comment,
        PlannedTokenKind::Delimiter,
        PlannedTokenKind::Number,
        PlannedTokenKind::Boolean,
        PlannedTokenKind::Property,
        PlannedTokenKind::Class,
    ] {
        assert_has_kind(&plan, kind);
    }

    let section = exact_token(source, &snapshot, &plan, "todo", 0);
    assert_eq!(section.kind, PlannedTokenKind::Namespace);
    assert!(section.has_modifier(PlannedTokenModifier::Definition));
    assert!(section.has_modifier(PlannedTokenModifier::Outline));

    let item = exact_token(source, &snapshot, &plan, "task", 0);
    assert_eq!(item.kind, PlannedTokenKind::Variable);
    assert!(item.has_modifier(PlannedTokenModifier::Definition));
    assert!(item.has_modifier(PlannedTokenModifier::Entity));

    for occurrence in 0..2 {
        let repeated = exact_token(source, &snapshot, &plan, "重复", occurrence);
        assert_eq!(repeated.kind, PlannedTokenKind::String);
        assert!(repeated.has_modifier(PlannedTokenModifier::Payload));
    }

    let metadata_key = exact_token(source, &snapshot, &plan, "assigned", 0);
    assert_eq!(metadata_key.kind, PlannedTokenKind::Property);
    assert!(metadata_key.has_modifier(PlannedTokenModifier::Payload));

    assert_eq!(
        exact_token(source, &snapshot, &plan, "2038", 0).kind,
        PlannedTokenKind::Number
    );
    assert_eq!(
        exact_token(source, &snapshot, &plan, "true", 0).kind,
        PlannedTokenKind::Boolean
    );
    assert_eq!(
        exact_token(source, &snapshot, &plan, "::ICON", 0).kind,
        PlannedTokenKind::Keyword
    );

    let class = exact_token(source, &snapshot, &plan, "urgent", 0);
    assert_eq!(class.kind, PlannedTokenKind::Class);
    assert!(class.has_modifier(PlannedTokenModifier::Reference));
    assert!(class.has_modifier(PlannedTokenModifier::Payload));

    let unicode = exact_token(source, &snapshot, &plan, "🤓 后续", 0);
    assert_eq!(unicode.kind, PlannedTokenKind::String);
    assert_eq!(unicode.length as usize, "🤓 后续".encode_utf16().count());
    assert_ne!(unicode.length as usize, "🤓 后续".len());
}

#[test]
fn kanban_recovery_plan_keeps_partial_and_later_safe_tokens() {
    let source = concat!(
        "kanban\r\n",
        "  todo[\"之前\"]\r\n",
        "    broken[\"Open 🤓\r\n",
        "    invalid[Valid] trailing\r\n",
        "    later[\"后来\"]@{ ticket: 42, active: false }\r\n",
    );
    let (snapshot, plan) = plan(source, "recovery");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    assert_sorted_non_overlapping(plan.tokens());
    for kind in [
        PlannedTokenKind::Keyword,
        PlannedTokenKind::Delimiter,
        PlannedTokenKind::Literal,
        PlannedTokenKind::Number,
        PlannedTokenKind::Boolean,
    ] {
        assert_has_kind(&plan, kind);
    }

    let broken = exact_token(source, &snapshot, &plan, "broken", 0);
    assert_eq!(broken.kind, PlannedTokenKind::Identifier);
    assert!(broken.has_modifier(PlannedTokenModifier::Definition));
    assert!(!broken.has_modifier(PlannedTokenModifier::Entity));

    let partial_label = exact_token(source, &snapshot, &plan, "Open 🤓", 0);
    assert_eq!(partial_label.kind, PlannedTokenKind::String);
    assert_eq!(
        partial_label.length as usize,
        "Open 🤓".encode_utf16().count()
    );

    assert_eq!(
        exact_token(source, &snapshot, &plan, "trailing", 0).kind,
        PlannedTokenKind::Literal
    );
    assert_eq!(
        exact_token(source, &snapshot, &plan, "42", 0).kind,
        PlannedTokenKind::Number
    );
    assert_eq!(
        exact_token(source, &snapshot, &plan, "false", 0).kind,
        PlannedTokenKind::Boolean
    );

    let later = exact_token(source, &snapshot, &plan, "later", 0);
    assert_eq!(later.kind, PlannedTokenKind::Variable);
    assert!(later.has_modifier(PlannedTokenModifier::Definition));
    assert!(later.has_modifier(PlannedTokenModifier::Entity));

    let later_label = exact_token(source, &snapshot, &plan, "后来", 0);
    assert_eq!(later_label.kind, PlannedTokenKind::String);
    assert_eq!(later_label.length as usize, "后来".encode_utf16().count());
}

fn plan(source: &str, suffix: &str) -> (DocumentSnapshot, SemanticTokenPlan) {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace
        .upsert(
            format!("file:///tmp/kanban-{suffix}.mmd"),
            1,
            source.to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("Kanban semantic token plan");
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
            "planner emitted overlapping or unsorted Kanban tokens: {pair:?}"
        );
    }
}
