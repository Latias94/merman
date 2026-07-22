use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

fn render_with_font_only_theme(source: &str, diagram_id: &str) -> (String, String) {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "theme": "default",
        "themeVariables": {
            "fontFamily": "Inter, sans-serif"
        }
    })));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse succeeds")
        .expect("diagram is detected");
    let c_scale = parsed
        .metadata()
        .effective_config
        .get_str("themeVariables.cScale0")
        .expect("resolved cScale0")
        .to_string();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("layout succeeds");
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("SVG render succeeds")
        .svg()
        .to_owned();
    (c_scale, svg)
}

#[test]
fn font_only_theme_uses_one_resolved_palette_across_scale_consumers() {
    const EXPECTED_SCALE: &str = "hsl(240, 100%, 76.2745098039%)";
    let cases = vec![
        ("radar", "radar-beta\naxis A, B\ncurve sample{1, 2}\n"),
        ("kanban", "kanban\n  Todo\n    item1\n"),
        (
            "timeline",
            "timeline\n  section Release\n    Plan : Build\n",
        ),
        ("treemap", "treemap-beta\n\"Root\"\n  \"Child\": 1\n"),
    ];

    #[cfg(feature = "cytoscape-layout")]
    let cases = {
        let mut cases = cases;
        cases.push(("mindmap", "mindmap\n  root((Root))\n    child(Child)\n"));
        cases
    };

    for (diagram_id, source) in cases {
        let (c_scale, svg) = render_with_font_only_theme(source, diagram_id);
        assert_eq!(c_scale, EXPECTED_SCALE, "diagram {diagram_id}");
        assert!(
            svg.contains(EXPECTED_SCALE),
            "diagram {diagram_id} must consume the resolved palette instead of deriving its own: {svg}"
        );
    }
}
