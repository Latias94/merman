use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

const CLUSTER_SOURCE: &str = "flowchart TD\nsubgraph Group[Group]\nA\nend\n";

fn cluster_theme(fill: CanvasPaint, stroke: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Cluster,
                        ThemeStylePatch::default()
                            .with_fill(fill)
                            .with_stroke(stroke),
                    )
                    .for_family(merman_render::DiagramFamilyId::FLOWCHART),
                ),
            ),
        )
        .expect("compile Flowchart Cluster theme")
}

fn render_strict(source: &str, theme: &DiagramTheme, site_config: MermaidConfig) -> String {
    let parsed = merman_render::__private::install_parse_compatibility(
        theme,
        Engine::new().with_site_config(site_config),
    )
    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
    .expect("parse Flowchart source")
    .expect("detect Flowchart source");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict themed session");
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Flowchart Cluster theme")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("cluster-theme".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render portable Flowchart Cluster theme")
        .svg()
        .to_string()
}

#[test]
fn flowchart_cluster_typed_paint_reaches_classic_and_hand_drawn_writers() {
    let theme = cluster_theme(
        CanvasPaint::solid("#ef4444").expect("valid fill"),
        CanvasPaint::solid("#2563eb").expect("valid stroke"),
    );

    for look in ["classic", "handDrawn"] {
        let svg = render_strict(
            CLUSTER_SOURCE,
            &theme,
            MermaidConfig::from_value(json!({
                "look": look,
                "handDrawnSeed": 7
            })),
        );

        if look == "classic" {
            assert!(svg.contains("<rect style="), "classic Cluster rect");
            assert!(
                svg.contains("fill:#ef4444 !important"),
                "typed Cluster fill must reach the classic rect writer"
            );
            assert!(
                svg.contains("stroke:#2563eb !important"),
                "typed Cluster stroke must reach the classic rect writer"
            );
        } else {
            assert!(svg.contains("<path "), "hand-drawn Cluster paths");
            assert!(
                svg.contains(r##"stroke="#ef4444""##)
                    && svg.contains("style=\"stroke:#ef4444 !important\""),
                "typed Cluster fill must reach the hand-drawn hachure writer"
            );
            assert!(
                svg.contains(r##"stroke="#2563eb""##)
                    && svg.contains("style=\"stroke:#2563eb !important\""),
                "typed Cluster stroke must reach the hand-drawn border writer"
            );
        }
    }
}

#[test]
fn flowchart_cluster_transparent_paint_is_terminal_and_portable() {
    let theme = cluster_theme(CanvasPaint::Transparent, CanvasPaint::Transparent);

    for look in ["classic", "handDrawn"] {
        let svg = render_strict(
            CLUSTER_SOURCE,
            &theme,
            MermaidConfig::from_value(json!({
                "look": look,
                "handDrawnSeed": 7
            })),
        );
        if look == "classic" {
            assert!(
                svg.contains("fill:none !important"),
                "transparent Cluster fill must reach the classic rect writer"
            );
            assert!(
                svg.contains("stroke:none !important"),
                "transparent Cluster stroke must reach the classic rect writer"
            );
        } else {
            assert!(
                svg.matches("style=\"stroke:none !important\"").count() >= 2,
                "transparent Cluster fill and stroke must reach both hand-drawn writers"
            );
        }
    }
}

#[test]
fn flowchart_cluster_source_and_config_paint_override_typed_values_per_facet() {
    let theme = cluster_theme(
        CanvasPaint::solid("#ef4444").expect("valid fill"),
        CanvasPaint::solid("#2563eb").expect("valid stroke"),
    );
    let class_svg = render_strict(
        "flowchart TD\nsubgraph Group[Group]\nA\nend\nclassDef source fill:#22c55e\nclass Group source\n",
        &theme,
        MermaidConfig::default(),
    );
    assert!(class_svg.contains("fill:#22c55e !important"));
    assert!(class_svg.contains("stroke:#2563eb !important"));
    assert!(!class_svg.contains("fill:#ef4444 !important"));

    let style_svg = render_strict(
        "flowchart TD\nsubgraph Group[Group]\nA\nend\nstyle Group stroke:#0f766e\n",
        &theme,
        MermaidConfig::default(),
    );
    assert!(style_svg.contains("fill:#ef4444 !important"));
    assert!(style_svg.contains("stroke:#0f766e !important"));
    assert!(!style_svg.contains("stroke:#2563eb !important"));

    let config_svg = render_strict(
        CLUSTER_SOURCE,
        &theme,
        MermaidConfig::from_value(json!({
            "themeVariables": {
                "clusterBkg": "#f59e0b",
                "clusterBorder": "#334155"
            }
        })),
    );
    assert!(config_svg.contains(".cluster rect{fill:#f59e0b;stroke:#334155;"));
    assert!(!config_svg.contains("fill:#ef4444 !important"));
    assert!(!config_svg.contains("stroke:#2563eb !important"));
}

#[test]
fn flowchart_cluster_hand_drawn_precedence_is_independent_per_paint_facet() {
    let theme = cluster_theme(
        CanvasPaint::solid("#ef4444").expect("valid fill"),
        CanvasPaint::solid("#2563eb").expect("valid stroke"),
    );
    let class_svg = render_strict(
        "flowchart TD\nsubgraph Group[Group]\nA\nend\nclassDef source fill:#22c55e\nclass Group source\n",
        &theme,
        MermaidConfig::from_value(json!({
            "look": "handDrawn",
            "handDrawnSeed": 7
        })),
    );
    assert!(class_svg.contains("style=\"stroke:#22c55e !important\""));
    assert!(class_svg.contains("style=\"stroke:#2563eb !important\""));
    assert!(!class_svg.contains("#ef4444"));

    let style_svg = render_strict(
        "flowchart TD\nsubgraph Group[Group]\nA\nend\nstyle Group stroke:#0f766e\n",
        &theme,
        MermaidConfig::from_value(json!({
            "look": "handDrawn",
            "handDrawnSeed": 7
        })),
    );
    assert!(style_svg.contains("style=\"stroke:#ef4444 !important\""));
    assert!(style_svg.contains("style=\"stroke:#0f766e !important\""));
    assert!(!style_svg.contains("#2563eb"));

    let fill_config_svg = render_strict(
        CLUSTER_SOURCE,
        &theme,
        MermaidConfig::from_value(json!({
            "look": "handDrawn",
            "handDrawnSeed": 7,
            "themeVariables": {
                "clusterBkg": "#f59e0b"
            }
        })),
    );
    assert!(fill_config_svg.contains(r##"stroke="#f59e0b""##));
    assert!(fill_config_svg.contains("style=\"stroke:#2563eb !important\""));
    assert!(!fill_config_svg.contains("#ef4444"));

    let stroke_config_svg = render_strict(
        CLUSTER_SOURCE,
        &theme,
        MermaidConfig::from_value(json!({
            "look": "handDrawn",
            "handDrawnSeed": 7,
            "themeVariables": {
                "clusterBorder": "#334155"
            }
        })),
    );
    assert!(stroke_config_svg.contains("style=\"stroke:#ef4444 !important\""));
    assert!(stroke_config_svg.contains(r##"stroke="#334155""##));
    assert!(!stroke_config_svg.contains("#2563eb"));
}

#[test]
fn swimlane_shared_renderer_does_not_consume_flowchart_only_cluster_paint() {
    let theme = cluster_theme(
        CanvasPaint::solid("#ef4444").expect("valid fill"),
        CanvasPaint::solid("#2563eb").expect("valid stroke"),
    );
    let source = "flowchart TD\nsubgraph Lane[Lane]\nsubgraph Group[Group]\nA\nend\nend\n";

    for look in ["classic", "handDrawn"] {
        let svg = render_strict(
            source,
            &theme,
            MermaidConfig::from_value(json!({
                "layout": "swimlane",
                "look": look,
                "handDrawnSeed": 7
            })),
        );

        assert!(svg.contains("Group"), "nested Swimlane cluster must render");
        assert!(
            !svg.contains("#ef4444") && !svg.contains("#2563eb"),
            "Swimlane must not consume Flowchart-only Cluster paint in {look} output"
        );
    }
}

#[test]
fn flowchart_cluster_theme_is_not_applicable_without_a_cluster() {
    let theme = cluster_theme(
        CanvasPaint::solid("#ef4444").expect("valid fill"),
        CanvasPaint::solid("#2563eb").expect("valid stroke"),
    );
    let svg = render_strict("flowchart LR\nA --> B\n", &theme, MermaidConfig::default());

    assert!(!svg.contains("fill:#ef4444 !important"));
    assert!(!svg.contains("stroke:#2563eb !important"));
}
