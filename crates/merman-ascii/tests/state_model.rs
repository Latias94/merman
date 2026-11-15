mod support;

use merman_ascii::{
    AsciiColorMode, AsciiError, AsciiLayoutProfile, AsciiRenderOptions, TerminalWidthProfile,
};
use merman_core::{ParseOptions, presentation::MermaidEngine};
use merman_model::families::state::{
    StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
};
use merman_model::presentation::RenderSemanticModel;
use std::path::Path;
use support::render_model;

fn render_state(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("state diagram should parse")
        .expect("state diagram should be detected");

    assert_eq!(parsed.metadata().diagram_type, "stateDiagram");
    render_model(parsed.model(), options)
}

fn read_local_semantic_fixture(path: &str) -> String {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata/local-semantic")
        .join(path);
    std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()))
}

fn first_line_index_containing(rendered: &str, needle: &str) -> usize {
    rendered
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing {needle:?} in rendered fixture:\n{rendered}"))
}

fn text_position(rendered: &str, needle: &str) -> (usize, usize) {
    rendered
        .lines()
        .enumerate()
        .find_map(|(y, line)| line.find(needle).map(|x| (x, y)))
        .unwrap_or_else(|| panic!("missing {needle:?} in rendered fixture:\n{rendered}"))
}

fn direct_state_node(
    id: &str,
    shape: &str,
    parent_id: Option<&str>,
    position: Option<&str>,
) -> StateDiagramRenderNode {
    StateDiagramRenderNode {
        id: id.to_string(),
        label_style: String::new(),
        label: None,
        description: None,
        dom_id: String::new(),
        is_group: shape == "noteGroup",
        node_type: (shape == "noteGroup").then(|| "group".to_string()),
        parent_id: parent_id.map(str::to_string),
        css_classes: String::new(),
        css_compiled_styles: Vec::new(),
        css_styles: Vec::new(),
        dir: None,
        explicit_dir: None,
        padding: None,
        rx: None,
        ry: None,
        shape: shape.to_string(),
        position: position.map(str::to_string),
        color_index: None,
        wrapping_width: None,
        min_width: None,
    }
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for escaped in chars.by_ref() {
                if escaped == 'm' {
                    break;
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
}

#[test]
fn state_simple_transition_renders_through_render_model() {
    let rendered = render_state("stateDiagram-v2\nA --> B: go", &AsciiRenderOptions::ascii())
        .expect("simple state transition should render");

    assert!(
        rendered.lines().any(|line| line.contains("| A")),
        "source state should render as a graph node:\n{rendered}"
    );
    assert!(
        rendered.contains("go"),
        "transition label should render on the graph edge:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|line| line.contains("| B")),
        "target state should render as a graph node:\n{rendered}"
    );
}

#[test]
fn state_default_vertical_spacing_keeps_short_lifecycles_readable() {
    for direction in ["TB", "BT"] {
        let source = format!(
            "stateDiagram-v2\ndirection {direction}\n[*] --> Alpha\nAlpha --> Beta\nBeta --> Gamma\nGamma --> [*]\n"
        );
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                let options = options.with_terminal_width_profile(profile);
                let rendered = render_state(&source, &options).unwrap();
                support::assert_rectangular_terminal_grid_with_profile(&rendered, profile);
                // Five three-row contours and two routing cells per transition. The planner
                // retains room for an independent head and a visible connecting stroke.
                assert!(rendered.lines().count() <= 5 * 3 + 4 * 2, "{rendered}");
                assert_eq!(
                    rendered.matches(['v', '^', '▼', '▲']).count(),
                    4,
                    "{rendered}"
                );
                let rows = ["Alpha", "Beta", "Gamma"].map(|label| {
                    assert_eq!(rendered.matches(label).count(), 1, "{rendered}");
                    first_line_index_containing(&rendered, label)
                });
                assert!(
                    if direction == "TB" {
                        rows.windows(2).all(|pair| pair[0] < pair[1])
                    } else {
                        rows.windows(2).all(|pair| pair[0] > pair[1])
                    },
                    "{rendered}"
                );

                let spacious = render_state(
                    &source,
                    &options.with_node_padding(1).with_graph_padding_y(5),
                )
                .unwrap();
                assert_eq!(spacious.lines().count(), 41, "{spacious}");
            }
        }
    }
}

#[test]
fn state_labels_do_not_use_flowchart_node_label_wrap_width() {
    let input = "stateDiagram-v2\nstate \"Alpha Beta Gamma Delta\" as A";
    let rendered = render_state(
        input,
        &AsciiRenderOptions::ascii().with_flowchart_node_label_wrap_width(8),
    )
    .expect("state label should render");

    assert!(
        rendered.contains("Alpha Beta Gamma Delta"),
        "flowchart label wrapping must not affect state nodes:\n{rendered}"
    );
}

#[test]
fn state_layout_does_not_inherit_the_flowchart_compact_profile() {
    let input = concat!(
        "stateDiagram-v2\n",
        "state Parent {\n",
        "  A --> B: advance\n",
        "}\n",
        "Parent --> Done: finish",
    );
    render_state(input, &AsciiRenderOptions::ascii())
        .expect("canonical state layout should render");
    let error = render_state(
        input,
        &AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Compact),
    )
    .expect_err("State must reject the unadmitted Flowchart compact profile");

    assert_eq!(
        error,
        AsciiError::InvalidOption {
            field: "layout_profile",
            message: "is not admitted for this diagram family",
        }
    );
}

