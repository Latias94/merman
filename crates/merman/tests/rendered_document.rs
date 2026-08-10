use merman::svg::{
    BlendMode, CanvasLayer, CanvasPaint, CanvasSpec, DiagramTheme, DiagramThemeCompiler,
    DiagramThemeSpec, DocumentResidualReason, DocumentResidualStage, FontAssetSpec,
    FontCatalogSpec, FontSource, FontStack, GradientStop, HeadlessError, HeadlessRenderer,
    LinearGradient, RenderEnvironment, RenderError, RenderExecutionPath, RenderFamilyKind,
    SvgPipeline, SvgPipelinePreset, TargetAdmissionReason, TargetAdmissionStatus,
    TextLayoutFailure, ThemeAssets, ThemeColorValue, ThemePreset, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, TrustedThemeLane, TypographySpec,
};
use merman::{Engine, MermaidConfig};

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
fn assert_custom_font_state_native_exports_reuse_the_prepared_catalog(source: &str) {
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
        png_report
            .document_report()
            .prepared_text_used_font_sources(),
        &[FontSource::Embedded]
    );
    assert_eq!(
        png_report.target_admission().status(),
        TargetAdmissionStatus::Portable,
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
        pdf_report
            .document_report()
            .prepared_text_used_font_sources(),
        &[FontSource::Embedded]
    );
    assert_eq!(
        pdf_report.target_admission().status(),
        TargetAdmissionStatus::Portable,
    );
}

#[cfg(all(feature = "png", feature = "pdf"))]
#[test]
fn custom_font_state_native_exports_reuse_the_prepared_catalog() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false}}%%
stateDiagram-v2
state "Portable 图 layout" as Ready
Ready --> Done : Prepared edge"#;

    assert_custom_font_state_native_exports_reuse_the_prepared_catalog(SOURCE);
}

#[cfg(all(feature = "png", feature = "pdf"))]
#[test]
fn custom_font_state_html_native_exports_verify_prepared_fallback_lines() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": true, "flowchart": {"wrappingWidth": 72}}}%%
stateDiagram-v2
state "Portable State label wraps here" as Ready
Ready --> Done : Prepared edge"#;

    assert_custom_font_state_native_exports_reuse_the_prepared_catalog(SOURCE);
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
            .contains(&TargetAdmissionReason::UnsupportedRootCapability)
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
            .contains(&TargetAdmissionReason::UnsupportedRootCapability)
    );
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
