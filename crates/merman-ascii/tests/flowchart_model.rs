mod support;

use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiError, AsciiRenderOptions,
    AsciiResourceLimitId, AsciiResourcePolicy, AsciiRgb,
};
use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::flowchart::{
    FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility, FlowNode, FlowchartModel,
};
use merman_core::resources::ResourceProfile;
use merman_core::{Engine, ParseOptions};
use std::path::Path;
use support::{render_model, render_model_with_resources};
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

    render_model_with_resources(parsed.model(), options, resources)
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
