use super::*;

#[test]
fn sequence_rect_par_over_blocks_are_core_control_signals() {
    struct Case {
        name: &'static str,
        input: &'static str,
        signals: &'static [(i32, &'static str)],
    }

    let cases = [
        Case {
            name: "rect",
            input: "sequenceDiagram\nparticipant A\nparticipant B\nrect rgba(0,0,0,0.1)\nA->>B: Shaded\nend",
            signals: &[
                (LINETYPE_RECT_START, "rgba(0,0,0,0.1)"),
                (LINETYPE_RECT_END, ""),
            ],
        },
        Case {
            name: "par_over",
            input: "sequenceDiagram\nparticipant A\nparticipant B\npar_over Everyone\nA->>B: Work\nend",
            signals: &[
                (LINETYPE_PAR_OVER_START, "Everyone"),
                (LINETYPE_PAR_END, ""),
            ],
        },
    ];

    for case in cases {
        let model = parse_sequence_render_model(case.input);
        let control_messages = model
            .messages
            .iter()
            .filter(|message| message.from.is_none() && message.to.is_none())
            .collect::<Vec<_>>();

        assert_eq!(
            control_messages.len(),
            case.signals.len(),
            "{} should have expected control marker count",
            case.name
        );

        let actual = control_messages
            .iter()
            .map(|message| (message.message_type, message.message_text()))
            .collect::<Vec<_>>();
        assert_eq!(
            actual, case.signals,
            "{} should preserve core control line types and labels",
            case.name
        );
        assert!(
            model
                .messages
                .iter()
                .any(|message| message.from.is_some() && message.to.is_some()),
            "{} should still include drawable messages inside the block",
            case.name
        );
    }
}

#[test]
fn sequence_rect_control_blocks_render_unicode_frames() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nrect rgba(0,0,0,0.1)\nA->>B: Shaded\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("rect should render with Unicode charset");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("┌ rect rgba(0,0,0,0.1) ")),
        "rect should render a labeled Unicode top frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains("Shaded")),
        "rect should keep contained rows inside the Unicode frame:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|line| line.starts_with('└')),
        "rect should render a Unicode bottom frame:\n{rendered}"
    );
}

#[test]
fn sequence_rect_control_blocks_render_ascii_frames() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nrect rgba(0,0,0,0.1)\nA->>B: Shaded\nend",
        &AsciiRenderOptions::ascii(),
    )
    .expect("rect should render with ASCII charset");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("+ rect rgba(0,0,0,0.1) ")),
        "rect should render a labeled ASCII top frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('|') && line.contains("Shaded")),
        "rect should keep contained rows inside the ASCII frame:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|line| line.starts_with('+')),
        "rect should render an ASCII bottom frame:\n{rendered}"
    );
}

#[test]
fn sequence_par_over_control_blocks_render_unicode_frames() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\npar_over Everyone\nA->>B: Work\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("par_over should render with Unicode charset");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("┌ par_over Everyone ")),
        "par_over should render a labeled Unicode top frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains("Work")),
        "par_over should keep contained rows inside the Unicode frame:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|line| line.starts_with('└')),
        "par_over should render a Unicode bottom frame:\n{rendered}"
    );
}

#[test]
fn sequence_par_over_control_blocks_render_ascii_frames() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\npar_over Everyone\nA->>B: Work\nend",
        &AsciiRenderOptions::ascii(),
    )
    .expect("par_over should render with ASCII charset");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("+ par_over Everyone ")),
        "par_over should render a labeled ASCII top frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('|') && line.contains("Work")),
        "par_over should keep contained rows inside the ASCII frame:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|line| line.starts_with('+')),
        "par_over should render an ASCII bottom frame:\n{rendered}"
    );
}

#[test]
fn sequence_rect_par_over_control_blocks_support_notes_activations_and_boxes() {
    let cases = [
        (
            "rect rgba(0,0,0,0.1)",
            "rect rgba(0,0,0,0.1)",
            "sequenceDiagram\nbox Group\nparticipant A\nparticipant B\nend\nrect rgba(0,0,0,0.1)\nNote over A,B: Wait\nA->>+B: Start\nB-->>-A: Done\nend",
        ),
        (
            "par_over Everyone",
            "par_over Everyone",
            "sequenceDiagram\nbox Group\nparticipant A\nparticipant B\nend\npar_over Everyone\nNote over A,B: Wait\nA->>+B: Start\nB-->>-A: Done\nend",
        ),
    ];

    for (label, frame_marker, input) in cases {
        let rendered = render_sequence(input, &AsciiRenderOptions::unicode())
            .unwrap_or_else(|err| panic!("{label} should render: {err}"));

        assert!(
            rendered.contains("Group"),
            "{label} should preserve participant box labels:\n{rendered}"
        );
        assert!(
            rendered.contains(frame_marker),
            "{label} should render the control frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with('│') && line.contains("Wait")),
            "{label} should keep notes inside the frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with('│') && line.contains('┃')),
            "{label} should keep active lifelines inside the frame:\n{rendered}"
        );
    }
}

