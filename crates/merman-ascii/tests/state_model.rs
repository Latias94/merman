use merman_ascii::{AsciiColorMode, AsciiRenderOptions, render_model};
use merman_core::{Engine, ParseOptions};
use std::path::Path;

fn render_state(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("state diagram should parse")
        .expect("state diagram should be detected");

    assert_eq!(parsed.meta.diagram_type, "stateDiagram");
    render_model(&parsed.model, options)
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
