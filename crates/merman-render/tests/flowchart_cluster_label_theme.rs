use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec,
    EffectBinding, EffectGraph, EffectInput, EffectPrimitive, OrdinalPalette, OrdinalSelector,
    Specified, ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

const ONE: &str = "flowchart TD\nsubgraph First[First title]\nA[Alpha]\nend\n";
const TWO: &str = "flowchart TD\nsubgraph First[First title]\nA[Alpha]\nend\nsubgraph Second[Second title]\nB[Beta]\nend\n";

fn fill(target: ThemeTarget, color: &str) -> ThemeRule {
    ThemeRule::new(
        target,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid(color).unwrap()),
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

fn config(swimlane: bool, base: &str, html: bool) -> MermaidConfig {
    let mut config = json!({"theme": base, "htmlLabels": html, "flowchart": {"htmlLabels": html}});
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
                diagram_id: Some("cluster-label-theme".to_owned()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
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

#[test]
fn cluster_label_fill_directly_reaches_both_css_terminals_in_default_and_base() {
    for swimlane in [false, true] {
        for base in ["default", "base"] {
            for html in [false, true] {
                for reverse in [false, true] {
                    let mut rules = vec![
                        fill(ThemeTarget::Text, "#135790"),
                        fill(ThemeTarget::Title, "#246801"),
                        fill(ThemeTarget::ClusterLabel, "#ab3257"),
                    ];
                    if reverse {
                        rules.reverse();
                    }
                    let rendered = render(
                        ONE,
                        &compile(rules),
                        config(swimlane, base, html),
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .unwrap();
                    for selector in [".cluster-label text", ".cluster text"] {
                        assert!(
                            rendered
                                .svg()
                                .contains(&format!("{selector}{{fill:#ab3257;}}"))
                        );
                    }
                    for selector in [".cluster-label span", ".cluster span"] {
                        assert!(
                            rendered
                                .svg()
                                .contains(&format!("{selector}{{color:#ab3257;}}"))
                        );
                    }
                    // The generic text rule still owns Alpha; Title owns no remaining terminal.
                    assert_evidence(rendered, 2, 1, 0);
                }
            }
        }
    }
}

#[test]
fn cluster_label_default_selector_keeps_author_order_within_the_more_specific_role() {
    for swimlane in [false, true] {
        for base in ["default", "base"] {
            for default_last in [false, true] {
                let mut rules = vec![
                    fill(ThemeTarget::ClusterLabel, "#ab3257").with_variant(ThemeVariant::Default),
                    fill(ThemeTarget::ClusterLabel, "#246801"),
                ];
                if default_last {
                    rules.reverse();
                }
                rules.push(fill(ThemeTarget::Title, "#135790").with_variant(ThemeVariant::Default));
                let rendered = render(
                    ONE,
                    &compile(rules),
                    config(swimlane, base, false),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .unwrap();
                let color = if default_last { "#ab3257" } else { "#246801" };
                assert!(
                    rendered
                        .svg()
                        .contains(&format!(".cluster-label text{{fill:{color};}}"))
                );
                assert_evidence(rendered, 1, 2, 0);
            }
        }
    }
}

#[test]
fn cluster_label_clear_is_a_winner_and_transparency_is_an_applied_paint() {
    for swimlane in [false, true] {
        for clear in [false, true] {
            let mut patch = ThemeStylePatch::default();
            patch.paint.fill = if clear {
                Specified::Clear
            } else {
                Specified::Value(CanvasPaint::Transparent)
            };
            let theme = compile([
                fill(ThemeTarget::Title, "#246801"),
                ThemeRule::new(ThemeTarget::ClusterLabel, patch),
            ]);
            let rendered = render(
                ONE,
                &theme,
                config(swimlane, "base", false),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert!(
                !rendered
                    .svg()
                    .contains(".cluster-label text{fill:#246801;}")
            );
            if !clear {
                assert!(
                    rendered
                        .svg()
                        .contains(".cluster-label text{fill:transparent;}")
                );
            }
            assert_evidence(rendered, usize::from(!clear), 1, usize::from(clear));
            assert_eq!(
                render(
                    ONE,
                    &theme,
                    config(swimlane, "base", false),
                    ThemePortabilityRequirement::RequirePortable
                )
                .is_err(),
                clear
            );
        }
    }
}

#[test]
fn cluster_label_ordinal_residual_does_not_certify_the_shadowed_title_fallback() {
    for swimlane in [false, true] {
        for (source, ordinal, applied, absent, residual) in [
            (ONE, 1, 0, 1, 1),
            (TWO, 1, 1, 0, 1),
            (TWO, 2, 1, 0, 1),
            (TWO, 3, 1, 1, 0),
        ] {
            let theme = compile([
                fill(ThemeTarget::Title, "#246801"),
                fill(ThemeTarget::ClusterLabel, "#ab3257")
                    .with_variant(ThemeVariant::Default)
                    .with_ordinal(OrdinalSelector::exact(ordinal).unwrap()),
            ]);
            let rendered = render(
                source,
                &theme,
                config(swimlane, "default", false),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert_evidence(rendered, applied, absent, residual);
            assert_eq!(
                render(
                    source,
                    &theme,
                    config(swimlane, "default", false),
                    ThemePortabilityRequirement::RequirePortable
                )
                .is_err(),
                residual != 0
            );
        }
    }
}

#[test]
fn cluster_label_sibling_facets_are_checked_independently_of_the_fill_winner() {
    for swimlane in [false, true] {
        for mixed in [false, true] {
            let mut patch =
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#ac5432").unwrap());
            if mixed {
                patch = patch.with_fill(CanvasPaint::solid("#ab3257").unwrap());
            }
            let theme = compile([
                fill(ThemeTarget::Title, "#246801"),
                ThemeRule::new(ThemeTarget::ClusterLabel, patch),
            ]);
            let rendered = render(
                ONE,
                &theme,
                config(swimlane, "default", false),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let color = if mixed { "#ab3257" } else { "#246801" };
            assert!(
                rendered
                    .svg()
                    .contains(&format!(".cluster-label text{{fill:{color};}}"))
            );
            assert_evidence(rendered, usize::from(!mixed), usize::from(mixed), 1);
            assert!(
                render(
                    ONE,
                    &theme,
                    config(swimlane, "default", false),
                    ThemePortabilityRequirement::RequirePortable
                )
                .is_err()
            );
        }
    }
}

#[test]
fn cluster_label_fill_yields_to_explicit_config_and_emitted_source_color() {
    let theme = compile([fill(ThemeTarget::ClusterLabel, "#ab3257")]);
    for swimlane in [false, true] {
        for base in ["default", "base"] {
            let mut value = json!({"theme": base, "htmlLabels": false, "themeVariables": {"titleColor": "#135790"}});
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
            assert!(
                rendered
                    .svg()
                    .contains(".cluster-label text{fill:#135790;}")
            );
            assert_evidence(rendered, 0, 1, 0);
        }
        for html in [false, true] {
            let source = format!(
                "{ONE}{}",
                if html {
                    "style First color:#135790\n"
                } else {
                    "classDef ink color:#135790\nclass First ink\n"
                }
            );
            let rendered = render(
                &source,
                &theme,
                config(swimlane, "default", html),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            assert_evidence(rendered, 0, 1, 0);
        }
    }
}

#[test]
fn cluster_label_without_a_visible_terminal_is_not_applicable() {
    for swimlane in [false, true] {
        let theme = compile([fill(ThemeTarget::ClusterLabel, "#ab3257")]);
        let rendered = render(
            "flowchart TD\nA[Alpha]\n",
            &theme,
            config(swimlane, "default", false),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        assert_evidence(rendered, 0, 1, 0);
    }
}

#[test]
fn cluster_label_palette_is_residual_only_when_the_terminal_has_no_rule_fill() {
    for swimlane in [false, true] {
        for static_title in [false, true] {
            let palette =
                OrdinalPalette::new([ThemeColorValue::parse("#ab3257").unwrap()]).unwrap();
            let mut rules =
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::ClusterLabel, palette);
            if static_title {
                rules = rules.with_rule(fill(ThemeTarget::Title, "#246801"));
            }
            let theme = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_styles(rules))
                .unwrap();
            let rendered = render(
                ONE,
                &theme,
                config(swimlane, "base", false),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert_evidence(
                rendered,
                usize::from(static_title),
                usize::from(static_title),
                usize::from(!static_title),
            );
            assert_eq!(
                render(
                    ONE,
                    &theme,
                    config(swimlane, "base", false),
                    ThemePortabilityRequirement::RequirePortable
                )
                .is_err(),
                !static_title
            );
        }
    }
}

#[test]
fn cluster_label_effect_binding_preserves_residuals_and_honors_a_cleared_effect() {
    for swimlane in [false, true] {
        for clear in [false, true] {
            let effects = DiagramEffectSet::default()
                .with_graph(
                    EffectGraph::new(
                        "blur",
                        [EffectPrimitive::GaussianBlur {
                            input: EffectInput::SourceGraphic,
                            std_deviation: 1.0,
                        }],
                    )
                    .unwrap(),
                )
                .unwrap()
                .with_binding(EffectBinding::new(ThemeTarget::ClusterLabel, "blur").unwrap())
                .unwrap();
            let mut spec = DiagramThemeSpec::new().with_effects(effects);
            if clear {
                let mut patch = ThemeStylePatch::default();
                patch.effects.effect = Specified::Clear;
                spec = spec.with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(ThemeTarget::ClusterLabel, patch)),
                );
            }
            let theme = DiagramThemeCompiler::new().compile(spec).unwrap();
            let rendered = render(
                ONE,
                &theme,
                config(swimlane, "default", false),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            // Clear is itself unsupported, while the binding it suppresses is not applicable.
            assert_evidence(rendered, 0, usize::from(clear), 1);
            assert!(
                render(
                    ONE,
                    &theme,
                    config(swimlane, "default", false),
                    ThemePortabilityRequirement::RequirePortable
                )
                .is_err()
            );
        }
    }
}

#[test]
fn cluster_label_config_ownership_follows_the_actual_theme_dependency() {
    let theme = compile([fill(ThemeTarget::ClusterLabel, "#ab3257")]);
    for swimlane in [false, true] {
        for base in ["default", "base"] {
            let mut value = json!({"theme": base, "htmlLabels": false,
                "themeVariables": {"tertiaryTextColor": "#135790"}});
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
            // Base derives titleColor from tertiaryTextColor; Default derives it from textColor.
            let owned = base == "base";
            let color = if owned { "#135790" } else { "#ab3257" };
            assert!(
                rendered
                    .svg()
                    .contains(&format!(".cluster-label text{{fill:{color};}}"))
            );
            assert_evidence(rendered, usize::from(!owned), usize::from(owned), 0);
        }
    }
}

#[test]
fn cluster_label_absent_or_stroke_only_preserves_the_existing_text_title_fill_order() {
    for swimlane in [false, true] {
        for base in ["default", "base"] {
            for title_last in [false, true] {
                for stroke_only in [false, true] {
                    let mut rules = vec![
                        fill(ThemeTarget::Title, "#246801"),
                        fill(ThemeTarget::Text, "#135790"),
                    ];
                    if title_last {
                        rules.reverse();
                    }
                    if stroke_only {
                        rules.push(ThemeRule::new(
                            ThemeTarget::ClusterLabel,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#ab3257").unwrap()),
                        ));
                    }
                    let theme = compile(rules);
                    let rendered = render(
                        ONE,
                        &theme,
                        config(swimlane, base, false),
                        ThemePortabilityRequirement::BestEffort,
                    )
                    .unwrap();
                    let color = if title_last { "#246801" } else { "#135790" };
                    assert!(
                        rendered
                            .svg()
                            .contains(&format!(".cluster-label text{{fill:{color};}}"))
                    );
                    // Text always owns Alpha. Its later fill also owns the cluster when Title is earlier.
                    assert_evidence(
                        rendered,
                        1 + usize::from(title_last),
                        usize::from(!title_last),
                        usize::from(stroke_only),
                    );
                    assert_eq!(
                        render(
                            ONE,
                            &theme,
                            config(swimlane, base, false),
                            ThemePortabilityRequirement::RequirePortable
                        )
                        .is_err(),
                        stroke_only
                    );
                }
            }
        }
    }
}
