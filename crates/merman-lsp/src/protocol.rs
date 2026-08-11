use std::str::FromStr;

use crate::session::{DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS, DEFAULT_LSP_MAX_SOURCE_BYTES};
pub use merman_analysis::FIXED_TODAY_SCHEMA_PATTERN;
use merman_analysis::{AnalysisConfigContract, AnalysisConfigHostDefaults};
pub use merman_analysis::{RULE_CATALOG_RESPONSE_VERSION, RuleCatalogEntry, RuleCatalogResponse};
use merman_core::EditorRenamePolicy;
use merman_editor_core::{
    DocumentUri, EditorLocation, Position as CorePosition, Range as CoreRange,
    SEMANTIC_TOKEN_DESCRIPTOR_DIGEST, semantic_token_descriptor,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower_lsp_server::ls_types::{Location, Position, Range, Uri};

pub const EXPERIMENTAL_SCHEMA_VERSION: u32 = 1;
pub const CONFIG_SCHEMA_RESPONSE_VERSION: u32 = 1;
pub const RULE_CATALOG_METHOD: &str = "merman/ruleCatalog";
pub const CONFIG_SCHEMA_METHOD: &str = "merman/configSchema";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceEditEncoding {
    DocumentChanges,
    Changes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticIdentityData {
    pub(crate) id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) document_version: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticVersionData {
    pub(crate) document_version: i32,
}

impl WorkspaceEditEncoding {
    pub const fn from_document_changes_support(supported: bool) -> Self {
        if supported {
            Self::DocumentChanges
        } else {
            Self::Changes
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigSchemaResponse {
    pub version: u32,
    pub rule_catalog_method: String,
    pub accepted_roots: Vec<String>,
    pub profiles: Vec<String>,
    pub severities: Vec<String>,
    pub configurable_rule_ids: Vec<String>,
    pub schema: Value,
}

impl ConfigSchemaResponse {
    pub fn current() -> Self {
        let contract = AnalysisConfigContract::current().json_schema(AnalysisConfigHostDefaults {
            max_source_bytes: Some(DEFAULT_LSP_MAX_SOURCE_BYTES),
            max_document_diagrams: Some(DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS),
        });
        Self {
            version: CONFIG_SCHEMA_RESPONSE_VERSION,
            rule_catalog_method: RULE_CATALOG_METHOD.to_string(),
            accepted_roots: contract.accepted_roots,
            profiles: contract.profiles,
            severities: contract.severities,
            configurable_rule_ids: contract.configurable_rule_ids,
            schema: contract.schema,
        }
    }
}

pub fn experimental_capabilities() -> serde_json::Value {
    let editor_language = semantic_token_descriptor();
    let diagram_families = merman_core::diagram_family_capabilities()
        .iter()
        .map(|family| {
            json!({
                "diagramType": family.diagram_type,
                "semanticParser": family.has_semantic_parser,
                "renderParser": family.has_render_parser,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "merman": {
            "schemaVersion": EXPERIMENTAL_SCHEMA_VERSION,
            "editorLanguage": {
                "schemaVersion": editor_language.schema_version,
                "descriptorDigest": SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
                "packedEncoding": editor_language.packed.encoding,
                "wordsPerToken": editor_language.packed.words_per_token,
                "renamePolicies": EditorRenamePolicy::IDS,
            },
            "diagramSupport": {
                "families": diagram_families,
            },
            "requests": {
                "ruleCatalog": RULE_CATALOG_METHOD,
                "configSchema": CONFIG_SCHEMA_METHOD
            }
        }
    })
}

pub fn core_position_from_lsp(position: Position) -> CorePosition {
    CorePosition::new(position.line as usize, position.character as usize)
}

pub fn range_to_lsp(range: CoreRange) -> Range {
    Range::new(
        Position::new(range.start.line as u32, range.start.character as u32),
        Position::new(range.end.line as u32, range.end.character as u32),
    )
}

pub fn document_uri_to_lsp(uri: &DocumentUri, fallback_uri: &Uri) -> Uri {
    Uri::from_str(uri.as_str()).unwrap_or_else(|_| fallback_uri.clone())
}

pub fn location_to_lsp(location: EditorLocation, fallback_uri: &Uri) -> Location {
    let uri = document_uri_to_lsp(&location.uri, fallback_uri);
    Location::new(uri, range_to_lsp(location.range))
}

pub(crate) fn generated_markdown_to_plain_text(markdown: &str) -> String {
    let mut plain = String::with_capacity(markdown.len());
    for (index, line) in markdown.lines().enumerate() {
        if index > 0 {
            plain.push('\n');
        }
        let line = line.strip_prefix("### ").unwrap_or(line);
        let mut chars = line.chars();
        while let Some(character) = chars.next() {
            if character == '\\' {
                if let Some(next) = chars.next() {
                    plain.push(next);
                }
            } else if character != '`' {
                plain.push(character);
            }
        }
    }
    plain.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_projection_preserves_percent_encoding_and_non_file_schemes() {
        let fallback = Uri::from_str("file:///tmp/fallback.mmd").unwrap();

        for raw in ["file:///tmp/diagram%20draft.mmd", "untitled:notes%20draft"] {
            let projected = document_uri_to_lsp(&DocumentUri::from(raw), &fallback);

            assert_eq!(projected.as_str(), raw);
            assert_eq!(serde_json::to_value(projected).unwrap(), json!(raw));
        }
    }

    #[test]
    fn rule_catalog_response_contains_governed_authoring_rule() {
        let catalog = RuleCatalogResponse::current();

        assert_eq!(catalog.version, RULE_CATALOG_RESPONSE_VERSION);
        assert!(catalog.rules.iter().any(|rule| {
            rule.id == "merman.authoring.flowchart.explicit_direction"
                && rule.origin.as_str() == "merman_authoring"
                && rule.default_profile.as_str() == "recommended"
                && rule
                    .evidence
                    .contains(&"docs/adr/0072-lint-rule-governance.md")
                && rule.configurable
                && rule.fixable
        }));
        assert!(catalog.rules.iter().any(|rule| {
            rule.id == "merman.authoring.config.prefer_frontmatter_config"
                && rule.origin.as_str() == "merman_authoring"
                && rule.default_profile.as_str() == "recommended"
                && rule.default_severity.as_str() == "hint"
                && rule.category.as_str() == "config"
                && rule.evidence.contains(
                    &"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md",
                )
                && rule.configurable
                && rule.fixable
        }));
        assert!(catalog.rules.iter().any(|rule| {
            rule.id == "merman.compatibility.config.deprecated_flowchart_html_labels"
                && rule.origin.as_str() == "mermaid_compatibility"
                && rule.default_profile.as_str() == "core"
                && rule.default_enabled
                && rule.default_severity.as_str() == "warning"
                && rule.category.as_str() == "config"
                && rule.evidence.contains(
                    &"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md",
                )
                && rule.configurable
                && !rule.fixable
        }));
        assert!(catalog.rules.iter().any(|rule| {
            rule.id == "merman.compatibility.config.deprecated_external_diagram_loading"
                && rule.origin.as_str() == "mermaid_compatibility"
                && rule.default_profile.as_str() == "core"
                && rule.default_enabled
                && rule.default_severity.as_str() == "warning"
                && rule.category.as_str() == "config"
                && rule.evidence.contains(
                    &"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/config.ts",
                )
                && rule.configurable
                && !rule.fixable
        }));
    }

    #[test]
    fn experimental_capability_advertises_rule_catalog_request() {
        let capabilities = experimental_capabilities();
        let descriptor = semantic_token_descriptor();

        assert_eq!(EXPERIMENTAL_SCHEMA_VERSION, 1);
        assert_eq!(
            capabilities["merman"]["requests"]["ruleCatalog"],
            RULE_CATALOG_METHOD
        );
        assert_eq!(
            capabilities["merman"]["requests"]["configSchema"],
            CONFIG_SCHEMA_METHOD
        );
        assert_eq!(
            capabilities["merman"]["schemaVersion"],
            EXPERIMENTAL_SCHEMA_VERSION
        );
        assert_eq!(
            capabilities["merman"]["editorLanguage"],
            serde_json::json!({
                "schemaVersion": descriptor.schema_version,
                "descriptorDigest": descriptor.digest,
                "packedEncoding": descriptor.packed.encoding,
                "wordsPerToken": descriptor.packed.words_per_token,
                "renamePolicies": EditorRenamePolicy::IDS,
            })
        );
        let families = capabilities["merman"]["diagramSupport"]["families"]
            .as_array()
            .expect("diagram family capabilities");
        assert!(families.iter().any(|family| {
            family["diagramType"] == "gitGraph"
                && family["semanticParser"] == true
                && family["renderParser"] == true
        }));
        assert!(
            families
                .iter()
                .any(|family| family["diagramType"] == "mindmap")
        );
    }

    #[test]
    fn config_schema_response_describes_lint_settings() {
        let response = ConfigSchemaResponse::current();

        assert_eq!(response.version, CONFIG_SCHEMA_RESPONSE_VERSION);
        assert_eq!(response.rule_catalog_method, RULE_CATALOG_METHOD);
        assert_eq!(response.profiles, ["core", "recommended", "strict"]);
        assert_eq!(response.severities, ["error", "warning", "info", "hint"]);
        assert!(
            response
                .configurable_rule_ids
                .contains(&"merman.authoring.config.prefer_frontmatter_config".to_string())
        );
        assert!(
            response
                .configurable_rule_ids
                .contains(&"merman.authoring.flowchart.explicit_direction".to_string())
        );
        assert!(
            response.configurable_rule_ids.contains(
                &"merman.compatibility.config.deprecated_flowchart_html_labels".to_string()
            )
        );
        assert!(response.configurable_rule_ids.contains(
            &"merman.compatibility.config.deprecated_external_diagram_loading".to_string()
        ));
        assert_eq!(
            response.schema["$defs"]["analysisOptions"]["properties"]["lint"]["properties"]["profile"]
                ["enum"],
            json!([null, "core", "recommended", "strict"])
        );
        assert_eq!(
            response.schema["$defs"]["ruleId"]["enum"],
            json!(response.configurable_rule_ids)
        );
        assert_eq!(
            response.schema["$defs"]["severity"]["enum"],
            json!(["error", "warning", "info", "hint"])
        );
        assert_eq!(
            response.schema["$defs"]["analysisOptions"]["properties"]["resources"]["properties"]["limits"]
                ["properties"]["max_source_bytes"]["default"],
            json!(DEFAULT_LSP_MAX_SOURCE_BYTES)
        );
        assert_eq!(
            response.schema["$defs"]["analysisOptions"]["properties"]["resources"]["properties"]["limits"]
                ["properties"]["max_document_diagrams"]["default"],
            json!(DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS)
        );
        assert_eq!(
            response.schema["$defs"]["analysisOptions"]["properties"]["fixed_today"]["pattern"],
            json!(FIXED_TODAY_SCHEMA_PATTERN)
        );
        assert!(
            response.schema["$defs"]["analysisOptions"]["properties"]
                .get("parse")
                .is_none()
        );
        assert_eq!(
            response.schema["oneOf"][0]["allOf"][0],
            json!({ "$ref": "#/$defs/analysisOptions" })
        );
        assert_eq!(
            response.schema["oneOf"][1]["properties"]["merman"],
            json!({ "$ref": "#/$defs/analysisOptions" })
        );
        assert_eq!(
            response.schema["oneOf"][2]["properties"]["analysis"],
            json!({ "$ref": "#/$defs/analysisOptions" })
        );
    }

    #[test]
    fn vscode_analysis_settings_match_lsp_config_schema_keys() {
        let response = ConfigSchemaResponse::current();
        let mut schema_keys = std::collections::BTreeSet::new();
        collect_analysis_schema_leaf_keys(
            &response.schema["$defs"]["analysisOptions"],
            "",
            &mut schema_keys,
        );

        let package_json_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/vscode-extension/package.json");
        let package_json: Value = serde_json::from_str(
            &std::fs::read_to_string(package_json_path)
                .expect("expected VS Code package.json to be readable"),
        )
        .expect("expected VS Code package.json to parse as JSON");
        let mut vscode_keys = std::collections::BTreeSet::new();
        collect_vscode_analysis_setting_keys(&package_json, &mut vscode_keys);

        assert_eq!(vscode_keys, schema_keys);
        let expected_fixed_today_pattern = format!("^$|{FIXED_TODAY_SCHEMA_PATTERN}");
        assert_eq!(
            vscode_analysis_setting(&package_json, "merman.analysis.fixed_today")
                .and_then(|setting| setting["pattern"].as_str()),
            Some(expected_fixed_today_pattern.as_str())
        );

        let vscode_profiles =
            vscode_analysis_setting(&package_json, "merman.analysis.lint.profile")
                .and_then(|setting| setting["enum"].as_array())
                .expect("VS Code must publish lint profiles")
                .iter()
                .filter_map(Value::as_str)
                .filter(|profile| !profile.is_empty())
                .collect::<Vec<_>>();
        assert_eq!(vscode_profiles, response.profiles);

        let vscode_severities =
            vscode_analysis_setting(&package_json, "merman.analysis.lint.rule_severities")
                .and_then(|setting| setting["items"]["properties"]["severity"]["enum"].as_array())
                .expect("VS Code must publish diagnostic severities");
        assert_eq!(
            Value::Array(vscode_severities.clone()),
            json!(response.severities)
        );

        let analysis_options = &response.schema["$defs"]["analysisOptions"];
        let offset_setting =
            vscode_analysis_setting(&package_json, "merman.analysis.fixed_local_offset_minutes")
                .expect("VS Code offset setting");
        let offset_schema = &analysis_options["properties"]["fixed_local_offset_minutes"];
        assert_eq!(offset_setting["type"], offset_schema["type"]);
        assert_eq!(offset_setting["minimum"], offset_schema["minimum"]);
        assert_eq!(offset_setting["maximum"], offset_schema["maximum"]);

        for (setting_key, schema_path) in [
            (
                "merman.analysis.resources.limits.max_source_bytes",
                &analysis_options["properties"]["resources"]["properties"]["limits"]["properties"]
                    ["max_source_bytes"],
            ),
            (
                "merman.analysis.resources.limits.max_document_diagrams",
                &analysis_options["properties"]["resources"]["properties"]["limits"]["properties"]
                    ["max_document_diagrams"],
            ),
        ] {
            let setting = vscode_analysis_setting(&package_json, setting_key)
                .expect("VS Code resource setting");
            assert_eq!(setting["type"], json!(["integer", "null"]), "{setting_key}");
            assert_eq!(schema_path["type"], "integer", "{setting_key}");
            assert_eq!(setting["minimum"], schema_path["minimum"], "{setting_key}");
            assert_eq!(setting["maximum"], schema_path["maximum"], "{setting_key}");
            assert_eq!(setting["default"], Value::Null, "{setting_key}");
        }
        assert_eq!(
            vscode_analysis_setting(
                &package_json,
                "merman.analysis.resources.limits.max_document_diagrams",
            )
            .expect("VS Code document limit setting")["default"],
            Value::Null
        );
    }

    fn collect_analysis_schema_leaf_keys(
        schema: &Value,
        prefix: &str,
        keys: &mut std::collections::BTreeSet<String>,
    ) {
        let Some(properties) = schema["properties"].as_object() else {
            return;
        };

        for (key, value) in properties {
            let full_key = if prefix.is_empty() {
                key.to_string()
            } else {
                format!("{prefix}.{key}")
            };
            if value["properties"].is_object() {
                collect_analysis_schema_leaf_keys(value, &full_key, keys);
            } else {
                keys.insert(full_key);
            }
        }
    }

    fn collect_vscode_analysis_setting_keys(
        package_json: &Value,
        keys: &mut std::collections::BTreeSet<String>,
    ) {
        let Some(configuration) = package_json["contributes"]["configuration"].as_array() else {
            return;
        };

        for section in configuration {
            let Some(properties) = section["properties"].as_object() else {
                continue;
            };
            for key in properties.keys() {
                if let Some(analysis_key) = key.strip_prefix("merman.analysis.") {
                    keys.insert(analysis_key.to_string());
                }
            }
        }
    }

    fn vscode_analysis_setting<'a>(package_json: &'a Value, key: &str) -> Option<&'a Value> {
        package_json["contributes"]["configuration"]
            .as_array()?
            .iter()
            .find_map(|section| section["properties"].get(key))
    }
}
