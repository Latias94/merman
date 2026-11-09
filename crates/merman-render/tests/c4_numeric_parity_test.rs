#![cfg(feature = "diagram-c4")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::C4DiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render(source: &str) -> (C4DiagramLayout, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse C4")
        .expect("C4 diagram")
        .into_presentation();
    let environment = RenderEnvironment::deterministic();
    let session = environment.begin_session().expect("render session");
    let options = LayoutOptions::headless_svg_defaults().with_screen_available_width(4000.0);
    let artifact = family::prepare(&parsed, &options, session).expect("prepare C4");
    let projection = artifact.layout_json().expect("layout projection");
    let layout =
        serde_json::from_value(projection["layout"]["C4Diagram"].clone()).expect("typed C4 layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render C4 SVG")
        .svg()
        .to_owned();
    (layout, svg)
}

fn label_position(svg: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid C4 SVG");
    let label = document
        .descendants()
        .find(|node| node.has_tag_name("tspan") && node.text() == Some("Calls"))
        .expect("relation label")
        .parent()
        .expect("relation text element");
    let number = |name| {
        label
            .attribute(name)
            .expect("text coordinate")
            .parse::<f64>()
            .expect("numeric coordinate")
    };
    (number("x"), number("y"))
}

#[test]
fn c4_offsets_apply_parse_int_again_only_after_javascript_string_coercion() {
    // Mermaid 12.3.0 (2086904e), svgDraw.ts:35-36 reparses String(rel.offsetX/Y).
    // At 1e21 the Number is rendered as "1e+21", whose integer prefix is only 1.
    // A sparse named offset still contains the original decimal string at this stage.
    let body = "C4Context\nSystem(a, \"Alpha\")\nSystem(b, \"Beta\")\nRel(a, b, \"Calls\")\n";
    let (_, baseline_svg) = render(body);
    let baseline = label_position(&baseline_svg);
    for (token, numeric_offset, string_offset) in [
        ("1000000000000000000000", 1.0, 1e21),
        ("999999999999999999999", 1.0, 1e21),
        (
            "999999999999999900000",
            9.999999999999999e20,
            9.999999999999999e20,
        ),
        ("-1000000000000000000000", -1.0, -1e21),
        (
            "9007199254740993",
            9_007_199_254_740_992.0,
            9_007_199_254_740_992.0,
        ),
        (
            "-9007199254740993",
            -9_007_199_254_740_992.0,
            -9_007_199_254_740_992.0,
        ),
        (
            "0x20000000000001tail",
            9_007_199_254_740_992.0,
            9_007_199_254_740_992.0,
        ),
        (
            "-0x100000000000000000000000000000001",
            -3.0,
            -2_f64.powi(128),
        ),
        ("+60.75", 60.0, 60.0),
        ("-0", 0.0, 0.0),
    ] {
        for (arguments, expected) in [
            (
                format!("\"red\", \"blue\", $offsetX=\"{token}\", $offsetY=\"{token}\""),
                numeric_offset,
            ),
            (
                format!("$offsetX=\"{token}\", $offsetY=\"{token}\""),
                string_offset,
            ),
        ] {
            let source = format!("{body}UpdateRelStyle(a, b, {arguments})\n");
            let (_, svg) = render(&source);
            let actual = label_position(&svg);
            // Account for the two additions in byTspan (midpoint + offset, then half
            // the measured label width), without deriving the expected coercion from Rust.
            for (actual, base) in [(actual.0, baseline.0), (actual.1, baseline.1)] {
                let expected_position = base + expected;
                let tolerance = (expected_position.abs() * f64::EPSILON * 2.0).max(1e-8);
                assert!(
                    (actual - expected_position).abs() <= tolerance,
                    "{arguments}: expected coordinate {expected_position}, got {actual}",
                );
            }
        }
    }

    // Large row limits must survive the database and prevent the default four/two wrap.
    let mut source = String::from(
        "C4Context\nUpdateLayoutConfig(\"1000000000000000000000\", \"1000000000000000000000\")\n",
    );
    for index in 0..5 {
        source.push_str(&format!("System(n{index}, \"Node\")\n"));
    }
    for index in 0..3 {
        source.push_str(&format!(
            "Boundary(b{index}, \"Boundary\") {{\nSystem(c{index}, \"Child\")\n}}\n"
        ));
    }
    let (layout, _) = render(&source);
    let root_shapes: Vec<_> = layout
        .shapes
        .iter()
        .filter(|shape| shape.parent_boundary == "global")
        .collect();
    assert_eq!(root_shapes.len(), 5);
    assert!(root_shapes.iter().all(|shape| shape.y == root_shapes[0].y));
    let boundaries: Vec<_> = layout
        .boundaries
        .iter()
        .filter(|boundary| boundary.alias != "global")
        .collect();
    assert_eq!(boundaries.len(), 3);
    assert!(
        boundaries
            .iter()
            .all(|boundary| boundary.y == boundaries[0].y)
    );
}

#[test]
fn c4_truthy_offsets_preserve_nonfinite_label_coordinates() {
    let huge = "9".repeat(309);
    for (arguments, expected) in [
        (format!("\"red\", \"blue\", \"{huge}\", \"{huge}\""), "NaN"),
        (
            format!("$offsetX=\"{huge}\", $offsetY=\"{huge}\""),
            "Infinity",
        ),
        (
            format!("$offsetX=\"-{huge}\", $offsetY=\"-{huge}\""),
            "-Infinity",
        ),
        (
            "$offsetX=\"invalid\", $offsetY=\"invalid\"".to_string(),
            "NaN",
        ),
    ] {
        let source = format!(
            "C4Context\nSystem(a, \"Alpha\")\nSystem(b, \"Beta\")\nRel(a, b, \"Calls\")\nUpdateRelStyle(a, b, {arguments})\n"
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact =
            family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let label = document
            .descendants()
            .find(|node| node.has_tag_name("tspan") && node.text() == Some("Calls"))
            .unwrap()
            .parent()
            .unwrap();
        assert_eq!(label.attribute("x"), Some(expected), "{arguments}");
        assert_eq!(label.attribute("y"), Some(expected), "{arguments}");
    }
}
