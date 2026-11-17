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
        assert_eq!(
            unicode_rendered, "╭─────╮\n│     │\n│  S  │\n│     │\n╰─────╯\n",
            "compact stadium shape should keep its pill caps and label"
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
fn flowchart_typed_model_discloses_icon_and_image_metadata() {
    let mut icon = single_node_flowchart_model("rect", "Icon");
    icon.nodes[0].icon = Some("fa:circle".to_string());
    icon.nodes[0].form = Some("circle".to_string());
    icon.nodes[0].pos = Some("b".to_string());
    let mut image = single_node_flowchart_model("rect", "Image");
    image.nodes[0].img = Some("asset.png".to_string());
    image.nodes[0].constraint = Some("on".to_string());
    image.nodes[0].asset_width = Some(128.0);
    image.nodes[0].asset_height = Some(64.0);

    for (model, expected) in [
        (
            icon,
            vec![
                "Icon",
                "icon: \"fa:circle\"",
                "form: \"circle\"",
                "label position: \"b\"",
            ],
        ),
        (
            image,
            vec![
                "Image",
                "image: \"asset.png\"",
                "constraint: \"on\"",
                "asset width: 128",
                "asset height: 64",
            ],
        ),
    ] {
        let rendered = render_model(
            &RenderSemanticModel::Flowchart(model),
            &AsciiRenderOptions::ascii(),
        )
        .expect("asset metadata should have a complete textual terminal projection");
        for expected in expected {
            assert!(
                rendered.contains(expected),
                "missing {expected:?}:\n{rendered}"
            );
        }
        assert_rectangular_terminal_grid(&rendered);
    }
}

#[test]
fn flowchart_parser_asset_projection_preserves_authored_identity_and_relations() {
    let rendered = render_flowchart(
        r#"flowchart LR
        A@{ icon: "fa:user", form: "square", label: "Account" }
        A -->|upload| B@{ img: "photo.png", label: "Portrait", w: 80, h: 60, constraint: "on" }
        B --> A"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("parsed assets should remain connected using their actual node IDs");
    for expected in [
        "Account",
        "fa:user",
        "Portrait",
        "photo.png",
        "upload",
        "asset width: 80",
        "asset height: 60",
    ] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?}:\n{rendered}"
        );
    }
    assert!(!rendered.contains("A@"));
    assert_rectangular_terminal_grid(&rendered);
}

#[test]
fn flowchart_asset_details_keep_controls_literal_and_wide_cells_aligned() {
    let mut model = single_node_flowchart_model("iconSquare", "图标");
    model.nodes[0].icon = Some("mdi:用户·\\n\u{1b}[31m".to_string());
    let model = RenderSemanticModel::Flowchart(model);
    for profile in [
        merman_ascii::TerminalWidthProfile::Unicode,
        merman_ascii::TerminalWidthProfile::Cjk,
    ] {
        let mut options = AsciiRenderOptions::unicode().with_terminal_width_profile(profile);
        options.flowchart_node_label_wrap_width = 80;
        let rendered = render_model(&model, &options)
            .expect("asset details must use terminal-safe literal text");
        assert!(rendered.contains("图标"));
        assert!(rendered.contains("mdi:用户·\\\\n"));
        assert!(rendered.contains("\\u{1B}"), "{rendered}");
        assert!(!rendered.contains('\u{1b}'));
        support::assert_rectangular_terminal_grid_with_profile(&rendered, profile);
    }
}

#[test]
fn flowchart_asset_details_preserve_spaces_in_image_paths() {
    let rendered = render_flowchart(
        "flowchart LR\nA@{ img: \"a  b.png\", label: \"Portrait\" }",
        &AsciiRenderOptions::unicode().with_flowchart_node_label_wrap_width(80),
    )
    .expect("quoted asset paths must retain authored spaces");

    assert!(rendered.contains("image: \"a  b.png\""), "{rendered}");
    assert_rectangular_terminal_grid(&rendered);
}

