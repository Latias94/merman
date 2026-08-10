use super::*;

#[test]
fn sequence_autonumber_accepts_decimal_start_and_step() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nautonumber 10.1 .01\nA->>B: First\nB-->>A: Second\nA->>B: Third",
        &AsciiRenderOptions::unicode(),
    )
    .expect("sequence decimal autonumber should render");
    let normalized = normalize_sequence_output(&rendered);

    assert!(
        normalized.contains("10.1. First"),
        "expected decimal start in ASCII output:\n{normalized}"
    );
    assert!(
        normalized.contains("10.11. Second"),
        "expected rounded decimal step in ASCII output:\n{normalized}"
    );
    assert!(
        normalized.contains("10.12. Third"),
        "expected second rounded decimal step in ASCII output:\n{normalized}"
    );
    assert!(
        !normalized.contains("10.110000"),
        "expected decimal labels to avoid floating point artifacts:\n{normalized}"
    );
}

#[test]
fn sequence_autonumber_off_preserves_and_advances_hidden_state() {
    let rendered = render_sequence(
        r#"sequenceDiagram
participant A
participant B
autonumber 10 5
A->>B: Visible first
autonumber off
A->>B: Hidden first
B-->>A: Hidden second
autonumber
A->>B: Visible resumed"#,
        &AsciiRenderOptions::unicode(),
    )
    .expect("sequence autonumber visibility changes should render");
    let normalized = normalize_sequence_output(&rendered);

    assert!(
        normalized.contains("10. Visible first"),
        "expected configured autonumber start before disabling it:\n{normalized}"
    );
    assert!(
        normalized.contains("Hidden first")
            && !normalized.contains("15. Hidden first")
            && normalized.contains("Hidden second")
            && !normalized.contains("20. Hidden second"),
        "hidden signals should advance the counter without displaying it:\n{normalized}"
    );
    assert!(
        normalized.contains("25. Visible resumed"),
        "reenabling autonumber should retain the counter and step across hidden signals:\n{normalized}"
    );
}

#[test]
fn sequence_notes_render_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Start\nNote right of A: right\nNote left of B: left\nNote over A,B: over both\nB-->>A: Done",
        &AsciiRenderOptions::unicode(),
    )
    .expect("single-line sequence notes should render");

    assert!(
        rendered.contains("│ right │"),
        "right-of note should render as a note box:\n{rendered}"
    );
    assert!(
        rendered.contains("│ left │"),
        "left-of note should render as a note box:\n{rendered}"
    );
    assert!(
        rendered.contains("│ over both │"),
        "over note should render as a note box:\n{rendered}"
    );
    assert!(
        rendered.contains("├────────►│"),
        "normal messages around notes should keep rendering:\n{rendered}"
    );
}

#[test]
fn sequence_note_left_of_leftmost_actor_reserves_a_lifeline_gutter() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nNote left of A: Left\nA->>B: Ping",
        &AsciiRenderOptions::unicode(),
    )
    .expect("leftmost actor note should render with a dedicated gutter");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('┌') && line.contains("┐  │")),
        "left-of note should end before the leftmost lifeline with the configured gap:\n{rendered}"
    );
    assert!(
        rendered.contains("Left") && rendered.contains("Ping"),
        "note gutters should preserve both note and message content:\n{rendered}"
    );
}

#[test]
fn sequence_multiline_notes_render_from_typed_model() {
    let mut model = basic_sequence_model();
    model.notes.push(SequenceNote {
        actor: "A".into(),
        message: "line 1\nline 2".to_string(),
        placement: 1,
        wrap: false,
    });
    model.messages.push(SequenceMessage {
        id: "n0".to_string(),
        from: Some("A".to_string()),
        to: Some("A".to_string()),
        message_type: 2,
        message: SequenceMessagePayload::Text("line 1\nline 2".to_string()),
        wrap: false,
        activate: false,
        placement: Some(1),
        central_connection: 0,
    });

    let rendered = render_sequence_model(&model, &AsciiRenderOptions::ascii())
        .expect("sequence should render");
    let lines = normalize_sequence_output(&rendered)
        .lines()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let line_1 = lines
        .iter()
        .position(|line| line.contains("line 1"))
        .expect("first note line should render");
    let line_2 = lines
        .iter()
        .position(|line| line.contains("line 2"))
        .expect("second note line should render");

    assert_eq!(
        line_2,
        line_1 + 1,
        "multiline notes should render as adjacent note content rows:\n{rendered}"
    );
}

