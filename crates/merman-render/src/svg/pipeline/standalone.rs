use std::sync::Arc;

use merman_core::OperationPhase;

use crate::Result;
use crate::diagram_theme::ThemePortabilityRequirement;
use crate::environment::RenderSession;
use crate::text::{PreparedTextEvidenceLease, PreparedTextTerminalReceipt};

use super::{
    ResvgCompatibleSvg, SvgFinalizationReport, SvgPipeline, SvgPipelinePreset,
    SvgPostprocessExecution, SvgResourceFingerprint, final_validation, resource_closure,
};

/// Terminal compatibility state for an exact standalone SVG artifact.
///
/// This state describes the sealed native projection used for compatibility admission. It never
/// describes a separately normalized SVG draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandaloneSvgTerminalStatus {
    /// Native compatibility was not requested for this artifact.
    Unverified,
    /// The exact artifact passed structural and resource-closure validation.
    Compatible,
    /// Structural or resource-closure validation of the exact artifact failed.
    ValidationFailed,
}

impl StandaloneSvgTerminalStatus {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unverified => "unverified",
            Self::Compatible => "compatible",
            Self::ValidationFailed => "validation-failed",
        }
    }
}

/// Renderer-owned terminal artifact for the standalone SVG target.
///
/// The public SVG, optional prepared-text native projection, resource fingerprint, and terminal
/// evidence are sealed together. Ordinary output retains resource accounting without claiming
/// native compatibility. Strict non-resvg pipelines are observed in place, without normalizing
/// either projection into different bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandaloneSvgArtifact {
    inner: StandaloneSvgArtifactKind,
    #[cfg(merman_internal_theme_acceptance)]
    artifact_receipt: Option<crate::svg_artifact_receipts::SvgArtifactReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StandaloneSvgArtifactKind {
    ResvgCompatible(ResvgCompatibleSvg),
    Standalone(StandaloneSvg),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StandaloneSvg {
    svg: String,
    prepared_text_svg: Option<Arc<str>>,
    prepared_text_evidence: PreparedTextEvidenceLease,
    prepared_text_terminal_receipt: Option<PreparedTextTerminalReceipt>,
    prepared_text_evidence_valid: bool,
    selected_pipeline: SvgPipelinePreset,
    terminal_status: StandaloneSvgTerminalStatus,
    finalization_report: Option<SvgFinalizationReport>,
    resource_fingerprint: SvgResourceFingerprint,
}

impl StandaloneSvgArtifact {
    pub(crate) fn from_resvg_compatible(svg: ResvgCompatibleSvg) -> Self {
        #[cfg(merman_internal_theme_acceptance)]
        let artifact_receipt =
            crate::svg_artifact_receipts::SvgArtifactReceipt::observe_finalized_svg(svg.as_str());
        Self {
            inner: StandaloneSvgArtifactKind::ResvgCompatible(svg),
            #[cfg(merman_internal_theme_acceptance)]
            artifact_receipt,
        }
    }

    pub(crate) fn finalize_exact(
        svg: String,
        prepared_text_svg: Option<String>,
        prepared_text_evidence: PreparedTextEvidenceLease,
        prepared_text_evidence_valid: bool,
        pipeline: &SvgPipeline,
        session: &RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Postprocess)?;
        let prepared_text_svg = prepared_text_svg.map(Arc::from);
        let prepared_text_terminal_receipt =
            if prepared_text_evidence_valid && !prepared_text_evidence.is_empty() {
                prepared_text_svg.as_deref().and_then(|tokenized_svg| {
                    PreparedTextTerminalReceipt::from_ledger(
                        tokenized_svg,
                        prepared_text_evidence.entries(),
                    )
                })
            } else {
                None
            };
        let prepared_text_evidence_valid = prepared_text_evidence_valid
            && (prepared_text_evidence.is_empty() || prepared_text_terminal_receipt.is_some());
        let native_svg = prepared_text_svg.as_deref().unwrap_or(svg.as_str());
        let resource_fingerprint = resource_closure::fingerprint_svg_resources(
            native_svg,
            session.font_catalog().fingerprint().as_bytes(),
            session.font_source_policy(),
        );
        let (terminal_status, finalization_report) = if session.portability_requirement()
            == ThemePortabilityRequirement::BestEffort
            && pipeline.preset() != SvgPipelinePreset::ResvgSafe
        {
            final_validation::check_svg_resource_budget_with_execution(
                native_svg,
                SvgPostprocessExecution::new(session),
            )?;
            (StandaloneSvgTerminalStatus::Unverified, None)
        } else {
            let validation = final_validation::validate_resvg_compatible_svg_with_checkpoint(
                native_svg,
                session.resource_policy(),
                &mut || session.checkpoint(OperationPhase::Postprocess),
            );
            // Native compatibility failures remain observational for non-resvg pipelines.
            // Caller resource limits and cancellation must still fail the operation.
            session.checkpoint(OperationPhase::Postprocess)?;
            match validation {
                Ok(terminal) => (
                    StandaloneSvgTerminalStatus::Compatible,
                    Some(SvgFinalizationReport::from_pipeline(pipeline, &terminal)),
                ),
                Err(crate::Error::SvgPostprocess { .. })
                    if pipeline.preset() != SvgPipelinePreset::ResvgSafe =>
                {
                    (StandaloneSvgTerminalStatus::ValidationFailed, None)
                }
                Err(crate::Error::ResourceLimitExceeded(limit))
                    if pipeline.preset() != SvgPipelinePreset::ResvgSafe
                        && limit.is_svg_backend_compatibility_ceiling() =>
                {
                    (StandaloneSvgTerminalStatus::ValidationFailed, None)
                }
                Err(error) => return Err(error),
            }
        };
        #[cfg(merman_internal_theme_acceptance)]
        let artifact_receipt =
            crate::svg_artifact_receipts::SvgArtifactReceipt::observe_finalized_svg(&svg);
        Ok(Self {
            inner: StandaloneSvgArtifactKind::Standalone(StandaloneSvg {
                svg,
                prepared_text_svg,
                prepared_text_evidence,
                prepared_text_terminal_receipt,
                prepared_text_evidence_valid,
                selected_pipeline: pipeline.preset(),
                terminal_status,
                finalization_report,
                resource_fingerprint,
            }),
            #[cfg(merman_internal_theme_acceptance)]
            artifact_receipt,
        })
    }

    pub fn as_str(&self) -> &str {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.as_str(),
            StandaloneSvgArtifactKind::Standalone(svg) => &svg.svg,
        }
    }

    /// Returns the prepared-text identity projection used by native consumers.
    ///
    /// This may differ from [`Self::as_str`] only in renderer-owned prepared-label identifiers.
    pub fn native_export_svg(&self) -> &str {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.native_export_svg(),
            StandaloneSvgArtifactKind::Standalone(svg) => {
                svg.prepared_text_svg.as_deref().unwrap_or(&svg.svg)
            }
        }
    }

    pub const fn selected_pipeline(&self) -> SvgPipelinePreset {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.finalization_report.preset,
            StandaloneSvgArtifactKind::Standalone(svg) => svg.selected_pipeline,
        }
    }

    pub const fn terminal_status(&self) -> StandaloneSvgTerminalStatus {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(_) => {
                StandaloneSvgTerminalStatus::Compatible
            }
            StandaloneSvgArtifactKind::Standalone(svg) => svg.terminal_status,
        }
    }

    pub const fn finalization_report(&self) -> Option<&SvgFinalizationReport> {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => Some(&svg.finalization_report),
            StandaloneSvgArtifactKind::Standalone(svg) => svg.finalization_report.as_ref(),
        }
    }

    pub const fn resource_fingerprint(&self) -> SvgResourceFingerprint {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.resource_fingerprint,
            StandaloneSvgArtifactKind::Standalone(svg) => svg.resource_fingerprint,
        }
    }

    pub const fn prepared_text_evidence_valid(&self) -> bool {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.prepared_text_evidence_valid,
            StandaloneSvgArtifactKind::Standalone(svg) => svg.prepared_text_evidence_valid,
        }
    }

    pub fn text_fonts_are_self_contained(&self) -> bool {
        self.finalization_report()
            .is_some_and(|report| report.font_seal().is_complete())
    }

    pub fn as_resvg_compatible(&self) -> Option<&ResvgCompatibleSvg> {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => Some(svg),
            StandaloneSvgArtifactKind::Standalone(_) => None,
        }
    }

    /// Returns the renderer-owned observation of this exact finalized SVG when the private theme
    /// acceptance feature is enabled.
    #[cfg(merman_internal_theme_acceptance)]
    pub fn svg_artifact_receipt(
        &self,
    ) -> Option<&crate::svg_artifact_receipts::SvgArtifactReceipt> {
        self.artifact_receipt.as_ref()
    }

    pub fn into_string(self) -> String {
        match self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.into_string(),
            StandaloneSvgArtifactKind::Standalone(svg) => svg.svg,
        }
    }
}

