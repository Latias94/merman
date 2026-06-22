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
            r#"<g class="basic label-container" transform="translate"#,
            r##"fill="#f8fafc""##,
            r##"stroke="#ef4444""##,
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
                r#"marker-end="url(#flowchart-seed-surfaces_flowchart-v2-pointEnd__123456)""#
            )
            && edge_7.contains(r#"data-look="handDrawn""#),
        "hand-drawn edge should keep Mermaid transition class, marker, and data attributes: {edge_7}"
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
