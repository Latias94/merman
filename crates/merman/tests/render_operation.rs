#[cfg(feature = "svg")]
use std::borrow::Cow;
#[cfg(feature = "svg")]
use std::sync::Arc;
use std::time::Duration;

use merman::{
    OperationControl, OperationPhase, ParseOptions, RenderError, RenderOutput, RenderRequest,
    Renderer, SemanticArtifact, ThemeEvidenceStatus,
    resources::{InputResourceLimitId, InputResourcePolicy},
};

#[cfg(feature = "svg")]
use sha2::{Digest as _, Sha256};

#[cfg(feature = "svg")]
#[derive(Debug)]
struct CancellingTextMeasurer {
    control: OperationControl,
}

#[cfg(feature = "svg")]
impl merman::svg::HostTextMeasurer for CancellingTextMeasurer {
    fn measure(
        &self,
        _request: merman::svg::HostTextMeasurementRequest<'_>,
    ) -> merman::svg::HostMeasurementResult {
        self.control.cancel();
        Ok(None)
    }
}

#[cfg(feature = "svg")]
#[derive(Debug)]
struct FixedHostTextMeasurer;

#[cfg(feature = "svg")]
#[derive(Debug)]
struct MissingFragmentPostprocessor;

#[cfg(feature = "svg")]
impl merman::svg::SvgPostprocessor for MissingFragmentPostprocessor {
    fn name(&self) -> &'static str {
        "test-missing-fragment"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &merman::svg::SvgPostprocessContext<'_>,
    ) -> merman::svg::RenderResult<Cow<'a, str>> {
        let mut svg = svg.into_owned();
        let root_end = svg
            .rfind("</svg>")
            .expect("renderer output should contain a closing SVG root");
        svg.insert_str(
            root_end,
            r#"<path data-test-missing-fragment="true" fill="url(#missing-resource)" d="M0 0h1v1z"/>"#,
        );
        Ok(Cow::Owned(svg))
    }
}

#[cfg(feature = "svg")]
impl merman::svg::HostTextMeasurer for FixedHostTextMeasurer {
    fn measure(
        &self,
        request: merman::svg::HostTextMeasurementRequest<'_>,
    ) -> merman::svg::HostMeasurementResult {
        let raw_width = request.text.chars().count().max(1) as f64 * 8.0;
        let max_width = request
            .max_width
            .filter(|width| width.is_finite() && *width > 0.0);
        let line_count = max_width
            .map(|width| (raw_width / width).ceil() as usize)
            .unwrap_or(1)
            .max(request.text.lines().count())
            .max(1)
            .min(request.text.len().saturating_add(1));
        let metrics = merman::svg::TextMetrics {
            width: max_width.map_or(raw_width, |width| raw_width.min(width)),
            height: line_count as f64 * 20.0,
            line_count,
        };
        let measurement = match request.operation.required_result_kind() {
            merman::svg::TextMeasurementResultKind::Metrics => {
                merman::svg::HostTextMeasurement::Metrics(metrics)
            }
            merman::svg::TextMeasurementResultKind::Length => {
                let length = match request.operation {
                    merman::svg::TextMeasurementOperation::RawBBoxHeight
                    | merman::svg::TextMeasurementOperation::SimpleBBoxHeight
                    | merman::svg::TextMeasurementOperation::TspanBBoxHeight => metrics.height,
                    merman::svg::TextMeasurementOperation::CreateTextBBoxYOffset
                    | merman::svg::TextMeasurementOperation::CreateTextMiddleBBoxYOffset => 0.0,
                    _ => raw_width,
                };
                merman::svg::HostTextMeasurement::Length(length)
            }
            merman::svg::TextMeasurementResultKind::HorizontalExtents => {
                merman::svg::HostTextMeasurement::HorizontalExtents {
                    left: raw_width / 2.0,
                    right: raw_width / 2.0,
                }
            }
            merman::svg::TextMeasurementResultKind::WrappedWithRawWidth => {
                merman::svg::HostTextMeasurement::WrappedWithRawWidth {
                    metrics,
                    raw_width: Some(raw_width),
                }
            }
        };
        Ok(Some(measurement))
    }
}

#[cfg(feature = "svg")]
fn portable_state_theme() -> merman::svg::DiagramTheme {
    let palette = merman::svg::OrdinalPalette::new([
        merman::svg::ThemeColorValue::parse("#0f172a").expect("valid first palette color"),
        merman::svg::ThemeColorValue::parse("#22d3ee").expect("valid second palette color"),
    ])
    .expect("valid state palette");
    merman::svg::DiagramThemeCompiler::new()
        .compile(
            merman::svg::DiagramThemeSpec::new().with_styles(
                merman::svg::ThemeRuleSet::default()
                    .with_ordinal_palette(merman::svg::ThemeTarget::State, palette),
            ),
        )
        .expect("portable state theme should compile")
}

#[cfg(feature = "svg")]
fn portable_pie_palette_theme() -> merman::svg::DiagramTheme {
    let palette = merman::svg::OrdinalPalette::new([
        merman::svg::ThemeColorValue::parse("#ef4444").expect("valid first palette color"),
        merman::svg::ThemeColorValue::parse("#22c55e").expect("valid second palette color"),
        merman::svg::ThemeColorValue::parse("#2563eb").expect("valid third palette color"),
    ])
    .expect("valid Pie palette");
    merman::svg::DiagramThemeCompiler::new()
        .compile(
            merman::svg::DiagramThemeSpec::new().with_styles(
                merman::svg::ThemeRuleSet::default()
                    .with_ordinal_palette(merman::svg::ThemeTarget::PieSlice, palette),
            ),
        )
        .expect("portable Pie theme should compile")
}

#[cfg(feature = "svg")]
fn flowchart_paint_theme() -> merman::svg::DiagramTheme {
    let styles = merman::svg::ThemeRuleSet::default().with_rule(
        merman::svg::ThemeRule::new(
            merman::svg::ThemeTarget::Node,
            merman::svg::ThemeStylePatch::default().with_fill(
                merman::svg::CanvasPaint::solid("#1d4ed8").expect("fixture paint is valid"),
            ),
        )
        .for_family(merman::DiagramFamilyId::FLOWCHART),
    );
    merman::svg::DiagramThemeCompiler::new()
        .compile(merman::svg::DiagramThemeSpec::new().with_styles(styles))
        .expect("flowchart paint theme should compile")
}

#[cfg(feature = "svg")]
fn render_host_measured_document(
    portability: merman::svg::ThemePortabilityRequirement,
) -> merman::RenderedDocument {
    render_host_measured_document_with_profile(portability, "merman.test-fixed-display-host")
}

