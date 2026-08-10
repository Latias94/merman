use std::collections::BTreeMap;

use merman_core::{MermaidConfig, MermaidThemeId};
use serde_json::{Map, Value};

use super::admission::ThemeRequirements;
use super::canvas::CanvasSpec;
use super::effects::DiagramEffectSet;
use super::semantic::ThemeRuleSet;
use super::typography::TypographySpec;
use super::{FontCatalogSpec, ThemeCompileValidationError};

/// Restricted Mermaid compatibility inputs retained as an escape hatch for upstream variables.
/// Renderer selection, look, HTML labels, and output policy deliberately do not belong here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MermaidThemeCompatibility {
    theme: Option<MermaidThemeId>,
    dark_mode: Option<bool>,
    variables: BTreeMap<String, MermaidThemeValue>,
}

/// A bounded scalar accepted by Mermaid's upstream `themeVariables` compatibility lane.
///
/// Structured colors, typography, effects, and resources belong to the typed theme spec. This
/// union only preserves the scalar JSON values Mermaid itself consumes for compatibility fields.
#[derive(Debug, Clone, PartialEq)]
pub enum MermaidThemeValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

impl From<&str> for MermaidThemeValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for MermaidThemeValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<f64> for MermaidThemeValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<bool> for MermaidThemeValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl MermaidThemeCompatibility {
    /// Parses and selects an upstream Mermaid theme name.
    pub fn with_theme(self, theme: impl AsRef<str>) -> Result<Self, ThemeCompileValidationError> {
        let theme = MermaidThemeId::parse(theme.as_ref()).map_err(|error| {
            ThemeCompileValidationError::UnsupportedMermaidTheme {
                field: "mermaid.theme",
                value: error.input().to_string(),
            }
        })?;
        Ok(self.with_theme_id(theme))
    }

    /// Selects an already validated theme from the pinned Mermaid catalog.
    pub fn with_theme_id(mut self, theme: MermaidThemeId) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Sets Mermaid's mirrored root and `themeVariables.darkMode` compatibility value.
    /// Repeating the same value is idempotent; assigning a conflicting value fails closed.
    pub fn with_dark_mode(self, dark_mode: bool) -> Result<Self, ThemeCompileValidationError> {
        self.with_canonical_dark_mode(dark_mode, "mermaid.darkMode")
    }

    /// Adds one upstream Mermaid `themeVariables` scalar.
    ///
    /// `darkMode` is reserved and must be a boolean. It is normalized into the same canonical
    /// slot as [`Self::with_dark_mode`] so the root and `themeVariables` views cannot diverge.
    pub fn with_variable(
        mut self,
        key: impl Into<String>,
        value: impl Into<MermaidThemeValue>,
    ) -> Result<Self, ThemeCompileValidationError> {
        let key = validate_identifier(key.into(), "mermaid.theme_variables.key")?;
        let value = validate_theme_value(value.into(), "mermaid.theme_variables.value")?;
        if key == "darkMode" {
            let MermaidThemeValue::Boolean(dark_mode) = value else {
                return Err(ThemeCompileValidationError::InvalidValue {
                    field: "mermaid.theme_variables.darkMode",
                });
            };
            return self.with_canonical_dark_mode(dark_mode, "mermaid.theme_variables.darkMode");
        }
        self.variables.insert(key, value);
        Ok(self)
    }

    pub const fn theme(&self) -> Option<MermaidThemeId> {
        self.theme
    }

    pub fn theme_name(&self) -> Option<&str> {
        self.theme.map(MermaidThemeId::as_str)
    }

    pub const fn dark_mode(&self) -> Option<bool> {
        self.dark_mode
    }

    pub fn variables(&self) -> impl ExactSizeIterator<Item = (&str, &MermaidThemeValue)> {
        self.variables
            .iter()
            .map(|(key, value)| (key.as_str(), value))
    }

    pub(crate) fn to_mermaid_config(&self) -> MermaidConfig {
        let mut root = Map::new();
        if let Some(theme) = &self.theme {
            root.insert(
                "theme".to_string(),
                Value::String(theme.as_str().to_string()),
            );
        }
        if let Some(dark_mode) = self.dark_mode {
            root.insert("darkMode".to_string(), Value::Bool(dark_mode));
        }
        let mut variables = self
            .variables
            .iter()
            .map(|(key, value)| (key.clone(), value.to_json()))
            .collect::<Map<_, _>>();
        if let Some(dark_mode) = self.dark_mode {
            variables.insert("darkMode".to_string(), Value::Bool(dark_mode));
        }
        if !variables.is_empty() {
            root.insert("themeVariables".to_string(), Value::Object(variables));
        }
        MermaidConfig::from_value(Value::Object(root))
    }

