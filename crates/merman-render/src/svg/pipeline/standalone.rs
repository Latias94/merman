use std::sync::Arc;

use merman_core::OperationPhase;

use crate::Result;
use crate::environment::RenderSession;
use crate::text::{PreparedTextEvidenceLease, PreparedTextTerminalReceipt};

use super::{
    ResvgCompatibleSvg, SvgFinalizationReport, SvgPipeline, SvgPipelinePreset,
    SvgResourceFingerprint, final_validation, resource_closure,
};

/// Terminal compatibility state for an exact standalone SVG artifact.
///
/// This state describes the bytes returned to the caller. It never describes a separately
/// normalized SVG draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandaloneSvgTerminalStatus {
    /// The exact artifact passed structural and resource-closure validation.
    Compatible,
    /// Structural or resource-closure validation of the exact artifact failed.
    ValidationFailed,
}

impl StandaloneSvgTerminalStatus {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compatible => "compatible",
            Self::ValidationFailed => "validation-failed",
        }
    }
}

/// Renderer-owned terminal artifact for the standalone SVG target.
///
/// The public SVG, optional prepared-text native projection, resource fingerprint, and terminal
/// evidence are sealed together. Non-resvg pipelines are observed in place: validation failure is
/// retained as terminal evidence and does not cause a completed best-effort artifact to be
/// rewritten or discarded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandaloneSvgArtifact {
    inner: StandaloneSvgArtifactKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StandaloneSvgArtifactKind {
    ResvgCompatible(ResvgCompatibleSvg),
    Observed(ObservedStandaloneSvg),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedStandaloneSvg {
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
        Self {
            inner: StandaloneSvgArtifactKind::ResvgCompatible(svg),
        }
    }

    pub(crate) fn observe_exact(
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
        let validation =
            final_validation::validate_resvg_compatible_svg(&svg, session.resource_policy());
        // Compatibility validation is observational. Operation-control and resource failures are
        // not compatibility evidence and must still fail the operation.
        session.checkpoint(OperationPhase::Postprocess)?;
        let (terminal_status, finalization_report) = match validation {
            Ok(terminal) => (
                StandaloneSvgTerminalStatus::Compatible,
                Some(SvgFinalizationReport::from_pipeline(pipeline, &terminal)),
            ),
            Err(crate::Error::SvgPostprocess { .. }) => {
                (StandaloneSvgTerminalStatus::ValidationFailed, None)
            }
            Err(error) => return Err(error),
        };
        Ok(Self {
            inner: StandaloneSvgArtifactKind::Observed(ObservedStandaloneSvg {
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
        })
    }

    pub fn as_str(&self) -> &str {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.as_str(),
            StandaloneSvgArtifactKind::Observed(svg) => &svg.svg,
        }
    }

    /// Returns the prepared-text identity projection used by native consumers.
    ///
    /// This may differ from [`Self::as_str`] only in renderer-owned prepared-label identifiers.
    pub fn native_export_svg(&self) -> &str {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.native_export_svg(),
            StandaloneSvgArtifactKind::Observed(svg) => {
                svg.prepared_text_svg.as_deref().unwrap_or(&svg.svg)
            }
        }
    }

    pub const fn selected_pipeline(&self) -> SvgPipelinePreset {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.finalization_report.preset,
            StandaloneSvgArtifactKind::Observed(svg) => svg.selected_pipeline,
        }
    }

    pub const fn terminal_status(&self) -> StandaloneSvgTerminalStatus {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(_) => {
                StandaloneSvgTerminalStatus::Compatible
            }
            StandaloneSvgArtifactKind::Observed(svg) => svg.terminal_status,
        }
    }

    pub const fn finalization_report(&self) -> Option<&SvgFinalizationReport> {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => Some(&svg.finalization_report),
            StandaloneSvgArtifactKind::Observed(svg) => svg.finalization_report.as_ref(),
        }
    }

    pub const fn resource_fingerprint(&self) -> SvgResourceFingerprint {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.resource_fingerprint,
            StandaloneSvgArtifactKind::Observed(svg) => svg.resource_fingerprint,
        }
    }

    pub const fn prepared_text_evidence_valid(&self) -> bool {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.prepared_text_evidence_valid,
            StandaloneSvgArtifactKind::Observed(svg) => svg.prepared_text_evidence_valid,
        }
    }

    pub fn text_fonts_are_self_contained(&self) -> bool {
        self.finalization_report()
            .is_some_and(|report| report.font_seal().is_complete())
    }

    pub fn as_resvg_compatible(&self) -> Option<&ResvgCompatibleSvg> {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => Some(svg),
            StandaloneSvgArtifactKind::Observed(_) => None,
        }
    }

    pub fn into_string(self) -> String {
        match self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.into_string(),
            StandaloneSvgArtifactKind::Observed(svg) => svg.svg,
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
        let error = StandaloneSvgArtifact::observe_exact(
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
        let error = StandaloneSvgArtifact::observe_exact(
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
}