impl From<ResvgCompatibleSvg> for StandaloneSvgArtifact {
    fn from(svg: ResvgCompatibleSvg) -> Self {
        Self::from_resvg_compatible(svg)
    }
}

impl AsRef<str> for StandaloneSvgArtifact {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};

    fn session_with_max_svg_elements(max_svg_elements: usize) -> RenderSession {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgElements, max_svg_elements)
            .unwrap();
        crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .unwrap()
    }

    fn assert_max_svg_elements(error: crate::Error) {
        let crate::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected max_svg_elements resource failure, got {error}");
        };
        assert_eq!(limit.limit, "max_svg_elements");
    }

    #[test]
    fn omitted_pipeline_observation_propagates_svg_element_limit() {
        let session = session_with_max_svg_elements(2);
        // `RenderedFamilySvg::finalize_standalone(None)` reaches this direct observation path and
        // records the implicit default as the parity preset without running a draft pipeline.
        let error = StandaloneSvgArtifact::finalize_exact(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><g/><g/></svg>"#.to_owned(),
            None,
            PreparedTextEvidenceLease::default(),
            true,
            &SvgPipeline::parity(),
            &session,
        )
        .unwrap_err();

        assert_max_svg_elements(error);
    }

    #[test]
    fn readable_pipeline_observation_propagates_expanded_svg_element_limit() {
        let session = session_with_max_svg_elements(6);
        let pipeline = SvgPipeline::readable();
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><g id="leaf"><path/></g></defs><use href="#leaf"/><use href="#leaf"/></svg>"##;
        let processed = pipeline
            .process_owned_to_string(svg.to_owned(), &session)
            .expect("the explicit non-resvg pipeline should stay within the raw element limit");
        let error = StandaloneSvgArtifact::finalize_exact(
            processed,
            None,
            PreparedTextEvidenceLease::default(),
            true,
            &pipeline,
            &session,
        )
        .unwrap_err();

        assert_max_svg_elements(error);
    }

    #[test]
    fn foreign_object_does_not_mask_raw_svg_element_budget_failure() {
        let session = session_with_max_svg_elements(2);
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject/><g/></svg>"#;

        let error = StandaloneSvgArtifact::finalize_exact(
            svg.to_owned(),
            None,
            PreparedTextEvidenceLease::default(),
            true,
            &SvgPipeline::parity(),
            &session,
        )
        .expect_err("raw SVG element budgets must run before terminal compatibility checks");

        assert_max_svg_elements(error);
    }

    #[test]
    fn parity_observation_rejects_analyzable_use_expansion_over_budget() {
        let session = session_with_max_svg_elements(10);
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><g id="leaf"><path/></g><g id="branch"><use href="#leaf"/><use href="#leaf"/></g></defs><use href="#branch"/><use href="#branch"/></svg>"##;

        let error = StandaloneSvgArtifact::finalize_exact(
            svg.to_owned(),
            None,
            PreparedTextEvidenceLease::default(),
            true,
            &SvgPipeline::parity(),
            &session,
        )
        .expect_err("an analyzable use DAG must retain its expanded resource budget");

        assert_max_svg_elements(error);
    }

    #[test]
    fn parity_observation_enforces_expansion_budget_above_native_backend_ceiling() {
        let mut svg = String::from(r#"<svg><defs><g id="leaf"><path/></g>"#);
        for index in 0..20 {
            let target = if index == 0 {
                "leaf".to_owned()
            } else {
                format!("branch-{}", index - 1)
            };
            svg.push_str(&format!(
                r##"<g id="branch-{index}"><use href="#{target}"/><use href="#{target}"/></g>"##
            ));
        }
        svg.push_str(r##"</defs><use href="#branch-19"/></svg>"##);
        let session = session_with_max_svg_elements(crate::resources::MAX_RESVG_TREE_NODES * 2);
        let error = StandaloneSvgArtifact::finalize_exact(
            svg,
            None,
            PreparedTextEvidenceLease::default(),
            true,
            &SvgPipeline::parity(),
            &session,
        )
        .expect_err("a native ceiling must not hide a larger explicit resource budget");
        assert_max_svg_elements(error);
    }

    #[test]
    fn foreign_object_finalization_retains_svg_without_native_assurance() {
        let dag = r##"<defs><g id="leaf"><path/></g><g id="branch"><use href="#leaf"/><use href="#leaf"/></g></defs><use href="#branch"/><use href="#branch"/>"##;
        for svg in [
            format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject/>{dag}</svg>"#),
            format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{dag}<foreignObject/></svg>"#),
        ] {
            let session = session_with_max_svg_elements(64);
            let artifact = StandaloneSvgArtifact::finalize_exact(
                svg.clone(),
                None,
                PreparedTextEvidenceLease::default(),
                true,
                &SvgPipeline::parity(),
                &session,
            )
            .expect("ordinary observation retains native-incompatible SVG bytes");

            assert_eq!(
                artifact.terminal_status(),
                StandaloneSvgTerminalStatus::Unverified
            );
            assert!(artifact.finalization_report().is_none());
            assert_eq!(artifact.as_str(), svg);
        }
    }

    #[test]
    fn ordinary_parity_does_not_apply_native_backend_depth_ceiling() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .begin_session()
            .unwrap();
        let depth = crate::resources::MAX_RESVG_TREE_DEPTH + 1;
        let mut svg =
            String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">"#);
        svg.push_str(&"<g>".repeat(depth));
        svg.push_str(&"</g>".repeat(depth));
        svg.push_str("</svg>");

        let artifact = StandaloneSvgArtifact::finalize_exact(
            svg,
            None,
            PreparedTextEvidenceLease::default(),
            true,
            &SvgPipeline::parity(),
            &session,
        )
        .expect("backend incompatibility is evidence, not a standalone SVG resource failure");

        assert_eq!(
            artifact.terminal_status(),
            StandaloneSvgTerminalStatus::Unverified
        );
        assert!(artifact.finalization_report().is_none());
        #[cfg(merman_internal_theme_acceptance)]
        assert!(artifact.svg_artifact_receipt().is_none());
    }

    #[test]
    fn resvg_safe_observation_cannot_downgrade_terminal_validation_failure() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .begin_session()
            .unwrap();
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject/></svg>"#;

        let error = StandaloneSvgArtifact::finalize_exact(
            svg.to_owned(),
            None,
            PreparedTextEvidenceLease::default(),
            true,
            &SvgPipeline::resvg_safe(),
            &session,
        )
        .expect_err("a resvg-safe artifact must fail closed on terminal validation errors");

        assert!(matches!(error, crate::Error::SvgPostprocess { .. }));
    }

    #[test]
    fn native_prepared_text_projection_controls_resvg_admission() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .begin_session()
            .unwrap();
        let public_svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0h1v1z"/></svg>"#;
        let native_svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject/></svg>"#;

        let error = StandaloneSvgArtifact::finalize_exact(
            public_svg.to_owned(),
            Some(native_svg.to_owned()),
            PreparedTextEvidenceLease::default(),
            false,
            &SvgPipeline::resvg_safe(),
            &session,
        )
        .expect_err("resvg admission must validate the native prepared-text projection");

        assert!(matches!(error, crate::Error::SvgPostprocess { .. }));
    }

    #[cfg(merman_internal_theme_acceptance)]
    #[test]
    fn finalized_svg_artifact_owns_the_generic_theme_receipt() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M0 0h1v1z"/></svg>"#;
        let sealed = crate::svg::SvgPipeline::resvg_safe()
            .process_resvg_compatible(svg, &session)
            .unwrap();
        let artifact = StandaloneSvgArtifact::from_resvg_compatible(sealed);
        let receipt = artifact
            .svg_artifact_receipt()
            .expect("the finalized renderer artifact must retain its receipt");

        assert!(receipt.proves_artifact(receipt.artifact_digest()));
        assert_eq!(receipt.view_box(), [0.0, 0.0, 10.0, 10.0]);
    }
}
