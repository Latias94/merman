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

/// Exercises source-owned mutation while preserving the imported frozen declarations exactly.
pub(crate) fn set_config_value(
    config: PresentationConfig,
    path: &str,
    value: serde_json::Value,
) -> PresentationConfig {
    let control = OperationControl::new();
    let mut builder = merman_core::presentation::SourcePresentationConfig::from_frozen_controlled(
        config, &control,
    )
    .unwrap();
    builder.set_value(path, value);
    builder
        .into_presentation_config_controlled(
            &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
            &control,
        )
        .unwrap()
        .unwrap()
}
