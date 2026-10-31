use std::any::Any;
use std::fmt::{self, Debug, Formatter};

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) use crate::presentation::diagram::RegistryOwner;
#[cfg(feature = "presentation")]
pub use crate::presentation::diagram::*;
#[cfg(feature = "compat-json")]
pub(crate) use crate::presentation::diagram::{
    BuiltInDiagramSemanticParser, ResolvedSemanticParser, parse_or_unsupported_controlled,
};
#[cfg(feature = "presentation")]
pub(crate) use crate::presentation::diagram::{
    BuiltInRenderSemanticParser, RenderSemanticParseOutput, ResolvedRenderParser,
};

/// A panic captured with its original payload for the legacy rethrow adapter.
pub struct CapturedPanic {
    message: String,
    payload: Box<dyn Any + Send + 'static>,
}

impl CapturedPanic {
    #[cfg(feature = "editor-facts")]
    pub(crate) fn from_payload(payload: Box<dyn Any + Send + 'static>) -> Self {
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("panic while analyzing Mermaid source")
            .to_string();
        Self { message, payload }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn into_payload(self) -> Box<dyn Any + Send + 'static> {
        self.payload
    }
}

impl Debug for CapturedPanic {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapturedPanic")
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}