#[test]
fn sequence_rect_par_over_control_blocks_support_created_and_destroyed_actors() {
    let cases = [
        (
            "rect rgba(0,0,0,0.1)",
            "sequenceDiagram\nparticipant A\nparticipant B\nrect rgba(0,0,0,0.1)\ncreate participant C\nB->>C: Hello C\nC->>B: Still here\ndestroy C\nB--xC: Bye C\nend",
        ),
        (
            "par_over Everyone",
            "sequenceDiagram\nparticipant A\nparticipant B\npar_over Everyone\ncreate participant C\nB->>C: Hello C\nC->>B: Still here\ndestroy C\nB--xC: Bye C\nend",
        ),
    ];

    for (label, input) in cases {
        let rendered = render_sequence(input, &AsciiRenderOptions::unicode())
            .unwrap_or_else(|err| panic!("{label} should render: {err}"));

        assert!(
            rendered.contains(label),
            "{label} should render the control frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with('│') && line.contains("Hello C")),
            "{label} should keep created actor messages inside the frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with('│') && line.contains("Bye C")),
            "{label} should keep destroying messages inside the frame:\n{rendered}"
        );
    }
}

#[test]
fn sequence_rect_par_over_nested_control_blocks_render() {
    let cases = [
        (
            "rect contains par_over",
            "rect rgba(0,0,0,0.1)",
            "par_over Everyone",
            "sequenceDiagram\nparticipant A\nparticipant B\nrect rgba(0,0,0,0.1)\npar_over Everyone\nA->>B: Work\nend\nend",
        ),
        (
            "par_over contains rect",
            "par_over Everyone",
            "rect rgba(0,0,0,0.1)",
            "sequenceDiagram\nparticipant A\nparticipant B\npar_over Everyone\nrect rgba(0,0,0,0.1)\nA->>B: Work\nend\nend",
        ),
    ];

    for (name, outer, inner, input) in cases {
        let rendered = render_sequence(input, &AsciiRenderOptions::unicode())
            .unwrap_or_else(|err| panic!("{name} should render: {err}"));

        assert!(
            rendered.contains(outer),
            "{name} should render the outer frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with("│ ┌") && line.contains(inner)),
            "{name} should render the inner frame inside the outer frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with("│ │") && line.contains("Work")),
            "{name} should keep messages inside both nested frames:\n{rendered}"
        );
    }
}

#[test]
fn sequence_rect_par_over_empty_sections_render_visible_lifeline_rows() {
    let mut cases = Vec::new();

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model
        .messages
        .push(message(None, None, LINETYPE_RECT_START));
    model.messages.push(empty_message(LINETYPE_RECT_END));
    cases.push(("rect", model));

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model
        .messages
        .push(message(None, None, LINETYPE_PAR_OVER_START));
    model.messages.push(empty_message(LINETYPE_PAR_END));
    cases.push(("par_over", model));

    for (keyword, model) in cases {
        let rendered = render_sequence_model(&model, &AsciiRenderOptions::unicode())
            .unwrap_or_else(|err| panic!("empty {keyword} should render: {err}"));
        assert!(
            rendered.contains(&format!("{keyword} Hi")),
            "empty {keyword} should retain its frame title:\n{rendered}"
        );
        assert!(
            rendered.lines().any(|line| line.starts_with('│')),
            "empty {keyword} should retain a visible lifeline row:\n{rendered}"
        );
    }
}

#[test]
fn sequence_rect_par_over_malformed_ordering_is_explicitly_unsupported() {
    let mut cases = Vec::new();

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model
        .messages
        .push(message(None, None, LINETYPE_RECT_START));
    model.messages.push(message(Some("A"), Some("B"), 0));
    model.messages.push(empty_message(LINETYPE_PAR_END));
    cases.push(model);

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model
        .messages
        .push(message(None, None, LINETYPE_PAR_OVER_START));
    model.messages.push(message(Some("A"), Some("B"), 0));
    model.messages.push(empty_message(LINETYPE_RECT_END));
    cases.push(model);

    for model in cases {
        assert_unsupported_sequence_model(model, "control block ordering");
    }
}
