use merman_ascii::{AsciiColorMode, AsciiError, AsciiRenderOptions, render_model};
use merman_core::{Engine, ParseOptions};

fn render_state(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("state diagram should parse")
        .expect("state diagram should be detected");

    assert_eq!(parsed.meta.diagram_type, "stateDiagram");
    render_model(&parsed.model, options)
}

fn assert_unsupported_state(input: &str, feature: &'static str) {
    let err = render_state(input, &AsciiRenderOptions::ascii()).unwrap_err();

    assert_eq!(
        err,
        AsciiError::UnsupportedFeature {
            diagram_type: "state",
            feature,
        }
    );
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
fn state_start_and_end_pseudo_states_render_as_visible_nodes() {
    let rendered = render_state(
        "stateDiagram-v2\n[*] --> A\nA --> [*]",
        &AsciiRenderOptions::ascii(),
    )
    .expect("start and end pseudo states should render");

    assert!(
        rendered.matches("| * |").count() >= 2,
        "start and end pseudo states should render as visible star nodes:\n{rendered}"
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
}

#[test]
fn state_group_transition_endpoints_are_explicitly_unsupported() {
    assert_unsupported_state(
        "stateDiagram-v2\nstate Parent {\n  Child\n}\nA --> Parent",
        "state group transition endpoints",
    );
}
