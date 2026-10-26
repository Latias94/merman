//! Internal operation ownership for the public rendering facade.

#[cfg(feature = "presentation")]
use crate::render::prepared::PreparedSource;
use crate::render::{RenderError, ResourceLimitCause, ResourceLimitExceeded};
#[cfg(feature = "presentation")]
use merman_core::{MermaidEngine, ParsedDiagramRender, PresentationMetadata};
use merman_core::{
    OperationControl, OperationLedgerError, OperationPhase, OperationResourceDomain,
    OperationResourceLimitExceeded, OperationResourceOverride, OperationResourceProvenance,
    ParseOptions, ParsedSemanticDiagram, SemanticDiagramModel, SemanticMetadata, SemanticParser,
    resources::{InputResourceLimitExceeded, InputResourcePolicy, ModelComplexity},
    runtime::{OperationContext, RuntimePolicy},
};
#[cfg(feature = "svg")]
use merman_render::diagram_theme::DiagramTheme;

#[derive(Debug)]
pub(crate) struct Operation {
    pub(crate) control: OperationControl,
    pub(crate) context: OperationContext,
    resources: InputResourcePolicy,
    #[cfg(feature = "svg")]
    theme: Option<DiagramTheme>,
}

impl Operation {
    pub(crate) fn begin(
        runtime: &RuntimePolicy,
        source: &str,
        control: OperationControl,
        resources: InputResourcePolicy,
        #[cfg(feature = "svg")] theme: Option<DiagramTheme>,
    ) -> Result<Self, RenderError> {
        Self::begin_source_bytes(
            runtime,
            source.len(),
            control,
            resources,
            #[cfg(feature = "svg")]
            theme,
        )
    }

    fn begin_source_bytes(
        runtime: &RuntimePolicy,
        source_bytes: usize,
        control: OperationControl,
        resources: InputResourcePolicy,
        #[cfg(feature = "svg")] theme: Option<DiagramTheme>,
    ) -> Result<Self, RenderError> {
        checkpoint(&control, OperationPhase::Admission)?;
        if let Err(error) = resources.check_source_byte_count(source_bytes) {
            return Err(terminate_input_resource_error(
                &control,
                OperationPhase::Admission,
                error,
            ));
        }
        let context = runtime.begin_operation().map_err(RenderError::from)?;
        checkpoint(&control, OperationPhase::Admission)?;
        Ok(Self {
            control,
            context,
            resources,
            #[cfg(feature = "svg")]
            theme,
        })
    }

    pub(crate) fn parse_semantic(
        self,
        parser: &SemanticParser,
        source: &str,
        options: ParseOptions,
    ) -> Result<Option<SemanticArtifact>, RenderError> {
        checkpoint(&self.control, OperationPhase::Parse)?;
        let parsed = parser
            .parse_diagram_with_options_controlled_in_context_sync(
                source,
                options,
                &self.control,
                &self.context,
            )
            .map_err(RenderError::Cancelled)?
            .map_err(RenderError::from)?;
        let Some(parsed) = parsed else {
            return Ok(None);
        };
        self.admit_model(ModelComplexity::from_semantic_model(&parsed.model))?;
        Ok(Some(SemanticArtifact {
            parsed: Box::new(parsed),
            operation: self.into_execution(),
        }))
    }

    #[cfg(feature = "presentation")]
    pub(crate) fn resume_prepared(
        runtime: &RuntimePolicy,
        prepared: &PreparedSource,
        control: OperationControl,
        resources: InputResourcePolicy,
    ) -> Result<PresentationArtifact, RenderError> {
        let operation = Self::begin_source_bytes(
            runtime,
            prepared.source_bytes(),
            control,
            resources,
            #[cfg(feature = "svg")]
            prepared.theme().cloned(),
        )?;
        operation.admit_model(prepared.model_complexity())?;
        Ok(PresentationArtifact::from_prepared(
            prepared.clone(),
            operation.into_execution(),
        ))
    }

    #[cfg(feature = "presentation")]
    pub(crate) fn parse_presentation(
        self,
        engine: &MermaidEngine,
        source: &str,
        parse_options: ParseOptions,
    ) -> Result<Option<PresentationArtifact>, RenderError> {
        checkpoint(&self.control, OperationPhase::Parse)?;
        let parsed = engine
            .parse_diagram_for_render_model_controlled_in_context_sync(
                source,
                parse_options,
                &self.control,
                &self.context,
            )
            .map_err(RenderError::Cancelled)?
            .map_err(RenderError::from)?;
        let Some(parsed) = parsed else {
            return Ok(None);
        };
        let complexity = ModelComplexity::from_parsed_render(&parsed);
        self.admit_model(complexity)?;
        let prepared = PreparedSource::new(
            parsed,
            self.context.clone(),
            parse_options,
            source.len(),
            complexity,
            #[cfg(feature = "svg")]
            self.theme.clone(),
        );
        Ok(Some(PresentationArtifact::from_prepared(
            prepared,
            self.into_execution(),
        )))
    }

