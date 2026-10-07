use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, OrdinalSelector,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

const CLUSTER_SOURCE: &str = "flowchart TD\nsubgraph Group[Group]\nA\nend\n";

fn cluster_theme(fill: CanvasPaint, stroke: CanvasPaint) -> DiagramTheme {
    cluster_theme_with_variant(None, fill, stroke)
}

fn cluster_theme_with_variant(
    variant: Option<merman_render::diagram_theme::ThemeVariant>,
    fill: CanvasPaint,
    stroke: CanvasPaint,
) -> DiagramTheme {
    let mut fill_rule = ThemeRule::new(
        ThemeTarget::Cluster,
        ThemeStylePatch::default().with_fill(fill),
    );
    let mut stroke_rule = ThemeRule::new(
        ThemeTarget::Cluster,
        ThemeStylePatch::default().with_stroke(stroke),
    );
    if let Some(variant) = variant {
        fill_rule = fill_rule.with_variant(variant);
        stroke_rule = stroke_rule.with_variant(variant);
    }
    fill_rule = fill_rule.for_family(merman_render::DiagramFamilyId::FLOWCHART);
    stroke_rule = stroke_rule.for_family(merman_render::DiagramFamilyId::FLOWCHART);

    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(fill_rule)
                    .with_rule(stroke_rule),
            ),
        )
        .expect("compile Flowchart Cluster theme")
}

fn cluster_ordinal_theme(ordinal: usize) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Cluster,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid("#a855f7").expect("valid ordinal Cluster fill"),
                        ),
                    )
                    .for_family(merman_render::DiagramFamilyId::FLOWCHART)
                    .with_ordinal(OrdinalSelector::exact(ordinal).expect("valid Cluster ordinal")),
                ),
            ),
        )
        .expect("compile ordinal Flowchart Cluster theme")
}

fn cluster_static_then_ordinal_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Cluster,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#ef4444").expect("valid static Cluster fill"),
                            ),
                        )
                        .for_family(merman_render::DiagramFamilyId::FLOWCHART),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Cluster,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#a855f7").expect("valid ordinal Cluster fill"),
                            ),
                        )
                        .for_family(merman_render::DiagramFamilyId::FLOWCHART)
                        .with_ordinal(
                            OrdinalSelector::exact(1).expect("valid first Cluster ordinal"),
                        ),
                    ),
            ),
        )
        .expect("compile mixed Flowchart Cluster theme")
}

fn try_render(
    source: &str,
    theme: &DiagramTheme,
    site_config: MermaidConfig,
    requirement: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(
        theme,
        Engine::new().with_site_config(site_config),
    )
    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
    .expect("parse Flowchart source")
    .expect("detect Flowchart source");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(requirement)
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
}

