use merman_ascii::{AsciiColorMode, AsciiRenderOptions, render_model};
use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::state::{
    StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
};
use merman_core::{Engine, ParseOptions};
use std::path::Path;

fn render_state(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
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

    let rendered = render_state(&input, &AsciiRenderOptions::ascii())
        .expect("CJK local semantic state fixture should render");

    for expected in [
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
    ] {
        assert!(
            rendered.contains(expected),
            "CJK state fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        first_line_index_containing(&rendered, "连接中")
            < first_line_index_containing(&rendered, "等待"),
        "CJK state fixture should keep the composite lifecycle readable:\n{rendered}"
    );
    assert!(
        first_line_index_containing(&rendered, "等待")
            < first_line_index_containing(&rendered, "认证")
            && first_line_index_containing(&rendered, "认证")
                < first_line_index_containing(&rendered, "已连接")
            && first_line_index_containing(&rendered, "已连接")
                < first_line_index_containing(&rendered, "断开中")
            && first_line_index_containing(&rendered, "断开中")
                < first_line_index_containing(&rendered, "完成"),
        "CJK state fixture should keep the internal lifecycle progression in order:\n{rendered}"
    );
    assert!(
        rendered.lines().count() >= 7,
        "CJK state fixture should produce a multi-line layout:\n{rendered}"
    );
}
