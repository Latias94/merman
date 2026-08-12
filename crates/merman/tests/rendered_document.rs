use merman::svg::{
    CanvasLayer, CanvasPaint, CanvasSpec, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler,
    DiagramThemeSpec, DocumentResidualReason, DocumentResidualStage, EffectBinding, EffectGraph,
    EffectInput, EffectPrimitive, FilterRegion, FontAssetSpec, FontCatalogSpec,
    FontEmbeddingRequirement, FontSource, FontStack, GradientStop, HeadlessError, HeadlessRenderer,
    LinearGradient, RenderEnvironment, RenderError, RenderExecutionPath, RenderFamilyKind,
    SvgPipeline, SvgPipelinePreset, TargetAdmissionReason, TargetAdmissionStatus,
    TextLayoutFailure, ThemeAssets, ThemeColorValue, ThemePreset, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, TrustedThemeLane, TypographySpec,
};
use merman::{Engine, MermaidConfig};

#[cfg(all(feature = "png", feature = "pdf"))]
use merman::svg::BlendMode;

fn compile_preset(preset: ThemePreset) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile_preset(preset)
        .expect("theme preset should compile")
}

fn embedded_font_theme_spec() -> DiagramThemeSpec {
    let latin = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let cjk = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Xiaolai-Regular-CJK-Test.woff2"
    ));
    let catalog = FontCatalogSpec::new([
        FontAssetSpec::new("excalifont", latin),
        FontAssetSpec::new("xiaolai", cjk),
    ]);
    let typography = ThemeTextStyle::default().with_font_stack(
        FontStack::new(["Excalifont", "Xiaolai SC"]).expect("fixture font stack should be valid"),
    );
    DiagramThemeSpec::new()
        .with_typography(TypographySpec::default().with_default(typography))
        .with_assets(ThemeAssets::default().with_font_catalog(catalog))
}

fn embedded_font_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(embedded_font_theme_spec())
        .expect("embedded font theme should compile")
}

fn full_embedded_font_theme() -> DiagramTheme {
    let latin = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let catalog = FontCatalogSpec::new([FontAssetSpec::new("excalifont", latin)])
        .with_available_sources([FontSource::Embedded])
        .with_embedding_requirement(FontEmbeddingRequirement::FullFont);
    let typography = ThemeTextStyle::default().with_font_stack(
        FontStack::single("Excalifont").expect("fixture font family should be valid"),
    );
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(TypographySpec::default().with_default(typography))
                .with_assets(ThemeAssets::default().with_font_catalog(catalog)),
        )
        .expect("full embedded font theme should compile")
}

fn embedded_font_state_hard_shadow_theme() -> DiagramTheme {
    let graph = EffectGraph::new(
        "state-hard-shadow",
        FilterRegion::bounded(-0.2, -0.2, 1.4, 1.4),
        [EffectPrimitive::DropShadow {
            input: EffectInput::SourceGraphic,
            offset_x: 5.0,
            offset_y: 5.0,
            blur_radius: 0.0,
            spread: 0.0,
            color: ThemeColorValue::parse("#111827").expect("valid shadow color"),
        }],
    )
    .expect("valid hard-shadow graph");
    let effects = DiagramEffectSet::default()
        .with_graph(graph)
        .expect("unique effect graph")
        .with_binding(
            EffectBinding::new(ThemeTarget::State, "state-hard-shadow")
                .expect("valid State effect binding"),
        )
        .expect("unique State effect binding");
    DiagramThemeCompiler::new()
        .compile(embedded_font_theme_spec().with_effects(effects))
        .expect("embedded-font State hard-shadow theme should compile")
}

#[test]
fn rendered_document_keeps_svg_resources_and_operation_evidence_correlated() {
    let document = HeadlessRenderer::new()
        .render_document_sync("info")
        .unwrap()
        .expect("info diagram");
    let document_resource_fingerprint = document.resource_fingerprint();
    let admitted = document
        .admit_svg()
        .expect("best-effort SVG should be admitted under the default policy");

    assert!(admitted.as_str().starts_with("<svg"));
    assert!(
        admitted
            .document_report()
            .svg_finalization_report()
            .resource_closure()
            .is_closed()
    );
    assert_eq!(
        admitted.report().execution_path(),
        RenderExecutionPath::HeadlessOperationTyped
    );
    assert_eq!(admitted.report().theme_recipe_fingerprint(), None);
    assert_eq!(admitted.report().theme_recipe_report(), None);
    assert_eq!(admitted.report().family_kind(), RenderFamilyKind::Info);
    assert_eq!(
        admitted
            .document_report()
            .svg_finalization_report()
            .preset(),
        SvgPipelinePreset::ResvgSafe
    );
    assert_eq!(
        admitted.document_report().resource_fingerprint(),
        document_resource_fingerprint
    );
    assert_eq!(
        admitted.target_admission().status(),
        TargetAdmissionStatus::HostDependent
    );
}

