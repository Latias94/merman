use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

fn render_paths(source: &str, color: &str, seed: u32) -> Vec<String> {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "layout":"dagre", "theme":"default", "look":"neo", "htmlLabels":false,
        "handDrawnSeed":seed,
        "themeVariables": {
            "noteBkgColor":color, "noteBorderColor":color,
            "mainBkg":color, "nodeBorder":color, "lineColor":color,
            "requirementBackground":color, "requirementBorderColor":color
        }
    })));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse diagram")
        .expect("detect diagram");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let svg = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare diagram")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("paint colors must not reject rectangle geometry")
        .svg()
        .to_owned();
    let document = roxmltree::Document::parse(&svg).unwrap();
    let paths: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("path"))
        .filter_map(|node| node.attribute("d"))
        .map(str::to_owned)
        .collect();
    assert!(!paths.is_empty(), "expected painted paths: {source}");
    assert!(
        paths.iter().all(|path| path != "M0,0"),
        "placeholder geometry: {source}"
    );
    paths
}

#[test]
fn shared_rough_rectangles_keep_geometry_and_random_sequence_independent_of_css_colors() {
    for source in [
        "usecase-beta\nA(Login)\nnote for A \"Remember\"\nnote for A \"Again\"",
        "flowchart TB\nA@{ shape: note, label: \"Remember\" }\nB@{ shape: fork }\nA --> B",
        "stateDiagram-v2\nstate A\nnote right of A\nRemember\nend note",
        "requirementDiagram\nrequirement Req {\nid: 1\ntext: Remember\nrisk: high\nverifymethod: analysis\n}",
    ] {
        for seed in [0, 42] {
            let expected = render_paths(source, "#ff0000", seed);
            for color in [
                "rgb(255, 0, 0)",
                "hsl(0, 100%, 50%)",
                "red",
                "var(--note-color)",
            ] {
                assert_eq!(
                    render_paths(source, color, seed),
                    expected,
                    "{source}; color={color}; seed={seed}"
                );
            }
        }
    }
}

#[test]
fn rough_shape_geometry_accepts_css_color_syntax_without_placeholder_paths() {
    for source in [
        "flowchart TD\nA@{ shape: bolt }",
        "flowchart TD\nA@{ shape: person, label: Person }",
        "flowchart TD\nA@{ shape: div-rect, label: Divide }",
        "flowchart TD\nA@{ shape: brace, label: Brace }",
        "%%{init: {\"look\":\"handDrawn\"}}%%\nflowchart TD\nA[Card] --> B{Choice}",
        "block-beta\nA[\"Card\"] B((\"Circle\"))",
    ] {
        for seed in [0, 42] {
            let expected = render_paths(source, "#ff0000", seed);
            for color in [
                "red",
                "rgb(255, 0, 0)",
                "hsl(0, 100%, 50%)",
                "var(--node-color)",
            ] {
                assert_eq!(
                    render_paths(source, color, seed),
                    expected,
                    "{source}; color={color}; seed={seed}"
                );
            }
        }
    }
}

#[test]
fn rough_shape_geometry_never_slices_arbitrary_css_color_as_utf8_bytes() {
    for color in ["#😀ab", "#éa", "#猫", "#abcdefg", "#ggg"] {
        let paths = render_paths("flowchart TD\nA@{ shape: bolt }", color, 42);
        assert_eq!(
            paths,
            render_paths("flowchart TD\nA@{ shape: bolt }", "#ff0000", 42)
        );
    }
}
