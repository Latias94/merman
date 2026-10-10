mod support;

use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiError, AsciiLayoutProfile,
    AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy, AsciiRgb,
};
use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::flowchart::{
    FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility, FlowNode, FlowchartModel,
};
use merman_core::resources::ResourceProfile;
use merman_core::{Engine, OperationControl, ParseOptions};
use std::path::Path;
use support::render_model;
use unicode_width::UnicodeWidthStr;

fn render_flowchart(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    render_flowchart_with_resources(input, options, AsciiResourcePolicy::default())
}

fn render_flowchart_with_resources(
    input: &str,
    options: &AsciiRenderOptions,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("flowchart should parse")
        .expect("flowchart should be detected");

    let context = merman_core::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .expect("deterministic test operation context");
    AsciiRenderer::new(*options)?.render_parsed(
        &parsed,
        &OperationControl::new(),
        &context,
        resources,
    )
}

fn parse_flowchart_error(input: &str) -> String {
    Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect_err("flowchart should fail to parse")
        .to_string()
}

fn fixture_expected(directory: &str, name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata/mermaid-ascii")
        .join(directory)
        .join(name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
        .replace("\r\n", "\n");
    let (_, expected) = content
        .split_once("\n---\n")
        .unwrap_or_else(|| panic!("fixture missing separator: {}", path.display()));
    expected.to_string()
}

fn local_semantic_input(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata/local-semantic")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
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

fn strip_html_spans(input: &str) -> String {
    let mut output = String::new();
    let mut index = 0;
    while index < input.len() {
        let rest = &input[index..];
        if rest.starts_with("<span ") {
            index += rest.find('>').expect("span start tag should be closed") + 1;
            continue;
        }
        if rest.starts_with("</span>") {
            index += "</span>".len();
            continue;
        }
        let ch = rest
            .chars()
            .next()
            .expect("index should be on a char boundary");
        if let Some(entity) = rest.strip_prefix("&gt;") {
            output.push('>');
            index += rest.len() - entity.len();
        } else if let Some(entity) = rest.strip_prefix("&lt;") {
            output.push('<');
            index += rest.len() - entity.len();
        } else if let Some(entity) = rest.strip_prefix("&amp;") {
            output.push('&');
            index += rest.len() - entity.len();
        } else {
            output.push(ch);
            index += ch.len_utf8();
        }
    }
    output
}

fn normalize_ascii_art(input: &str) -> String {
    input
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn first_line_index_containing(rendered: &str, needle: &str) -> usize {
    rendered
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing {needle:?} in rendered fixture:\n{rendered}"))
}

fn assert_rectangular_char_grid(rendered: &str) {
    let mut lines = rendered.lines();
    let Some(first) = lines.next() else {
        return;
    };
    let width = first.chars().count();
    for line in lines {
        assert_eq!(
            line.chars().count(),
            width,
            "rendered lines should stay aligned:\n{rendered}"
        );
    }
}

fn terminal_test_width(input: &str) -> usize {
    UnicodeWidthStr::width(input)
}

fn assert_rectangular_terminal_grid(rendered: &str) {
    let mut lines = rendered.lines();
    let Some(first) = lines.next() else {
        return;
    };
    let width = terminal_test_width(first);
    for line in lines {
        assert_eq!(
            terminal_test_width(line),
            width,
            "rendered lines should stay terminal-cell aligned:\n{rendered}"
        );
    }
}

fn single_node_flowchart_model(layout_shape: &str, label: &str) -> FlowchartModel {
    FlowchartModel {
        keyword: "graph".to_string(),
        acc_descr: None,
        acc_title: None,
        class_defs: Default::default(),
        direction: Some("LR".to_string()),
        edge_defaults: None,
        vertex_calls: Vec::new(),
        nodes: vec![FlowNode {
            id: "A".to_string(),
            provenance: Default::default(),
            label: Some(label.to_string()),
            label_type: None,
            layout_shape: Some(layout_shape.to_string()),
            shape: None,
            icon: None,
            form: None,
            pos: None,
            img: None,
            constraint: None,
            asset_width: None,
            asset_height: None,
            classes: Vec::new(),
            styles: Vec::new(),
            link: None,
            link_target: None,
            have_callback: false,
        }],
        edges: Vec::new(),
        subgraphs: Vec::new(),
        tooltips: Default::default(),
        warning_facts: Vec::new(),
    }
}

#[path = "flowchart_model/appearance.rs"]
mod appearance;
#[path = "flowchart_model/boundary_routes.rs"]
mod boundary_routes;
#[path = "flowchart_model/direction_and_labels.rs"]
mod direction_and_labels;
#[path = "flowchart_model/edges.rs"]
mod edges;
#[path = "flowchart_model/graph_routing.rs"]
mod graph_routing;
#[path = "flowchart_model/shapes.rs"]
mod shapes;
#[path = "flowchart_model/subgraphs.rs"]
mod subgraphs;

#[test]
fn feedback_edge_preserves_diamond_vertex_and_enters_target_side() {
    let input = "flowchart TD\n A[Write code] --> B{Tests pass?}\n B -- Yes --> C[Open PR]\n B -- No --> A\n";
    for mut options in [AsciiRenderOptions::unicode(), AsciiRenderOptions::ascii()] {
        for profile in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
            options.layout_profile = profile;
            let rendered = render_flowchart(input, &options).unwrap();
            let rows: Vec<Vec<char>> = rendered
                .lines()
                .map(|line| line.chars().collect())
                .collect();
            let diamond_row = rendered
                .lines()
                .position(|line| line.contains("Tests pass?"))
                .unwrap();
            let vertex_x = rows[diamond_row].iter().rposition(|ch| *ch == '>').unwrap();
            assert!(
                rows[diamond_row][vertex_x + 1..]
                    .iter()
                    .any(|ch| matches!(ch, '-' | '─')),
                "the side escape must connect to the preserved diamond vertex: {rendered}"
            );
            let target_row = rendered
                .lines()
                .position(|line| line.contains("Write code"))
                .unwrap();
            assert!(
                rows[target_row].iter().any(|ch| matches!(ch, '◄' | '<')),
                "the feedback head must point into the right side of its target: {rendered}"
            );
        }
    }
}

#[test]
fn reverse_edge_labels_keep_reading_clearance() {
    let rendered = render_flowchart(
        "flowchart TD\n Draft -->|submit| Review\n Review -->|changes requested| Draft\n",
        &AsciiRenderOptions::unicode(),
    )
    .unwrap();
    assert_eq!(rendered.matches("submit").count(), 1);
    assert_eq!(rendered.matches("changes requested").count(), 1);
    assert!(!rendered.contains("submitchanges requested"), "{rendered}");
    assert!(!rendered.contains("changes requestedsubmit"), "{rendered}");
}

#[test]
fn zero_vertical_padding_reduces_only_node_height() {
    let input = "flowchart TD\n A[Write code] --> B[Open PR] --> C[Merge]\n";
    let default = render_flowchart(input, &AsciiRenderOptions::unicode()).unwrap();
    let compact_nodes =
        render_flowchart(input, &AsciiRenderOptions::unicode().with_node_padding_y(0)).unwrap();
    assert_eq!(default.lines().count(), 25);
    assert_eq!(compact_nodes.lines().count(), 19);
    assert_eq!(
        default.lines().map(|line| line.chars().count()).max(),
        compact_nodes.lines().map(|line| line.chars().count()).max()
    );
    for label in ["Write code", "Open PR", "Merge"] {
        assert_eq!(compact_nodes.matches(label).count(), 1);
    }
}

#[test]
fn diamond_self_loop_source_head_keeps_its_straight_berth_and_bend() {
    for direction in ["LR", "RL", "TD", "BT"] {
        for (options, left_head, right_head, horizontal) in [
            (AsciiRenderOptions::ascii(), '<', '>', '-'),
            (AsciiRenderOptions::unicode(), '◄', '►', '─'),
        ] {
            for profile in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
                let rendered = render_flowchart(
                    &format!("flowchart {direction}\nA{{AAA}} <--> A"),
                    &options.with_layout_profile(profile),
                )
                .expect("a bidirectional diamond self loop should render");
                let row = rendered.lines().find(|line| line.contains("AAA")).unwrap();
                let chars = row.chars().collect::<Vec<_>>();
                let label_start = chars.windows(3).position(|part| part == ['A'; 3]).unwrap();
                let vertex = if direction == "RL" {
                    chars[..label_start]
                        .iter()
                        .rposition(|ch| *ch == '<')
                        .unwrap()
                } else {
                    label_start
                        + 3
                        + chars[label_start + 3..]
                            .iter()
                            .position(|ch| *ch == '>')
                            .unwrap()
                };
                let head = if direction == "RL" {
                    vertex - 1
                } else {
                    vertex + 1
                };
                assert_eq!(
                    chars[head],
                    if direction == "RL" {
                        right_head
                    } else {
                        left_head
                    },
                    "{rendered}"
                );
                let after_head = if direction == "RL" {
                    &chars[..head]
                } else {
                    &chars[head + 1..]
                };
                assert!(
                    after_head
                        .iter()
                        .any(|ch| matches!(ch, '+' | '┌' | '┐' | '└' | '┘')),
                    "the source marker must leave the exterior bend intact: {rendered}"
                );
                assert!(
                    after_head.iter().all(|ch| *ch == ' '
                        || *ch == horizontal
                        || matches!(ch, '+' | '┌' | '┐' | '└' | '┘')),
                    "source head must remain on the straight terminal run: {rendered}"
                );
            }
        }
    }
}
