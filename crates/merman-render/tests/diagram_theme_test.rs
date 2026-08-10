use merman_render::diagram_theme::{
    CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec, FontCatalogSpec, FontStack,
    TextLayoutCapability, TextTransform, ThemeAssets, ThemeCapability, ThemeColorValue,
    ThemePreset, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle,
    ThemeTokens, ThemeVariant, TypographySpec, theme_preset_descriptors,
};
use merman_render::family::RenderFamilyKind;

#[test]
fn theme_catalog_contains_only_visual_presets() {
    let ids = theme_preset_descriptors()
        .iter()
        .map(|descriptor| descriptor.id())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        [
            "editor-light",
            "editor-dark",
            "one-dark",
            "gruvbox-light",
            "gruvbox-dark",
            "ayu-light",
            "ayu-dark",
        ]
    );
    assert_eq!(ThemePreset::ALL.len(), 7);
    assert!(ThemePreset::from_id("merman-modern").is_err());
}

#[test]
fn built_in_presets_compile_without_selecting_layout_or_look() {
    for preset in ThemePreset::ALL {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(preset)
            .expect("built-in theme preset should compile");
        let config = theme.mermaid_config();

        assert_eq!(config.get_str("theme"), Some("base"));
        assert_eq!(
            config.get_bool("darkMode"),
            Some(preset.is_dark()),
            "{} dark-mode compatibility value",
            preset.id()
        );
        assert_eq!(config.get_str("look"), None);
        assert_eq!(config.get_str("flowchart.defaultRenderer"), None);
    }
}

#[test]
fn built_in_presets_publish_stable_semantic_representatives() {
    let expected = [
        (ThemePreset::EditorLight, "#ffffff", "#64748b", "#2563eb"),
        (ThemePreset::EditorDark, "#0f172a", "#94a3b8", "#60a5fa"),
        (ThemePreset::OneDark, "#282c34", "#61afef", "#61afef"),
        (ThemePreset::GruvboxLight, "#fbf1c7", "#7c6f64", "#458588"),
        (ThemePreset::GruvboxDark, "#282828", "#d5c4a1", "#83a598"),
        (ThemePreset::AyuLight, "#fcfcfc", "#5c6166", "#55b4d4"),
        (ThemePreset::AyuDark, "#0b0e14", "#59c2ff", "#59c2ff"),
    ];

    for (preset, canvas, line, first_series) in expected {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(preset)
            .expect("built-in theme preset should compile");
        assert_eq!(
            solid_color(theme.spec().canvas().base()).as_deref(),
            Some(canvas)
        );

        let flowchart = theme.resolve(RenderFamilyKind::Flowchart);
        assert_eq!(
            flowchart
                .style(ThemeTarget::Edge, ThemeVariant::Default, None)
                .stroke()
                .and_then(solid_color)
                .as_deref(),
            Some(line)
        );
        let chart = theme.resolve(RenderFamilyKind::XyChart);
        assert_eq!(
            chart
                .series_color(ThemeTarget::ChartSeries, 1)
                .map(ThemeColorValue::as_css)
                .as_deref(),
            Some(first_series)
        );
    }
}

#[test]
fn compiled_theme_is_reusable_and_custom_tokens_fail_closed() {
    let spec = ThemeTokens::default()
        .with_canvas("#010203")
        .expect("valid canvas")
        .into_theme_spec();
    let compiler = DiagramThemeCompiler::new();
    let first = compiler.compile(spec.clone()).expect("compile theme");
    let second = compiler.compile(spec).expect("compile theme again");

    assert_eq!(first.recipe_fingerprint(), second.recipe_fingerprint());
    assert!(
        ThemeTokens::default()
            .with_canvas("white; color: red")
            .is_err()
    );
}

#[test]
fn custom_catalog_implies_prepared_text_capabilities() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let spec = DiagramThemeSpec::new().with_assets(ThemeAssets::default().with_font_catalog(
        FontCatalogSpec::new([FontAssetSpec::new("excalifont", bytes)]),
    ));
    let theme = DiagramThemeCompiler::new()
        .compile(spec)
        .expect("custom catalog theme should compile");

    for capability in [
        TextLayoutCapability::CatalogBinding,
        TextLayoutCapability::UnicodeClusterFallback,
        TextLayoutCapability::OpenTypeShaping,
    ] {
        assert!(
            theme.report().requires_text_capability(capability),
            "custom catalog must require {capability}"
        );
    }
}

#[test]
fn typography_defaults_and_semantic_paints_have_fine_grained_requirements() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single("Excalifont").expect("font stack is valid"))
        .with_transform(TextTransform::Uppercase)
        .with_word_spacing_px(2.0)
        .expect("word spacing is finite");
    let style = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
    let spec = DiagramThemeSpec::new()
        .with_typography(TypographySpec::default().with_default(typography))
        .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, style)));
    let theme = DiagramThemeCompiler::new()
        .compile(spec)
        .expect("theme should compile");

    assert!(
        theme
            .report()
            .requires_capability(ThemeCapability::Typography)
    );
    assert!(
        theme
            .report()
            .requires_capability(ThemeCapability::TextTransform)
    );
    assert!(
        theme
            .report()
            .requires_capability(ThemeCapability::WordSpacing)
    );
    assert!(
        theme
            .report()
            .requires_capability(ThemeCapability::SolidPaint)
    );
}

fn solid_color(paint: &CanvasPaint) -> Option<String> {
    match paint {
        CanvasPaint::Solid(color) => Some(color.as_css()),
        _ => None,
    }
}
