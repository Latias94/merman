use merman_editor_core::{
    DocumentKind, DocumentSnapshot, DocumentWorkspace, PlannedToken, PlannedTokenKind,
    PlannedTokenModifier, SemanticTokenPlan, plan_semantic_tokens_for_snapshot,
};

#[test]
fn timeline_complete_plan_merges_parser_lexemes_semantics_and_utf16_exactly() {
    let source = concat!(
        "timeline LR\r\n",
        "# family comment 🤓\r\n",
        "title 项目时间轴\r\n",
        "accTitle: 可访问标题\r\n",
        "accDescr: 行内说明\r\n",
        "accDescr {\r\n",
        "  多行说明 🤓\r\n",
        "}\r\n",
        "section 规划\r\n",
        "🤓 任务: 首个事件: 后续\r\n",
        ": 续行\r\n",
    );
    let (snapshot, plan) = plan(source, "complete");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    for kind in [
        PlannedTokenKind::Keyword,
        PlannedTokenKind::Comment,
        PlannedTokenKind::Literal,
        PlannedTokenKind::Delimiter,
        PlannedTokenKind::String,
        PlannedTokenKind::Namespace,
        PlannedTokenKind::Event,
    ] {
        assert_has_kind(&plan, kind);
    }
    assert_sorted_non_overlapping(plan.tokens());

    let section = exact_token(source, &snapshot, &plan, "规划", 0);
    assert_eq!(section.kind, PlannedTokenKind::Namespace);
    assert!(section.has_modifier(PlannedTokenModifier::Outline));

    let task = exact_token(source, &snapshot, &plan, "🤓 任务", 0);
    assert_eq!(task.kind, PlannedTokenKind::Event);
    assert_eq!(task.length as usize, "🤓 任务".encode_utf16().count());
    assert_ne!(task.length as usize, "🤓 任务".len());
    assert!(task.has_modifier(PlannedTokenModifier::Outline));

    let event = exact_token(source, &snapshot, &plan, "首个事件", 0);
    assert_eq!(event.kind, PlannedTokenKind::String);
    assert!(event.has_modifier(PlannedTokenModifier::Payload));
    let event_byte_start = source.find("首个事件").expect("event byte offset");
    let line_start = source[..event_byte_start]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    let expected_utf16_start = source[line_start..event_byte_start].encode_utf16().count();
    assert_eq!(event.start as usize, expected_utf16_start);
    assert_ne!(event.start as usize, event_byte_start - line_start);

    for text in [
        "项目时间轴",
        "可访问标题",
        "行内说明",
        "多行说明 🤓",
        "后续",
        "续行",
    ] {
        let token = exact_token(source, &snapshot, &plan, text, 0);
        assert_eq!(token.kind, PlannedTokenKind::String);
        assert!(token.has_modifier(PlannedTokenModifier::Payload));
    }
}

#[test]
fn timeline_recovery_plan_keeps_confirmed_prefix_invalid_tail_and_later_semantics() {
    let source = concat!(
        "timeline TD\r\n",
        "section Before\r\n",
        "🤓 Broken:event\r\n",
        "After: 后续\r\n",
    );
    let (snapshot, plan) = plan(source, "recovery");

    assert_eq!(plan.packed().len(), plan.tokens().len() * 5);
    for kind in [
        PlannedTokenKind::Keyword,
        PlannedTokenKind::Literal,
        PlannedTokenKind::Delimiter,
        PlannedTokenKind::Namespace,
        PlannedTokenKind::Event,
        PlannedTokenKind::String,
    ] {
        assert_has_kind(&plan, kind);
    }
    assert_sorted_non_overlapping(plan.tokens());

    let section = exact_token(source, &snapshot, &plan, "Before", 0);
    assert_eq!(section.kind, PlannedTokenKind::Namespace);
    assert!(section.has_modifier(PlannedTokenModifier::Outline));

    let broken_task = exact_token(source, &snapshot, &plan, "🤓 Broken", 0);
    assert_eq!(broken_task.kind, PlannedTokenKind::Event);
    assert!(broken_task.has_modifier(PlannedTokenModifier::Outline));
    assert_eq!(
        broken_task.length as usize,
        "🤓 Broken".encode_utf16().count()
    );

    let invalid = exact_token(source, &snapshot, &plan, "event", 0);
    assert_eq!(invalid.kind, PlannedTokenKind::Literal);

    let later_task = exact_token(source, &snapshot, &plan, "After", 0);
    assert_eq!(later_task.kind, PlannedTokenKind::Event);
    assert!(later_task.has_modifier(PlannedTokenModifier::Outline));
    let later_event = exact_token(source, &snapshot, &plan, "后续", 0);
    assert_eq!(later_event.kind, PlannedTokenKind::String);
    assert!(later_event.has_modifier(PlannedTokenModifier::Payload));
}

fn plan(source: &str, suffix: &str) -> (DocumentSnapshot, SemanticTokenPlan) {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace.upsert(
        format!("file:///tmp/timeline-{suffix}.mmd"),
        1,
        source.to_string(),
        DocumentKind::Diagram,
    );
    let plan = plan_semantic_tokens_for_snapshot(&snapshot).expect("Timeline semantic token plan");
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
        .source_map
        .utf16_position(start)
        .expect("token start UTF-16 position");
    let end = snapshot
        .source_map
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
            "planner emitted overlapping or unsorted Timeline tokens: {pair:?}"
        );
    }
}
