use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, GradientStop,
    LinearGradient, OrdinalSelector, Specified, ThemeColorValue, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

const ONE: &str = "flowchart LR\nA[Alpha] --> B[Beta]\n";
const TWO: &str = "flowchart LR\nA[Alpha] --> B[Beta] --> C[Gamma]\n";

fn fill(paint: CanvasPaint) -> ThemeRule {
    ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_fill(paint),
    )
}

fn compile(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                rules
                    .into_iter()
                    .fold(ThemeRuleSet::default(), |set, rule| set.with_rule(rule)),
            ),
        )
        .unwrap()
}

fn config(swimlane: bool, look: &str) -> MermaidConfig {
    let mut config = json!({"look": look, "htmlLabels": false});
    if swimlane {
        config["layout"] = json!("swimlane");
    }
    MermaidConfig::from_value(config)
}

fn render(
    source: &str,
    theme: &DiagramTheme,
    config: MermaidConfig,
    requirement: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(
        theme,
        Engine::new().with_site_config(config),
    )
    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
    .unwrap()
    .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(requirement)
        .begin_session_with_theme(theme)
        .unwrap();
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("edge-fill-theme".to_owned()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
}

fn assert_edge_stroke(svg: &str, stroke: &str, edge_count: usize) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let edges: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .collect();
    assert_eq!(edges.len(), edge_count);
    for edge in edges {
        assert!(edge.attribute("d").is_some_and(|path| !path.is_empty()));
        assert!(
            edge.attribute("style")
                .unwrap_or("")
                .contains(&format!("stroke:{stroke} !important")),
            "{svg}"
        );
    }
}

fn assert_evidence(
    rendered: family::RenderedFamilySvg,
    applied: usize,
    absent: usize,
    residual: usize,
) {
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), applied + absent + residual);
    assert_eq!(evidence.applied_count(), applied);
    assert_eq!(evidence.not_applicable_count(), absent);
    assert_eq!(evidence.theme_residual_count(), residual);
}

fn gradient() -> CanvasPaint {
    CanvasPaint::LinearGradient(
        LinearGradient::new(
            90.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#123456").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#abcdef").unwrap()).unwrap(),
            ],
        )
        .unwrap(),
    )
}

