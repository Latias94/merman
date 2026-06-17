use merman_core::{Engine, MermaidConfig};

pub fn legacy_init_theme_compat_config() -> MermaidConfig {
    MermaidConfig::from_value(serde_json::json!({
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
pub fn legacy_init_theme_compat_engine() -> Engine {
    Engine::new().with_site_config(legacy_init_theme_compat_config())
}
