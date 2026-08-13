//! Internal operation ownership for the public rendering facade.
//!
//! This module owns the one source-to-semantic execution boundary. Target adapters receive the
//! completed operation projection and must not create a replacement runtime context or control.

use merman_core::{
    Engine, OperationControl, OperationPhase, ParseMetadata, ParseOptions, ParsedDiagramRender,
    resources::InputResourcePolicy, runtime::OperationContext,
};

use crate::render::RenderError;
#[cfg(feature = "svg")]
use merman_render::diagram_theme::DiagramTheme;

/// Immutable operation-owned values shared by every target adapter.
#[derive(Debug)]
pub(crate) struct Operation {
    engine: Engine,
    pub(crate) control: OperationControl,
    pub(crate) context: OperationContext,
    resources: InputResourcePolicy,
    #[cfg(feature = "svg")]
    theme: Option<DiagramTheme>,
}

impl Operation {
    pub(crate) fn begin(
        engine: &Engine,
        source: &str,
        control: OperationControl,
        resources: InputResourcePolicy,
        #[cfg(feature = "svg")] theme: Option<DiagramTheme>,
    ) -> Result<Self, RenderError> {
        control
            .checkpoint_at(OperationPhase::Admission)
            .map_err(RenderError::Cancelled)?;
        resources
            .check_source_bytes(source)
            .map_err(crate::render::ResourceLimitExceeded::from_input)?;
        let context = engine
            .begin_operation()
            .map_err(RenderError::RuntimePolicy)?;
        control
            .checkpoint_at(OperationPhase::Admission)
            .map_err(RenderError::Cancelled)?;
        Ok(Self {
            engine: engine.clone(),
            control,
            context,
            resources,
            #[cfg(feature = "svg")]
            theme,
        })
    }

    pub(crate) fn parse_render_model(
        self,
        source: &str,
        parse_options: ParseOptions,
    ) -> Result<Option<SemanticArtifact>, RenderError> {
        self.control
            .checkpoint_at(OperationPhase::Parse)
            .map_err(RenderError::Cancelled)?;
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
            Ok(result) => result.map_err(RenderError::Parse)?,
        };
        let Some(parsed) = parsed else {
            return Ok(None);
        };
        self.resources
            .check_parsed_render(&parsed)
            .map_err(crate::render::ResourceLimitExceeded::from_input)?;
        self.control
            .checkpoint_at(OperationPhase::Semantic)
            .map_err(RenderError::Cancelled)?;
        Ok(Some(SemanticArtifact {
            state: Box::new(SemanticArtifactState {
                parsed,
                operation: OperationExecution {
                    control: self.control,
                    #[cfg(any(feature = "svg", feature = "ascii"))]
                    context: self.context,
                    #[cfg(feature = "svg")]
                    theme: self.theme,
                },
            }),
        }))
    }
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

    /// Returns the catalog-owned family selected after detection defaults and configuration
    /// effects were applied for this canonical operation.
    pub fn family_id(&self) -> Option<merman_core::DiagramFamilyId> {
        self.state.parsed.family_id()
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
