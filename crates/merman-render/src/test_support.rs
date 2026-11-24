//! Source-fixture adapters used only by renderer unit tests.

use merman_model::presentation::PresentationConfig;
use merman_model::resources::{InputResourcePolicy, ResourceProfile};
use merman_model::{MermaidConfig, OperationControl};

pub(crate) fn presentation_config(value: serde_json::Value) -> PresentationConfig {
    presentation_config_from_config(MermaidConfig::from_value(value))
}

pub(crate) fn presentation_config_from_config(config: MermaidConfig) -> PresentationConfig {
    PresentationConfig::from_config_controlled(
        config,
        &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap()
}
