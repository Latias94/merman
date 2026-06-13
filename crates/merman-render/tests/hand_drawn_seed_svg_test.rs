use futures::executor::block_on;
use merman_core::{Engine, ParseOptions};
use merman_render::svg::{SvgRenderOptions, render_layouted_svg};
use merman_render::{LayoutOptions, layout_parsed};
use serde_json::{Value, json};

fn render_svg(diagram_id: &str, source: &str) -> String {
    let engine = Engine::new();
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
                        "requirementBkgColor": "#f0fdf4",
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
            r##"fill="#ECECFF""##,
            r##"stroke="#16a34a" stroke-width="1.3""##,
            r#"class="divider""#,
        ],
    );
}