#[cfg(feature = "svg")]
fn render_host_measured_document_with_profile(
    portability: merman::svg::ThemePortabilityRequirement,
    profile_id: &str,
) -> merman::RenderedDocument {
    let identity = merman::svg::TextMeasurementProfileIdentity::new(
        merman::svg::MeasurementProfileId::new(profile_id).expect("test profile id"),
        "render-operation-test@1",
    )
    .expect("test profile identity");
    let measurement = merman::svg::TextMeasurementPolicy::host_display(
        identity,
        Arc::new(FixedHostTextMeasurer),
        merman::svg::TextMeasurementPhase::ALL,
    );
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic()
            .with_text_measurement_policy(measurement)
            .with_theme_portability_requirement(portability),
        ..merman::SvgRequest::default()
    };
    let output = Renderer::new()
        .render(
            RenderRequest::document(
                "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\nA[Start] --> B[Done]",
                OperationControl::new(),
                request,
            )
            .with_theme(flowchart_paint_theme()),
        )
        .expect("host-measured document should complete before target admission");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a host-measured document");
    };
    document
}

#[test]
fn semantic_request_uses_the_canonical_operation_runner() {
    let output = Renderer::new()
        .render(RenderRequest::semantic(
            "flowchart TD\nA[Start] --> B[Done]",
            OperationControl::new(),
        ))
        .expect("semantic request should succeed");

    let RenderOutput::Semantic(Some(artifact)) = output else {
        panic!("expected a semantic artifact");
    };
    assert_eq!(artifact.diagram_type(), "flowchart-v2");
    assert_eq!(artifact.semantic_kind(), "flowchart");
}

#[test]
fn cancelled_semantic_request_returns_no_partial_artifact() {
    let control = OperationControl::new();
    control.cancel();

    let error = Renderer::new()
        .render(RenderRequest::semantic("flowchart TD\nA --> B", control))
        .expect_err("cancelled operation must not return an artifact");

    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Admission
                && cancelled.reason == merman::CancelReason::Requested
    ));
}

#[test]
fn expired_deadline_is_reported_as_deadline_cancellation() {
    let control = OperationControl::new().with_deadline(Duration::ZERO);

    let error = Renderer::new()
        .render(RenderRequest::semantic("flowchart TD\nA --> B", control))
        .expect_err("expired operation must stop before parsing");

    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.reason == merman::CancelReason::DeadlineExceeded
    ));
}

#[test]
fn prepare_semantic_delegates_to_the_same_runner() {
    let artifact: Option<SemanticArtifact> = Renderer::new()
        .prepare_semantic("info", OperationControl::new())
        .expect("prepare should succeed");
    assert_eq!(
        artifact.expect("info should be detected").semantic_kind(),
        "info"
    );
}

#[test]
fn renderer_defaults_apply_when_the_request_does_not_override_them() {
    let renderer = Renderer::new().with_resource_policy(
        InputResourcePolicy::default()
            .with_limit(InputResourceLimitId::MaxSourceBytes, 4)
            .expect("valid source limit"),
    );

    let error = renderer
        .render(RenderRequest::semantic(
            "flowchart TD\nA --> B",
            OperationControl::new(),
        ))
        .expect_err("the renderer's source limit must apply to one-shot requests");

    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_source_bytes" && limit.maximum == 4
    ));
}

#[test]
fn request_overrides_take_precedence_over_renderer_defaults() {
    let renderer = Renderer::new()
        .with_parse_options(ParseOptions::strict())
        .with_resource_policy(
            InputResourcePolicy::default()
                .with_limit(InputResourceLimitId::MaxSourceBytes, 4)
                .expect("valid source limit"),
        );
    let request_resources = InputResourcePolicy::default()
        .with_limit(InputResourceLimitId::MaxSourceBytes, 4_096)
        .expect("valid source limit");

    let output = renderer
        .render(
            RenderRequest::semantic("flowchart TD\nA --> B", OperationControl::new())
                .with_parse_options(ParseOptions::lenient())
                .with_resource_policy(request_resources),
        )
        .expect("request overrides should replace renderer defaults");

    assert!(matches!(output, RenderOutput::Semantic(Some(_))));
}

