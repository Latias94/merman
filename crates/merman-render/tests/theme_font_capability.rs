//! Theme typography accepts font names and rejects embedded font resources.

use merman_render::diagram_theme::{
    DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec, FontCatalogError, FontCatalogSpec,
    FontStack, ThemeAssets, ThemeCompileError, ThemeTextStyle, TypographySpec,
    theme_preset_descriptors,
};

#[test]
fn embedded_font_bytes_are_rejected() {
    let result = DiagramThemeCompiler::new().compile(embedded_theme());
    assert!(
        result.is_err(),
        "theme compilation must reject embedded font resources"
    );
    assert!(matches!(
        result,
        Err(ThemeCompileError::FontCatalog(
            FontCatalogError::EmbeddedFontsUnavailable
        ))
    ));
}

fn embedded_theme() -> DiagramThemeSpec {
    DiagramThemeSpec::new().with_assets(ThemeAssets::default().with_font_catalog(
        FontCatalogSpec::new([FontAssetSpec::new(
            "excalifont",
            include_bytes!("../../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"),
        )]),
    ))
}

#[test]
fn named_fonts_and_public_presets_do_not_require_embedded_bytes() {
    let compiler = DiagramThemeCompiler::new();
    compiler
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_default(
                    ThemeTextStyle::default()
                        .with_font_stack(FontStack::single("Caller Font").unwrap()),
                ),
            ),
        )
        .unwrap();
    for preset in theme_preset_descriptors() {
        compiler.compile_preset(preset.preset()).unwrap();
    }
}

#[cfg(feature = "all-diagrams")]
#[test]
fn absolute_theme_sizes_keep_configuration_priority_and_layout_across_families() {
    use merman_core::presentation::{SourcePresentationConfig, ThemeCompatibilityPlan};
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_render::environment::RenderEnvironment;
    use merman_render::family;
    use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
    use merman_render::{DiagramFamilyId, LayoutOptions};

    for (family_id, source) in [
        (
            DiagramFamilyId::JOURNEY,
            "journey\n  section Plan\n    Task: 5: Alice\n",
        ),
        (
            DiagramFamilyId::TIMELINE,
            "timeline\nsection Plan\nTask : Event\n",
        ),
        (
            DiagramFamilyId::MINDMAP,
            "mindmap\n  root[Root]\n    child[Child]\n",
        ),
        (
            DiagramFamilyId::ARCHITECTURE,
            "architecture-beta\nservice api(server)[API]\n",
        ),
        (
            DiagramFamilyId::C4,
            "C4Context\ntitle Root Size\nPerson(alice, \"Alice\")\n",
        ),
        (
            DiagramFamilyId::GIT_GRAPH,
            "gitGraph\ncommit\nbranch topic\ncommit\n",
        ),
        (DiagramFamilyId::ER, "erDiagram\nACCOUNT {\n string id\n}\n"),
        (
            DiagramFamilyId::CLASS,
            "classDiagram\nclass Account {\n +String id\n}\n",
        ),
        (
            DiagramFamilyId::KANBAN,
            "kanban\n  todo[Todo]\n    task[Task]\n",
        ),
    ] {
        for typed in [false, true] {
            // Mindmap does not admit a typed base size. Its inherited config path still needs
            // the same conversion for label measurement and the common stylesheet.
            if typed && family_id == DiagramFamilyId::MINDMAP {
                continue;
            }
            let spec = if typed {
                DiagramThemeSpec::new().with_typography(
                    TypographySpec::default().with_family_style(
                        family_id,
                        ThemeTextStyle::default().with_font_size_px(31.0).unwrap(),
                    ),
                )
            } else {
                DiagramThemeSpec::new()
            };
            let theme = DiagramThemeCompiler::new().compile(spec).unwrap();
            let render = |size: &str| {
                let plan =
                    ThemeCompatibilityPlan::from_recipe(theme.compatibility_recipe().clone());
                let mut config = serde_json::json!({
                    "look": "classic", "themeVariables": {"fontSize": size}
                });
                if family_id == DiagramFamilyId::MINDMAP {
                    config["layout"] = "tidy-tree".into();
                }
                let parsed = MermaidEngine::new()
                    .with_theme_plan(&plan)
                    .with_site_config(SourcePresentationConfig::from_value(config))
                    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                let session = RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .unwrap();
                let prepared =
                    family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
                let layout = prepared.layout_json().unwrap()["layout"].clone();
                let rendered = prepared
                    .render_svg(
                        &SvgRenderOptions {
                            diagram_id: Some("absolute-font".into()),
                            ..Default::default()
                        },
                        &SvgDebugOptions::default(),
                    )
                    .unwrap();
                (layout, rendered.svg().to_owned())
            };
            let (px_layout, px_svg) = render("32px");
            let (pt_layout, pt_svg) = render("24pt");
            assert!(
                px_svg.contains("font-size:32px;"),
                "{family_id:?}, typed={typed}"
            );
            assert_eq!(pt_layout, px_layout, "{family_id:?} layout, typed={typed}");
            if matches!(
                family_id,
                DiagramFamilyId::CLASS | DiagramFamilyId::TIMELINE
            ) {
                assert!(
                    pt_svg.contains("font-size:24pt;"),
                    "{family_id:?} preserves source CSS while measuring its absolute used size"
                );
            } else {
                assert_eq!(pt_svg, px_svg, "{family_id:?} SVG, typed={typed}");
            }
        }
    }
}
