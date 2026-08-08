use merman::Engine;
use merman::svg::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec, FontCatalogSpec,
    FontSourcePolicy, HeadlessRenderer, RenderEnvironment, RenderExecutionPath, ThemeAssets,
    ThemePreset,
};

fn compile_preset(preset: ThemePreset) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile_preset(preset)
        .expect("theme preset should compile")
}

fn embedded_font_theme() -> DiagramTheme {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let catalog = FontCatalogSpec::new([FontAssetSpec::new("excalifont", bytes)]);
    let spec =
        DiagramThemeSpec::new().with_assets(ThemeAssets::default().with_font_catalog(catalog));

    DiagramThemeCompiler::new()
        .with_font_source_policy(FontSourcePolicy::embedded_only())
        .compile(spec)
        .expect("embedded font theme should compile")
}

#[test]
fn rendered_document_keeps_svg_resources_and_operation_evidence_correlated() {
    let document = HeadlessRenderer::new()
        .render_document_sync("info")
        .unwrap()
        .expect("info diagram");

    assert!(document.svg_text().starts_with("<svg"));
    assert!(document.resource_closure().is_closed());
    assert_eq!(
        document.report().execution_path(),
        RenderExecutionPath::HeadlessOperationTyped
    );
    assert_eq!(
        document.font_catalog().fingerprint(),
        document.report().font_catalog_fingerprint()
    );
    assert_eq!(
        document.font_source_policy(),
        document.report().font_source_policy()
    );
    assert_eq!(
        document.resource_fingerprint(),
        document.svg().resource_fingerprint()
    );
    assert_eq!(document.theme_fingerprint(), None);
    assert_eq!(document.theme_resolution_report(), None);
}

#[test]
fn themed_renderer_operations_freeze_one_theme_resource_identity() {
    const SOURCE: &str = "sequenceDiagram\nAlice->>Bob: Hello";

    let environment_theme = embedded_font_theme();
    let selected_theme = compile_preset(ThemePreset::OneDark);
    let environment = RenderEnvironment::deterministic()
        .with_font_catalog(environment_theme.font_catalog().clone())
        .with_font_source_policy(FontSourcePolicy::embedded_only());
    let renderer = HeadlessRenderer::from_engine_and_environment(Engine::new(), environment)
        .with_theme(selected_theme.clone());

    renderer
        .parse_metadata_sync(SOURCE)
        .expect("direct metadata parse should use the renderer operation session");
    renderer
        .parse_diagram_sync(SOURCE)
        .expect("direct diagram parse should use the renderer operation session")
        .expect("sequence diagram should be detected");
    let document = renderer
        .render_document_sync(SOURCE)
        .expect("document render should succeed")
        .expect("sequence diagram should render");

    assert_eq!(
        document.theme_fingerprint(),
        Some(selected_theme.fingerprint())
    );
    assert_eq!(
        document.report().theme_fingerprint(),
        Some(selected_theme.fingerprint())
    );
    assert_eq!(
        document.theme_resolution_report(),
        Some(selected_theme.report())
    );
    assert_eq!(
        document.report().theme_resolution_report(),
        Some(selected_theme.report())
    );
    assert_eq!(
        selected_theme.report().font_catalog_fingerprint(),
        document.report().font_catalog_fingerprint()
    );
    assert_eq!(
        document.report().font_catalog_fingerprint(),
        document.font_catalog().fingerprint()
    );
    assert_eq!(
        document.font_source_policy(),
        selected_theme.font_source_policy()
    );
    assert_ne!(
        document.font_catalog().fingerprint(),
        environment_theme.font_catalog().fingerprint(),
        "a selected theme must not retain the environment catalog"
    );
}

#[test]
fn independent_renderers_do_not_leak_theme_identity() {
    let shared_theme = compile_preset(ThemePreset::EditorLight);
    let competing_theme = compile_preset(ThemePreset::EditorDark);
    let first = HeadlessRenderer::new().with_theme(shared_theme.clone());
    let competing = HeadlessRenderer::new().with_theme(competing_theme.clone());
    let second = HeadlessRenderer::new().with_theme(shared_theme.clone());
    let plain = HeadlessRenderer::new();

    let first_document = first
        .render_document_sync("info")
        .unwrap()
        .expect("first themed document");
    let competing_document = competing
        .render_document_sync("info")
        .unwrap()
        .expect("competing themed document");
    let second_document = second
        .render_document_sync("info")
        .unwrap()
        .expect("second themed document");
    let plain_document = plain
        .render_document_sync("info")
        .unwrap()
        .expect("unthemed document");

    assert_eq!(
        first_document.theme_fingerprint(),
        Some(shared_theme.fingerprint())
    );
    assert_eq!(
        competing_document.theme_fingerprint(),
        Some(competing_theme.fingerprint())
    );
    assert_eq!(
        second_document.theme_fingerprint(),
        Some(shared_theme.fingerprint())
    );
    assert_eq!(plain_document.theme_fingerprint(), None);
}
