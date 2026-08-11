use super::*;

#[test]
fn sequence_color_truecolor_emits_participant_lifeline_activation_and_message_roles() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 1, 1))
        .with_role(AsciiColorRole::SequenceFrame, AsciiRgb::new(2, 2, 2))
        .with_role(AsciiColorRole::SequenceLifeline, AsciiRgb::new(3, 3, 3))
        .with_role(AsciiColorRole::SequenceActivation, AsciiRgb::new(4, 4, 4))
        .with_role(AsciiColorRole::EdgeLine, AsciiRgb::new(5, 5, 5))
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::new(6, 6, 6))
        .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::new(7, 7, 7))
        .with_role(AsciiColorRole::Junction, AsciiRgb::new(8, 8, 8));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::TrueColor)
        .with_color_theme(theme);

    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>+B: Start\nB-->>-A: Done",
        &options,
    )
    .expect("sequence should render with color roles");

    assert_eq!(
        strip_ansi(&rendered),
        concat!(
            "+---+     +---+\n",
            "| A |     | B |\n",
            "+-+-+     +-+-+\n",
            "  |         |\n",
            "  | Start   |\n",
            "  +-------->|\n",
            "  |         #\n",
            "  | Done    #\n",
            "  |<........+\n",
            "  |         |\n",
        )
    );
    for expected_code in [
        "\u{1b}[38;2;1;1;1m",
        "\u{1b}[38;2;2;2;2m",
        "\u{1b}[38;2;3;3;3m",
        "\u{1b}[38;2;4;4;4m",
        "\u{1b}[38;2;5;5;5m",
        "\u{1b}[38;2;6;6;6m",
        "\u{1b}[38;2;7;7;7m",
        "\u{1b}[38;2;8;8;8m",
    ] {
        assert!(
            rendered.contains(expected_code),
            "missing {expected_code:?} in {rendered:?}"
        );
    }
}

#[test]
fn sequence_color_html_wraps_boxes_notes_control_frames_and_messages_without_changing_plain_text() {
    let input = "sequenceDiagram\nbox Group\nparticipant A\nparticipant B\nend\nloop Work\nA->>+B: Start\nNote over A,B: Wait\nB-->>-A: Done\nend";
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x101010))
        .with_role(
            AsciiColorRole::SequenceFrame,
            AsciiRgb::from_hex24(0x202020),
        )
        .with_role(
            AsciiColorRole::SequenceLifeline,
            AsciiRgb::from_hex24(0x303030),
        )
        .with_role(
            AsciiColorRole::SequenceActivation,
            AsciiRgb::from_hex24(0x404040),
        )
        .with_role(AsciiColorRole::EdgeLine, AsciiRgb::from_hex24(0x505050))
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::from_hex24(0x606060))
        .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x707070))
        .with_role(AsciiColorRole::Junction, AsciiRgb::from_hex24(0x808080));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::Html)
        .with_color_theme(theme);

    let rendered = render_sequence(input, &options)
        .expect("sequence with boxes, frames, notes, and messages should render");
    let plain = render_sequence(input, &AsciiRenderOptions::ascii())
        .expect("the plain sequence should render with identical geometry");

    assert_eq!(
        strip_html_spans(&rendered),
        plain,
        "HTML color spans must not change control-frame geometry"
    );
    assert_eq!(
        plain,
        concat!(
            "+- Group ----------+\n",
            "| +---+     +---+  |\n",
            "| | A |     | B |  |\n",
            "| +-+-+     +-+-+  |\n",
            "| + loop Work --+  |\n",
            "| | |         | |  |\n",
            "| | | Start   | |  |\n",
            "| | +-------->| |  |\n",
            "| | |         # |  |\n",
            "| |+-----------+|  |\n",
            "| ||   Wait    ||  |\n",
            "| |+-----------+|  |\n",
            "| | |         # |  |\n",
            "| | | Done    # |  |\n",
            "| | |<........+ |  |\n",
            "| +-------------+  |\n",
            "|   |         |    |\n",
            "+------------------+\n",
        )
    );
    for expected_fragment in [
        "<span style=\"color:#202020\">+-</span><span style=\"color:#101010\"> Group </span>",
        "<span style=\"color:#202020\">|</span> <span style=\"color:#202020\">+</span><span style=\"color:#101010\"> loop Work </span>",
        "<span style=\"color:#202020\">|+-----------+|</span>",
        "<span style=\"color:#707070\">Start</span>",
        "<span style=\"color:#505050\">--------</span><span style=\"color:#606060\">&gt;</span>",
        "<span style=\"color:#404040\">#</span>",
        "<span style=\"color:#101010\">Wait</span>",
    ] {
        assert!(
            rendered.contains(expected_fragment),
            "missing {expected_fragment:?} in {rendered:?}"
        );
    }
}

