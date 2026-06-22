use crate::common::{BindingError, internal_json_error};
use serde::Serialize;
use std::sync::OnceLock;

static SUPPORTED_DIAGRAMS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static ASCII_SUPPORTED_DIAGRAMS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static SUPPORTED_THEMES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static SUPPORTED_HOST_THEME_PRESETS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static BINDING_CAPABILITIES_JSON: OnceLock<Vec<u8>> = OnceLock::new();

#[cfg(feature = "ascii")]
pub const ASCII_SUPPORTED_DIAGRAMS: &[&str] = &["class", "er", "flowchart", "sequence", "xychart"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BindingCapabilities {
    pub render: bool,
    pub ascii: bool,
    pub core_full: bool,
    pub core_host: bool,
    pub ratex_math: bool,
    pub text_measurement: TextMeasurementCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TextMeasurementCapabilities {
    pub vendored: bool,
    pub deterministic: bool,
    pub host_callback: bool,
    pub font_assets: bool,
}

pub const fn binding_capabilities() -> BindingCapabilities {
    BindingCapabilities {
        render: cfg!(feature = "render"),
        ascii: cfg!(feature = "ascii"),
        core_full: cfg!(feature = "core-full") || cfg!(feature = "ascii"),
        core_host: cfg!(feature = "core-host") || cfg!(feature = "ascii"),
        ratex_math: cfg!(feature = "ratex-math"),
        text_measurement: TextMeasurementCapabilities {
            vendored: cfg!(feature = "render"),
            deterministic: cfg!(feature = "render"),
            host_callback: false,
            font_assets: false,
        },
    }
}

pub fn binding_capabilities_json() -> Result<Vec<u8>, BindingError> {
    if let Some(bytes) = BINDING_CAPABILITIES_JSON.get() {
        return Ok(bytes.clone());
    }

    let bytes = serde_json::to_vec(&binding_capabilities()).map_err(internal_json_error)?;
    let _ = BINDING_CAPABILITIES_JSON.set(bytes.clone());
    Ok(bytes)
}

pub fn supported_themes() -> &'static [&'static str] {
    merman::supported_themes()
}

pub fn supported_host_theme_presets() -> &'static [&'static str] {
    #[cfg(feature = "render")]
    {
        merman::supported_host_theme_presets()
    }
    #[cfg(not(feature = "render"))]
    {
        &[]
    }
}

pub fn supported_diagrams() -> &'static [&'static str] {
    merman::supported_diagrams()
}

pub fn ascii_supported_diagrams() -> &'static [&'static str] {
    #[cfg(feature = "ascii")]
    {
        ASCII_SUPPORTED_DIAGRAMS
    }
    #[cfg(not(feature = "ascii"))]
    {
        &[]
    }
}

pub fn supported_diagrams_json() -> Result<Vec<u8>, BindingError> {
    cached_json(&SUPPORTED_DIAGRAMS_JSON, supported_diagrams)
}

pub fn ascii_supported_diagrams_json() -> Result<Vec<u8>, BindingError> {
    cached_json(&ASCII_SUPPORTED_DIAGRAMS_JSON, ascii_supported_diagrams)
}

pub fn supported_themes_json() -> Result<Vec<u8>, BindingError> {
    cached_json(&SUPPORTED_THEMES_JSON, supported_themes)
}

pub fn supported_host_theme_presets_json() -> Result<Vec<u8>, BindingError> {
    cached_json(
        &SUPPORTED_HOST_THEME_PRESETS_JSON,
        supported_host_theme_presets,
    )
}

