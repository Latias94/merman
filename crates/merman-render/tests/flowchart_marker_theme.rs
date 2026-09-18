use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec,
    EffectBinding, EffectGraph, EffectInput, EffectPrimitive, GradientStop, LinearGradient,
    OrdinalPalette, OrdinalSelector, Specified, ThemeColorValue, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

const POINT: &str = "flowchart LR\nA[Alpha] --> B[Beta]\n";

fn rule(stroke: bool) -> ThemeRule {
    let paint = CanvasPaint::solid("#ca3579").unwrap();
    ThemeRule::new(
        ThemeTarget::Marker,
        if stroke {
            ThemeStylePatch::default().with_stroke(paint)
        } else {
            ThemeStylePatch::default().with_fill(paint)
        },
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
    let mut value = json!({"htmlLabels": false, "look": look});
    if swimlane {
        value["layout"] = json!("swimlane");
    }
    MermaidConfig::from_value(value)
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
                diagram_id: Some("marker-theme".to_owned()),
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

fn marker_references(svg: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(svg).unwrap();
    marker_references_in(&document)
}

fn marker_references_in(document: &roxmltree::Document<'_>) -> Vec<String> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .flat_map(|path| [path.attribute("marker-start"), path.attribute("marker-end")])
        .flatten()
        .map(str::to_owned)
        .collect()
}

#[test]
fn unused_marker_defs_do_not_create_theme_occurrences() {
    for swimlane in [false, true] {
        for source in [
            "flowchart LR\nA[Alpha]\n",
            "flowchart LR\nA[Alpha] --- B[Beta]\n",
        ] {
            let rendered = render(
                source,
                &compile([rule(false), rule(true)]),
                config(swimlane, "classic"),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(
                document
                    .descendants()
                    .filter(|node| node.has_tag_name("marker"))
                    .count()
                    >= 6
            );
            assert!(marker_references(rendered.svg()).is_empty());
            assert_evidence(rendered, 0, 2, 0);
        }
    }
}

#[test]
fn referenced_marker_fill_and_stroke_are_unsupported_for_every_look() {
    for swimlane in [false, true] {
        for look in ["classic", "neo", "handDrawn"] {
            for arrow in ["-->", "--o", "--x"] {
                for explicit_default in [false, true] {
                    let source = format!("flowchart LR\nA[Alpha] {arrow} B[Beta]\n");
                    let rules = [rule(false), rule(true)].map(|rule| {
                        if explicit_default {
                            rule.with_variant(ThemeVariant::Default)
                        } else {
                            rule
                        }
                    });
                    let theme = compile(rules);
                    let rendered = render(
                        &source,
                        &theme,
                        config(swimlane, look),
                        ThemePortabilityRequirement::BestEffort,
                    )
                    .unwrap();
                    assert_eq!(marker_references(rendered.svg()).len(), 1);
                    assert_evidence(rendered, 0, 0, 2);
                    assert!(
                        render(
                            &source,
                            &theme,
                            config(swimlane, look),
                            ThemePortabilityRequirement::RequirePortable
                        )
                        .is_err()
                    );
                }
            }
        }
    }
}

#[test]
fn marker_ordinals_count_start_and_end_references_instead_of_edges_or_defs() {
    for swimlane in [false, true] {
        for (source, count) in [
            ("flowchart LR\nA <--> B\n", 2),
            ("flowchart LR\nA --- B --> C\n", 1),
            ("flowchart LR\nX L_A_B_0@--> Y\nA --> B\n", 2),
        ] {
            for ordinal in 1..=3 {
                let theme =
                    compile([rule(false).with_ordinal(OrdinalSelector::exact(ordinal).unwrap())]);
                let rendered = render(
                    source,
                    &theme,
                    config(swimlane, "classic"),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                assert_eq!(marker_references(rendered.svg()).len(), count);
                let matches = ordinal <= count;
                assert_evidence(rendered, 0, usize::from(!matches), usize::from(matches));
                assert_eq!(
                    render(
                        source,
                        &theme,
                        config(swimlane, "classic"),
                        ThemePortabilityRequirement::RequirePortable
                    )
                    .is_err(),
                    matches
                );
            }
        }
    }
}

#[test]
fn overwritten_marker_rules_and_unmatched_variants_are_not_applicable() {
    for swimlane in [false, true] {
        let theme = compile([
            rule(false).with_ordinal(OrdinalSelector::exact(1).unwrap()),
            rule(false).with_variant(ThemeVariant::Default),
            rule(true).with_variant(ThemeVariant::Active),
        ]);
        let rendered = render(
            POINT,
            &theme,
            config(swimlane, "classic"),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert_evidence(rendered, 0, 2, 1);
    }
}

#[test]
fn source_marker_colors_own_only_the_shape_attributes_that_are_written() {
    for swimlane in [false, true] {
        for look in ["classic", "neo", "handDrawn"] {
            for (arrow, expected_fill_owned) in [("-->", true), ("--o", false), ("--x", false)] {
                let source = format!("flowchart LR\nA {arrow} B\nlinkStyle 0 stroke:#246801\n");
                let theme = compile([rule(false), rule(true)]);
                let rendered = render(
                    &source,
                    &theme,
                    config(swimlane, look),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                assert_eq!(marker_references(rendered.svg()).len(), 1);
                let owned = if look == "handDrawn" {
                    0
                } else {
                    1 + usize::from(expected_fill_owned)
                };
                assert_evidence(rendered, 0, owned, 2 - owned);
                assert_eq!(
                    render(
                        &source,
                        &theme,
                        config(swimlane, look),
                        ThemePortabilityRequirement::RequirePortable
                    )
                    .is_err(),
                    owned != 2
                );
            }
        }
    }
}

#[test]
fn partially_owned_marker_paint_does_not_hide_a_sibling_in_the_same_rule() {
    for swimlane in [false, true] {
        let source = "flowchart LR\nA --o B\nlinkStyle 0 stroke:#246801\n";
        let theme = compile([ThemeRule::new(
            ThemeTarget::Marker,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#ca3579").unwrap())
                .with_stroke(CanvasPaint::solid("#ca3579").unwrap()),
        )]);
        let rendered = render(
            source,
            &theme,
            config(swimlane, "classic"),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert_evidence(rendered, 0, 0, 1);
        assert!(
            render(
                source,
                &theme,
                config(swimlane, "classic"),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_err()
        );
    }
}

#[test]
fn neo_margin_marker_ownership_does_not_claim_visible_stroke_pixels_or_own_the_unpainted_start() {
    for swimlane in [false, true] {
        let source = "flowchart LR\nA --o B\nlinkStyle 0 stroke:#246801\n";
        let rendered = render(
            source,
            &compile([rule(true)]),
            config(swimlane, "neo"),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let refs = marker_references(rendered.svg());
        let marker_id = refs[0]
            .strip_prefix("url(#")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let marker = document
            .descendants()
            .find(|node| node.attribute("id") == Some(marker_id))
            .unwrap();
        let shape = marker
            .descendants()
            .find(|node| node.has_tag_name("circle"))
            .unwrap();
        assert_eq!(shape.attribute("stroke"), Some("#246801"));
        assert!(
            shape
                .attribute("style")
                .unwrap()
                .contains("stroke-width: 0;")
        );
        // NA proves source precedence of the requested color, not a visible or Applied stroke.
        assert_evidence(rendered, 0, 1, 0);

        let source = "flowchart LR\nA <--> B\nlinkStyle 0 stroke:#246801\n";
        let rendered = render(
            source,
            &compile([rule(false)]),
            config(swimlane, "neo"),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert_eq!(marker_references(rendered.svg()).len(), 2);
        // PointStart-margin has no per-shape paint attributes; PointEnd cannot mask it.
        assert_evidence(rendered, 0, 0, 1);
    }
}

#[test]
fn marker_config_ownership_uses_line_color_and_ignores_the_retired_arrowhead_selector() {
    for swimlane in [false, true] {
        for variable in ["lineColor", "arrowheadColor"] {
            let mut value = json!({"htmlLabels": false, "themeVariables": {(variable): "#246801"}});
            if swimlane {
                value["layout"] = json!("swimlane");
            }
            let theme = compile([rule(false), rule(true)]);
            let owned = variable == "lineColor";
            let rendered = render(
                POINT,
                &theme,
                MermaidConfig::from_value(value.clone()),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert_evidence(
                rendered,
                0,
                if owned { 2 } else { 0 },
                if owned { 0 } else { 2 },
            );
            assert_eq!(
                render(
                    POINT,
                    &theme,
                    MermaidConfig::from_value(value),
                    ThemePortabilityRequirement::RequirePortable
                )
                .is_err(),
                !owned
            );
        }
    }
}

#[test]
fn unverified_source_marker_color_cannot_suppress_unsupported_theme_requests() {
    for swimlane in [false, true] {
        let source = "flowchart LR\nA --> B\nlinkStyle 0 stroke:var(--marker-ink)\n";
        let theme = compile([rule(false), rule(true)]);
        let rendered = render(
            source,
            &theme,
            config(swimlane, "classic"),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert_evidence(rendered, 0, 0, 2);
        assert!(
            render(
                source,
                &theme,
                config(swimlane, "classic"),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_err()
        );
    }
}

#[test]
fn unsupported_marker_values_and_fallback_mechanisms_require_real_references() {
    let gradient = CanvasPaint::LinearGradient(
        LinearGradient::new(
            90.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#123456").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#abcdef").unwrap()).unwrap(),
            ],
        )
        .unwrap(),
    );
    for swimlane in [false, true] {
        for paint in [
            Specified::Clear,
            Specified::Value(CanvasPaint::Transparent),
            Specified::Value(gradient.clone()),
        ] {
            let mut patch = ThemeStylePatch::default();
            patch.paint.fill = paint;
            let theme = compile([ThemeRule::new(ThemeTarget::Marker, patch)]);
            for (source, visible) in [(POINT, true), ("flowchart LR\nA --- B\n", false)] {
                let rendered = render(
                    source,
                    &theme,
                    config(swimlane, "classic"),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                assert_evidence(rendered, 0, usize::from(!visible), usize::from(visible));
            }
        }
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
            .with_binding(EffectBinding::new(ThemeTarget::Marker, "blur").unwrap())
            .unwrap();
        let palette = OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_effects(effects).with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Marker, palette),
            ))
            .unwrap();
        for (source, visible) in [(POINT, true), ("flowchart LR\nA --- B\n", false)] {
            let rendered = render(
                source,
                &theme,
                config(swimlane, "classic"),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert_evidence(
                rendered,
                0,
                if visible { 0 } else { 2 },
                if visible { 2 } else { 0 },
            );
        }
    }
}

fn referenced_marker_paints(svg: &str) -> Vec<(String, Option<String>, Option<String>)> {
    let document = roxmltree::Document::parse(svg).unwrap();
    marker_references_in(&document)
        .iter()
        .map(|reference| {
            let id = reference
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let marker = document
                .descendants()
                .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
                .unwrap();
            let shape = marker.children().find(|node| node.is_element()).unwrap();
            (
                shape.tag_name().name().to_owned(),
                shape.attribute("stroke").map(str::to_owned),
                shape.attribute("fill").map(str::to_owned),
            )
        })
        .collect()
}

#[test]
fn typed_edge_paint_reaches_its_actual_direction_markers() {
    let theme = compile([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#ca3579").unwrap()),
    )]);
    for swimlane in [false, true] {
        for look in ["classic", "neo", "handDrawn"] {
            for (arrow, count, filled) in [("<-->", 2, true), ("o--o", 2, true), ("x--x", 2, false)]
            {
                let source = format!("flowchart LR\nA {arrow} B\n");
                let rendered = render(
                    &source,
                    &theme,
                    config(swimlane, look),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .unwrap();
                let paints = referenced_marker_paints(rendered.svg());
                assert_eq!(paints.len(), count);
                for (_, stroke, fill) in paints {
                    assert_eq!(
                        stroke.as_deref(),
                        Some("#ca3579"),
                        "{swimlane}/{look}/{arrow}"
                    );
                    assert_eq!(
                        fill.as_deref(),
                        filled.then_some("#ca3579"),
                        "{swimlane}/{look}/{arrow}"
                    );
                }
            }
        }
    }
}

#[test]
fn source_and_config_edge_owners_preserve_their_marker_behavior() {
    let theme = compile([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#ca3579").unwrap()),
    )]);
    let plain = compile([]);
    for swimlane in [false, true] {
        for look in ["classic", "neo", "handDrawn"] {
            let source = "flowchart LR\nA <--> B\nlinkStyle 0 stroke:#246801\n";
            let themed = render(
                source,
                &theme,
                config(swimlane, look),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let base = render(
                source,
                &plain,
                config(swimlane, look),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert_eq!(
                referenced_marker_paints(themed.svg()),
                referenced_marker_paints(base.svg())
            );
            assert_eq!(
                marker_references(themed.svg()),
                marker_references(base.svg())
            );
        }
    }
    let cfg = MermaidConfig::from_value(
        json!({"htmlLabels":false,"themeVariables":{"lineColor":"#123456"}}),
    );
    let themed = render(
        POINT,
        &theme,
        cfg.clone(),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    let base = render(POINT, &plain, cfg, ThemePortabilityRequirement::BestEffort).unwrap();
    assert_eq!(
        referenced_marker_paints(themed.svg()),
        referenced_marker_paints(base.svg())
    );
    assert_eq!(
        marker_references(themed.svg()),
        marker_references(base.svg())
    );
}

#[test]
fn public_cyberpunk_scene_has_cyan_direction_markers_after_recipe_exchange() {
    use merman_render::diagram_theme::ThemePreset;
    let compiler = DiagramThemeCompiler::new();
    let theme = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
    let wire =
        serde_json::to_vec(&compiler.export_preset(ThemePreset::Cyberpunk).unwrap()).unwrap();
    let imported = DiagramThemeCompiler::new()
        .compile_recipe(serde_json::from_slice(&wire).unwrap())
        .unwrap();
    let source =
        include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/flowchart.mmd");
    for selected in [&theme, &imported] {
        let rendered = render(
            source,
            selected,
            config(false, "classic"),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let paints = referenced_marker_paints(rendered.svg());
        assert_eq!(paints.len(), 3);
        assert!(
            paints
                .iter()
                .all(|(_, stroke, fill)| stroke.as_deref() == Some("#00f2ff")
                    && fill.as_deref() == Some("#00f2ff")),
            "{paints:?}"
        );
    }
}

#[test]
fn edge_clear_transparency_and_fill_fallback_are_shared_with_markers() {
    for (style, expected) in [
        (
            ThemeStylePatch::default().with_stroke(CanvasPaint::Transparent),
            Some("none"),
        ),
        (
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123abc").unwrap()),
            Some("#123abc"),
        ),
        (
            ThemeStylePatch {
                stroke: merman_render::diagram_theme::ThemeStrokePatch {
                    paint: Specified::Clear,
                    ..Default::default()
                },
                ..Default::default()
            },
            None,
        ),
    ] {
        let theme = compile([ThemeRule::new(ThemeTarget::Edge, style)]);
        let rendered = render(
            POINT,
            &theme,
            config(false, "classic"),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert_eq!(
            render(
                POINT,
                &theme,
                config(false, "classic"),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_ok(),
            expected.is_some()
        );
        let paints = referenced_marker_paints(rendered.svg());
        assert_eq!(paints.len(), 1);
        assert_eq!(paints[0].1.as_deref(), expected);
        assert_eq!(paints[0].2.as_deref(), expected);
    }
}

#[test]
fn source_and_typed_same_color_do_not_alias_distinct_marker_shapes() {
    let theme = compile([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#ca3579").unwrap()),
    )]);
    // The source-owned Neo start marker preserves its existing unpainted shape; the typed
    // start marker needs its own definition even though their color values are identical.
    let source = "flowchart LR\nA <--> B\nC <--> D\nlinkStyle 0 stroke:#ca3579\n";
    let rendered = render(
        source,
        &theme,
        config(false, "neo"),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    let refs = marker_references(rendered.svg());
    assert_eq!(refs.len(), 4);
    assert_ne!(refs[0], refs[2]);
    let paints = referenced_marker_paints(rendered.svg());
    assert_eq!(paints[0].2, None);
    assert_eq!(paints[2].2.as_deref(), Some("#ca3579"));
}

#[test]
fn transparent_edge_paint_clears_all_visible_marker_channels() {
    let theme = compile([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(CanvasPaint::Transparent),
    )]);
    for look in ["classic", "neo", "handDrawn"] {
        for (arrow, filled) in [("<-->", true), ("o--o", true), ("x--x", false)] {
            let source = format!("flowchart LR\nA {arrow} B");
            let rendered = render(
                &source,
                &theme,
                config(false, look),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let paints = referenced_marker_paints(rendered.svg());
            assert_eq!(paints.len(), 2);
            for (_, stroke, fill) in paints {
                assert_eq!(stroke.as_deref(), Some("none"), "{look}/{arrow}");
                assert_eq!(fill.as_deref(), filled.then_some("none"), "{look}/{arrow}");
            }
        }
    }
}