#[test]
fn standalone_svg_with_a_full_embedded_font_is_portable() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false}}%%
stateDiagram-v2
state "Portable label" as Ready"#;
    let environment = RenderEnvironment::deterministic().with_theme_portability_requirement(
        merman::svg::ThemePortabilityRequirement::RequirePortable,
    );
    let document = HeadlessRenderer::from_engine_and_environment(Engine::new(), environment)
        .with_theme(full_embedded_font_theme())
        .render_document_sync(SOURCE)
        .expect("full-font State document should render")
        .expect("State diagram should be detected");

    let admission = document.svg_target_admission();
    assert_eq!(
        admission.status(),
        TargetAdmissionStatus::Portable,
        "reasons={:?}, residuals={:?}",
        admission.reasons(),
        document.document_report().residuals(),
    );
    assert!(
        !admission
            .reasons()
            .contains(&TargetAdmissionReason::ExternalSvgFontResolution)
    );

    let admitted = document
        .admit_svg()
        .expect("strict standalone SVG should pass terminal admission");
    assert!(
        admitted
            .as_str()
            .contains(r#"data-merman-typed-fonts="v1""#),
        "{}",
        admitted.as_str(),
    );
    assert!(admitted.as_str().contains("data:font/ttf;base64,"));
    assert!(!admitted.as_str().contains("merman-prepared-"));
}

#[test]
fn standalone_svg_without_a_complete_font_seal_remains_host_dependent() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false}}%%
stateDiagram-v2
state "Host-resolved label" as Ready"#;
    let document = HeadlessRenderer::from_engine_and_environment(
        Engine::new(),
        RenderEnvironment::deterministic(),
    )
    .with_theme(embedded_font_theme())
    .render_document_sync(SOURCE)
    .expect("embedded-font State document should render")
    .expect("State diagram should be detected");

    let admission = document.svg_target_admission();
    assert_eq!(admission.status(), TargetAdmissionStatus::HostDependent);
    assert!(
        admission
            .reasons()
            .contains(&TargetAdmissionReason::ExternalSvgFontResolution)
    );
    let admitted = document
        .admit_svg()
        .expect("host-dependent SVG remains available under best-effort admission");
    assert!(
        !admitted
            .as_str()
            .contains(r#"data-merman-typed-fonts="v1""#)
    );
}

#[test]
fn terminal_svg_metadata_is_retained_by_the_document_and_admitted_svg() {
    const SOURCE: &str = r#"flowchart LR
accTitle: Accessible diagram title
accDescr: Accessible diagram description
A --> B"#;
    let document = HeadlessRenderer::new()
        .render_document_sync(SOURCE)
        .expect("document render should succeed")
        .expect("flowchart should be detected");

    assert_eq!(
        document.metadata().title(),
        Some("Accessible diagram title")
    );
    assert_eq!(
        document.metadata().description(),
        Some("Accessible diagram description")
    );

    let admitted = document
        .admit_svg()
        .expect("best-effort SVG should retain terminal metadata");
    assert_eq!(
        admitted.metadata().title(),
        Some("Accessible diagram title")
    );
    assert_eq!(
        admitted.metadata().description(),
        Some("Accessible diagram description")
    );
}

#[cfg(feature = "png")]
#[test]
fn prepared_png_export_freezes_target_admission_before_encoding() {
    let document = HeadlessRenderer::new()
        .render_document_sync("flowchart LR\nA[Prepared] --> B[PNG]")
        .expect("document render should succeed")
        .expect("flowchart should be detected");
    let document_fingerprint = document.resource_fingerprint();
    let prepared = document
        .prepare_png_export(&merman::svg::export::RasterOptions::default())
        .expect("best-effort PNG target should prepare");

    assert_eq!(
        prepared.target_admission().target(),
        merman::svg::RenderTargetKind::Png
    );
    assert_eq!(
        prepared.export_report().resource_fingerprint(),
        document_fingerprint
    );

    let (bytes, report) = prepared.encode().expect("prepared PNG should encode");
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(
        report.target_admission().target(),
        merman::svg::RenderTargetKind::Png
    );
    assert_eq!(
        report.document_report().resource_fingerprint(),
        document_fingerprint
    );
    assert_eq!(
        report.export_report().resource_fingerprint(),
        document_fingerprint
    );
}