    pub(crate) fn validate(&self) -> Result<(), ThemeCompileValidationError> {
        if self.variables.len() > 512 {
            return Err(ThemeCompileValidationError::LimitExceeded {
                field: "mermaid.theme_variables",
            });
        }
        for (key, value) in &self.variables {
            validate_identifier(key.clone(), "mermaid.theme_variables.key")?;
            validate_theme_value(value.clone(), "mermaid.theme_variables.value")?;
        }
        if self.variables.contains_key("darkMode") {
            return Err(ThemeCompileValidationError::InvalidValue {
                field: "mermaid.theme_variables.darkMode",
            });
        }
        Ok(())
    }

    fn with_canonical_dark_mode(
        mut self,
        dark_mode: bool,
        field: &'static str,
    ) -> Result<Self, ThemeCompileValidationError> {
        if self.dark_mode.is_some_and(|current| current != dark_mode) {
            return Err(ThemeCompileValidationError::InvalidValue { field });
        }
        self.dark_mode = Some(dark_mode);
        Ok(self)
    }
}

impl MermaidThemeValue {
    fn to_json(&self) -> Value {
        match self {
            Self::String(value) => Value::String(value.clone()),
            Self::Number(value) => Value::Number(
                serde_json::Number::from_f64(*value)
                    .expect("validated Mermaid theme numbers are finite"),
            ),
            Self::Boolean(value) => Value::Bool(*value),
        }
    }
}

fn validate_theme_value(
    value: MermaidThemeValue,
    field: &'static str,
) -> Result<MermaidThemeValue, ThemeCompileValidationError> {
    match value {
        MermaidThemeValue::String(value) => Ok(MermaidThemeValue::String(
            validate_compatibility_value(value, field)?,
        )),
        MermaidThemeValue::Number(value) if value.is_finite() && value.abs() <= 1_000_000_000.0 => {
            Ok(MermaidThemeValue::Number(value))
        }
        MermaidThemeValue::Boolean(value) => Ok(MermaidThemeValue::Boolean(value)),
        MermaidThemeValue::Number(_) => Err(ThemeCompileValidationError::InvalidValue { field }),
    }
}

#[derive(Debug, Clone, Default)]
pub struct ThemeAssets {
    font_catalog: Option<FontCatalogSpec>,
}

impl ThemeAssets {
    pub fn with_font_catalog(mut self, catalog: FontCatalogSpec) -> Self {
        self.font_catalog = Some(catalog);
        self
    }

