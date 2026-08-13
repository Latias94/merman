use merman_render::diagram_theme::{
    CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec, FontCatalogSpec, FontStack,
    TextLayoutCapability, TextTransform, ThemeAssets, ThemeCapability, ThemePreset, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeTokens, TypographySpec,
    theme_preset_descriptors,
};

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
    assert!(ThemePreset::from_id("merman-modern").is_err());
    assert!(
        theme_preset_descriptors()
            .iter()
            .all(|descriptor| descriptor.maturity() == "alpha")
    );
}

#[test]
fn built_in_presets_compile_without_selecting_layout_or_look() {
    for descriptor in theme_preset_descriptors() {
        DiagramThemeCompiler::new()
            .compile_preset(descriptor.preset())
            .expect("built-in theme preset should compile");
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