#[cfg(feature = "jpeg")]
#[test]
fn prepared_jpeg_export_freezes_target_admission_before_encoding() {
    let document = HeadlessRenderer::new()
        .render_document_sync("flowchart LR\nA[Prepared] --> B[JPEG]")
        .expect("document render should succeed")
        .expect("flowchart should be detected");
    let document_fingerprint = document.resource_fingerprint();
    let prepared = document
        .prepare_jpeg_export(&merman::svg::export::RasterOptions::default())
        .expect("best-effort JPEG target should prepare");

    assert_eq!(
        prepared.target_admission().target(),
        merman::svg::RenderTargetKind::Jpeg
    );
    assert_eq!(
        prepared.export_report().resource_fingerprint(),
        document_fingerprint
    );

    let (bytes, report) = prepared.encode().expect("prepared JPEG should encode");
    assert!(bytes.starts_with(&[0xff, 0xd8, 0xff]));
    assert_eq!(
        report.target_admission().target(),
        merman::svg::RenderTargetKind::Jpeg
    );
    assert_eq!(
        report.document_report().resource_fingerprint(),
        document_fingerprint
    );
    assert_eq!(
        report.export_report().resource_fingerprint(),
        document_fingerprint
    );
}

#[cfg(feature = "pdf")]
#[test]
fn prepared_pdf_export_freezes_target_admission_before_encoding() {
    let document = HeadlessRenderer::new()
        .render_document_sync("flowchart LR\nA[Prepared] --> B[PDF]")
        .expect("document render should succeed")
        .expect("flowchart should be detected");
    let document_fingerprint = document.resource_fingerprint();
    let prepared = document
        .prepare_pdf_export(&merman::svg::export::PdfOptions::default())
        .expect("best-effort PDF target should prepare");

    assert_eq!(
        prepared.target_admission().target(),
        merman::svg::RenderTargetKind::Pdf
    );
    assert_eq!(
        prepared.export_report().resource_fingerprint(),
        document_fingerprint
    );

    let (bytes, report) = prepared.encode().expect("prepared PDF should encode");
    assert!(bytes.starts_with(b"%PDF-"));
    assert_eq!(
        report.target_admission().target(),
        merman::svg::RenderTargetKind::Pdf
    );
    assert_eq!(
        report.document_report().resource_fingerprint(),
        document_fingerprint
    );
    assert_eq!(
        report.export_report().resource_fingerprint(),
        document_fingerprint
    );
}

#[test]
fn document_report_retains_a_blocking_root_residual_for_best_effort_output() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(0.0, ThemeColorValue::parse("#0f172a").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("#22d3ee").unwrap()).unwrap(),
        ],
    )
    .unwrap();
    let canvas = CanvasSpec::transparent()
        .with_layer(CanvasLayer::new(CanvasPaint::LinearGradient(gradient)))
        .expect("valid canvas layer");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_canvas(canvas))
        .expect("theme should compile");
    let document = HeadlessRenderer::new()
        .with_theme(theme)
        .render_document_sync("info")
        .unwrap()
        .expect("info diagram");

    assert!(
        document
            .document_report()
            .residuals()
            .iter()
            .any(|residual| {
                residual.stage() == merman::svg::DocumentResidualStage::RootTheme
                    && residual.reason() == merman::svg::DocumentResidualReason::Unverified
            })
    );
    assert_eq!(
        document.svg_target_admission().status(),
        TargetAdmissionStatus::Rejected
    );
}

#[test]
fn require_portable_pending_document_cannot_escape_target_admission() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("empty theme should compile");
    let environment = RenderEnvironment::deterministic().with_theme_portability_requirement(
        merman::svg::ThemePortabilityRequirement::RequirePortable,
    );
    let document = HeadlessRenderer::from_engine_and_environment(Engine::new(), environment)
        .with_theme(theme)
        .render_document_sync("flowchart TD\n  A[Pending] --> B[Rejected]")
        .expect("strict document construction should complete before target admission")
        .expect("flowchart should be detected");

    let report = document.svg_target_admission();
    assert_ne!(report.status(), TargetAdmissionStatus::Portable);
    let rejection = document
        .admit_svg()
        .expect_err("strict pending documents must not expose an SVG artifact");
    assert_eq!(rejection, report);
}

#[test]
fn document_report_retains_a_trusted_theme_css_residual() {
    const SOURCE: &str = r##"%%{init: {"themeCSS": ".node rect { stroke-width: 3px; }"}}%%
flowchart TD
  A[Hello] --> B[World]
"##;
    let document = HeadlessRenderer::new()
        .with_site_config(MermaidConfig::from_value(serde_json::json!({
            "secure": [
                "secure",
                "securityLevel",
                "startOnLoad",
                "maxTextSize",
                "suppressErrorRendering",
                "maxEdges"
            ]
        })))
        .render_document_sync(SOURCE)
        .expect("themeCSS document should render in best-effort compatibility mode")
        .expect("flowchart should be detected");

    let admitted = document
        .admit_svg()
        .expect("best-effort themeCSS SVG should be admitted");
    assert!(
        admitted
            .as_str()
            .contains("#merman .node rect { stroke-width: 3px;")
    );
    assert!(
        admitted
            .report()
            .used_trusted_theme_lanes()
            .contains(TrustedThemeLane::RawThemeCss)
    );
    assert!(
        admitted
            .document_report()
            .residuals()
            .iter()
            .any(|residual| {
                residual.stage() == DocumentResidualStage::TrustedLane
                    && residual.reason() == DocumentResidualReason::Unverified
                    && residual.count() == 1
            })
    );
    assert_eq!(
        admitted.target_admission().status(),
        TargetAdmissionStatus::Rejected
    );
}

