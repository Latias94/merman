use super::*;

#[test]
fn sequence_control_frame_uses_the_local_participant_span() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nparticipant C\nA->>C: kickoff\nloop poll\nB->>C: check\nend",
        &AsciiRenderOptions::ascii(),
    )
    .expect("a partial-span control frame should render");

    let frame_top = rendered
        .lines()
        .find(|line| line.contains("loop poll"))
        .unwrap_or_else(|| panic!("loop frame should render:\n{rendered}"));
    let outer_lifeline = frame_top.find('|').unwrap_or_else(|| {
        panic!("A's lifeline should remain visible outside the frame:\n{rendered}")
    });
    let frame_left = frame_top
        .find('+')
        .unwrap_or_else(|| panic!("the local frame should have a left border:\n{rendered}"));
    assert!(
        outer_lifeline < frame_left,
        "the loop frame should start after A's unrelated lifeline:\n{rendered}"
    );

    let message_row = rendered
        .lines()
        .find(|line| line.contains("check"))
        .unwrap_or_else(|| panic!("the local message should remain visible:\n{rendered}"));
    let frame_border = message_row
        .match_indices('|')
        .nth(1)
        .map(|(index, _)| index)
        .unwrap_or_else(|| {
            panic!("the local frame body should preserve both borders:\n{rendered}")
        });
    assert!(
        message_row[..frame_border].contains('|'),
        "A's lifeline should remain outside the local frame body:\n{rendered}"
    );
}

#[test]
fn sequence_local_control_frame_includes_note_gutters_in_both_charsets() {
    let input = concat!(
        "sequenceDiagram\n",
        "participant A\n",
        "participant B\n",
        "participant C\n",
        "A->>C: kickoff\n",
        "loop poll\n",
        "Note right of C: queued\n",
        "B->>C: check\n",
        "end",
    );

    for (options, top_left, top_right, lifeline) in [
        (AsciiRenderOptions::ascii(), '+', '+', '|'),
        (AsciiRenderOptions::unicode(), '┌', '┐', '│'),
    ] {
        let rendered = render_sequence(input, &options)
            .expect("a partial-span frame should reserve its note gutter");
        let frame_top = rendered
            .lines()
            .find(|line| line.contains("loop poll"))
            .unwrap_or_else(|| panic!("the local loop should render:\n{rendered}"));
        let frame_left = frame_top
            .find(top_left)
            .unwrap_or_else(|| panic!("the local frame should have a left border:\n{rendered}"));
        let frame_right = frame_top
            .rfind(top_right)
            .unwrap_or_else(|| panic!("the local frame should have a right border:\n{rendered}"));
        let unrelated_lifeline = frame_top
            .find(lifeline)
            .unwrap_or_else(|| panic!("A's unrelated lifeline should remain visible:\n{rendered}"));
        assert!(
            unrelated_lifeline < frame_left,
            "the local frame should not claim actor A:\n{rendered}"
        );

        let note_row = rendered
            .lines()
            .find(|line| line.contains("queued"))
            .unwrap_or_else(|| panic!("the note should render inside the frame:\n{rendered}"));
        let note_start = note_row
            .find("queued")
            .unwrap_or_else(|| panic!("the note text should have a stable row:\n{rendered}"));
        let note_end = note_start + "queued".len();
        assert!(
            frame_left < note_start && note_end <= frame_right,
            "the planned frame should include the right-side note gutter:\n{rendered}"
        );
    }
}

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
fn sequence_three_level_control_renders_with_a_leftmost_note() {
    let rendered = render_sequence(
        concat!(
            "sequenceDiagram\n",
            "participant A\n",
            "participant B\n",
            "participant C\n",
            "loop Outer\n",
            "alt Choice\n",
            "opt Inner\n",
            "Note left of A: queued\n",
            "A->>B: Work\n",
            "end\n",
            "end\n",
            "end",
        ),
        &AsciiRenderOptions::unicode(),
    )
    .expect("three nested controls with a leftmost note should render");

    for title in ["loop Outer", "alt Choice", "opt Inner"] {
        assert!(
            rendered.contains(title),
            "the {title:?} frame should remain visible:\n{rendered}"
        );
    }
    assert!(
        rendered.contains("queued"),
        "the leftmost note should remain visible:\n{rendered}"
    );
    assert!(
        rendered.contains("Work"),
        "the message following the nested note should remain visible:\n{rendered}"
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

    let frame_top = rendered
        .lines()
        .find(|line| line.contains("loop Setup"))
        .unwrap_or_else(|| panic!("loop should render around lifecycle rows:\n{rendered}"));
    let unrelated_lifeline = frame_top
        .find('│')
        .unwrap_or_else(|| panic!("A's lifeline should remain visible:\n{rendered}"));
    let frame_left = frame_top
        .find('┌')
        .unwrap_or_else(|| panic!("the lifecycle frame should have a left border:\n{rendered}"));
    assert!(
        unrelated_lifeline < frame_left,
        "the B-C lifecycle frame should leave unrelated actor A outside:\n{rendered}"
    );
    assert_text_between_borders(&rendered, "Hello C", '│');
    assert_text_between_borders(&rendered, "Bye C", '│');
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
fn sequence_local_control_frame_can_extend_beyond_a_partial_participant_box() {
    let rendered = render_sequence(
        concat!(
            "sequenceDiagram\n",
            "box Pair\n",
            "participant A\n",
            "participant B\n",
            "end\n",
            "participant C\n",
            "alt choose\n",
            "A->>C: yes\n",
            "else retry\n",
            "A->>C: no\n",
            "end",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("a local frame may extend beyond a partial participant box");

    assert!(
        rendered.contains("Pair"),
        "the partial box should render:\n{rendered}"
    );
    assert!(
        rendered.contains("alt choose"),
        "the local frame should render:\n{rendered}"
    );
    assert!(
        rendered.contains("yes"),
        "the first section should render:\n{rendered}"
    );
    assert!(
        rendered.contains("no"),
        "the second section should render:\n{rendered}"
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
    let frame_top = rendered
        .lines()
        .find(|line| line.contains("loop Setup"))
        .unwrap_or_else(|| panic!("the lifecycle control frame should render:\n{rendered}"));
    let outer_border = frame_top
        .find('|')
        .unwrap_or_else(|| panic!("the outer sequence box should render:\n{rendered}"));
    let frame_left = frame_top
        .find('+')
        .unwrap_or_else(|| panic!("the inner control frame should render:\n{rendered}"));
    assert!(
        frame_left >= outer_border + 2,
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

fn assert_text_between_borders(rendered: &str, text: &str, border: char) {
    let line = rendered
        .lines()
        .find(|line| line.contains(text))
        .unwrap_or_else(|| panic!("{text:?} should render inside the frame:\n{rendered}"));
    let start = line
        .find(text)
        .unwrap_or_else(|| panic!("{text:?} should have a stable row:\n{rendered}"));
    let end = start + text.len();
    assert!(
        line[..start].contains(border) && line[end..].contains(border),
        "{text:?} should remain between both frame borders:\n{rendered}"
    );
}