    fn admit_model(&self, complexity: ModelComplexity) -> Result<(), RenderError> {
        checkpoint(&self.control, OperationPhase::Semantic)?;
        if let Err(error) = self.resources.check_model_complexity(complexity) {
            checkpoint(&self.control, OperationPhase::Semantic)?;
            return Err(terminate_input_resource_error(
                &self.control,
                OperationPhase::Semantic,
                error,
            ));
        }
        checkpoint(&self.control, OperationPhase::Semantic)
    }

    fn into_execution(self) -> OperationExecution {
        OperationExecution {
            control: self.control,
            #[cfg(any(feature = "svg", feature = "ascii"))]
            context: self.context,
            #[cfg(feature = "svg")]
            theme: self.theme,
        }
    }
}

pub(crate) fn checkpoint(
    control: &OperationControl,
    phase: OperationPhase,
) -> Result<(), RenderError> {
    control
        .terminal_checkpoint_at(phase)
        .map_err(operation_terminal_error)
}

fn terminate_input_resource_error(
    control: &OperationControl,
    phase: OperationPhase,
    error: InputResourceLimitExceeded,
) -> RenderError {
    let projected = ResourceLimitExceeded::from_input(error.clone());
    let operation_error = OperationResourceLimitExceeded {
        id: error.limit,
        phase,
        resource_phase: projected.phase,
        limit: saturating_u64(error.max),
        consumed: 0,
        requested: saturating_u64(error.actual),
        provenance: OperationResourceProvenance::new(
            OperationResourceDomain::Input,
            Some(error.profile),
            error
                .explicit_overrides
                .iter()
                .map(|override_| OperationResourceOverride {
                    id: override_.id.as_str(),
                    value: saturating_u64(override_.value),
                }),
        ),
    };
    let terminal = control.terminate_resource_limit(operation_error.clone());
    if terminal == OperationLedgerError::ResourceLimitExceeded(operation_error) {
        return RenderError::ResourceLimitExceeded(projected);
    }
    operation_terminal_error(terminal)
}

pub(crate) fn operation_terminal_error(error: OperationLedgerError) -> RenderError {
    match error {
        OperationLedgerError::Cancelled(error) => RenderError::Cancelled(error),
        OperationLedgerError::ResourceLimitExceeded(error) => {
            RenderError::ResourceLimitExceeded(ResourceLimitExceeded {
                id: error.id,
                phase: error.resource_phase,
                actual: error.consumed.saturating_add(error.requested),
                maximum: error.limit,
                cause: ResourceLimitCause::Ceiling,
                provenance: Some(error.provenance),
            })
        }
        OperationLedgerError::ArithmeticOverflow {
            id,
            resource_phase,
            actual,
            maximum,
            provenance,
            ..
        } => RenderError::ResourceLimitExceeded(ResourceLimitExceeded {
            id,
            phase: resource_phase,
            actual,
            maximum,
            cause: ResourceLimitCause::ArithmeticOverflow,
            provenance: Some(provenance),
        }),
    }
}

fn saturating_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// Operation state that remains relevant after parsing has completed.
#[derive(Debug)]
pub(crate) struct OperationExecution {
    pub(crate) control: OperationControl,
    #[cfg(any(feature = "svg", feature = "ascii"))]
    pub(crate) context: OperationContext,
    #[cfg(feature = "svg")]
    pub(crate) theme: Option<DiagramTheme>,
}

/// Canonical diagram meaning paired with the operation that admitted it.
///
/// This artifact has no presentation context, compatibility JSON, or graphical promotion API.
#[derive(Debug)]
pub struct SemanticArtifact {
    parsed: Box<ParsedSemanticDiagram>,
    operation: OperationExecution,
}

impl SemanticArtifact {
    pub fn metadata(&self) -> &SemanticMetadata {
        &self.parsed.metadata
    }

    pub fn model(&self) -> &SemanticDiagramModel {
        &self.parsed.model
    }

    pub fn diagram_type(&self) -> &str {
        &self.metadata().diagram_type
    }
    pub fn semantic_kind(&self) -> &'static str {
        merman_core::diagram_type_render_model_kind(self.diagram_type()).unwrap_or("unknown")
    }
    pub fn family_id(&self) -> Option<merman_core::DiagramFamilyId> {
        merman_core::diagram_type_family_id(self.diagram_type())
    }
    pub(crate) fn control(&self) -> &OperationControl {
        &self.operation.control
    }
    #[cfg(feature = "ascii")]
    pub(crate) fn into_parts(self) -> (ParsedSemanticDiagram, OperationExecution) {
        (*self.parsed, self.operation)
    }
}

