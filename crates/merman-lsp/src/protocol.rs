use merman_analysis::{AnalysisRuleProfile, RuleCatalogEntry, RuleOrigin};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const EXPERIMENTAL_SCHEMA_VERSION: u32 = 1;
pub const RULE_CATALOG_RESPONSE_VERSION: u32 = 1;
pub const RULE_CATALOG_METHOD: &str = "merman/ruleCatalog";

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

pub fn experimental_capabilities() -> serde_json::Value {
    json!({
        "merman": {
            "schemaVersion": EXPERIMENTAL_SCHEMA_VERSION,
            "requests": {
                "ruleCatalog": RULE_CATALOG_METHOD
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
    }

    #[test]
    fn experimental_capability_advertises_rule_catalog_request() {
        let capabilities = experimental_capabilities();

        assert_eq!(
            capabilities["merman"]["requests"]["ruleCatalog"],
            RULE_CATALOG_METHOD
        );
        assert_eq!(
            capabilities["merman"]["schemaVersion"],
            EXPERIMENTAL_SCHEMA_VERSION
        );
    }
}
