use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::Value;

use crate::config::values_equal_controlled;
use crate::resources::InputResourcePolicy;
use crate::{MermaidConfig, OperationControl, OperationControlResult};

use super::validation::ConfigConstructionBudget;
use super::{
    ErPaintInputs, FamilyPaintInputs, GitGraphPaintInputs, PresentationConfigError,
    ThemeCompatibilityEvidence, ThemeCompatibilityField, ThemeCompatibilityRecipe,
};

/// Declared authority over materialized palette paints, independent of typography and geometry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PalettePaintAuthority {
    #[default]
    PropertyInputs,
    SelectedPalette,
}

/// Unvalidated frozen declarations, accepted only by the checked config constructor.
///
/// This type has no configuration consumer API. Supplying it declares output facts and does not
/// establish that a source operation, detector, or theme evaluator produced those facts.
#[derive(Debug, Clone)]
pub struct FrozenPresentationConfigParts {
    values: MermaidConfig,
    explicit_config_paths: Arc<BTreeSet<Arc<str>>>,
    recipe: Option<ThemeCompatibilityRecipe>,
    residual_fields: Arc<[ThemeCompatibilityField]>,
    palette_paint_authority: PalettePaintAuthority,
    family_paint_inputs: Option<FamilyPaintInputs>,
}

impl FrozenPresentationConfigParts {
    pub fn new(values: MermaidConfig) -> Self {
        Self {
            values,
            explicit_config_paths: Arc::new(BTreeSet::new()),
            recipe: None,
            residual_fields: Arc::from([]),
            palette_paint_authority: PalettePaintAuthority::PropertyInputs,
            family_paint_inputs: None,
        }
    }

    /// Retains exact dotted ownership spellings, including empty segments and the root owner.
    ///
    /// Paths describe assignments, so they need not resolve to a present scalar. A parent scalar
    /// can own a queried descendant, and source assignments can outlive a later value removal.
    /// Literal dotted or empty keys retain the source contract's dotted-path interpretation.
    pub fn with_explicit_config_paths(mut self, paths: Arc<BTreeSet<Arc<str>>>) -> Self {
        self.explicit_config_paths = paths;
        self
    }

    pub fn with_theme_compatibility(
        mut self,
        recipe: Option<ThemeCompatibilityRecipe>,
        residual_fields: Arc<[ThemeCompatibilityField]>,
    ) -> Self {
        self.recipe = recipe;
        self.residual_fields = residual_fields;
        self
    }

    pub fn with_palette_paint_authority(mut self, authority: PalettePaintAuthority) -> Self {
        self.palette_paint_authority = authority;
        self
    }

    pub fn with_family_paint_inputs(mut self, inputs: Option<FamilyPaintInputs>) -> Self {
        self.family_paint_inputs = inputs;
        self
    }
}

/// Immutable resolved configuration with explicit output ownership declarations.
///
/// Equality includes all ownership and recipe facts. Values can be equal while their declared
/// presentation behavior differs. No mutation journal or source initialization state is retained.
#[derive(Debug, Clone)]
pub struct PresentationConfig {
    values: MermaidConfig,
    explicit_config_paths: Arc<BTreeSet<Arc<str>>>,
    theme_compatibility: ThemeCompatibilityEvidence,
    palette_paint_authority: PalettePaintAuthority,
    family_paint_inputs: Option<FamilyPaintInputs>,
}

impl PartialEq for PresentationConfig {
    fn eq(&self, other: &Self) -> bool {
        self.explicit_config_paths == other.explicit_config_paths
            && self.theme_compatibility == other.theme_compatibility
            && self.palette_paint_authority == other.palette_paint_authority
            && self.family_paint_inputs == other.family_paint_inputs
            && matches!(
                values_equal_controlled(self.as_value(), other.as_value(), None),
                Ok(true)
            )
    }
}

impl Eq for PresentationConfig {}

impl Default for PresentationConfig {
    fn default() -> Self {
        Self::empty_object()
    }
}

impl PresentationConfig {
    pub fn empty_object() -> Self {
        Self {
            values: MermaidConfig::empty_object(),
            explicit_config_paths: Arc::new(BTreeSet::new()),
            theme_compatibility: ThemeCompatibilityEvidence::default(),
            palette_paint_authority: PalettePaintAuthority::PropertyInputs,
            family_paint_inputs: None,
        }
    }

