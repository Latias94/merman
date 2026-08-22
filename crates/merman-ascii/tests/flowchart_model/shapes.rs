use super::*;

#[test]
fn flowchart_parser_circle_shape_renders_as_circular_terminal_shape() {
    for shape in ["circle", "circ"] {
        let rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"C\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();

        assert!(rendered.contains("C"), "circle shape should keep its label");
        assert!(
            rendered
                .lines()
                .next()
                .is_some_and(|line| line.starts_with('o')),
            "circle shape should use circular corner markers:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_parser_ellipse_shape_renders_as_rounded_terminal_shape() {
    let rendered =
        render_flowchart("flowchart LR\nA(-Ellipse-)", &AsciiRenderOptions::ascii()).unwrap();

    assert!(
        rendered.contains("Ellipse"),
        "ellipse shape should keep its label"
    );
    assert!(
        rendered
            .lines()
            .next()
            .is_some_and(|line| line.starts_with('/') && line.ends_with('\\')),
        "ellipse shape should reuse rounded box markers:\n{rendered}"
    );
}

#[test]
fn flowchart_parser_stadium_shape_renders_as_pill_terminal_shape() {
    for shape in ["stadium", "pill", "terminal"] {
        let ascii_rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"S\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();

        assert!(
            ascii_rendered.contains("S"),
            "stadium shape should keep its label"
        );
        let ascii_first_line = ascii_rendered
            .lines()
            .next()
            .expect("stadium shape should render at least one line");
        assert!(
            ascii_first_line.starts_with('(') && ascii_first_line.ends_with(')'),
            "stadium shape should use pill-style terminal markers:\n{ascii_rendered}"
        );

        let unicode_rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"S\" }}"),
            &AsciiRenderOptions::unicode(),
        )
        .unwrap();
        let unicode_first_line = unicode_rendered
            .lines()
            .next()
            .expect("stadium shape should render at least one line");
        assert!(
            unicode_first_line.starts_with('╭') && unicode_first_line.ends_with('╮'),
            "stadium shape should use rounded corners in unicode mode:\n{unicode_rendered}"
        );
    }
}