#[cfg(feature = "svg")]
#[test]
fn typed_svg_targets_share_the_prepared_operation() {
    let renderer = Renderer::new();
    let source = "flowchart TD\nA[Start] --> B[Done]";

    let layout = renderer
        .render(RenderRequest::layout_json(
            source,
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("layout target should succeed");
    let RenderOutput::LayoutJson(Some(layout)) = layout else {
        panic!("expected typed layout JSON");
    };
    assert!(layout.layout().get("layout").is_some());

    let plan = renderer
        .render(RenderRequest::svg_plan(
            source,
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("SVG plan target should succeed");
    let RenderOutput::SvgPlan(Some(plan)) = plan else {
        panic!("expected typed SVG capability plan");
    };
    assert!(plan.is_ready());
}

#[cfg(feature = "svg")]
#[test]
fn completed_svg_evidence_freezes_family_and_theme_identity() {
    let theme = merman::svg::DiagramThemeCompiler::new()
        .compile_preset(merman::svg::ThemePreset::OneDark)
        .expect("one-dark theme should compile");
    let request = merman::SvgRequest::default();

    let output = Renderer::new()
        .render(
            RenderRequest::svg(
                "sequenceDiagram\nAlice->>Bob: Hello",
                OperationControl::new(),
                request,
            )
            .with_theme(theme.clone()),
        )
        .expect("themed sequence SVG should render");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected a themed sequence SVG");
    };

    assert!(output.svg().starts_with("<svg"));
    assert_eq!(
        output.evidence().family_id(),
        merman::DiagramFamilyId::SEQUENCE
    );
    assert_eq!(
        output.evidence().theme_recipe_fingerprint(),
        Some(theme.recipe_fingerprint())
    );
}

#[cfg(feature = "svg")]
#[test]
fn unthemed_svg_reports_theme_evidence_as_not_applicable() {
    let output = Renderer::new()
        .render(RenderRequest::svg(
            "flowchart TD\nA[Start] --> B[Done]",
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("unthemed SVG should render");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected an SVG output");
    };

    let summary = output.evidence().theme_evidence();
    assert_eq!(summary.status(), ThemeEvidenceStatus::NotApplicable);
    assert!(!summary.output_mutated());
    assert!(!summary.is_verified());
    assert!(summary.is_satisfied());
}

#[cfg(feature = "svg")]
#[test]
fn themed_state_svg_reports_verified_coarse_theme_evidence() {
    let theme = portable_state_theme();
    let output = Renderer::new()
        .render(
            RenderRequest::svg(
                "stateDiagram-v2\n[*] --> Ready\nReady --> [*]",
                OperationControl::new(),
                merman::SvgRequest::default(),
            )
            .with_theme(theme),
        )
        .expect("themed state SVG should render");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected a themed state SVG");
    };

    let summary = output.evidence().theme_evidence();
    assert_eq!(summary.status(), ThemeEvidenceStatus::Verified);
    assert!(!summary.output_mutated());
    assert!(summary.is_verified());
    assert!(summary.is_satisfied());

    let admission = output.admission();
    assert_eq!(admission.artifact_kind(), merman::RenderArtifactKind::Svg);
    assert_eq!(
        admission.artifact_digest(),
        <[u8; 32]>::from(Sha256::digest(output.svg().as_bytes()))
    );
}

#[cfg(feature = "svg")]
#[test]
fn pie_ordinal_palette_drives_terminal_slice_fills_and_verified_evidence() {
    let output = Renderer::new()
        .render(
            RenderRequest::svg(
                "pie\n  \"Alpha\" : 60\n  \"Hidden\" : 0.1\n  \"Gamma\" : 39.9\n",
                OperationControl::new(),
                merman::SvgRequest::default(),
            )
            .with_theme(portable_pie_palette_theme()),
        )
        .expect("themed Pie SVG should render");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected a themed Pie SVG");
    };

    let document = roxmltree::Document::parse(output.svg()).expect("valid Pie SVG");
    let slice_fills = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "pieCircle")
                })
        })
        .map(|node| node.attribute("fill").expect("Pie slice fill"))
        .collect::<Vec<_>>();
    assert_eq!(slice_fills, ["#ef4444", "#2563eb"]);
    let legend_styles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node.parent().is_some_and(|parent| {
                    parent.has_tag_name("g") && parent.attribute("class") == Some("legend")
                })
        })
        .map(|node| node.attribute("style").expect("Pie legend style"))
        .collect::<Vec<_>>();
    assert_eq!(
        legend_styles,
        [
            "fill: rgb(239, 68, 68); stroke: rgb(239, 68, 68);",
            "fill: rgb(34, 197, 94); stroke: rgb(34, 197, 94);",
            "fill: rgb(37, 99, 235); stroke: rgb(37, 99, 235);",
        ]
    );

    let summary = output.evidence().theme_evidence();
    assert_eq!(summary.status(), ThemeEvidenceStatus::Verified);
    assert!(summary.is_verified());
    assert!(summary.is_satisfied());
}

#[cfg(feature = "svg")]
#[test]
fn pie_site_palette_slot_outranks_the_typed_default() {
    let renderer = Renderer::new().with_engine(merman::Engine::new().with_site_config(
        merman::MermaidConfig::from_value(serde_json::json!({
            "themeVariables": {"pie1": "#111827"}
        })),
    ));
    let output = renderer
        .render(
            RenderRequest::svg(
                "pie\n  \"Site\" : 1\n  \"Typed\" : 1\n",
                OperationControl::new(),
                merman::SvgRequest::default(),
            )
            .with_theme(portable_pie_palette_theme()),
        )
        .expect("source-owned Pie palette slot should render");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected a themed Pie SVG");
    };

    let document = roxmltree::Document::parse(output.svg()).expect("valid Pie SVG");
    let slice_fills = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "pieCircle")
                })
        })
        .map(|node| node.attribute("fill").expect("Pie slice fill"))
        .collect::<Vec<_>>();
    assert_eq!(slice_fills, ["#111827", "#22c55e"]);
    assert_eq!(
        output.evidence().theme_evidence().status(),
        ThemeEvidenceStatus::Verified
    );
}

#[cfg(feature = "svg")]
#[test]
fn standalone_class_svg_separates_verified_relation_width_from_font_portability() {
    let styles = merman::svg::ThemeRuleSet::default().with_rule(
        merman::svg::ThemeRule::new(
            merman::svg::ThemeTarget::Edge,
            merman::svg::ThemeStylePatch::default()
                .with_stroke_width(5.0)
                .expect("valid Class relation width"),
        )
        .for_family(merman::DiagramFamilyId::CLASS),
    );
    let theme = merman::svg::DiagramThemeCompiler::new()
        .compile(merman::svg::DiagramThemeSpec::new().with_styles(styles))
        .expect("compile Class relation theme");
    let request = merman::SvgRequest {
        pipeline: Some(merman::svg::SvgPipeline::resvg_safe()),
        ..merman::SvgRequest::default()
    };
    let output = Renderer::new()
        .render(
            RenderRequest::svg(
                "classDiagram\nclass A\nclass B\nA --> B\n",
                OperationControl::new(),
                request,
            )
            .with_theme(theme),
        )
        .expect("best-effort standalone Class SVG should retain both evidence axes");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected a standalone Class SVG output");
    };

    assert_eq!(
        output.evidence().family_id(),
        merman::DiagramFamilyId::CLASS
    );
    assert_eq!(
        output.evidence().theme_evidence().status(),
        ThemeEvidenceStatus::Verified
    );
    assert!(output.svg().contains("stroke-width:5px !important"));
    let admission = output.admission();
    assert_eq!(
        admission.status(),
        merman::TargetAdmissionStatus::HostDependent
    );
    assert!(!admission.status().is_portable());
    assert_ne!(admission.receipt_digest(), [0; 32]);
    assert!(
        admission
            .reasons()
            .contains(&merman::TargetAdmissionReason::SvgFontsNotSelfContained)
    );
    assert_eq!(
        admission.artifact_digest(),
        <[u8; 32]>::from(Sha256::digest(output.svg().as_bytes()))
    );
}

#[cfg(feature = "svg")]
#[test]
fn custom_postprocessing_is_visible_in_coarse_theme_evidence() {
    let theme = portable_state_theme();
    let request = merman::SvgRequest {
        pipeline: Some(
            merman::svg::SvgPipeline::parity()
                .with_postprocessor(merman::svg::RootBackgroundPostprocessor::new("#111827")),
        ),
        ..Default::default()
    };
    let output = Renderer::new()
        .render(
            RenderRequest::svg(
                "stateDiagram-v2\n[*] --> Ready\nReady --> [*]",
                OperationControl::new(),
                request,
            )
            .with_theme(theme),
        )
        .expect("postprocessed themed SVG should render in best-effort mode");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected a postprocessed SVG");
    };

    let summary = output.evidence().theme_evidence();
    assert_eq!(summary.status(), ThemeEvidenceStatus::Residual);
    assert!(summary.output_mutated());
    assert!(!summary.is_verified());
    assert!(!summary.is_satisfied());
}