fn render_strict(source: &str, theme: &DiagramTheme, site_config: MermaidConfig) -> String {
    try_render(
        source,
        theme,
        site_config,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render portable Flowchart Cluster theme")
    .svg()
    .to_string()
}

#[test]
fn flowchart_cluster_ordinal_routes_follow_visible_terminal_occurrences() {
    let matching = cluster_ordinal_theme(1);
    let rendered = try_render(
        CLUSTER_SOURCE,
        &matching,
        MermaidConfig::default(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("render matching ordinal Cluster theme in best-effort mode");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert!(
        try_render(
            CLUSTER_SOURCE,
            &matching,
            MermaidConfig::default(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .is_err(),
        "a visible unsupported Cluster ordinal must fail closed"
    );

    let out_of_range = cluster_ordinal_theme(2);
    let rendered = try_render(
        CLUSTER_SOURCE,
        &out_of_range,
        MermaidConfig::default(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("an out-of-range Cluster ordinal is not applicable");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn flowchart_cluster_ordinal_winner_suppresses_the_shadowed_static_paint() {
    let theme = cluster_static_then_ordinal_theme();
    let rendered = try_render(
        CLUSTER_SOURCE,
        &theme,
        MermaidConfig::default(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("render mixed Cluster winner theme");

    assert!(
        !rendered.svg().contains("fill:#ef4444 !important"),
        "the shadowed static fill must not reach the ordinal occurrence"
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
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
fn flowchart_cluster_explicit_default_paint_reaches_terminal_writers_and_evidence() {
    for (fill, stroke, expected_fill, expected_stroke) in [
        (
            CanvasPaint::solid("#ef4444").expect("valid explicit-Default fill"),
            CanvasPaint::solid("#2563eb").expect("valid explicit-Default stroke"),
            "#ef4444",
            "#2563eb",
        ),
        (
            CanvasPaint::Transparent,
            CanvasPaint::Transparent,
            "none",
            "none",
        ),
    ] {
        let theme = cluster_theme_with_variant(
            Some(merman_render::diagram_theme::ThemeVariant::Default),
            fill,
            stroke,
        );

        for look in ["classic", "handDrawn"] {
            let rendered = try_render(
                CLUSTER_SOURCE,
                &theme,
                MermaidConfig::from_value(json!({
                    "look": look,
                    "handDrawnSeed": 7
                })),
                ThemePortabilityRequirement::RequirePortable,
            )
            .expect("render explicitly default-themed Flowchart Cluster");

            if look == "classic" {
                let document = roxmltree::Document::parse(rendered.svg())
                    .expect("valid explicitly default-themed Flowchart SVG");
                let cluster = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("rect")
                            && node.attribute("style").is_some_and(|style| {
                                style.contains(&format!("fill:{expected_fill} !important"))
                            })
                    })
                    .expect("explicitly default-themed Cluster rect");
                let style = cluster.attribute("style").expect("Cluster rect style");
                assert!(
                    style.contains(&format!("stroke:{expected_stroke} !important")),
                    "Cluster rect style: {style}"
                );
            } else {
                assert!(
                    rendered
                        .svg()
                        .contains(&format!("style=\"stroke:{expected_fill} !important\"")),
                    "explicitly default-themed Cluster fill must reach hachure writer"
                );
                assert!(
                    rendered
                        .svg()
                        .contains(&format!("style=\"stroke:{expected_stroke} !important\"")),
                    "explicitly default-themed Cluster stroke must reach border writer"
                );
            }

            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2, "look={look}");
            assert_eq!(evidence.accounted_count(), 2, "look={look}");
            assert_eq!(evidence.applied_count(), 2, "look={look}");
            assert_eq!(evidence.not_applicable_count(), 0, "look={look}");
            assert_eq!(evidence.theme_residual_count(), 0, "look={look}");
            assert_eq!(evidence.compatibility_residual_count(), 0, "look={look}");
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

#[test]
fn cluster_stroke_width_reaches_rect_and_rough_outline_with_source_precedence() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default().with_stroke_width(3.0).unwrap(),
            ))),
        )
        .unwrap();
    for layout in ["dagre", "swimlane"] {
        for look in ["classic", "neo", "handDrawn"] {
            for (source_style, expected) in [("", "3px"), ("style Group stroke-width:0px\n", "0px")]
            {
                let rendered = try_render(
                    &format!("{CLUSTER_SOURCE}{source_style}"),
                    &theme,
                    MermaidConfig::from_value(json!({
                        "layout": layout,
                        "look": look,
                        "flowchart": {"look": look},
                        "handDrawnSeed": 7,
                    })),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .unwrap_or_else(|error| panic!("{layout}/{look}/{source_style}: {error}"));
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let cluster = document
                    .descendants()
                    .find(|node| {
                        node.attribute("data-et") == Some("cluster")
                            || node.attribute("class").is_some_and(|classes| {
                                classes.split_whitespace().any(|class| class == "swimlane")
                            })
                    })
                    .expect("visible cluster");
                let outlines = cluster
                    .descendants()
                    .filter(|node| {
                        (node.has_tag_name("rect") || node.has_tag_name("path"))
                            && node
                                .attribute("style")
                                .is_some_and(|style| style.contains("stroke-width:"))
                    })
                    .collect::<Vec<_>>();
                assert!(
                    !outlines.is_empty(),
                    "{layout}/{look}: emitted width terminal"
                );
                for outline in outlines {
                    assert!(
                        outline
                            .attribute("style")
                            .unwrap()
                            .contains(&format!("stroke-width:{expected} !important")),
                        "{layout}/{look}: {outline:?}"
                    );
                    if look == "handDrawn" {
                        assert_eq!(
                            outline.attribute("stroke-width"),
                            Some(if source_style.is_empty() { "3" } else { "0" })
                        );
                    }
                }
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.theme_residual_count(), 0, "{layout}/{look}");
                assert_eq!(
                    evidence.applied_count(),
                    usize::from(source_style.is_empty()),
                    "{layout}/{look}"
                );
            }
        }
    }
}

#[test]
fn cluster_zero_width_is_applied_without_claiming_absent_or_agentflow_terminals() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default().with_stroke_width(0.0).unwrap(),
            ))),
        )
        .unwrap();
    for look in ["classic", "neo", "handDrawn"] {
        let rendered = try_render(
            CLUSTER_SOURCE,
            &theme,
            MermaidConfig::from_value(json!({"look": look, "flowchart": {"look": look}})),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        assert!(rendered.svg().contains("stroke-width:0px !important"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
    let absent = try_render(
        "flowchart LR\nA --> B\n",
        &theme,
        MermaidConfig::default(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    let evidence = merman_render::__private::family_evidence(absent.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);

    let source = "agentflow-beta LR\nflow group[Group]\na[Alpha] --> b[Beta]\nend\n";
    let config = MermaidConfig::from_value(json!({"look": "classic"}));
    let unrelated = try_render(
        source,
        &theme,
        config,
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    assert!(!unrelated.svg().contains("stroke-width:0px !important"));
    let evidence = merman_render::__private::family_evidence(unrelated.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    // Agentflow shares a shape writer, but Cluster is outside its theme target domain.
    assert_eq!(evidence.required_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}