fn cached_json(
    cache: &OnceLock<Vec<u8>>,
    values: fn() -> &'static [&'static str],
) -> Result<Vec<u8>, BindingError> {
    if let Some(bytes) = cache.get() {
        return Ok(bytes.clone());
    }

    let bytes = serde_json::to_vec(values()).map_err(internal_json_error)?;
    let _ = cache.set(bytes.clone());
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn supported_themes_exposes_core_theme_surface() {
        assert_eq!(
            supported_themes(),
            &[
                "default",
                "base",
                "dark",
                "forest",
                "neutral",
                "neo",
                "neo-dark",
                "redux",
                "redux-dark",
                "redux-color",
                "redux-dark-color"
            ]
        );
    }

    #[test]
    fn binding_capabilities_follow_feature_flags() {
        let capabilities = binding_capabilities();

        assert_eq!(capabilities.render, cfg!(feature = "render"));
        assert_eq!(capabilities.ascii, cfg!(feature = "ascii"));
        assert_eq!(
            capabilities.core_full,
            cfg!(feature = "core-full") || cfg!(feature = "ascii")
        );
        assert_eq!(
            capabilities.core_host,
            cfg!(feature = "core-host") || cfg!(feature = "ascii")
        );
        assert_eq!(capabilities.ratex_math, cfg!(feature = "ratex-math"));
        assert_eq!(
            capabilities.text_measurement.vendored,
            cfg!(feature = "render")
        );
        assert_eq!(
            capabilities.text_measurement.deterministic,
            cfg!(feature = "render")
        );
        assert!(!capabilities.text_measurement.host_callback);
        assert!(!capabilities.text_measurement.font_assets);
    }

    #[test]
    fn binding_capabilities_json_reports_text_measurement_boundary() {
        let capabilities: Value =
            serde_json::from_slice(&binding_capabilities_json().unwrap()).unwrap();

        assert_eq!(capabilities["render"], cfg!(feature = "render"));
        assert_eq!(
            capabilities["text_measurement"]["vendored"],
            cfg!(feature = "render")
        );
        assert_eq!(
            capabilities["text_measurement"]["deterministic"],
            cfg!(feature = "render")
        );
        assert_eq!(capabilities["text_measurement"]["host_callback"], false);
        assert_eq!(capabilities["text_measurement"]["font_assets"], false);
    }

    #[test]
    fn supported_diagrams_exposes_binding_surface() {
        assert_eq!(supported_diagrams(), merman::supported_diagrams());
        assert!(supported_diagrams().contains(&"flowchart"));
        assert!(supported_diagrams().contains(&"sequence"));
        assert!(supported_diagrams().contains(&"requirement"));
    }

    #[test]
    fn supported_host_theme_presets_exposes_render_theme_surface() {
        if cfg!(feature = "render") {
            assert_eq!(
                supported_host_theme_presets(),
                &[
                    "editor-light",
                    "editor-dark",
                    "one-dark",
                    "gruvbox-light",
                    "gruvbox-dark",
                    "ayu-light",
                    "ayu-dark"
                ]
            );
        } else {
            assert!(supported_host_theme_presets().is_empty());
        }
    }

    #[test]
    fn ascii_supported_diagrams_reflects_feature_surface() {
        if cfg!(feature = "ascii") {
            assert_eq!(
                ascii_supported_diagrams(),
                &["class", "er", "flowchart", "sequence", "xychart"]
            );
        } else {
            assert!(ascii_supported_diagrams().is_empty());
        }
    }

    #[test]
    fn metadata_json_helpers_return_arrays() {
        let diagrams: Value = serde_json::from_slice(&supported_diagrams_json().unwrap()).unwrap();
        let ascii_diagrams: Value =
            serde_json::from_slice(&ascii_supported_diagrams_json().unwrap()).unwrap();
        let themes: Value = serde_json::from_slice(&supported_themes_json().unwrap()).unwrap();
        let host_presets: Value =
            serde_json::from_slice(&supported_host_theme_presets_json().unwrap()).unwrap();

        assert!(
            diagrams
                .as_array()
                .unwrap()
                .contains(&Value::String("flowchart".to_string()))
        );
        assert!(ascii_diagrams.is_array());
        assert!(
            themes
                .as_array()
                .unwrap()
                .contains(&Value::String("default".to_string()))
        );
        assert!(host_presets.is_array());
        if cfg!(feature = "render") {
            assert!(
                host_presets
                    .as_array()
                    .unwrap()
                    .contains(&Value::String("one-dark".to_string()))
            );
        }
    }
}
