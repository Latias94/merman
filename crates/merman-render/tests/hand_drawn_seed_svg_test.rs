use futures::executor::block_on;
mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::svg::{SvgRenderOptions, render_layouted_svg};
use merman_render::{LayoutOptions, layout_parsed};
use serde_json::{Value, json};

fn render_svg(diagram_id: &str, source: &str) -> String {
    let engine = legacy_init_theme_compat_engine();
    let parsed = block_on(engine.parse_diagram(source, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::headless_svg_defaults();
    let out = layout_parsed(&parsed, &layout_options).expect("layout ok");

    render_layouted_svg(
        &out,
        layout_options.text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            apply_root_overrides: false,
            ..SvgRenderOptions::default()
        },
    )
    .expect("render svg")
}

fn source_with_init(init: Value, diagram: &str) -> String {
    format!("%%{{init: {init}}}%%\n{diagram}")
}

fn assert_seeded_svg_contract<F>(
    family: &str,
    diagram_id: &str,
    source_for_seed: F,
    expected_fragments: &[&str],
) where
    F: Fn(u64) -> String,
{
    let seed_7 = render_svg(diagram_id, &source_for_seed(7));
    let seed_7_again = render_svg(diagram_id, &source_for_seed(7));
    let seed_8 = render_svg(diagram_id, &source_for_seed(8));

    assert_eq!(
        seed_7, seed_7_again,
        "same handDrawnSeed should keep {family} rough SVG deterministic"
    );
    assert_ne!(
        seed_7, seed_8,
        "different handDrawnSeed should change visible {family} rough paths"
    );

    for expected in expected_fragments {
        assert!(
            seed_7.contains(expected),
            "{family} seed proof should include visible fragment {expected:?}: {seed_7}"
        );
    }
}

fn path_chunk_by_id<'a>(svg: &'a str, id: &str) -> &'a str {
    let id_attr = format!(r#"id="{id}""#);
    let id_start = svg.find(&id_attr).expect("path id");
    let path_start = svg[..id_start].rfind("<path ").expect("path start");
    let path_end = svg[id_start..].find("/>").expect("path end") + id_start + "/>".len();
    &svg[path_start..path_end]
}

fn fixed_chunk_after<'a>(svg: &'a str, needle: &str, len: usize) -> &'a str {
    let start = svg.find(needle).expect("chunk needle");
    &svg[start..(start + len).min(svg.len())]
}

