#![cfg(all(
    feature = "svg",
    feature = "diagram-flowchart",
    feature = "diagram-sequence",
    feature = "diagram-class",
    feature = "diagram-state",
    feature = "diagram-er",
    feature = "diagram-gantt",
    feature = "diagram-pie",
))]

use merman::svg::{SvgOutputPolicy, SvgPipeline};
use merman::{
    Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest,
};
use serde_json::{Value, json};

const CASES: &[(&str, &str)] = &[
    ("flowchart", "flowchart LR\n A -->|label| B"),
    ("sequence", "sequenceDiagram\n Alice->>Bob: Hello"),
    ("class", "classDiagram\n A <|-- B"),
    ("state", "stateDiagram-v2\n [*] --> A\n A --> [*]"),
    ("er", "erDiagram\n A ||--o{ B : owns"),
    (
        "gantt",
        "gantt\n dateFormat YYYY-MM-DD\n section Work\n Task :2026-01-01,1d",
    ),
    ("pie", "pie\n \"A\" : 1"),
];

fn render(source: &str, config: Value, pipeline: Option<SvgPipeline>) -> String {
    let output = Renderer::new()
        .with_engine(Engine::new().with_site_config(MermaidConfig::from_value(config)))
        .render(RenderRequest::svg(
            source,
            OperationControl::new(),
            SvgRequest {
                pipeline,
                ..Default::default()
            },
        ))
        .expect("diagram should render");
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("expected SVG output");
    };
    svg.into_parts().0
}

#[test]
fn core_svg_omits_canvas_background_across_families_themes_and_sizing() {
    for &(family, source) in CASES {
        for theme in ["default", "dark", "base"] {
            for use_max_width in [true, false] {
                let svg = render(
                    source,
                    json!({
                        "theme": theme,
                        "themeVariables": {"background": "#112233"},
                        (family): {"useMaxWidth": use_max_width},
                    }),
                    None,
                );
                let doc = roxmltree::Document::parse(&svg).expect("valid SVG XML");
                let root = doc.root_element();
                let style = root.attribute("style").unwrap_or_default();
                assert!(
                    !style.contains("background"),
                    "{family}/{theme}/{use_max_width}: {style}"
                );
                if family == "pie" {
                    assert_eq!(style.contains("max-width:"), use_max_width);
                }
                assert_ne!(root.attribute("style"), Some(""));
                assert!(root.attribute("viewBox").is_some(), "{family}: {svg}");
            }
        }
    }
}

#[test]
fn explicit_canvas_policy_preserves_diagram_colors_and_geometry() {
    for &(family, source) in CASES {
        let core = render(source, json!({"htmlLabels": false}), None);
        let core_doc = roxmltree::Document::parse(&core).expect("valid core SVG");
        for color in ["transparent", "white", "#112233"] {
            let projected = render(
                source,
                json!({"htmlLabels": false}),
                Some(
                    SvgOutputPolicy {
                        root_background_color: Some(color.to_string()),
                        ..Default::default()
                    }
                    .pipeline(),
                ),
            );
            let projected_doc =
                roxmltree::Document::parse(&projected).expect("valid projected SVG");
            assert!(
                projected_doc
                    .root_element()
                    .attribute("style")
                    .unwrap()
                    .contains(&format!("background-color: {color};")),
                "{family}/{color}"
            );
            assert_eq!(
                core_doc.root_element().attribute("viewBox"),
                projected_doc.root_element().attribute("viewBox"),
                "{family}/{color}"
            );
            assert_eq!(
                core.split_once('>').unwrap().1,
                projected.split_once('>').unwrap().1,
                "canvas policy changed diagram content for {family}/{color}"
            );
        }
    }
}