#[test]
fn document_report_retains_a_state_semantic_residual_after_svg_finalization() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#0f172a").expect("valid first stop"),
            )
            .expect("valid first stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#22d3ee").expect("valid second stop"),
            )
            .expect("valid second stop"),
        ],
    )
    .expect("valid gradient");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::State,
                ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient)),
            ))),
        )
        .expect("State gradient theme should compile");
    let document = HeadlessRenderer::new()
        .with_theme(theme)
        .render_document_sync("stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n")
        .expect("best-effort State render should succeed")
        .expect("State diagram should be detected");

    assert!(
        document
            .document_report()
            .residuals()
            .iter()
            .any(|residual| {
                residual.stage() == DocumentResidualStage::FamilyTheme
                    && residual.reason() == DocumentResidualReason::Unverified
                    && residual.count() == 1
            }),
        "terminal document evidence must retain the unsupported State gradient"
    );
    assert_eq!(
        document.svg_target_admission().status(),
        TargetAdmissionStatus::Rejected
    );
}

#[test]
fn themed_renderer_operations_freeze_one_theme_resource_identity() {
    const SOURCE: &str = "sequenceDiagram\nAlice->>Bob: Hello";

    let environment_theme = embedded_font_theme();
    let selected_theme = compile_preset(ThemePreset::OneDark);
    let environment = RenderEnvironment::deterministic()
        .with_font_catalog(environment_theme.font_catalog().clone());
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
        document.theme_recipe_fingerprint(),
        Some(selected_theme.recipe_fingerprint())
    );
    assert_eq!(
        document.report().theme_recipe_fingerprint(),
        Some(selected_theme.recipe_fingerprint())
    );
    assert_eq!(document.family_kind(), RenderFamilyKind::Sequence);
    assert_eq!(document.report().family_kind(), RenderFamilyKind::Sequence);
    assert_eq!(
        document.theme_recipe_report(),
        Some(selected_theme.report())
    );
    assert_eq!(
        document.report().theme_recipe_report(),
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
        document.font_source_policy().priority().collect::<Vec<_>>(),
        vec![merman::svg::FontSource::System]
    );
    assert_ne!(
        document.font_catalog().fingerprint(),
        environment_theme.font_catalog().fingerprint(),
        "a selected theme must not retain the environment catalog"
    );
}

#[test]
fn custom_font_flowchart_document_uses_the_retained_prepared_catalog() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": false}}}%%
flowchart LR
A[Portable 图 layout] --> B[Ready]"#;
    let theme = embedded_font_theme();
    let document = HeadlessRenderer::new()
        .with_theme(theme.clone())
        .render_document_sync(SOURCE)
        .expect("custom font document should render")
        .expect("flowchart should be detected");

    let prepared = document
        .report()
        .prepared_text_layout()
        .expect("custom catalog should be prepared before layout");
    assert_eq!(
        prepared.catalog_fingerprint(),
        theme.font_catalog().fingerprint()
    );
    assert_eq!(
        prepared.catalog_fingerprint(),
        document.font_catalog().fingerprint()
    );
    assert_eq!(prepared.used_font_sources(), &[FontSource::Embedded]);
    assert!(!prepared.is_host_dependent());
    assert_eq!(
        document.document_report().prepared_text_used_font_sources(),
        &[FontSource::Embedded]
    );
    assert!(!document.document_report().prepared_text_is_host_dependent());
    assert!(
        !document
            .svg_target_admission()
            .reasons()
            .contains(&TargetAdmissionReason::PreparedHostBackend)
    );
    assert_eq!(document.family_kind(), RenderFamilyKind::Flowchart);
    let legacy_measurements = document.report().measurement().entries();
    assert!(
        legacy_measurements.iter().all(|entry| {
            entry.provenance().source == merman::svg::TextMeasurementSource::Profile
        })
    );
    let admitted = document
        .admit_svg()
        .expect("best-effort custom-font Flowchart SVG should be admitted");
    assert!(!admitted.as_str().contains("merman-prepared-"));
}

