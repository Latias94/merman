use crate::{AnalysisOptions, AnalysisRuleConfig, AnalysisRuleProfile, DiagnosticSeverity};
use chrono::NaiveDate;
use merman_core::MermaidConfig;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Map;
use serde_json::Value;
use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceOptionsJson {
    #[serde(default)]
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

// The shared root format is forward-compatible, while direct consumers of the public nested
// schema types retain their strict unknown-field validation.
#[derive(Deserialize)]
struct PermissiveAnalysisOptionsJson {
    fixed_today: Option<String>,
    fixed_local_offset_minutes: Option<i32>,
    site_config: Option<Value>,
    resources: Option<ResourceOptionsJson>,
    lint: Option<PermissiveLintOptionsJson>,
}

#[derive(Deserialize)]
struct PermissiveLintOptionsJson {
    profile: Option<String>,
    #[serde(default)]
    enable_rules: Vec<String>,
    #[serde(default)]
    disable_rules: Vec<String>,
    #[serde(default)]
    rule_severities: Vec<PermissiveLintRuleSeverityOverrideJson>,
}

#[derive(Deserialize)]
struct PermissiveLintRuleSeverityOverrideJson {
    rule_id: String,
    severity: String,
}

impl<'de> Deserialize<'de> for AnalysisOptionsJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(PermissiveAnalysisOptionsJson::deserialize(deserializer)?.into())
    }
}

impl From<PermissiveAnalysisOptionsJson> for AnalysisOptionsJson {
    fn from(options: PermissiveAnalysisOptionsJson) -> Self {
        Self {
            fixed_today: options.fixed_today,
            fixed_local_offset_minutes: options.fixed_local_offset_minutes,
            site_config: options.site_config,
            resources: options.resources,
            lint: options.lint.map(Into::into),
        }
    }
}

impl From<PermissiveLintOptionsJson> for LintOptionsJson {
    fn from(options: PermissiveLintOptionsJson) -> Self {
        Self {
            profile: options.profile,
            enable_rules: options.enable_rules,
            disable_rules: options.disable_rules,
            rule_severities: options
                .rule_severities
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<PermissiveLintRuleSeverityOverrideJson> for LintRuleSeverityOverrideJson {
    fn from(override_: PermissiveLintRuleSeverityOverrideJson) -> Self {
        Self {
            rule_id: override_.rule_id,
            severity: override_.severity,
        }
    }
}

/// An invalid analysis-options JSON shape or value.
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

/// Decodes the shared root JSON shape and converts it to validated analysis options.
///
/// This accepts direct options or the supported `analysis`/`merman` wrapper forms. Unknown root
/// and lint fields are ignored; strict nested resource validation and removed-option checks still
/// apply.
pub fn analysis_options_from_json_value(
    value: &Value,
) -> Result<AnalysisOptions, AnalysisOptionsJsonError> {
    analysis_options_json_from_json_value(value)?.to_analysis_options()
}

/// Decodes the forward-compatible shared root JSON shape without converting it.
///
/// Callers that intentionally validate one nested value should deserialize the corresponding
/// public nested type directly, which rejects unknown fields.
pub fn analysis_options_json_from_json_value(
    value: &Value,
) -> Result<AnalysisOptionsJson, AnalysisOptionsJsonError> {
    reject_removed_analysis_parse_option(value)?;
    let options_value = analysis_options_root_value(value)?;
    serde_json::from_value(options_value.clone()).map_err(|err| {
        AnalysisOptionsJsonError::new(format!("invalid analysis options JSON: {err}"))
    })
}

fn analysis_options_root_value(value: &Value) -> Result<&Value, AnalysisOptionsJsonError> {
    let Value::Object(map) = value else {
        return Ok(value);
    };

    if map.contains_key("merman") && map.contains_key("analysis") {
        return Err(AnalysisOptionsJsonError::new(
            "options JSON must not contain both `merman` and `analysis` wrappers",
        ));
    }

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

    let wrapped_key = ["merman", "analysis"].into_iter().find(|key| {
        map.get(*key)
            .and_then(Value::as_object)
            .is_some_and(analysis_option_keys_present)
    });
    if let Some(key) = wrapped_key {
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
        "resources",
        "lint",
    ]
    .iter()
    .any(|key| map.contains_key(*key))
}

fn reject_removed_analysis_parse_option(value: &Value) -> Result<(), AnalysisOptionsJsonError> {
    let Value::Object(map) = value else {
        return Ok(());
    };
    let removed = map.contains_key("parse")
        || ["merman", "analysis"].iter().any(|key| {
            map.get(*key)
                .and_then(Value::as_object)
                .is_some_and(|options| options.contains_key("parse"))
        });
    if removed {
        return Err(AnalysisOptionsJsonError::new(
            "analysis option `parse` was removed; analysis always retains family parse failures",
        ));
    }
    Ok(())
}

impl AnalysisOptionsJson {
    pub fn to_analysis_options(&self) -> Result<AnalysisOptions, AnalysisOptionsJsonError> {
        let today = self.fixed_today()?;
        let offset_minutes = self.fixed_local_offset_minutes()?;
        let mut analysis = AnalysisOptions::default()
            .with_max_source_bytes(self.max_source_bytes()?)
            .with_max_document_diagrams(self.max_document_diagrams()?);

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
        self.resource_limit(merman_core::resources::InputResourceLimitId::MaxSourceBytes.as_str())
    }

    pub fn max_document_diagrams(&self) -> Result<Option<usize>, AnalysisOptionsJsonError> {
        self.resource_limit(crate::MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID)
    }

    fn resource_limit(&self, limit_id: &str) -> Result<Option<usize>, AnalysisOptionsJsonError> {
        let Some(resources) = self.resources.as_ref() else {
            return Ok(None);
        };
        if let Some(unknown) = resources
            .limits
            .keys()
            .find(|id| analysis_resource_limit_minimum_value(id).is_none())
        {
            return Err(AnalysisOptionsJsonError::new(format!(
                "unknown analysis resource limit id: {unknown}"
            )));
        }
        let limit = resources.limits.get(limit_id).copied();
        let minimum_value = analysis_resource_limit_minimum_value(limit_id)
            .expect("analysis resource limit id must be validated by its owner descriptor");
        if limit.is_some_and(|value| value < minimum_value) {
            return Err(AnalysisOptionsJsonError::new(format!(
                "resources.limits.{limit_id} must be at least {minimum_value}"
            )));
        }
        Ok(limit)
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

fn analysis_resource_limit_minimum_value(limit_id: &str) -> Option<usize> {
    let source = merman_core::resources::InputResourceLimitId::MaxSourceBytes.descriptor();
    if limit_id == source.stable_id {
        return Some(source.minimum_value);
    }
    crate::ANALYSIS_RESOURCE_LIMIT_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.stable_id == limit_id)
        .map(|descriptor| descriptor.minimum_value)
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

        assert_eq!(
            context.today_local(),
            NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()
        );
        assert_eq!(context.local_time_zone().fixed_offset_minutes(), Some(0));
        assert_eq!(
            context.clock_source(),
            merman_core::runtime::RuntimeValueSource::Fixed
        );
    }

    #[test]
    fn shared_analysis_options_json_rejects_unrepresentable_fixed_local_midnight() {
        let options = AnalysisOptionsJson {
            fixed_today: Some("-262143-01-01".to_string()),
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
