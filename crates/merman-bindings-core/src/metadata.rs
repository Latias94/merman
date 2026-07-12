use crate::common::{BindingError, internal_json_error};
use serde::Serialize;
use std::sync::OnceLock;

static SUPPORTED_DIAGRAMS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static ASCII_SUPPORTED_DIAGRAMS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static ASCII_CAPABILITIES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static SUPPORTED_THEMES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static SUPPORTED_HOST_THEME_PRESETS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static DIAGRAM_FAMILY_CAPABILITIES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static BINDING_CAPABILITIES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
#[cfg(feature = "analysis")]
static LINT_RULE_CATALOG_JSON: OnceLock<Vec<u8>> = OnceLock::new();
#[cfg(feature = "analysis")]
static CONFIGURABLE_LINT_RULE_CATALOG_JSON: OnceLock<Vec<u8>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BindingCapabilities {
    pub render: bool,
    pub analysis: bool,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BindingAsciiCapability {
    pub diagram_type: &'static str,
    pub display_name: &'static str,
    pub support_level: &'static str,
    pub summary_fallback: bool,
    pub supported_semantics: &'static [&'static str],
    pub limits: &'static [&'static str],
    pub evidence: Vec<BindingAsciiCapabilityEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BindingAsciiCapabilityEvidence {
    pub kind: &'static str,
    pub source: &'static str,
    pub note: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuleCatalogEntry {
    pub id: &'static str,
    pub description: &'static str,
    pub evidence: &'static [&'static str],
    pub default_severity: &'static str,
    pub category: &'static str,
    pub default_enabled: bool,
    pub default_profile: &'static str,
    pub origin: &'static str,
    pub configurable: bool,
    pub fixable: bool,
}