fn cluster_shape_chunk<'a>(svg: &'a str, id: &str) -> &'a str {
    let needle = format!(r#"<g class="cluster" id="{id}" data-look="handDrawn">"#);
    let start = svg.find(&needle).expect("cluster start");
    let shape_end = svg[start..]
        .find(r#"<g class="cluster-label""#)
        .expect("cluster label start")
        + start;
    &svg[start..shape_end]
}

fn node_shape_chunk<'a>(svg: &'a str, id: &str) -> &'a str {
    let id_attr = format!(r#"id="{id}""#);
    let id_start = svg.find(&id_attr).expect("node id");
    let node_start = svg[..id_start].rfind("<g ").expect("node start");
    let label_start = svg[id_start..]
        .find(r#"<g class="label""#)
        .map(|idx| id_start + idx)
        .expect("node label start");
    &svg[node_start..label_start]
}

fn node_shape_chunk_by_id_prefix<'a>(svg: &'a str, id_prefix: &str) -> &'a str {
    let id_attr_prefix = format!(r#"id="{id_prefix}"#);
    let id_start = svg.find(&id_attr_prefix).expect("node id prefix");
    let node_start = svg[..id_start].rfind("<g ").expect("node start");
    let label_start = svg[id_start..]
        .find(r#"<g class="label""#)
        .map(|idx| id_start + idx)
        .expect("node label start");
    &svg[node_start..label_start]
}

fn first_path_d(chunk: &str) -> &str {
    let marker = r#"<path d=""#;
    let d_start = chunk.find(marker).expect("first path") + marker.len();
    let d_end = chunk[d_start..].find('"').expect("first path d end") + d_start;
    &chunk[d_start..d_end]
}

fn path_element_count(chunk: &str) -> usize {
    chunk.matches(r#"<path d=""#).count()
}

fn first_svg_move_y(d: &str) -> f64 {
    let rest = d.strip_prefix('M').expect("path starts with move");
    rest.split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
        .filter(|part| !part.is_empty())
        .nth(1)
        .expect("move y")
        .parse::<f64>()
        .expect("move y number")
}

#[test]
fn flowchart_svg_hand_drawn_basic_rect_uses_rough_node_wrapper_and_hachure_paths() {
    let source_for_seed = |seed| {
        source_with_init(
            json!({
                "look": "handDrawn",
                "handDrawnSeed": seed,
                "themeVariables": {
                    "mainBkg": "#f8fafc",
                    "nodeBorder": "#ef4444"
                }
            }),
            r#"flowchart TD
  A[Start]
"#,
        )
    };

    let seed_7 = render_svg("flowchart-hand-rect", &source_for_seed(7));
    let seed_7_again = render_svg("flowchart-hand-rect", &source_for_seed(7));
    let seed_8 = render_svg("flowchart-hand-rect", &source_for_seed(8));

    assert_eq!(
        seed_7, seed_7_again,
        "same handDrawnSeed should keep basic Flowchart node rough paths deterministic"
    );
    assert_ne!(
        seed_7, seed_8,
        "different handDrawnSeed should change the visible basic Flowchart node rough paths"
    );
    assert!(
        seed_7.contains(r#"<g class="rough-node default" id="flowchart-hand-rect-flowchart-A-0""#),
        "hand-drawn basic node should use Mermaid's rough-node wrapper class: {seed_7}"
    );
    assert!(
        !seed_7.contains(r#"<g class="node default" id="flowchart-hand-rect-flowchart-A-0""#),
        "hand-drawn basic node should not keep the classic node wrapper class: {seed_7}"
    );
    assert!(
        seed_7.contains(r#"<g class="basic label-container" style=""><path d=""#)
            && seed_7.contains(
                r##"stroke="#f8fafc" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d=""##
            )
            && seed_7.contains(
                r##"stroke="#ef4444" stroke-width="1.3" fill="none" stroke-dasharray="0 0"/>"##
            ),
        "hand-drawn basic node should render RoughJS hachure fill and outline paths: {seed_7}"
    );
    assert!(
        !seed_7.contains(r#"<rect class="basic label-container""#),
        "hand-drawn basic node should not fall back to a plain rect: {seed_7}"
    );
}

#[test]
fn flowchart_svg_hand_drawn_decision_hachure_keeps_diamond_silhouette() {
    let svg = render_svg(
        "flowchart-hand-decision",
        &source_with_init(
            json!({
                "look": "handDrawn",
                "handDrawnSeed": 7,
                "theme": "dark"
            }),
            r#"flowchart TB
  A{Decision}
"#,
        ),
    );

    let chunk = node_shape_chunk(&svg, "flowchart-hand-decision-flowchart-A-0");
    assert!(
        chunk.contains(
            r#"<g class="rough-node default" id="flowchart-hand-decision-flowchart-A-0""#
        ),
        "hand-drawn decision should render through the rough-node wrapper: {chunk}"
    );

    let first_y = first_svg_move_y(first_path_d(chunk));
    assert!(
        first_y > -80.0 && first_y < -40.0,
        "hand-drawn decision hachure should start near the left edge, not the top vertex; y={first_y}: {chunk}"
    );
}

#[test]
fn flowchart_svg_hand_drawn_high_risk_shapes_keep_hachure_starts_in_bounds() {
    let svg = render_svg(
        "flowchart-hand-risk-shapes",
        &source_with_init(
            json!({
                "look": "handDrawn",
                "handDrawnSeed": 7,
                "theme": "dark"
            }),
            r#"flowchart LR
  D@{ shape: diam, label: "diam" }
  H@{ shape: hex, label: "hex" }
  LR@{ shape: lean-r, label: "lean-r" }
  LL@{ shape: lean-l, label: "lean-l" }
  TB@{ shape: trap-b, label: "trap-b" }
  TT@{ shape: trap-t, label: "trap-t" }
  R@{ shape: rounded, label: "rounded" }
  S@{ shape: stadium, label: "stadium" }
  D --> H --> LR --> LL --> TB --> TT --> R --> S
"#,
        ),
    );

    for (node_id, min_y, max_y) in [
        ("D", -80.0, -30.0),
        ("H", -65.0, -20.0),
        ("LR", -65.0, -20.0),
        ("LL", -65.0, -20.0),
        ("TB", -65.0, -20.0),
        ("TT", -80.0, -30.0),
        ("R", -50.0, -5.0),
        ("S", -35.0, 10.0),
    ] {
        let chunk = node_shape_chunk_by_id_prefix(
            &svg,
            &format!("flowchart-hand-risk-shapes-flowchart-{node_id}-"),
        );
        assert!(
            chunk.contains(r#"class="rough-node"#),
            "{node_id} should render through the rough-node wrapper: {chunk}"
        );
        assert_eq!(
            path_element_count(chunk),
            2,
            "{node_id} should render one hachure fill path and one rough outline path: {chunk}"
        );

        let first_y = first_svg_move_y(first_path_d(chunk));
        assert!(
            first_y > min_y && first_y < max_y,
            "{node_id} hachure start should stay within the expected local shape band; y={first_y}: {chunk}"
        );
    }
}

#[test]
fn flowchart_svg_hand_drawn_class_default_styles_reach_rough_nodes() {
    let svg = render_svg(
        "flowchart-hand-class",
        &source_with_init(
            json!({
                "look": "handDrawn",
                "themeVariables": {
                    "mainBkg": "#ececff",
                    "nodeBorder": "#9370db"
                }
            }),
            r#"graph TD
  A[myClass1] --> B[default]
  classDef default stroke-width:2px,fill:none,stroke:silver
  classDef myClass1 color:#0000ff
  class A myClass1
"#,
        ),
    );

    assert!(
        svg.contains(r#"class="rough-node default myClass1""#),
        "classDef default should keep hand-drawn rough-node wrappers and attached classes: {svg}"
    );
    assert!(
        svg.contains(r#"stroke="silver" stroke-width="2""#),
        "classDef default stroke should reach the hand-drawn rough path: {svg}"
    );
}

#[test]
fn flowchart_svg_hand_drawn_seed_controls_visible_rough_paths() {
    assert_seeded_svg_contract(
        "Flowchart",
        "flowchart-seed",
        |seed| {
            source_with_init(
                json!({
                    "look": "handDrawn",
                    "handDrawnSeed": seed,
                    "themeVariables": {
                        "mainBkg": "#f8fafc",
                        "nodeBorder": "#ef4444",
                        "lineColor": "#0f172a",
                        "strokeWidth": 3
                    }
                }),
                r#"flowchart TD
  A{{Hex}}
"#,
            )
        },
        &[
            r#"id="flowchart-seed-flowchart-A-0" transform="translate"#,
            r#"data-look="handDrawn""#,
            r#"<g transform="translate"#,
            r##"stroke="#f8fafc" stroke-width="4" fill="none" stroke-dasharray="0 0""##,
            r##"stroke="#ef4444" stroke-width="1.2999999523162842" fill="none" stroke-dasharray="0 0""##,
        ],
    );
}

#[test]
fn flowchart_svg_hand_drawn_seed_controls_edge_and_cluster_rough_paths() {
    let source_for_seed = |seed| {
        source_with_init(
            json!({
                "look": "handDrawn",
                "handDrawnSeed": seed,
                "themeVariables": {
                    "clusterBkg": "#f8fafc",
                    "clusterBorder": "#ef4444",
                    "mainBkg": "#e0f2fe",
                    "nodeBorder": "#0369a1"
                }
            }),
            r#"flowchart LR
subgraph Group
  A[Start] --> B{Choose}
end
B --> C[Done]
linkStyle 0 stroke:#123456,stroke-width:2px
"#,
        )
    };

    let seed_7 = render_svg("flowchart-seed-surfaces", &source_for_seed(7));
    let seed_7_again = render_svg("flowchart-seed-surfaces", &source_for_seed(7));
    let seed_8 = render_svg("flowchart-seed-surfaces", &source_for_seed(8));

    assert_eq!(
        seed_7, seed_7_again,
        "same handDrawnSeed should keep Flowchart edge and cluster rough SVG deterministic"
    );

    let edge_7 = path_chunk_by_id(&seed_7, "flowchart-seed-surfaces-L_A_B_0");
    let edge_8 = path_chunk_by_id(&seed_8, "flowchart-seed-surfaces-L_A_B_0");
    assert_ne!(
        edge_7, edge_8,
        "different handDrawnSeed should change the visible rough edge path"
    );
    assert!(
        edge_7.contains("transition")
            && edge_7.contains(
                r#"marker-end="url(#flowchart-seed-surfaces_flowchart-v2-pointEnd_stroke__123456)""#
            )
            && edge_7.contains(r#"data-look="handDrawn""#),
        "hand-drawn edge should keep Mermaid transition class, marker, and data attributes: {edge_7}"
    );
    assert!(
        seed_7.contains(r#"id="flowchart-seed-surfaces_flowchart-v2-pointEnd_stroke__123456""#)
            && !seed_7.contains(
                r##"id="flowchart-seed-surfaces_flowchart-v2-pointEnd_stroke__123456" class="marker flowchart-v2" viewBox="0 0 10 10" refX="5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;" stroke="#123456" fill="#123456"/>"##
            ),
        "hand-drawn colored marker ids should follow Mermaid's raw stroke token without inline sanitized color attrs"
    );

    let cluster_7 = cluster_shape_chunk(&seed_7, "flowchart-seed-surfaces-Group");
    let cluster_8 = cluster_shape_chunk(&seed_8, "flowchart-seed-surfaces-Group");
    assert_ne!(
        cluster_7, cluster_8,
        "different handDrawnSeed should change the visible rough cluster path"
    );

    let cluster_group_7 = fixed_chunk_after(
        &seed_7,
        r#"<g class="cluster" id="flowchart-seed-surfaces-Group" data-look="handDrawn">"#,
        1600,
    );
    assert!(
        cluster_7.contains("<path ") && !cluster_7.contains("<rect "),
        "hand-drawn cluster should use a rough path group instead of a plain rect: {cluster_group_7}"
    );
}

#[test]
fn class_svg_hand_drawn_seed_controls_visible_rough_paths() {
    assert_seeded_svg_contract(
        "Class",
        "class-seed",
        |seed| {
            source_with_init(
                json!({
                    "look": "handDrawn",
                    "handDrawnSeed": seed,
                    "themeVariables": {
                        "mainBkg": "#f8fafc",
                        "nodeBorder": "#ef4444",
                        "lineColor": "#0f172a"
                    }
                }),
                r#"classDiagram
  A --> B
  class A {
    +start()
  }
"#,
            )
        },
        &[
            r#"class="rough-node default" id="class-seed-classId-A-0""#,
            r#"class="edge-thickness-normal edge-pattern-solid transition relation""#,
            r##"stroke="#000" stroke-width="1" fill="none""##,
            r##"stroke="#f8fafc" stroke-width="4""##,
            r##"stroke="#ef4444" stroke-width="1.3""##,
        ],
    );
}

#[test]
fn er_svg_hand_drawn_seed_controls_visible_rough_paths() {
    assert_seeded_svg_contract(
        "ER",
        "er-seed",
        |seed| {
            source_with_init(
                json!({
                    "handDrawnSeed": seed,
                    "themeVariables": {
                        "mainBkg": "#eff6ff",
                        "nodeBorder": "#2563eb"
                    }
                }),
                r#"erDiagram
  CUSTOMER {
    string id
    string name
  }
  ORDER {
    string id
  }
  CUSTOMER ||--o{ ORDER : places
"#,
            )
        },
        &[
            r#"id="er-seed-entity-CUSTOMER-0" class="node default" data-look="classic""#,
            r#"class="outer-path""#,
            r##"fill="#eff6ff""##,
            r##"stroke="#2563eb""##,
            r#"class="row-rect-odd""#,
        ],
    );
}

#[test]
fn requirement_svg_hand_drawn_seed_controls_visible_rough_paths() {
    assert_seeded_svg_contract(
        "Requirement",
        "requirement-seed",
        |seed| {
            source_with_init(
                json!({
                    "handDrawnSeed": seed,
                    "themeVariables": {
                        "mainBkg": "#f0fdf4",
                        "nodeBorder": "#16a34a",
                        "requirementBackground": "#f0fdf4",
                        "requirementBorderColor": "#16a34a"
                    }
                }),
                r#"requirementDiagram
  requirement req1 {
    id: 1
    text: Seeded requirement
    risk: high
    verifymethod: analysis
  }
"#,
            )
        },
        &[
            r#"id="requirement-seed-req1" data-look="classic""#,
            r#"class="basic label-container outer-path""#,
            r##"fill="#f0fdf4""##,
            r##"stroke="#16a34a" stroke-width="1.3""##,
            r#"class="divider""#,
        ],
    );
}