    pub const fn font_catalog(&self) -> Option<&FontCatalogSpec> {
        self.font_catalog.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct DiagramThemeSpec {
    mermaid: MermaidThemeCompatibility,
    typography: TypographySpec,
    styles: ThemeRuleSet,
    canvas: CanvasSpec,
    effects: DiagramEffectSet,
    assets: ThemeAssets,
    requirements: ThemeRequirements,
}

impl Default for DiagramThemeSpec {
    fn default() -> Self {
        Self {
            mermaid: MermaidThemeCompatibility::default(),
            typography: TypographySpec::default(),
            styles: ThemeRuleSet::default(),
            canvas: CanvasSpec::default(),
            effects: DiagramEffectSet::default(),
            assets: ThemeAssets::default(),
            requirements: ThemeRequirements::default(),
        }
    }
}

impl DiagramThemeSpec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_mermaid_compatibility(mut self, mermaid: MermaidThemeCompatibility) -> Self {
        self.mermaid = mermaid;
        self
    }

    pub fn with_typography(mut self, typography: TypographySpec) -> Self {
        self.typography = typography;
        self
    }

    pub fn with_styles(mut self, styles: ThemeRuleSet) -> Self {
        self.styles = styles;
        self
    }

    pub fn with_canvas(mut self, canvas: CanvasSpec) -> Self {
        self.canvas = canvas;
        self
    }

    pub fn with_effects(mut self, effects: DiagramEffectSet) -> Self {
        self.effects = effects;
        self
    }

    pub fn with_assets(mut self, assets: ThemeAssets) -> Self {
        self.assets = assets;
        self
    }

    pub fn with_requirements(mut self, requirements: ThemeRequirements) -> Self {
        self.requirements = requirements;
        self
    }

    pub const fn mermaid(&self) -> &MermaidThemeCompatibility {
        &self.mermaid
    }

    pub const fn typography(&self) -> &TypographySpec {
        &self.typography
    }

    pub const fn styles(&self) -> &ThemeRuleSet {
        &self.styles
    }

    pub const fn canvas(&self) -> &CanvasSpec {
        &self.canvas
    }

    pub const fn effects(&self) -> &DiagramEffectSet {
        &self.effects
    }

    pub const fn assets(&self) -> &ThemeAssets {
        &self.assets
    }

    pub const fn requirements(&self) -> &ThemeRequirements {
        &self.requirements
    }

    pub(crate) fn validate(&self) -> Result<(), ThemeCompileValidationError> {
        self.mermaid.validate()?;
        self.typography.validate()?;
        self.styles.validate()?;
        self.canvas.validate()?;
        self.effects.validate()?;
        for rule in self.styles.rules() {
            let super::Specified::Value(effect_id) = &rule.style().effects.effect else {
                continue;
            };
            if self.effects.graph(effect_id).is_none() {
                return Err(ThemeCompileValidationError::UnknownId {
                    field: "styles.rule.effect",
                });
            }
        }
        Ok(())
    }
}

fn validate_identifier(
    value: String,
    field: &'static str,
) -> Result<String, ThemeCompileValidationError> {
    let value = value.trim().to_string();
    if value.is_empty()
        || value.len() > 128
        || value.chars().any(|character| {
            character.is_control()
                || !(character.is_ascii_alphanumeric() || "-_".contains(character))
        })
    {
        return Err(ThemeCompileValidationError::InvalidValue { field });
    }
    Ok(value)
}

fn validate_compatibility_value(
    value: String,
    field: &'static str,
) -> Result<String, ThemeCompileValidationError> {
    let value = value.trim().to_string();
    let lower = value.to_ascii_lowercase();
    let resource_like = lower.contains("://")
        || lower.starts_with("//")
        || lower.starts_with("data:")
        || lower.starts_with("file:")
        || lower.starts_with("./")
        || lower.starts_with("../")
        || lower.starts_with('/')
        || lower.contains("var(");
    if value.is_empty()
        || value.len() > 1024
        || resource_like
        || !crate::mermaid_style::is_resource_free_css_value(&value)
    {
        return Err(ThemeCompileValidationError::InvalidValue { field });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_values_accept_bounded_resource_free_css_values() {
        for value in [
            "#123456",
            "rgba(0, 0, 0, 0.25)",
            "drop-shadow(0 1px 2px rgba(0, 0, 0, 0.2))",
            "Inter, ui-sans-serif, sans-serif",
            "1.5px",
        ] {
            MermaidThemeCompatibility::default()
                .with_variable("primaryColor", value)
                .unwrap_or_else(|error| panic!("{value:?} should be accepted: {error}"));
        }
    }

    #[test]
    fn compatibility_values_preserve_json_scalar_types() {
        let compatibility = MermaidThemeCompatibility::default()
            .with_variable("fontSize", 16.0)
            .unwrap()
            .with_variable("darkMode", true)
            .unwrap()
            .with_variable("primaryColor", "#123456")
            .unwrap();

        let config = compatibility.to_mermaid_config();
        assert_eq!(config.get_bool("darkMode"), Some(true));
        let variables = config
            .as_value()
            .get("themeVariables")
            .and_then(Value::as_object)
            .expect("theme variables object");
        assert_eq!(variables.get("fontSize"), Some(&Value::from(16.0)));
        assert_eq!(variables.get("darkMode"), Some(&Value::Bool(true)));
        assert_eq!(
            variables.get("primaryColor"),
            Some(&Value::String("#123456".to_string()))
        );
    }

    #[test]
    fn mermaid_theme_compatibility_uses_the_core_theme_catalog() {
        for &theme in MermaidThemeId::ALL {
            let compatibility = MermaidThemeCompatibility::default()
                .with_theme(theme.as_str())
                .unwrap();
            assert_eq!(compatibility.theme(), Some(theme));
            assert_eq!(compatibility.theme_name(), Some(theme.as_str()));
            assert_eq!(
                compatibility.to_mermaid_config().get_str("theme"),
                Some(theme.as_str())
            );
        }

        let error = MermaidThemeCompatibility::default()
            .with_theme("unknown")
            .unwrap_err();
        assert_eq!(
            error,
            ThemeCompileValidationError::UnsupportedMermaidTheme {
                field: "mermaid.theme",
                value: "unknown".to_string(),
            }
        );
    }

    #[test]
    fn matching_dark_mode_inputs_are_canonicalized_independent_of_builder_order() {
        let root_then_variable = MermaidThemeCompatibility::default()
            .with_dark_mode(true)
            .unwrap()
            .with_variable("darkMode", true)
            .unwrap();
        let variable_then_root = MermaidThemeCompatibility::default()
            .with_variable("darkMode", true)
            .unwrap()
            .with_dark_mode(true)
            .unwrap();

        for compatibility in [&root_then_variable, &variable_then_root] {
            compatibility.validate().unwrap();
            assert!(compatibility.variables().all(|(key, _)| key != "darkMode"));
            assert_eq!(
                compatibility.to_mermaid_config().as_value(),
                &serde_json::json!({
                    "darkMode": true,
                    "themeVariables": { "darkMode": true }
                })
            );
        }

        let compiler = crate::diagram_theme::DiagramThemeCompiler::new();
        let first = compiler
            .compile(DiagramThemeSpec::new().with_mermaid_compatibility(root_then_variable))
            .unwrap();
        let second = compiler
            .compile(DiagramThemeSpec::new().with_mermaid_compatibility(variable_then_root))
            .unwrap();
        assert_eq!(first.recipe_fingerprint(), second.recipe_fingerprint());
    }

    #[test]
    fn conflicting_dark_mode_inputs_cannot_be_constructed() {
        let variable_error = MermaidThemeCompatibility::default()
            .with_dark_mode(true)
            .unwrap()
            .with_variable("darkMode", false)
            .unwrap_err();
        assert_eq!(
            variable_error,
            ThemeCompileValidationError::InvalidValue {
                field: "mermaid.theme_variables.darkMode"
            }
        );

        let root_error = MermaidThemeCompatibility::default()
            .with_variable("darkMode", false)
            .unwrap()
            .with_dark_mode(true)
            .unwrap_err();
        assert_eq!(
            root_error,
            ThemeCompileValidationError::InvalidValue {
                field: "mermaid.darkMode"
            }
        );

        let non_boolean_error = MermaidThemeCompatibility::default()
            .with_variable("darkMode", "true")
            .unwrap_err();
        assert_eq!(
            non_boolean_error,
            ThemeCompileValidationError::InvalidValue {
                field: "mermaid.theme_variables.darkMode"
            }
        );
    }

    #[test]
    fn non_canonical_dark_mode_state_still_fails_closed_during_theme_compilation() {
        let compatibility = MermaidThemeCompatibility {
            theme: None,
            dark_mode: Some(true),
            variables: BTreeMap::from([(
                "darkMode".to_string(),
                MermaidThemeValue::Boolean(false),
            )]),
        };
        let error = crate::diagram_theme::DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_mermaid_compatibility(compatibility))
            .unwrap_err();
        assert!(matches!(
            error,
            crate::diagram_theme::ThemeCompileError::Validation(
                ThemeCompileValidationError::InvalidValue {
                    field: "mermaid.theme_variables.darkMode"
                }
            )
        ));
    }

    #[test]
    fn compatibility_numbers_must_be_finite_and_bounded() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1_000_000_001.0] {
            assert!(
                MermaidThemeCompatibility::default()
                    .with_variable("fontSize", value)
                    .is_err(),
                "{value:?} must fail closed"
            );
        }
    }

    #[test]
    fn compatibility_values_reject_resource_and_host_css_references() {
        for value in [
            "url(https://example.test/theme.svg#paint)",
            r"u\72l(//example.test/theme.svg#paint)",
            r"linear-gradient(red, u\72l('../theme.svg#paint'))",
            "url(../theme.svg#paint)",
            "data:image/svg+xml;base64,AAAA",
            "file:///tmp/theme.svg",
            "https://example.test/theme.css",
            "../theme.css",
            "var(--host-theme-color)",
            r"v\61r(--host-theme-color)",
            "env(safe-area-inset-top)",
            "attr(data-theme color)",
            "src(local-font.woff2)",
            "expression(alert(1))",
            "rgba(0, 0, 0, 0.2",
        ] {
            assert!(
                MermaidThemeCompatibility::default()
                    .with_variable("primaryColor", value)
                    .is_err(),
                "{value:?} must fail closed"
            );
        }
    }
}