#[test]
fn custom_font_state_document_uses_the_retained_prepared_catalog() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false}}%%
stateDiagram-v2
state "Portable 图 layout" as Ready
Ready --> Done : Prepared edge"#;
    let theme = embedded_font_theme();
    let document = HeadlessRenderer::new()
        .with_theme(theme.clone())
        .render_document_sync(SOURCE)
        .expect("custom font State document should render")
        .expect("State diagram should be detected");

    let prepared = document
        .report()
        .prepared_text_layout()
        .expect("State custom catalog should be prepared before layout");
    assert_eq!(
        prepared.catalog_fingerprint(),
        theme.font_catalog().fingerprint()
    );
    assert_eq!(prepared.used_font_sources(), &[FontSource::Embedded]);
    assert!(!prepared.is_host_dependent());
    assert_eq!(
        document.document_report().prepared_text_used_font_sources(),
        &[FontSource::Embedded]
    );
    assert!(!document.document_report().prepared_text_is_host_dependent());
    assert_eq!(document.family_kind(), RenderFamilyKind::State);
    let admitted = document
        .admit_svg()
        .expect("best-effort custom-font State SVG should be admitted");
    assert!(admitted.as_str().contains("Portable 图 layout"));
    assert!(admitted.as_str().contains("Prepared edge"));
    assert!(!admitted.as_str().contains("merman-prepared-"));
}

#[cfg(all(feature = "png", feature = "pdf"))]
fn assert_custom_font_state_native_exports_reuse_the_prepared_catalog(
    source: &str,
    expected_admission: TargetAdmissionStatus,
    expected_terminal_incomplete: bool,
) {
    let theme = embedded_font_theme();

    let (png, png_report) = HeadlessRenderer::new()
        .with_theme(theme.clone())
        .render_png_with_report_sync(source, &merman::svg::export::RasterOptions::default())
        .expect("custom font State PNG should render")
        .expect("State diagram should be detected");
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    let png_fonts = png_report.export_report().fonts();
    assert_eq!(
        png_fonts.catalog_fingerprint(),
        theme.font_catalog().fingerprint()
    );
    assert!(png_fonts.used_embedded_fonts());
    assert!(!png_fonts.used_system_fonts());
    assert!(png_fonts.prepared_label_expected_count() > 0);
    assert_eq!(
        png_fonts.prepared_label_verified_count(),
        png_fonts.prepared_label_expected_count()
    );
    assert_eq!(png_fonts.prepared_label_mismatch_count(), 0);
    assert!(png_fonts.prepared_text_evidence_matches());
    assert_eq!(
        png_fonts.prepared_label_terminal_incomplete_count() != 0,
        expected_terminal_incomplete,
    );
    assert_eq!(
        png_fonts.prepared_text_terminal_proof_complete(),
        !expected_terminal_incomplete,
    );
    assert_eq!(png_fonts.is_host_dependent(), expected_terminal_incomplete,);
    assert_eq!(
        png_report
            .document_report()
            .prepared_text_used_font_sources(),
        &[FontSource::Embedded]
    );
    assert_eq!(png_report.target_admission().status(), expected_admission,);
    assert_eq!(
        png_report
            .target_admission()
            .reasons()
            .contains(&TargetAdmissionReason::PreparedTextTerminalProofIncomplete),
        expected_terminal_incomplete,
    );

    let (pdf, pdf_report) = HeadlessRenderer::new()
        .with_theme(theme.clone())
        .render_pdf_with_report_sync(source, &merman::svg::export::PdfOptions::default())
        .expect("custom font State PDF should render")
        .expect("State diagram should be detected");
    assert!(pdf.starts_with(b"%PDF-"));
    let pdf_fonts = pdf_report.export_report().fonts();
    assert_eq!(
        pdf_fonts.catalog_fingerprint(),
        theme.font_catalog().fingerprint()
    );
    assert!(pdf_fonts.used_embedded_fonts());
    assert!(!pdf_fonts.used_system_fonts());
    assert!(pdf_fonts.prepared_label_expected_count() > 0);
    assert_eq!(
        pdf_fonts.prepared_label_verified_count(),
        pdf_fonts.prepared_label_expected_count()
    );
    assert_eq!(pdf_fonts.prepared_label_mismatch_count(), 0);
    assert!(pdf_fonts.prepared_text_evidence_matches());
    assert_eq!(
        pdf_fonts.prepared_label_terminal_incomplete_count() != 0,
        expected_terminal_incomplete,
    );
    assert_eq!(
        pdf_fonts.prepared_text_terminal_proof_complete(),
        !expected_terminal_incomplete,
    );
    assert_eq!(pdf_fonts.is_host_dependent(), expected_terminal_incomplete,);
    assert_eq!(
        pdf_report
            .document_report()
            .prepared_text_used_font_sources(),
        &[FontSource::Embedded]
    );
    assert_eq!(pdf_report.target_admission().status(), expected_admission,);
    assert_eq!(
        pdf_report
            .target_admission()
            .reasons()
            .contains(&TargetAdmissionReason::PreparedTextTerminalProofIncomplete),
        expected_terminal_incomplete,
    );
}

#[cfg(all(feature = "png", feature = "pdf"))]
#[test]
fn custom_font_state_multi_face_native_exports_are_not_misreported_as_portable() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false}}%%
stateDiagram-v2
state "Portable 图 layout" as Ready
Ready --> Done : Prepared edge"#;

    assert_custom_font_state_native_exports_reuse_the_prepared_catalog(
        SOURCE,
        TargetAdmissionStatus::HostDependent,
        true,
    );
}

