use merman_core::OperationPhase;

use crate::Result;
use crate::diagram_theme::ThemePortabilityRequirement;
use crate::environment::RenderSession;

use super::{
    ResvgCompatibleSvg, SvgFinalizationReport, SvgPipeline, SvgPipelinePreset,
    SvgPostprocessExecution, final_validation,
};

/// Native compatibility of the exact selected standalone SVG bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandaloneSvgTerminalStatus {
    /// Native compatibility was not requested.
    Unverified,
    /// The selected bytes passed native structural and resource admission.
    Compatible,
    /// The selected bytes do not satisfy native compatibility.
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

/// Immutable SVG output, with native admission only when it was requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandaloneSvgArtifact {
    inner: StandaloneSvgArtifactKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StandaloneSvgArtifactKind {
    ResvgCompatible(ResvgCompatibleSvg),
    Standalone(StandaloneSvg),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StandaloneSvg {
    svg: String,
    selected_pipeline: SvgPipelinePreset,
    terminal_status: StandaloneSvgTerminalStatus,
    finalization_report: Option<SvgFinalizationReport>,
}

impl StandaloneSvgArtifact {
    pub(crate) fn from_resvg_compatible(svg: ResvgCompatibleSvg) -> Self {
        Self {
            inner: StandaloneSvgArtifactKind::ResvgCompatible(svg),
        }
    }

    pub(crate) fn finalize_exact(
        svg: String,
        pipeline: &SvgPipeline,
        session: &RenderSession,
    ) -> Result<Self> {
        let execution = SvgPostprocessExecution::new(session);
        execution.checkpoint()?;
        execution.preflight_svg_byte_count(svg.len())?;
        let (terminal_status, finalization_report) = if session.portability_requirement()
            == ThemePortabilityRequirement::BestEffort
            && pipeline.preset() != SvgPipelinePreset::ResvgSafe
        {
            // The producer owns serialization. Ordinary SVG generation does not request
            // native admission or a second interpretation of its completed document.
            (StandaloneSvgTerminalStatus::Unverified, None)
        } else {
            let validation = if pipeline.preset() == SvgPipelinePreset::ResvgSafe {
                final_validation::validate_resvg_compatible_svg_with_execution(&svg, execution)
            } else {
                final_validation::observe_resvg_compatibility_with_execution(&svg, execution)
            };
            execution.checkpoint()?;
            match validation {
                Ok(terminal) => (
                    StandaloneSvgTerminalStatus::Compatible,
                    Some(SvgFinalizationReport::from_pipeline(pipeline, terminal)),
                ),
                Err(crate::Error::SvgPostprocess { .. })
                    if pipeline.preset() != SvgPipelinePreset::ResvgSafe =>
                {
                    final_validation::check_svg_resource_budget_with_execution(&svg, execution)?;
                    (StandaloneSvgTerminalStatus::ValidationFailed, None)
                }
                Err(crate::Error::ResourceLimitExceeded(limit))
                    if pipeline.preset() != SvgPipelinePreset::ResvgSafe
                        && limit.is_svg_backend_compatibility_ceiling() =>
                {
                    final_validation::check_svg_resource_budget_with_execution(&svg, execution)?;
                    (StandaloneSvgTerminalStatus::ValidationFailed, None)
                }
                Err(crate::Error::ResourceLimitExceeded(limit)) => {
                    return Err(execution.terminate_resource_error(limit));
                }
                Err(error) => return Err(error),
            }
        };
        session.checkpoint(OperationPhase::Postprocess)?;
        Ok(Self {
            inner: StandaloneSvgArtifactKind::Standalone(StandaloneSvg {
                svg,
                selected_pipeline: pipeline.preset(),
                terminal_status,
                finalization_report,
            }),
        })
    }

    pub fn as_str(&self) -> &str {
        match &self.inner {
            StandaloneSvgArtifactKind::ResvgCompatible(svg) => svg.as_str(),
            StandaloneSvgArtifactKind::Standalone(svg) => &svg.svg,
        }
    }

    /// Returns the exact selected SVG bytes.
    pub fn native_export_svg(&self) -> &str {
        self.as_str()
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

    #[test]
    #[cfg(all(feature = "diagram-class", feature = "diagram-journey"))]
    fn ordinary_generated_svg_does_not_run_terminal_document_observers() {
        for source in [
            "classDiagram\nA --> B",
            "journey\n  title Release\n  section Delivery\n    Ship: 5: Maintainer",
        ] {
            let parsed = merman_core::MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
                .unwrap()
                .unwrap();
            let session = crate::environment::RenderEnvironment::deterministic()
                .begin_session()
                .unwrap();
            let artifact = crate::family::prepare(
                &parsed,
                &crate::LayoutOptions::headless_svg_defaults(),
                session,
            )
            .unwrap();
            let ((svg, terminal_work), xml_work) =
                final_validation::measure_xml_validation_work(|| {
                    final_validation::measure_terminal_validation_work(|| {
                        artifact
                            .render_svg(
                                &crate::svg::SvgRenderOptions::default(),
                                &crate::svg::SvgDebugOptions::default(),
                            )
                            .unwrap()
                            .finalize_standalone(Some(&SvgPipeline::parity()))
                            .unwrap()
                            .into_string()
                    })
                });
            assert!(!svg.is_empty());
            assert_eq!(xml_work.calls, 0);
            assert_eq!(terminal_work.calls, 0);
            crate::xml_dom::Document::parse(&svg).expect("producer emits valid SVG XML");
        }
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
    fn parity_backend_depth_ceiling_remains_observational() {
        let depth = crate::resources::MAX_RESVG_TREE_DEPTH + 1;
        let mut svg =
            String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">"#);
        svg.push_str(&"<g>".repeat(depth));
        svg.push_str(&"</g>".repeat(depth));
        svg.push_str("</svg>");

        for (portability, expected_status) in [
            (
                ThemePortabilityRequirement::BestEffort,
                StandaloneSvgTerminalStatus::Unverified,
            ),
            (
                ThemePortabilityRequirement::RequirePortable,
                StandaloneSvgTerminalStatus::ValidationFailed,
            ),
        ] {
            let control = merman_core::OperationControl::new();
            let session = crate::environment::RenderEnvironment::deterministic()
                .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
                .with_theme_portability_requirement(portability)
                .begin_session_with_control(control.clone())
                .unwrap();
            let artifact = StandaloneSvgArtifact::finalize_exact(
                svg.clone(),
                &SvgPipeline::parity(),
                &session,
            )
            .expect("backend incompatibility is evidence, not a standalone SVG resource failure");

            assert_eq!(artifact.terminal_status(), expected_status);
            assert!(artifact.finalization_report().is_none());
            assert_eq!(
                control.terminal_checkpoint_at(OperationPhase::Export),
                Ok(())
            );
        }
    }
}
