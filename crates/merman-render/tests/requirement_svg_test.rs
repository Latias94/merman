use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_requirement(source: &str) -> String {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "theme": "default",
        "look": "classic"
    })));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse Requirement")
        .expect("Requirement detected");
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin render session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Requirement");
    artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("requirement-css".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render Requirement SVG")
        .svg()
        .to_string()
}

#[test]
fn requirement_source_styles_reject_css_injection_tokens() {
    let svg = render_requirement(
        r#"requirementDiagram
  requirement unsafe {
    id: 1
    text: Unsafe source style
    risk: low
    verifymethod: analysis
  }
  classDef unsafe fill:url(https://example.test/paint.svg),stroke:#654321
  class unsafe unsafe
  style unsafe fill:red;stroke:blue,color:#123456
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Requirement SVG");
    let node = document
        .descendants()
        .find(|node| node.attribute("id") == Some("requirement-css-unsafe"))
        .expect("unsafe Requirement node");
    let styles = node
        .descendants()
        .filter_map(|node| node.attribute("style"))
        .collect::<Vec<_>>()
        .join(";");

    assert!(!styles.contains("example.test"), "{styles}");
    assert!(!styles.contains("red;stroke:blue"), "{styles}");
    assert!(styles.contains("stroke:#654321 !important"), "{styles}");
    assert!(styles.contains("color:#123456 !important"), "{styles}");
}

#[test]
fn requirement_safe_styles_preserve_order_overrides_and_quoted_values() {
    let svg = render_requirement(
        r##"requirementDiagram
  requirement safe {
    id: 1
    text: Safe source style
    risk: low
    verifymethod: analysis
  }
  classDef branded fill:#112233,stroke:#445566,font-family:"A!important&B"
  class safe branded
  style safe fill:#778899!important,stroke-width:3px
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Requirement SVG");
    let node = document
        .descendants()
        .find(|node| node.attribute("id") == Some("requirement-css-safe"))
        .expect("safe Requirement node");
    let styles = node
        .descendants()
        .filter_map(|node| node.attribute("style"))
        .collect::<Vec<_>>();

    assert!(
        styles.contains(
            &"fill:#778899 !important;stroke:#445566 !important;stroke-width:3px !important"
        ),
        "{styles:?}"
    );
    assert!(
        styles
            .iter()
            .any(|style| style.contains(r#"font-family:"A!important&B" !important"#)),
        "{styles:?}"
    );
    assert!(!svg.contains("!important !important"), "{svg}");
}

#[test]
fn requirement_svg_multiline_name_applies_role_to_the_first_row_only() {
    let svg = render_requirement(
        r#"%%{init: {"htmlLabels": false}}%%
requirementDiagram
requirement "W<br/>WW WW" {
  text: ok
}
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Requirement SVG");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node
                    .children()
                    .filter(|row| row.has_tag_name("tspan"))
                    .count()
                    == 2
                && node
                    .descendants()
                    .filter(|child| child.is_text())
                    .filter_map(|child| child.text())
                    .collect::<String>()
                    == "WWW WW"
        })
        .expect("two-row requirement name");
    let rows = label
        .children()
        .filter(|row| row.has_tag_name("tspan"))
        .collect::<Vec<_>>();
    let first_word = rows[0]
        .children()
        .find(|word| word.has_tag_name("tspan"))
        .unwrap();
    assert!(
        first_word.attribute("font-weight") == Some("bold")
            || first_word
                .attribute("style")
                .is_some_and(|style| style.contains("font-weight: bold"))
    );
    let later_words = rows[1]
        .children()
        .filter(|word| word.has_tag_name("tspan"))
        .collect::<Vec<_>>();
    assert_eq!(later_words.len(), 2);
    for word in later_words {
        assert_eq!(word.attribute("font-weight"), Some("normal"));
        assert_eq!(word.attribute("style"), None);
    }
}