#[test]
fn flowchart_parser_doublecircle_shape_renders_as_double_terminal_shape() {
    for shape in ["doublecircle", "dbl-circ", "double-circle"] {
        let rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"D\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();

        assert!(
            rendered.contains("D"),
            "double-circle shape should keep its label"
        );
        assert!(
            rendered
                .lines()
                .next()
                .is_some_and(|line| line.starts_with('@')),
            "double-circle shape should use bullseye corner markers:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_parser_public_state_pseudo_and_fork_join_shapes_render() {
    let cases = [
        ("fork", false),
        ("join", false),
        ("start", false),
        ("stop", false),
    ];

    for (shape, should_show_label) in cases {
        let label = if shape == "start" { "S" } else { "F" };
        let rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"{label}\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();

        if should_show_label {
            assert!(
                rendered.contains(label),
                "{shape} shape should keep its label:\n{rendered}"
            );
        } else {
            assert!(
                !rendered.contains(label),
                "{shape} shape should not keep its label:\n{rendered}"
            );
        }
    }
}

#[test]
fn flowchart_fork_join_axis_is_perpendicular_to_the_graph_direction() {
    let left_right = render_flowchart(
        "flowchart LR\nA@{ shape: fork, label: \"hidden\" }",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();
    assert_eq!(
        left_right.lines().filter(|line| line.trim() == "#").count(),
        7,
        "LR fork should be a vertical synchronization bar:\n{left_right}"
    );

    let top_down = render_flowchart(
        "flowchart TD\nA@{ shape: join, label: \"hidden\" }",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();
    assert!(
        top_down.lines().any(|line| line.contains("=======")),
        "TD join should be a horizontal synchronization bar:\n{top_down}"
    );
}

#[test]
fn flowchart_process_and_decorated_process_shapes_keep_distinct_terminal_geometry() {
    let render = |shape: &str| {
        render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"P\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap()
    };

    let rect = render("process");
    let stacked = render("st-rect");
    let lined = render("lin-rect");
    let tagged = render("tag-rect");

    for rendered in [&rect, &stacked, &lined, &tagged] {
        assert!(rendered.contains('P'));
    }
    assert_ne!(stacked, rect, "stacked process must not collapse to rect");
    assert_ne!(lined, rect, "lined process must not collapse to rect");
    assert_ne!(tagged, rect, "tagged process must not collapse to rect");
    assert_ne!(stacked, lined);
    assert_ne!(stacked, tagged);
    assert_ne!(lined, tagged);
}

#[test]
fn flowchart_typed_model_internal_shape_aliases_render() {
    let cases = [
        ("forkJoin", "F", false),
        ("stateStart", "S", false),
        ("stateEnd", "E", false),
        ("rect_left_inv_arrow", "Odd", true),
    ];

    for (shape, label, should_show_label) in cases {
        let model = RenderSemanticModel::Flowchart(single_node_flowchart_model(shape, label));
        let rendered = render_model(&model, &AsciiRenderOptions::ascii()).unwrap();

        if should_show_label {
            assert!(
                rendered.contains(label),
                "{shape} internal shape should keep its label:\n{rendered}"
            );
        } else {
            assert!(
                !rendered.contains(label),
                "{shape} internal shape should not keep its label:\n{rendered}"
            );
        }
    }
}

#[test]
fn flowchart_typed_model_rejects_icon_and_image_metadata() {
    let mut icon = single_node_flowchart_model("rect", "Icon");
    icon.nodes[0].icon = Some("fa:circle".to_string());
    let mut image = single_node_flowchart_model("rect", "Image");
    image.nodes[0].img = Some("data:image/png;base64,AA==".to_string());

    for model in [icon, image] {
        let error = render_model(
            &RenderSemanticModel::Flowchart(model),
            &AsciiRenderOptions::ascii(),
        )
        .expect_err("icon and image metadata must not silently render as a plain node");
        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "flowchart icon and image node metadata",
            }
        );
    }
}

#[test]
fn flowchart_parser_diamond_shape_renders_as_decision_terminal_shape() {
    let rendered =
        render_flowchart("flowchart LR\nA{A} --> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "/---\\     +---+\n/   \\     |   |\n< A >---->| B |\n\\   /     |   |\n\\---/     +---+\n"
    );
}

#[test]
fn flowchart_parser_subroutine_and_cylinder_shapes_render_terminal_approximations() {
    let rendered = render_flowchart(
        "flowchart LR\nA[[Sub]] --> B[(DB)]",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();

    assert_eq!(
        rendered,
        concat!(
            "+-------+     /------\\\n",
            "| |   | |     |------|\n",
            "| |Sub| |---->|  DB  |\n",
            "| |   | |     |      |\n",
            "+-------+     \\------/\n",
        )
    );
}

#[test]
fn flowchart_parser_dotted_edges_render_with_dotted_line() {
    let rendered =
        render_flowchart("flowchart LR\nA -.-> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "+---+     +---+\n|   |     |   |\n| A |....>| B |\n|   |     |   |\n+---+     +---+\n"
    );
}

#[test]
fn flowchart_parser_thick_edges_render_with_heavy_ascii_line() {
    let rendered = render_flowchart("flowchart LR\nA ==> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "+---+     +---+\n|   |     |   |\n| A |====>| B |\n|   |     |   |\n+---+     +---+\n"
    );
}

#[test]
fn flowchart_parser_thick_edges_render_with_heavy_unicode_line() {
    let rendered =
        render_flowchart("flowchart LR\nA ==> B", &AsciiRenderOptions::unicode()).unwrap();

    assert_eq!(
        rendered,
        "┌───┐     ┌───┐\n│   │     │   │\n│ A ┝━━━━►│ B │\n│   │     │   │\n└───┘     └───┘\n"
    );
}

#[test]
fn flowchart_parser_thick_top_down_edges_render_with_heavy_ascii_line() {
    let rendered = render_flowchart("flowchart TB\nA ==> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        concat!(
            "+---+\n", "|   |\n", "| A |\n", "|   |\n", "+---+\n", "  #  \n", "  #  \n", "  #  \n",
            "  #  \n", "  v  \n", "+---+\n", "|   |\n", "| B |\n", "|   |\n", "+---+\n",
        )
    );
}

#[test]
fn flowchart_parser_lean_right_shape_renders() {
    for shape in ["lean-r", "lean-right", "in-out"] {
        let rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"Lean\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();

        assert!(
            rendered.contains("Lean"),
            "lean shape should keep its label"
        );
        assert!(
            rendered.lines().any(|line| line.starts_with('/')),
            "lean shape should be slanted:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_parser_lean_left_shape_renders() {
    for shape in ["lean-l", "lean-left", "out-in"] {
        let rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"Lean\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();

        assert!(
            rendered.contains("Lean"),
            "lean shape should keep its label"
        );
        assert!(
            rendered.contains('\\') && rendered.contains('/'),
            "lean shape should be slanted:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_parser_hexagon_shape_renders() {
    for shape in ["hexagon", "hex", "prepare"] {
        let rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"Hex\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();

        assert!(
            rendered.contains("Hex"),
            "hexagon shape should keep its label"
        );
        assert!(
            rendered
                .lines()
                .next()
                .is_some_and(|line| line.starts_with('*')),
            "hexagon shape should use decorated corners:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_parser_asymmetric_and_paper_tape_shape_families_follow_upstream_aliases() {
    let odd = render_flowchart(
        "flowchart LR\nA@{ shape: odd, label: \"Odd\" }",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();
    assert!(odd.contains("Odd"));
    assert!(
        odd.lines().next().is_some_and(|line| line.starts_with('>')),
        "odd shape should keep its left-pointing corners:\n{odd}"
    );

    let paper_tape_aliases = ["flag", "paper-tape"].map(|shape| {
        render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"Tape\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap()
    });
    let flag = &paper_tape_aliases[0];
    let paper_tape = &paper_tape_aliases[1];
    assert_eq!(flag, paper_tape);
    assert!(paper_tape.contains("Tape"));
    assert!(
        paper_tape
            .lines()
            .next()
            .is_some_and(|line| line.contains('~'))
            && paper_tape
                .lines()
                .last()
                .is_some_and(|line| line.contains('~')),
        "paper-tape aliases should keep wavy top and bottom edges:\n{paper_tape}"
    );
}

#[test]
fn flowchart_parser_manual_input_and_stored_data_aliases_keep_distinct_geometry() {
    let manual_inputs = ["sl-rect", "manual-input", "sloped-rectangle"].map(|shape| {
        render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"Input\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap()
    });
    assert!(manual_inputs.windows(2).all(|pair| pair[0] == pair[1]));
    assert!(manual_inputs[0].contains("Input"));
    assert!(
        manual_inputs[0].lines().any(|line| line.starts_with('/')),
        "manual-input aliases should retain the sloped input edge:\n{}",
        manual_inputs[0]
    );

    let stored_data = ["bow-rect", "stored-data", "bow-tie-rectangle"].map(|shape| {
        render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"Data\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap()
    });
    assert!(stored_data.windows(2).all(|pair| pair[0] == pair[1]));
    assert!(stored_data[0].contains("Data"));
    assert!(
        stored_data[0].contains(')') && stored_data[0].contains('('),
        "stored-data aliases should retain concave bow-tie sides:\n{}",
        stored_data[0]
    );
}

#[test]
fn flowchart_parser_filled_circle_and_text_shapes_preserve_upstream_label_policy() {
    for shape in ["f-circ", "junction", "filled-circle"] {
        let rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"Hidden\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();
        assert!(!rendered.contains("Hidden"), "shape: {shape}\n{rendered}");
    }

    let label_rect = render_flowchart(
        "flowchart LR\nA@{ shape: text, label: \"Label only\" }",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();
    assert_eq!(label_rect.trim(), "Label only");
}

#[test]
fn flowchart_parser_trapezoid_shape_renders() {
    for shape in ["trapezoid", "trap-b", "priority", "trapezoid-bottom"] {
        let rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"Trap\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();

        assert!(
            rendered.contains("Trap"),
            "trapezoid shape should keep its label"
        );
        assert!(
            rendered
                .lines()
                .next()
                .is_some_and(|line| line.starts_with('/')),
            "trapezoid shape should use slanted top corners:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_parser_inverse_trapezoid_shape_renders() {
    for shape in ["inv-trapezoid", "trap-t", "manual", "trapezoid-top"] {
        let rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"Inv\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();

        assert!(
            rendered.contains("Inv"),
            "inverse trapezoid shape should keep its label"
        );
        assert!(
            rendered
                .lines()
                .last()
                .is_some_and(|line| line.starts_with('\\')),
            "inverse trapezoid shape should use slanted bottom corners:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_parser_datastore_shape_renders() {
    for shape in ["datastore", "data-store"] {
        let rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"Store\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();

        assert!(
            rendered.contains("Store"),
            "datastore shape should keep its label"
        );
        assert!(
            rendered.lines().any(|line| line.contains("Store")),
            "datastore shape should render as a box:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_parser_document_shape_renders() {
    for shape in ["doc", "document"] {
        let rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"Doc\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();

        assert!(
            rendered.contains("Doc"),
            "document shape should keep its label"
        );
        assert!(
            rendered.contains('~'),
            "document shape should use a folded bottom edge:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_parser_document_variants_render_like_document_shape() {
    for shape in [
        "docs",
        "documents",
        "st-doc",
        "stacked-document",
        "lin-doc",
        "lined-document",
        "tag-doc",
        "tagged-document",
    ] {
        let rendered = render_flowchart(
            &format!("flowchart LR\nA@{{ shape: {shape}, label: \"Doc\" }}"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap();

        assert!(
            rendered.contains("Doc"),
            "{shape} shape should keep its label"
        );
        assert!(
            rendered.contains('~'),
            "{shape} shape should reuse the folded bottom edge:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_parser_shape_data_pipeline_example_renders_readable_ascii_shapes() {
    let rendered = render_flowchart(
        r#"flowchart LR
    Source@{ shape: lean-r, label: "Event stream" } --> Load[Normalize]
    Load --> Store@{ shape: datastore, label: "Warehouse" }
    Store --> Report@{ shape: doc, label: "Daily report" }"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("shape-data pipeline example should render");

    for expected in ["Event stream", "Normalize", "Warehouse", "Daily report"] {
        assert!(
            rendered.contains(expected),
            "shape-data pipeline should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        rendered.lines().any(|line| line.starts_with(" /")),
        "lean-r shape should keep its left slant continuous below the top edge:\n{rendered}"
    );
    assert!(
        rendered.contains("~~~~~~~~~~~~~"),
        "document shape should keep the folded bottom edge:\n{rendered}"
    );
}

#[test]
fn flowchart_parser_rejects_remaining_uncommon_shapes() {
    let shape = "icon";
    let input = format!("flowchart LR\nA@{{ shape: {shape}, label: \"X\" }}");
    let err = render_flowchart(&input, &AsciiRenderOptions::ascii())
        .expect_err("unsupported shape should be rejected");
    assert!(matches!(
        err,
        merman_ascii::AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "unsupported flowchart node shape projections",
        }
    ));
}

#[test]
fn flowchart_parser_rejects_internal_icon_layout_shape_names() {
    for shape in ["iconSquare", "iconCircle", "iconRounded", "imageSquare"] {
        let input = format!("flowchart LR\nA@{{ shape: {shape}, label: \"X\" }}");
        let err = parse_flowchart_error(&input);
        assert!(
            err.contains(&format!(
                "No such shape: {shape}. Shape names should be lowercase."
            )),
            "internal layout shape should be rejected before ASCII rendering:\n{err}"
        );
    }
}
