use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::resources::InputResourcePolicy;
use crate::{OperationControl, OperationControlResult};

use super::recipe::is_theme_compatibility_identifier;
use super::validation::ConfigConstructionBudget;
use super::{PresentationConfigError, ThemeCompatibilityRecipe};

/// Conceptual Mermaid compatibility input, independent of its surviving physical assignments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeCompatibilityFieldKind {
    Theme,
    DarkMode,
    Variable,
}

/// One declared conceptual field and its exact surviving physical paths.
///
/// A derived path can survive after the original input is shadowed. Different conceptual fields
/// can also own the same derived path. Neither relationship authenticates source history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeCompatibilityField {
    opaque_id: Arc<str>,
    kind: ThemeCompatibilityFieldKind,
    surviving_assignment_paths: Arc<[Arc<str>]>,
}

impl ThemeCompatibilityField {
    /// Checks the field vocabulary, path domain, uniqueness, and construction budget.
    ///
    /// The containing config additionally checks the normalized recipe association and the
    /// presence of surviving assignments. Path order is retained exactly as declared.
    pub fn try_new_controlled(
        opaque_id: Arc<str>,
        kind: ThemeCompatibilityFieldKind,
        surviving_assignment_paths: Arc<[Arc<str>]>,
        policy: &InputResourcePolicy,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Self, PresentationConfigError>> {
        let field = Self {
            opaque_id,
            kind,
            surviving_assignment_paths,
        };
        let mut budget = ConfigConstructionBudget::new(policy, control);
        if let Err(error) = field.charge(&mut budget)? {
            return Ok(Err(error));
        }
        if let Err(error) = field.validate_shape(control)? {
            return Ok(Err(error));
        }
        control.checkpoint()?;
        Ok(Ok(field))
    }

    pub fn opaque_id(&self) -> &str {
        &self.opaque_id
    }

    pub const fn kind(&self) -> ThemeCompatibilityFieldKind {
        self.kind
    }

    pub fn surviving_assignment_paths(&self) -> impl ExactSizeIterator<Item = &str> {
        self.surviving_assignment_paths.iter().map(AsRef::as_ref)
    }

    pub fn shared_assignment_paths(&self) -> &Arc<[Arc<str>]> {
        &self.surviving_assignment_paths
    }

    /// Records one exact surviving physical path accounted for by a family consumer.
    pub fn consume_assignment_path(
        &self,
        assignment_path: &str,
        disposition: ThemeCompatibilityConsumptionDisposition,
    ) -> Option<ThemeCompatibilityFieldConsumption> {
        self.surviving_assignment_paths
            .iter()
            .find(|path| path.as_ref() == assignment_path)
            .map(|path| ThemeCompatibilityFieldConsumption {
                field_opaque_id: Arc::clone(&self.opaque_id),
                assignment_path: Arc::clone(path),
                disposition,
            })
    }

    pub fn consume_all(
        &self,
        disposition: ThemeCompatibilityConsumptionDisposition,
    ) -> impl ExactSizeIterator<Item = ThemeCompatibilityFieldConsumption> + '_ {
        self.surviving_assignment_paths
            .iter()
            .map(move |path| ThemeCompatibilityFieldConsumption {
                field_opaque_id: Arc::clone(&self.opaque_id),
                assignment_path: Arc::clone(path),
                disposition,
            })
    }

    pub(super) fn charge(
        &self,
        budget: &mut ConfigConstructionBudget<'_>,
    ) -> OperationControlResult<Result<(), PresentationConfigError>> {
        if let Err(error) = budget.charge(1, self.opaque_id.len(), 0)? {
            return Ok(Err(error));
        }
        for path in self.surviving_assignment_paths.iter() {
            if let Err(error) = budget.charge(1, path.len(), 0)? {
                return Ok(Err(error));
            }
        }
        Ok(Ok(()))
    }

    fn validate_shape(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(), PresentationConfigError>> {
        control.checkpoint()?;
        if !self.has_valid_identity() {
            return Ok(Err(PresentationConfigError::InvalidFieldIdentity {
                field: Arc::clone(&self.opaque_id),
                kind: self.kind,
            }));
        }
        if self.surviving_assignment_paths.is_empty() {
            return Ok(Err(PresentationConfigError::EmptyFieldPaths {
                field: Arc::clone(&self.opaque_id),
            }));
        }
        let mut seen = BTreeSet::new();
        for path in self.surviving_assignment_paths.iter() {
            control.checkpoint()?;
            let original_root = match self.kind {
                ThemeCompatibilityFieldKind::Theme => path.as_ref() == "theme",
                ThemeCompatibilityFieldKind::DarkMode => path.as_ref() == "darkMode",
                ThemeCompatibilityFieldKind::Variable => false,
            };
            if !original_root
                && !path
                    .strip_prefix("themeVariables.")
                    .is_some_and(|path| path.split('.').all(is_theme_compatibility_identifier))
            {
                return Ok(Err(PresentationConfigError::InvalidAssignmentPath {
                    field: Arc::clone(&self.opaque_id),
                    path: Arc::clone(path),
                }));
            }
            if !seen.insert(path.as_ref()) {
                return Ok(Err(PresentationConfigError::DuplicateAssignmentPath {
                    field: Arc::clone(&self.opaque_id),
                    path: Arc::clone(path),
                }));
            }
        }
        Ok(Ok(()))
    }

    fn has_valid_identity(&self) -> bool {
        match self.kind {
            ThemeCompatibilityFieldKind::Theme => self.opaque_id.as_ref() == "mermaid.theme",
            ThemeCompatibilityFieldKind::DarkMode => self.opaque_id.as_ref() == "mermaid.darkMode",
            ThemeCompatibilityFieldKind::Variable => self
                .variable_key()
                .is_some_and(|key| key != "darkMode" && is_theme_compatibility_identifier(key)),
        }
    }

    fn variable_key(&self) -> Option<&str> {
        self.opaque_id.strip_prefix("mermaid.themeVariables.")
    }

    pub(super) fn is_declared_by(&self, recipe: &ThemeCompatibilityRecipe) -> bool {
        let values = recipe.compatibility_config().as_value();
        match self.kind {
            ThemeCompatibilityFieldKind::Theme => values.get("theme").is_some(),
            ThemeCompatibilityFieldKind::DarkMode => values.get("darkMode").is_some(),
            ThemeCompatibilityFieldKind::Variable => self.variable_key().is_some_and(|key| {
                values
                    .get("themeVariables")
                    .and_then(|variables| variables.get(key))
                    .is_some()
            }),
        }
    }
}

/// Why a family adapter can retire one compatibility assignment path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeCompatibilityConsumptionDisposition {
    ReplacedByTypedSurface,
    NotReadByFamily,
}

