use std::str::FromStr;

use crate::session::{DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS, DEFAULT_LSP_MAX_SOURCE_BYTES};
pub use merman_analysis::FIXED_TODAY_SCHEMA_PATTERN;
use merman_analysis::{
    AnalysisConfigClientConstraints, AnalysisConfigContract, AnalysisConfigHostDefaults,
};
pub use merman_analysis::{RULE_CATALOG_RESPONSE_VERSION, RuleCatalogEntry, RuleCatalogResponse};
use merman_core::EditorRenamePolicy;
use merman_editor_core::{
    DocumentUri, EditorLocation, Position as CorePosition, Range as CoreRange,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower_lsp_server::ls_types::{Location, Position, Range, Uri};

pub const EXPERIMENTAL_SCHEMA_VERSION: u32 = 2;
pub const CONFIG_SCHEMA_RESPONSE_VERSION: u32 = 2;
pub const RULE_CATALOG_METHOD: &str = "merman/ruleCatalog";
pub const CONFIG_SCHEMA_METHOD: &str = "merman/configSchema";

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigSchemaResponse {
    pub version: u32,
    pub rule_catalog_method: String,
    pub accepted_roots: Vec<String>,
    pub profiles: Vec<String>,
    pub severities: Vec<String>,
    pub configurable_rule_ids: Vec<String>,
    pub constraints: AnalysisConfigClientConstraints,
    pub schema: Value,
}

impl ConfigSchemaResponse {
    pub fn current() -> Self {
        let host_defaults = AnalysisConfigHostDefaults::try_new(
            Some(DEFAULT_LSP_MAX_SOURCE_BYTES),
            Some(DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS),
        )
        .expect("LSP resource defaults must satisfy the analysis contract");
        let contract = AnalysisConfigContract::current();
        let schema = contract.json_schema(host_defaults);
        let client = contract.client_projection();
        Self {
            version: CONFIG_SCHEMA_RESPONSE_VERSION,
            rule_catalog_method: RULE_CATALOG_METHOD.to_string(),
            accepted_roots: client.accepted_roots,
            profiles: client.profiles,
            severities: client.severities,
            configurable_rule_ids: client.configurable_rule_ids,
            constraints: client.constraints,
            schema: schema.schema,
        }
    }
}

pub fn experimental_capabilities() -> serde_json::Value {
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

        assert_eq!(EXPERIMENTAL_SCHEMA_VERSION, 2);
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
    fn config_schema_response_projects_the_analysis_contract_over_the_transport_seam() {
        let response = ConfigSchemaResponse::current();

        assert_eq!(response.version, CONFIG_SCHEMA_RESPONSE_VERSION);
        assert_eq!(response.rule_catalog_method, RULE_CATALOG_METHOD);
        assert_eq!(response.accepted_roots, ["direct", "merman", "analysis"]);
        assert_eq!(response.constraints.settings.len(), 9);
        assert!(!response.profiles.is_empty());
        assert!(!response.severities.is_empty());
        assert!(!response.configurable_rule_ids.is_empty());
        assert!(response.schema.is_object());

        let wire = serde_json::to_value(&response).unwrap();
        assert_eq!(wire["version"], CONFIG_SCHEMA_RESPONSE_VERSION);
        assert_eq!(wire["rule_catalog_method"], RULE_CATALOG_METHOD);
        assert!(wire["constraints"].is_object());
        assert!(wire["schema"].is_object());
    }
}
