use merman::diagram_theme::{
    DiagramThemeCompiler, ThemeCanvasPaintWireV1, ThemeColorTokenV1, ThemeDefinitionV1,
    ThemeMaterializer, ThemeTokensV1,
};

#[test]
fn versioned_authoring_materializes_and_compiles_through_the_rust_facade() {
    let definition = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_color(ThemeColorTokenV1::Canvas, "#0f172a")
            .with_color(ThemeColorTokenV1::Surface, "#111827")
            .with_color(ThemeColorTokenV1::Text, "#e5e7eb"),
    );
    let materialized = ThemeMaterializer::new()
        .materialize_theme(&definition)
        .expect("versioned authoring should materialize");
    let spec = materialized.into_spec();
    let ThemeCanvasPaintWireV1::Color(canvas) = spec
        .canvas
        .as_ref()
        .and_then(|canvas| canvas.base.as_ref())
        .expect("the authored canvas token should materialize")
    else {
        panic!("the authored canvas token should materialize to a solid color");
    };
    assert_eq!(canvas, "#0f172a");

    DiagramThemeCompiler::new()
        .compile_spec_wire(spec)
        .expect("the materialized complete spec should compile");
}