#[cfg(all(feature = "png", feature = "pdf"))]
#[test]
fn custom_font_state_html_native_exports_verify_prepared_fallback_lines() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": true, "flowchart": {"wrappingWidth": 72}}}%%
stateDiagram-v2
state "Portable State label wraps here" as Ready
Ready --> Done : Prepared edge"#;

    assert_custom_font_state_native_exports_reuse_the_prepared_catalog(
        SOURCE,
        TargetAdmissionStatus::Portable,
        false,
    );
}

#[cfg(all(feature = "png", feature = "pdf"))]
#[test]
fn require_portable_rejects_multi_face_native_text_without_exact_terminal_ranges() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false}}%%
stateDiagram-v2
state "Portable 图 layout" as Ready
Ready --> Done : Prepared edge"#;
    let environment = RenderEnvironment::deterministic().with_theme_portability_requirement(
        merman::svg::ThemePortabilityRequirement::RequirePortable,
    );

    let png_error =
        HeadlessRenderer::from_engine_and_environment(Engine::new(), environment.clone())
            .with_theme(embedded_font_theme())
            .render_png_with_report_sync(SOURCE, &merman::svg::export::RasterOptions::default())
            .expect_err("strict multi-face PNG must fail terminal admission");
    let png_rejection = png_error
        .target_admission_rejection()
        .expect("PNG failure must retain target admission");
    assert_eq!(png_rejection.status(), TargetAdmissionStatus::HostDependent);
    assert!(
        png_rejection
            .reasons()
            .contains(&TargetAdmissionReason::PreparedTextTerminalProofIncomplete)
    );

    let pdf_error = HeadlessRenderer::from_engine_and_environment(Engine::new(), environment)
        .with_theme(embedded_font_theme())
        .render_pdf_with_report_sync(SOURCE, &merman::svg::export::PdfOptions::default())
        .expect_err("strict multi-face PDF must fail terminal admission");
    let pdf_rejection = pdf_error
        .target_admission_rejection()
        .expect("PDF failure must retain target admission");
    assert_eq!(pdf_rejection.status(), TargetAdmissionStatus::HostDependent);
    assert!(
        pdf_rejection
            .reasons()
            .contains(&TargetAdmissionReason::PreparedTextTerminalProofIncomplete)
    );
}

#[cfg(all(feature = "png", feature = "pdf"))]
#[test]
fn root_blend_layer_is_explicitly_admitted_by_native_targets() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false}}%%
stateDiagram-v2
state "Portable blend" as Ready
Ready --> Done : Native targets"#;
    let canvas = CanvasSpec::transparent()
        .with_layer(
            CanvasLayer::new(CanvasPaint::solid("#22d3ee").expect("valid layer paint"))
                .with_opacity(0.65)
                .expect("valid layer opacity")
                .with_blend_mode(BlendMode::Multiply),
        )
        .expect("bounded root layer");
    let theme = DiagramThemeCompiler::new()
        .compile(embedded_font_theme_spec().with_canvas(canvas))
        .expect("root blend theme should compile");

    let (_, png_report) = HeadlessRenderer::new()
        .with_theme(theme.clone())
        .render_png_with_report_sync(SOURCE, &merman::svg::export::RasterOptions::default())
        .expect("root blend PNG should render")
        .expect("State diagram should be detected");
    assert_eq!(
        png_report.target_admission().status(),
        TargetAdmissionStatus::Portable,
        "reasons={:?}, residuals={:?}",
        png_report.target_admission().reasons(),
        png_report.document_report().residuals(),
    );
    assert!(
        !png_report
            .target_admission()
            .reasons()
            .contains(&TargetAdmissionReason::UnsupportedThemeCapability)
    );

    let (_, pdf_report) = HeadlessRenderer::new()
        .with_theme(theme)
        .render_pdf_with_report_sync(SOURCE, &merman::svg::export::PdfOptions::default())
        .expect("root blend PDF should render")
        .expect("State diagram should be detected");
    assert_eq!(
        pdf_report.target_admission().status(),
        TargetAdmissionStatus::Portable,
        "reasons={:?}, residuals={:?}",
        pdf_report.target_admission().reasons(),
        pdf_report.document_report().residuals(),
    );
    assert!(
        !pdf_report
            .target_admission()
            .reasons()
            .contains(&TargetAdmissionReason::UnsupportedThemeCapability)
    );
}

