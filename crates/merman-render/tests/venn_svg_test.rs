use merman_core::{MermaidEngine, ParseOptions};
use merman_render::DiagramFamilyId;
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack,
    MermaidThemeCompatibility, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::VennDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

const THEME_TYPOGRAPHY_SOURCE: &str = r#"venn-beta
title Typography Surface
set A["Alpha"]:20
  text A1["Nested Alpha"]
set B["Beta"]:12
union A,B["Shared"]:3
  text AB1["Nested Shared"]
"#;

fn venn_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::VENN, typography),
        ))
        .expect("compile Venn typography theme")
}

fn venn_paint_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                rules
                    .into_iter()
                    .fold(ThemeRuleSet::default(), |rules, rule| rules.with_rule(rule)),
            ),
        )
        .expect("compile Venn paint theme")
}

fn venn_core_paint_theme(transparent: bool, variant: Option<ThemeVariant>) -> DiagramTheme {
    let paint = |color| {
        if transparent {
            CanvasPaint::Transparent
        } else {
            solid(color)
        }
    };
    venn_paint_theme(
        [
            ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default()
                    .with_fill(paint("#123456"))
                    .with_stroke(paint("#345678")),
            ),
            ThemeRule::new(
                ThemeTarget::NodeLabel,
                ThemeStylePatch::default().with_fill(paint("#56789a")),
            ),
        ]
        .map(|rule| variant.map_or(rule.clone(), |variant| rule.with_variant(variant))),
    )
}

