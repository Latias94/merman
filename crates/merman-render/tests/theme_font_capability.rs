//! Resource-free themes do not require the embedded-font capability.

use merman_render::diagram_theme::{
    DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec, FontCatalogError, FontCatalogSpec,
    FontStack, ThemeAssets, ThemeCompileError, ThemeTextStyle, TypographySpec,
    theme_preset_descriptors,
};

#[cfg(not(feature = "embedded-fonts"))]
#[test]
fn embedded_font_bytes_require_the_explicit_capability() {
    let result = DiagramThemeCompiler::new().compile(embedded_theme());
    assert!(
        result.is_err(),
        "feature-disabled compilation must not decode embedded fonts"
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
fn artifact_policy_cannot_be_widened_by_an_enabled_dependency() {
    let compiler = DiagramThemeCompiler::new()
        .with_embedded_fonts_allowed(false)
        .with_embedded_fonts_allowed(true);
    assert!(matches!(
        compiler.compile(embedded_theme()),
        Err(ThemeCompileError::FontCatalog(
            FontCatalogError::EmbeddedFontsUnavailable
        ))
    ));
}

#[test]
fn named_fonts_and_public_presets_do_not_require_embedded_bytes() {
    let compiler = DiagramThemeCompiler::new().with_embedded_fonts_allowed(false);
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

#[cfg(feature = "embedded-fonts")]
#[test]
fn enabled_capability_keeps_the_native_font_catalog_available() {
    let theme = DiagramThemeCompiler::new()
        .compile(embedded_theme())
        .unwrap();
    assert!(!theme.font_catalog().faces().is_empty());
}
