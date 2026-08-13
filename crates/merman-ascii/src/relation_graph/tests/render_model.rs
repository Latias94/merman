use super::*;

#[test]
fn render_stacked_boxes_preserves_plain_text() {
    let boxes = [
        RelationGraphBox::new("a".to_string(), vec!["A".to_string(), "|".to_string()], 1),
        RelationGraphBox::new("b".to_string(), vec!["B".to_string(), "|".to_string()], 1),
    ];

    assert_eq!(render_stacked_boxes(&boxes), "A\n|\n\nB\n|\n");
}

#[test]
fn render_stacked_boxes_with_section_appends_summary() {
    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(&options);
    let boxes = [
        RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
        RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
    ];
    let section_lines = vec![
        RelationGraphLine::plain("A --> B".to_string(), TerminalWidthProfile::Unicode),
        RelationGraphLine::plain("B --> A".to_string(), TerminalWidthProfile::Unicode),
    ];

    assert_eq!(
        render_stacked_boxes_with_section(
            &boxes,
            RelationGraphLine::plain("relations:".to_string(), TerminalWidthProfile::Unicode,),
            &section_lines,
            &options,
            &mut resources,
        )
        .expect("summary section should render"),
        "A\n\nB\n\nrelations:\nA --> B\nB --> A\n"
    );
}

#[test]
fn render_stacked_boxes_with_section_colors_title_and_summary_lines() {
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::Html)
        .with_color_theme(
            AsciiColorTheme::default_light()
                .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x111111))
                .with_role(AsciiColorRole::MutedText, AsciiRgb::from_hex24(0x222222))
                .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x333333)),
        );
    let mut resources = test_resources(&options);
    let boxes = vec![RelationGraphBox::new_with_lines(
        "a".to_string(),
        vec![RelationGraphLine::with_role(
            "A".to_string(),
            AsciiColorRole::Text,
            TerminalWidthProfile::Unicode,
        )],
        1,
        TerminalWidthProfile::Unicode,
    )];
    let section_lines = vec![RelationGraphLine::with_role(
        "A --> B".to_string(),
        AsciiColorRole::EdgeLabel,
        TerminalWidthProfile::Unicode,
    )];
    let rendered = render_stacked_boxes_with_section(
        &boxes,
        RelationGraphLine::with_role(
            "relations:".to_string(),
            AsciiColorRole::MutedText,
            TerminalWidthProfile::Unicode,
        ),
        &section_lines,
        &options,
        &mut resources,
    )
    .expect("colored summary section should render");

    assert_eq!(
        rendered,
        concat!(
            "<span style=\"color:#111111\">A</span>\n",
            "\n",
            "<span style=\"color:#222222\">relations:</span>\n",
            "<span style=\"color:#333333\">A --&gt; B</span>\n",
        )
    );
}

#[test]
fn relation_graph_box_from_sections_builds_shared_sectioned_boxes() {
    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(&options);
    let style = RelationGraphBoxStyle {
        top_left: '+',
        top_right: '+',
        bottom_left: '+',
        bottom_right: '+',
        horizontal: '-',
        vertical: '|',
        separator_left: '+',
        separator_right: '+',
        border_role: AsciiColorRole::NodeBorder,
        text_role: AsciiColorRole::Text,
    };
    let relation_box = RelationGraphBox::from_sections(
        "box".to_string(),
        &[vec!["A".to_string()], vec!["B".to_string()]],
        1,
        style,
        TerminalWidthProfile::Unicode,
        &mut resources,
    )
    .expect("sectioned box should render");
    let mut canvas = Canvas::new(relation_box.width(), relation_box.height());

    relation_box
        .draw_at(&mut canvas, 0, 0, &resources)
        .expect("box should fit the canvas");

    assert_eq!(relation_box.width(), 5);
    assert_eq!(relation_box.height(), 5);
    assert_eq!(
        canvas
            .finish_trimmed_with_options(&options)
            .expect("canvas should encode"),
        "+---+\n| A |\n+---+\n| B |\n+---+\n"
    );
}

#[test]
fn relation_graph_box_draws_role_lines_to_trimmed_canvas() {
    let theme =
        AsciiColorTheme::default_light().with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
    let line = RelationGraphLine::with_role(
        "AB".to_string(),
        AsciiColorRole::Text,
        TerminalWidthProfile::Unicode,
    );
    let relation_box = RelationGraphBox::new_with_lines(
        "box".to_string(),
        vec![line],
        2,
        TerminalWidthProfile::Unicode,
    );
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::TrueColor)
        .with_color_theme(theme);
    let resources = test_resources(&options);
    let mut canvas = Canvas::new(4, 1);
    relation_box
        .draw_at(&mut canvas, 0, 0, &resources)
        .expect("box should fit the canvas");

    let output = canvas
        .finish_trimmed_with_options(&options)
        .expect("canvas should encode");

    assert_eq!(output, "\u{1b}[38;2;1;2;3mAB\u{1b}[0m\n");
}

