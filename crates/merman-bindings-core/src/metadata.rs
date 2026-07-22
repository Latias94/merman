use crate::common::{
    BINDING_OPTIONS_SCHEMA_VERSION, BINDING_RESULT_PAYLOAD_VERSION, BindingError,
    internal_json_error,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub const RUNTIME_CONTRACT_SCHEMA_VERSION: u32 = 2;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeContract {
    pub schema_version: u32,
    pub abi_version: u32,
    pub package_version: &'static str,
    pub options_schema_version: u32,
    pub payload_schemas: BTreeMap<&'static str, u32>,
    pub features: BindingCapabilities,
    pub registry: RuntimeRegistryContract,
    pub resources: Option<RuntimeResourceContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeRegistryContract {
    pub diagram_family_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeResourceContract {
    pub schema_version: u32,
    pub general_binding_default_profile: &'static str,
    pub cli_default_profile: &'static str,
    pub limits: Vec<RuntimeResourceLimit>,
    pub profiles: Vec<RuntimeResourceProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeResourceLimit {
    pub id: &'static str,
    pub phase: &'static str,
    pub description: &'static str,
    pub overridable: bool,
    pub hard_cap: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeResourceProfile {
    pub id: &'static str,
    pub purpose: &'static str,
    pub trust_assumption: &'static str,
    pub recommended_binding_default: bool,
    pub limits: BTreeMap<&'static str, Option<usize>>,
}

pub use merman::DiagramFamilyCapability as BindingDiagramFamilyCapability;

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

pub fn runtime_contract(abi_version: u32) -> RuntimeContract {
    let payload_schemas = BTreeMap::from([("binding_result", BINDING_RESULT_PAYLOAD_VERSION)]);
    #[cfg(feature = "analysis")]
    let payload_schemas = {
        let mut payload_schemas = payload_schemas;
        payload_schemas.insert("analysis", merman_analysis::ANALYSIS_PAYLOAD_VERSION);
        payload_schemas.insert(
            "analysis_facts",
            merman_analysis::ANALYSIS_FACTS_PAYLOAD_VERSION,
        );
        payload_schemas
    };

    RuntimeContract {
        schema_version: RUNTIME_CONTRACT_SCHEMA_VERSION,
        abi_version,
        package_version: env!("CARGO_PKG_VERSION"),
        options_schema_version: BINDING_OPTIONS_SCHEMA_VERSION,
        payload_schemas,
        features: binding_capabilities(),
        registry: RuntimeRegistryContract {
            diagram_family_count: diagram_family_capabilities().len(),
        },
        resources: runtime_resource_contract(),
    }
}

pub fn runtime_contract_json(abi_version: u32) -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(&runtime_contract(abi_version)).map_err(internal_json_error)
}

#[cfg(feature = "render")]
fn runtime_resource_contract() -> Option<RuntimeResourceContract> {
    let limits = merman::render::resource_limit_descriptors()
        .iter()
        .map(|descriptor| RuntimeResourceLimit {
            id: descriptor.stable_id,
            phase: descriptor.phase.as_str(),
            description: descriptor.description,
            overridable: descriptor.overridable,
            hard_cap: descriptor.hard_cap,
        })
        .collect();
    let profiles = merman::render::resource_profile_descriptors()
        .iter()
        .map(|descriptor| RuntimeResourceProfile {
            id: descriptor.id,
            purpose: descriptor.purpose,
            trust_assumption: descriptor.trust_assumption,
            recommended_binding_default: descriptor.recommended_binding_default,
            limits: merman::render::resource_limit_descriptors()
                .iter()
                .map(|limit| (limit.stable_id, descriptor.limits.value(limit.id)))
                .collect(),
        })
        .collect();
    Some(RuntimeResourceContract {
        schema_version: merman::render::RESOURCE_CONTRACT_SCHEMA_VERSION,
        general_binding_default_profile: merman::render::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE
            .id(),
        cli_default_profile: merman::render::CLI_DEFAULT_RESOURCE_PROFILE.id(),
        limits,
        profiles,
    })
}

#[cfg(not(feature = "render"))]
const fn runtime_resource_contract() -> Option<RuntimeResourceContract> {
    None
}

pub fn diagram_family_capabilities() -> Vec<BindingDiagramFamilyCapability> {
    merman::diagram_family_capabilities().to_vec()
}

pub fn binding_capabilities_json() -> Result<Vec<u8>, BindingError> {
    if let Some(bytes) = BINDING_CAPABILITIES_JSON.get() {
        return Ok(bytes.clone());
    }

    let bytes = binding_capabilities_json_for(binding_capabilities())?;
    let _ = BINDING_CAPABILITIES_JSON.set(bytes.clone());
    Ok(bytes)
}

pub fn binding_capabilities_json_for(
    capabilities: BindingCapabilities,
) -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(&capabilities).map_err(internal_json_error)
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
        Err(crate::common::feature_required_error(
            "lint rule catalog",
            "analysis",
        ))
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
        Err(crate::common::feature_required_error(
            "configurable lint rule catalog",
            "analysis",
        ))
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
    fn runtime_contract_is_versioned_and_projects_the_resource_descriptor() {
        let contract = runtime_contract(2);
        assert_eq!(contract.schema_version, RUNTIME_CONTRACT_SCHEMA_VERSION);
        assert_eq!(contract.abi_version, 2);
        assert_eq!(contract.package_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            contract.options_schema_version,
            BINDING_OPTIONS_SCHEMA_VERSION
        );
        assert_eq!(contract.features, binding_capabilities());
        assert_eq!(
            contract.registry.diagram_family_count,
            diagram_family_capabilities().len()
        );

        #[cfg(feature = "render")]
        {
            let resources = contract.resources.expect("render resource catalog");
            assert_eq!(
                resources.schema_version,
                merman::render::RESOURCE_CONTRACT_SCHEMA_VERSION
            );
            assert_eq!(resources.profiles.len(), 4);
            assert_eq!(resources.limits.len(), 16);
            assert_eq!(resources.general_binding_default_profile, "interactive");
            assert_eq!(resources.cli_default_profile, "trusted-native");
            let tree_depth = resources
                .limits
                .iter()
                .find(|limit| limit.id == "max_svg_tree_depth")
                .expect("tree-depth capability");
            assert!(tree_depth.hard_cap);
            assert!(!tree_depth.overridable);
            let interactive = resources
                .profiles
                .iter()
                .find(|profile| profile.id == "interactive")
                .expect("interactive profile");
            assert_eq!(interactive.limits["max_venn_areas"], Some(8_000));
            assert_eq!(
                interactive.limits["max_swimlane_line_hop_segment_pairs"],
                Some(250_000)
            );
        }
        #[cfg(not(feature = "render"))]
        assert!(contract.resources.is_none());

        let json: Value = serde_json::from_slice(&runtime_contract_json(2).unwrap()).unwrap();
        assert_eq!(json["schema_version"], RUNTIME_CONTRACT_SCHEMA_VERSION);
        assert_eq!(json["abi_version"], 2);
    }

    #[test]
    fn supported_diagrams_exposes_binding_surface() {
        assert_eq!(supported_diagrams(), merman::supported_diagrams());
        assert!(supported_diagrams().contains(&"flowchart"));
        assert!(supported_diagrams().contains(&"sequence"));
        assert!(supported_diagrams().contains(&"requirement"));
    }

    #[test]
    fn diagram_family_capabilities_expose_the_complete_core_catalog() {
        let capabilities = diagram_family_capabilities();
        assert_eq!(capabilities, merman::diagram_family_capabilities());

        let flowchart = capabilities
            .iter()
            .find(|capability| capability.diagram_type == "flowchart")
            .expect("flowchart capability should be present");
        assert_eq!(flowchart.metadata_id, Some("flowchart"));
        assert_eq!(flowchart.logical_family_kind, "flowchart");
        assert_eq!(flowchart.render_model_kind, Some("flowchart"));
        assert!(flowchart.has_detector);
        assert!(flowchart.has_semantic_parser);
        assert!(flowchart.has_editor_parser);
        assert!(flowchart.has_combined_parser);
        assert!(flowchart.has_render_parser);
        assert!(!flowchart.has_header);
        assert_eq!(flowchart.config_namespace, Some("flowchart"));

        let swimlane = capabilities
            .iter()
            .find(|capability| capability.diagram_type == "swimlane")
            .expect("11.16 swimlane capability should be present");
        assert_eq!(swimlane.metadata_id, Some("swimlane"));
        assert_eq!(swimlane.logical_family_kind, "swimlane");
        assert_eq!(swimlane.render_model_kind, Some("flowchart"));
        assert!(swimlane.has_detector);
        assert!(swimlane.has_semantic_parser);
        assert!(swimlane.has_editor_parser);
        assert!(swimlane.has_combined_parser);
        assert!(swimlane.has_render_parser);
        assert!(swimlane.has_header);
        assert_eq!(swimlane.config_namespace, Some("swimlane"));

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

        assert!(
            capabilities
                .iter()
                .any(|capability| capability.diagram_type == "mindmap")
        );
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
            assert!(!supported.contains(&"zenuml"));
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

        assert!(
            capabilities
                .iter()
                .all(|capability| capability.diagram_type != "zenuml"),
            "ZenUML has no family-owned terminal projection"
        );
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
        let flowchart = family_capabilities
            .as_array()
            .unwrap()
            .iter()
            .find(|capability| capability["diagram_type"] == "flowchart")
            .expect("flowchart family capability should be present");
        assert_eq!(flowchart["logical_family_kind"], "flowchart");
        assert_eq!(flowchart["render_model_kind"], "flowchart");
        assert_eq!(flowchart["has_detector"], true);
        assert_eq!(flowchart["has_editor_parser"], true);
        assert_eq!(flowchart["has_combined_parser"], true);
        assert_eq!(flowchart["has_header"], false);
        assert_eq!(flowchart["config_namespace"], "flowchart");
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