#[test]
fn state_hard_shadow_capability_remains_admitted_for_svg() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false}}%%
stateDiagram-v2
state "Filtered state" as Ready"#;
    let theme = embedded_font_state_hard_shadow_theme();

    let report = HeadlessRenderer::new()
        .with_theme(theme)
        .render_svg_report_sync(SOURCE)
        .expect("State hard-shadow SVG should render")
        .expect("State diagram should be detected");

    assert!(
        !report
            .target_admission()
            .reasons()
            .contains(&TargetAdmissionReason::UnsupportedThemeCapability)
    );
    assert!(
        report
            .target_admission()
            .reasons()
            .contains(&TargetAdmissionReason::UnsealedSvg),
        "reasons={:?}",
        report.target_admission().reasons(),
    );
    let document = roxmltree::Document::parse(report.svg()).expect("valid State SVG");
    let typed_filter_ids = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("filter")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("-theme-effect-state-hard-shadow"))
        })
        .map(|node| node.attribute("id").expect("typed filter id"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(typed_filter_ids.len(), 1, "{}", report.svg());
    let referenced_filter_ids = document
        .descendants()
        .filter_map(|node| node.attribute("filter"))
        .filter_map(|value| value.strip_prefix("url(#"))
        .filter_map(|value| value.strip_suffix(')'))
        .filter(|id| id.contains("-theme-effect-state-hard-shadow"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(referenced_filter_ids, typed_filter_ids, "{}", report.svg());
}

#[cfg(feature = "png")]
#[test]
fn state_hard_shadow_capability_is_admitted_by_png_target() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false}}%%
stateDiagram-v2
state "Filtered state" as Ready"#;

    let (png, png_report) = HeadlessRenderer::new()
        .with_theme(embedded_font_state_hard_shadow_theme())
        .render_png_with_report_sync(SOURCE, &merman::svg::export::RasterOptions::default())
        .expect("State hard-shadow PNG should render")
        .expect("State diagram should be detected");
    assert_eq!(
        png_report.target_admission().status(),
        TargetAdmissionStatus::Portable
    );
    assert!(
        png_report
            .target_admission()
            .reasons()
            .iter()
            .all(|reason| *reason != TargetAdmissionReason::UnsupportedThemeCapability)
    );
    assert!(png_report.export_report().conversion().filtered_groups > 0);
    assert!(png_report.export_report().conversion().filter_primitives > 0);
    assert_png_contains_quantized_rgb(&png, [0x11, 0x18, 0x27], 64);
}

#[cfg(feature = "png")]
fn assert_png_contains_quantized_rgb(bytes: &[u8], expected: [u8; 3], minimum_pixel_count: usize) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("hard-shadow PNG header");
    let mut pixels = vec![
        0;
        reader
            .output_buffer_size()
            .expect("hard-shadow PNG output buffer size")
    ];
    let frame = reader
        .next_frame(&mut pixels)
        .expect("hard-shadow PNG frame");
    assert_eq!(frame.color_type, png::ColorType::Rgba);
    assert_eq!(frame.bit_depth, png::BitDepth::Eight);
    let quantized = expected.map(srgb_u8_linear_roundtrip);
    let matching_pixel_count = pixels[..frame.buffer_size()]
        .chunks_exact(4)
        .filter(|pixel| pixel[3] == u8::MAX && (pixel[..3] == expected || pixel[..3] == quantized))
        .count();
    assert!(
        matching_pixel_count >= minimum_pixel_count,
        "encoded PNG must contain at least {minimum_pixel_count} opaque hard-shadow pixels in source color #{:02x}{:02x}{:02x} or its 8-bit linearRGB quantization #{:02x}{:02x}{:02x}; found {matching_pixel_count} in {}x{}",
        expected[0],
        expected[1],
        expected[2],
        quantized[0],
        quantized[1],
        quantized[2],
        frame.width,
        frame.height,
    );
}

#[cfg(feature = "png")]
fn srgb_u8_linear_roundtrip(channel: u8) -> u8 {
    let srgb = f64::from(channel) / 255.0;
    let linear = if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    };
    let quantized_linear = (linear * 255.0).round().clamp(0.0, 255.0) / 255.0;
    let roundtrip = if quantized_linear <= 0.0031308 {
        quantized_linear * 12.92
    } else {
        1.055 * quantized_linear.powf(1.0 / 2.4) - 0.055
    };
    (roundtrip * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(feature = "jpeg")]
#[test]
fn state_hard_shadow_capability_is_admitted_by_jpeg_target() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false}}%%
stateDiagram-v2
state "Filtered state" as Ready"#;

    let (_, jpeg_report) = HeadlessRenderer::new()
        .with_theme(embedded_font_state_hard_shadow_theme())
        .render_jpeg_with_report_sync(SOURCE, &merman::svg::export::RasterOptions::default())
        .expect("State hard-shadow JPEG should render")
        .expect("State diagram should be detected");
    assert_eq!(
        jpeg_report.target_admission().status(),
        TargetAdmissionStatus::Portable
    );
    assert!(
        jpeg_report
            .target_admission()
            .reasons()
            .iter()
            .all(|reason| *reason != TargetAdmissionReason::UnsupportedThemeCapability)
    );
    assert!(jpeg_report.export_report().conversion().filtered_groups > 0);
    assert!(jpeg_report.export_report().conversion().filter_primitives > 0);
}

