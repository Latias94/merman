use merman_analysis::{AnalysisRuleProfile, DiagnosticSeverity, RuleCatalogEntry, RuleOrigin};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const EXPERIMENTAL_SCHEMA_VERSION: u32 = 1;
pub const RULE_CATALOG_RESPONSE_VERSION: u32 = 1;
pub const CONFIG_SCHEMA_RESPONSE_VERSION: u32 = 1;
pub const RULE_CATALOG_METHOD: &str = "merman/ruleCatalog";
pub const CONFIG_SCHEMA_METHOD: &str = "merman/configSchema";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleCatalogResponse {
    pub version: u32,
    pub rules: Vec<LspRuleCatalogEntry>,
}

impl RuleCatalogResponse {
    pub fn current() -> Self {
        Self {
            version: RULE_CATALOG_RESPONSE_VERSION,
            rules: merman_analysis::rule_catalog()
                .into_iter()
                .map(LspRuleCatalogEntry::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRuleCatalogEntry {
    pub id: String,
    pub description: String,
    pub evidence: Vec<String>,
    pub default_severity: String,
    pub category: String,
    pub default_enabled: bool,
    pub default_profile: String,
    pub origin: String,
    pub configurable: bool,
    pub fixable: bool,
}

impl From<RuleCatalogEntry> for LspRuleCatalogEntry {
    fn from(rule: RuleCatalogEntry) -> Self {
        Self {
            id: rule.id.to_string(),
            description: rule.description.to_string(),
            evidence: rule
                .evidence
                .iter()
                .map(|evidence| evidence.to_string())
                .collect(),
            default_severity: rule.default_severity.as_str().to_string(),
            category: rule.category.as_str().to_string(),
            default_enabled: rule.default_enabled,
            default_profile: profile_name(rule.default_profile).to_string(),
            origin: origin_name(rule.origin).to_string(),
            configurable: rule.configurable,
            fixable: rule.fixable,
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
        let profiles = lint_profiles();
        let severities = lint_severities();
        let configurable_rule_ids = configurable_rule_ids();
        Self {
            version: CONFIG_SCHEMA_RESPONSE_VERSION,
            rule_catalog_method: RULE_CATALOG_METHOD.to_string(),
            accepted_roots: vec![
                "direct".to_string(),
                "merman".to_string(),
                "analysis".to_string(),
            ],
            schema: analysis_options_schema(&profiles, &severities, &configurable_rule_ids),
            profiles,
            severities,
            configurable_rule_ids,
        }
    }
}

pub fn experimental_capabilities() -> serde_json::Value {
    json!({
        "merman": {
            "schemaVersion": EXPERIMENTAL_SCHEMA_VERSION,
            "requests": {
                "ruleCatalog": RULE_CATALOG_METHOD,
                "configSchema": CONFIG_SCHEMA_METHOD
            }
        }
    })
}

fn profile_name(profile: AnalysisRuleProfile) -> &'static str {
    profile.as_str()
}

fn origin_name(origin: RuleOrigin) -> &'static str {
    origin.as_str()
}

fn severity_name(severity: DiagnosticSeverity) -> &'static str {
    severity.as_str()
}

fn lint_profiles() -> Vec<String> {
    [
        AnalysisRuleProfile::Core,
        AnalysisRuleProfile::Recommended,
        AnalysisRuleProfile::Strict,
    ]
    .into_iter()
    .map(profile_name)
    .map(str::to_string)
    .collect()
}

fn lint_severities() -> Vec<String> {
    [
        DiagnosticSeverity::Error,
        DiagnosticSeverity::Warning,
        DiagnosticSeverity::Info,
        DiagnosticSeverity::Hint,
    ]
    .into_iter()
    .map(severity_name)
    .map(str::to_string)
    .collect()
}

fn configurable_rule_ids() -> Vec<String> {
    merman_analysis::configurable_rule_catalog()
        .into_iter()
        .map(|rule| rule.id.to_string())
        .collect()
}

fn analysis_options_schema(
    profiles: &[String],
    severities: &[String],
    configurable_rule_ids: &[String],
) -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Merman analysis options",
        "description": "Options accepted by Merman LSP initializationOptions and workspace/didChangeConfiguration. Clients may pass these options directly, or under a merman or analysis object.",
        "$defs": {
            "ruleId": {
                "type": "string",
                "enum": configurable_rule_ids,
                "description": "A configurable Merman analysis rule id."
            },
            "severity": {
                "type": "string",
                "enum": severities,
                "description": "Diagnostic severity for an explicit rule override."
            },
            "analysisOptions": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "fixed_today": {
                        "type": "string",
                        "pattern": "^\\d{4}-\\d{2}-\\d{2}$",
                        "description": "Fixed local date used by time-sensitive analysis in YYYY-MM-DD format."
                    },
                    "fixed_local_offset_minutes": {
                        "type": "integer",
                        "minimum": -1439,
                        "maximum": 1439,
                        "description": "Fixed local UTC offset in minutes."
                    },
                    "site_config": {
                        "type": "object",
                        "additionalProperties": true,
                        "description": "Mermaid site configuration forwarded to the shared parser/config layer."
                    },
                    "parse": {
                        "type": "object",
                        "additionalProperties": true,
                        "properties": {
                            "suppress_errors": {
                                "type": "boolean",
                                "default": false,
                                "description": "Parse leniently when true."
                            }
                        }
                    },
                    "resources": {
                        "type": "object",
                        "additionalProperties": true,
                        "properties": {
                            "max_source_bytes": {
                                "type": "integer",
                                "minimum": 0,
                                "description": "Maximum source bytes accepted by analysis before a resource diagnostic is emitted."
                            }
                        }
                    },
                    "lint": {
                        "type": "object",
                        "additionalProperties": true,
                        "properties": {
                            "profile": {
                                "type": "string",
                                "enum": profiles,
                                "default": "core",
                                "description": "Base lint profile. Recommended and strict may enable additional governed authoring rules."
                            },
                            "enable_rules": {
                                "type": "array",
                                "items": { "$ref": "#/$defs/ruleId" },
                                "uniqueItems": true,
                                "description": "Configurable rule ids to enable explicitly."
                            },
                            "disable_rules": {
                                "type": "array",
                                "items": { "$ref": "#/$defs/ruleId" },
                                "uniqueItems": true,
                                "description": "Configurable rule ids to disable explicitly."
                            },
                            "rule_severities": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "required": ["rule_id", "severity"],
                                    "additionalProperties": true,
                                    "properties": {
                                        "rule_id": { "$ref": "#/$defs/ruleId" },
                                        "severity": { "$ref": "#/$defs/severity" }
                                    }
                                },
                                "description": "Per-rule diagnostic severity overrides."
                            }
                        }
                    }
                }
            }
        },
        "allOf": [
            { "$ref": "#/$defs/analysisOptions" }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_catalog_response_contains_governed_authoring_rule() {
        let catalog = RuleCatalogResponse::current();

        assert_eq!(catalog.version, RULE_CATALOG_RESPONSE_VERSION);
        assert!(catalog.rules.iter().any(|rule| {
            rule.id == "merman.authoring.flowchart.explicit_direction"
                && rule.origin == "merman_authoring"
                && rule.default_profile == "recommended"
                && rule
                    .evidence
                    .contains(&"docs/adr/0072-lint-rule-governance.md".to_string())
                && rule.configurable
                && rule.fixable
        }));
        assert!(catalog.rules.iter().any(|rule| {
            rule.id == "merman.authoring.config.prefer_frontmatter_config"
                && rule.origin == "merman_authoring"
                && rule.default_profile == "recommended"
                && rule.default_severity == "hint"
                && rule.category == "config"
                && rule.evidence.contains(
                    &"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md".to_string(),
                )
                && rule.configurable
                && rule.fixable
        }));
        assert!(catalog.rules.iter().any(|rule| {
            rule.id == "merman.compatibility.config.deprecated_flowchart_html_labels"
                && rule.origin == "mermaid_compatibility"
                && rule.default_profile == "core"
                && rule.default_enabled
                && rule.default_severity == "warning"
                && rule.category == "config"
                && rule.evidence.contains(
                    &"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md".to_string(),
                )
                && rule.configurable
                && !rule.fixable
        }));
        assert!(catalog.rules.iter().any(|rule| {
            rule.id == "merman.compatibility.config.deprecated_external_diagram_loading"
                && rule.origin == "mermaid_compatibility"
                && rule.default_profile == "core"
                && rule.default_enabled
                && rule.default_severity == "warning"
                && rule.category == "config"
                && rule.evidence.contains(
                    &"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/config.ts".to_string(),
                )
                && rule.configurable
                && !rule.fixable
        }));
    }

    #[test]
    fn experimental_capability_advertises_rule_catalog_request() {
        let capabilities = experimental_capabilities();

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
            json!(["core", "recommended", "strict"])
        );
        assert_eq!(
            response.schema["$defs"]["ruleId"]["enum"],
            json!(response.configurable_rule_ids)
        );
        assert_eq!(
            response.schema["$defs"]["severity"]["enum"],
            json!(["error", "warning", "info", "hint"])
        );
    }
}
