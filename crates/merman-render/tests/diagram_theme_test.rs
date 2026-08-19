use merman_render::diagram_theme::{
    CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec, FontCatalogSpec, FontStack,
    TextLayoutCapability, TextTransform, ThemeAssets, ThemeCapability, ThemePreset, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
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
            "brutalist",
            "spotless",
            "cyberpunk",
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
fn preset_catalog_ids_round_trip_through_all_ten_enum_projections() {
    const EDITOR_LIGHT_ID: &str = ThemePreset::EditorLight.id();
    const EDITOR_DARK_IS_DARK: bool = ThemePreset::EditorDark.is_dark();
    const PRESET_COUNT: usize = theme_preset_descriptors().len();

    assert_eq!(EDITOR_LIGHT_ID, "editor-light");
    assert!(EDITOR_DARK_IS_DARK);
    assert_eq!(PRESET_COUNT, 10);

    let descriptors = theme_preset_descriptors();
    assert_eq!(descriptors.len(), 10);
    let expected_display_names = [
        "Editor Light",
        "Editor Dark",
        "One Dark",
        "Gruvbox Light",
        "Gruvbox Dark",
        "Ayu Light",
        "Ayu Dark",
        "Brutalist",
        "Spotless",
        "Cyberpunk",
    ];

    for (descriptor, expected_display_name) in descriptors.iter().zip(expected_display_names) {
        let parsed = ThemePreset::from_id(descriptor.id()).expect("catalog ID must parse");
        assert_eq!(parsed, descriptor.preset());
        assert_eq!(parsed.id(), descriptor.id());
        assert_eq!(descriptor.display_name(), expected_display_name);
        assert_eq!(descriptor.maturity(), "alpha");
        assert!(descriptor.qualified_cells().is_empty());
    }
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