#[cfg(feature = "svg")]
#[test]
fn rendered_document_retains_terminal_resource_identity() {
    let theme = merman::svg::DiagramThemeCompiler::new()
        .compile_preset(merman::svg::ThemePreset::OneDark)
        .expect("one-dark theme should compile");

    let output = Renderer::new()
        .render(
            RenderRequest::document(
                "flowchart TD\nA[Start] --> B[Done]",
                OperationControl::new(),
                merman::SvgRequest::default(),
            )
            .with_theme(theme.clone()),
        )
        .expect("rendered document should complete");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a rendered document");
    };

    assert!(document.svg().starts_with("<svg"));
    assert_ne!(document.resource_fingerprint().as_bytes(), &[0; 32]);
    assert_eq!(
        document.portability().family_id(),
        document.evidence().family_id()
    );
    assert_eq!(
        document.portability().resource_fingerprint(),
        document.resource_fingerprint()
    );
    assert_eq!(
        document.portability().font_catalog_fingerprint(),
        document.evidence().font_catalog_fingerprint()
    );
    assert_eq!(
        document.standalone_svg_admission().artifact_kind(),
        merman::RenderArtifactKind::Svg
    );
    assert_eq!(
        document.standalone_svg_admission().document_digest(),
        document.document_digest()
    );
    assert_eq!(
        document.standalone_svg_admission().resource_fingerprint(),
        document.resource_fingerprint()
    );
    assert_eq!(
        document
            .standalone_svg_admission()
            .font_catalog_fingerprint(),
        document.evidence().font_catalog_fingerprint()
    );
    assert_eq!(
        document.evidence().theme_recipe_fingerprint(),
        Some(theme.recipe_fingerprint())
    );
}

#[cfg(feature = "svg")]
#[test]
fn sequence_document_seals_prepared_text_with_the_embedded_full_font() {
    const SOURCE: &str = "sequenceDiagram\nautonumber\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Prepared message\nNote over Alice,Bob: Prepared note";

    let font_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let font_catalog = merman::svg::FontCatalogSpec::new([merman::svg::FontAssetSpec::new(
        "sequence-excalifont",
        font_bytes,
    )])
    .with_available_sources([merman::svg::FontSource::Embedded])
    .with_embedding_requirement(merman::svg::FontEmbeddingRequirement::FullFont);
    let theme = merman::svg::DiagramThemeCompiler::new()
        .compile(
            merman::svg::DiagramThemeSpec::new()
                .with_assets(merman::svg::ThemeAssets::default().with_font_catalog(font_catalog))
                .with_canvas(
                    merman::svg::CanvasSpec::solid("#f7f3e8").expect("valid Sequence canvas"),
                ),
        )
        .expect("compile Sequence full-font theme");
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        pipeline: Some(merman::svg::SvgPipeline::resvg_safe()),
        ..merman::SvgRequest::default()
    };
    let renderer = Renderer::new().with_engine(merman::Engine::new().with_site_config(
        merman::MermaidConfig::from_value(serde_json::json!({
            "htmlLabels": false,
            "themeVariables": {"fontFamily": "Excalifont"}
        })),
    ));
    let output = renderer
        .render(
            RenderRequest::document(SOURCE, OperationControl::new(), request.clone())
                .with_theme(theme.clone()),
        )
        .expect("portable Sequence document should use the embedded theme font");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a completed Sequence document");
    };

    assert!(document.portability().prepared_text_evidence_valid());
    assert!(document.portability().is_evidence_valid());
    assert!(!document.portability().is_host_dependent());
    assert!(document.portability().reasons().is_empty());

    let admission = document.standalone_svg_admission();
    assert_eq!(admission.status(), merman::TargetAdmissionStatus::Portable);
    assert_eq!(admission.font_source(), merman::TargetFontSource::Embedded);
    assert!(admission.reasons().is_empty());
    assert!(document.svg().contains("data-merman-typed-fonts=\"v1\""));
    assert!(document.svg().contains("@font-face{"));
    assert!(document.svg().contains("font-family:Excalifont"));
    assert!(!document.svg().contains("merman-prepared-"));

    let renderer = Renderer::new().with_engine(merman::Engine::new().with_site_config(
        merman::MermaidConfig::from_value(serde_json::json!({
            "htmlLabels": false,
            "fontFamily": "Missing Root Font",
            "themeVariables": {"fontFamily": "Excalifont"}
        })),
    ));
    let error = renderer
        .render(RenderRequest::document(SOURCE, OperationControl::new(), request).with_theme(theme))
        .expect_err("the explicit root font must retain Sequence role ownership");
    let RenderError::Svg(error) = error else {
        panic!("missing selected Sequence font should fail during typed layout: {error}");
    };
    assert!(matches!(error, merman::svg::RenderError::TextLayout(_)));
    assert!(
        error
            .to_string()
            .contains("could not resolve an admitted font or glyph")
    );
}

#[cfg(feature = "svg")]
#[test]
fn rendered_document_digests_are_deterministic_for_one_operation_identity() {
    fn render() -> merman::RenderedDocument {
        let output = Renderer::new()
            .render(RenderRequest::document(
                "flowchart TD\nA[Start] --> B[Done]",
                OperationControl::new(),
                merman::SvgRequest::default(),
            ))
            .expect("rendered document should complete");
        let RenderOutput::Document(Some(document)) = output else {
            panic!("expected a rendered document");
        };
        document
    }

    let first = render();
    let second = render();
    assert_eq!(first.document_digest(), second.document_digest());
    assert_eq!(
        first.standalone_svg_admission().target_evidence_digest(),
        second.standalone_svg_admission().target_evidence_digest()
    );
    assert_eq!(
        first.standalone_svg_admission().receipt_digest(),
        second.standalone_svg_admission().receipt_digest()
    );
    assert_eq!(
        first.standalone_svg_admission().artifact_digest(),
        second.standalone_svg_admission().artifact_digest()
    );
}

#[cfg(feature = "svg")]
#[test]
fn rendered_document_digest_binds_actual_host_measurement_identity() {
    let first = render_host_measured_document_with_profile(
        merman::svg::ThemePortabilityRequirement::BestEffort,
        "merman.test-display-host-a",
    );
    let second = render_host_measured_document_with_profile(
        merman::svg::ThemePortabilityRequirement::BestEffort,
        "merman.test-display-host-b",
    );

    assert_eq!(first.svg(), second.svg());
    assert_ne!(first.document_digest(), second.document_digest());
}

