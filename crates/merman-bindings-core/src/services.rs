use crate::{BindingError, ConstructorServiceKey, common};
#[cfg(feature = "svg")]
use std::sync::Arc;

/// Immutable constructor-owned services shared by a reusable binding engine.
///
/// Foreign transports remain responsible for callback admission, retention, quiescence, and
/// out-of-lock destruction. This value only carries transport-neutral Rust service objects into
/// the engine's single materialization path.
#[derive(Clone, Default)]
#[non_exhaustive]
pub struct BindingEngineServices {
    #[cfg(feature = "svg")]
    host_text_measurer: Option<Arc<dyn crate::HostTextMeasurer>>,
}

impl std::fmt::Debug for BindingEngineServices {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = formatter.debug_struct("BindingEngineServices");
        #[cfg(feature = "svg")]
        debug.field("host_text_measurer", &self.host_text_measurer.is_some());
        debug.finish_non_exhaustive()
    }
}

impl BindingEngineServices {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "svg")]
            host_text_measurer: None,
        }
    }

    #[cfg(feature = "svg")]
    #[must_use]
    pub fn with_host_text_measurer(mut self, measurer: Arc<dyn crate::HostTextMeasurer>) -> Self {
        self.host_text_measurer = Some(measurer);
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        #[cfg(feature = "svg")]
        {
            self.host_text_measurer.is_none()
        }
        #[cfg(not(feature = "svg"))]
        {
            true
        }
    }

    pub(crate) fn validate_options(
        &self,
        options: &common::BindingOptions,
    ) -> Result<(), BindingError> {
        #[cfg(feature = "svg")]
        if self.host_text_measurer.is_some() && options.text_measurement_selector_explicit {
            return Err(BindingError::invalid_argument(
                "constructor service `host-text-measurement` conflicts with explicit option `environment.text_measurement`",
            ));
        }
        #[cfg(not(feature = "svg"))]
        let _ = options;
        Ok(())
    }

    #[cfg(feature = "svg")]
    pub(crate) fn host_text_measurer(&self) -> Option<Arc<dyn crate::HostTextMeasurer>> {
        self.host_text_measurer.as_ref().map(Arc::clone)
    }

    pub(crate) fn service_keys(&self) -> impl Iterator<Item = ConstructorServiceKey> {
        #[cfg(feature = "svg")]
        {
            self.host_text_measurer
                .is_some()
                .then_some(ConstructorServiceKey::HostTextMeasurement)
                .into_iter()
        }
        #[cfg(not(feature = "svg"))]
        {
            std::iter::empty()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_services_exist_without_optional_features() {
        let services = BindingEngineServices::new();
        assert!(services.is_empty());
        assert!(services.clone().is_empty());
    }
}