#[test]
fn sequence_note_html_breaks_render_multiline_note_boxes() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nA->>A: Ping\nNote right of A: First<br/>Second",
        &AsciiRenderOptions::ascii(),
    )
    .expect("sequence should render");
    let lines = normalize_sequence_output(&rendered)
        .lines()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let first = lines
        .iter()
        .position(|line| line.contains("First"))
        .expect("first note line should render");
    let second = lines
        .iter()
        .position(|line| line.contains("Second"))
        .expect("second note line should render");

    assert_eq!(
        second,
        first + 1,
        "HTML line breaks should become adjacent note content rows:\n{rendered}"
    );
    assert!(
        !rendered.contains("<br"),
        "HTML break markup should not leak into ASCII output:\n{rendered}"
    );
}

#[test]
fn sequence_wrapped_messages_render_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B:wrap: Alpha Beta Gamma",
        &AsciiRenderOptions::unicode(),
    )
    .expect("wrapped sequence messages should render");

    assert!(
        rendered.contains("Alpha") && rendered.contains("Beta") && rendered.contains("Gamma"),
        "wrapped message should keep all words:\n{rendered}"
    );
    assert!(
        !rendered.contains("Alpha Beta Gamma"),
        "wrapped message should not render as one long line:\n{rendered}"
    );
}

#[test]
fn sequence_wrapped_notes_render_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nNote over A,B:wrap: Alpha Beta Gamma Delta Epsilon Zeta",
        &AsciiRenderOptions::unicode(),
    )
    .expect("wrapped sequence notes should render");

    assert!(
        rendered.contains("Alpha") && rendered.contains("Zeta"),
        "wrapped note should keep all words:\n{rendered}"
    );
    assert!(
        !rendered.contains("Alpha Beta Gamma Delta Epsilon Zeta"),
        "wrapped note should not render as one long line:\n{rendered}"
    );
}

#[test]
fn sequence_wrapped_messages_respect_display_width_for_cjk() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B:wrap: 数据数据数据数据",
        &AsciiRenderOptions::unicode(),
    )
    .expect("wrapped CJK sequence messages should render");

    assert!(
        !rendered.contains("数据数据数据数据"),
        "wide text without spaces should wrap by display width:\n{rendered}"
    );
}

#[test]
fn sequence_message_labels_reserve_display_cells_for_cjk() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: 数据",
        &AsciiRenderOptions::ascii(),
    )
    .expect("CJK sequence messages should render");

    let label_line = rendered
        .lines()
        .find(|line| line.contains("数据"))
        .expect("message label should be rendered");
    let arrow_line = rendered
        .lines()
        .find(|line| line.contains('>'))
        .expect("message arrow should be rendered");

    assert_eq!(
        cjk_test_width(label_line),
        cjk_test_width(arrow_line),
        "message labels should reserve the same terminal columns as the arrow row:\n{rendered}"
    );
}

#[test]
fn sequence_boxes_render_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nbox green Group 1\nparticipant A\nparticipant B\nend\nA->>B: Inside",
        &AsciiRenderOptions::unicode(),
    )
    .expect("sequence boxes should render");

    assert!(
        rendered
            .lines()
            .next()
            .is_some_and(|line| line.contains("Group 1")),
        "box title should render in the enclosing box border:\n{rendered}"
    );
    assert!(
        rendered.contains("│ A │") && rendered.contains("│ B │"),
        "boxed participants should keep rendering:\n{rendered}"
    );
    assert!(
        rendered.contains("├────────►│"),
        "messages inside boxes should keep rendering:\n{rendered}"
    );
}

