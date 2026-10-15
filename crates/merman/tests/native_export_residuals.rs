#![cfg(all(feature = "svg", feature = "diagram-info"))]

use merman::svg::{
    SvgPipeline, SvgPostprocessContext, SvgPostprocessor, ThemePortabilityRequirement,
};
use merman::{
    OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest,
    TargetAdmissionReason,
};
use std::borrow::Cow;

#[cfg(any(feature = "png", feature = "pdf"))]
const DIRECTIONAL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" filterUnits="userSpaceOnUse" x="-20" y="-20" width="80" height="80"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><g transform="translate(60 20) rotate(90)" filter="url(#effect)"><rect width="20" height="10" fill="red"/></g></svg>"##;
#[cfg(feature = "pdf")]
const IMPLICIT_VIEWPORT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width:400px"><rect x="-10" y="10" width="20" height="20" fill="red"/></svg>"#;
const FALLBACK_CSS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><g data-merman-fallback-css-residual="true"><rect width="20" height="10" fill="red"/></g></svg>"#;

struct ReplaceScene(&'static str);

impl SvgPostprocessor for ReplaceScene {
    fn name(&self) -> &'static str {
        "test-export-residual-scene"
    }

    fn process<'a>(
        &self,
        _: Cow<'a, str>,
        _: &SvgPostprocessContext<'_>,
    ) -> merman::svg::RenderResult<Cow<'a, str>> {
        Ok(Cow::Owned(self.0.to_owned()))
    }
}

#[cfg(any(feature = "png", feature = "pdf"))]
fn document(scene: &'static str, strict: bool) -> merman::RenderedDocument {
    let environment =
        SvgEnvironment::deterministic().with_theme_portability_requirement(if strict {
            ThemePortabilityRequirement::RequirePortable
        } else {
            ThemePortabilityRequirement::BestEffort
        });
    let result = Renderer::new()
        .render(RenderRequest::document(
            "info",
            OperationControl::new(),
            SvgRequest {
                environment,
                pipeline: Some(SvgPipeline::resvg_safe().with_postprocessor(ReplaceScene(scene))),
                ..Default::default()
            },
        ))
        .expect("completing a document does not enforce one output target's strict admission");
    let RenderOutput::Document(Some(document)) = result else {
        panic!("expected document");
    };
    document
}

fn assert_residual(receipt: &merman::TargetAdmissionReceipt, reason: TargetAdmissionReason) {
    assert!(!receipt.status().is_portable());
    assert!(receipt.reasons().contains(&reason), "{receipt:?}");
    assert_ne!(receipt.target_evidence_digest(), [0; 32]);
    assert_ne!(receipt.receipt_digest(), [0; 32]);
}

fn assert_strict_reason(error: merman::RenderError, reason: TargetAdmissionReason) {
    let merman::RenderError::TargetAdmission(error) = error else {
        panic!("strict quality admission must own this error: {error}");
    };
    assert_residual(error.receipt(), reason);
}

#[cfg(feature = "png")]
#[test]
fn raster_directional_filter_residual_keeps_default_output_and_rejects_strict_portability() {
    for scene in [DIRECTIONAL, FALLBACK_CSS] {
        let reason = if scene == DIRECTIONAL {
            TargetAdmissionReason::NativeDirectionalFilterTransformResidual
        } else {
            TargetAdmissionReason::FallbackCssResidual
        };
        let output = document(scene, false)
            .export_png(&Default::default(), OperationControl::new())
            .expect("best-effort PNG must retain output with a quality residual");
        assert!(output.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_residual(output.admission(), reason);
        let error = document(scene, true)
            .export_png(&Default::default(), OperationControl::new())
            .expect_err("only explicit strict portability rejects the residual");
        assert_strict_reason(error, reason);
    }
}

#[cfg(feature = "pdf")]
#[test]
fn pdf_quality_residuals_keep_default_output_and_reject_strict_portability() {
    for (scene, reason) in [
        (
            DIRECTIONAL,
            TargetAdmissionReason::NativeDirectionalFilterTransformResidual,
        ),
        (
            IMPLICIT_VIEWPORT,
            TargetAdmissionReason::PdfImplicitViewportClippingResidual,
        ),
        (FALLBACK_CSS, TargetAdmissionReason::FallbackCssResidual),
    ] {
        let output = document(scene, false)
            .export_pdf(&Default::default(), OperationControl::new())
            .expect("best-effort PDF must retain output with a quality residual");
        assert!(output.bytes().starts_with(b"%PDF"));
        assert_residual(output.admission(), reason);
        let error = document(scene, true)
            .export_pdf(&Default::default(), OperationControl::new())
            .expect_err("explicit strict portability rejects the residual");
        assert_strict_reason(error, reason);
    }
}

#[test]
fn standalone_fallback_css_residual_is_a_quality_downgrade_only_on_finalized_svg() {
    let render = |pipeline| {
        Renderer::new()
            .render(RenderRequest::svg(
                "info",
                OperationControl::new(),
                SvgRequest {
                    pipeline: Some(pipeline),
                    ..Default::default()
                },
            ))
            .expect("default SVG preserves output")
    };
    let RenderOutput::Svg(Some(safe)) =
        render(SvgPipeline::resvg_safe().with_postprocessor(ReplaceScene(FALLBACK_CSS)))
    else {
        panic!("expected SVG");
    };
    assert_residual(safe.admission(), TargetAdmissionReason::FallbackCssResidual);
    let RenderOutput::Svg(Some(parity)) =
        render(SvgPipeline::parity().with_postprocessor(ReplaceScene(FALLBACK_CSS)))
    else {
        panic!("expected SVG");
    };
    assert!(
        parity
            .admission()
            .reasons()
            .contains(&TargetAdmissionReason::SvgTerminalUnverified)
    );
    assert!(
        !parity
            .admission()
            .reasons()
            .contains(&TargetAdmissionReason::FallbackCssResidual)
    );
    let error = Renderer::new()
        .render(RenderRequest::svg(
            "info",
            OperationControl::new(),
            SvgRequest {
                environment: SvgEnvironment::deterministic().with_theme_portability_requirement(
                    ThemePortabilityRequirement::RequirePortable,
                ),
                pipeline: Some(
                    SvgPipeline::resvg_safe().with_postprocessor(ReplaceScene(FALLBACK_CSS)),
                ),
                ..Default::default()
            },
        ))
        .expect_err("explicit strict SVG portability rejects the CSS quality residual");
    assert_strict_reason(error, TargetAdmissionReason::FallbackCssResidual);
}