#[cfg(feature = "svg")]
#[test]
fn unthemed_document_digest_binds_the_effective_portability_requirement() {
    fn render(portability: merman::svg::ThemePortabilityRequirement) -> merman::RenderedDocument {
        let request = merman::SvgRequest {
            environment: merman::SvgEnvironment::deterministic()
                .with_theme_portability_requirement(portability),
            ..merman::SvgRequest::default()
        };
        let output = Renderer::new()
            .render(RenderRequest::document(
                "info",
                OperationControl::new(),
                request,
            ))
            .expect("unthemed document should complete");
        let RenderOutput::Document(Some(document)) = output else {
            panic!("expected an unthemed rendered document");
        };
        document
    }

    let best_effort = render(merman::svg::ThemePortabilityRequirement::BestEffort);
    let require_portable = render(merman::svg::ThemePortabilityRequirement::RequirePortable);
    assert_eq!(best_effort.svg(), require_portable.svg());
    assert_ne!(
        best_effort.document_digest(),
        require_portable.document_digest()
    );
}

#[cfg(feature = "svg")]
#[test]
fn host_display_measurements_make_document_and_svg_target_host_dependent() {
    let document =
        render_host_measured_document(merman::svg::ThemePortabilityRequirement::BestEffort);

    assert!(
        document
            .evidence()
            .measurement()
            .entries()
            .iter()
            .any(|entry| {
                entry.count() > 0
                    && entry.provenance().source == merman::svg::TextMeasurementSource::Host
            })
    );
    assert!(document.portability().is_host_dependent());
    assert!(
        document
            .portability()
            .reasons()
            .contains(&merman::TargetAdmissionReason::HostDependentTextLayout)
    );
    assert_eq!(
        document.standalone_svg_admission().status(),
        merman::TargetAdmissionStatus::HostDependent
    );
    assert!(
        document
            .standalone_svg_admission()
            .reasons()
            .contains(&merman::TargetAdmissionReason::HostDependentTextLayout)
    );
}

#[cfg(feature = "svg")]
#[test]
fn require_portable_svg_target_rejects_actual_host_display_measurements() {
    let identity = merman::svg::TextMeasurementProfileIdentity::new(
        merman::svg::MeasurementProfileId::new("merman.test-strict-svg-host")
            .expect("test profile id"),
        "render-operation-test@1",
    )
    .expect("test profile identity");
    let measurement = merman::svg::TextMeasurementPolicy::host_display(
        identity,
        Arc::new(FixedHostTextMeasurer),
        merman::svg::TextMeasurementPhase::ALL,
    );
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic()
            .with_text_measurement_policy(measurement)
            .with_theme_portability_requirement(
                merman::svg::ThemePortabilityRequirement::RequirePortable,
            ),
        pipeline: Some(merman::svg::SvgPipeline::resvg_safe()),
        ..merman::SvgRequest::default()
    };

    let error = Renderer::new()
        .render(
            RenderRequest::svg(
                "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\nA[Start] --> B[Done]",
                OperationControl::new(),
                request,
            )
            .with_theme(flowchart_paint_theme()),
        )
        .expect_err("strict SVG admission must reject actual host measurement provenance");
    let RenderError::TargetAdmission(error) = error else {
        panic!("strict SVG rejection must retain target evidence: {error}");
    };
    assert_eq!(
        error.receipt().artifact_kind(),
        merman::RenderArtifactKind::Svg
    );
    assert_eq!(
        error.receipt().status(),
        merman::TargetAdmissionStatus::HostDependent
    );
    assert!(
        error
            .receipt()
            .reasons()
            .contains(&merman::TargetAdmissionReason::HostDependentTextLayout)
    );
}

#[cfg(feature = "svg")]
#[test]
fn require_portable_svg_output_retains_the_exact_target_receipt() {
    let font_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let font_catalog = merman::svg::FontCatalogSpec::new([merman::svg::FontAssetSpec::new(
        "strict-svg-excalifont",
        font_bytes,
    )])
    .with_available_sources([merman::svg::FontSource::Embedded])
    .with_embedding_requirement(merman::svg::FontEmbeddingRequirement::FullFont);
    let theme = merman::svg::DiagramThemeCompiler::new()
        .compile(
            merman::svg::DiagramThemeSpec::new()
                .with_assets(merman::svg::ThemeAssets::default().with_font_catalog(font_catalog))
                .with_canvas(
                    merman::svg::CanvasSpec::solid("#f7f3e8").expect("valid Sequence canvas"),
                ),
        )
        .expect("compile Sequence full-font theme");
    let renderer = Renderer::new().with_engine(merman::Engine::new().with_site_config(
        merman::MermaidConfig::from_value(serde_json::json!({
            "htmlLabels": false,
            "themeVariables": {"fontFamily": "Excalifont"}
        })),
    ));
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        pipeline: Some(merman::svg::SvgPipeline::resvg_safe()),
        ..merman::SvgRequest::default()
    };

    let source =
        "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Portable message";
    let output = renderer
        .render(
            RenderRequest::svg(source, OperationControl::new(), request.clone())
                .with_theme(theme.clone()),
        )
        .expect("portable standalone SVG should pass strict target admission");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected a strict standalone SVG output");
    };
    let admission = output.admission();
    assert_eq!(admission.artifact_kind(), merman::RenderArtifactKind::Svg);
    assert_eq!(admission.status(), merman::TargetAdmissionStatus::Portable);
    assert_eq!(
        admission.artifact_digest(),
        <[u8; 32]>::from(Sha256::digest(output.svg().as_bytes()))
    );
    assert_ne!(admission.receipt_digest(), [0; 32]);

    let document = renderer
        .render(RenderRequest::document(source, OperationControl::new(), request).with_theme(theme))
        .expect("the same resvg-safe request should complete as a document");
    let RenderOutput::Document(Some(document)) = document else {
        panic!("expected a rendered document");
    };
    assert_eq!(output.svg(), document.svg());
    assert_eq!(
        admission.resource_fingerprint(),
        document.standalone_svg_admission().resource_fingerprint()
    );
    assert_eq!(
        admission.artifact_digest(),
        document.standalone_svg_admission().artifact_digest()
    );
    assert_eq!(
        admission.document_digest(),
        document.standalone_svg_admission().document_digest()
    );
    assert_eq!(
        admission.target_evidence_digest(),
        document.standalone_svg_admission().target_evidence_digest()
    );
    assert_eq!(
        admission.receipt_digest(),
        document.standalone_svg_admission().receipt_digest()
    );
}