#[test]
fn sequence_wrapped_boxes_render_multiline_labels() {
    let rendered = render_sequence(
        "sequenceDiagram\nbox :wrap: Alpha Beta Gamma Delta\nparticipant A\nend\nA->>A: Ping",
        &AsciiRenderOptions::ascii(),
    )
    .expect("wrapped sequence boxes should render");
    let normalized = normalize_sequence_output(&rendered);
    let lines = normalized.lines().collect::<Vec<_>>();
    let alpha = lines
        .iter()
        .position(|line| line.contains("Alpha"))
        .expect("first wrapped box label line should render");
    let gamma = lines
        .iter()
        .position(|line| line.contains("Gamma"))
        .expect("second wrapped box label line should render");

    assert_eq!(
        gamma,
        alpha + 1,
        "wrapped box label lines should stay adjacent above participant content:\n{rendered}"
    );
    assert!(
        rendered.contains("Beta") && rendered.contains("Delta"),
        "wrapped box labels should keep all words:\n{rendered}"
    );
    assert!(
        !rendered.contains("Alpha Beta Gamma Delta") && !rendered.contains(":wrap:"),
        "wrapped box label should not render as one long line or leak wrap syntax:\n{rendered}"
    );
}

#[test]
fn sequence_box_html_breaks_render_multiline_labels() {
    let rendered = render_sequence(
        "sequenceDiagram\nbox First<br/>Second\nparticipant A\nend\nA->>A: Ping",
        &AsciiRenderOptions::ascii(),
    )
    .expect("sequence box labels with HTML breaks should render");
    let normalized = normalize_sequence_output(&rendered);
    let lines = normalized.lines().collect::<Vec<_>>();
    let first = lines
        .iter()
        .position(|line| line.contains("First"))
        .expect("first box label line should render");
    let second = lines
        .iter()
        .position(|line| line.contains("Second"))
        .expect("second box label line should render");

    assert_eq!(
        second,
        first + 1,
        "HTML breaks in box labels should create adjacent label rows:\n{rendered}"
    );
    assert!(
        !rendered.contains("<br"),
        "HTML break markup should not leak into sequence box labels:\n{rendered}"
    );
}

#[test]
fn sequence_boxes_with_unknown_actors_are_explicitly_unsupported() {
    let mut model = basic_sequence_model();
    model.boxes.push(SequenceBox {
        actor_keys: vec!["B".to_string()],
        fill: "green".to_string(),
        name: Some("Group".to_string()),
        wrap: false,
    });

    assert_unsupported_sequence_model(model, "boxes with unknown actors");
}

#[test]
fn sequence_empty_boxes_render_as_diagram_wide_regions() {
    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.boxes.push(SequenceBox {
        actor_keys: Vec::new(),
        fill: "green".to_string(),
        name: Some("System boundary".to_string()),
        wrap: false,
    });
    model.messages.push(message(Some("A"), Some("B"), 0));

    let rendered = render_sequence_model(&model, &AsciiRenderOptions::ascii())
        .expect("empty sequence boxes should render as diagram-wide regions");

    assert!(
        rendered.contains("System boundary"),
        "empty box title should remain visible:\n{rendered}"
    );
    assert!(
        rendered.contains("Hi"),
        "empty box should preserve sequence contents:\n{rendered}"
    );
    assert!(
        rendered.contains("+"),
        "empty box should render a terminal region border:\n{rendered}"
    );
}

#[test]
fn sequence_activations_render_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>+B: Start\nB-->>A: Working\nB-->>-A: Done",
        &AsciiRenderOptions::unicode(),
    )
    .expect("sequence activations should render");

    assert!(
        rendered.contains("┃"),
        "active participant lifeline should render with an activation bar:\n{rendered}"
    );
    assert!(
        rendered.contains("│ Working"),
        "messages should still render while a participant is active:\n{rendered}"
    );
}

#[test]
fn sequence_global_wrap_does_not_invalidate_empty_activation_records() {
    let rendered = render_sequence(
        "%%{wrap}%%\nsequenceDiagram\nparticipant A\nparticipant B\nactivate B\nA->>B: Work\ndeactivate B",
        &AsciiRenderOptions::unicode(),
    )
    .expect("global wrapping is harmless metadata on empty activation records");

    assert!(rendered.contains("Work") && rendered.contains('┃'));
}

