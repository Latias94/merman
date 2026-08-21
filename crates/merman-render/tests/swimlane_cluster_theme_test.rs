use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{DiagramFamilyId, LayoutOptions};
use serde_json::json;

const SWIMLANE_SOURCE: &str = "flowchart TD\nsubgraph Lane[Lane]\nA\nend\n";

fn swimlane_cluster_theme(fill: CanvasPaint, stroke: CanvasPaint) -> DiagramTheme {
    compile_swimlane_cluster_theme(
        ThemeStylePatch::default()
            .with_fill(fill)
            .with_stroke(stroke),
    )
}

fn swimlane_cluster_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    compile_swimlane_cluster_theme(ThemeStylePatch::default().with_stroke(stroke))
}

fn compile_swimlane_cluster_theme(style: ThemeStylePatch) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                ThemeRule::new(ThemeTarget::Cluster, style).for_family(DiagramFamilyId::SWIMLANE),
            )),
        )
        .expect("compile Swimlane Cluster theme")
}

fn render_swimlane_cluster(
    source: &str,
    theme: &DiagramTheme,
    site_config: MermaidConfig,
) -> family::RenderedFamilySvg {
    let parsed = merman_render::__private::install_parse_compatibility(
        theme,
        Engine::new().with_site_config(site_config),
    )
    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
    .expect("parse themed Swimlane")
    .expect("detect themed Swimlane");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Swimlane session");

    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Swimlane")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("swimlane-cluster-theme".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Swimlane")
}

fn lane_rect_style(
    document: &roxmltree::Document<'_>,
    class_name: &str,
) -> (String, String, String) {
    let lane = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("Lane")
                && node.attribute("class").is_some_and(|classes| {
                    classes.split_whitespace().any(|class| class == "swimlane")
                })
        })
        .expect("Lane terminal group");
    let rect = lane
        .children()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some(class_name))
        .unwrap_or_else(|| panic!("Lane {class_name} terminal rect"));
    (
        rect.attribute("style").unwrap_or_default().to_string(),
        rect.attribute("fill").unwrap_or_default().to_string(),
        rect.attribute("stroke").unwrap_or_default().to_string(),
    )
}

fn stylesheet_text(document: &roxmltree::Document<'_>) -> String {
    document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Swimlane stylesheet")
        .to_string()
}

fn lane_hand_drawn_shell_strokes(document: &roxmltree::Document<'_>) -> Vec<Vec<String>> {
    let lane = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("Lane")
                && node.attribute("class").is_some_and(|classes| {
                    classes.split_whitespace().any(|class| class == "swimlane")
                })
        })
        .expect("Lane terminal group");

    lane.children()
        .filter(|node| node.has_tag_name("g") && node.attribute("class").is_none())
        .map(|shell| {
            shell
                .children()
                .filter(|node| node.has_tag_name("path"))
                .map(|path| path.attribute("stroke").unwrap_or_default().to_string())
                .collect()
        })
        .collect()
}