#[cfg(feature = "svg")]
#[test]
fn svg_admission_preserves_the_caller_selected_pipeline() {
    const SOURCE: &str = "---\nconfig:\n  htmlLabels: true\n  flowchart:\n    htmlLabels: true\n---\nflowchart TD\nA[Alpha] --> B[Beta]";

    fn render_best_effort(pipeline: merman::svg::SvgPipeline) -> merman::SvgOutput {
        let request = merman::SvgRequest {
            pipeline: Some(pipeline),
            ..merman::SvgRequest::default()
        };
        let output = Renderer::new()
            .render(
                RenderRequest::svg(SOURCE, OperationControl::new(), request)
                    .with_theme(flowchart_paint_theme()),
            )
            .expect("best-effort SVG should retain the selected pipeline output");
        let RenderOutput::Svg(Some(output)) = output else {
            panic!("expected standalone SVG output");
        };
        output
    }

    fn render_strict(pipeline: merman::svg::SvgPipeline) -> merman::TargetAdmissionError {
        let request = merman::SvgRequest {
            environment: merman::SvgEnvironment::deterministic()
                .with_theme_portability_requirement(
                    merman::svg::ThemePortabilityRequirement::RequirePortable,
                ),
            pipeline: Some(pipeline),
            ..merman::SvgRequest::default()
        };
        let error = Renderer::new()
            .render(
                RenderRequest::svg(SOURCE, OperationControl::new(), request)
                    .with_theme(flowchart_paint_theme()),
            )
            .expect_err("strict SVG should reject an incompatible selected pipeline artifact");
        let RenderError::TargetAdmission(error) = error else {
            panic!("strict SVG rejection should retain target evidence: {error}");
        };
        error
    }

    let parity = render_best_effort(merman::svg::SvgPipeline::parity());
    assert!(parity.svg().contains("<foreignObject"), "{}", parity.svg());
    assert!(
        !parity
            .svg()
            .contains(r#"data-merman-foreignobject="fallback""#),
        "{}",
        parity.svg()
    );
    let parity_digest = <[u8; 32]>::from(Sha256::digest(parity.svg().as_bytes()));
    let parity_receipt = parity.admission();
    assert_eq!(parity_receipt.artifact_digest(), parity_digest);
    assert_eq!(
        parity_receipt.status(),
        merman::TargetAdmissionStatus::Rejected
    );
    assert!(
        parity_receipt
            .reasons()
            .contains(&merman::TargetAdmissionReason::SvgTerminalValidationFailed)
    );
    let parity_error = render_strict(merman::svg::SvgPipeline::parity());
    assert_eq!(parity_error.receipt().artifact_digest(), parity_digest);
    assert_eq!(
        parity_error.receipt().status(),
        merman::TargetAdmissionStatus::Rejected
    );

    let readable = render_best_effort(merman::svg::SvgPipeline::readable());
    assert!(
        readable.svg().contains("<foreignObject"),
        "{}",
        readable.svg()
    );
    assert!(
        readable
            .svg()
            .contains(r#"data-merman-foreignobject="fallback""#),
        "{}",
        readable.svg()
    );
    let readable_digest = <[u8; 32]>::from(Sha256::digest(readable.svg().as_bytes()));
    let readable_error = render_strict(merman::svg::SvgPipeline::readable());
    assert_eq!(readable_error.receipt().artifact_digest(), readable_digest);

    let resvg_safe = render_best_effort(merman::svg::SvgPipeline::resvg_safe());
    assert!(
        !resvg_safe.svg().contains("<foreignObject"),
        "{}",
        resvg_safe.svg()
    );
    assert_ne!(parity.svg(), readable.svg());
    assert_ne!(readable.svg(), resvg_safe.svg());
}

#[cfg(feature = "svg")]
#[test]
fn best_effort_svg_seals_terminal_validation_failure_without_discarding_the_artifact() {
    const SOURCE: &str = "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\nA[Alpha] --> B[Beta]";

    let render = |pipeline| {
        let output = Renderer::new()
            .render(
                RenderRequest::svg(
                    SOURCE,
                    OperationControl::new(),
                    merman::SvgRequest {
                        pipeline: Some(pipeline),
                        ..merman::SvgRequest::default()
                    },
                )
                .with_theme(flowchart_paint_theme()),
            )
            .expect("best-effort SVG should return the selected artifact");
        let RenderOutput::Svg(Some(output)) = output else {
            panic!("expected standalone SVG output");
        };
        output
    };

    let baseline = render(merman::svg::SvgPipeline::parity());
    let output =
        render(merman::svg::SvgPipeline::parity().with_postprocessor(MissingFragmentPostprocessor));
    assert!(output.svg().contains("data-test-missing-fragment"));
    let admission = output.admission();
    assert_eq!(
        admission.artifact_digest(),
        <[u8; 32]>::from(Sha256::digest(output.svg().as_bytes()))
    );
    assert_eq!(admission.status(), merman::TargetAdmissionStatus::Rejected);
    assert!(
        admission
            .reasons()
            .contains(&merman::TargetAdmissionReason::SvgTerminalValidationFailed)
    );
    assert_ne!(
        admission.resource_fingerprint(),
        baseline.admission().resource_fingerprint()
    );
    assert_ne!(
        admission.document_digest(),
        baseline.admission().document_digest()
    );

    let strict_error = Renderer::new()
        .render(
            RenderRequest::svg(
                SOURCE,
                OperationControl::new(),
                merman::SvgRequest {
                    environment: merman::SvgEnvironment::deterministic()
                        .with_theme_portability_requirement(
                            merman::svg::ThemePortabilityRequirement::RequirePortable,
                        ),
                    pipeline: Some(
                        merman::svg::SvgPipeline::parity()
                            .with_postprocessor(MissingFragmentPostprocessor),
                    ),
                    ..merman::SvgRequest::default()
                },
            )
            .with_theme(flowchart_paint_theme()),
        )
        .expect_err("strict SVG should reject the same exact invalid artifact");
    let RenderError::TargetAdmission(strict_error) = strict_error else {
        panic!("strict rejection should retain target evidence: {strict_error}");
    };
    assert_eq!(
        strict_error.receipt().artifact_digest(),
        admission.artifact_digest()
    );
    assert!(
        strict_error
            .receipt()
            .reasons()
            .contains(&merman::TargetAdmissionReason::SvgTerminalValidationFailed)
    );
}

#[cfg(feature = "svg")]
#[test]
fn standalone_svg_terminal_observation_propagates_resource_limits() {
    for pipeline in [None, Some(merman::svg::SvgPipeline::parity())] {
        let resources = merman::svg::RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(merman::svg::ResourceLimitId::MaxSvgElements, 2)
            .expect("valid SVG element limit");
        let request = merman::SvgRequest {
            environment: merman::SvgEnvironment::deterministic().with_resource_policy(resources),
            pipeline,
            ..merman::SvgRequest::default()
        };
        let error = Renderer::new()
            .render(RenderRequest::svg(
                "flowchart TD\nA --> B",
                OperationControl::new(),
                request,
            ))
            .expect_err("terminal SVG observation must not downgrade a resource limit");

        assert!(matches!(
            error,
            RenderError::ResourceLimitExceeded(limit)
                if limit.id == "max_svg_elements"
                    && limit.phase == "svg_postprocess"
                    && limit.maximum == 2
        ));
    }
}

#[cfg(feature = "png")]
#[test]
fn host_display_measurements_make_png_target_host_dependent() {
    let document =
        render_host_measured_document(merman::svg::ThemePortabilityRequirement::BestEffort);

    let output = document
        .export_png(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect("best-effort PNG export should retain host-dependent evidence");
    assert_eq!(
        output.admission().status(),
        merman::TargetAdmissionStatus::HostDependent
    );
    assert!(
        output
            .admission()
            .reasons()
            .contains(&merman::TargetAdmissionReason::HostDependentTextLayout)
    );
}

#[cfg(feature = "png")]
#[test]
fn require_portable_rejects_actual_host_display_measurements() {
    let document =
        render_host_measured_document(merman::svg::ThemePortabilityRequirement::RequirePortable);

    let error = document
        .export_png(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect_err("strict PNG admission must reject actual host measurement provenance");
    let RenderError::TargetAdmission(error) = error else {
        panic!("strict host measurement rejection must retain target evidence: {error}");
    };
    assert_eq!(
        error.receipt().status(),
        merman::TargetAdmissionStatus::HostDependent
    );
    assert!(
        error
            .receipt()
            .reasons()
            .contains(&merman::TargetAdmissionReason::HostDependentTextLayout)
    );
}

#[cfg(feature = "png")]
#[test]
fn require_portable_without_a_theme_does_not_fall_back_to_best_effort() {
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        pipeline: Some(
            merman::svg::SvgPipeline::resvg_safe()
                .with_postprocessor(merman::svg::RootBackgroundPostprocessor::new("#111827")),
        ),
        ..merman::SvgRequest::default()
    };
    let output = Renderer::new()
        .render(RenderRequest::document(
            "info",
            OperationControl::new(),
            request,
        ))
        .expect("target-neutral document completion should retain rejected evidence");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected an unthemed rendered document");
    };
    assert_eq!(document.evidence().theme_recipe_fingerprint(), None);
    assert!(
        document
            .portability()
            .reasons()
            .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete)
    );

    let error = document
        .export_png(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect_err("the unthemed session must retain the host's strict portability requirement");
    let RenderError::TargetAdmission(error) = error else {
        panic!("strict unthemed rejection must retain target evidence: {error}");
    };
    assert_eq!(
        error.receipt().artifact_kind(),
        merman::RenderArtifactKind::Png
    );
    assert_eq!(
        error.receipt().status(),
        merman::TargetAdmissionStatus::Rejected
    );
}

#[cfg(feature = "svg")]
#[test]
fn strict_document_completion_preserves_the_standalone_svg_receipt() {
    let theme = portable_state_theme();
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        ..Default::default()
    };

    let output = Renderer::new()
        .render(
            RenderRequest::document(
                "stateDiagram-v2\n[*] --> Ready\nReady --> [*]",
                OperationControl::new(),
                request,
            )
            .with_theme(theme),
        )
        .expect("document completion must not be preempted by one target's admission");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a completed document");
    };
    assert_eq!(
        document.standalone_svg_admission().artifact_kind(),
        merman::RenderArtifactKind::Svg
    );
    assert_ne!(
        document.standalone_svg_admission().status(),
        merman::TargetAdmissionStatus::Portable
    );
    assert!(!document.standalone_svg_admission().reasons().is_empty());
    assert_eq!(
        document.standalone_svg_admission().document_digest(),
        document.document_digest()
    );
}