/// A full Mermaid presentation artifact paired with its original operation.
///
/// This artifact retains the admitted theme/configuration and can be handed off to a reusable
/// full source. It cannot be created by promoting an already parsed semantic artifact.
#[derive(Debug)]
#[cfg(feature = "presentation")]
pub struct PresentationArtifact {
    state: Box<PresentationArtifactState>,
}

#[derive(Debug)]
#[cfg(feature = "presentation")]
struct PresentationArtifactState {
    prepared: PreparedSource,
    operation: OperationExecution,
}

#[cfg(feature = "presentation")]
impl PresentationArtifact {
    /// Returns metadata captured by the controlled parse operation.
    pub fn metadata(&self) -> &PresentationMetadata {
        self.state.prepared.metadata()
    }

    /// Returns the stable family/model kind selected by the semantic parser.
    pub fn semantic_kind(&self) -> &'static str {
        self.state.prepared.semantic_kind()
    }

    /// Returns the typed Mermaid diagram id selected during preprocessing.
    pub fn diagram_type(&self) -> &str {
        &self.metadata().diagram_type
    }

    /// Returns the catalog-owned family selected after detection defaults and configuration
    /// effects were applied for this canonical operation.
    pub fn family_id(&self) -> Option<merman_core::DiagramFamilyId> {
        self.state.prepared.family_id()
    }

    pub(crate) fn parsed(&self) -> &ParsedDiagramRender {
        self.state.prepared.parsed()
    }

    pub(crate) fn control(&self) -> &OperationControl {
        &self.state.operation.control
    }

    /// Retains this successful parse without retaining its control, ledger, or output state.
    pub fn into_prepared_source(self) -> PreparedSource {
        self.state.prepared
    }

    pub(crate) fn from_prepared(prepared: PreparedSource, operation: OperationExecution) -> Self {
        Self {
            state: Box::new(PresentationArtifactState {
                prepared,
                operation,
            }),
        }
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    pub(crate) fn into_parts(self) -> (PreparedSource, OperationExecution) {
        let PresentationArtifactState {
            prepared,
            operation,
        } = *self.state;
        (prepared, operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_provenance(domain: OperationResourceDomain) -> OperationResourceProvenance {
        OperationResourceProvenance::new(
            domain,
            Some(merman_core::resources::ResourceProfile::Interactive),
            [OperationResourceOverride {
                id: "max_svg_bytes",
                value: 17,
            }],
        )
    }

    #[test]
    fn facade_terminal_mapper_replays_original_target_resource_metadata() {
        let ceiling_control = OperationControl::new();
        ceiling_control.terminate_resource_limit(OperationResourceLimitExceeded {
            id: "max_svg_bytes",
            phase: OperationPhase::Postprocess,
            resource_phase: "svg_postprocess",
            limit: 17,
            consumed: 10,
            requested: 8,
            provenance: test_provenance(OperationResourceDomain::Render),
        });
        assert_resource_error(
            checkpoint(&ceiling_control, OperationPhase::Postprocess)
                .expect_err("the SVG ceiling must replay"),
            "max_svg_bytes",
            "svg_postprocess",
            18,
            17,
            ResourceLimitCause::Ceiling,
        );
        ceiling_control.cancel();
        assert_resource_error(
            checkpoint(&ceiling_control, OperationPhase::Emit)
                .expect_err("later cancellation must not replace the SVG ceiling"),
            "max_svg_bytes",
            "svg_postprocess",
            18,
            17,
            ResourceLimitCause::Ceiling,
        );

        let overflow_control = OperationControl::new();
        overflow_control.terminate_resource_overflow(
            "max_ascii_output_bytes",
            OperationPhase::Emit,
            "ascii_output",
            u64::from(u32::MAX),
            123,
            test_provenance(OperationResourceDomain::Ascii),
        );
        assert_resource_error(
            checkpoint(&overflow_control, OperationPhase::Layout)
                .expect_err("the ASCII overflow must replay"),
            "max_ascii_output_bytes",
            "ascii_output",
            u64::from(u32::MAX),
            123,
            ResourceLimitCause::ArithmeticOverflow,
        );
    }

    fn assert_resource_error(
        error: RenderError,
        id: &'static str,
        phase: &'static str,
        actual: u64,
        maximum: u64,
        cause: ResourceLimitCause,
    ) {
        assert!(matches!(
            error,
            RenderError::ResourceLimitExceeded(details)
                if details.id == id
                    && details.phase == phase
                    && details.actual == actual
                    && details.maximum == maximum
                    && details.cause == cause
        ));
    }
}