#[test]
fn flowchart_wrapped_asset_compartments_survive_mirrored_layouts_and_width_profiles() {
    use merman_ascii::TerminalWidthProfile;

    for direction in ["BT", "RL"] {
        let mut model = single_node_flowchart_model("imageSquare", "Alpha Beta 超·");
        model.direction = Some(direction.to_string());
        model.nodes[0].icon = Some("图·".to_string());
        model.nodes[0].img = Some("a  b".to_string());
        let model = RenderSemanticModel::Flowchart(model);
        for layout in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
            for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                let options = AsciiRenderOptions::unicode()
                    .with_layout_profile(layout)
                    .with_terminal_width_profile(profile)
                    .with_flowchart_node_label_wrap_width(8);
                let rendered = render_model(&model, &options)
                    .unwrap_or_else(|error| panic!("{direction}/{layout:?}/{profile:?}: {error}"));
                let rows: Vec<_> = rendered.lines().collect();
                let alpha = first_line_index_containing(&rendered, "Alpha");
                let beta = first_line_index_containing(&rendered, "Beta");
                let last_title = first_line_index_containing(&rendered, "超·");
                let details = first_line_index_containing(&rendered, "shape:");
                let title_rows = if profile == TerminalWidthProfile::Cjk {
                    3
                } else {
                    2
                };
                assert_eq!(
                    beta,
                    alpha + 2,
                    "{direction}/{layout:?}/{profile:?}:\n{rendered}"
                );
                assert_eq!(
                    last_title,
                    alpha + 2 * (title_rows - 1),
                    "title wrapping must follow the selected width profile:\n{rendered}"
                );
                assert_eq!(
                    details,
                    last_title + 2,
                    "details must follow the complete title and its divider:\n{rendered}"
                );
                let (left, right, horizontal) = if profile == TerminalWidthProfile::Cjk {
                    ('+', '+', '-')
                } else {
                    ('├', '┤', '─')
                };
                let divider = rows[last_title + 1]
                    .trim()
                    .strip_prefix(left)
                    .and_then(|row| row.strip_suffix(right))
                    .unwrap_or_else(|| panic!("missing title/detail divider:\n{rendered}"));
                assert!(
                    !divider.is_empty() && divider.chars().all(|ch| ch == horizontal),
                    "the divider must span the compartment:\n{rendered}"
                );
                for field in ["\"图·\"", "\"a  b\""] {
                    assert!(
                        first_line_index_containing(&rendered, field) > details,
                        "quoted asset values must remain in the details compartment:\n{rendered}"
                    );
                }
                support::assert_rectangular_terminal_grid_with_profile(&rendered, profile);
            }
        }
    }
}

#[test]
fn flowchart_fixed_control_shapes_reject_asset_metadata() {
    let diagnostic = AsciiError::UnsupportedFeature {
        diagram_type: "flowchart",
        feature: "metadata on fixed control node shapes",
    };
    assert_eq!(
        render_flowchart(
            "flowchart LR\nA@{ shape: sm-circ, w: 80, pos: b }",
            &AsciiRenderOptions::ascii(),
        )
        .expect_err("fixed control shapes must diagnose metadata they cannot display"),
        diagnostic
    );
    for (shape, direction) in [
        ("sm-circ", "LR"),
        ("fr-circ", "LR"),
        ("choice", "TD"),
        ("fork", "LR"),
        ("join", "TD"),
    ] {
        let mut model = single_node_flowchart_model(shape, "Hidden");
        model.direction = Some(direction.to_string());
        model.nodes[0].icon = Some("fa:user".to_string());
        assert_eq!(
            render_model(
                &RenderSemanticModel::Flowchart(model),
                &AsciiRenderOptions::ascii(),
            )
            .expect_err("fixed control shapes must not silently drop icon metadata"),
            diagnostic,
            "{shape}/{direction}"
        );
    }
}

#[test]
fn flowchart_parser_diamond_shape_renders_as_decision_terminal_shape() {
    let rendered =
        render_flowchart("flowchart LR\nA{A} --> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "/---\\          \n/   \\     +---+\n< A >---->| B |\n\\   /     +---+\n\\---/          \n"
    );
}

#[test]
fn flowchart_parser_unicode_diamond_uses_sloped_shoulders() {
    // Sloped shoulders need an interior row above and below the decision vertices.
    let rendered = render_flowchart(
        "flowchart LR\nA{A} --> B",
        &AsciiRenderOptions::unicode().with_node_padding(1),
    )
    .unwrap();

    assert_eq!(
        rendered,
        "╭───╮     ┌───┐\n╱   ╲     │   │\n< A >────►│ B │\n╲   ╱     │   │\n╰───╯     └───┘\n"
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
            "| |   | |     |      |\n",
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
        "+---+     +---+\n| A |....>| B |\n+---+     +---+\n"
    );
}

#[test]
fn flowchart_parser_thick_edges_render_with_heavy_ascii_line() {
    let rendered = render_flowchart("flowchart LR\nA ==> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "+---+     +---+\n| A |====>| B |\n+---+     +---+\n"
    );
}

#[test]
fn flowchart_parser_thick_edges_render_with_heavy_unicode_line() {
    let rendered =
        render_flowchart("flowchart LR\nA ==> B", &AsciiRenderOptions::unicode()).unwrap();

    assert_eq!(
        rendered,
        "┌───┐     ┌───┐\n│ A ┝━━━━►│ B │\n└───┘     └───┘\n"
    );
}

