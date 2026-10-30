use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Map, Value};

use crate::MermaidConfig;

const MAX_THEME_COMPATIBILITY_VARIABLES: usize = 512;
const MAX_THEME_COMPATIBILITY_IDENTIFIER_BYTES: usize = 128;
const MAX_THEME_COMPATIBILITY_STRING_BYTES: usize = 1024;
const MAX_THEME_COMPATIBILITY_NUMBER_ABS: f64 = 1_000_000_000.0;

/// Invalid bounded Mermaid compatibility input for a compiled theme recipe.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ThemeCompatibilityRecipeError {
    #[error("theme compatibility config must be an object")]
    InvalidRoot,
    #[error("unsupported theme compatibility field `{field}`")]
    UnsupportedField { field: String },
    #[error("invalid theme compatibility value for `{field}`")]
    InvalidValue { field: &'static str },
    #[error("theme compatibility limit exceeded for `{field}`")]
    LimitExceeded { field: &'static str },
}

/// A compiled recipe identity paired with its complete normalized Mermaid compatibility input.
///
/// Equality includes both the identity and normalized values. This is declared data, not proof
/// that a parser or theme evaluator produced a presentation value.
#[derive(Clone, PartialEq, Eq)]
pub struct ThemeCompatibilityRecipe {
    recipe_identity: [u8; 32],
    compatibility_config: MermaidConfig,
}

impl fmt::Debug for ThemeCompatibilityRecipe {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThemeCompatibilityRecipe")
            .field("recipe_identity", &self.recipe_identity)
            .finish_non_exhaustive()
    }
}

impl ThemeCompatibilityRecipe {
    /// Validates and normalizes the bounded compatibility subset without evaluating a theme.
    pub fn try_new(
        recipe_identity: [u8; 32],
        compatibility_config: MermaidConfig,
    ) -> Result<Self, ThemeCompatibilityRecipeError> {
        Ok(Self {
            recipe_identity,
            compatibility_config: MermaidConfig::from_value(normalize_theme_compatibility_config(
                compatibility_config.as_value(),
            )?),
        })
    }

    pub const fn recipe_identity(&self) -> &[u8; 32] {
        &self.recipe_identity
    }

    pub fn compatibility_config(&self) -> &MermaidConfig {
        &self.compatibility_config
    }
}

fn normalize_theme_compatibility_config(
    config: &Value,
) -> Result<Value, ThemeCompatibilityRecipeError> {
    let root = config
        .as_object()
        .ok_or(ThemeCompatibilityRecipeError::InvalidRoot)?;
    for key in root.keys() {
        if !matches!(key.as_str(), "theme" | "darkMode" | "themeVariables") {
            return Err(ThemeCompatibilityRecipeError::UnsupportedField { field: key.clone() });
        }
    }

    let mut normalized = Map::new();
    if let Some(theme) = root.get("theme") {
        let Value::String(theme) = theme else {
            return Err(ThemeCompatibilityRecipeError::InvalidValue { field: "theme" });
        };
        normalized.insert(
            "theme".to_string(),
            Value::String(normalize_theme_compatibility_identifier(theme, "theme")?),
        );
    }

    let root_dark_mode = root
        .get("darkMode")
        .map(|value| match value {
            Value::Bool(value) => Ok(*value),
            _ => Err(ThemeCompatibilityRecipeError::InvalidValue { field: "darkMode" }),
        })
        .transpose()?;
    let mut variables = BTreeMap::new();
    let mut variable_dark_mode = None;
    if let Some(raw_variables) = root.get("themeVariables") {
        let Value::Object(raw_variables) = raw_variables else {
            return Err(ThemeCompatibilityRecipeError::InvalidValue {
                field: "themeVariables",
            });
        };
        let mut variable_count = 0usize;
        for (raw_key, raw_value) in raw_variables {
            let key = normalize_theme_compatibility_identifier(raw_key, "themeVariables.key")?;
            if key == "darkMode" {
                let Value::Bool(value) = raw_value else {
                    return Err(ThemeCompatibilityRecipeError::InvalidValue {
                        field: "themeVariables.darkMode",
                    });
                };
                variable_dark_mode = Some(*value);
                continue;
            }
            variable_count = variable_count.saturating_add(1);
            if variable_count > MAX_THEME_COMPATIBILITY_VARIABLES {
                return Err(ThemeCompatibilityRecipeError::LimitExceeded {
                    field: "themeVariables",
                });
            }
            let value = normalize_theme_compatibility_scalar(raw_value)?;
            if variables.insert(key, value).is_some() {
                return Err(ThemeCompatibilityRecipeError::InvalidValue {
                    field: "themeVariables.key",
                });
            }
        }
    }

    let dark_mode = match (root_dark_mode, variable_dark_mode) {
        (Some(root), Some(variable)) if root != variable => {
            return Err(ThemeCompatibilityRecipeError::InvalidValue { field: "darkMode" });
        }
        (Some(value), _) | (_, Some(value)) => Some(value),
        (None, None) => None,
    };
    if let Some(dark_mode) = dark_mode {
        normalized.insert("darkMode".to_string(), Value::Bool(dark_mode));
        variables.insert("darkMode".to_string(), Value::Bool(dark_mode));
    }
    if !variables.is_empty() {
        normalized.insert(
            "themeVariables".to_string(),
            Value::Object(variables.into_iter().collect()),
        );
    }
    Ok(Value::Object(normalized))
}