#[cfg(feature = "pdf")]
#[test]
fn state_hard_shadow_capability_is_admitted_by_pdf_target() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false}}%%
stateDiagram-v2
state "Filtered state" as Ready"#;

    let (_, pdf_report) = HeadlessRenderer::new()
        .with_theme(embedded_font_state_hard_shadow_theme())
        .render_pdf_with_report_sync(SOURCE, &merman::svg::export::PdfOptions::default())
        .expect("State hard-shadow PDF should render")
        .expect("State diagram should be detected");
    assert_eq!(
        pdf_report.target_admission().status(),
        TargetAdmissionStatus::Portable
    );
    assert!(
        pdf_report
            .target_admission()
            .reasons()
            .iter()
            .all(|reason| *reason != TargetAdmissionReason::UnsupportedThemeCapability)
    );
    assert!(pdf_report.export_report().conversion().filtered_groups > 0);
    assert!(pdf_report.export_report().conversion().filter_primitives > 0);
    assert!(pdf_report.export_report().filters().filtered_groups > 0);
    assert!(pdf_report.export_report().filters().effective_image_pixels > 0);
}

#[test]
fn custom_font_state_html_labels_emit_the_prepared_wrap() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": true, "flowchart": {"wrappingWidth": 72}}}%%
stateDiagram-v2
state "Portable State label wraps here" as Ready
Ready --> Done"#;
    let rendered = HeadlessRenderer::new()
        .with_theme(embedded_font_theme())
        .render_svg_report_sync(SOURCE)
        .expect("custom font HTML State document should render")
        .expect("State diagram should be detected");

    assert!(
        rendered.svg().contains("<foreignObject"),
        "{}",
        rendered.svg()
    );
    assert!(
        rendered.svg().contains("<br />"),
        "prepared HTML wrapping should be emitted explicitly"
    );
    assert!(
        !rendered.svg().contains("data-merman-prepared-text-label"),
        "prepared evidence attributes must stay out of public SVG"
    );
    assert!(
        !rendered.svg().contains("merman-prepared-"),
        "prepared evidence ids must stay out of public SVG"
    );
    assert_eq!(
        rendered
            .report()
            .prepared_text_layout()
            .expect("State HTML labels should use prepared text")
            .used_font_sources(),
        &[FontSource::Embedded]
    );
}

#[test]
fn custom_font_state_rich_markdown_fails_closed() {
    const SOURCE: &str = r#"stateDiagram-v2
state "**Portable**" as Ready
Ready --> Done"#;
    let error = HeadlessRenderer::new()
        .with_theme(embedded_font_theme())
        .render_document_sync(SOURCE)
        .expect_err("rich State Markdown is not admitted by the portable text path yet");

    assert!(matches!(
        error,
        HeadlessError::Render(RenderError::TextLayout(
            TextLayoutFailure::UnsupportedLabelMode
        ))
    ));
}

#[test]
fn custom_font_state_diagram_title_fails_closed_without_signed_bbox_evidence() {
    const SOURCE: &str = r#"---
title: Portable State title
---
stateDiagram-v2
Ready --> Done"#;
    let error = HeadlessRenderer::new()
        .with_theme(embedded_font_theme())
        .render_document_sync(SOURCE)
        .expect_err("State diagram titles require signed prepared bbox evidence");

    assert!(matches!(
        error,
        HeadlessError::Render(RenderError::TextLayout(
            TextLayoutFailure::UnsupportedLabelMode
        ))
    ));
}

#[test]
fn themed_pipeline_report_freezes_the_authoritative_swimlane_family() {
    let theme = compile_preset(ThemePreset::OneDark);
    let renderer = HeadlessRenderer::new()
        .with_site_config(merman_core::MermaidConfig::from_value(serde_json::json!({
            "layout": "swimlane"
        })))
        .with_theme(theme.clone());

    let rendered = renderer
        .render_svg_with_pipeline_report_sync("flowchart LR\nA --> B\n", &SvgPipeline::readable())
        .expect("pipeline render should succeed")
        .expect("flowchart source should render");

    assert_eq!(rendered.family_kind(), RenderFamilyKind::Swimlane);
    assert_eq!(rendered.report().family_kind(), RenderFamilyKind::Swimlane);
    assert_eq!(
        rendered.report().theme_recipe_fingerprint(),
        Some(theme.recipe_fingerprint())
    );
    assert_eq!(
        rendered.report().theme_recipe_report(),
        Some(theme.report())
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
        first_document.theme_recipe_fingerprint(),
        Some(shared_theme.recipe_fingerprint())
    );
    assert_eq!(
        competing_document.theme_recipe_fingerprint(),
        Some(competing_theme.recipe_fingerprint())
    );
    assert_eq!(
        second_document.theme_recipe_fingerprint(),
        Some(shared_theme.recipe_fingerprint())
    );
    assert_eq!(plain_document.theme_recipe_fingerprint(), None);
}
