use merman_core::MermaidEngine;
use merman_core::presentation::SourcePresentationConfig;

pub fn legacy_init_theme_compat_config() -> SourcePresentationConfig {
    SourcePresentationConfig::from_value(serde_json::json!({
        "secure": [
            "secure",
            "securityLevel",
            "startOnLoad",
            "maxTextSize",
            "suppressErrorRendering",
            "maxEdges"
        ]
    }))
}

#[allow(dead_code)]
pub fn legacy_init_theme_compat_engine() -> MermaidEngine {
    MermaidEngine::new().with_site_config(legacy_init_theme_compat_config())
}

#[allow(dead_code)]
pub fn presentation_config(
    value: serde_json::Value,
) -> merman_model::presentation::PresentationConfig {
    presentation_config_from_config(merman_model::MermaidConfig::from_value(value))
}

#[allow(dead_code)]
pub fn presentation_config_from_config(
    config: merman_model::MermaidConfig,
) -> merman_model::presentation::PresentationConfig {
    merman_model::presentation::PresentationConfig::from_config_controlled(
        config,
        &merman_model::resources::InputResourcePolicy::for_profile(
            merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
        ),
        &merman_model::OperationControl::new(),
    )
    .unwrap()
    .unwrap()
}

#[allow(dead_code)]
pub fn install_parse_compatibility(
    theme: &merman_render::diagram_theme::DiagramTheme,
    engine: merman_core::MermaidEngine,
) -> merman_core::MermaidEngine {
    let plan = merman_core::presentation::ThemeCompatibilityPlan::from_recipe(
        theme.compatibility_recipe().clone(),
    );
    engine.with_theme_plan(&plan)
}