pub const fn binding_capabilities() -> BindingCapabilities {
    BindingCapabilities {
        render: cfg!(feature = "render"),
        analysis: cfg!(feature = "analysis"),
        ascii: cfg!(feature = "ascii"),
        core_full: cfg!(feature = "core-full"),
        core_host: cfg!(feature = "core-host"),
        elk_layout: cfg!(feature = "elk-layout"),
        ratex_math: cfg!(feature = "ratex-math"),
        editor_language: cfg!(feature = "editor-language"),
        text_measurement: TextMeasurementCapabilities {
            vendored: cfg!(feature = "render"),
            deterministic: cfg!(feature = "render"),
            host_callback: cfg!(feature = "render"),
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
        merman::ascii::ascii_supported_diagram_types()
    }
    #[cfg(not(feature = "ascii"))]
    {
        &[]
    }
}

pub fn ascii_capabilities() -> Vec<BindingAsciiCapability> {
    #[cfg(feature = "ascii")]
    {
        merman::ascii::ascii_capabilities()
            .iter()
            .map(|capability| BindingAsciiCapability {
                diagram_type: capability.diagram_type,
                display_name: capability.display_name,
                support_level: capability.support_level.as_str(),
                summary_fallback: capability.summary_fallback,
                supported_semantics: capability.supported_semantics,
                limits: capability.limits,
                evidence: capability
                    .evidence
                    .iter()
                    .map(|evidence| BindingAsciiCapabilityEvidence {
                        kind: evidence.kind.as_str(),
                        source: evidence.source,
                        note: evidence.note,
                    })
                    .collect(),
            })
            .collect()
    }
    #[cfg(not(feature = "ascii"))]
    {
        Vec::new()
    }
}

pub fn supported_diagrams_json() -> Result<Vec<u8>, BindingError> {
    cached_json(&SUPPORTED_DIAGRAMS_JSON, supported_diagrams)
}

pub fn ascii_supported_diagrams_json() -> Result<Vec<u8>, BindingError> {
    cached_json(&ASCII_SUPPORTED_DIAGRAMS_JSON, ascii_supported_diagrams)
}

pub fn ascii_capabilities_json() -> Result<Vec<u8>, BindingError> {
    if let Some(bytes) = ASCII_CAPABILITIES_JSON.get() {
        return Ok(bytes.clone());
    }

    let bytes = serde_json::to_vec(&ascii_capabilities()).map_err(internal_json_error)?;
    let _ = ASCII_CAPABILITIES_JSON.set(bytes.clone());
    Ok(bytes)
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

pub fn lint_rule_catalog() -> Vec<RuleCatalogEntry> {
    #[cfg(feature = "analysis")]
    {
        merman_analysis::rule_catalog()
            .into_iter()
            .map(rule_catalog_entry)
            .collect()
    }
    #[cfg(not(feature = "analysis"))]
    {
        Vec::new()
    }
}

pub fn configurable_lint_rule_catalog() -> Vec<RuleCatalogEntry> {
    #[cfg(feature = "analysis")]
    {
        merman_analysis::configurable_rule_catalog()
            .into_iter()
            .map(rule_catalog_entry)
            .collect()
    }
    #[cfg(not(feature = "analysis"))]
    {
        Vec::new()
    }
}

pub fn lint_rule_catalog_json() -> Result<Vec<u8>, BindingError> {
    #[cfg(not(feature = "analysis"))]
    {
        return Err(crate::common::feature_required_error(
            "lint rule catalog",
            "analysis",
        ));
    }

    #[cfg(feature = "analysis")]
    {
        if let Some(bytes) = LINT_RULE_CATALOG_JSON.get() {
            return Ok(bytes.clone());
        }

        let bytes =
            merman_analysis::rule_catalog_response_json_bytes().map_err(internal_json_error)?;
        let _ = LINT_RULE_CATALOG_JSON.set(bytes.clone());
        Ok(bytes)
    }
}

pub fn configurable_lint_rule_catalog_json() -> Result<Vec<u8>, BindingError> {
    #[cfg(not(feature = "analysis"))]
    {
        return Err(crate::common::feature_required_error(
            "configurable lint rule catalog",
            "analysis",
        ));
    }

    #[cfg(feature = "analysis")]
    {
        if let Some(bytes) = CONFIGURABLE_LINT_RULE_CATALOG_JSON.get() {
            return Ok(bytes.clone());
        }

        let bytes = merman_analysis::configurable_rule_catalog_response_json_bytes()
            .map_err(internal_json_error)?;
        let _ = CONFIGURABLE_LINT_RULE_CATALOG_JSON.set(bytes.clone());
        Ok(bytes)
    }
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

#[cfg(feature = "analysis")]
fn rule_catalog_entry(rule: merman_analysis::RuleCatalogEntry) -> RuleCatalogEntry {
    RuleCatalogEntry {
        id: rule.id,
        description: rule.description,
        evidence: rule.evidence,
        default_severity: rule.default_severity.as_str(),
        category: rule.category.as_str(),
        default_enabled: rule.default_enabled,
        default_profile: rule.default_profile.as_str(),
        origin: rule.origin.as_str(),
        configurable: rule.configurable,
        fixable: rule.fixable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BindingStatus;
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
        assert_eq!(capabilities.analysis, cfg!(feature = "analysis"));
        assert_eq!(capabilities.ascii, cfg!(feature = "ascii"));
        assert_eq!(capabilities.core_full, cfg!(feature = "core-full"));
        assert_eq!(capabilities.core_host, cfg!(feature = "core-host"));
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
        assert_eq!(
            capabilities.text_measurement.host_callback,
            cfg!(feature = "render")
        );
        assert!(!capabilities.text_measurement.font_assets);
    }

    #[test]
    fn binding_capabilities_json_reports_text_measurement_boundary() {
        let capabilities: Value =
            serde_json::from_slice(&binding_capabilities_json().unwrap()).unwrap();

        assert_eq!(capabilities["render"], cfg!(feature = "render"));
        assert_eq!(capabilities["analysis"], cfg!(feature = "analysis"));
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
        assert_eq!(
            capabilities["text_measurement"]["host_callback"],
            cfg!(feature = "render")
        );
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
            if cfg!(feature = "core-full") {
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
    fn diagram_family_capabilities_expose_detector_parser_and_render_surface() {
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

        let swimlane = capabilities
            .iter()
            .find(|capability| capability.diagram_type == "swimlane")
            .expect("parser-only 11.16 swimlane capability should be present");
        assert_eq!(swimlane.metadata_id, None);
        assert!(swimlane.has_semantic_parser);
        assert!(!swimlane.has_render_parser);

        let cynefin = capabilities
            .iter()
            .find(|capability| capability.diagram_type == "cynefin")
            .expect("11.16 cynefin capability should be present");
        assert_eq!(cynefin.metadata_id, Some("cynefin"));
        assert!(cynefin.has_semantic_parser);
        assert!(cynefin.has_render_parser);

        let railroad = capabilities
            .iter()
            .find(|capability| capability.diagram_type == "railroad")
            .expect("11.16 railroad capability should be present");
        assert_eq!(railroad.metadata_id, Some("railroad"));
        assert!(railroad.has_semantic_parser);
        assert!(railroad.has_render_parser);

        for diagram_type in ["railroadEbnf", "railroadAbnf", "railroadPeg"] {
            let railroad_variant = capabilities
                .iter()
                .find(|capability| capability.diagram_type == diagram_type)
                .unwrap_or_else(|| panic!("11.16 {diagram_type} capability should be present"));
            assert_eq!(railroad_variant.metadata_id, Some(diagram_type));
            assert!(railroad_variant.has_semantic_parser);
            assert!(railroad_variant.has_render_parser);
        }

        let has_mindmap = capabilities
            .iter()
            .any(|capability| capability.diagram_type == "mindmap");
        assert_eq!(has_mindmap, cfg!(feature = "core-full"));
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
                &[
                    "class",
                    "er",
                    "flowchart",
                    "gantt",
                    "gitgraph",
                    "journey",
                    "kanban",
                    "mindmap",
                    "packet",
                    "sequence",
                    "state",
                    "timeline",
                    "treeView",
                    "xychart",
                    "zenuml",
                ]
            );
        } else {
            assert!(ascii_supported_diagrams().is_empty());
        }
    }

    #[test]
    fn ascii_supported_diagrams_are_derived_from_capability_records() {
        let capabilities = ascii_capabilities();

        if cfg!(feature = "ascii") {
            let supported: Vec<_> = capabilities
                .iter()
                .filter(|capability| capability.support_level != "unsupported")
                .map(|capability| capability.diagram_type)
                .collect();

            assert_eq!(ascii_supported_diagrams(), supported.as_slice());
            assert!(supported.contains(&"zenuml"));
        } else {
            assert!(capabilities.is_empty());
            assert!(ascii_supported_diagrams().is_empty());
        }
    }

    #[test]
    fn ascii_capabilities_report_support_levels_limits_and_evidence() {
        let capabilities = ascii_capabilities();

        if !cfg!(feature = "ascii") {
            assert!(capabilities.is_empty());
            return;
        }

        let flowchart = ascii_capability(&capabilities, "flowchart");
        assert_eq!(flowchart.support_level, "full");
        assert!(!flowchart.summary_fallback);
        assert!(flowchart.supported_semantics.contains(&"root directions"));
        assert!(flowchart.evidence.iter().any(|evidence| {
            evidence.kind == "local_advantage" && evidence.note.contains("true RL/BT")
        }));

        let class = ascii_capability(&capabilities, "class");
        assert_eq!(class.support_level, "partial");
        assert!(class.summary_fallback);
        assert!(class.limits.iter().any(|limit| limit.contains("namespace")));
        assert!(class.evidence.iter().any(|evidence| {
            evidence.kind == "beautiful_mermaid_prior_art"
                && evidence.source.contains("repo-ref/beautiful-mermaid")
        }));

        let er = ascii_capability(&capabilities, "er");
        assert_eq!(er.support_level, "partial");
        assert!(er.summary_fallback);

        let gantt = ascii_capability(&capabilities, "gantt");
        assert_eq!(gantt.support_level, "summary");

        let xychart = ascii_capability(&capabilities, "xychart");
        assert_eq!(xychart.support_level, "partial");
        assert!(xychart.evidence.iter().any(|evidence| {
            evidence.kind == "beautiful_mermaid_prior_art"
                && evidence.source.contains("xychart-ascii.test.ts")
        }));

        let zenuml = ascii_capability(&capabilities, "zenuml");
        assert_eq!(zenuml.support_level, "partial");
    }

    #[test]
    fn metadata_json_helpers_return_json_contracts() {
        let diagrams: Value = serde_json::from_slice(&supported_diagrams_json().unwrap()).unwrap();
        let ascii_diagrams: Value =
            serde_json::from_slice(&ascii_supported_diagrams_json().unwrap()).unwrap();
        let ascii_capabilities: Value =
            serde_json::from_slice(&ascii_capabilities_json().unwrap()).unwrap();
        let themes: Value = serde_json::from_slice(&supported_themes_json().unwrap()).unwrap();
        let host_presets: Value =
            serde_json::from_slice(&supported_host_theme_presets_json().unwrap()).unwrap();
        let family_capabilities: Value =
            serde_json::from_slice(&diagram_family_capabilities_json().unwrap()).unwrap();
        assert!(
            diagrams
                .as_array()
                .unwrap()
                .contains(&Value::String("flowchart".to_string()))
        );
        assert!(ascii_diagrams.is_array());
        assert!(ascii_capabilities.is_array());
        if cfg!(feature = "ascii") {
            let flowchart = ascii_capabilities
                .as_array()
                .unwrap()
                .iter()
                .find(|capability| capability["diagram_type"] == "flowchart")
                .expect("flowchart ASCII capability should be present");
            assert_eq!(flowchart["support_level"], "full");
            assert!(
                flowchart["evidence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|evidence| evidence["kind"] == "local_advantage")
            );
        }
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
        if cfg!(feature = "analysis") {
            let lint_rules: Value =
                serde_json::from_slice(&lint_rule_catalog_json().unwrap()).unwrap();
            let configurable_lint_rules: Value =
                serde_json::from_slice(&configurable_lint_rule_catalog_json().unwrap()).unwrap();

            assert_eq!(lint_rules["version"], 1);
            let lint_rules = lint_rules["rules"].as_array().unwrap();
            assert!(lint_rules.iter().any(|rule| {
                rule["id"] == "merman.authoring.flowchart.explicit_direction"
                    && rule["origin"] == "merman_authoring"
                    && rule["evidence"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|value| value == "docs/adr/0072-lint-rule-governance.md")
            }));
            assert_eq!(configurable_lint_rules["version"], 1);
            assert!(
                configurable_lint_rules["rules"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|rule| rule["category"] != "internal")
            );
        } else {
            assert_eq!(
                lint_rule_catalog_json().unwrap_err().status(),
                BindingStatus::UnsupportedFormat
            );
            assert_eq!(
                configurable_lint_rule_catalog_json().unwrap_err().status(),
                BindingStatus::UnsupportedFormat
            );
        }
    }

    fn ascii_capability<'a>(
        capabilities: &'a [BindingAsciiCapability],
        diagram_type: &str,
    ) -> &'a BindingAsciiCapability {
        capabilities
            .iter()
            .find(|capability| capability.diagram_type == diagram_type)
            .unwrap_or_else(|| panic!("missing ASCII capability for {diagram_type}"))
    }
}
