use crate::{AnalysisOptions, AnalysisRuleConfig, DiagnosticSeverity};
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
    let options_value = analysis_options_root_value(value);
    let options: AnalysisOptionsJson =
        serde_json::from_value(options_value.clone()).map_err(|err| {
            AnalysisOptionsJsonError::new(format!("invalid analysis options JSON: {err}"))
        })?;
    options.to_analysis_options()
}

fn analysis_options_root_value(value: &Value) -> &Value {
    let Value::Object(map) = value else {
        return value;
    };

    if analysis_option_keys_present(map) {
        return value;
    }

    for key in ["merman", "analysis"] {
        if let Some(Value::Object(inner)) = map.get(key) {
            if analysis_option_keys_present(inner) {
                return map
                    .get(key)
                    .expect("checked key existence and object shape");
            }
        }
    }

    value
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
            .with_max_source_bytes(self.max_source_bytes());

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

    pub fn max_source_bytes(&self) -> Option<usize> {
        self.resources
            .as_ref()
            .and_then(|resources| resources.max_source_bytes)
    }

    pub fn rule_config(&self) -> Result<AnalysisRuleConfig, AnalysisOptionsJsonError> {
        let Some(lint) = self.lint.as_ref() else {
            return Ok(AnalysisRuleConfig::default());
        };

        let mut config = AnalysisRuleConfig::default();
        for rule_id in &lint.disable_rules {
            if rule_id.trim().is_empty() {
                return Err(AnalysisOptionsJsonError::new(
                    "lint.disable_rules entries must not be empty",
                ));
            }
            config.disable_rule(rule_id.clone());
        }

        for override_ in &lint.rule_severities {
            if override_.rule_id.trim().is_empty() {
                return Err(AnalysisOptionsJsonError::new(
                    "lint.rule_severities.rule_id must not be empty",
                ));
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_descriptors;

    #[test]
    fn shared_analysis_options_json_honors_lint_configuration() {
        let options = AnalysisOptionsJson {
            lint: Some(LintOptionsJson {
                disable_rules: vec!["merman.git_graph.duplicate_commit_id".to_string()],
                rule_severities: vec![LintRuleSeverityOverrideJson {
                    rule_id: "merman.config.prefer_init_directive".to_string(),
                    severity: "hint".to_string(),
                }],
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
            .find(|descriptor| descriptor.id == "merman.config.prefer_init_directive")
            .unwrap();

        assert!(!analysis.rule_config.is_rule_enabled(*duplicate_commit));
        assert_eq!(
            analysis.rule_config.severity_for(*prefer_init),
            DiagnosticSeverity::Hint
        );
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
                    "rule_severities": [
                        {
                            "rule_id": "merman.config.prefer_init_directive",
                            "severity": "warning"
                        }
                    ]
                }
            }
        });
        let analysis = analysis_options_from_json_value(&wrapped).unwrap();
        let prefer_init = rule_descriptors()
            .iter()
            .find(|descriptor| descriptor.id == "merman.config.prefer_init_directive")
            .unwrap();

        assert_eq!(
            analysis.rule_config.severity_for(*prefer_init),
            DiagnosticSeverity::Warning
        );
    }
}