#[test]
fn sequence_actor_lifecycle_renders_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Before\ncreate participant C\nB->>C: Hello C\nC->>B: Still here\ndestroy C\nB--xC: Bye C\nB->>A: After",
        &AsciiRenderOptions::unicode(),
    )
    .expect("sequence actor create/destroy should render");

    let header = rendered.lines().take(3).collect::<Vec<_>>().join("\n");
    assert!(
        !header.contains("│ C │"),
        "created participant should not render in the initial header:\n{rendered}"
    );
    assert_eq!(
        rendered.matches("│ C │").count(),
        1,
        "created participant should render once at its creation point:\n{rendered}"
    );
    assert!(
        rendered.contains("×"),
        "destroyed participant should render a termination marker:\n{rendered}"
    );
}

#[test]
fn sequence_note_after_destroy_uses_the_static_actor_anchor() {
    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.messages.push(message(Some("A"), Some("B"), 0));
    model.destroyed_actors.insert("B".to_string(), 0);

    let mut note = message(Some("B"), Some("B"), 2);
    note.message = SequenceMessagePayload::Text("After destroy".to_string());
    note.placement = Some(1);
    model.messages.push(note);

    let rendered = render_sequence_model(&model, &AsciiRenderOptions::unicode())
        .expect("notes should retain static anchors after actor destruction");

    assert!(
        rendered.contains('×') && rendered.contains("After destroy"),
        "destroy marker and later anchored note should both remain visible:\n{rendered}"
    );
}

#[test]
fn sequence_actor_lifecycle_validates_hand_built_indices() {
    let mut cases = Vec::new();

    let mut model = basic_sequence_model();
    model.messages.push(message(Some("A"), Some("A"), 0));
    model.created_actors.insert("B".to_string(), 0);
    cases.push((model, "actor lifecycle actors"));

    let mut model = basic_sequence_model();
    model.messages.push(message(Some("A"), Some("A"), 0));
    model.created_actors.insert("A".to_string(), 2);
    cases.push((model, "actor lifecycle message indices"));

    let mut model = basic_sequence_model();
    model.messages.push(message(Some("A"), Some("A"), 0));
    model.created_actors.insert("A".to_string(), 1);
    cases.push((model, "actor creation messages"));

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.messages.push(message(Some("A"), Some("A"), 0));
    model.created_actors.insert("B".to_string(), 0);
    cases.push((model, "actor creation messages"));

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.messages.push(message(Some("A"), Some("A"), 0));
    model.destroyed_actors.insert("B".to_string(), 0);
    cases.push((model, "actor destruction messages"));

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.messages.push(message(Some("A"), Some("B"), 0));
    model.messages.push(message(Some("A"), Some("B"), 0));
    model.destroyed_actors.insert("B".to_string(), 0);
    cases.push((model, "actor lifecycle visibility"));

    for (model, feature) in cases {
        assert_unsupported_sequence_model(model, feature);
    }
}

#[test]
fn sequence_actor_lifecycle_finds_the_signal_after_intervening_records() {
    for (input, actor, expected_label, expected_signal) in [
        (
            concat!(
                "sequenceDiagram\n",
                "participant A\n",
                "create participant B\n",
                "Note over A: pending\n",
                "autonumber\n",
                "A->>B: ready\n",
            ),
            "B",
            "pending",
            "ready",
        ),
        (
            concat!(
                "sequenceDiagram\n",
                "participant A\n",
                "participant B\n",
                "destroy B\n",
                "Note over A: closing\n",
                "autonumber\n",
                "A--xB: bye\n",
            ),
            "B",
            "closing",
            "bye",
        ),
    ] {
        let model = parse_sequence_render_model(input);
        let lifecycle_index = model
            .created_actors
            .get(actor)
            .or_else(|| model.destroyed_actors.get(actor));
        assert_eq!(lifecycle_index, Some(&0));
        assert_eq!(model.messages[0].message_type, 2, "the anchor is a note");
        assert_eq!(
            model.messages[1].message_type, LINETYPE_AUTONUMBER,
            "autonumber is also retained before the associated signal"
        );

        let rendered = render_sequence_model(&model, &AsciiRenderOptions::unicode())
            .expect("lifecycle endpoint validation must use the later associated signal");
        assert!(
            rendered.contains(expected_label) && rendered.contains(expected_signal),
            "intervening records and the associated lifecycle signal must remain visible:\n{rendered}"
        );
    }
}
