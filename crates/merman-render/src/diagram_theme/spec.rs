use std::collections::BTreeMap;

use merman_core::MermaidConfig;
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
    theme: Option<String>,
    dark_mode: Option<bool>,
    variables: BTreeMap<String, String>,
}

impl MermaidThemeCompatibility {
    pub fn with_theme(
        mut self,
        theme: impl Into<String>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.theme = Some(validate_identifier(theme.into(), "mermaid.theme")?);
        Ok(self)
    }

    pub fn with_dark_mode(mut self, dark_mode: bool) -> Self {
        self.dark_mode = Some(dark_mode);
        self
    }

    pub fn with_variable(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, ThemeCompileValidationError> {
        let key = validate_identifier(key.into(), "mermaid.theme_variables.key")?;
        let value = validate_compatibility_value(value.into(), "mermaid.theme_variables.value")?;
        self.variables.insert(key, value);
        Ok(self)
    }

    pub fn theme(&self) -> Option<&str> {
        self.theme.as_deref()
    }

    pub const fn dark_mode(&self) -> Option<bool> {
        self.dark_mode
    }

    pub fn variables(&self) -> impl ExactSizeIterator<Item = (&str, &str)> {
        self.variables
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    pub(crate) fn to_mermaid_config(&self) -> MermaidConfig {
        let mut root = Map::new();
        if let Some(theme) = &self.theme {
            root.insert("theme".to_string(), Value::String(theme.clone()));
        }
        if let Some(dark_mode) = self.dark_mode {
            root.insert("darkMode".to_string(), Value::Bool(dark_mode));
        }
        if !self.variables.is_empty() {
            root.insert(
                "themeVariables".to_string(),
                Value::Object(
                    self.variables
                        .iter()
                        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
                        .collect(),
                ),
            );
        }
        MermaidConfig::from_value(Value::Object(root))
    }

    pub(crate) fn validate(&self) -> Result<(), ThemeCompileValidationError> {
        if let Some(theme) = &self.theme {
            validate_identifier(theme.clone(), "mermaid.theme")?;
        }
        if self.variables.len() > 512 {
            return Err(ThemeCompileValidationError::LimitExceeded {
                field: "mermaid.theme_variables",
            });
        }
        for (key, value) in &self.variables {
            validate_identifier(key.clone(), "mermaid.theme_variables.key")?;
            validate_compatibility_value(value.clone(), "mermaid.theme_variables.value")?;
        }
        Ok(())
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
        self.effects.validate()
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
    if value.is_empty()
        || value.len() > 1024
        || value.chars().any(|character| {
            character.is_control() || matches!(character, '<' | '>' | '{' | '}' | ';')
        })
    {
        return Err(ThemeCompileValidationError::InvalidValue { field });
    }
    Ok(value)
}
