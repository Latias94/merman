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
