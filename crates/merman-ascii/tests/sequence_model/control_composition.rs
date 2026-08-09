use super::*;

#[test]
fn sequence_nested_control_blocks_render() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nloop Outer\nopt Inner\nA->>B: Work\nend\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("nested control blocks should render");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("┌ loop Outer ")),
        "outer frame should render:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("│ ┌ opt Inner ")),
        "inner frame should render inside the outer frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("│ │") && line.contains("Work")),
        "message rows should stay inside both frames:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("│ ├") && line.contains('►')),
        "nested frame borders must not replace the message source junction:\n{rendered}"
    );
}

#[test]
fn sequence_empty_control_block_sections_render_visible_lifeline_rows() {
    let mut cases = Vec::new();

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model
        .messages
        .push(message(None, None, LINETYPE_LOOP_START));
    model.messages.push(empty_message(LINETYPE_LOOP_END));
    cases.push(("loop", model, None));

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.messages.push(message(None, None, LINETYPE_ALT_START));
    model.messages.push(message(None, None, LINETYPE_ALT_ELSE));
    model.messages.push(empty_message(LINETYPE_ALT_END));
    cases.push(("alt", model, Some("else")));

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.messages.push(message(None, None, LINETYPE_PAR_START));
    model.messages.push(message(None, None, LINETYPE_PAR_AND));
    model.messages.push(empty_message(LINETYPE_PAR_END));
    cases.push(("par", model, Some("and")));

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model
        .messages
        .push(message(None, None, LINETYPE_CRITICAL_START));
    model
        .messages
        .push(message(None, None, LINETYPE_CRITICAL_OPTION));
    model.messages.push(empty_message(LINETYPE_CRITICAL_END));
    cases.push(("critical", model, Some("option")));

    for (keyword, model, separator) in cases {
        let rendered = render_sequence_model(&model, &AsciiRenderOptions::unicode())
            .unwrap_or_else(|err| panic!("empty {keyword} should render: {err}"));
        assert!(
            rendered.contains(&format!("{keyword} Hi")),
            "empty {keyword} should retain its frame title:\n{rendered}"
        );
        if let Some(separator) = separator {
            assert!(
                rendered.contains(&format!("{separator} Hi")),
                "empty {keyword} should retain its empty section separator:\n{rendered}"
            );
        }
        let expected_sections = usize::from(separator.is_some()) + 1;
        assert!(
            rendered
                .lines()
                .filter(|line| line.starts_with('│'))
                .count()
                >= expected_sections,
            "every empty {keyword} section should retain a visible lifeline row:\n{rendered}"
        );
    }
}

#[test]
fn sequence_control_blocks_support_activation_lifecycle_rows() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nloop Work\nA->>+B: Start\nB-->>-A: Done\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("control blocks should support activation rows");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("┌ loop Work ")),
        "loop should render while activation events are present:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains('┃')),
        "active lifeline should remain visible inside the frame:\n{rendered}"
    );
}

#[test]
fn sequence_control_blocks_support_created_and_destroyed_actors() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nloop Setup\ncreate participant C\nB->>C: Hello C\nC->>B: Still here\ndestroy C\nB--xC: Bye C\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("control blocks should support create and destroy lifecycle rows");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("┌ loop Setup ")),
        "loop should render around create/destroy lifecycle rows:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains("Hello C")),
        "created actor message should remain inside the frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains("Bye C")),
        "destroying message should remain inside the frame:\n{rendered}"
    );
}

#[test]
fn sequence_control_blocks_render_inside_participant_boxes() {
    let rendered = render_sequence(
        "sequenceDiagram\nbox Group\nparticipant A\nparticipant B\nend\nloop Work\nA->>B: Hi\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("control blocks should render with boxed participants");

    assert!(
        rendered.contains("Group"),
        "participant box label should still render:\n{rendered}"
    );
    assert!(
        rendered.contains("loop Work"),
        "control frame should still render inside participant box output:\n{rendered}"
    );
}

#[test]
fn sequence_box_keeps_inner_padding_around_participants_and_frames() {
    let rendered = render_sequence(
        "sequenceDiagram\nbox Group\nparticipant A\nparticipant B\nend\nloop Work\nA->>B: Hi\nend",
        &AsciiRenderOptions::ascii(),
    )
    .expect("boxed control block should render");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("| + loop Work ")),
        "control frame should keep one column of padding inside sequence box:\n{rendered}"
    );
    assert!(
        !rendered.lines().any(|line| line.starts_with("|+")),
        "sequence box inner content should not touch the left border:\n{rendered}"
    );
    assert!(
        !rendered.lines().any(|line| line.starts_with("||")),
        "sequence box body rows should not merge with inner frame or participant borders:\n{rendered}"
    );
}

#[test]
fn sequence_box_with_lifecycle_and_mirror_keeps_boundaries() {
    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    add_sequence_participant(&mut model, "C");
    model.boxes.push(SequenceBox {
        actor_keys: vec!["A".to_string(), "B".to_string(), "C".to_string()],
        fill: "green".to_string(),
        name: Some("Group".to_string()),
        wrap: false,
    });
    model.messages.push(SequenceMessage {
        id: "m0".to_string(),
        from: None,
        to: None,
        message_type: LINETYPE_LOOP_START,
        message: SequenceMessagePayload::Text("Setup".to_string()),
        wrap: false,
        activate: false,
        placement: None,
        central_connection: 0,
    });
    model.messages.push(SequenceMessage {
        id: "m1".to_string(),
        from: Some("B".to_string()),
        to: Some("C".to_string()),
        message_type: 0,
        message: SequenceMessagePayload::Text("Hello C".to_string()),
        wrap: false,
        activate: false,
        placement: None,
        central_connection: 0,
    });
    model.messages.push(SequenceMessage {
        id: "m2".to_string(),
        from: Some("C".to_string()),
        to: Some("B".to_string()),
        message_type: 0,
        message: SequenceMessagePayload::Text("Still here".to_string()),
        wrap: false,
        activate: false,
        placement: None,
        central_connection: 0,
    });
    model.messages.push(SequenceMessage {
        id: "m3".to_string(),
        from: Some("B".to_string()),
        to: Some("C".to_string()),
        message_type: 6,
        message: SequenceMessagePayload::Text("Bye C".to_string()),
        wrap: false,
        activate: false,
        placement: None,
        central_connection: 0,
    });
    model.messages.push(empty_message(LINETYPE_LOOP_END));
    model.created_actors.insert("C".to_string(), 1);
    model.destroyed_actors.insert("C".to_string(), 3);

    let rendered = render_sequence_model(
        &model,
        &AsciiRenderOptions::ascii().with_sequence_mirror_actors(true),
    )
    .expect("boxed lifecycle control block with mirrored actors should render");

    for expected in ["Group", "loop Setup", "Hello C", "Still here", "Bye C"] {
        assert!(
            rendered.contains(expected),
            "boxed lifecycle sequence should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        rendered.matches("| C |").count() == 1,
        "created then destroyed actor should render at lifecycle point, not in the final mirror footer:\n{rendered}"
    );
    assert!(
        rendered.contains('x'),
        "destroyed actor should render a termination marker:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("| + loop Setup ")),
        "control frame should keep padding inside the outer sequence box:\n{rendered}"
    );
    assert!(
        !rendered.lines().any(|line| line.starts_with("|+")),
        "outer sequence box and inner frame borders should not merge:\n{rendered}"
    );
    assert!(
        !rendered.lines().any(|line| line.starts_with("||")),
        "outer sequence box and participant or frame borders should not merge:\n{rendered}"
    );
}
