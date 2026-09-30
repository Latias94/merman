use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_state(config: &str, source: &str) -> String {
    let source = format!("---\nconfig:\n  layout: dagre\n{config}---\n{source}");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
        .expect("parse State")
        .expect("State diagram");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("layout State")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State")
        .svg()
        .to_owned()
}

#[test]
fn state_container_palette_preserves_preorder_regions_and_author_opt_out() {
    let svg = render_state(
        "  theme: redux-color\n  look: neo\n  themeVariables:\n    borderColorArray: ['#112233', '#445566']\n    bkgColorArray: ['#ddeeff']\n",
        r#"stateDiagram-v2
state Parent {
  state Nested {
    N1 --> N2
  }
}
state Parallel {
  [*] --> R1
  --
  [*] --> R2
}
state Styled {
  [*] --> S1
  --
  [*] --> S2
}
classDef pinned fill:#ffffff,stroke:#000000
class Styled pinned
state Later {
  L1 --> L2
}
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    for (id, expected) in [
        ("Parent", Some("color-0")),
        ("Nested", Some("color-1")),
        ("Parallel", Some("color-0")),
        ("Styled", None),
        ("Later", Some("color-0")),
    ] {
        let cluster = document
            .descendants()
            .find(|node| node.attribute("data-id") == Some(id))
            .expect("named composite");
        assert_eq!(cluster.attribute("data-color-id"), expected, "{id}");
    }
    let regions: Vec<_> = document
        .descendants()
        .filter(|node| {
            node.attribute("class").is_some_and(|class| {
                class
                    .split_whitespace()
                    .any(|name| name == "statediagram-cluster")
            }) && node.descendants().any(|child| {
                child.has_tag_name("rect") && child.attribute("class") == Some("divider")
            })
        })
        .collect();
    assert_eq!(regions.len(), 4);
    assert_eq!(
        regions
            .iter()
            .filter(|node| node.attribute("data-color-id") == Some("color-0"))
            .count(),
        2
    );
    assert_eq!(
        regions
            .iter()
            .filter(|node| node.attribute("data-color-id").is_none())
            .count(),
        2
    );
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .unwrap();
    assert_eq!(css.matches("data-color-id=").count(), 14);
    assert!(css.contains(".statediagram-cluster rect.inner{stroke:#112233;}"));
    assert!(css.contains(".statediagram-cluster rect.outer{stroke:#445566;fill:#ddeeff;}"));
    assert!(!css.contains(".inner path[fill='none']"));
}

#[test]
fn state_palette_theme_gate_and_empty_background_keep_body_colors() {
    for theme in ["redux-dark-color", "default"] {
        let svg = render_state(
            &format!(
                "  theme: {theme}\n  look: handDrawn\n  themeVariables:\n    borderColorArray: ['#112233']\n    bkgColorArray: []\n"
            ),
            "stateDiagram-v2\nstate Composite {\n  A --> B\n}\n",
        );
        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        let cluster = document
            .descendants()
            .find(|node| node.attribute("data-id") == Some("Composite"))
            .unwrap();
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .unwrap();
        if theme == "default" {
            assert_eq!(cluster.attribute("data-color-id"), None);
            assert!(!css.contains("data-color-id="));
        } else {
            assert_eq!(cluster.attribute("data-color-id"), Some("color-0"));
            assert_eq!(css.matches("data-color-id=").count(), 5);
            assert!(
                css.contains(".statediagram-cluster .outer path[fill='none']{stroke:#112233;}")
            );
            assert!(!css.contains(".statediagram-cluster .outer path[stroke='none']"));
            assert!(css.contains(".statediagram-cluster rect.outer{stroke:#112233;}"));
        }
    }
}
