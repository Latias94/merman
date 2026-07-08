use crate::{
    AnalysisOptions, AnalysisRuleConfig, AnalysisRuleProfile, DiagnosticSeverity,
    configurable_rule_descriptor,
};
use chrono::NaiveDate;
use merman_core::MermaidConfig;
use serde::{Deserialize, Serialize};
use serde_json::Map;
use serde_json::Value;
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisOptionsJson {
    pub fixed_today: Option<String>,
    pub fixed_local_offset_minutes: Option<i32>,
    pub site_config: Option<Value>,
    pub parse: Option<ParseOptionsJson>,
    pub resources: Option<ResourceOptionsJson>,
    pub lint: Option<LintOptionsJson>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseOptionsJson {
    pub suppress_errors: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceOptionsJson {
    pub profile: Option<String>,
    pub max_source_bytes: Option<usize>,
    pub max_svg_bytes: Option<usize>,
    pub max_flowchart_nodes: Option<usize>,
    pub max_flowchart_edges: Option<usize>,
    pub max_flowchart_subgraphs: Option<usize>,
    pub max_class_nodes: Option<usize>,
    pub max_class_edges: Option<usize>,
    pub max_class_namespaces: Option<usize>,
    pub max_label_bytes: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintOptionsJson {
    pub profile: Option<String>,
    #[serde(default)]
    pub enable_rules: Vec<String>,
    #[serde(default)]
    pub disable_rules: Vec<String>,
    #[serde(default)]
    pub rule_severities: Vec<LintRuleSeverityOverrideJson>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintRuleSeverityOverrideJson {
    pub rule_id: String,
    pub severity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisOptionsJsonError {
    message: String,
}

impl AnalysisOptionsJsonError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for AnalysisOptionsJsonError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for AnalysisOptionsJsonError {}

pub fn analysis_options_from_json_value(
    value: &Value,
) -> Result<AnalysisOptions, AnalysisOptionsJsonError> {
    analysis_options_json_from_json_value(value)?.to_analysis_options()
}

pub fn analysis_options_json_from_json_value(
    value: &Value,
) -> Result<AnalysisOptionsJson, AnalysisOptionsJsonError> {
    let options_value = analysis_options_root_value(value)?;
    serde_json::from_value(options_value.clone()).map_err(|err| {
        AnalysisOptionsJsonError::new(format!("invalid analysis options JSON: {err}"))
    })
}

fn analysis_options_root_value(value: &Value) -> Result<&Value, AnalysisOptionsJsonError> {
    let Value::Object(map) = value else {
        return Ok(value);
    };

    if analysis_option_keys_present(map) {
        if ["merman", "analysis"]
            .iter()
            .any(|key| map.get(*key).is_some_and(Value::is_object))
        {
            return Err(AnalysisOptionsJsonError::new(
                "options JSON must not mix top-level analysis options with `analysis` or `merman` wrappers",
            ));
        }
        return Ok(value);
    }

    let mut wrapped_keys = ["merman", "analysis"].into_iter().filter(|key| {
        map.get(*key)
            .and_then(Value::as_object)
            .is_some_and(analysis_option_keys_present)
    });
    if let Some(key) = wrapped_keys.next() {
        if wrapped_keys.next().is_some() {
            return Err(AnalysisOptionsJsonError::new(
                "options JSON must not contain both `merman` and `analysis` wrappers with analysis options",
            ));
        }
        return Ok(map
            .get(key)
            .expect("checked key existence and object shape"));
    }

    Ok(value)
}

fn analysis_option_keys_present(map: &Map<String, Value>) -> bool {
    [
        "fixed_today",
        "fixed_local_offset_minutes",
        "site_config",
        "parse",
        "resources",
        "lint",
    ]
    .iter()
    .any(|key| map.contains_key(*key))
}

impl AnalysisOptionsJson {
    pub fn to_analysis_options(&self) -> Result<AnalysisOptions, AnalysisOptionsJsonError> {
        let mut analysis = AnalysisOptions::default()
            .with_parse_options(self.parse_options())
            .with_max_source_bytes(self.max_source_bytes()?);

        if let Some(site_config) = self.site_config()? {
            analysis = analysis.with_site_config(site_config);
        }
        if let Some(today) = self.fixed_today()? {
            analysis = analysis.with_fixed_today(Some(today));
        }
        if let Some(offset_minutes) = self.fixed_local_offset_minutes()? {
            analysis = analysis.with_fixed_local_offset_minutes(Some(offset_minutes));
        }

        analysis = analysis.with_rule_config(self.rule_config()?);
        Ok(analysis)
    }

    pub fn parse_options(&self) -> merman_core::ParseOptions {
        if self
            .parse
            .as_ref()
            .and_then(|parse| parse.suppress_errors)
            .unwrap_or(false)
        {
            merman_core::ParseOptions::lenient()
        } else {
            merman_core::ParseOptions::strict()
        }
    }

    pub fn max_source_bytes(&self) -> Result<Option<usize>, AnalysisOptionsJsonError> {
        let max_source_bytes = self
            .resources
            .as_ref()
            .and_then(|resources| resources.max_source_bytes)
            .filter(|max_source_bytes| *max_source_bytes > 0);
        Ok(max_source_bytes)
    }

    pub fn rule_config(&self) -> Result<AnalysisRuleConfig, AnalysisOptionsJsonError> {
        let Some(lint) = self.lint.as_ref() else {
            return Ok(AnalysisRuleConfig::default());
        };

        let mut config = AnalysisRuleConfig::default();
        if let Some(profile) = lint.profile.as_deref() {
            config.set_profile(parse_lint_profile(profile)?);
        }

        for rule_id in &lint.enable_rules {
            if rule_id.trim().is_empty() {
                return Err(AnalysisOptionsJsonError::new(
                    "lint.enable_rules entries must not be empty",
                ));
            }
            validate_configurable_rule_id(rule_id, "lint.enable_rules")?;
            config.enable_rule(rule_id.clone());
        }

        for rule_id in &lint.disable_rules {
            if rule_id.trim().is_empty() {
                return Err(AnalysisOptionsJsonError::new(
                    "lint.disable_rules entries must not be empty",
                ));
            }
            validate_configurable_rule_id(rule_id, "lint.disable_rules")?;
            config.disable_rule(rule_id.clone());
        }

        for override_ in &lint.rule_severities {
            if override_.rule_id.trim().is_empty() {
                return Err(AnalysisOptionsJsonError::new(
                    "lint.rule_severities.rule_id must not be empty",
                ));
            }
            validate_configurable_rule_id(&override_.rule_id, "lint.rule_severities.rule_id")?;
            config.set_rule_severity(
                override_.rule_id.clone(),
                parse_lint_severity(&override_.severity)?,
            );
        }

        Ok(config)
    }

    pub fn fixed_today(&self) -> Result<Option<NaiveDate>, AnalysisOptionsJsonError> {
        let Some(today) = self.fixed_today.as_deref() else {
            return Ok(None);
        };
        NaiveDate::parse_from_str(today, "%Y-%m-%d")
            .map(Some)
            .map_err(|_| {
                AnalysisOptionsJsonError::new("fixed_today must be a date in YYYY-MM-DD format")
            })
    }

    pub fn fixed_local_offset_minutes(&self) -> Result<Option<i32>, AnalysisOptionsJsonError> {
        let Some(offset_minutes) = self.fixed_local_offset_minutes else {
            return Ok(None);
        };
        let valid = offset_minutes
            .checked_mul(60)
            .and_then(chrono::FixedOffset::east_opt)
            .is_some();
        if !valid {
            return Err(AnalysisOptionsJsonError::new(
                "fixed_local_offset_minutes must be between -1439 and 1439",
            ));
        }
        Ok(Some(offset_minutes))
    }

    pub fn site_config(&self) -> Result<Option<MermaidConfig>, AnalysisOptionsJsonError> {
        let Some(site_config) = self.site_config.as_ref() else {
            return Ok(None);
        };
        if !site_config.is_object() {
            return Err(AnalysisOptionsJsonError::new(
                "site_config must be a JSON object",
            ));
        }
        Ok(Some(MermaidConfig::from_value(site_config.clone())))
    }
}

fn parse_lint_profile(value: &str) -> Result<AnalysisRuleProfile, AnalysisOptionsJsonError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "core" => Ok(AnalysisRuleProfile::Core),
        "recommended" => Ok(AnalysisRuleProfile::Recommended),
        "strict" => Ok(AnalysisRuleProfile::Strict),
        _ => Err(AnalysisOptionsJsonError::new(
            "lint.profile must be core, recommended, or strict",
        )),
    }
}

fn parse_lint_severity(value: &str) -> Result<DiagnosticSeverity, AnalysisOptionsJsonError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" => Ok(DiagnosticSeverity::Error),
        "warning" | "warn" => Ok(DiagnosticSeverity::Warning),
        "info" => Ok(DiagnosticSeverity::Info),
        "hint" => Ok(DiagnosticSeverity::Hint),
        _ => Err(AnalysisOptionsJsonError::new(
            "lint.rule_severities.severity must be error, warning, info, or hint",
        )),
    }
}

fn validate_configurable_rule_id(
    rule_id: &str,
    field: &str,
) -> Result<(), AnalysisOptionsJsonError> {
    if configurable_rule_descriptor(rule_id).is_none() {
        return Err(AnalysisOptionsJsonError::new(format!(
            "{field} entry `{rule_id}` must reference a configurable analysis rule id",
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{rule_descriptors, rules::RESOURCE_LIMIT_RULE_ID};

    #[test]
    fn shared_analysis_options_json_honors_lint_configuration() {
        let options = AnalysisOptionsJson {
            lint: Some(LintOptionsJson {
                disable_rules: vec!["merman.git_graph.duplicate_commit_id".to_string()],
                rule_severities: vec![LintRuleSeverityOverrideJson {
                    rule_id: "merman.authoring.config.prefer_init_directive".to_string(),
                    severity: "hint".to_string(),
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let analysis = options.to_analysis_options().unwrap();
        let descriptors = rule_descriptors();
        let duplicate_commit = descriptors
            .iter()
            .find(|descriptor| descriptor.id == "merman.git_graph.duplicate_commit_id")
            .unwrap();
        let prefer_init = descriptors
            .iter()
            .find(|descriptor| descriptor.id == "merman.authoring.config.prefer_init_directive")
            .unwrap();
        let prefer_frontmatter = descriptors
            .iter()
            .find(|descriptor| descriptor.id == "merman.authoring.config.prefer_frontmatter_config")
            .unwrap();

        assert_eq!(analysis.rule_config.profile(), AnalysisRuleProfile::Core);
        assert!(!analysis.rule_config.is_rule_enabled(*duplicate_commit));
        assert!(!analysis.rule_config.is_rule_enabled(*prefer_init));
        assert!(!analysis.rule_config.is_rule_enabled(*prefer_frontmatter));
        assert_eq!(
            analysis.rule_config.severity_for(*prefer_init),
            DiagnosticSeverity::Hint
        );
    }

    #[test]
    fn shared_analysis_options_json_accepts_lint_profiles_and_explicit_enablement() {
        let wrapped = serde_json::json!({
            "lint": {
                "profile": "recommended"
            }
        });
        let analysis = analysis_options_from_json_value(&wrapped).unwrap();
        let prefer_init = rule_descriptors()
            .iter()
            .find(|descriptor| descriptor.id == "merman.authoring.config.prefer_init_directive")
            .unwrap();
        let prefer_frontmatter = rule_descriptors()
            .iter()
            .find(|descriptor| descriptor.id == "merman.authoring.config.prefer_frontmatter_config")
            .unwrap();

        assert_eq!(
            analysis.rule_config.profile(),
            AnalysisRuleProfile::Recommended
        );
        assert!(analysis.rule_config.is_rule_enabled(*prefer_init));
        assert!(analysis.rule_config.is_rule_enabled(*prefer_frontmatter));

        let wrapped = serde_json::json!({
            "lint": {
                "enable_rules": [
                    "merman.authoring.config.prefer_init_directive",
                    "merman.authoring.config.prefer_frontmatter_config"
                ]
            }
        });
        let analysis = analysis_options_from_json_value(&wrapped).unwrap();

        assert_eq!(analysis.rule_config.profile(), AnalysisRuleProfile::Core);
        assert!(analysis.rule_config.is_rule_enabled(*prefer_init));
        assert!(analysis.rule_config.is_rule_enabled(*prefer_frontmatter));
    }

    #[test]
    fn shared_analysis_options_json_rejects_unknown_lint_rule_ids() {
        let options = AnalysisOptionsJson {
            lint: Some(LintOptionsJson {
                disable_rules: vec!["merman.unknown.rule".to_string()],
                ..Default::default()
            }),
            ..Default::default()
        };

        let err = options.to_analysis_options().unwrap_err();
        assert!(
            err.to_string().contains("configurable analysis rule id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn shared_analysis_options_json_rejects_external_lint_rule_ids() {
        let cases = [
            serde_json::json!({
                "lint": {
                    "enable_rules": ["require-direction"]
                }
            }),
            serde_json::json!({
                "lint": {
                    "disable_rules": ["mermaid-lint/no-empty-labels"]
                }
            }),
            serde_json::json!({
                "lint": {
                    "rule_severities": [
                        {
                            "rule_id": "duplicate-ids",
                            "severity": "warning"
                        }
                    ]
                }
            }),
        ];

        for options in cases {
            let err = analysis_options_from_json_value(&options).unwrap_err();
            assert!(
                err.to_string().contains("configurable analysis rule id"),
                "unexpected error for {options}: {err}"
            );
        }
    }

    #[test]
    fn shared_analysis_options_json_rejects_internal_lint_rule_ids() {
        let wrapped = serde_json::json!({
            "lint": {
                "rule_severities": [
                    {
                        "rule_id": "merman.internal.panic",
                        "severity": "warning"
                    }
                ]
            }
        });

        let err = analysis_options_from_json_value(&wrapped).unwrap_err();
        assert!(
            err.to_string().contains("configurable analysis rule id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn shared_analysis_options_json_rejects_resource_lint_rule_ids() {
        let cases = [
            serde_json::json!({
                "lint": {
                    "enable_rules": [RESOURCE_LIMIT_RULE_ID]
                }
            }),
            serde_json::json!({
                "lint": {
                    "disable_rules": [RESOURCE_LIMIT_RULE_ID]
                }
            }),
            serde_json::json!({
                "lint": {
                    "rule_severities": [
                        {
                            "rule_id": RESOURCE_LIMIT_RULE_ID,
                            "severity": "hint"
                        }
                    ]
                }
            }),
        ];

        for options in cases {
            let err = analysis_options_from_json_value(&options).unwrap_err();
            assert!(
                err.to_string().contains("configurable analysis rule id"),
                "unexpected error for {options}: {err}"
            );
        }
    }

    #[test]
    fn shared_analysis_options_json_accepts_namespaced_wrappers() {
        let wrapped = serde_json::json!({
            "merman": {
                "lint": {
                    "disable_rules": ["merman.git_graph.duplicate_commit_id"]
                }
            }
        });
        let analysis = analysis_options_from_json_value(&wrapped).unwrap();
        let duplicate_commit = rule_descriptors()
            .iter()
            .find(|descriptor| descriptor.id == "merman.git_graph.duplicate_commit_id")
            .unwrap();

        assert!(!analysis.rule_config.is_rule_enabled(*duplicate_commit));

        let wrapped = serde_json::json!({
            "analysis": {
                "lint": {
                    "profile": "recommended",
                    "rule_severities": [
                        {
                            "rule_id": "merman.authoring.config.prefer_init_directive",
                            "severity": "warning"
                        }
                    ]
                }
            }
        });
        let analysis = analysis_options_from_json_value(&wrapped).unwrap();
        let prefer_init = rule_descriptors()
            .iter()
            .find(|descriptor| descriptor.id == "merman.authoring.config.prefer_init_directive")
            .unwrap();
        let prefer_frontmatter = rule_descriptors()
            .iter()
            .find(|descriptor| descriptor.id == "merman.authoring.config.prefer_frontmatter_config")
            .unwrap();

        assert_eq!(
            analysis.rule_config.profile(),
            AnalysisRuleProfile::Recommended
        );
        assert_eq!(
            analysis.rule_config.severity_for(*prefer_init),
            DiagnosticSeverity::Warning
        );
        assert!(analysis.rule_config.is_rule_enabled(*prefer_init));
        assert!(analysis.rule_config.is_rule_enabled(*prefer_frontmatter));
    }

    #[test]
    fn shared_analysis_options_json_treats_zero_source_limit_as_default() {
        let zero = serde_json::json!({
            "analysis": {
                "resources": {
                    "max_source_bytes": 0
                }
            }
        });
        let positive = serde_json::json!({
            "analysis": {
                "resources": {
                    "max_source_bytes": 1024
                }
            }
        });

        assert_eq!(
            analysis_options_from_json_value(&zero)
                .unwrap()
                .max_source_bytes,
            None
        );
        assert_eq!(
            analysis_options_from_json_value(&positive)
                .unwrap()
                .max_source_bytes,
            Some(1024)
        );
    }

    #[test]
    fn shared_analysis_options_json_rejects_two_namespaced_analysis_wrappers() {
        let mixed = serde_json::json!({
            "merman": {
                "lint": {
                    "profile": "recommended"
                }
            },
            "analysis": {
                "resources": {
                    "max_source_bytes": 1024
                }
            }
        });

        let err = analysis_options_from_json_value(&mixed).unwrap_err();

        assert!(
            err.to_string()
                .contains("must not contain both `merman` and `analysis` wrappers"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn shared_analysis_options_json_rejects_mixed_direct_and_namespaced_options() {
        let mixed = serde_json::json!({
            "resources": {
                "max_source_bytes": 1024
            },
            "analysis": {
                "lint": {
                    "profile": "recommended"
                }
            }
        });

        let err = analysis_options_from_json_value(&mixed).unwrap_err();

        assert!(
            err.to_string()
                .contains("must not mix top-level analysis options with `analysis` or `merman`"),
            "unexpected error: {err}"
        );
    }
}
