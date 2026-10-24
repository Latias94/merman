#![cfg(all(feature = "math", feature = "diagram-flowchart"))]

use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions};

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|value| value.split_ascii_whitespace().any(|token| token == class))
}

#[test]
fn native_math_edge_label_paints_its_background_before_the_formula() {
    let source = r##"---
config:
  htmlLabels: true
  theme: base
  themeVariables:
    edgeLabelBackground: '#c0ffee'
---
flowchart LR
A -->|$$x^2$$| B
"##;
    let environment = RenderEnvironment::deterministic().with_compiled_math_renderer();
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::default(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let parity = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_string();
    let reference = roxmltree::Document::parse(&parity).unwrap();
    let source_label = reference
        .descendants()
        .find(|node| {
            node.has_tag_name("foreignObject")
                && node
                    .descendants()
                    .any(|child| has_class(child, "merman-native-math-svg"))
        })
        .expect("RaTeX formula inside the source HTML label");
    assert!(
        source_label
            .ancestors()
            .any(|node| node.has_tag_name("g") && has_class(node, "edgeLabel"))
    );
    let label_id = source_label
        .parent_element()
        .unwrap()
        .attribute("data-id")
        .expect("source edge label identity");
    let source_paths = source_label
        .descendants()
        .filter(|node| node.has_tag_name("path"))
        .map(|node| node.attribute("d").unwrap())
        .collect::<Vec<_>>();
    assert!(!source_paths.is_empty());
    assert!(!source_label.descendants().any(|node| node.is_text()));

    let svg = SvgPipeline::resvg_safe()
        .process_to_string(&parity, &environment.begin_session().unwrap())
        .unwrap();
    let document = roxmltree::Document::parse(&svg).unwrap();
    let label = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some(label_id))
        .expect("processed source edge label");
    let instance = label
        .descendants()
        .find(|node| node.has_tag_name("use"))
        .expect("edge label fallback instance");
    let definition_id = instance
        .attribute("href")
        .unwrap()
        .strip_prefix('#')
        .unwrap();
    let definition = document
        .descendants()
        .find(|node| node.attribute("id") == Some(definition_id))
        .expect("this edge label's fallback definition");
    let viewport = definition
        .children()
        .find(|node| node.has_tag_name("svg"))
        .expect("fallback label viewport");
    let projection = viewport
        .children()
        .find(|node| has_class(*node, "merman-prepared-math-native"))
        .expect("native formula in the referenced label viewport");
    let background = viewport
        .children()
        .take_while(|node| *node != projection)
        .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#c0ffee"))
        .expect("painted edge background must precede the formula in the same viewport");
    for coordinate in ["x", "y"] {
        assert_eq!(background.attribute(coordinate), Some("0"));
    }
    for dimension in ["width", "height"] {
        let expected: f64 = source_label.attribute(dimension).unwrap().parse().unwrap();
        let actual: f64 = background.attribute(dimension).unwrap().parse().unwrap();
        assert!(expected.is_finite() && expected > 0.0);
        assert_eq!(actual, expected);
    }
    assert_eq!(
        projection
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .map(|node| node.attribute("d").unwrap())
            .collect::<Vec<_>>(),
        source_paths,
    );
}
