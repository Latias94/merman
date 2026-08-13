//! Internal operation ownership for the public rendering facade.
//!
//! This module owns the one source-to-semantic execution boundary. Target adapters receive the
//! completed operation projection and must not create a replacement runtime context or control.

use merman_core::{
    Engine, OperationControl, OperationPhase, ParseMetadata, ParseOptions, ParsedDiagramRender,
    resources::InputResourcePolicy, runtime::OperationContext,
};

use crate::render::RenderError;

/// Immutable operation-owned values shared by every target adapter.
#[derive(Debug, Clone)]
pub(crate) struct Operation {
    pub(crate) engine: Engine,
    pub(crate) control: OperationControl,
    pub(crate) context: OperationContext,
    pub(crate) resources: InputResourcePolicy,
}

impl Operation {
    pub(crate) fn begin(
        engine: &Engine,
        source: &str,
        control: OperationControl,
        resources: InputResourcePolicy,
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
        })
    }

    pub(crate) fn parse_render_model(
        &self,
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
            parsed,
            #[cfg(any(feature = "svg", feature = "ascii"))]
            operation: self.clone(),
        }))
    }
}

/// A format-neutral semantic artifact paired with the operation that produced it.
///
/// The artifact is intentionally not constructible from an arbitrary metadata/model pair. SVG
/// and ASCII adapters consume this canonical pair and choose their own layout and emission path.
#[derive(Debug)]
pub struct SemanticArtifact {
    pub(crate) parsed: ParsedDiagramRender,
    #[cfg(any(feature = "svg", feature = "ascii"))]
    pub(crate) operation: Operation,
}

impl SemanticArtifact {
    /// Returns metadata captured by the controlled parse operation.
    pub fn metadata(&self) -> &ParseMetadata {
        self.parsed.metadata()
    }

    /// Returns the stable family/model kind selected by the semantic parser.
    pub fn semantic_kind(&self) -> &'static str {
        self.parsed.model().kind()
    }

    /// Returns the typed Mermaid diagram id selected during preprocessing.
    pub fn diagram_type(&self) -> &str {
        &self.metadata().diagram_type
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    pub(crate) fn into_parts(self) -> (ParsedDiagramRender, Operation) {
        (self.parsed, self.operation)
    }
}
