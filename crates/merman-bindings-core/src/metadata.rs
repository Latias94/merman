use crate::common::{BindingError, internal_json_error};
use serde::Serialize;
use std::sync::OnceLock;

static SUPPORTED_DIAGRAMS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static ASCII_SUPPORTED_DIAGRAMS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static SUPPORTED_THEMES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static SUPPORTED_HOST_THEME_PRESETS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static DIAGRAM_FAMILY_CAPABILITIES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static BINDING_CAPABILITIES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static LINT_RULE_CATALOG_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static CONFIGURABLE_LINT_RULE_CATALOG_JSON: OnceLock<Vec<u8>> = OnceLock::new();

#[cfg(feature = "ascii")]
pub const ASCII_SUPPORTED_DIAGRAMS: &[&str] = &["class", "er", "flowchart", "sequence", "xychart"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BindingCapabilities {
    pub render: bool,
    pub ascii: bool,
    pub core_full: bool,
    pub core_host: bool,
    pub elk_layout: bool,
    pub ratex_math: bool,
    pub editor_language: bool,
    pub text_measurement: TextMeasurementCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TextMeasurementCapabilities {
    pub vendored: bool,
    pub deterministic: bool,
    pub host_callback: bool,
    pub font_assets: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BindingDiagramFamilyCapability {
    pub diagram_type: &'static str,
    pub metadata_id: Option<&'static str>,
    pub has_semantic_parser: bool,
    pub has_render_parser: bool,
}

pub const fn binding_capabilities() -> BindingCapabilities {
    BindingCapabilities {
        render: cfg!(feature = "render"),
        ascii: cfg!(feature = "ascii"),
        core_full: cfg!(feature = "core-full") || cfg!(feature = "ascii"),
        core_host: cfg!(feature = "core-host") || cfg!(feature = "ascii"),
        elk_layout: cfg!(feature = "elk-layout"),
        ratex_math: cfg!(feature = "ratex-math"),
        editor_language: cfg!(feature = "editor-language"),
        text_measurement: TextMeasurementCapabilities {
            vendored: cfg!(feature = "render"),
            deterministic: cfg!(feature = "render"),
            host_callback: false,
            font_assets: false,
        },
    }
}

pub fn selected_registry_profile() -> &'static str {
    merman::selected_baseline_registry_profile().as_str()
}

pub fn diagram_family_capabilities() -> Vec<BindingDiagramFamilyCapability> {
    merman::diagram_family_capabilities()
        .iter()
        .map(|capability| BindingDiagramFamilyCapability {
            diagram_type: capability.diagram_type,
            metadata_id: capability.metadata_id,
            has_semantic_parser: capability.has_semantic_parser,
            has_render_parser: capability.has_render_parser,
        })
        .collect()
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

pub fn lint_rule_catalog() -> Vec<merman_analysis::RuleCatalogEntry> {
    merman_analysis::rule_catalog()
}

pub fn configurable_lint_rule_catalog() -> Vec<merman_analysis::RuleCatalogEntry> {
    merman_analysis::configurable_rule_catalog()
}

pub fn lint_rule_catalog_json() -> Result<Vec<u8>, BindingError> {
    if let Some(bytes) = LINT_RULE_CATALOG_JSON.get() {
        return Ok(bytes.clone());
    }

    let bytes = merman_analysis::rule_catalog_json_bytes().map_err(internal_json_error)?;
    let _ = LINT_RULE_CATALOG_JSON.set(bytes.clone());
    Ok(bytes)
}

pub fn configurable_lint_rule_catalog_json() -> Result<Vec<u8>, BindingError> {
    if let Some(bytes) = CONFIGURABLE_LINT_RULE_CATALOG_JSON.get() {
        return Ok(bytes.clone());
    }

    let bytes =
        merman_analysis::configurable_rule_catalog_json_bytes().map_err(internal_json_error)?;
    let _ = CONFIGURABLE_LINT_RULE_CATALOG_JSON.set(bytes.clone());
    Ok(bytes)
}

pub fn diagram_family_capabilities_json() -> Result<Vec<u8>, BindingError> {
    if let Some(bytes) = DIAGRAM_FAMILY_CAPABILITIES_JSON.get() {
        return Ok(bytes.clone());
    }

    let bytes = serde_json::to_vec(&diagram_family_capabilities()).map_err(internal_json_error)?;
    let _ = DIAGRAM_FAMILY_CAPABILITIES_JSON.set(bytes.clone());
    Ok(bytes)
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
        assert_eq!(capabilities.elk_layout, cfg!(feature = "elk-layout"));
        assert_eq!(capabilities.ratex_math, cfg!(feature = "ratex-math"));
        assert_eq!(
            capabilities.editor_language,
            cfg!(feature = "editor-language")
        );
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
        assert_eq!(
            capabilities["editor_language"],
            cfg!(feature = "editor-language")
        );
        assert_eq!(capabilities["text_measurement"]["host_callback"], false);
        assert_eq!(capabilities["text_measurement"]["font_assets"], false);
    }

    #[test]
    fn selected_registry_profile_reports_core_profile() {
        assert_eq!(
            selected_registry_profile(),
            merman::selected_baseline_registry_profile().as_str()
        );
        assert_eq!(
            selected_registry_profile(),
            if cfg!(feature = "core-full") || cfg!(feature = "ascii") {
                "full"
            } else {
                "tiny"
            }
        );
    }

    #[test]
    fn supported_diagrams_exposes_binding_surface() {
        assert_eq!(supported_diagrams(), merman::supported_diagrams());
        assert!(supported_diagrams().contains(&"flowchart"));
        assert!(supported_diagrams().contains(&"sequence"));
        assert!(supported_diagrams().contains(&"requirement"));
    }

    #[test]
    fn diagram_family_capabilities_expose_parser_and_render_surface() {
        let capabilities = diagram_family_capabilities();
        assert_eq!(
            capabilities.len(),
            merman::diagram_family_capabilities().len()
        );

        let flowchart = capabilities
            .iter()
            .find(|capability| capability.diagram_type == "flowchart")
            .expect("flowchart capability should be present");
        assert_eq!(flowchart.metadata_id, Some("flowchart"));
        assert!(flowchart.has_semantic_parser);
        assert!(flowchart.has_render_parser);

        let has_mindmap = capabilities
            .iter()
            .any(|capability| capability.diagram_type == "mindmap");
        assert_eq!(has_mindmap, selected_registry_profile() == "full");
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
        let family_capabilities: Value =
            serde_json::from_slice(&diagram_family_capabilities_json().unwrap()).unwrap();
        let lint_rules: Value = serde_json::from_slice(&lint_rule_catalog_json().unwrap()).unwrap();
        let configurable_lint_rules: Value =
            serde_json::from_slice(&configurable_lint_rule_catalog_json().unwrap()).unwrap();

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
        assert!(
            family_capabilities
                .as_array()
                .unwrap()
                .iter()
                .any(|capability| capability["diagram_type"] == "flowchart")
        );
        assert!(lint_rules.as_array().unwrap().iter().any(|rule| {
            rule["id"] == "merman.authoring.flowchart.explicit_direction"
                && rule["origin"] == "merman_authoring"
                && rule["evidence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value == "docs/adr/0072-lint-rule-governance.md")
        }));
        assert!(
            configurable_lint_rules
                .as_array()
                .unwrap()
                .iter()
                .all(|rule| rule["category"] != "internal")
        );
    }
}
