use futures::executor::block_on;
mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_render::svg::{SvgRenderOptions, render_layouted_svg};
use merman_render::{LayoutOptions, layout_parsed};

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

struct LookDomCase {
    name: &'static str,
    diagram_id: &'static str,
    source: &'static str,
    expected_fragments: &'static [&'static str],
}

#[test]
fn configured_look_reaches_declared_dom_consumers() {
    let cases = [
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
            source: r##"%%{init: {"look": "neo", "themeVariables": {"mainBkg": "#606060", "stateBorder": "#040404", "strokeWidth": 4}}}%%
stateDiagram-v2
[*] --> Active
state Active {
  Idle --> Busy
}
"##,
            expected_fragments: &[
                r#"data-look="neo""#,
                r##"[data-look="neo"].statediagram-cluster rect{fill:#606060;stroke:#040404;stroke-width:4;}"##,
            ],
        },
        LookDomCase {
            name: "mindmap",
            diagram_id: "look-mindmap",
            source: r#"%%{init: {"look": "neo"}}%%
mindmap
  Root
    Child
"#,
            expected_fragments: &[r#"id="look-mindmap-node_0" data-look="neo""#],
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

    for case in cases {
        let svg = render_svg(case.diagram_id, case.source);

        for expected in case.expected_fragments {
            assert!(
                svg.contains(expected),
                "{} should contain look fragment {expected:?}: {svg}",
                case.name
            );
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
    let svg = render_svg(
        "look-sequence",
        r##"%%{init: {"look": "neo", "themeVariables": {"dropShadow": "drop-shadow(1px 2px 3px rgba(0,0,0,.4))"}}}%%
sequenceDiagram
  participant A
  participant B
  A->>B: Hi
"##,
    );

    assert!(
        svg.contains(
            r#"#look-sequence .labelBox{stroke:#9370DB;fill:#ECECFF;filter:drop-shadow(1px 2px 3px rgba(0,0,0,.4));}"#
        ),
        "sequence should consume look=neo through presentation CSS/theme paths: {svg}"
    );
    assert!(
        !svg.contains(r#"data-look="classic""#),
        "sequence should not leak classic DOM look attributes: {svg}"
    );
}
