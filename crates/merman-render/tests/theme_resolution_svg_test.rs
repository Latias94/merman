use merman_core::theme_color::{ColorChannel, ThemeColor};
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

fn quantized_rgba(value: &str) -> [u8; 4] {
    let color = ThemeColor::parse(value).expect("theme color should be valid");
    let channel = |kind| color.channel(kind).round().clamp(0.0, 255.0) as u8;
    [
        channel(ColorChannel::Red),
        channel(ColorChannel::Green),
        channel(ColorChannel::Blue),
        (color.channel(ColorChannel::Alpha) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8,
    ]
}

#[test]
fn font_only_theme_uses_one_resolved_palette_across_scale_consumers() {
    const EXPECTED_SCALE: &str = "hsl(240, 100%, 76.2745098039%)";
    let cases = vec![
        ("radar", "radar-beta\naxis A, B\ncurve sample{1, 2}\n", None),
        ("kanban", "kanban\n  Todo\n    item1\n", None),
        (
            "timeline",
            "timeline\n  section Release\n    Plan : Build\n",
            None,
        ),
        (
            "treemap",
            "treemap-beta\n\"Root\"\n  \"Child\": 1\n",
            Some("treemap.section.1.shape"),
        ),
    ];

    #[cfg(feature = "layout-cytoscape")]
    let cases = {
        let mut cases = cases;
        cases.push((
            "mindmap",
            "mindmap\n  root((Root))\n    child(Child)\n",
            None,
        ));
        cases
    };

    for (diagram_id, source, canonical_fill_resource) in cases {
        let (c_scale, svg) = render_with_font_only_theme(source, diagram_id);
        assert_eq!(c_scale, EXPECTED_SCALE, "diagram {diagram_id}");
        if let Some(resource) = canonical_fill_resource {
            let document = roxmltree::Document::parse(&svg)
                .expect("canonical palette consumer should emit valid XML");
            let rendered_fill = document
                .descendants()
                .find(|node| {
                    node.attribute("data-merman-resource") == Some(resource)
                        && node.attribute("fill").is_some_and(|fill| fill != "none")
                })
                .and_then(|node| node.attribute("fill"))
                .expect("canonical palette consumer should expose its fill resource");
            assert_eq!(
                quantized_rgba(rendered_fill),
                quantized_rgba(&c_scale),
                "diagram {diagram_id} must consume the resolved palette: {svg}"
            );
        } else {
            assert!(
                svg.contains(EXPECTED_SCALE),
                "diagram {diagram_id} must consume the resolved palette instead of deriving its own: {svg}"
            );
        }
    }
}