#[test]
fn sequence_box_fill_truecolor_maps_background_without_plain_text_changes() {
    let input =
        "sequenceDiagram\nbox green Group\nparticipant A\nparticipant B\nend\nA->>B: Inside";
    let plain =
        render_sequence(input, &AsciiRenderOptions::ascii()).expect("plain sequence should render");
    let rendered = render_sequence(
        input,
        &AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::TrueColor),
    )
    .expect("sequence box fill should render in truecolor mode");

    assert_eq!(strip_ansi(&rendered), plain);
    assert!(
        rendered.contains("\u{1b}[48;2;0;128;0m"),
        "missing sequence box background in {rendered:?}"
    );
    assert!(
        !rendered.contains("\u{1b}[38;2;0;128;0m"),
        "box fill should not be emitted as foreground in {rendered:?}"
    );
}

#[test]
fn sequence_box_hsl_fill_truecolor_maps_background_without_plain_text_changes() {
    let input = concat!(
        "sequenceDiagram\n",
        "box hsl(120, 100%, 25%) Group\n",
        "participant A\n",
        "participant B\n",
        "end\n",
        "A->>B: Inside",
    );
    let plain =
        render_sequence(input, &AsciiRenderOptions::ascii()).expect("plain sequence should render");
    let rendered = render_sequence(
        input,
        &AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::TrueColor),
    )
    .expect("sequence box hsl fill should render in truecolor mode");

    assert_eq!(strip_ansi(&rendered), plain);
    assert!(
        rendered.contains("\u{1b}[48;2;0;128;0m"),
        "missing sequence box hsl background in {rendered:?}"
    );
}

#[test]
fn sequence_rect_rgb_color_html_maps_background_without_plain_text_changes() {
    let input =
        "sequenceDiagram\nparticipant A\nparticipant B\nrect rgb(255,238,204)\nA->>B: Shaded\nend";
    let plain =
        render_sequence(input, &AsciiRenderOptions::ascii()).expect("plain sequence should render");
    let rendered = render_sequence(
        input,
        &AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::Html),
    )
    .expect("sequence rect fill should render in HTML color mode");

    assert_eq!(strip_html_spans(&rendered), plain);
    assert!(
        !plain.contains("rgb(255,238,204)"),
        "parseable rect colors should be treated as style, not visible labels:\n{plain}"
    );
    assert!(
        rendered.contains("background-color:#ffeecc"),
        "missing rect background in {rendered:?}"
    );
}

#[test]
fn sequence_default_keeps_mermaid_mirror_actors_disabled() {
    let rendered = render_sequence(
        concat!(
            "sequenceDiagram\n",
            "    participant U as User\n",
            "    participant B as Browser\n",
            "    participant S as Server\n",
            "    U->>B: Click Login\n",
            "    B->>S: POST /login\n",
            "    S-->>B: Return Token\n",
            "    B-->>U: Show Success\n",
        ),
        &AsciiRenderOptions::unicode(),
    )
    .unwrap();

    assert!(
        !rendered.contains("┌───┴──┐"),
        "default Mermaid-compatible sequence output should not mirror actors: {rendered}"
    );
}

#[test]
fn sequence_option_mirrors_participant_boxes_below_lifelines() {
    let options = AsciiRenderOptions::unicode().with_sequence_mirror_actors(true);

    let rendered = render_sequence(
        concat!(
            "sequenceDiagram\n",
            "    participant U as User\n",
            "    participant B as Browser\n",
            "    participant S as Server\n",
            "    U->>B: Click Login\n",
            "    B->>S: POST /login\n",
            "    S-->>B: Return Token\n",
            "    B-->>U: Show Success\n",
        ),
        &options,
    )
    .unwrap();

    assert!(
        rendered.ends_with(concat!(
            "    │             │               │\n",
            "┌───┴──┐     ┌────┴────┐     ┌────┴───┐\n",
            "│ User │     │ Browser │     │ Server │\n",
            "└──────┘     └─────────┘     └────────┘\n",
        )),
        "mirrored sequence actors should close the lifelines with bottom participant boxes: {rendered}"
    );
}

#[test]
fn sequence_box_hex_color_is_not_treated_as_drawable_background() {
    let model = parse_sequence_render_model(
        "sequenceDiagram\nbox #112233 Group\nparticipant A\nend\nA->>A: Self",
    );

    assert_eq!(model.boxes.len(), 1);
    assert_eq!(model.boxes[0].fill, "transparent");
    assert!(model.boxes[0].name.is_none());
}