#[test]
fn state_lr_direction_renders_states_on_one_row() {
    let rendered = render_state(
        "stateDiagram-v2\ndirection LR\nA --> B: go",
        &AsciiRenderOptions::ascii(),
    )
    .expect("LR state direction should render");

    assert!(
        rendered
            .lines()
            .any(|line| line.contains("| A |") && line.contains("| B |")),
        "LR state output should place source and target on the same row:\n{rendered}"
    );
}

#[test]
fn state_composite_without_explicit_direction_inherits_nearest_explicit_ancestor() {
    let rendered = render_state(
        concat!(
            "stateDiagram-v2\n",
            "direction LR\n",
            "state Outer {\n",
            "  state Inner {\n",
            "    A --> B\n",
            "  }\n",
            "}\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("nested state composites should inherit the root direction");

    let a = rendered
        .lines()
        .enumerate()
        .find_map(|(y, line)| line.find(" A ").map(|x| (x, y)))
        .expect("missing state A");
    let b = rendered
        .lines()
        .enumerate()
        .find_map(|(y, line)| line.find(" B ").map(|x| (x, y)))
        .expect("missing state B");
    assert_eq!(
        a.1, b.1,
        "nearest explicit LR direction should apply to the nested composite:\n{rendered}"
    );
}

#[test]
fn state_nested_composite_inherits_nearest_non_root_explicit_direction() {
    let rendered = render_state(
        concat!(
            "stateDiagram-v2\n",
            "direction TB\n",
            "state Outer {\n",
            "  direction LR\n",
            "  state Inner {\n",
            "    A --> B\n",
            "  }\n",
            "}\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("implicit nested composite should inherit the nearest explicit ancestor direction");

    let a = rendered
        .lines()
        .enumerate()
        .find_map(|(y, line)| line.find(" A ").map(|x| (x, y)))
        .expect("missing state A");
    let b = rendered
        .lines()
        .enumerate()
        .find_map(|(y, line)| line.find(" B ").map(|x| (x, y)))
        .expect("missing state B");
    assert_eq!(
        a.1, b.1,
        "Inner must inherit Outer LR instead of falling back to root TB:\n{rendered}"
    );
}

#[test]
fn state_nested_explicit_direction_overrides_its_explicit_ancestor() {
    let rendered = render_state(
        concat!(
            "stateDiagram-v2\n",
            "direction TB\n",
            "state Outer {\n",
            "  direction LR\n",
            "  state Inner {\n",
            "    direction TB\n",
            "    A --> B\n",
            "  }\n",
            "}\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("explicit nested composite direction should override its ancestor");

    let a = rendered
        .lines()
        .enumerate()
        .find_map(|(y, line)| line.find(" A ").map(|x| (x, y)))
        .expect("missing state A");
    let b = rendered
        .lines()
        .enumerate()
        .find_map(|(y, line)| line.find(" B ").map(|x| (x, y)))
        .expect("missing state B");
    assert_eq!(
        a.0, b.0,
        "Inner TB must override Outer LR instead of being flattened by it:\n{rendered}"
    );
    assert!(
        a.1 < b.1,
        "Inner TB must keep authored source-before-target order:\n{rendered}"
    );
}

#[test]
fn state_composite_explicit_reverse_direction_mirrors_child_layout() {
    let rendered = render_state(
        concat!(
            "stateDiagram-v2\n",
            "direction TB\n",
            "state Outer {\n",
            "  direction RL\n",
            "  A --> B\n",
            "}\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("explicit reverse state direction should render");

    let a = rendered
        .lines()
        .enumerate()
        .find_map(|(y, line)| line.find(" A ").map(|x| (x, y)))
        .expect("missing state A");
    let b = rendered
        .lines()
        .enumerate()
        .find_map(|(y, line)| line.find(" B ").map(|x| (x, y)))
        .expect("missing state B");
    assert_eq!(
        a.1, b.1,
        "explicit RL direction should keep child states on one row:\n{rendered}"
    );
    assert!(
        b.0 < a.0,
        "explicit RL direction should place B before A:\n{rendered}"
    );
}

#[test]
fn state_start_and_end_pseudo_states_render_as_distinct_visible_nodes() {
    let rendered = render_state(
        "stateDiagram-v2\n[*] --> A\nA --> [*]",
        &AsciiRenderOptions::ascii(),
    )
    .expect("start and end pseudo states should render");

    assert!(
        rendered.contains("| * |"),
        "start pseudo state should render as a visible star node:\n{rendered}"
    );
    assert!(
        rendered.contains("| @ |"),
        "end pseudo state should render with a distinct terminal symbol:\n{rendered}"
    );
    assert!(
        !rendered.contains("root_start") && !rendered.contains("root_end"),
        "start/end implementation ids should not leak into ASCII output:\n{rendered}"
    );
}

#[test]
fn state_alias_description_renders_human_label() {
    let rendered = render_state(
        "stateDiagram-v2\nstate \"Small State 1\" as namedState1\nnamedState1 --> Done",
        &AsciiRenderOptions::ascii(),
    )
    .expect("state aliases and descriptions should render");

    assert!(
        rendered.contains("Small State 1"),
        "state description should be used as the visible label:\n{rendered}"
    );
    assert!(
        !rendered.contains("namedState1"),
        "internal state id should not replace the human label:\n{rendered}"
    );
}

#[test]
fn state_title_and_body_render_as_distinct_compartments() {
    for (options, divider) in [
        (AsciiRenderOptions::ascii(), '-'),
        (AsciiRenderOptions::unicode(), '─'),
    ] {
        let rendered = render_state(
            "stateDiagram-v2\nstate \"Power mode\" as Power: Running",
            &options,
        )
        .expect("state title and body should render");

        let title = text_position(&rendered, "Power mode");
        let body = text_position(&rendered, "Running");
        assert!(title.1 < body.1, "title must precede body:\n{rendered}");
        assert!(
            rendered
                .lines()
                .skip(title.1 + 1)
                .take(body.1.saturating_sub(title.1 + 1))
                .any(|line| line.contains(divider)),
            "a structural divider must preserve title/body roles:\n{rendered}"
        );
    }
}

#[test]
fn direct_state_model_keeps_multiline_compartment_boundary_after_bt_mirroring() {
    let mut titled = direct_state_node("Titled", "rectWithTitle", None, None);
    titled.label = Some("Title one<br>Title two".into());
    titled.description = Some(vec!["Body one<br>Body two".to_string()]);
    let model = StateDiagramRenderModel {
        direction: "BT".to_string(),
        nodes: vec![titled],
        ..StateDiagramRenderModel::default()
    };

    let rendered = render_model(
        &RenderSemanticModel::State(model),
        &AsciiRenderOptions::unicode(),
    )
    .expect("BT state compartments should render");

    let title = text_position(&rendered, "Title two");
    let body = text_position(&rendered, "Body one");
    assert!(
        rendered
            .lines()
            .skip(title.1 + 1)
            .take(body.1.saturating_sub(title.1 + 1))
            .any(|line| line.contains('─')),
        "the mirrored divider must remain between multiline title and body roles:\n{rendered}"
    );
}

#[test]
fn state_composite_without_group_transition_renders_group_box() {
    let rendered = render_state(
        "stateDiagram-v2\nstate Parent {\n  Child\n}",
        &AsciiRenderOptions::ascii(),
    )
    .expect("composite state without group edge endpoints should render");

    assert!(
        rendered.contains("Parent"),
        "composite state title should render as a group label:\n{rendered}"
    );
    assert!(
        rendered.contains("Child"),
        "composite state child should render inside the graph output:\n{rendered}"
    );
}

#[test]
fn state_notes_render_as_note_nodes() {
    let rendered = render_state(
        "stateDiagram-v2\nA --> B\nnote right of A : note text",
        &AsciiRenderOptions::ascii(),
    )
    .expect("state notes should render as terminal note nodes");

    assert!(
        rendered.contains("note text"),
        "note text should render in the ASCII output:\n{rendered}"
    );
    assert!(
        !rendered.contains("----note") && !rendered.contains("----parent"),
        "state note implementation ids should not leak into ASCII output:\n{rendered}"
    );
}

#[test]
fn state_multiple_notes_preserve_text_and_side_ownership() {
    let rendered = render_state(
        concat!(
            "stateDiagram-v2\n",
            "A\n",
            "note left of A : left text\n",
            "note right of A : right text\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("multiple notes on one state should render");

    assert!(
        rendered.contains("left text") && rendered.contains("right text"),
        "both note payloads must survive typed projection:\n{rendered}"
    );
    let left = text_position(&rendered, "left text");
    let state = text_position(&rendered, " A ");
    let right = text_position(&rendered, "right text");
    assert_eq!(
        left.1, state.1,
        "left note and state should share a horizontal lane:\n{rendered}"
    );
    assert_eq!(
        state.1, right.1,
        "state and right note should share a horizontal lane:\n{rendered}"
    );
    assert!(
        left.0 < state.0 && state.0 < right.0,
        "note side constraints must determine physical placement:\n{rendered}"
    );
}

#[test]
fn state_note_sides_remain_physical_across_root_directions() {
    for direction in ["TB", "BT", "LR", "RL"] {
        let rendered = render_state(
            &format!(
                concat!(
                    "stateDiagram-v2\n",
                    "direction {direction}\n",
                    "A\n",
                    "note left of A : left side\n",
                    "note right of A : right side\n",
                ),
                direction = direction,
            ),
            &AsciiRenderOptions::unicode(),
        )
        .unwrap_or_else(|error| panic!("{direction} state notes should render: {error}"));

        let left = text_position(&rendered, "left side");
        let state = text_position(&rendered, " A ");
        let right = text_position(&rendered, "right side");
        assert!(
            left.0 < state.0 && state.0 < right.0,
            "{direction} must preserve physical note sides after transforms:\n{rendered}"
        );
    }
}

#[test]
fn state_local_reverse_direction_preserves_physical_note_sides() {
    let rendered = render_state(
        concat!(
            "stateDiagram-v2\n",
            "direction TB\n",
            "state \"Outer\" as Outer {\n",
            "  direction RL\n",
            "  A\n",
            "  B\n",
            "  A --> B\n",
            "  note left of A : left local\n",
            "  note right of A : right local\n",
            "}\n",
        ),
        &AsciiRenderOptions::unicode(),
    )
    .expect("local reverse direction should preserve note ownership");

    let left = text_position(&rendered, "left local");
    let state = text_position(&rendered, " A ");
    let right = text_position(&rendered, "right local");
    assert!(
        left.0 < state.0 && state.0 < right.0,
        "local RL placement must not reverse physical note sides:\n{rendered}"
    );
}

#[test]
fn state_composite_notes_anchor_outside_the_group_boundary() {
    for direction in ["TB", "BT", "LR", "RL"] {
        let rendered = render_state(
            &format!(
                concat!(
                    "stateDiagram-v2\n",
                    "direction {direction}\n",
                    "state \"Composite\" as Parent {{\n",
                    "  Child\n",
                    "}}\n",
                    "note left of Parent : left composite\n",
                    "note right of Parent : right composite\n",
                ),
                direction = direction,
            ),
            &AsciiRenderOptions::unicode(),
        )
        .unwrap_or_else(|error| panic!("{direction} composite notes should render: {error}"));

        let left = text_position(&rendered, "left composite");
        let group = text_position(&rendered, "Composite");
        let right = text_position(&rendered, "right composite");
        assert!(
            left.0 < group.0 && group.0 < right.0,
            "{direction} composite note constraints should preserve both physical sides:\n{rendered}"
        );
    }
}

#[test]
fn direct_state_model_preserves_compartments_and_note_side_constraints() {
    let mut titled = direct_state_node("Primary", "rectWithTitle", None, None);
    titled.description = Some(vec!["Details".to_string()]);
    let model = StateDiagramRenderModel {
        direction: "TB".to_string(),
        nodes: vec![
            titled,
            direct_state_node("right direct", "noteGroup", None, Some("right of")),
            direct_state_node("right-note", "note", Some("right direct"), Some("right of")),
            direct_state_node("left direct", "noteGroup", None, Some("left of")),
            direct_state_node("left-note", "note", Some("left direct"), Some("left of")),
        ],
        edges: vec![
            StateDiagramRenderEdge {
                id: "right-edge".to_string(),
                start: "Primary".to_string(),
                end: "right-note".to_string(),
                classes: "transition note-edge".to_string(),
                arrow_type_end: String::new(),
                label: String::new(),
            },
            StateDiagramRenderEdge {
                id: "left-edge".to_string(),
                start: "left-note".to_string(),
                end: "Primary".to_string(),
                classes: "transition note-edge".to_string(),
                arrow_type_end: String::new(),
                label: String::new(),
            },
        ],
        ..StateDiagramRenderModel::default()
    };

    let rendered = render_model(
        &RenderSemanticModel::State(model),
        &AsciiRenderOptions::unicode(),
    )
    .expect("valid direct state model should render");

    let title = text_position(&rendered, "Primary");
    let body = text_position(&rendered, "Details");
    assert!(
        rendered
            .lines()
            .skip(title.1 + 1)
            .take(body.1.saturating_sub(title.1 + 1))
            .any(|line| line.contains('─')),
        "direct-model title/body roles need a structural divider:\n{rendered}"
    );

    let left = text_position(&rendered, "left direct");
    let state = text_position(&rendered, "Primary");
    let right = text_position(&rendered, "right direct");
    assert!(
        left.0 < state.0 && state.0 < right.0,
        "direct-model note side constraints must survive node reordering:\n{rendered}"
    );
}

#[test]
fn direct_state_model_rejects_duplicate_node_ids_before_graph_projection() {
    let mut first = direct_state_node("A", "rect", None, None);
    first.label = Some("First A".into());
    let mut second = direct_state_node("A", "rect", None, None);
    second.label = Some("Second A".into());
    let model = StateDiagramRenderModel {
        direction: "TB".to_string(),
        nodes: vec![first, second],
        edges: vec![StateDiagramRenderEdge {
            id: "self".to_string(),
            start: "A".to_string(),
            end: "A".to_string(),
            classes: "transition".to_string(),
            arrow_type_end: "arrow_barb".to_string(),
            label: "ambiguous".to_string(),
        }],
        ..StateDiagramRenderModel::default()
    };

    let error = render_model(
        &RenderSemanticModel::State(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("duplicate node ids must not select different rank and route instances");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "state",
            feature: "duplicate node ids",
        }
    );
}

#[test]
fn direct_state_model_rejects_edges_with_missing_endpoint_nodes() {
    let model = StateDiagramRenderModel {
        direction: "TB".to_string(),
        nodes: vec![direct_state_node("A", "rect", None, None)],
        edges: vec![StateDiagramRenderEdge {
            id: "missing-target".to_string(),
            start: "A".to_string(),
            end: "Missing".to_string(),
            classes: "transition".to_string(),
            arrow_type_end: "arrow_barb".to_string(),
            label: String::new(),
        }],
        ..StateDiagramRenderModel::default()
    };

    let error = render_model(
        &RenderSemanticModel::State(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("state edges must reference declared nodes before routing");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "state",
            feature: "edges with missing endpoint nodes",
        }
    );
}

#[test]
fn direct_state_model_rejects_non_string_label_values() {
    let mut node = direct_state_node("A", "rect", None, None);
    node.label = Some(false.into());
    let model = StateDiagramRenderModel {
        direction: "TB".to_string(),
        nodes: vec![node],
        ..StateDiagramRenderModel::default()
    };

    let error = render_model(
        &RenderSemanticModel::State(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("non-string labels must not be silently omitted");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "state",
            feature: "state labels with unsupported values",
        }
    );
}

#[test]
fn direct_state_model_rejects_unknown_arrow_type() {
    let model = StateDiagramRenderModel {
        direction: "TB".to_string(),
        nodes: vec![direct_state_node("A", "rect", None, None)],
        edges: vec![StateDiagramRenderEdge {
            id: "self".to_string(),
            start: "A".to_string(),
            end: "A".to_string(),
            classes: "transition".to_string(),
            arrow_type_end: "unsupported-arrow".to_string(),
            label: String::new(),
        }],
        ..StateDiagramRenderModel::default()
    };
    let error = render_model(
        &RenderSemanticModel::State(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("detached state models must validate their explicit arrow selectors");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "state",
            feature: "state arrow types",
        }
    );
}

#[test]
fn direct_state_model_preserves_label_style_priority() {
    let mut node = direct_state_node("A", "rect", None, None);
    node.label = Some("Styled state".into());
    node.css_compiled_styles = vec!["color:#112233".to_string()];
    node.css_styles = vec!["color:#445566".to_string()];
    node.label_style = "color:#778899".to_string();
    let model = RenderSemanticModel::State(StateDiagramRenderModel {
        direction: "TB".to_string(),
        nodes: vec![node],
        ..StateDiagramRenderModel::default()
    });
    let options = AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::TrueColor);
    let rendered = render_model(&model, &options).expect("detached label style should render");
    let plain = render_model(&model, &AsciiRenderOptions::ascii()).unwrap();
    assert_eq!(strip_ansi(&rendered), plain);
    assert!(rendered.contains("\u{1b}[38;2;119;136;153m"));
    assert!(!rendered.contains("\u{1b}[38;2;17;34;51m"));
    assert!(!rendered.contains("\u{1b}[38;2;68;85;102m"));
}

#[test]
fn direct_state_model_rejects_unknown_parent_ids() {
    let model = StateDiagramRenderModel {
        direction: "TB".to_string(),
        nodes: vec![direct_state_node("Child", "rect", Some("Missing"), None)],
        ..StateDiagramRenderModel::default()
    };

    let error = render_model(
        &RenderSemanticModel::State(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("unknown parent ids must not flatten state ownership");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "state",
            feature: "unknown state parent ids",
        }
    );
}

#[test]
fn direct_state_model_rejects_non_group_and_cyclic_parents() {
    let non_group_model = StateDiagramRenderModel {
        direction: "TB".to_string(),
        nodes: vec![
            direct_state_node("Leaf", "rect", None, None),
            direct_state_node("Child", "rect", Some("Leaf"), None),
        ],
        ..StateDiagramRenderModel::default()
    };
    let error = render_model(
        &RenderSemanticModel::State(non_group_model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("leaf states must not own child states");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "state",
            feature: "state parents that are not groups",
        }
    );

    let mut outer = direct_state_node("Outer", "roundedWithTitle", Some("Inner"), None);
    outer.is_group = true;
    outer.node_type = Some("group".to_string());
    let mut inner = direct_state_node("Inner", "roundedWithTitle", Some("Outer"), None);
    inner.is_group = true;
    inner.node_type = Some("group".to_string());
    let cyclic_model = StateDiagramRenderModel {
        direction: "TB".to_string(),
        nodes: vec![outer, inner],
        ..StateDiagramRenderModel::default()
    };
    let error = render_model(
        &RenderSemanticModel::State(cyclic_model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("cyclic state ownership must not be projected");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "state",
            feature: "cyclic state parent ids",
        }
    );
}

#[test]
fn state_note_edges_render_without_arrowheads() {
    let rendered = render_state(
        "stateDiagram-v2\nS1\nnote right of S1 : note text",
        &AsciiRenderOptions::ascii(),
    )
    .expect("state note edges should render as open connectors");

    assert!(
        rendered.contains("S1") && rendered.contains("note text"),
        "state and note should both render:\n{rendered}"
    );
    assert!(
        !rendered
            .chars()
            .any(|ch| matches!(ch, '>' | '<' | '^' | 'v')),
        "note-only state output should not contain arrowheads:\n{rendered}"
    );
}

#[test]
fn state_block_notes_render_multiline_note_nodes() {
    let rendered = render_state(
        "stateDiagram-v2\nA --> B\nnote right of A\n  line1\n  line2\nend note",
        &AsciiRenderOptions::ascii(),
    )
    .expect("state block notes should render as multiline terminal note nodes");

    assert!(
        rendered.contains("line1") && rendered.contains("line2"),
        "block note lines should render in the ASCII output:\n{rendered}"
    );
}

#[test]
fn state_links_do_not_block_ascii_rendering() {
    let rendered = render_state(
        "stateDiagram-v2\nS1\nclick S1 \"https://example.com\" \"Go\"",
        &AsciiRenderOptions::ascii(),
    )
    .expect("state links should not block ASCII rendering");

    assert!(
        rendered.contains("S1"),
        "linked states should keep state nodes renderable:\n{rendered}"
    );
    assert!(
        !rendered.contains("example.com"),
        "state link URLs are SVG metadata and should not leak into ASCII output:\n{rendered}"
    );
}

#[test]
fn state_style_color_truecolor_maps_classdef_and_inline_node_foreground_without_plain_text_changes()
{
    let input = concat!(
        "stateDiagram-v2\n",
        "classDef warm color:#112233,border:1px solid #445566,background:#ffeecc\n",
        "A:::warm --> B\n",
        "style B color:#778899,border:1px solid #aabbcc,background:#001122\n",
    );
    let options = AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::TrueColor);

    let rendered = render_state(input, &options).expect("state foreground styles should render");
    let plain = render_state(input, &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(strip_ansi(&rendered), plain);
    for expected_code in [
        "\u{1b}[38;2;17;34;51m",
        "\u{1b}[38;2;68;85;102m",
        "\u{1b}[38;2;119;136;153m",
        "\u{1b}[38;2;170;187;204m",
    ] {
        assert!(
            rendered.contains(expected_code),
            "missing {expected_code:?} in {rendered:?}"
        );
    }
    for ignored_background_code in ["\u{1b}[38;2;255;238;204m", "\u{1b}[38;2;0;17;34m"] {
        assert!(
            !rendered.contains(ignored_background_code),
            "background style should not be emitted as foreground in {rendered:?}"
        );
    }
    for expected_background_code in ["\u{1b}[48;2;255;238;204m", "\u{1b}[48;2;0;17;34m"] {
        assert!(
            rendered.contains(expected_background_code),
            "missing background {expected_background_code:?} in {rendered:?}"
        );
    }
}

#[test]
fn state_group_transition_endpoints_attach_to_group_boundary() {
    let rendered = render_state(
        "stateDiagram-v2\nstate Parent {\n  Child\n}\nA --> Parent",
        &AsciiRenderOptions::ascii(),
    )
    .expect("state transitions should be able to target composite state boundaries");

    assert!(
        rendered.contains("Parent"),
        "target composite state should render as a group label:\n{rendered}"
    );
    assert!(
        rendered.contains("Child"),
        "target composite state should keep its child state visible:\n{rendered}"
    );
    assert!(
        rendered.contains("A"),
        "source state should render outside the target group:\n{rendered}"
    );
}

#[test]
fn state_composite_entry_transition_attaches_to_group_boundary() {
    let rendered = render_state(
        "stateDiagram-v2\n[*] --> Active\nstate Active {\n  [*] --> A\n  A --> B\n}",
        &AsciiRenderOptions::ascii(),
    )
    .expect("entry transitions should attach to composite state boundaries");

    assert!(
        rendered.contains("Active"),
        "composite state title should render:\n{rendered}"
    );
    assert!(
        rendered.contains("A") && rendered.contains("B"),
        "composite state children should render:\n{rendered}"
    );
    assert!(
        rendered.matches("| * |").count() >= 2,
        "root and nested start pseudo states should render:\n{rendered}"
    );
}

#[test]
fn state_fork_and_join_pseudo_states_render_as_sync_bars() {
    let rendered = render_state(
        "stateDiagram-v2\nstate fork_state <<fork>>\n[*] --> fork_state\nfork_state --> State2\nfork_state --> State3\nstate join_state <<join>>\nState2 --> join_state\nState3 --> join_state\njoin_state --> State4\nState4 --> [*]",
        &AsciiRenderOptions::ascii(),
    )
    .expect("fork and join pseudo states should render");

    assert!(
        rendered.lines().any(|line| line.contains("State2"))
            && rendered.lines().any(|line| line.contains("State3"))
            && rendered.lines().any(|line| line.contains("State4")),
        "fork/join branches should keep their target states visible:\n{rendered}"
    );
    assert!(
        rendered.contains("======="),
        "fork/join pseudo states should render as thick synchronization bars:\n{rendered}"
    );
    assert!(
        !rendered.contains("fork_state") && !rendered.contains("join_state"),
        "fork/join implementation ids should not leak into ASCII output:\n{rendered}"
    );
}

#[test]
fn state_choice_pseudo_state_renders_without_internal_id() {
    let rendered = render_state(
        "stateDiagram-v2\nstate choice_state <<choice>>\n[*] --> choice_state\nchoice_state --> A: yes\nchoice_state --> B: no",
        &AsciiRenderOptions::ascii(),
    )
    .expect("choice pseudo state should render");

    assert!(
        rendered.contains("yes") && rendered.contains("no"),
        "choice branch labels should render on outgoing edges:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.contains('<') && line.contains('>')),
        "choice pseudo state should render as a visible diamond-like node:\n{rendered}"
    );
    assert!(
        !rendered.contains("choice_state"),
        "choice implementation id should not leak into ASCII output:\n{rendered}"
    );
}

#[test]
fn state_dividers_render_as_stacked_sections() {
    let rendered = render_state(
        "stateDiagram-v2\nstate Active {\n  A\n  --\n  B\n}",
        &AsciiRenderOptions::ascii(),
    )
    .expect("state dividers should render as stacked sections");

    assert!(
        rendered.contains("Active"),
        "parent composite state should render:\n{rendered}"
    );
    assert!(
        rendered.contains("A") && rendered.contains("B"),
        "divider sections should keep their child states visible:\n{rendered}"
    );
    assert!(
        rendered.lines().filter(|line| line.contains("...")).count() >= 2,
        "divider sections should render horizontal separators:\n{rendered}"
    );
    assert!(
        !rendered.contains("divider-id") && !rendered.contains("id-"),
        "divider implementation ids should not leak into ASCII output:\n{rendered}"
    );
}

#[test]
fn state_empty_concurrent_regions_remain_structural_separators() {
    for body in [
        "--\nA",
        "A\n--",
        "A\n--\n--\nB",
        "--\n--",
        "A\n--\nB\nA --> B: send\nB --> A: retry",
    ] {
        for direction in ["LR", "RL", "TB", "BT"] {
            let source =
                format!("stateDiagram-v2\ndirection {direction}\nstate RUN {{\n{body}\n}}");
            for options in concurrency_options() {
                let rendered = render_state(&source, &options)
                    .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
                support::assert_rectangular_terminal_grid(&rendered);
                assert_eq!(rendered.matches("RUN").count(), 1, "{rendered}");
                assert!(
                    !rendered.contains("divider-id") && !rendered.contains("id-"),
                    "structural region ids must not become visible nodes:\n{rendered}"
                );
                for label in ["A", "B", "send", "retry"] {
                    if body.contains(label) {
                        assert_eq!(
                            rendered
                                .split(|ch: char| !ch.is_alphanumeric())
                                .filter(|word| *word == label)
                                .count(),
                            1,
                            "{source}\n{rendered}"
                        );
                    }
                }
            }
        }
    }
}

fn concurrency_options() -> [AsciiRenderOptions; 4] {
    [
        AsciiRenderOptions::ascii(),
        AsciiRenderOptions::unicode(),
        AsciiRenderOptions::ascii()
            .with_node_padding(0)
            .with_graph_padding_x(0)
            .with_graph_padding_y(0),
        AsciiRenderOptions::unicode()
            .with_node_padding(0)
            .with_graph_padding_x(0)
            .with_graph_padding_y(0),
    ]
}

#[test]
fn direct_state_root_divider_keeps_clear_of_ordinary_peer_nodes() {
    for direction in ["LR", "RL", "TB", "BT"] {
        for contained in [false, true] {
            let mut divider = direct_state_node("root-region", "divider", None, None);
            divider.is_group = true;
            let mut nodes = vec![
                direct_state_node("A", "rect", contained.then_some("BOX"), None),
                divider,
            ];
            if contained {
                let mut group = direct_state_node("BOX", "roundedWithTitle", None, None);
                group.is_group = true;
                nodes.insert(0, group);
            }
            let model = RenderSemanticModel::State(StateDiagramRenderModel {
                direction: direction.into(),
                nodes,
                ..StateDiagramRenderModel::default()
            });
            for options in concurrency_options() {
                let rendered = render_model(&model, &options).unwrap();
                support::assert_rectangular_terminal_grid(&rendered);
                assert_eq!(rendered.matches('A').count(), 1, "{rendered}");
                assert!(
                    rendered.contains('.') || rendered.contains('┄'),
                    "the separator must remain visible:\n{rendered}"
                );
                assert!(!rendered.contains("root-region"), "{rendered}");
            }
        }
    }
}

#[test]
fn state_transitions_can_skip_concurrent_regions_without_losing_heads_or_labels() {
    fn target_heads(rendered: &str) -> Vec<char> {
        let rows: Vec<Vec<char>> = rendered
            .lines()
            .map(|line| line.chars().collect())
            .collect();
        let vertical = |ch: char| matches!(ch, '|' | '│' | '+' | '├' | '┤');
        let frames: Vec<_> = ['A', 'B', 'C', 'D']
            .into_iter()
            .map(|label| {
                let (x, y) = rows
                    .iter()
                    .enumerate()
                    .find_map(|(y, row)| row.iter().position(|ch| *ch == label).map(|x| (x, y)))
                    .unwrap_or_else(|| panic!("missing state {label}:\n{rendered}"));
                let left = (0..x).rev().find(|x| vertical(rows[y][*x])).unwrap();
                let right = (x + 1..rows[y].len())
                    .find(|x| vertical(rows[y][*x]))
                    .unwrap();
                let top = (0..y)
                    .rev()
                    .find(|y| matches!(rows[*y][left], '+' | '┌'))
                    .unwrap();
                let bottom = (y + 1..rows.len())
                    .find(|y| matches!(rows[*y][left], '+' | '└'))
                    .unwrap();
                (label, left, right, top, bottom)
            })
            .collect();
        let mut targets = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.iter().enumerate() {
                let (dx, dy) = match ch {
                    '>' | '►' => (1, 0),
                    '<' | '◄' => (-1, 0),
                    '^' | '▲' => (0, -1),
                    'v' | '▼' => (0, 1),
                    _ => continue,
                };
                let contact = x.checked_add_signed(dx).zip(y.checked_add_signed(dy));
                let owners: Vec<_> = frames
                    .iter()
                    .filter(|(_, left, right, top, bottom)| {
                        contact.is_some_and(|(x, y)| {
                            (dy == 0 && (x == *left || x == *right) && *top < y && y < *bottom)
                                || (dx == 0
                                    && (y == *top || y == *bottom)
                                    && *left < x
                                    && x < *right)
                        })
                    })
                    .collect();
                assert_eq!(
                    owners.len(),
                    1,
                    "head ({x}, {y}) must touch its state:\n{rendered}"
                );
                targets.push(owners[0].0);
            }
        }
        targets.sort_unstable();
        targets
    }

    for direction in ["LR", "RL", "TB", "BT"] {
        for edges in [
            "A --> D: send\nD --> A: retry",
            "A --> D: one\nB --> D: two\nC --> D: three",
        ] {
            let source = format!(
                "stateDiagram-v2\ndirection {direction}\nstate RUNNING {{\nA\n--\nB\n--\nC\n--\nD\n}}\n{edges}"
            );
            for options in concurrency_options() {
                let rendered = render_state(&source, &options)
                    .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
                support::assert_rectangular_terminal_grid(&rendered);
                let expected_targets = if edges.contains("retry") {
                    vec!['A', 'D']
                } else {
                    vec!['D'; 3]
                };
                assert_eq!(target_heads(&rendered), expected_targets, "{rendered}");
                for label in ["one", "two", "three", "send", "retry"] {
                    if edges.contains(label) {
                        assert_eq!(rendered.matches(label).count(), 1, "{rendered}");
                    }
                }
                assert!(
                    !rendered.contains("divider-id") && !rendered.contains("id-"),
                    "{rendered}"
                );
            }
        }
    }
}

#[test]
fn state_concurrent_branches_and_nested_pseudo_states_keep_their_transitions() {
    for (body, labels, expected_heads) in [
        (
            "state RUNNING {\n[*] --> A\nA --> B: work\nA --> C: skip\nB --> C: done\n--\n[*] --> Watch\nWatch --> Watch: tick\n}\nRUNNING --> Done: stop",
            vec!["work", "skip", "done", "tick", "stop"],
            8,
        ),
        (
            "state RUNNING {\nstate Worker {\n[*] --> Build\nBuild --> [*]: built\n--\n[*] --> Monitor\nMonitor --> [*]: stopped\n}\nWorker --> [*]: complete\n--\n[*] --> Audit\nAudit --> [*]: recorded\n}\nRUNNING --> Done",
            vec!["built", "stopped", "complete", "recorded"],
            9,
        ),
    ] {
        for direction in ["LR", "RL", "TB", "BT"] {
            let source = format!("stateDiagram-v2\ndirection {direction}\n[*] --> RUNNING\n{body}");
            for options in concurrency_options() {
                let rendered = render_state(&source, &options)
                    .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
                support::assert_rectangular_terminal_grid(&rendered);
                let heads = rendered
                    .chars()
                    .filter(|ch| matches!(ch, '>' | '<' | '^' | 'v' | '►' | '◄' | '▲' | '▼'))
                    .count();
                assert_eq!(heads, expected_heads, "{rendered}");
                for label in &labels {
                    assert_eq!(rendered.matches(label).count(), 1, "{rendered}");
                }
                assert!(
                    !rendered.contains("divider-id") && !rendered.contains("id-"),
                    "{rendered}"
                );
            }
        }
    }
}

#[test]
fn state_local_semantic_fixture_covers_composite_boundaries() {
    let input = read_local_semantic_fixture("state/composite_boundary.mmd");

    let rendered = render_state(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic state fixture should render");

    for expected in ["Outer", "Ready", "Busy", "Idle"] {
        assert!(
            rendered.contains(expected),
            "local semantic state fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        rendered.lines().count() >= 5,
        "local semantic state fixture should produce a multi-line layout:\n{rendered}"
    );
}

#[test]
fn state_local_semantic_fixture_covers_cjk_connection_lifecycle() {
    let input = read_local_semantic_fixture("state/cjk_connection_lifecycle.mmd");

    let expected_text = [
        "空闲",
        "连接中",
        "已连接",
        "断开中",
        "重连中",
        "连接",
        "成功",
        "超时",
        "达到上限",
        "完成",
    ];
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            let rendered = render_state(&input, &options.with_terminal_width_profile(profile))
                .expect("CJK local semantic state fixture should render");
            support::assert_rectangular_terminal_grid_with_profile(&rendered, profile);
            for expected in expected_text {
                assert!(
                    rendered.contains(expected),
                    "CJK state fixture should keep {expected:?} visible:\n{rendered}"
                );
            }
            let rows = ["连接中", "等待", "认证", "已连接", "断开中"]
                .map(|label| first_line_index_containing(&rendered, label));
            assert!(
                rows.windows(2).all(|pair| pair[0] < pair[1]),
                "CJK state fixture should keep the composite progression in order:\n{rendered}"
            );
            assert_eq!(
                rendered
                    .matches(['>', '<', 'v', '^', '►', '◄', '▼', '▲'])
                    .count(),
                9,
                "{rendered}"
            );
            assert!(
                rendered.lines().count() <= 48,
                "the Canonical lifecycle should stay within the admitted reading height:\n{rendered}"
            );
        }
    }
}

#[test]
fn bidirectional_state_labels_keep_reading_clearance() {
    let rendered = render_state(
        "stateDiagram-v2\n Draft --> Review: submit\n Review --> Draft: changes requested\n",
        &AsciiRenderOptions::unicode(),
    )
    .unwrap();
    assert_eq!(rendered.matches("submit").count(), 1);
    assert_eq!(rendered.matches("changes requested").count(), 1);
    assert!(!rendered.contains("submitchanges requested"), "{rendered}");
    assert!(!rendered.contains("changes requestedsubmit"), "{rendered}");
}
