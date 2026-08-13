use crate::{
    AnalysisConfigContract, AnalysisOptions, AnalysisRuleConfig, AnalysisRuleProfile,
    DiagnosticSeverity,
};
use merman_core::{
    MermaidConfig,
    time::{CivilDate, UtcOffset},
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// The forward-compatible root JSON shape for analysis configuration.
///
/// Unknown fields at this root and inside `lint` are ignored so configuration transports can add
/// fields without breaking older readers. The versioned `resources` object remains strict. Direct
/// deserialization of the public nested types is also strict.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AnalysisOptionsJson {
    pub fixed_today: Option<String>,
    pub fixed_local_offset_minutes: Option<i32>,
    pub site_config: Option<Value>,
    pub resources: Option<ResourceOptionsJson>,
    pub lint: Option<LintOptionsJson>,
}

/// Strict resource-limit JSON for callers validating the versioned nested schema directly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ResourceOptionsJson {
    pub limits: BTreeMap<String, usize>,
}

/// Strict lint JSON for direct nested-schema validation.
///
/// Decode [`AnalysisOptionsJson`] when forward compatibility with future lint fields is required.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LintOptionsJson {
    pub profile: Option<String>,
    #[serde(default)]
    pub enable_rules: Vec<String>,
    #[serde(default)]
    pub disable_rules: Vec<String>,
    #[serde(default)]
    pub rule_severities: Vec<LintRuleSeverityOverrideJson>,
}

/// One strict rule-severity override in the public nested lint schema.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LintRuleSeverityOverrideJson {
    pub rule_id: String,
    pub severity: String,
}

impl<'de> Deserialize<'de> for AnalysisOptionsJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        AnalysisConfigContract::current()
            .decode_json(&value)
            .map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for ResourceOptionsJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        crate::config_contract::decode_resource_options(&value).map_err(serde::de::Error::custom)
    }
}

/// An invalid analysis-options JSON shape or value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisOptionsJsonError {
    message: String,
}

impl AnalysisOptionsJsonError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
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

/// Decodes the shared root JSON shape and converts it to validated analysis options.
///
/// This accepts direct options or the supported `analysis`/`merman` wrapper forms. Unknown root
/// and lint fields are ignored; strict nested resource validation and removed-option checks still
/// apply.
pub fn analysis_options_from_json_value(
    value: &Value,
) -> Result<AnalysisOptions, AnalysisOptionsJsonError> {
    AnalysisConfigContract::current().decode(value)
}

/// Decodes the forward-compatible shared root JSON shape without converting it.
///
/// Callers that intentionally validate one nested value should deserialize the corresponding
/// public nested type directly, which rejects unknown fields.
pub fn analysis_options_json_from_json_value(
    value: &Value,
) -> Result<AnalysisOptionsJson, AnalysisOptionsJsonError> {
    AnalysisConfigContract::current().decode_json(value)
}

impl AnalysisOptionsJson {
    pub fn to_analysis_options(&self) -> Result<AnalysisOptions, AnalysisOptionsJsonError> {
        let today = self.fixed_today()?;
        let offset_minutes = self.fixed_local_offset_minutes()?;
        let limits = self.validated_resource_limits()?;
        let mut analysis = AnalysisOptions::default()
            .with_max_source_bytes(Self::resource_limit_from(
                limits,
                merman_core::resources::InputResourceLimitId::MaxSourceBytes.as_str(),
            ))
            .with_max_document_diagrams(Self::resource_limit_from(
                limits,
                crate::MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID,
            ));

        if let Some(site_config) = self.site_config()? {
            analysis = analysis.with_site_config(site_config);
        }
        if let Some(offset_minutes) = offset_minutes {
            analysis = analysis
                .try_with_fixed_local_offset_minutes(offset_minutes)
                .map_err(|error| AnalysisOptionsJsonError::new(error.to_string()))?;
        }
        if let Some(today) = today {
            analysis = analysis
                .try_with_fixed_today_at_local_midnight(today)
                .map_err(|error| AnalysisOptionsJsonError::new(error.to_string()))?;
        }

        analysis = analysis.with_rule_config(self.rule_config()?);
        Ok(analysis)
    }

