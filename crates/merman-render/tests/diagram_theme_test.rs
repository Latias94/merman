use merman_render::diagram_theme::{
    CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, FontStack, TextTransform, ThemeCapability,
    ThemePreset, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle,
    TypographySpec, theme_preset_descriptors,
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
            "ghibli",
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
fn preset_catalog_ids_round_trip_through_enum_projections() {
    const EDITOR_LIGHT_ID: &str = ThemePreset::EditorLight.id();
    const EDITOR_DARK_IS_DARK: bool = ThemePreset::EditorDark.is_dark();

    assert_eq!(EDITOR_LIGHT_ID, "editor-light");
    const { assert!(EDITOR_DARK_IS_DARK) };

    let descriptors = theme_preset_descriptors();
    assert_eq!(
        descriptors
            .iter()
            .map(|descriptor| (descriptor.preset(), descriptor.display_name()))
            .collect::<Vec<_>>(),
        [
            (ThemePreset::EditorLight, "Editor Light"),
            (ThemePreset::EditorDark, "Editor Dark"),
            (ThemePreset::OneDark, "One Dark"),
            (ThemePreset::GruvboxLight, "Gruvbox Light"),
            (ThemePreset::GruvboxDark, "Gruvbox Dark"),
            (ThemePreset::AyuLight, "Ayu Light"),
            (ThemePreset::AyuDark, "Ayu Dark"),
            (ThemePreset::Brutalist, "Brutalist"),
            (ThemePreset::Spotless, "Spotless"),
            (ThemePreset::Cyberpunk, "Cyberpunk"),
            (ThemePreset::Ghibli, "Ghibli"),
        ]
    );

    for descriptor in descriptors {
        let parsed = ThemePreset::from_id(descriptor.id()).expect("catalog ID must parse");
        assert_eq!(parsed, descriptor.preset());
        assert_eq!(parsed.id(), descriptor.id());
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
