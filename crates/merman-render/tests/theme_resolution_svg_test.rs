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

fn render_flowchart_label_weights(
    source: &str,
    theme: &merman_render::diagram_theme::DiagramTheme,
    html_labels: bool,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(
        theme,
        Engine::new().with_site_config(MermaidConfig::from_value(
            json!({"htmlLabels": html_labels}),
        )),
    )
    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
    .unwrap()
    .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(
            merman_render::diagram_theme::ThemePortabilityRequirement::RequirePortable,
        )
        .begin_session_with_theme(theme)
        .unwrap();
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn label_weight_theme(clear_nodes: bool) -> merman_render::diagram_theme::DiagramTheme {
    use merman_render::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, Specified, TextStylePatch, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget,
    };
    let rule = |target, weight| {
        ThemeRule::new(
            target,
            ThemeStylePatch {
                typography: TextStylePatch {
                    font_weight: weight,
                    ..TextStylePatch::default()
                },
                ..ThemeStylePatch::default()
            },
        )
    };
    let mut rules = ThemeRuleSet::default()
        .with_rule(rule(ThemeTarget::NodeLabel, Specified::Value(600)))
        .with_rule(rule(ThemeTarget::EdgeLabel, Specified::Value(500)));
    if clear_nodes {
        rules = rules.with_rule(rule(ThemeTarget::NodeLabel, Specified::Clear));
    }
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(rules))
        .unwrap()
}

fn explicit_label_weight(svg: &str, label: &str) -> String {
    let document = roxmltree::Document::parse(svg).unwrap();
    let text = document
        .descendants()
        .find(|node| node.is_text() && node.text() == Some(label))
        .unwrap_or_else(|| panic!("missing label {label}: {svg}"));
    for node in text.ancestors().filter(roxmltree::Node::is_element) {
        if let Some(style) = node.attribute("style") {
            for declaration in style.split(';').rev() {
                if let Some(("font-weight", value)) = declaration.trim().split_once(':') {
                    return value
                        .trim()
                        .trim_end_matches("!important")
                        .trim()
                        .to_owned();
                }
            }
        }
        if let Some(weight) = node.attribute("font-weight") {
            return weight.to_owned();
        }
    }
    panic!("no explicit weight for {label}: {svg}")
}

#[test]
fn flowchart_label_weights_reach_svg_and_preserve_source_and_clear() {
    for prefix in ["", "---\nconfig:\n  layout: swimlane\n---\n"] {
        for html_labels in [false, true] {
            for clear in [false, true] {
                let theme = label_weight_theme(clear);
                let source = format!(
                    "{prefix}flowchart LR\nA[NodeAlpha] -->|EdgeGamma| B[NodeBeta]\nstyle B font-weight:800"
                );
                let rendered = render_flowchart_label_weights(&source, &theme, html_labels)
                    .unwrap_or_else(|error| panic!("html={html_labels}, clear={clear}: {error}"));
                assert_eq!(
                    explicit_label_weight(rendered.svg(), "NodeAlpha"),
                    if clear { "400" } else { "600" }
                );
                assert_eq!(explicit_label_weight(rendered.svg(), "NodeBeta"), "800");
                assert_eq!(explicit_label_weight(rendered.svg(), "EdgeGamma"), "500");
                let completion = rendered.into_completion();
                assert_eq!(
                    merman_render::__private::family_evidence(completion.report())
                        .theme_residual_count(),
                    0
                );
            }
        }
    }
}

#[test]
fn flowchart_label_weight_rejects_unverified_content_and_preserves_relative_source() {
    let theme = label_weight_theme(false);
    for html in [false, true] {
        let source =
            "flowchart LR\nA[NodeAlpha] -->|EdgeGamma| B[NodeBeta]\nstyle B font-weight:bolder";
        let rendered = render_flowchart_label_weights(source, &theme, html).unwrap();
        assert_eq!(explicit_label_weight(rendered.svg(), "NodeBeta"), "900");
    }
    for source in [
        "flowchart LR\nA[NodeAlpha] -->|EdgeGamma| B[NodeBeta]\nstyle B font-weight:banana",
        "flowchart LR\nA[\"<strong>NodeAlpha</strong>\"] -->|EdgeGamma| B[NodeBeta]",
        "flowchart LR\nA[\"<span style='font-weight:700'>NodeAlpha</span>\"] -->|EdgeGamma| B[NodeBeta]",
        "flowchart LR\nA[NodeAlpha] -->|<b>EdgeGamma</b>| B[NodeBeta]",
        "flowchart LR\nA[\"<h1>NodeAlpha</h1>\"] -->|EdgeGamma| B[NodeBeta]",
        "flowchart LR\nA[\"<button>NodeAlpha</button>\"] -->|EdgeGamma| B[NodeBeta]",
        "flowchart LR\nA[\"<textarea>NodeAlpha</textarea>\"] -->|EdgeGamma| B[NodeBeta]",
        "flowchart LR\nA[NodeAlpha] -->|<button>EdgeGamma</button>| B[NodeBeta]",
        "flowchart LR\nA[NodeAlpha] -->|<select><option>EdgeGamma</option></select>| B[NodeBeta]",
    ] {
        assert!(
            render_flowchart_label_weights(source, &theme, true).is_err(),
            "{source}"
        );
    }
}

#[test]
fn flowchart_label_weight_config_and_class_precedence_reach_terminals() {
    let theme = label_weight_theme(false);
    for html in [false, true] {
        for (config, expected, relative) in [("900", "900", "700"), ("bolder", "700", "400")] {
            let source = format!(
                "---\nconfig:\n  fontWeight: '{config}'\n---\nflowchart LR\nA[NodeAlpha] -->|EdgeGamma| B[NodeBeta]\nclassDef emphasis font-weight:800!important\nclass B emphasis\nstyle B font-weight:lighter!important"
            );
            let rendered = render_flowchart_label_weights(&source, &theme, html).unwrap();
            assert_eq!(explicit_label_weight(rendered.svg(), "NodeAlpha"), expected);
            assert_eq!(explicit_label_weight(rendered.svg(), "EdgeGamma"), expected);
            assert_eq!(explicit_label_weight(rendered.svg(), "NodeBeta"), relative);
        }
        let invalid = "---\nconfig:\n  fontWeight: banana\n---\nflowchart LR\nA[NodeAlpha] -->|EdgeGamma| B[NodeBeta]";
        assert!(render_flowchart_label_weights(invalid, &theme, html).is_err());
        let invalid_source = "---\nconfig:\n  fontWeight: '900'\n---\nflowchart LR\nA[NodeAlpha] -->|EdgeGamma| B[NodeBeta]\nstyle B font-weight:banana";
        assert!(render_flowchart_label_weights(invalid_source, &theme, html).is_err());
    }
}