    pub fn max_source_bytes(&self) -> Result<Option<usize>, AnalysisOptionsJsonError> {
        Ok(Self::resource_limit_from(
            self.validated_resource_limits()?,
            merman_core::resources::InputResourceLimitId::MaxSourceBytes.as_str(),
        ))
    }

    pub fn max_document_diagrams(&self) -> Result<Option<usize>, AnalysisOptionsJsonError> {
        Ok(Self::resource_limit_from(
            self.validated_resource_limits()?,
            crate::MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID,
        ))
    }

    fn validated_resource_limits(
        &self,
    ) -> Result<Option<&BTreeMap<String, usize>>, AnalysisOptionsJsonError> {
        let Some(resources) = self.resources.as_ref() else {
            return Ok(None);
        };
        crate::config_contract::validate_resource_limit_values(&resources.limits)?;
        Ok(Some(&resources.limits))
    }

    fn resource_limit_from(
        limits: Option<&BTreeMap<String, usize>>,
        limit_id: &str,
    ) -> Option<usize> {
        limits.and_then(|limits| limits.get(limit_id).copied())
    }

    pub fn rule_config(&self) -> Result<AnalysisRuleConfig, AnalysisOptionsJsonError> {
        let mut config = AnalysisRuleConfig::default()
            .with_profile(crate::config_contract::default_lint_profile());
        let Some(lint) = self.lint.as_ref() else {
            return Ok(config);
        };
        if let Some(profile) = lint.profile.as_deref() {
            config.set_profile(parse_lint_profile(profile)?);
        }

        for rule_id in &lint.enable_rules {
            if rule_id.trim().is_empty() {
                return Err(AnalysisOptionsJsonError::new(
                    "lint.enable_rules entries must not be empty",
                ));
            }
            config
                .enable_rule(rule_id.clone())
                .map_err(|error| rule_config_error("lint.enable_rules", error))?;
        }

        for rule_id in &lint.disable_rules {
            if rule_id.trim().is_empty() {
                return Err(AnalysisOptionsJsonError::new(
                    "lint.disable_rules entries must not be empty",
                ));
            }
            config
                .disable_rule(rule_id.clone())
                .map_err(|error| rule_config_error("lint.disable_rules", error))?;
        }

        for override_ in &lint.rule_severities {
            if override_.rule_id.trim().is_empty() {
                return Err(AnalysisOptionsJsonError::new(
                    "lint.rule_severities.rule_id must not be empty",
                ));
            }
            config
                .set_rule_severity(
                    override_.rule_id.clone(),
                    parse_lint_severity(&override_.severity)?,
                )
                .map_err(|error| rule_config_error("lint.rule_severities.rule_id", error))?;
        }

        Ok(config)
    }

    pub fn fixed_today(&self) -> Result<Option<CivilDate>, AnalysisOptionsJsonError> {
        let Some(today) = self.fixed_today.as_deref() else {
            return Ok(None);
        };
        CivilDate::from_str(today).map(Some).map_err(|_| {
            AnalysisOptionsJsonError::new(
                "fixed_today must be a canonical civil date such as YYYY-MM-DD or +10000-MM-DD",
            )
        })
    }