#[test]
fn flowchart_parser_thick_top_down_edges_render_with_heavy_ascii_line() {
    let rendered = render_flowchart("flowchart TB\nA ==> B", &AsciiRenderOptions::ascii()).unwrap();

    let route = rendered
        .strip_prefix("+---+\n| A |\n+---+\n")
        .and_then(|rest| rest.strip_suffix("+---+\n| B |\n+---+\n"))
        .unwrap_or_else(|| {
            panic!("heavy edge must connect the closed source and target:\n{rendered}")
        });
    let stroke = route.strip_suffix("  v  \n").unwrap_or_else(|| {
        panic!("heavy edge must enter the target at its top center:\n{rendered}")
    });
    assert!(
        !stroke.is_empty() && stroke.lines().all(|row| row == "  #  "),
        "heavy edge must keep a continuous, distinguishable stroke before its arrowhead:\n{rendered}"
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
fn flowchart_parser_uncommon_shape_aliases_have_explicit_terminal_roles() {
    let mut options = AsciiRenderOptions::ascii();
    options.flowchart_node_label_wrap_width = 80;
    for (aliases, role) in [
        (
            &["notch-rect", "card", "notched-rectangle"][..],
            "shape: card",
        ),
        (&["hourglass", "collate"][..], "shape: hourglass"),
        (
            &["bolt", "com-link", "lightning-bolt"][..],
            "shape: communication link",
        ),
        (&["brace", "brace-l", "comment"][..], "shape: left comment"),
        (&["brace-r"][..], "shape: right comment"),
        (&["braces"][..], "shape: paired comment"),
        (&["delay", "half-rounded-rectangle"][..], "shape: delay"),
        (
            &["lin-cyl", "disk", "lined-cylinder"][..],
            "shape: disk storage",
        ),
        (
            &["curv-trap", "curved-trapezoid", "display"][..],
            "shape: display",
        ),
        (
            &[
                "div-rect",
                "div-proc",
                "divided-rectangle",
                "divided-process",
            ][..],
            "shape: divided process",
        ),
        (&["tri", "triangle", "extract"][..], "shape: extract"),
        (
            &["notch-pent", "notched-pentagon", "loop-limit"][..],
            "shape: loop limit",
        ),
        (
            &["flip-tri", "flipped-triangle", "manual-file"][..],
            "shape: manual file",
        ),
        (
            &["cross-circ", "crossed-circle", "summary"][..],
            "shape: summary",
        ),
        (
            &["win-pane", "window-pane", "internal-storage"][..],
            "shape: internal storage",
        ),
        (
            &["h-cyl", "horizontal-cylinder", "das"][..],
            "shape: direct access storage",
        ),
        (&["cloud"][..], "shape: cloud"),
        (&["icon"][..], "shape: icon"),
    ] {
        let mut previous = None;
        for alias in aliases {
            let input = format!("flowchart LR\nA@{{ shape: {alias}, label: \"Payload\" }}");
            let rendered = render_flowchart(&input, &options).expect("known role should render");
            assert!(rendered.contains("Payload"), "{alias}:\n{rendered}");
            assert!(rendered.contains(role), "{alias}:\n{rendered}");
            assert_rectangular_terminal_grid(&rendered);
            if let Some(previous) = &previous {
                assert_eq!(&rendered, previous, "aliases must share a terminal role");
            }
            previous = Some(rendered);
        }
    }
}

#[test]
fn flowchart_annotated_shape_ports_and_labels_are_planned_together() {
    let rendered = render_flowchart(
        "flowchart LR\nA[Request] -->|wait| B@{ shape: delay, label: \"Queued\" } --> C[Result]\nB -->|retry| A",
        &AsciiRenderOptions::unicode(),
    ).expect("role-bearing boxes must participate in ordinary route planning");
    for expected in [
        "Request",
        "wait",
        "Queued",
        "shape: delay",
        "Result",
        "retry",
    ] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?}:\n{rendered}"
        );
    }
    assert!(
        rendered.contains('├'),
        "type details should have a measured compartment:\n{rendered}"
    );
    assert_rectangular_terminal_grid(&rendered);
}

#[test]
fn flowchart_typed_model_unknown_and_cross_family_shapes_remain_diagnostic() {
    for (shape, feature) in [
        ("future-shape", "unknown flowchart node shapes"),
        ("classBox", "unsupported flowchart node shape projections"),
        (
            "bpmn-activity",
            "unsupported flowchart node shape projections",
        ),
    ] {
        assert_eq!(
            render_model(
                &RenderSemanticModel::Flowchart(single_node_flowchart_model(shape, "Payload")),
                &AsciiRenderOptions::ascii(),
            )
            .unwrap_err(),
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature
            }
        );
    }
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