#[test]
fn venn_core_paints_reach_real_set_paths_and_labels_with_same_render_evidence() {
    for transparent in [false, true] {
        for variant in [None, Some(ThemeVariant::Default)] {
            let theme = venn_core_paint_theme(transparent, variant);
            let rendered = render_venn_with_theme_requirement(
                THEME_TYPOGRAPHY_SOURCE,
                &theme,
                MermaidEngine::new(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let circles = document
                .descendants()
                .filter(|node| {
                    node.attribute("class").is_some_and(|class| {
                        class.split_whitespace().any(|class| class == "venn-circle")
                    })
                })
                .collect::<Vec<_>>();
            assert_eq!(circles.len(), 2);
            for circle in circles {
                let path = circle
                    .children()
                    .find(|node| node.has_tag_name("path"))
                    .unwrap();
                let style = path.attribute("style").unwrap();
                let fill = if transparent {
                    "transparent"
                } else {
                    "#123456"
                };
                let stroke = if transparent {
                    "transparent"
                } else {
                    "#345678"
                };
                assert!(style.contains(&format!("fill: {fill};")));
                assert!(style.contains(&format!("stroke: {stroke};")));
                let text = circle
                    .children()
                    .find(|node| node.has_tag_name("text"))
                    .unwrap();
                let color = if transparent {
                    "transparent"
                } else {
                    "#56789a"
                };
                assert!(
                    text.attribute("style")
                        .unwrap()
                        .contains(&format!("fill: {color};"))
                );
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.applied_count(), 2);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn venn_static_paint_evidence_follows_real_occurrence_winners() {
    use merman_render::diagram_theme::OrdinalSelector;

    let source = "venn-beta\nset A[\"Alpha\"]:20\n";
    for (reverse, ordinal, mixed, applied, not_applicable, residual) in [
        (false, 1, false, 0, 1, 1),
        (true, 1, false, 1, 1, 0),
        (false, 2, false, 1, 1, 0),
        (false, 1, true, 1, 0, 1),
    ] {
        let mut patch = ThemeStylePatch::default().with_fill(solid("#111111"));
        if mixed {
            patch = patch.with_stroke(solid("#334455"));
        }
        let mut rules = vec![
            ThemeRule::new(ThemeTarget::Node, patch),
            ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(solid("#222222")),
            )
            .with_ordinal(OrdinalSelector::exact(ordinal).unwrap()),
        ];
        if reverse {
            rules.reverse();
        }
        let rendered = render_venn_with_theme_requirement(
            source,
            &venn_paint_theme(rules),
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let path = document
            .descendants()
            .find(|node| node.attribute("data-venn-sets") == Some("A"))
            .unwrap()
            .children()
            .find(|node| node.has_tag_name("path"))
            .unwrap();
        assert!(path.attribute("style").unwrap().contains("fill: #111111;"));
        if mixed {
            assert!(
                path.attribute("style")
                    .unwrap()
                    .contains("stroke: #334455;")
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.applied_count(), applied);
        assert_eq!(evidence.not_applicable_count(), not_applicable);
        assert_eq!(evidence.theme_residual_count(), residual);
    }
}

#[test]
fn venn_explicit_default_title_fill_reaches_the_real_title() {
    let rendered = render_venn_with_theme_requirement(
        "venn-beta\ntitle T\nset A[\"Alpha\"]:20\n",
        &venn_paint_theme([ThemeRule::new(
            ThemeTarget::Title,
            ThemeStylePatch::default().with_fill(solid("#123456")),
        )
        .with_variant(ThemeVariant::Default)]),
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let title = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.text() == Some("T"))
        .unwrap();
    assert!(title.attribute("style").unwrap().contains("fill: #123456;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn venn_source_and_config_paints_own_only_their_actual_properties() {
    let theme = venn_core_paint_theme(false, None);
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": { "venn2": "#abcdef" }
        })),
    );
    let rendered = render_venn_with_theme_requirement(
        "venn-beta\nset A[\"Alpha\"]:20\nset B[\"Beta\"]:12\nunion A,B:3\nstyle A fill:#de3163, color:#654321\n",
        &theme, engine, ThemePortabilityRequirement::RequirePortable,
    ).unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let a = document
        .descendants()
        .find(|node| node.attribute("data-venn-sets") == Some("A"))
        .unwrap();
    let style = a
        .children()
        .find(|node| node.has_tag_name("path"))
        .unwrap()
        .attribute("style")
        .unwrap();
    assert!(style.contains("fill: #de3163;"));
    assert!(style.contains("stroke: #345678;"));
    assert!(
        a.children()
            .find(|node| node.has_tag_name("text"))
            .unwrap()
            .attribute("style")
            .unwrap()
            .contains("fill: #654321;")
    );
    let b = document
        .descendants()
        .find(|node| node.attribute("data-venn-sets") == Some("B"))
        .unwrap();
    let style = b
        .children()
        .find(|node| node.has_tag_name("path"))
        .unwrap()
        .attribute("style")
        .unwrap();
    assert!(style.contains("fill: #abcdef;"));
    assert!(style.contains("stroke: #abcdef;"));
    let text_color = merman_markup::theme_color::darken("#abcdef", 30.0).unwrap();
    assert!(
        b.children()
            .find(|node| node.has_tag_name("text"))
            .unwrap()
            .attribute("style")
            .unwrap()
            .contains(&format!("fill: {text_color};"))
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn venn_node_label_paint_respects_the_selected_palette_without_claiming_site_defaults() {
    let theme = venn_paint_theme([ThemeRule::new(
        ThemeTarget::NodeLabel,
        ThemeStylePatch::default().with_fill(solid("#56789a")),
    )]);
    for palette in [
        "default",
        "base",
        "dark",
        "forest",
        "neutral",
        "neo",
        "neo-dark",
        "redux",
        "redux-dark",
        "redux-color",
        "redux-dark-color",
    ] {
        for source_owner in [false, true] {
            let config = serde_json::json!({"theme": palette});
            let source = if source_owner {
                format!("%%{{init: {config}}}%%\n{THEME_TYPOGRAPHY_SOURCE}")
            } else {
                THEME_TYPOGRAPHY_SOURCE.to_owned()
            };
            let engine = || {
                if source_owner {
                    MermaidEngine::new()
                } else {
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            config.clone(),
                        ),
                    )
                }
            };
            let (_, baseline) = render_typed_venn_with_engine(&source, engine());
            let rendered = render_venn_with_theme_requirement(
                &source,
                &theme,
                engine(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let baseline = roxmltree::Document::parse(&baseline).unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for set in ["A", "B"] {
                let text_style = |doc: &roxmltree::Document<'_>| {
                    find_venn_area(doc, set, "venn-circle")
                        .children()
                        .find(|node| node.has_tag_name("text"))
                        .unwrap()
                        .attribute("style")
                        .unwrap()
                        .to_owned()
                };
                if source_owner {
                    assert_eq!(text_style(&document), text_style(&baseline), "{palette}");
                } else {
                    assert!(
                        text_style(&document).contains("fill: #56789a;"),
                        "{palette}"
                    );
                }
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), usize::from(!source_owner));
            assert_eq!(evidence.not_applicable_count(), usize::from(source_owner));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn venn_empty_palette_entries_are_skipped_before_contrast_and_ownership_selection() {
    let theme = venn_paint_theme([ThemeRule::new(
        ThemeTarget::Node,
        ThemeStylePatch::default().with_fill(solid("#123456")),
    )]);
    for source_owner in [false, true] {
        let config = serde_json::json!({
            "theme": "default",
            "themeVariables": {"venn1": "", "venn2": "#abcdef"}
        });
        let source = if source_owner {
            format!("%%{{init: {config}}}%%\n{THEME_TYPOGRAPHY_SOURCE}")
        } else {
            THEME_TYPOGRAPHY_SOURCE.to_owned()
        };
        let engine = || {
            if source_owner {
                MermaidEngine::new()
            } else {
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(config.clone()),
                )
            }
        };
        for svg in [
            render_typed_venn_with_engine(&source, engine()).1,
            render_venn_with_theme_requirement(
                &source,
                &theme,
                engine(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap()
            .svg()
            .to_owned(),
        ] {
            let document = roxmltree::Document::parse(&svg).unwrap();
            let path = find_venn_area(&document, "A", "venn-circle")
                .children()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            assert!(path.attribute("style").unwrap().contains("fill: #abcdef;"));
        }
    }
}

#[test]
fn venn_title_fill_uses_the_same_nonempty_role_for_paint_and_ownership() {
    let theme = venn_paint_theme([ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default().with_fill(solid("#123456")),
    )]);
    for source_owner in [false, true] {
        for explicit_fallback in [false, true] {
            let mut config = serde_json::json!({
                "theme": "default", "themeVariables": {"vennTitleTextColor": ""}
            });
            if explicit_fallback {
                config["themeVariables"]["titleColor"] = serde_json::json!("#abcdef");
            }
            let source = if source_owner {
                format!("%%{{init: {config}}}%%\n{THEME_TYPOGRAPHY_SOURCE}")
            } else {
                THEME_TYPOGRAPHY_SOURCE.to_owned()
            };
            let engine = if source_owner {
                MermaidEngine::new()
            } else {
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(config),
                )
            };
            let rendered = render_venn_with_theme_requirement(
                &source,
                &theme,
                engine,
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let title = document
                .descendants()
                .find(|node| node.attribute("class") == Some("venn-title"))
                .unwrap();
            let expected = if explicit_fallback {
                "#abcdef"
            } else if source_owner {
                "#333"
            } else {
                "#123456"
            };
            assert_eq!(
                title.attribute("style"),
                Some(format!("fill: {expected};").as_str())
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let owned = source_owner || explicit_fallback;
            assert_eq!(evidence.applied_count(), usize::from(!owned));
            assert_eq!(evidence.not_applicable_count(), usize::from(owned));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn venn_text_fill_follows_the_initialized_or_fallback_color_role() {
    let theme = venn_paint_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(solid("#123456")),
    )]);
    for local in [serde_json::Value::Null, serde_json::json!(false)] {
        for explicit_primary in [false, true] {
            let mut site = serde_json::json!({
                "theme": "default", "themeVariables": {"vennSetTextColor": local}
            });
            if explicit_primary {
                site["themeVariables"]["primaryTextColor"] = serde_json::json!("#abcdef");
            }
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(site),
            );
            let config = common::install_parse_compatibility(&theme, engine.clone())
                .parse_metadata_sync(THEME_TYPOGRAPHY_SOURCE)
                .unwrap()
                .effective_config;
            // Null triggers Default's snapshot initialization; non-string roles use the renderer fallback.
            assert_eq!(
                config.get_str("themeVariables.vennSetTextColor"),
                local.is_null().then_some("#333")
            );
            let fallback_owned = !local.is_null() && explicit_primary;
            let rendered = render_venn_with_theme_requirement(
                THEME_TYPOGRAPHY_SOURCE,
                &theme,
                engine,
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let intersection = find_venn_area(&document, "A_B", "venn-intersection")
                .children()
                .find(|node| node.has_tag_name("text"))
                .unwrap();
            let expected = if fallback_owned { "#abcdef" } else { "#123456" };
            assert!(
                intersection
                    .attribute("style")
                    .unwrap()
                    .contains(&format!("fill: {expected};")),
                "local={local}, explicit-primary={explicit_primary}: {}",
                intersection.attribute("style").unwrap()
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), usize::from(!fallback_owned));
            assert_eq!(evidence.not_applicable_count(), usize::from(fallback_owned));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn venn_core_paints_reach_hand_drawn_hachure_and_outline_terminals() {
    let theme = venn_core_paint_theme(false, None);
    let source = format!(
        "---\nconfig:\n  look: handDrawn\n  handDrawnSeed: 7\n---\n{THEME_TYPOGRAPHY_SOURCE}"
    );
    let rendered = render_venn_with_theme_requirement(
        &source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    for circle in document.descendants().filter(|node| {
        node.attribute("class")
            .is_some_and(|class| class.split_whitespace().any(|class| class == "venn-circle"))
    }) {
        let paths = circle
            .children()
            .find(|node| node.has_tag_name("g"))
            .unwrap()
            .children()
            .filter(|node| node.has_tag_name("path"))
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 2);
        let fill = merman_markup::theme_color::transparentize("#123456", 0.7).unwrap();
        assert_eq!(paths[0].attribute("stroke"), Some(fill.as_str()));
        assert_eq!(paths[1].attribute("stroke"), Some("#345678"));
    }
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn venn_empty_compiled_theme_preserves_unthemed_svg_bytes() {
    let theme = venn_paint_theme([]);
    let environment = RenderEnvironment::deterministic();
    let render = |theme: Option<&DiagramTheme>| {
        let engine = match theme {
            Some(theme) => common::install_parse_compatibility(theme, MermaidEngine::new()),
            None => MermaidEngine::new(),
        };
        let parsed = engine
            .parse_diagram_for_render_model_sync(THEME_TYPOGRAPHY_SOURCE, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = match theme {
            Some(theme) => environment.begin_session_with_theme(theme).unwrap(),
            None => environment.begin_session().unwrap(),
        };
        family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("venn-theme".into()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
            )
            .unwrap()
            .svg()
            .to_owned()
    };
    assert_eq!(render(None).as_bytes(), render(Some(&theme)).as_bytes());
}

#[test]
fn venn_mixed_node_rule_accounts_supported_fill_and_the_actual_padding_winner() {
    use merman_render::diagram_theme::InsetsPx;
    for override_padding in [false, true] {
        let mut rules = vec![ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(solid("#b316cd"))
                .with_padding(InsetsPx::all(4.0)),
        )];
        if override_padding {
            rules.push(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_padding(InsetsPx::all(8.0)),
            ));
        }
        let rendered = render_venn_with_theme_requirement(
            THEME_TYPOGRAPHY_SOURCE,
            &venn_paint_theme(rules),
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(
            document
                .descendants()
                .filter(|node| node.attribute("class").is_some_and(|class| class
                    .split_whitespace()
                    .any(|class| class == "venn-circle")))
                .all(|node| node
                    .children()
                    .find(|node| node.has_tag_name("path"))
                    .unwrap()
                    .attribute("style")
                    .unwrap()
                    .contains("fill: #b316cd;"))
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(override_padding));
        assert_eq!(evidence.theme_residual_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
    }
}

#[test]
fn venn_label_stroke_is_residual_even_when_its_fill_is_consumed() {
    let source = "venn-beta\nset A[\"Alpha\"]:20\n";
    for mixed in [false, true] {
        for variant in [None, Some(ThemeVariant::Default)] {
            let mut style = ThemeStylePatch::default().with_stroke(solid("#ff00ff"));
            if mixed {
                style = style.with_fill(solid("#123456"));
            }
            let rule = ThemeRule::new(ThemeTarget::NodeLabel, style);
            let theme = venn_paint_theme([
                variant.map_or(rule.clone(), |variant| rule.with_variant(variant))
            ]);
            let rendered = render_venn_with_theme_requirement(
                source,
                &theme,
                MermaidEngine::new(),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let text = document
                .descendants()
                .find(|node| node.has_tag_name("text"))
                .unwrap();
            assert!(!text.attribute("style").unwrap().contains("stroke:"));
            if mixed {
                assert!(text.attribute("style").unwrap().contains("fill: #123456;"));
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(evidence.not_applicable_count(), 0);
            assert_eq!(evidence.theme_residual_count(), 1);
            let error = render_venn_with_theme_requirement(
                source,
                &theme,
                MermaidEngine::new(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .err()
            .unwrap();
            assert_eq!(
                error.unverified_family_theme(),
                Some((DiagramFamilyId::VENN, 1))
            );
        }
    }
    for (ordinal, variant, residual) in [
        (1, ThemeVariant::Default, true),
        (2, ThemeVariant::Default, false),
        (1, ThemeVariant::Warning, false),
    ] {
        let theme = venn_paint_theme([ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_stroke(solid("#ff00ff")),
        )
        .with_variant(variant)
        .with_ordinal(merman_render::diagram_theme::OrdinalSelector::exact(ordinal).unwrap())]);
        let rendered = render_venn_with_theme_requirement(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.theme_residual_count(), usize::from(residual));
        assert_eq!(evidence.not_applicable_count(), usize::from(!residual));
        assert_eq!(
            render_venn_with_theme_requirement(
                source,
                &theme,
                MermaidEngine::new(),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_err(),
            residual
        );
    }
}

fn solid(color: &str) -> CanvasPaint {
    CanvasPaint::solid(color).expect("valid Venn paint")
}

fn render_venn_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = common::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Venn")
        .expect("detect themed Venn")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Venn session");

    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("venn-theme".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn render_typed_venn(input: &str) -> (VennDiagramLayout, String) {
    render_typed_venn_with_engine(input, MermaidEngine::new())
}

fn render_typed_venn_with_engine(
    input: &str,
    engine: MermaidEngine,
) -> (VennDiagramLayout, String) {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    assert_eq!(parsed.metadata().diagram_type, "venn");

    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize Venn layout");
    let layout: VennDiagramLayout =
        serde_json::from_value(projection["layout"]["VennDiagram"].clone())
            .expect("Venn layout projection");
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("venn-test".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render SVG")
        .svg()
        .to_owned();

    (layout, svg)
}

fn try_render_venn_svg_with_resource_policy(
    input: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Venn resource-bound session");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse Venn resource-bound fixture")
        .expect("detect Venn resource-bound fixture")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("venn-bounded".to_string()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn venn_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "venn-beta\nset A[\"Alpha\"]:20\nset B[\"Beta\"]:12\nunion A,B[\"Shared\"]:3\n";
    let baseline = try_render_venn_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Venn baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Venn fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Venn SVG byte ceiling");
    let exact = try_render_venn_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Venn family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Venn SVG byte ceiling");
    let error = try_render_venn_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Venn family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Venn MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
}

#[test]
fn venn_typed_font_stack_reaches_every_local_text_selector_and_strict_receipt() {
    let font_stack =
        FontStack::new(["VennTyped", "monospace"]).expect("valid Venn typed font stack");
    let expected_font = font_stack.as_css().to_string();
    let theme = venn_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let rendered = render_venn_with_theme_requirement(
        THEME_TYPOGRAPHY_SOURCE,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render strict portable Venn font stack");

    for selector in [
        ".venn-title{",
        ".venn-circle text{",
        ".venn-intersection text{",
        ".venn-text-node{",
    ] {
        let start = rendered
            .svg()
            .find(selector)
            .expect("Venn selector must be emitted");
        let rule = &rendered.svg()[start..rendered.svg()[start..].find('}').unwrap() + start];
        assert!(
            rule.contains(&format!("font-family:{expected_font};")),
            "Venn selector `{selector}` must use the typed font stack: {rule}"
        );
    }

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn venn_theme_font_family_outranks_root_and_typed_font_family() {
    let theme = venn_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("VennTyped").expect("valid Venn typed font")),
    );
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "fontFamily": "VennRootFont",
            "themeVariables": { "fontFamily": "VennThemeFont" }
        })),
    );
    let rendered = render_venn_with_theme_requirement(
        THEME_TYPOGRAPHY_SOURCE,
        &theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render config-owned Venn font stack");

    assert!(rendered.svg().contains("font-family:VennThemeFont;"));
    assert!(!rendered.svg().contains("VennRootFont"));
    assert!(!rendered.svg().contains("VennTyped"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn venn_font_size_is_a_structured_unsupported_residual_without_a_legacy_projection() {
    let typography = ThemeTextStyle::default()
        .with_font_size_px(24.0)
        .expect("valid unsupported Venn font size");
    let theme = venn_typography_theme(typography);
    let rendered = render_venn_with_theme_requirement(
        THEME_TYPOGRAPHY_SOURCE,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort renders the structured Venn font-size residual");

    assert!(!rendered.svg().contains("font-size:24px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);

    let error = render_venn_with_theme_requirement(
        THEME_TYPOGRAPHY_SOURCE,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .err()
    .expect("RequirePortable rejects unsupported Venn font size");
    assert!(matches!(
        error,
        merman_render::Error::UnverifiedFamilyTheme {
            family_id: DiagramFamilyId::VENN,
            residual_count: 1,
        }
    ));
}

#[test]
fn venn_mixed_font_stack_and_size_fails_closed_without_legacy_projection() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single("VennMixed").expect("valid Venn mixed font stack"))
        .with_font_size_px(24.0)
        .expect("valid unsupported Venn mixed font size");
    let theme = venn_typography_theme(typography);
    let rendered = render_venn_with_theme_requirement(
        THEME_TYPOGRAPHY_SOURCE,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort renders the mixed Venn typography residual");

    assert!(rendered.svg().contains("VennMixed"));
    assert!(!rendered.svg().contains("font-size:24px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);

    let error = render_venn_with_theme_requirement(
        THEME_TYPOGRAPHY_SOURCE,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .err()
    .expect("RequirePortable rejects mixed unsupported Venn typography");
    assert!(matches!(
        error,
        merman_render::Error::UnverifiedFamilyTheme {
            family_id: DiagramFamilyId::VENN,
            residual_count: 1,
        }
    ));
}

#[test]
fn venn_typed_text_fill_reaches_visible_terminals() {
    let text_fill = "#123456";
    for variant in [None, Some(ThemeVariant::Default)] {
        let mut rule = ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(solid(text_fill)),
        )
        .for_family(DiagramFamilyId::VENN);
        if let Some(variant) = variant {
            rule = rule.with_variant(variant);
        }
        let theme = venn_paint_theme([rule]);
        let rendered = render_venn_with_theme_requirement(
            THEME_TYPOGRAPHY_SOURCE,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("render strict portable Venn text fill theme");

        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Venn SVG");
        let stylesheet = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Venn stylesheet");
        assert!(
            stylesheet.contains(".venn-intersection text{font-size:48px;fill:#123456;"),
            "variant={variant:?}"
        );
        assert!(stylesheet.contains(".venn-text-node{font-family:"));
        assert!(stylesheet.contains("color:#123456;"));
        assert!(
            rendered
                .svg()
                .contains("style=\"font-size: 24px; fill: #123456;\""),
            "variant={variant:?}"
        );

        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "variant={variant:?}");
        assert_eq!(evidence.applied_count(), 1, "variant={variant:?}");
        assert_eq!(evidence.not_applicable_count(), 0, "variant={variant:?}");
        assert_eq!(evidence.theme_residual_count(), 0, "variant={variant:?}");
    }
}

#[test]
fn venn_text_fill_is_not_applicable_when_config_or_source_owns_every_terminal() {
    let theme = venn_paint_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(solid("#123456")),
    )
    .for_family(DiagramFamilyId::VENN)]);
    let config_owned = render_venn_with_theme_requirement(
        THEME_TYPOGRAPHY_SOURCE,
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": { "vennSetTextColor": "#654321" }
            })),
        ),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("config-owned Venn text fill remains portable");
    assert!(config_owned.svg().contains("fill:#654321;"));
    assert!(!config_owned.svg().contains("fill:#123456;"));
    let evidence =
        merman_render::__private::family_evidence(config_owned.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);

    let source_owned = render_venn_with_theme_requirement(
        r#"venn-beta
set A["Alpha"]:20
  text A1["Nested Alpha"]
set B["Beta"]:12
union A,B["Shared"]:3
  text AB1["Nested Shared"]
style A,B color:#654321
style A1 color:#654321
style AB1 color:#654321
"#,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("source-owned Venn text fill remains portable");
    let evidence =
        merman_render::__private::family_evidence(source_owned.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn venn_text_fill_uses_final_venn_role_ownership() {
    for origin in ["recipe", "site", "source"] {
        for base in ["default", "base"] {
            let rule = ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(solid("#123456")),
            )
            .for_family(DiagramFamilyId::VENN);
            let spec = DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule));
            let spec = if origin != "recipe" {
                spec
            } else {
                spec.with_mermaid_compatibility(
                    MermaidThemeCompatibility::default()
                        .with_theme(base)
                        .unwrap(),
                )
            };
            let theme = DiagramThemeCompiler::new().compile(spec).unwrap();
            let mut config = serde_json::json!({"themeVariables":{"textColor":"#654321"}});
            if origin != "recipe" {
                config["theme"] = serde_json::json!(base);
            }
            let source = if origin == "source" {
                format!("%%{{init: {config}}}%%\n{THEME_TYPOGRAPHY_SOURCE}")
            } else {
                THEME_TYPOGRAPHY_SOURCE.to_owned()
            };
            let engine = if origin == "source" {
                MermaidEngine::new()
            } else {
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(config),
                )
            };
            let rendered = render_venn_with_theme_requirement(
                &source,
                &theme,
                engine,
                if origin == "recipe" {
                    ThemePortabilityRequirement::BestEffort
                } else {
                    ThemePortabilityRequirement::RequirePortable
                },
            )
            .unwrap();
            let owned = origin == "source" || base == "base";
            let expected = if base == "base" {
                "#654321"
            } else if origin == "source" {
                "#333"
            } else {
                "#123456"
            };
            assert!(
                rendered.svg().contains(&format!("fill:{expected};")),
                "origin={origin}, base={base}: {}",
                rendered.svg()
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(
                evidence.mermaid_compatibility_residual_count(),
                usize::from(origin == "recipe")
            );
            assert_eq!(evidence.applied_count(), usize::from(!owned));
            assert_eq!(evidence.not_applicable_count(), usize::from(owned));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn venn_text_fill_remains_applied_for_mixed_source_owned_terminals() {
    let theme = venn_paint_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(solid("#123456")),
    )
    .for_family(DiagramFamilyId::VENN)]);
    let rendered = render_venn_with_theme_requirement(
        r#"venn-beta
set A["Alpha"]:20
set B["Beta"]:12
union A,B["Shared"]:3
  text AB1["Nested Shared"]
style A,B color:#654321
"#,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("mixed source-owned Venn text terminals remain portable");

    let svg = rendered.svg();
    assert!(svg.contains("fill: #654321;"));
    assert!(svg.contains("color:#123456;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

fn find_venn_area<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    sets: &str,
    class: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-venn-sets") == Some(sets)
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|item| item == class))
        })
        .unwrap_or_else(|| panic!("Venn area `{sets}` with class `{class}`"))
}

#[test]
fn venn_typed_render_model_outputs_classic_svg_structure() {
    let input = r##"venn-beta
title Product Surface
set A["Core"]:20
set B["Editor"]:14
union A,B["Shared"]:4
"##;

    let (layout, svg) = render_typed_venn(input);
    assert_eq!(layout.areas.len(), 3);
    assert!(svg.contains(r#"aria-roledescription="venn""#));
    assert!(svg.contains(r#"viewBox="0 0 800 450""#));
    assert!(svg.contains(r#"<text class="venn-title""#));
    assert!(svg.contains(">Product Surface</text>"));
    assert!(svg.contains(r#"<g transform="translate(0, 24)">"#));
    assert!(svg.contains(r#"class="venn-area venn-circle venn-set-0""#));
    assert!(svg.contains(r#"class="venn-area venn-circle venn-set-1""#));
    assert!(svg.contains(r#"class="venn-area venn-intersection""#));
    assert!(svg.contains(r#"data-venn-sets="A_B""#));
    assert!(svg.contains(">Core</tspan></text>"));
    assert!(svg.contains(">Shared</tspan></text>"));
}

#[test]
fn venn_non_default_theme_rederives_title_and_set_text_colors() {
    let input = r##"%%{init: {"theme": "base", "themeVariables": {"titleColor": "#123456", "textColor": "#abcdef"}}}%%
venn-beta
title Dependency Surface
set A["Alpha"]:20
set B["Beta"]:12
union A,B["Shared"]:3
"##;

    let (_, svg) = render_typed_venn_with_engine(
        input,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "secure": []
            })),
        ),
    );

    assert!(
        svg.contains(".venn-title{font-size:32px;fill:#123456;"),
        "Venn stylesheet title fill must use the non-Default calculated dependency: {svg}"
    );
    assert!(
        svg.contains(r#"class="venn-title" font-size="16px" text-anchor="middle" dominant-baseline="middle" x="50%" y="16" style="fill: #123456;""#),
        "Venn title terminal must use the same calculated dependency: {svg}"
    );
    assert!(
        svg.contains(".venn-intersection text{font-size:48px;fill:#abcdef;"),
        "Venn set text must use the non-Default textColor dependency: {svg}"
    );
}

#[test]
fn venn_default_theme_preserves_constructor_snapshot_colors() {
    let input = r##"%%{init: {"theme": "default", "themeVariables": {"titleColor": "#123456", "textColor": "#abcdef"}}}%%
venn-beta
title Constructor Snapshot
set A["Alpha"]:20
set B["Beta"]:12
union A,B["Shared"]:3
"##;

    let (_, svg) = render_typed_venn_with_engine(
        input,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "secure": []
            })),
        ),
    );

    assert!(
        svg.contains(".venn-title{font-size:32px;fill:#333;"),
        "Default Venn title must retain its constructor snapshot: {svg}"
    );
    assert!(
        svg.contains(r#"class="venn-title" font-size="16px" text-anchor="middle" dominant-baseline="middle" x="50%" y="16" style="fill: #333;""#),
        "Default Venn terminal title must retain its constructor snapshot: {svg}"
    );
    assert!(
        svg.contains(".venn-intersection text{font-size:48px;fill:#333;"),
        "Default Venn set text must retain its constructor snapshot: {svg}"
    );
}

#[test]
fn venn_styles_and_text_nodes_render_inline_overrides() {
    let input = r##"%%{init: {"venn": {"useDebugLayout": true}, "themeVariables": {"vennSetTextColor": "#222222"}}}%%
venn-beta
set A["Frontend"]:20
  text A1["React"]
set B["Backend"]:16
union A,B["API"]:5
  text AB1["OpenAPI"]
style A fill:#ff6b6b, color:#101010, stroke:#202020, stroke-width:7, fill-opacity:0.42
style A,B fill:#00ffcc, color:#003333
style A1 color:#123456
"##;

    let (_layout, svg) = render_typed_venn(input);

    assert!(svg.contains(r#"style="fill: #ff6b6b; fill-opacity: 0.42; stroke: #202020; stroke-width: 7; stroke-opacity: 0.95;""#));
    assert!(svg.contains(r#"style="font-size: 24px; fill: #101010;""#));
    assert!(svg.contains(r#"style="fill-opacity: 1; fill: #00ffcc;""#));
    assert!(svg.contains(r#"style="font-size: 24px; fill: #003333;""#));
    assert!(svg.contains(r#"<g class="venn-text-nodes">"#));
    assert!(svg.contains(r#"<g class="venn-text-area" font-size="20px">"#));
    assert!(svg.contains(r#"class="venn-text-debug-circle""#));
    assert!(svg.contains(r#"class="venn-text-debug-cell""#));
    assert!(svg.contains(r#"<foreignObject class="venn-text-node-fo""#));
    assert!(svg.contains(r#"<span xmlns="http://www.w3.org/1999/xhtml" class="venn-text-node""#));
    assert!(svg.contains("color: #123456;\">React</span>"));
}

#[test]
fn venn_canonical_typed_path_renders_svg() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            r##"%%{init: {"venn": {"useMaxWidth": false, "width": 640, "height": 360}}}%%
venn-beta
set A
set B
union A,B
"##,
            ParseOptions::strict(),
        )
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    assert_eq!(parsed.metadata().diagram_type, "venn");

    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");
    assert_eq!(artifact.family_id(), merman_render::DiagramFamilyId::VENN);
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render SVG")
        .svg()
        .to_owned();

    assert!(svg.contains(r#"aria-roledescription="venn""#));
    assert!(svg.contains(r#"viewBox="0 0 640 360""#));
    assert!(!svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"height="360""#));
    assert!(svg.contains(r#"class="venn-area venn-intersection""#));
}

#[test]
fn venn_typed_layout_and_canonical_renderer_agree() {
    let input = r##"venn-beta
set A
set B
union A,B
"##;

    let (layout, svg) = render_typed_venn(input);

    assert_eq!(layout.areas.len(), 3);
    assert!(svg.contains(r#"aria-roledescription="venn""#));
    assert!(svg.contains(r#"class="venn-area venn-circle venn-set-0""#));
}

#[test]
fn venn_repeated_union_members_keep_upstream_constraints_and_finite_geometry() {
    let input = r##"venn-beta
set A["Alpha"]:20
set B["Beta"]:12
union A,A,B["Repeated"]:3
"##;

    let (layout, svg) = render_typed_venn(input);
    let (repeated_layout, repeated_svg) = render_typed_venn(input);

    // Mermaid 11.16 enumerates member positions, then de-duplicates pair keys. Therefore A,A,B
    // contributes exactly the A,A self-pair and the A,B pair to the layout-only constraints.
    assert_eq!(
        layout
            .areas
            .iter()
            .map(|area| (area.sets.join("|"), area.size))
            .collect::<Vec<_>>(),
        [
            ("A".to_string(), 20.0),
            ("B".to_string(), 12.0),
            ("A|A|B".to_string(), 3.0),
            ("A|A".to_string(), 5.0),
            ("A|B".to_string(), 3.0),
        ]
    );
    assert_eq!(
        serde_json::to_value(&layout).expect("serialize first typed layout"),
        serde_json::to_value(&repeated_layout).expect("serialize repeated typed layout"),
        "the source-backed Venn optimizer must remain deterministic"
    );
    assert_eq!(svg, repeated_svg);

    for area in &layout.areas {
        assert!(area.size.is_finite(), "non-finite size for {:?}", area.sets);
        assert!(
            area.text_x.is_finite() && area.text_y.is_finite(),
            "non-finite label position for {:?}",
            area.sets
        );
        assert!(!area.path.is_empty(), "missing path for {:?}", area.sets);
        assert!(
            !area.path.contains("NaN") && !area.path.contains("inf"),
            "non-finite path for {:?}: {}",
            area.sets,
            area.path
        );
        assert_eq!(area.circles.len(), area.sets.len());
        for circle in &area.circles {
            assert!(
                circle.x.is_finite()
                    && circle.y.is_finite()
                    && circle.radius.is_finite()
                    && circle.radius > 0.0,
                "non-finite circle for {:?}: {circle:?}",
                area.sets
            );
        }
    }

    let document = roxmltree::Document::parse(&svg).expect("valid repeated-member Venn SVG");
    let self_pair = find_venn_area(&document, "A_A", "venn-intersection");
    let repeated_union = find_venn_area(&document, "A_A_B", "venn-intersection");
    for area in [self_pair, repeated_union] {
        let path = area
            .children()
            .find(|child| child.has_tag_name("path"))
            .and_then(|path| path.attribute("d"))
            .expect("rendered intersection path");
        assert!(!path.is_empty());
        assert!(!path.contains("NaN") && !path.contains("inf"));
    }
    assert_eq!(svg.matches(">Repeated</tspan></text>").count(), 1);
}

#[test]
fn venn_hand_drawn_generation_consumes_the_operation_work_budget() {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 20_000)
        .unwrap();
    let source = "---\nconfig:\n  look: classic\n  handDrawnSeed: 1\n  venn:\n    width: 4096\n    height: 4096\n---\nvenn-beta\nset A\n";
    try_render_venn_svg_with_resource_policy(source, policy)
        .expect("the small model and classic geometry fit the work budget");
    let error = match try_render_venn_svg_with_resource_policy(
        &source.replace("look: classic", "look: handDrawn"),
        policy,
    ) {
        Ok(_) => panic!("hachure generation must consume the remaining operation work budget"),
        Err(error) => error,
    };
    let merman_render::Error::ResourceLimitExceeded(details) = error else {
        panic!("expected generation work rejection, got {error}");
    };
    assert_eq!(details.limit, ResourceLimitId::MaxLayoutWorkUnits.as_str());
    assert_eq!(details.cause, ResourceLimitCause::Ceiling);
    assert_eq!(details.phase, ResourceLimitPhase::LayoutModel);
}

#[test]
fn venn_hand_drawn_replaces_styled_areas_with_seeded_rough_paths() {
    let input = r##"---
config:
  look: handDrawn
  handDrawnSeed: 1
---
venn-beta
set A
set B
union A,B
style A fill:#ff6b6b,stroke:#202020,stroke-width:7
style A,B fill:#ffe66d,color:#003333
"##;

    let (_, svg) = render_typed_venn(input);
    let document = roxmltree::Document::parse(&svg).expect("valid Venn SVG");
    let circle_a = find_venn_area(&document, "A", "venn-circle");
    let circle_a_children: Vec<_> = circle_a
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(circle_a_children.len(), 2);
    assert!(circle_a_children[0].has_tag_name("g"));
    assert!(circle_a_children[1].has_tag_name("text"));

    let circle_a_paths: Vec<_> = circle_a_children[0]
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(circle_a_paths.len(), 2);
    assert!(circle_a_paths.iter().all(|path| path.has_tag_name("path")));
    assert_eq!(
        circle_a_paths[0].attribute("stroke"),
        Some("rgba(255, 107, 107, 0.3)")
    );
    assert_eq!(circle_a_paths[0].attribute("stroke-width"), Some("2"));
    assert_eq!(circle_a_paths[0].attribute("fill"), Some("none"));
    assert_eq!(circle_a_paths[1].attribute("stroke"), Some("#202020"));
    assert_eq!(circle_a_paths[1].attribute("stroke-width"), Some("7"));
    assert_eq!(circle_a_paths[1].attribute("fill"), Some("none"));

    let circle_b = find_venn_area(&document, "B", "venn-circle");
    let circle_b_fill_d = circle_b
        .children()
        .find(roxmltree::Node::is_element)
        .and_then(|rough_group| rough_group.children().find(roxmltree::Node::is_element))
        .and_then(|path| path.attribute("d"))
        .expect("set B rough fill path");
    assert_ne!(circle_a_paths[0].attribute("d"), Some(circle_b_fill_d));

    let intersection = find_venn_area(&document, "A_B", "venn-intersection");
    let intersection_children: Vec<_> = intersection
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(intersection_children.len(), 2);
    assert!(intersection_children[0].has_tag_name("g"));
    assert!(intersection_children[1].has_tag_name("text"));
    let intersection_paths: Vec<_> = intersection_children[0]
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(intersection_paths.len(), 1);
    assert_eq!(
        intersection_paths[0].attribute("stroke"),
        Some("rgba(255, 230, 109, 0.7)")
    );
    assert_eq!(intersection_paths[0].attribute("stroke-width"), Some("2"));
    assert_eq!(intersection_paths[0].attribute("fill"), Some("none"));

    let (_, repeated_svg) = render_typed_venn(input);
    assert_eq!(svg, repeated_svg);

    let (_, other_seed_svg) =
        render_typed_venn(&input.replace("handDrawnSeed: 1", "handDrawnSeed: 2"));
    let other_seed_document =
        roxmltree::Document::parse(&other_seed_svg).expect("valid Venn SVG for another seed");
    let other_seed_circle_a_fill_d = find_venn_area(&other_seed_document, "A", "venn-circle")
        .children()
        .find(roxmltree::Node::is_element)
        .and_then(|rough_group| rough_group.children().find(roxmltree::Node::is_element))
        .and_then(|path| path.attribute("d"))
        .expect("set A rough fill path for another seed");
    assert_ne!(
        circle_a_paths[0].attribute("d"),
        Some(other_seed_circle_a_fill_d)
    );
}

#[test]
fn venn_hand_drawn_keeps_unstyled_intersection_path_transparent() {
    let input = r##"---
config:
  look: handDrawn
  handDrawnSeed: 1
---
venn-beta
set A
set B
union A,B
"##;

    let (_, svg) = render_typed_venn(input);
    let document = roxmltree::Document::parse(&svg).expect("valid Venn SVG");
    let circle_a = find_venn_area(&document, "A", "venn-circle");
    let circle_a_fill = circle_a
        .children()
        .find(roxmltree::Node::is_element)
        .and_then(|rough_group| rough_group.children().find(roxmltree::Node::is_element))
        .expect("set A rough fill path");
    assert_eq!(
        circle_a_fill.attribute("stroke"),
        Some("rgba(232, 121, 249, 0.3)")
    );

    let intersection = find_venn_area(&document, "A_B", "venn-intersection");
    let children: Vec<_> = intersection
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();

    assert_eq!(children.len(), 2);
    assert!(children[0].has_tag_name("path"));
    assert_eq!(children[0].attribute("style"), Some("fill-opacity: 0;"));
    assert!(children[1].has_tag_name("text"));
}

#[test]
fn venn_font_ampersand_is_xml_text_encoded_without_changing_css() {
    let stack = FontStack::new(["A&B", "monospace"]).unwrap();
    let expected = stack.as_css();
    let theme = venn_typography_theme(ThemeTextStyle::default().with_font_stack(stack));
    let rendered = render_venn_with_theme_requirement(
        THEME_TYPOGRAPHY_SOURCE,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    let document = roxmltree::Document::parse(rendered.svg())
        .expect("font ampersand must produce valid SVG XML");
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .unwrap();
    assert!(css.contains(&format!("font-family:{expected};")));
    assert!(rendered.svg().contains("A&amp;B"));
    assert_eq!(
        merman_render::__private::family_evidence(rendered.into_completion().report())
            .applied_count(),
        1
    );
}

#[allow(dead_code)]
mod common;
