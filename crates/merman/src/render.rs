//! Canonical source-to-target rendering facade.
//!
//! `Renderer` owns long-lived defaults. Each [`RenderRequest`] owns one operation control and is
//! executed synchronously through the same internal operation runner. Target-specific layout and
//! emission remain private to their adapters.

use merman_core::{
    Engine, OperationCancelled, OperationControl, ParseOptions,
    resources::{InputResourceLimitExceeded, InputResourcePolicy},
    runtime::RuntimePolicyError,
};

use crate::operation_runner::Operation;

pub use crate::operation_runner::SemanticArtifact;

/// Structured error returned by the canonical rendering facade.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    #[error("operation cancelled during {0}")]
    Cancelled(#[from] OperationCancelled),
    #[error(transparent)]
    Parse(#[from] merman_core::Error),
    #[error(transparent)]
    RuntimePolicy(#[from] RuntimePolicyError),
    #[error(transparent)]
    InputResourceLimitExceeded(#[from] InputResourceLimitExceeded),
    #[error("render target is not available in this feature configuration: {0}")]
    UnsupportedTarget(&'static str),
}

/// Target request for the canonical facade.
///
/// The semantic target is available in every feature configuration. SVG, ASCII, and terminal
/// export variants are added by their adapters while retaining this single dispatch seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderTarget {
    Semantic,
}

/// One source-to-target request. The control is cloneable so a host can retain a handle and cancel
/// the synchronous worker from another task or thread.
#[derive(Debug, Clone)]
pub struct RenderRequest<'a> {
    pub source: &'a str,
    pub target: RenderTarget,
    pub control: OperationControl,
    pub parse_options: ParseOptions,
    pub resources: InputResourcePolicy,
}

impl<'a> RenderRequest<'a> {
    pub fn semantic(source: &'a str, control: OperationControl) -> Self {
        Self {
            source,
            target: RenderTarget::Semantic,
            control,
            parse_options: ParseOptions::default(),
            resources: InputResourcePolicy::default(),
        }
    }

    pub fn with_parse_options(mut self, parse_options: ParseOptions) -> Self {
        self.parse_options = parse_options;
        self
    }

    pub fn with_resource_policy(mut self, resources: InputResourcePolicy) -> Self {
        self.resources = resources;
        self
    }
}

/// Successful output from a canonical request.
#[derive(Debug)]
pub enum RenderOutput {
    Semantic(Option<SemanticArtifact>),
}

/// Long-lived renderer defaults and host-independent engine configuration.
#[derive(Debug, Clone)]
pub struct Renderer {
    engine: Engine,
    parse_options: ParseOptions,
    resources: InputResourcePolicy,
}

impl Default for Renderer {
    fn default() -> Self {
        Self {
            engine: Engine::new(),
            parse_options: ParseOptions::default(),
            resources: InputResourcePolicy::default(),
        }
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_engine(mut self, engine: Engine) -> Self {
        self.engine = engine;
        self
    }

    pub fn with_runtime_policy(mut self, policy: merman_core::runtime::RuntimePolicy) -> Self {
        self.engine = self.engine.with_runtime_policy(policy);
        self
    }

    pub fn with_parse_options(mut self, parse_options: ParseOptions) -> Self {
        self.parse_options = parse_options;
        self
    }

    pub fn with_resource_policy(mut self, resources: InputResourcePolicy) -> Self {
        self.resources = resources;
        self
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn parse_options(&self) -> ParseOptions {
        self.parse_options
    }

    pub fn resource_policy(&self) -> &InputResourcePolicy {
        &self.resources
    }

    /// Executes one typed target request through the canonical operation runner.
    pub fn render(&self, request: RenderRequest<'_>) -> Result<RenderOutput, RenderError> {
        let operation = Operation::begin(
            &self.engine,
            request.source,
            request.control,
            request.resources,
        )?;
        match request.target {
            RenderTarget::Semantic => Ok(RenderOutput::Semantic(
                operation.parse_render_model(request.source, request.parse_options)?,
            )),
        }
    }

    /// Prepares a format-neutral semantic artifact through the same runner used by `render`.
    pub fn prepare_semantic(
        &self,
        source: &str,
        control: OperationControl,
    ) -> Result<Option<SemanticArtifact>, RenderError> {
        self.prepare_semantic_with(source, control, self.parse_options, self.resources)
    }

    pub fn prepare_semantic_with(
        &self,
        source: &str,
        control: OperationControl,
        parse_options: ParseOptions,
        resources: InputResourcePolicy,
    ) -> Result<Option<SemanticArtifact>, RenderError> {
        let operation = Operation::begin(&self.engine, source, control, resources)?;
        operation.parse_render_model(source, parse_options)
    }
}
