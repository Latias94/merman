//! Source installation and cached initialization for shared theme recipes.

use std::fmt;
use std::sync::{Arc, OnceLock};

use super::{MermaidEngine, SiteConfigInitialization, SourcePresentationConfig};
use crate::MermaidConfig;
use merman_model::presentation::{ThemeCompatibilityRecipe, ThemeCompatibilityRecipeError};

/// Invalid bounded Mermaid compatibility input for an installed source theme.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(transparent)]
pub struct ThemeCompatibilityPlanError(#[from] ThemeCompatibilityRecipeError);

/// Source installation state for a compiled theme, including its initialization cache.
#[derive(Clone)]
pub struct ThemeCompatibilityPlan {
    recipe: ThemeCompatibilityRecipe,
    initialization: Arc<OnceLock<SiteConfigInitialization>>,
    compatibility_config: SourcePresentationConfig,
}

impl fmt::Debug for ThemeCompatibilityPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThemeCompatibilityPlan")
            .field("recipe", &self.recipe)
            .finish_non_exhaustive()
    }
}

impl ThemeCompatibilityPlan {
    pub fn try_new(
        recipe_identity: [u8; 32],
        compatibility_config: MermaidConfig,
    ) -> Result<Self, ThemeCompatibilityPlanError> {
        let recipe = ThemeCompatibilityRecipe::try_new(recipe_identity, compatibility_config)?;
        Ok(Self::from_recipe(recipe))
    }

    /// Installs an already normalized shared recipe without a second recipe definition.
    pub fn from_recipe(recipe: ThemeCompatibilityRecipe) -> Self {
        let compatibility_config =
            SourcePresentationConfig::from_theme_compatibility_recipe(recipe.clone());
        Self {
            recipe,
            initialization: Arc::new(OnceLock::new()),
            compatibility_config,
        }
    }

    pub fn recipe(&self) -> &ThemeCompatibilityRecipe {
        &self.recipe
    }
}

impl ThemeCompatibilityPlan {
    /// Installs one compiled theme's complete compatibility plan on a source engine.
    pub(super) fn install(&self, mut engine: MermaidEngine) -> MermaidEngine {
        engine.theme_compatibility_config = Some(self.compatibility_config.clone());
        if engine.site_config_overrides.is_pristine_empty_object() {
            let initialized = self
                .initialization
                .get_or_init(|| {
                    SiteConfigInitialization::new(
                        Some(self.compatibility_config.clone()),
                        &engine.site_config_overrides,
                    )
                })
                .for_pristine_install(&engine.site_config_overrides);
            engine.site_config = initialized.site_config;
            engine.initialization_inputs = initialized.initialization_inputs;
            engine.default_effective_config = initialized.default_effective_config;
        } else {
            engine.rebuild_site_config();
        }
        engine
    }
}