/// One path-level consumption event tied to its conceptual field identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeCompatibilityFieldConsumption {
    field_opaque_id: Arc<str>,
    assignment_path: Arc<str>,
    disposition: ThemeCompatibilityConsumptionDisposition,
}

impl ThemeCompatibilityFieldConsumption {
    pub fn field_opaque_id(&self) -> &str {
        &self.field_opaque_id
    }

    pub fn assignment_path(&self) -> &str {
        &self.assignment_path
    }

    pub const fn disposition(&self) -> ThemeCompatibilityConsumptionDisposition {
        self.disposition
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeCompatibilityReconciliation {
    remaining_field_count: usize,
    consumed_field_count: usize,
}

impl ThemeCompatibilityReconciliation {
    pub const fn remaining_field_count(self) -> usize {
        self.remaining_field_count
    }

    pub const fn consumed_field_count(self) -> usize {
        self.consumed_field_count
    }
}

/// Shared frozen recipe and residual fields, without any claim of a parse origin.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeCompatibilityEvidence {
    recipe: Option<ThemeCompatibilityRecipe>,
    mermaid_fields: Arc<[ThemeCompatibilityField]>,
}

impl ThemeCompatibilityEvidence {
    pub(super) fn new(
        recipe: Option<ThemeCompatibilityRecipe>,
        mermaid_fields: Arc<[ThemeCompatibilityField]>,
    ) -> Self {
        Self {
            recipe,
            mermaid_fields,
        }
    }

    pub fn recipe(&self) -> Option<&ThemeCompatibilityRecipe> {
        self.recipe.as_ref()
    }

    pub fn matches_recipe(&self, recipe: Option<&ThemeCompatibilityRecipe>) -> bool {
        self.recipe.as_ref() == recipe
    }

    pub fn mermaid_residual_count(&self) -> usize {
        self.mermaid_fields.len()
    }

    pub fn mermaid_residual_path_survives(&self, dotted_path: &str) -> bool {
        self.mermaid_fields.iter().any(|field| {
            field
                .surviving_assignment_paths()
                .any(|path| path == dotted_path)
        })
    }

    pub fn mermaid_theme_field_survives(&self) -> bool {
        self.mermaid_fields
            .iter()
            .any(|field| field.kind() == ThemeCompatibilityFieldKind::Theme)
    }

    pub fn mermaid_dark_mode_field_survives(&self) -> bool {
        self.mermaid_fields
            .iter()
            .any(|field| field.kind() == ThemeCompatibilityFieldKind::DarkMode)
    }

    pub fn mermaid_fields(&self) -> impl ExactSizeIterator<Item = &ThemeCompatibilityField> {
        self.mermaid_fields.iter()
    }

    pub fn shared_fields(&self) -> &Arc<[ThemeCompatibilityField]> {
        &self.mermaid_fields
    }

    /// Reconciles actual consumer events against conceptual fields.
    ///
    /// Identical duplicates are idempotent. Conflicting dispositions fail closed. A conceptual
    /// field is consumed only when every surviving physical path is covered.
    pub fn reconcile_mermaid_consumptions<'a>(
        &self,
        consumptions: impl IntoIterator<Item = &'a ThemeCompatibilityFieldConsumption>,
    ) -> ThemeCompatibilityReconciliation {
        let mut coverage =
            BTreeMap::<(&str, &str), Option<ThemeCompatibilityConsumptionDisposition>>::new();
        for consumption in consumptions {
            let key = (
                consumption.field_opaque_id.as_ref(),
                consumption.assignment_path.as_ref(),
            );
            match coverage.entry(key) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(Some(consumption.disposition));
                }
                std::collections::btree_map::Entry::Occupied(mut entry)
                    if entry.get() != &Some(consumption.disposition) =>
                {
                    entry.insert(None);
                }
                std::collections::btree_map::Entry::Occupied(_) => {}
            }
        }

        let consumed_field_count = self
            .mermaid_fields
            .iter()
            .filter(|field| {
                field.surviving_assignment_paths().all(|path| {
                    coverage
                        .get(&(field.opaque_id(), path))
                        .is_some_and(Option::is_some)
                })
            })
            .count();
        ThemeCompatibilityReconciliation {
            remaining_field_count: self
                .mermaid_fields
                .len()
                .saturating_sub(consumed_field_count),
            consumed_field_count,
        }
    }
}