#[cfg(feature = "png")]
#[test]
fn strict_document_png_projection_is_evaluated_by_the_png_target() {
    let theme = portable_state_theme();
    let svg = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        ..Default::default()
    };

    let output = Renderer::new()
        .render(
            RenderRequest::document(
                "stateDiagram-v2\n[*] --> Ready\nReady --> [*]",
                OperationControl::new(),
                svg,
            )
            .with_theme(theme),
        )
        .expect("one target's admission must not preempt document completion");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a completed document");
    };
    let error = document
        .export_png(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect_err("strict PNG admission should reject host-dependent text");
    let RenderError::TargetAdmission(error) = error else {
        panic!("strict PNG rejection must preserve target-owned evidence: {error}");
    };
    assert_eq!(
        error.receipt().artifact_kind(),
        merman::RenderArtifactKind::Png,
        "the standalone SVG receipt must not preempt target-local PNG admission"
    );
    assert_ne!(
        error.receipt().status(),
        merman::TargetAdmissionStatus::Portable
    );
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn one_rendered_document_projects_all_native_targets_with_atomic_receipts() {
    let output = Renderer::new()
        .render(RenderRequest::document(
            "info",
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("rendered document should complete");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a rendered document");
    };

    let png = document
        .export_png(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect("PNG projection should succeed");
    let jpeg = document
        .export_jpeg(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect("JPEG projection should succeed");
    let pdf = document
        .export_pdf(
            &merman::svg::export::PdfOptions::default(),
            OperationControl::new(),
        )
        .expect("PDF projection should succeed");

    assert!(png.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(png.plan(), png.export_report().raster());
    assert_eq!(jpeg.plan(), jpeg.export_report().raster());
    assert_eq!(pdf.plan(), pdf.export_report().filters());

    for (kind, bytes, receipt) in [
        (
            merman::RenderArtifactKind::Png,
            png.bytes(),
            png.admission(),
        ),
        (
            merman::RenderArtifactKind::Jpeg,
            jpeg.bytes(),
            jpeg.admission(),
        ),
        (
            merman::RenderArtifactKind::Pdf,
            pdf.bytes(),
            pdf.admission(),
        ),
    ] {
        assert_eq!(receipt.artifact_kind(), kind);
        assert_eq!(receipt.document_digest(), document.document_digest());
        assert_eq!(
            receipt.resource_fingerprint(),
            document.resource_fingerprint()
        );
        assert_eq!(
            receipt.font_catalog_fingerprint(),
            document.evidence().font_catalog_fingerprint()
        );
        let expected_artifact_digest: [u8; 32] = Sha256::digest(bytes).into();
        assert_eq!(receipt.artifact_digest(), expected_artifact_digest);
        assert_ne!(
            receipt.target_evidence_digest(),
            receipt.artifact_digest(),
            "target evidence and artifact bytes are independently bound"
        );
        assert_ne!(
            receipt.status(),
            merman::TargetAdmissionStatus::Rejected,
            "a successfully projected built-in target must not silently reject its own artifact"
        );
    }

    let receipts = [
        document.standalone_svg_admission(),
        png.admission(),
        jpeg.admission(),
        pdf.admission(),
    ];
    for receipt in &receipts {
        assert_eq!(receipt.document_digest(), document.document_digest());
        assert_ne!(receipt.receipt_digest(), [0; 32]);
    }
    for (index, receipt) in receipts.iter().enumerate() {
        for other in &receipts[index + 1..] {
            assert_ne!(
                receipt.receipt_digest(),
                other.receipt_digest(),
                "one document's target receipts must retain distinct canonical identities"
            );
        }
    }
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn rendered_document_prepares_each_native_target_before_encoding() {
    let output = Renderer::new()
        .render(RenderRequest::document(
            "info",
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("rendered document should complete");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a rendered document");
    };

    let png = document
        .prepare_png_export(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect("PNG should prepare");
    assert_eq!(png.plan(), png.export_report().raster());
    let png = png.encode().expect("prepared PNG should encode");

    let jpeg = document
        .prepare_jpeg_export(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect("JPEG should prepare");
    assert_eq!(jpeg.plan(), jpeg.export_report().raster());
    let jpeg = jpeg.encode().expect("prepared JPEG should encode");

    let pdf = document
        .prepare_pdf_export(
            &merman::svg::export::PdfOptions::default(),
            OperationControl::new(),
        )
        .expect("PDF should prepare");
    assert_eq!(pdf.plan(), pdf.export_report().filters());
    let pdf = pdf.encode().expect("prepared PDF should encode");

    for receipt in [png.admission(), jpeg.admission(), pdf.admission()] {
        assert_eq!(receipt.document_digest(), document.document_digest());
    }
}

#[cfg(feature = "png")]
#[test]
fn png_output_retains_the_same_coarse_render_evidence() {
    let theme = merman::svg::DiagramThemeCompiler::new()
        .compile_preset(merman::svg::ThemePreset::OneDark)
        .expect("one-dark theme should compile");
    let svg = merman::SvgRequest::default();

    let output = Renderer::new()
        .render(
            RenderRequest::png(
                "info",
                OperationControl::new(),
                merman::PngRequest {
                    svg,
                    options: merman::svg::export::RasterOptions::default(),
                },
            )
            .with_theme(theme.clone()),
        )
        .expect("themed PNG should render");
    let RenderOutput::Png(Some(output)) = output else {
        panic!("expected a themed PNG");
    };

    assert!(output.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(output.export_report().raster(), output.plan());
    assert_eq!(
        output.admission().artifact_kind(),
        merman::RenderArtifactKind::Png
    );
    assert_eq!(output.evidence().family_id(), merman::DiagramFamilyId::INFO);
    assert_eq!(
        output.evidence().theme_recipe_fingerprint(),
        Some(theme.recipe_fingerprint())
    );
}

#[cfg(feature = "svg")]
#[test]
fn semantic_artifact_exposes_compatibility_json_without_family_types() {
    let artifact = Renderer::new()
        .prepare_semantic("flowchart TD\nA --> B", OperationControl::new())
        .expect("parse should succeed")
        .expect("diagram should be detected");
    let json = artifact
        .compatibility_json()
        .expect("compatibility JSON should be projected");
    assert_eq!(json["type"], "flowchart-v2");
}

#[test]
fn compatibility_json_observes_the_artifact_operation_control() {
    let control = OperationControl::new();
    let artifact = Renderer::new()
        .prepare_semantic("flowchart TD\nA --> B", control.clone())
        .expect("parse should succeed")
        .expect("diagram should be detected");
    control.cancel();

    let error = artifact
        .compatibility_json()
        .expect_err("semantic projection must observe cancellation");
    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Semantic
    ));
}

#[cfg(feature = "svg")]
#[test]
fn layout_json_observes_cancellation_after_layout_preparation() {
    let control = OperationControl::new();
    let identity = merman::svg::TextMeasurementProfileIdentity::new(
        merman::svg::MeasurementProfileId::new("merman.test-cancelling-host")
            .expect("static profile id"),
        "render-operation-test@1",
    )
    .expect("static profile identity");
    let policy = merman::svg::TextMeasurementPolicy::host_display(
        identity,
        Arc::new(CancellingTextMeasurer {
            control: control.clone(),
        }),
        merman::svg::TextMeasurementPhase::ALL,
    );
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_text_measurement_policy(policy),
        ..Default::default()
    };

    let error = Renderer::new()
        .render(RenderRequest::layout_json(
            "flowchart TD\nA[Start] --> B[Done]",
            control,
            request,
        ))
        .expect_err("layout projection must not return after its control terminates");
    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Layout
                && cancelled.reason == merman::CancelReason::Requested
    ));
}

#[cfg(feature = "svg")]
#[test]
fn svg_request_cancellation_is_not_reported_as_a_resource_limit() {
    let control = OperationControl::new();
    control.cancel();
    let error = Renderer::new()
        .render(RenderRequest::svg(
            "flowchart TD\nA --> B",
            control,
            merman::SvgRequest::default(),
        ))
        .expect_err("cancelled SVG request must stop");
    assert!(matches!(error, RenderError::Cancelled(_)));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_request_uses_target_local_grid_policy_and_common_cancellation() {
    let control = OperationControl::new();
    control.cancel();
    let error = Renderer::new()
        .render(RenderRequest::ascii(
            "flowchart TD\nA --> B",
            control,
            merman::AsciiRequest::default(),
        ))
        .expect_err("cancelled ASCII request must stop");
    assert!(matches!(error, RenderError::Cancelled(_)));
}
