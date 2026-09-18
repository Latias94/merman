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
    let (svg, config) = render_with_engine(&engine, source, diagram_id);
    let c_scale = config
        .get_str("themeVariables.cScale0")
        .expect("resolved cScale0")
        .to_string();
    (c_scale, svg)
}

fn render_with_engine(engine: &Engine, source: &str, diagram_id: &str) -> (String, MermaidConfig) {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse succeeds")
        .expect("diagram is detected");
    let config = parsed.metadata().effective_config.clone();
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
    (svg, config)
}

#[test]
fn base_theme_source_and_host_overrides_reach_family_svg_styles() {
    // Mermaid 11.17.2 theme-base updateColors() derives all four families from these inputs.
    let config = json!({
        "theme": "base",
        "themeVariables": { "primaryColor": "#181818", "lineColor": "#a0a0a0" }
    });
    let cases = [
        (
            "sequence",
            "sequenceDiagram\nAlice->>Bob: hi\n",
            ".actor{stroke:hsl(0, 0%, 0%);fill:#181818;",
        ),
        (
            "state",
            "stateDiagram-v2\n[*] --> Idle: start\nIdle --> Busy: work\n",
            ".node rect{fill:#181818;stroke:hsl(0, 0%, 0%);",
        ),
        (
            "gantt",
            "gantt\ndateFormat YYYY-MM-DD\nsection Work\nTask :a, 2026-01-01, 1d\n",
            ".task3{fill:#181818;stroke:hsl(0, 0%, 0%);",
        ),
        ("pie", "pie\n\"A\" : 1\n\"B\" : 2\n", ""),
    ];
    for (family, body, expected_style) in cases {
        for (mode, engine, source) in [
            (
                "site",
                Engine::new().with_site_config(MermaidConfig::from_value(config.clone())),
                body.to_string(),
            ),
            (
                "init",
                Engine::new(),
                format!("%%{{init: {config}}}%%\n{body}"),
            ),
            (
                "frontmatter",
                Engine::new(),
                format!("---\nconfig: {config}\n---\n{body}"),
            ),
        ] {
            let (svg, effective) = render_with_engine(&engine, &source, family);
            assert_eq!(
                effective.get_str("themeVariables.primaryColor"),
                Some("#181818"),
                "{family}/{mode}: source theme must reach calculation"
            );
            if family == "pie" {
                let document = roxmltree::Document::parse(&svg).expect("valid SVG");
                let first_slice = document
                    .descendants()
                    .find(|node| node.attribute("class") == Some("pieCircle"))
                    .expect("first pie slice");
                assert_eq!(first_slice.attribute("fill"), Some("#181818"), "{mode}");
            } else {
                assert!(svg.contains(expected_style), "{family}/{mode}: {svg}");
            }
            if family == "state" {
                assert!(svg.contains(".transition{stroke:#a0a0a0;"), "{mode}: {svg}");
            }
        }
    }
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

    #[cfg(feature = "layout-cytoscape")]
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