#[test]
fn edge_fill_uses_the_existing_stroke_path_for_static_solid_and_transparent_selectors() {
    for swimlane in [false, true] {
        for look in ["classic", "neo", "handDrawn"] {
            for explicit_default in [false, true] {
                for transparent in [false, true] {
                    let paint = if transparent {
                        CanvasPaint::Transparent
                    } else {
                        CanvasPaint::solid("#ca3579").unwrap()
                    };
                    let mut rule = fill(paint);
                    if explicit_default {
                        rule = rule.with_variant(ThemeVariant::Default);
                    }
                    let rendered = render(
                        ONE,
                        &compile([rule]),
                        config(swimlane, look),
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .unwrap();
                    assert_edge_stroke(
                        rendered.svg(),
                        if transparent { "none" } else { "#ca3579" },
                        1,
                    );
                    assert_evidence(rendered, 1, 0, 0);
                }
            }
        }
    }
}

#[test]
fn edge_stroke_suppresses_fill_in_combined_and_split_rules_regardless_of_author_order() {
    for swimlane in [false, true] {
        for combined in [false, true] {
            for stroke_first in [false, true] {
                for transparent in [false, true] {
                    let stroke = if transparent {
                        CanvasPaint::Transparent
                    } else {
                        CanvasPaint::solid("#246801").unwrap()
                    };
                    let fill_paint = CanvasPaint::solid("#ca3579").unwrap();
                    let mut rules = if combined {
                        vec![ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch::default()
                                .with_fill(fill_paint)
                                .with_stroke(stroke),
                        )]
                    } else {
                        vec![
                            fill(fill_paint),
                            ThemeRule::new(
                                ThemeTarget::Edge,
                                ThemeStylePatch::default().with_stroke(stroke),
                            ),
                        ]
                    };
                    if stroke_first {
                        rules.reverse();
                    }
                    let rendered = render(
                        ONE,
                        &compile(rules),
                        config(swimlane, "classic"),
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .unwrap();
                    assert_edge_stroke(
                        rendered.svg(),
                        if transparent { "none" } else { "#246801" },
                        1,
                    );
                    assert_evidence(rendered, 1, usize::from(!combined), 0);
                }
            }
        }
    }
}

#[test]
fn clear_or_unsupported_stroke_blocks_a_valid_fill_fallback() {
    for swimlane in [false, true] {
        for combined in [false, true] {
            for clear in [false, true] {
                let mut stroke = ThemeStylePatch::default();
                stroke.stroke.paint = if clear {
                    Specified::Clear
                } else {
                    Specified::Value(gradient())
                };
                let fill_paint = CanvasPaint::solid("#ca3579").unwrap();
                let rules = if combined {
                    vec![ThemeRule::new(
                        ThemeTarget::Edge,
                        stroke.with_fill(fill_paint),
                    )]
                } else {
                    vec![fill(fill_paint), ThemeRule::new(ThemeTarget::Edge, stroke)]
                };
                let theme = compile(rules);
                let rendered = render(
                    ONE,
                    &theme,
                    config(swimlane, "classic"),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                assert!(!rendered.svg().contains("stroke:#ca3579 !important"));
                assert_evidence(rendered, 0, usize::from(!combined), 1);
                assert!(
                    render(
                        ONE,
                        &theme,
                        config(swimlane, "classic"),
                        ThemePortabilityRequirement::RequirePortable
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn invalid_fill_values_are_residual_only_when_the_stroke_terminal_selects_them() {
    for swimlane in [false, true] {
        for clear in [false, true] {
            for stroke_wins in [false, true] {
                let mut patch = ThemeStylePatch::default();
                patch.paint.fill = if clear {
                    Specified::Clear
                } else {
                    Specified::Value(gradient())
                };
                let mut rules = vec![ThemeRule::new(ThemeTarget::Edge, patch)];
                if stroke_wins {
                    rules.push(ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#246801").unwrap()),
                    ));
                }
                let theme = compile(rules);
                let rendered = render(
                    ONE,
                    &theme,
                    config(swimlane, "classic"),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                if stroke_wins {
                    assert_edge_stroke(rendered.svg(), "#246801", 1);
                }
                assert_evidence(
                    rendered,
                    usize::from(stroke_wins),
                    usize::from(stroke_wins),
                    usize::from(!stroke_wins),
                );
                assert_eq!(
                    render(
                        ONE,
                        &theme,
                        config(swimlane, "classic"),
                        ThemePortabilityRequirement::RequirePortable
                    )
                    .is_err(),
                    !stroke_wins
                );
            }
        }
    }
}

#[test]
fn edge_fill_does_not_hide_an_unconsumed_sibling_facet() {
    for swimlane in [false, true] {
        for combined in [false, true] {
            for stroke_wins in [false, true] {
                let mut opacity = ThemeStylePatch::default();
                opacity.paint.opacity = Specified::Value(0.4);
                let paint = CanvasPaint::solid("#ca3579").unwrap();
                let mut rules = if combined {
                    vec![ThemeRule::new(ThemeTarget::Edge, opacity.with_fill(paint))]
                } else {
                    vec![fill(paint), ThemeRule::new(ThemeTarget::Edge, opacity)]
                };
                if stroke_wins {
                    rules.push(ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#246801").unwrap()),
                    ));
                }
                let theme = compile(rules);
                let rendered = render(
                    ONE,
                    &theme,
                    config(swimlane, "classic"),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                assert_edge_stroke(
                    rendered.svg(),
                    if stroke_wins { "#246801" } else { "#ca3579" },
                    1,
                );
                assert_evidence(
                    rendered,
                    usize::from(stroke_wins || !combined),
                    usize::from(stroke_wins && !combined),
                    1,
                );
                assert!(
                    render(
                        ONE,
                        &theme,
                        config(swimlane, "classic"),
                        ThemePortabilityRequirement::RequirePortable
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn edge_fill_ordinals_are_reconciled_against_the_actual_stroke_owner_per_edge() {
    for swimlane in [false, true] {
        for stroke_wins in [false, true] {
            for (ordinal, matches) in [(1, true), (2, true), (3, false)] {
                let mut rules = vec![
                    fill(CanvasPaint::solid("#ca3579").unwrap())
                        .with_ordinal(OrdinalSelector::exact(ordinal).unwrap()),
                ];
                if stroke_wins {
                    rules.push(ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#246801").unwrap()),
                    ));
                }
                let theme = compile(rules);
                let rendered = render(
                    TWO,
                    &theme,
                    config(swimlane, "classic"),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                if stroke_wins {
                    assert_edge_stroke(rendered.svg(), "#246801", 2);
                }
                let residual = matches && !stroke_wins;
                assert_evidence(
                    rendered,
                    usize::from(stroke_wins),
                    usize::from(!residual),
                    usize::from(residual),
                );
                assert_eq!(
                    render(
                        TWO,
                        &theme,
                        config(swimlane, "classic"),
                        ThemePortabilityRequirement::RequirePortable
                    )
                    .is_err(),
                    residual
                );
            }
        }
    }
}

#[test]
fn ordinal_stroke_blocks_static_fill_only_on_its_matching_edge() {
    for swimlane in [false, true] {
        for (source, ordinal, applied, absent, residual) in [
            (ONE, 1, 0, 1, 1),
            (TWO, 1, 1, 0, 1),
            (TWO, 2, 1, 0, 1),
            (TWO, 3, 1, 1, 0),
        ] {
            let theme = compile([
                fill(CanvasPaint::solid("#ca3579").unwrap()),
                ThemeRule::new(
                    ThemeTarget::Edge,
                    ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#246801").unwrap()),
                )
                .with_ordinal(OrdinalSelector::exact(ordinal).unwrap()),
            ]);
            let rendered = render(
                source,
                &theme,
                config(swimlane, "classic"),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            // The unchanged writer can emit a static fallback, but cannot certify an ordinal winner.
            assert_evidence(rendered, applied, absent, residual);
            assert_eq!(
                render(
                    source,
                    &theme,
                    config(swimlane, "classic"),
                    ThemePortabilityRequirement::RequirePortable
                )
                .is_err(),
                residual != 0
            );
        }
    }
}

#[test]
fn edge_fill_uses_stroke_source_and_config_ownership() {
    let theme = compile([fill(CanvasPaint::solid("#ca3579").unwrap())]);
    for swimlane in [false, true] {
        let source = format!("{ONE}linkStyle 0 stroke:#246801\n");
        let rendered = render(
            &source,
            &theme,
            config(swimlane, "classic"),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        assert!(!rendered.svg().contains("stroke:#ca3579 !important"));
        assert_evidence(rendered, 0, 1, 0);
        for config_path in ["lineColor", "arrowheadColor"] {
            let mut value =
                json!({"htmlLabels": false, "themeVariables": {(config_path): "#246801"}});
            if swimlane {
                value["layout"] = json!("swimlane");
            }
            let rendered = render(
                ONE,
                &theme,
                MermaidConfig::from_value(value),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let owns_stroke = config_path == "lineColor";
            if !owns_stroke {
                assert_edge_stroke(rendered.svg(), "#ca3579", 1);
            }
            assert_evidence(
                rendered,
                usize::from(!owns_stroke),
                usize::from(owns_stroke),
                0,
            );
        }
    }
}

#[test]
fn edge_fill_without_edges_or_matching_selector_is_not_applicable() {
    for swimlane in [false, true] {
        for (source, rule) in [
            (
                "flowchart LR\nA[Alpha]\n",
                fill(CanvasPaint::solid("#ca3579").unwrap()),
            ),
            (
                ONE,
                fill(CanvasPaint::solid("#ca3579").unwrap()).with_variant(ThemeVariant::Active),
            ),
        ] {
            let rendered = render(
                source,
                &compile([rule]),
                config(swimlane, "classic"),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            assert_evidence(rendered, 0, 1, 0);
        }
    }
}

#[test]
fn duplicate_raw_edge_ids_keep_independent_ordinal_occurrences() {
    let source = "flowchart LR\nX L_A_B_0@--> Y\nA --> B\n";
    for swimlane in [false, true] {
        for ordinal in [2, 3] {
            let theme = compile([
                fill(CanvasPaint::solid("#ca3579").unwrap()),
                ThemeRule::new(
                    ThemeTarget::Edge,
                    ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#246801").unwrap()),
                )
                .with_ordinal(OrdinalSelector::exact(ordinal).unwrap()),
            ]);
            let rendered = render(
                source,
                &theme,
                config(swimlane, "classic"),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let paths: Vec<_> = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("path")
                        && node.attribute("data-edge") == Some("true")
                        && node.attribute("data-id") == Some("L_A_B_0")
                })
                .collect();
            assert_eq!(paths.len(), 2);
            assert_ne!(paths[0].attribute("id"), paths[1].attribute("id"));
            assert_ne!(paths[0].attribute("d"), paths[1].attribute("d"));
            assert_evidence(
                rendered,
                1,
                usize::from(ordinal == 3),
                usize::from(ordinal == 2),
            );
            assert_eq!(
                render(
                    source,
                    &theme,
                    config(swimlane, "classic"),
                    ThemePortabilityRequirement::RequirePortable
                )
                .is_err(),
                ordinal == 2
            );
        }
    }
}

#[test]
fn source_fill_does_not_take_ownership_of_the_edge_stroke_fallback() {
    let theme = compile([fill(CanvasPaint::solid("#ca3579").unwrap())]);
    for swimlane in [false, true] {
        let source = format!("{ONE}linkStyle 0 fill:#246801\n");
        let rendered = render(
            &source,
            &theme,
            config(swimlane, "classic"),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert_edge_stroke(rendered.svg(), "#ca3579", 1);
        assert_evidence(rendered, 1, 0, 0);
    }
}

#[test]
fn edge_paint_static_and_ordinal_winners_keep_the_original_order_for_each_semantic_facet() {
    for swimlane in [false, true] {
        for stroke in [false, true] {
            for ordinal_first in [false, true] {
                for source in [ONE, TWO] {
                    let rule = |color: &str| {
                        let paint = CanvasPaint::solid(color).unwrap();
                        let patch = if stroke {
                            ThemeStylePatch::default().with_stroke(paint)
                        } else {
                            ThemeStylePatch::default().with_fill(paint)
                        };
                        ThemeRule::new(ThemeTarget::Edge, patch)
                    };
                    let mut rules = vec![
                        rule("#ca3579"),
                        rule("#246801").with_ordinal(OrdinalSelector::exact(1).unwrap()),
                    ];
                    if ordinal_first {
                        rules.reverse();
                    }
                    let theme = compile(rules);
                    let rendered = render(
                        source,
                        &theme,
                        config(swimlane, "classic"),
                        ThemePortabilityRequirement::BestEffort,
                    )
                    .unwrap();
                    assert_evidence(
                        rendered,
                        usize::from(ordinal_first || source == TWO),
                        usize::from(ordinal_first || source == ONE),
                        usize::from(!ordinal_first),
                    );
                    assert_eq!(
                        render(
                            source,
                            &theme,
                            config(swimlane, "classic"),
                            ThemePortabilityRequirement::RequirePortable,
                        )
                        .is_err(),
                        !ordinal_first,
                    );
                }
            }
        }
    }
}