#[test]
fn relation_graph_box_content_line_preserves_border_and_text_roles() {
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::Html)
        .with_color_theme(
            AsciiColorTheme::default_light()
                .with_role(AsciiColorRole::NodeBorder, AsciiRgb::from_hex24(0x111111))
                .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x222222)),
        );
    let resources = test_resources(&options);
    let style = RelationGraphBoxStyle {
        top_left: '+',
        top_right: '+',
        bottom_left: '+',
        bottom_right: '+',
        horizontal: '-',
        vertical: '|',
        separator_left: '+',
        separator_right: '+',
        border_role: AsciiColorRole::NodeBorder,
        text_role: AsciiColorRole::Text,
    };
    let line =
        RelationGraphLine::box_content("A", 3, 1, style, TerminalWidthProfile::Unicode, &resources)
            .expect("box content should fit");
    let mut canvas = Canvas::new(5, 1);

    line.draw_at(&mut canvas, 0, 0)
        .expect("line should fit the canvas");

    assert_eq!(line.text(), "| A |");
    assert_eq!(
        canvas
            .finish_trimmed_with_options(&options)
            .expect("canvas should encode"),
        "<span style=\"color:#111111\">|</span> <span style=\"color:#222222\">A</span> <span style=\"color:#111111\">|</span>\n"
    );
}

#[test]
fn relation_line_chars_merge_crossing_relation_lines_to_junction() {
    let chars = RelationLineChars::new(['-', '|', '.', ':'], '+');
    let mut canvas = Canvas::new(1, 1);
    canvas.set_role(0, 0, '-', AsciiColorRole::EdgeLine);

    put_relation_char(&mut canvas, 0, 0, '|', chars).expect("test relation character should fit");

    assert_eq!(canvas.get(0, 0), Some('+'));
    assert_eq!(
        canvas.get_color(0, 0),
        Some(crate::canvas::CanvasColor::Role(AsciiColorRole::Junction))
    );
}

#[test]
fn parallel_relation_lane_offsets_group_by_endpoint_pair() {
    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(&options);
    let offsets = parallel_relation_lane_offsets(
        [("A", "B"), ("A", "B"), ("A", "C"), ("A", "B")],
        &mut resources,
    )
    .expect("lane offsets should fit");

    assert_eq!(offsets, vec![-6, 0, 0, 6]);
}

#[test]
fn parallel_relation_lane_offsets_group_reverse_endpoint_pairs() {
    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(&options);
    let offsets =
        parallel_relation_lane_offsets([("A", "B"), ("B", "A"), ("A", "B")], &mut resources)
            .expect("lane offsets should fit");

    assert_eq!(offsets, vec![-6, 0, 6]);
}

#[test]
fn relation_graph_label_splits_breaks_and_tracks_line_count() {
    let options = AsciiRenderOptions::ascii();
    let resources = test_resources(&options);
    let mut deferred = DeferredTextRegistry::new();
    let label = RelationGraphLabel::try_new(
        "north<br>south",
        TerminalWidthProfile::Unicode,
        &mut deferred,
        &resources,
    )
    .expect("label should fit the selected resource policy")
    .expect("label should be present");

    let mut lines = Vec::new();
    for line in label.lines() {
        let mut styled =
            crate::text::StyledLine::with_resources(TerminalWidthProfile::Unicode, &resources);
        styled
            .try_push_deferred_text(line, AsciiColorRole::EdgeLabel)
            .expect("deferred label line should fit");
        lines.push(styled);
    }
    let mut output_resources = test_resources(&options);
    let rendered = crate::canvas::finish_styled_line_iter_with_deferred_resources(
        lines.iter(),
        &options,
        true,
        &mut output_resources,
        &deferred,
    )
    .expect("deferred label should encode");
    assert_eq!(rendered, "north\nsouth\n");
    assert_eq!(label.half_width(), 2);
    assert_eq!(label.line_count(), 2);
}

#[test]
fn write_centered_relation_label_draws_each_line() {
    let options = AsciiRenderOptions::ascii();
    let resources = test_resources(&options);
    let mut deferred = DeferredTextRegistry::new();
    let label = RelationGraphLabel::try_new(
        "A<br>B",
        TerminalWidthProfile::Unicode,
        &mut deferred,
        &resources,
    )
    .expect("label should fit the selected resource policy")
    .expect("label should be present");
    let mut canvas = Canvas::new(3, 3);

    write_centered_relation_label(&mut canvas, 1, 1, &label, AsciiColorRole::EdgeLabel)
        .expect("test relation label should fit");

    let lines = canvas
        .into_styled_lines_preserving_extent()
        .expect("deferred canvas should convert to styled lines");
    let mut output_resources = test_resources(&options);
    let rendered = crate::canvas::finish_styled_line_iter_with_deferred_resources(
        lines.iter(),
        &options,
        false,
        &mut output_resources,
        &deferred,
    )
    .expect("deferred canvas should encode");
    assert_eq!(rendered, "   \n A \n B \n");
    assert_eq!(
        lines[1].surface_cells()[1].raw_style().foreground,
        Some(crate::canvas::CanvasColor::Role(AsciiColorRole::EdgeLabel))
    );
}