#[test]
fn swimlane_cluster_paint_reaches_lane_shell_and_terminal_receipt() {
    let theme = swimlane_cluster_theme(
        CanvasPaint::solid("#123456").expect("valid lane fill"),
        CanvasPaint::solid("#654321").expect("valid lane stroke"),
    );
    let rendered = render_swimlane_cluster(
        SWIMLANE_SOURCE,
        &theme,
        MermaidConfig::from_value(json!({"layout": "swimlane"})),
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Swimlane SVG");

    for class_name in ["swimlane-body", "swimlane-title"] {
        let (style, fill, stroke) = lane_rect_style(&document, class_name);
        assert!(
            style.contains("fill:#123456 !important"),
            "{class_name}: {style}"
        );
        assert!(
            style.contains("stroke:#654321 !important"),
            "{class_name}: {style}"
        );
        assert_eq!(fill, "#123456", "{class_name}");
        assert_eq!(stroke, "#654321", "{class_name}");
    }

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn swimlane_cluster_paint_reaches_both_hand_drawn_lane_surfaces() {
    let theme = swimlane_cluster_theme(
        CanvasPaint::solid("#123456").expect("valid lane fill"),
        CanvasPaint::solid("#654321").expect("valid lane stroke"),
    );
    let rendered = render_swimlane_cluster(
        SWIMLANE_SOURCE,
        &theme,
        MermaidConfig::from_value(json!({
            "layout": "swimlane",
            "look": "handDrawn",
            "handDrawnSeed": 7
        })),
    );

    assert_eq!(
        rendered
            .svg()
            .matches("style=\"stroke:#123456 !important\"")
            .count(),
        2,
        "body and title hachure paths must consume the typed fill"
    );
    assert_eq!(
        rendered
            .svg()
            .matches("style=\"stroke:#654321 !important\"")
            .count(),
        2,
        "body and title outline paths must consume the typed stroke"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn swimlane_cluster_stroke_only_preserves_the_compatibility_lane_fill() {
    let theme =
        swimlane_cluster_stroke_theme(CanvasPaint::solid("#654321").expect("valid lane stroke"));
    let rendered = render_swimlane_cluster(
        SWIMLANE_SOURCE,
        &theme,
        MermaidConfig::from_value(json!({"layout": "swimlane"})),
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Swimlane SVG");
    let stylesheet = stylesheet_text(&document);
    let (_, body_fill, _) = lane_rect_style(&document, "swimlane-body");
    let (_, title_fill, _) = lane_rect_style(&document, "swimlane-title");

    assert_eq!(body_fill, "none", "typed stroke must not claim body fill");
    assert_ne!(title_fill, "none", "compatibility fill must remain visible");
    assert!(
        stylesheet.contains(&format!(".cluster rect{{fill:{title_fill};")),
        "stroke-only typed paint must not remove the shared compatibility fill rule: {stylesheet}"
    );
    for class_name in ["swimlane-body", "swimlane-title"] {
        let (style, _, stroke) = lane_rect_style(&document, class_name);
        assert!(
            style.contains("stroke:#654321 !important"),
            "{class_name}: {style}"
        );
        assert_eq!(stroke, "#654321", "{class_name}");
    }

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn swimlane_hand_drawn_cluster_stroke_does_not_claim_or_change_fill() {
    let theme =
        swimlane_cluster_stroke_theme(CanvasPaint::solid("#654321").expect("valid lane stroke"));
    let rendered = render_swimlane_cluster(
        SWIMLANE_SOURCE,
        &theme,
        MermaidConfig::from_value(json!({
            "layout": "swimlane",
            "look": "handDrawn",
            "handDrawnSeed": 7,
            "themeVariables": {"clusterBkg": "#fef3c7"}
        })),
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Swimlane SVG");

    assert_eq!(
        lane_hand_drawn_shell_strokes(&document),
        vec![
            vec!["#654321".to_string()],
            vec!["#fef3c7".to_string(), "#654321".to_string()],
        ],
        "stroke-only typed paint must leave the body unfilled and preserve the title fill"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1, "only stroke was requested");
    assert_eq!(evidence.applied_count(), 1, "only stroke may be attributed");
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn swimlane_cluster_source_and_config_owners_outrank_typed_paint_per_facet() {
    let theme = swimlane_cluster_theme(
        CanvasPaint::solid("#123456").expect("valid lane fill"),
        CanvasPaint::solid("#654321").expect("valid lane stroke"),
    );
    let source_owned = render_swimlane_cluster(
        "flowchart TD\nsubgraph Lane[Lane]\nA\nend\nclassDef source fill:#22c55e\nclass Lane source\n",
        &theme,
        MermaidConfig::from_value(json!({"layout": "swimlane"})),
    );
    let source_document =
        roxmltree::Document::parse(source_owned.svg()).expect("valid source-owned Swimlane SVG");
    for class_name in ["swimlane-body", "swimlane-title"] {
        let (style, _, stroke) = lane_rect_style(&source_document, class_name);
        assert!(
            style.contains("fill:#22c55e !important"),
            "{class_name}: {style}"
        );
        assert!(!style.contains("#123456"), "{class_name}: {style}");
        assert!(
            style.contains("stroke:#654321 !important"),
            "{class_name}: {style}"
        );
        assert_eq!(stroke, "#654321", "{class_name}");
    }

    let config_owned = render_swimlane_cluster(
        SWIMLANE_SOURCE,
        &theme,
        MermaidConfig::from_value(json!({
            "layout": "swimlane",
            "themeVariables": {"clusterBorder": "#334155"}
        })),
    );
    let config_document =
        roxmltree::Document::parse(config_owned.svg()).expect("valid config-owned Swimlane SVG");
    for class_name in ["swimlane-body", "swimlane-title"] {
        let (style, fill, stroke) = lane_rect_style(&config_document, class_name);
        assert!(
            style.contains("fill:#123456 !important"),
            "{class_name}: {style}"
        );
        assert!(!style.contains("#654321"), "{class_name}: {style}");
        assert_eq!(fill, "#123456", "{class_name}");
        assert_eq!(stroke, "#334155", "{class_name}");
    }

    let config_fill_owned = render_swimlane_cluster(
        SWIMLANE_SOURCE,
        &theme,
        MermaidConfig::from_value(json!({
            "layout": "swimlane",
            "themeVariables": {"clusterBkg": "#fef3c7"}
        })),
    );
    let config_fill_document = roxmltree::Document::parse(config_fill_owned.svg())
        .expect("valid config-owned Swimlane SVG");
    let config_fill_stylesheet = stylesheet_text(&config_fill_document);
    assert!(
        config_fill_stylesheet.contains(".cluster rect{fill:#fef3c7;"),
        "config-owned fill must keep the compatibility rule when typed stroke owns only stroke: {config_fill_stylesheet}"
    );
    for class_name in ["swimlane-body", "swimlane-title"] {
        let (style, _, stroke) = lane_rect_style(&config_fill_document, class_name);
        assert!(!style.contains("#123456"), "{class_name}: {style}");
        assert!(
            style.contains("stroke:#654321 !important"),
            "{class_name}: {style}"
        );
        assert_eq!(stroke, "#654321", "{class_name}");
    }
}

#[test]
fn swimlane_cluster_transparent_paint_reaches_classic_lane_terminals() {
    let theme = swimlane_cluster_theme(CanvasPaint::Transparent, CanvasPaint::Transparent);
    let rendered = render_swimlane_cluster(
        SWIMLANE_SOURCE,
        &theme,
        MermaidConfig::from_value(json!({"layout": "swimlane"})),
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Swimlane SVG");

    for class_name in ["swimlane-body", "swimlane-title"] {
        let (style, fill, stroke) = lane_rect_style(&document, class_name);
        assert!(
            style.contains("fill:none !important"),
            "{class_name}: {style}"
        );
        assert!(
            style.contains("stroke:none !important"),
            "{class_name}: {style}"
        );
        assert_eq!(fill, "none", "{class_name}");
        assert_eq!(stroke, "none", "{class_name}");
    }
}