fn normalize_theme_compatibility_identifier(
    value: &str,
    field: &'static str,
) -> Result<String, ThemeCompatibilityRecipeError> {
    let normalized = value.trim();
    if !is_theme_compatibility_identifier(normalized) {
        return Err(ThemeCompatibilityRecipeError::InvalidValue { field });
    }
    Ok(normalized.to_string())
}

pub(super) fn is_theme_compatibility_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_THEME_COMPATIBILITY_IDENTIFIER_BYTES
        && value.chars().all(|character| {
            !character.is_control()
                && (character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        })
}

fn normalize_theme_compatibility_scalar(
    value: &Value,
) -> Result<Value, ThemeCompatibilityRecipeError> {
    match value {
        Value::String(value) => {
            let normalized = value.trim();
            if normalized.is_empty() || normalized.len() > MAX_THEME_COMPATIBILITY_STRING_BYTES {
                return Err(ThemeCompatibilityRecipeError::InvalidValue {
                    field: "themeVariables.value",
                });
            }
            Ok(Value::String(normalized.to_string()))
        }
        Value::Number(value) => {
            let value = value
                .as_f64()
                .ok_or(ThemeCompatibilityRecipeError::InvalidValue {
                    field: "themeVariables.value",
                })?;
            if !value.is_finite() || value.abs() > MAX_THEME_COMPATIBILITY_NUMBER_ABS {
                return Err(ThemeCompatibilityRecipeError::InvalidValue {
                    field: "themeVariables.value",
                });
            }
            let value = if value == 0.0 { 0.0 } else { value };
            serde_json::Number::from_f64(value)
                .map(Value::Number)
                .ok_or(ThemeCompatibilityRecipeError::InvalidValue {
                    field: "themeVariables.value",
                })
        }
        Value::Bool(value) => Ok(Value::Bool(*value)),
        Value::Null | Value::Array(_) | Value::Object(_) => {
            Err(ThemeCompatibilityRecipeError::InvalidValue {
                field: "themeVariables.value",
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn recipe_error(value: Value) -> ThemeCompatibilityRecipeError {
        ThemeCompatibilityRecipe::try_new([0; 32], MermaidConfig::from_value(value)).unwrap_err()
    }

    #[test]
    fn normalization_preserves_the_bounded_compatibility_contract() {
        for value in [Value::Null, json!(3), json!([])] {
            assert_eq!(
                recipe_error(value),
                ThemeCompatibilityRecipeError::InvalidRoot
            );
        }
        assert_eq!(
            recipe_error(json!({"theme": "dark", "look": "neo"})),
            ThemeCompatibilityRecipeError::UnsupportedField {
                field: "look".to_string(),
            }
        );
        for value in [
            json!({"theme": true}),
            json!({"theme": "theme.name"}),
            json!({"theme": "x".repeat(MAX_THEME_COMPATIBILITY_IDENTIFIER_BYTES + 1)}),
            json!({"darkMode": "true"}),
            json!({"darkMode": true, "themeVariables": {"darkMode": false}}),
            json!({"themeVariables": []}),
            json!({"themeVariables": {"darkMode": "true"}}),
            json!({"themeVariables": {"bad key": "value"}}),
            json!({"themeVariables": {"primaryColor": "red", " primaryColor ": "blue"}}),
            json!({"themeVariables": {"value": null}}),
            json!({"themeVariables": {"value": []}}),
            json!({"themeVariables": {"value": {}}}),
            json!({"themeVariables": {"value": "  "}}),
            json!({"themeVariables": {"value": "x".repeat(MAX_THEME_COMPATIBILITY_STRING_BYTES + 1)}}),
            json!({"themeVariables": {"value": MAX_THEME_COMPATIBILITY_NUMBER_ABS + 1.0}}),
        ] {
            assert!(matches!(
                recipe_error(value),
                ThemeCompatibilityRecipeError::InvalidValue { .. }
            ));
        }
    }

    #[test]
    fn normalization_retains_boundary_values_and_one_dark_mode_concept() {
        let mut variables = Map::new();
        for index in 0..508 {
            variables.insert(format!("variable{index}"), json!(index));
        }
        variables.insert(" primaryColor ".to_string(), json!(" value "));
        variables.insert("darkMode".to_string(), json!(true));
        variables.insert("zero".to_string(), json!(-0.0));
        variables.insert("max".to_string(), json!(MAX_THEME_COMPATIBILITY_NUMBER_ABS));
        variables.insert(
            "long".to_string(),
            json!("x".repeat(MAX_THEME_COMPATIBILITY_STRING_BYTES)),
        );
        let theme = "t".repeat(MAX_THEME_COMPATIBILITY_IDENTIFIER_BYTES);
        let recipe = ThemeCompatibilityRecipe::try_new(
            [0x5a; 32],
            MermaidConfig::from_value(json!({
                "theme": format!(" {theme} "),
                "darkMode": true,
                "themeVariables": variables,
            })),
        )
        .unwrap();
        let normalized = recipe.compatibility_config().as_value();
        assert_eq!(normalized["theme"], json!(theme));
        assert_eq!(normalized["darkMode"], json!(true));
        let variables = normalized["themeVariables"].as_object().unwrap();
        assert_eq!(variables.len(), MAX_THEME_COMPATIBILITY_VARIABLES + 1);
        assert_eq!(variables["primaryColor"], json!("value"));
        assert_eq!(variables["darkMode"], json!(true));
        assert_eq!(variables["zero"], json!(0.0));
        assert_eq!(variables["max"], json!(MAX_THEME_COMPATIBILITY_NUMBER_ABS));
        assert_eq!(
            variables["long"].as_str().unwrap().len(),
            MAX_THEME_COMPATIBILITY_STRING_BYTES
        );
    }

    #[test]
    fn normalization_enforces_the_existing_variable_limit() {
        let variables: Map<_, _> = (0..=MAX_THEME_COMPATIBILITY_VARIABLES)
            .map(|index| (format!("variable{index}"), json!(index)))
            .collect();
        assert_eq!(
            recipe_error(json!({"themeVariables": variables})),
            ThemeCompatibilityRecipeError::LimitExceeded {
                field: "themeVariables",
            }
        );
    }

    #[test]
    fn recipe_equality_compares_identity_and_all_normalized_values() {
        let recipe = |identity, values| {
            ThemeCompatibilityRecipe::try_new(identity, MermaidConfig::from_value(values)).unwrap()
        };
        let first = recipe(
            [0x4a; 32],
            json!({"theme": " base ", "themeVariables": {"darkMode": true, "zero": -0.0}}),
        );
        let equivalent = recipe(
            [0x4a; 32],
            json!({"themeVariables": {"zero": 0, "darkMode": true}, "darkMode": true, "theme": "base"}),
        );
        assert_eq!(first, equivalent);
        assert_ne!(
            first,
            recipe(
                [0x4a; 32],
                json!({"theme": "base", "darkMode": false, "themeVariables": {"zero": 0}}),
            )
        );
        assert_ne!(
            first,
            recipe(
                [0x4b; 32],
                json!({"theme": "base", "darkMode": true, "themeVariables": {"zero": 0}}),
            )
        );
    }
}
