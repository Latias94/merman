use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::C4DiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use std::sync::Arc;

fn render_c4_svg_with_environment(source: &str, environment: &RenderEnvironment) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let session = environment.begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("layout ok");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned()
}

#[derive(Debug)]
struct C4TypeWidthProbe;

impl TextMeasurer for C4TypeWidthProbe {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        let width = match text {
            "«person»" => 151.0,
            "«system»" => 252.0,
            "«external_person»" => 399.0,
            _ => 80.0,
        };
        TextMetrics {
            width,
            height: style.font_size.max(1.0),
            line_count: 1,
        }
    }
}

fn deep_c4_boundary_chain(depth: usize) -> String {
    let mut input = String::from("C4Context\n");
    for level in 0..depth {
        input.push_str(&format!("Boundary(b{level}, \"B{level}\") {{\n"));
    }
    input.push_str("System(leaf, \"Leaf\")\n");
    for _ in 0..depth {
        input.push_str("}\n");
    }
    input
}

#[test]
fn c4_public_layout_handles_deep_boundary_chain() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    const DEPTH: usize = 1500;
    let source = deep_c4_boundary_chain(DEPTH);

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    assert_eq!(parsed.metadata().diagram_type, "c4");

    let options = LayoutOptions::default();
    let artifact = family::prepare(parsed, &options, session)
        .expect("layout should not depend on recursive boundary traversal");
    let projection = artifact.layout_json().expect("serialize C4 layout");
    let c4: C4DiagramLayout = serde_json::from_value(projection["layout"]["C4Diagram"].clone())
        .expect("C4 layout projection");

    assert_eq!(c4.boundaries.len(), DEPTH + 1);
    assert_eq!(c4.shapes.len(), 1);
    assert_eq!(c4.shapes[0].alias, "leaf");
}

#[test]
fn c4_type_text_length_comes_from_canonical_text_measurement() {
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.c4-type-width").unwrap(),
        "test",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(C4TypeWidthProbe),
        )),
    );
    let svg = render_c4_svg_with_environment(
        r#"C4Context
Person(person, "Person")
System(system, "System")
Person_Ext(external, "External")
"#,
        &environment,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");

    for (label, expected) in [
        ("<<person>>", "151"),
        ("<<system>>", "252"),
        ("<<external_person>>", "399"),
    ] {
        let text = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some(label))
            .unwrap_or_else(|| panic!("missing C4 type label {label}: {svg}"));
        assert_eq!(text.attribute("textLength"), Some(expected), "{label}");
    }
}
