//! Internal operation ownership for the public rendering facade.
//!
//! This module owns the one source-to-semantic execution boundary. Target adapters receive the
//! completed operation projection and must not create a replacement runtime context or control.

use merman_core::{
    Engine, OperationControl, OperationLedgerError, OperationPhase, OperationResourceLimitExceeded,
    ParseMetadata, ParseOptions, ParsedDiagramRender,
    resources::{InputResourceLimitExceeded, InputResourcePolicy},
    runtime::OperationContext,
};

use crate::render::{RenderError, ResourceLimitCause, ResourceLimitExceeded};

/// Immutable operation-owned values shared by every target adapter.
#[derive(Debug)]
pub(crate) struct Operation {
    engine: Engine,
    pub(crate) control: OperationControl,
    pub(crate) context: OperationContext,
    resources: InputResourcePolicy,
}

impl Operation {
    pub(crate) fn begin(
        engine: &Engine,
        source: &str,
        control: OperationControl,
        resources: InputResourcePolicy,
    ) -> Result<Self, RenderError> {
        checkpoint(&control, OperationPhase::Admission)?;
        if let Err(error) = resources.check_source_bytes(source) {
            return Err(terminate_input_resource_error(
                &control,
                OperationPhase::Admission,
                error,
            ));
        }
        let context = engine.begin_operation().map_err(RenderError::from)?;
        checkpoint(&control, OperationPhase::Admission)?;
        Ok(Self {
            engine: engine.clone(),
            control,
            context,
            resources,
        })
    }

    pub(crate) fn parse_render_model(
        self,
        source: &str,
        parse_options: ParseOptions,
    ) -> Result<Option<SemanticArtifact>, RenderError> {
        checkpoint(&self.control, OperationPhase::Parse)?;
        let parsed = self
            .engine
            .parse_diagram_for_render_model_controlled_in_context_sync(
                source,
                parse_options,
                &self.control,
                &self.context,
            );
        let parsed = match parsed {
            Err(cancelled) => return Err(RenderError::Cancelled(cancelled)),
            Ok(result) => result.map_err(RenderError::from)?,
        };
        let Some(parsed) = parsed else {
            return Ok(None);
        };
        checkpoint(&self.control, OperationPhase::Semantic)?;
        if let Err(error) = self.resources.check_parsed_render(&parsed) {
            checkpoint(&self.control, OperationPhase::Semantic)?;
            return Err(terminate_input_resource_error(
                &self.control,
                OperationPhase::Semantic,
                error,
            ));
        }
        checkpoint(&self.control, OperationPhase::Semantic)?;
        Ok(Some(SemanticArtifact {
            state: Box::new(SemanticArtifactState {
                parsed,
                operation: OperationExecution {
                    control: self.control,
                    #[cfg(any(feature = "svg", feature = "ascii"))]
                    context: self.context,
                },
            }),
        }))
    }
}

fn checkpoint(control: &OperationControl, phase: OperationPhase) -> Result<(), RenderError> {
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
    };
    let terminal = control.terminate_resource_limit(operation_error);
    if terminal == OperationLedgerError::ResourceLimitExceeded(operation_error) {
        return RenderError::ResourceLimitExceeded(projected);
    }
    operation_terminal_error(terminal)
}

fn operation_terminal_error(error: OperationLedgerError) -> RenderError {
    match error {
        OperationLedgerError::Cancelled(error) => RenderError::Cancelled(error),
        OperationLedgerError::ResourceLimitExceeded(error) => {
            RenderError::ResourceLimitExceeded(ResourceLimitExceeded {
                id: error.id,
                phase: error.resource_phase,
                actual: error.consumed.saturating_add(error.requested),
                maximum: error.limit,
                cause: ResourceLimitCause::Ceiling,
            })
        }
        OperationLedgerError::ArithmeticOverflow {
            id,
            resource_phase,
            actual,
            maximum,
            ..
        } => RenderError::ResourceLimitExceeded(ResourceLimitExceeded {
            id,
            phase: resource_phase,
            actual,
            maximum,
            cause: ResourceLimitCause::ArithmeticOverflow,
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
}

/// A format-neutral semantic artifact paired with the operation that produced it.
///
/// The artifact is intentionally not constructible from an arbitrary metadata/model pair. SVG
/// and ASCII adapters consume this canonical pair and choose their own layout and emission path.
#[derive(Debug)]
pub struct SemanticArtifact {
    state: Box<SemanticArtifactState>,
}

#[derive(Debug)]
struct SemanticArtifactState {
    parsed: ParsedDiagramRender,
    operation: OperationExecution,
}

impl SemanticArtifact {
    /// Returns metadata captured by the controlled parse operation.
    pub fn metadata(&self) -> &ParseMetadata {
        self.state.parsed.metadata()
    }

    /// Returns the stable family/model kind selected by the semantic parser.
    pub fn semantic_kind(&self) -> &'static str {
        self.state.parsed.model().kind()
    }

    /// Returns the typed Mermaid diagram id selected during preprocessing.
    pub fn diagram_type(&self) -> &str {
        &self.metadata().diagram_type
    }

    pub(crate) fn parsed(&self) -> &ParsedDiagramRender {
        &self.state.parsed
    }

    pub(crate) fn control(&self) -> &OperationControl {
        &self.state.operation.control
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    pub(crate) fn into_parts(self) -> (ParsedDiagramRender, OperationExecution) {
        let SemanticArtifactState { parsed, operation } = *self.state;
        (parsed, operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
