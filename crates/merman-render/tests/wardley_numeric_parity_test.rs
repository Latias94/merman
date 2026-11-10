#![cfg(feature = "diagram-wardley")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::Value;

fn render_wardley(source: &str) -> (Value, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("Wardley annotation source parses")
        .expect("Wardley is detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic Wardley render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("Wardley family prepares");
    let projection = artifact.layout_json().expect("Wardley layout JSON");
    let layout = projection["layout"]["WardleyDiagram"].clone();
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("wardley-numeric-parity".into()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("Wardley annotations render as SVG");
    (layout, rendered.svg().to_owned())
}

fn text_in_group(svg: &str, class: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(svg).expect("valid Wardley SVG XML");
    document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text")
                && node
                    .parent()
                    .is_some_and(|parent| parent.attribute("class") == Some(class))
        })
        .map(|node| node.text().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn component_and_pipeline_label_offsets_reach_svg_as_javascript_numbers() {
    for (token, expected_x, expected_y) in [
        (
            "9007199254740993".to_string(),
            "9007199254741442",
            "-9007199254740692",
        ),
        (
            "18446744073709551616".into(),
            "18446744073709552000",
            "-18446744073709552000",
        ),
        ("1000000000000000000000".into(), "1e+21", "-1e+21"),
        ("9".repeat(309), "Infinity", "-Infinity"),
    ] {
        let source =
            format!("wardley-beta\ncomponent Regular [0.5, 0.5] label [{token}, - {token}]\n");
        let (_, svg) = render_wardley(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = document
            .descendants()
            .find(|node| {
                node.attribute("class") == Some("wardley-node-label")
                    && node.text() == Some("Regular")
            })
            .unwrap();
        assert_eq!(label.attribute("x"), Some(expected_x));
        assert_eq!(label.attribute("y"), Some(expected_y));
    }
    let (_, svg) = render_wardley(concat!(
        "wardley-beta\ncomponent Parent [0.5, 0.5]\npipeline Parent {\n",
        "component Child [0.25] label [-18446744073709551616, 18446744073709551616]\n}\n",
    ));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let label = document
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("wardley-node-label") && node.text() == Some("Child")
        })
        .unwrap();
    assert_eq!(label.attribute("x"), Some("-18446744073709552000"));
    assert_eq!(label.attribute("y"), Some("18446744073709552000"));
}

#[test]
fn annotation_points_and_box_lines_use_javascript_number_display() {
    for (token, expected) in [
        ("0".to_string(), "0"),
        ("9007199254740993".into(), "9007199254740992"),
        ("18446744073709551615".into(), "18446744073709552000"),
        ("18446744073709551616".into(), "18446744073709552000"),
        ("18446744073709555712".into(), "18446744073709556000"),
        ("1000000000000000128".into(), "1000000000000000100"),
        ("100000000000000000000".into(), "100000000000000000000"),
        ("1000000000000000000000".into(), "1e+21"),
        (
            format!("17976931348623157{}", "0".repeat(292)),
            "1.7976931348623157e+308",
        ),
        ("9".repeat(308), "1e+308"),
        ("9".repeat(309), "Infinity"),
    ] {
        let source = format!(
            "wardley-beta\nannotations [0.1, 0.1]\nannotation {token}, [0.5, 0.5] \"x\"\n",
        );
        let (layout, svg) = render_wardley(&source);
        let point = &layout["annotations"][0]["points"][0];
        assert_eq!(point["label"]["text"], expected, "{token}");
        assert_eq!(point["center"]["x"].as_f64(), Some(450.0));
        assert_eq!(point["center"]["y"].as_f64(), Some(300.0));
        let line = format!("{expected}. x");
        assert_eq!(layout["annotations_box"]["lines"][0]["text"], line);
        assert_eq!(text_in_group(&svg, "wardley-annotation"), [expected]);
        assert_eq!(text_in_group(&svg, "wardley-annotations-box"), [line]);
        let number = &layout["annotations"][0]["number"];
        if expected == "Infinity" {
            assert!(
                number.is_null(),
                "JSON.stringify maps overflowing numbers to null",
            );
        } else {
            assert_eq!(number.as_f64(), Some(token.parse::<f64>().unwrap()));
        }
    }
}

#[test]
fn annotation_box_sorting_preserves_numeric_order_and_stable_number_ties() {
    let mut source = "wardley-beta\nannotations [0.1, 0.1]\n".to_string();
    for (token, text) in [
        ("18446744073709555712".to_string(), "larger"),
        ("18446744073709551616".into(), "smaller"),
        ("9007199254740993".into(), "rounded-first"),
        ("9007199254740992".into(), "rounded-second"),
        ("9".repeat(309), "overflow-first"),
        (format!("1{}", "0".repeat(309)), "overflow-second"),
        ("1".into(), "ordinary"),
        ("2".into(), ""),
    ] {
        source.push_str(&format!("annotation {token}, [0.5, 0.5] \"{text}\"\n"));
    }
    let (layout, svg) = render_wardley(&source);
    let expected_points = [
        "18446744073709556000",
        "18446744073709552000",
        "9007199254740992",
        "9007199254740992",
        "Infinity",
        "Infinity",
        "1",
        "2",
    ];
    let expected_lines = [
        "1. ordinary",
        "9007199254740992. rounded-first",
        "9007199254740992. rounded-second",
        "18446744073709552000. smaller",
        "18446744073709556000. larger",
        "Infinity. overflow-first",
        "Infinity. overflow-second",
    ];
    let points: Vec<_> = layout["annotations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|annotation| annotation["points"][0]["label"]["text"].as_str().unwrap())
        .collect();
    let lines: Vec<_> = layout["annotations_box"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line["text"].as_str().unwrap())
        .collect();
    assert_eq!(
        points, expected_points,
        "point order remains authored order"
    );
    assert_eq!(
        lines, expected_lines,
        "box order follows the upstream stable Number sort",
    );
    assert_eq!(text_in_group(&svg, "wardley-annotation"), expected_points);
    assert_eq!(
        text_in_group(&svg, "wardley-annotations-box"),
        expected_lines
    );
    assert_eq!(
        layout["annotations"][0]["number"].as_f64(),
        Some(18_446_744_073_709_555_712.0),
    );
    assert_eq!(
        layout["annotations"][1]["number"].as_f64(),
        Some(18_446_744_073_709_551_616.0),
    );
}