    /// Freezes ordinary caller-authored values, owning their provided leaves and containers.
    ///
    /// Empty root objects assign nothing. Other empty containers, arrays, and scalars are actual
    /// supplied values. This route infers neither compatibility residuals nor palette authority.
    /// Construction work is bounded before deriving the ownership index.
    pub fn from_config_controlled(
        values: MermaidConfig,
        policy: &InputResourcePolicy,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Self, PresentationConfigError>> {
        let mut budget = ConfigConstructionBudget::new(policy, control);
        if let Err(error) = budget.charge_json(values.as_value())? {
            return Ok(Err(error));
        }
        let explicit_config_paths = match authored_paths(values.as_value(), &mut budget)? {
            Ok(paths) => paths,
            Err(error) => return Ok(Err(error)),
        };
        control.checkpoint()?;
        Ok(Ok(Self {
            values,
            explicit_config_paths: Arc::new(explicit_config_paths),
            theme_compatibility: ThemeCompatibilityEvidence::default(),
            palette_paint_authority: PalettePaintAuthority::PropertyInputs,
            family_paint_inputs: None,
        }))
    }

    /// Checks explicit frozen data without reconstructing or asserting a producer's history.
    ///
    /// The budget covers values, the complete recipe, and all ownership declarations before any
    /// derived validation index is allocated. Explicit path spellings, field/path order, and the
    /// distinction between an absent and a present empty family mask are preserved.
    pub fn try_from_frozen_parts_controlled(
        parts: FrozenPresentationConfigParts,
        policy: &InputResourcePolicy,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Self, PresentationConfigError>> {
        let mut budget = ConfigConstructionBudget::new(policy, control);
        if let Err(error) = budget.charge_json(parts.values.as_value())? {
            return Ok(Err(error));
        }
        if let Some(recipe) = &parts.recipe
            && let Err(error) = budget.charge_json(recipe.compatibility_config().as_value())?
        {
            return Ok(Err(error));
        }
        for path in parts.explicit_config_paths.iter() {
            if let Err(error) = budget.charge(1, path.len(), 0)? {
                return Ok(Err(error));
            }
        }
        for field in parts.residual_fields.iter() {
            if let Err(error) = field.charge(&mut budget)? {
                return Ok(Err(error));
            }
        }
        if parts.recipe.is_none() && !parts.residual_fields.is_empty() {
            return Ok(Err(PresentationConfigError::MissingThemeRecipe));
        }
        if parts.recipe.is_none() && parts.family_paint_inputs.is_some() {
            return Ok(Err(PresentationConfigError::MissingFamilyPaintRecipe));
        }

        let mut seen = BTreeSet::new();
        for field in parts.residual_fields.iter() {
            control.checkpoint()?;
            if !seen.insert(field.opaque_id()) {
                return Ok(Err(PresentationConfigError::DuplicateThemeField {
                    field: Arc::from(field.opaque_id()),
                }));
            }
            if let Some(recipe) = &parts.recipe
                && !field.is_declared_by(recipe)
            {
                return Ok(Err(PresentationConfigError::FieldAbsentFromRecipe {
                    field: Arc::from(field.opaque_id()),
                }));
            }
            for path in field.shared_assignment_paths().iter() {
                control.checkpoint()?;
                if config_value_at(parts.values.as_value(), path).is_none() {
                    return Ok(Err(PresentationConfigError::MissingAssignmentValue {
                        path: Arc::clone(path),
                    }));
                }
            }
        }
        drop(seen);
        control.checkpoint()?;
        Ok(Ok(Self {
            values: parts.values,
            explicit_config_paths: parts.explicit_config_paths,
            theme_compatibility: ThemeCompatibilityEvidence::new(
                parts.recipe,
                parts.residual_fields,
            ),
            palette_paint_authority: parts.palette_paint_authority,
            family_paint_inputs: parts.family_paint_inputs,
        }))
    }

    pub fn as_config(&self) -> &MermaidConfig {
        &self.values
    }

    pub fn as_value(&self) -> &Value {
        self.values.as_value()
    }

    pub fn get_str(&self, dotted_path: &str) -> Option<&str> {
        self.values.get_str(dotted_path)
    }

    pub fn get_bool(&self, dotted_path: &str) -> Option<bool> {
        self.values.get_bool(dotted_path)
    }

    pub fn is_empty_object(&self) -> bool {
        self.values.is_empty_object()
    }

    pub fn clone_value_bounded_controlled(
        &self,
        max_retained_bytes: usize,
        max_nesting_depth: usize,
        control: &OperationControl,
    ) -> OperationControlResult<Option<Value>> {
        self.values
            .clone_value_bounded_controlled(max_retained_bytes, max_nesting_depth, control)
    }

    pub fn theme_compatibility(&self) -> &ThemeCompatibilityEvidence {
        &self.theme_compatibility
    }

    pub fn explicit_config_paths(&self) -> impl ExactSizeIterator<Item = &str> {
        self.explicit_config_paths.iter().map(AsRef::as_ref)
    }

    pub fn shared_explicit_config_paths(&self) -> &Arc<BTreeSet<Arc<str>>> {
        &self.explicit_config_paths
    }

    pub const fn palette_paint_authority(&self) -> PalettePaintAuthority {
        self.palette_paint_authority
    }

    pub const fn family_paint_inputs(&self) -> Option<FamilyPaintInputs> {
        self.family_paint_inputs
    }

    pub const fn gitgraph_paint_inputs(&self) -> Option<GitGraphPaintInputs> {
        match self.family_paint_inputs {
            Some(FamilyPaintInputs::GitGraph(inputs)) => Some(inputs),
            Some(FamilyPaintInputs::Er(_)) | None => None,
        }
    }

    pub const fn er_paint_inputs(&self) -> Option<ErPaintInputs> {
        match self.family_paint_inputs {
            Some(FamilyPaintInputs::Er(inputs)) => Some(inputs),
            Some(FamilyPaintInputs::GitGraph(_)) | None => None,
        }
    }

    /// Clones only shared handles when exporting the exact frozen declarations for revalidation.
    pub fn frozen_parts(&self) -> FrozenPresentationConfigParts {
        FrozenPresentationConfigParts {
            values: self.values.clone(),
            explicit_config_paths: Arc::clone(&self.explicit_config_paths),
            recipe: self.theme_compatibility.recipe().cloned(),
            residual_fields: Arc::clone(self.theme_compatibility.shared_fields()),
            palette_paint_authority: self.palette_paint_authority,
            family_paint_inputs: self.family_paint_inputs,
        }
    }

    pub fn config_path_overrides_typed_default(&self, dotted_path: &str) -> bool {
        self.explicit_config_owns_path(dotted_path)
            || self
                .theme_compatibility
                .mermaid_fields()
                .flat_map(ThemeCompatibilityField::surviving_assignment_paths)
                .any(|candidate| dotted_paths_overlap(candidate, dotted_path))
    }

    /// Palette authority affects present theme paints without claiming typography or geometry.
    pub fn config_paint_overrides_typed_default(&self, dotted_path: &str) -> bool {
        self.config_path_overrides_typed_default(dotted_path)
            || (self.palette_paint_authority == PalettePaintAuthority::SelectedPalette
                && dotted_path.starts_with("themeVariables.")
                && config_value_at(self.as_value(), dotted_path).is_some())
    }

    /// Resolves exact/ancestor/descendant ownership without scanning unrelated declarations.
    pub fn explicit_config_owns_path(&self, dotted_path: &str) -> bool {
        if self.explicit_config_paths.is_empty() {
            return false;
        }
        if dotted_path.is_empty() {
            return true;
        }
        if self.explicit_config_paths.contains("")
            || self.explicit_config_paths.contains(dotted_path)
        {
            return true;
        }
        for (index, _) in dotted_path.match_indices('.') {
            if self.explicit_config_paths.contains(&dotted_path[..index]) {
                return true;
            }
        }
        let prefix = format!("{dotted_path}.");
        self.explicit_config_paths
            .range::<str, _>((
                std::ops::Bound::Included(prefix.as_str()),
                std::ops::Bound::Unbounded,
            ))
            .next()
            .is_some_and(|candidate| candidate.starts_with(&prefix))
    }
}

fn authored_paths(
    root: &Value,
    budget: &mut ConfigConstructionBudget<'_>,
) -> OperationControlResult<Result<BTreeSet<Arc<str>>, PresentationConfigError>> {
    let mut paths = BTreeSet::new();
    let Value::Object(root) = root else {
        if let Err(error) = budget.charge(1, 0, 0)? {
            return Ok(Err(error));
        }
        paths.insert(Arc::from(""));
        return Ok(Ok(paths));
    };

    let mut path = String::new();
    let mut pending = vec![(root.iter(), 0usize, false)];
    while let Some((mut values, prefix_len, has_parent)) = pending.pop() {
        budget.checkpoint()?;
        if let Some((key, value)) = values.next() {
            path.truncate(prefix_len);
            let Some(path_len) = prefix_len
                .checked_add(usize::from(has_parent))
                .and_then(|length| length.checked_add(key.len()))
            else {
                return Ok(Err(PresentationConfigError::SizeOverflow));
            };
            if let Err(error) = budget.charge(1, path_len, 0)? {
                return Ok(Err(error));
            }
            if has_parent {
                path.push('.');
            }
            path.push_str(key);
            pending.push((values, prefix_len, has_parent));
            match value {
                Value::Object(children) if !children.is_empty() => {
                    pending.push((children.iter(), path.len(), true));
                }
                _ => {
                    paths.insert(Arc::from(path.as_str()));
                }
            }
        }
    }
    Ok(Ok(paths))
}

fn config_value_at<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(root);
    }
    path.split('.')
        .try_fold(root, |value, segment| value.as_object()?.get(segment))
}

fn dotted_paths_overlap(left: &str, right: &str) -> bool {
    left.is_empty()
        || right.is_empty()
        || left == right
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('.'))
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('.'))
}