    pub fn fixed_local_offset_minutes(&self) -> Result<Option<i32>, AnalysisOptionsJsonError> {
        let Some(offset_minutes) = self.fixed_local_offset_minutes else {
            return Ok(None);
        };
        let valid = UtcOffset::from_minutes(offset_minutes).is_some();
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
    AnalysisRuleProfile::from_config_str(value).ok_or_else(|| {
        AnalysisOptionsJsonError::new(format!(
            "lint.profile {}",
            crate::config_contract::lint_profile_requirement()
        ))
    })
}

fn parse_lint_severity(value: &str) -> Result<DiagnosticSeverity, AnalysisOptionsJsonError> {
    DiagnosticSeverity::from_config_str(value).ok_or_else(|| {
        AnalysisOptionsJsonError::new(format!(
            "lint.rule_severities.severity {}",
            crate::config_contract::diagnostic_severity_requirement()
        ))
    })
}

fn rule_config_error(
    field: &str,
    error: crate::AnalysisRuleConfigError,
) -> AnalysisOptionsJsonError {
    AnalysisOptionsJsonError::new(format!(
        "{field} entry `{}` must reference a configurable analysis rule id",
        error.rule_id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{rule_descriptors, rules::RESOURCE_LIMIT_RULE_ID};

    #[test]
    fn shared_analysis_options_json_keeps_utc_without_an_explicit_timezone() {
        let options = AnalysisOptionsJson {
            fixed_today: Some("2026-01-15".to_string()),
            ..Default::default()
        };

        let analysis = options.to_analysis_options().unwrap();
        let context = analysis.runtime_policy().begin_operation().unwrap();

        assert_eq!(context.today_local(), CivilDate::new(2026, 1, 15).unwrap());
        assert_eq!(context.local_time_zone().fixed_offset_minutes(), Some(0));
        assert_eq!(
            context.clock_source(),
            merman_core::runtime::RuntimeValueSource::Fixed
        );
    }

    #[test]
    fn shared_analysis_options_json_accepts_mermaid_wide_dates() {
        let options = AnalysisOptionsJson {
            fixed_today: Some("+10000-01-01".to_string()),
            ..Default::default()
        };

        assert_eq!(options.fixed_today().unwrap(), CivilDate::new(10_000, 1, 1));
    }

    #[test]
    fn shared_analysis_options_json_rejects_unrepresentable_fixed_local_midnight() {
        let options = AnalysisOptionsJson {
            fixed_today: Some("-2147483648-01-01".to_string()),
            fixed_local_offset_minutes: Some(1439),
            ..Default::default()
        };

        let error = options
            .to_analysis_options()
            .expect_err("boundary local midnight must return an error");
        assert!(error.to_string().contains("fixed_today local datetime"));
    }

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

        assert_eq!(analysis.rule_config().profile(), AnalysisRuleProfile::Core);
        assert!(!analysis.rule_config().is_rule_enabled(*duplicate_commit));
        assert!(!analysis.rule_config().is_rule_enabled(*prefer_init));
        assert!(!analysis.rule_config().is_rule_enabled(*prefer_frontmatter));
        assert_eq!(
            analysis.rule_config().severity_for(*prefer_init),
            DiagnosticSeverity::Hint
        );
    }

    #[test]
    fn shared_analysis_options_json_ignores_future_lint_fields() {
        let options = serde_json::json!({
            "lint": {
                "profile": "recommended",
                "future_lint_option": { "enabled": true },
                "rule_severities": [
                    {
                        "rule_id": "merman.parse.no_diagram",
                        "severity": "hint",
                        "future_override_option": "accepted"
                    }
                ]
            }
        });

        let analysis = analysis_options_from_json_value(&options).unwrap();
        let no_diagram = rule_descriptors()
            .iter()
            .find(|descriptor| descriptor.id == "merman.parse.no_diagram")
            .unwrap();

        assert_eq!(
            analysis.rule_config().profile(),
            AnalysisRuleProfile::Recommended
        );
        assert_eq!(
            analysis.rule_config().severity_for(*no_diagram),
            DiagnosticSeverity::Hint
        );
    }

    #[test]
    fn public_lint_json_types_reject_unknown_fields() {
        let lint_error = serde_json::from_value::<LintOptionsJson>(serde_json::json!({
            "profile": "core",
            "future_lint_option": true
        }))
        .expect_err("the public lint schema must reject unknown fields");
        assert!(
            lint_error
                .to_string()
                .contains("unknown field `future_lint_option`")
        );
        let override_error =
            serde_json::from_value::<LintRuleSeverityOverrideJson>(serde_json::json!({
                "rule_id": "merman.parse.no_diagram",
                "severity": "hint",
                "future_override_option": true
            }))
            .expect_err("the public severity override schema must reject unknown fields");
        assert!(
            override_error
                .to_string()
                .contains("unknown field `future_override_option`")
        );
    }

    #[test]
    fn shared_analysis_options_json_keeps_resource_schema_strict() {
        let error = analysis_options_json_from_json_value(&serde_json::json!({
            "resources": {
                "limits": {},
                "future_resource_option": true
            }
        }))
        .expect_err("resource options remain a versioned strict schema");

        assert!(
            error
                .to_string()
                .contains("unknown field `future_resource_option`")
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
            analysis.rule_config().profile(),
            AnalysisRuleProfile::Recommended
        );
        assert!(analysis.rule_config().is_rule_enabled(*prefer_init));
        assert!(analysis.rule_config().is_rule_enabled(*prefer_frontmatter));

        let wrapped = serde_json::json!({
            "lint": {
                "enable_rules": [
                    "merman.authoring.config.prefer_init_directive",
                    "merman.authoring.config.prefer_frontmatter_config"
                ]
            }
        });
        let analysis = analysis_options_from_json_value(&wrapped).unwrap();

        assert_eq!(analysis.rule_config().profile(), AnalysisRuleProfile::Core);
        assert!(analysis.rule_config().is_rule_enabled(*prefer_init));
        assert!(analysis.rule_config().is_rule_enabled(*prefer_frontmatter));
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

        assert!(!analysis.rule_config().is_rule_enabled(*duplicate_commit));

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
            analysis.rule_config().profile(),
            AnalysisRuleProfile::Recommended
        );
        assert_eq!(
            analysis.rule_config().severity_for(*prefer_init),
            DiagnosticSeverity::Warning
        );
        assert!(analysis.rule_config().is_rule_enabled(*prefer_init));
        assert!(analysis.rule_config().is_rule_enabled(*prefer_frontmatter));
    }

    #[test]
    fn shared_analysis_options_json_rejects_removed_parse_options() {
        for options in [
            serde_json::json!({
                "parse": { "suppress_errors": true }
            }),
            serde_json::json!({
                "analysis": {
                    "parse": { "suppress_errors": true }
                }
            }),
            serde_json::json!({
                "merman": {
                    "parse": { "suppress_errors": true }
                }
            }),
        ] {
            let err = analysis_options_from_json_value(&options).unwrap_err();
            assert!(
                err.to_string()
                    .contains("analysis option `parse` was removed"),
                "unexpected error for {options}: {err}"
            );
        }
    }

    #[test]
    fn shared_analysis_options_json_applies_owner_minimum_resource_values() {
        let positive = serde_json::json!({
            "analysis": {
                "resources": {
                    "limits": {
                        "max_source_bytes": 1024,
                        "max_document_diagrams": 256
                    }
                }
            }
        });

        let zero_source = serde_json::json!({
            "analysis": { "resources": { "limits": { "max_source_bytes": 0 } } }
        });
        assert!(analysis_options_from_json_value(&zero_source).is_err());

        let zero_document = serde_json::json!({
            "analysis": { "resources": { "limits": { "max_document_diagrams": 0 } } }
        });
        assert_eq!(
            analysis_options_from_json_value(&zero_document)
                .unwrap()
                .max_document_diagrams(),
            Some(0)
        );

        let options = analysis_options_from_json_value(&positive).unwrap();
        assert_eq!(options.max_source_bytes(), Some(1024));
        assert_eq!(options.max_document_diagrams(), Some(256));

        let unknown = serde_json::json!({
            "resources": {
                "limits": { "max_future_analysis_resource": 1 }
            }
        });
        let error = analysis_options_from_json_value(&unknown).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unknown analysis resource limit id"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn shared_analysis_options_json_rejects_two_namespaced_analysis_wrappers() {
        for mixed in [
            serde_json::json!({ "merman": {}, "analysis": {} }),
            serde_json::json!({
                "merman": { "fixed_today": "2025-01-01" },
                "analysis": {}
            }),
            serde_json::json!({
                "merman": {},
                "analysis": { "fixed_today": "2025-01-01" }
            }),
        ] {
            let err = analysis_options_from_json_value(&mixed).unwrap_err();

            assert!(
                err.to_string()
                    .contains("must not contain both `merman` and `analysis` wrappers"),
                "unexpected error: {err}"
            );
        }
    }

    #[test]
    fn shared_analysis_options_json_rejects_mixed_direct_and_namespaced_options() {
        let mixed = serde_json::json!({
            "resources": {
                "limits": { "max_source_bytes": 1024 }
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
