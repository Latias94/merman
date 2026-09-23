use futures::executor::block_on;
mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_svg(diagram_id: &str, source: &str) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let engine = legacy_init_theme_compat_engine();
    render_svg_with_engine(diagram_id, &engine, source, session)
}

fn render_svg_with_engine(
    diagram_id: &str,
    engine: &Engine,
    source: &str,
    session: merman_render::environment::RenderSession,
) -> String {
    let parsed = block_on(engine.parse_diagram_for_render_model(source, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::headless_svg_defaults();
    let artifact = family::prepare(parsed, &layout_options, session).expect("layout ok");

    artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render svg")
        .svg()
        .to_owned()
}

struct LookDomCase {
    name: &'static str,
    diagram_id: &'static str,
    source: &'static str,
    expected_fragments: &'static [&'static str],
}

#[test]
fn configured_look_reaches_declared_dom_consumers() {
    let cases = vec![
        LookDomCase {
            name: "flowchart",
            diagram_id: "look-flowchart",
            source: r#"%%{init: {"look": "neo"}}%%
flowchart TB
subgraph Group
  A
end
"#,
            expected_fragments: &[
                r#"<g class="cluster" id="look-flowchart-Group" data-look="neo""#,
                r#"id="look-flowchart-flowchart-A-0" transform="translate"#,
            ],
        },
        LookDomCase {
            name: "class",
            diagram_id: "look-class",
            source: r#"%%{init: {"look": "neo"}}%%
classDiagram
namespace Zoo {
  class Animal
  class Keeper
}
Animal --> Keeper
"#,
            expected_fragments: &[r#"id="look-class-Zoo" data-look="neo""#],
        },
        LookDomCase {
            name: "er",
            diagram_id: "look-er",
            source: r#"%%{init: {"look": "neo"}}%%
erDiagram
  CUSTOMER ||--o{ ORDER : places
"#,
            expected_fragments: &[
                r#"id="look-er-entity-CUSTOMER-0" class="node default" data-look="neo""#,
            ],
        },
        LookDomCase {
            name: "state",
            diagram_id: "look-state",
            source: r##"%%{init: {"look": "neo", "themeVariables": {"mainBkg": "#606060", "nodeBorder": "#040404", "strokeWidth": 4}}}%%
stateDiagram-v2
[*] --> Active
state Active {
  Idle --> Busy
}
"##,
            expected_fragments: &[
                r#"data-look="neo""#,
                r##"[data-look="neo"][data-color-id="color-0"].statediagram-cluster rect.outer{stroke:#E879F9;fill:#FDF4FF;}"##,
            ],
        },
        LookDomCase {
            name: "requirement",
            diagram_id: "look-requirement",
            source: r#"%%{init: {"look": "neo"}}%%
requirementDiagram
  requirement req1 {
    id: 1
    text: Visible requirement
    risk: high
    verifymethod: analysis
  }
  element sys {
    type: system
  }
  sys - satisfies -> req1
"#,
            expected_fragments: &[
                r#"data-look="neo""#,
                r#"#look-requirement [data-look="neo"].node path"#,
            ],
        },
        LookDomCase {
            name: "kanban",
            diagram_id: "look-kanban",
            source: r#"%%{init: {"look": "neo"}}%%
kanban
  Todo
    Task
"#,
            expected_fragments: &[r#"id="look-kanban-Todo" data-look="neo""#],
        },
    ];

    #[cfg(feature = "layout-cytoscape")]
    let cases = {
        let mut cases = cases;
        cases.push(LookDomCase {
            name: "mindmap",
            diagram_id: "look-mindmap",
            source: r#"%%{init: {"look": "neo"}}%%
mindmap
  Root
    Child
"#,
            expected_fragments: &[r#"id="look-mindmap-node_0" data-look="neo""#],
        });
        cases
    };

    for case in cases {
        let svg = render_svg(case.diagram_id, case.source);

        for expected in case.expected_fragments {
            assert!(
                svg.contains(expected),
                "{} should contain look fragment {expected:?}: {svg}",
                case.name
            );
        }
        if matches!(case.name, "class" | "state" | "requirement") {
            let document = roxmltree::Document::parse(&svg).expect("valid SVG");
            let paths: Vec<_> = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("path") && node.attribute("data-edge") == Some("true")
                })
                .collect();
            assert!(
                !paths.is_empty(),
                "{} must exercise a shared edge",
                case.name
            );
            for path in paths {
                let style = path.attribute("style").expect("Neo edge mask style");
                assert!(
                    style.starts_with("stroke-dasharray: 0 0 "),
                    "{}: {style}",
                    case.name
                );
                assert!(
                    style.contains("; stroke-dashoffset: 0;"),
                    "{}: {style}",
                    case.name
                );
                if case.name == "requirement" {
                    assert!(
                        style.contains("stroke-dashoffset: 0;fill:none;stroke-dasharray: 10,7"),
                        "explicit Requirement dashes must override the generated prefix: {style}"
                    );
                }
            }
        }
        assert!(
            !svg.contains(r#"data-look="classic""#),
            "{} should not leak classic data-look when configured for neo: {svg}",
            case.name
        );
    }
}

#[test]
fn sequence_look_matrix_covers_css_theme_consumption() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "look": "neo",
        "themeVariables": {
            "dropShadow": "drop-shadow(1px 2px 3px rgba(0,0,0,.4))"
        }
    })));
    let svg = render_svg_with_engine(
        "look-sequence",
        &engine,
        r#"sequenceDiagram
  participant A
  participant B
  A->>B: Hi
"#,
        RenderEnvironment::deterministic().begin_session().unwrap(),
    );

    assert!(
        svg.contains(
            r#"#look-sequence .labelBox{stroke:#28253D;fill:#ffffff;filter:drop-shadow(1px 2px 3px rgba(0,0,0,.4));}"#
        ),
        "sequence should consume look=neo through presentation CSS/theme paths: {svg}"
    );
    assert!(
        !svg.contains(r#"data-look="classic""#),
        "sequence should not leak classic DOM look attributes: {svg}"
    );
}
