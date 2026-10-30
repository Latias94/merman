use super::{
    MermaidConfig, config_values_equal, drop_value_nonrecursive,
    mirror_legacy_font_family_into_theme_variables_value, replace_value_nonrecursive,
};
#[cfg(test)]
use super::{measure_config_work, record_config_work};
use crate::{OperationControl, OperationControlResult};
use merman_model::json::clone_value_nonrecursive;
use merman_model::presentation::{
    ErPaintInputs, FamilyPaintInputs, FrozenPresentationConfigParts, GitGraphPaintInputs,
    PalettePaintAuthority, PresentationConfig, PresentationConfigError, ThemeCompatibilityField,
    ThemeCompatibilityFieldKind, ThemeCompatibilityRecipe,
};
use merman_model::resources::{InputResourcePolicy, ModelComplexity, ResourceProfile};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

mod paint_inputs;

/// Source-owned configuration construction with presentation ownership and detector history.
///
/// Completed presentation data is exported through [`Self::into_presentation_config_controlled`].
/// The shared immutable configuration never retains this builder's mutation journal.
#[derive(Clone)]
pub struct SourcePresentationConfig {
    inputs: MermaidConfig,
    theme_compatibility: Option<ThemeCompatibilityState>,
    source_theme_palette: bool,
    explicit_config_paths: Arc<BTreeSet<Arc<str>>>,
    mutation_paths: Arc<BTreeMap<Arc<str>, MutationStamp>>,
    mutation_revision: u64,
}

// Only detector checkpoints advance this clock. Ordinary setters perform a read, not a
// contended increment, while writes to an older clone still acquire the current epoch.
static DETECTOR_EPOCH: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MutationStamp {
    // Zero denotes a framework write or a completed operation's sealed assignment.
    detector_epoch: u64,
    revision: u64,
}

impl fmt::Debug for SourcePresentationConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SourcePresentationConfig")
            .field(self.as_value())
            .finish()
    }
}

impl PartialEq for SourcePresentationConfig {
    fn eq(&self, other: &Self) -> bool {
        self.inputs == other.inputs
    }
}

impl Default for SourcePresentationConfig {
    fn default() -> Self {
        Self::empty_object()
    }
}

impl SourcePresentationConfig {
    pub fn empty_object() -> Self {
        Self::from_config(MermaidConfig::empty_object())
    }

    pub fn from_value(value: Value) -> Self {
        Self::from_config(MermaidConfig::from_value(value))
    }

    /// Starts an unbound presentation lifecycle from raw configuration values.
    pub fn from_config(inputs: MermaidConfig) -> Self {
        Self {
            inputs,
            theme_compatibility: None,
            source_theme_palette: false,
            explicit_config_paths: Arc::new(BTreeSet::new()),
            mutation_paths: Arc::new(BTreeMap::new()),
            mutation_revision: 0,
        }
    }

    pub fn as_config(&self) -> &MermaidConfig {
        &self.inputs
    }

    pub fn as_value(&self) -> &Value {
        self.inputs.as_value()
    }

    pub fn get_str(&self, dotted_path: &str) -> Option<&str> {
        self.inputs.get_str(dotted_path)
    }

    pub fn get_bool(&self, dotted_path: &str) -> Option<bool> {
        self.inputs.get_bool(dotted_path)
    }

    pub fn clone_value_bounded_controlled(
        &self,
        max_retained_bytes: usize,
        max_nesting_depth: usize,
        control: &OperationControl,
    ) -> OperationControlResult<Option<Value>> {
        self.inputs
            .clone_value_bounded_controlled(max_retained_bytes, max_nesting_depth, control)
    }

    pub(crate) fn is_empty_object(&self) -> bool {
        self.inputs.is_empty_object()
    }

    pub(crate) fn is_pristine_empty_object(&self) -> bool {
        self.is_empty_object()
            && self.theme_compatibility.is_none()
            && self.explicit_config_paths.is_empty()
            && self.mutation_paths.is_empty()
            && self.mutation_revision == 0
    }

    pub(crate) fn refresh_initialization_mutations_after(&mut self, before: &Self) {
        // Initialization deep-merges record their unique paths in BTreeSet order. Retain that
        // order and read the detector clock separately for every original public assignment.
        for stamp in Arc::make_mut(&mut self.mutation_paths).values_mut() {
            if stamp.revision > before.mutation_revision {
                stamp.detector_epoch = DETECTOR_EPOCH.load(Ordering::Relaxed);
            }
        }
    }

    pub(crate) fn inherit_initialization_mutation_epochs(&mut self, initialized: &Self) {
        // Theme calculation keeps inherited assignments but can overwrite a path with a later
        // framework revision. Only the original assignments inherit the installation's epochs.
        for (path, stamp) in Arc::make_mut(&mut self.mutation_paths) {
            if let Some(inherited) = initialized.mutation_paths.get(path)
                && stamp.revision == inherited.revision
            {
                stamp.detector_epoch = inherited.detector_epoch;
            }
        }
    }

    pub(crate) fn has_tracking_theme_compatibility(&self) -> bool {
        matches!(
            self.theme_compatibility,
            Some(ThemeCompatibilityState::Tracking(_))
        )
    }

    #[cfg(test)]
    pub(crate) fn theme_parse_binding(&self) -> Option<&ThemeCompatibilityRecipe> {
        match self.theme_compatibility.as_ref() {
            Some(ThemeCompatibilityState::Frozen(ownership)) => Some(&ownership.binding),
            Some(ThemeCompatibilityState::Tracking(_)) | None => None,
        }
    }

    /// Returns explicit Mermaid compatibility fields still owned by the parsed theme.
    #[cfg(test)]
    pub(crate) fn mermaid_compatibility_residual_count(&self) -> usize {
        match self.theme_compatibility.as_ref() {
            Some(ThemeCompatibilityState::Frozen(ownership)) => ownership
                .fields
                .iter()
                .filter(|field| field.paths.iter().any(|path| path.owned))
                .count(),
            Some(ThemeCompatibilityState::Tracking(_)) | None => 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn mermaid_compatibility_fields(&self) -> Option<Arc<[ThemeCompatibilityField]>> {
        match self.theme_compatibility.as_ref() {
            Some(ThemeCompatibilityState::Frozen(ownership)) => Some(
                ownership
                    .freeze_fields_controlled(
                        &source_config_construction_policy(),
                        &OperationControl::new(),
                    )
                    .unwrap()
                    .unwrap(),
            ),
            Some(ThemeCompatibilityState::Tracking(_)) | None => None,
        }
    }

    pub(crate) fn from_theme_compatibility_recipe(binding: ThemeCompatibilityRecipe) -> Self {
        let ownership = ThemeCompatibilityOwnership::from_config(
            binding.clone(),
            binding.compatibility_config().as_value(),
        );
        Self {
            inputs: binding.compatibility_config().clone(),
            theme_compatibility: Some(ThemeCompatibilityState::Tracking(Arc::new(ownership))),
            source_theme_palette: false,
            explicit_config_paths: Arc::new(BTreeSet::new()),
            mutation_paths: Arc::new(BTreeMap::new()),
            mutation_revision: 0,
        }
    }

    /// Imports exact frozen declarations as sealed source construction state.
    ///
    /// Only surviving fields and paths are imported. Replaying the complete recipe here would
    /// resurrect shadowed assignments and reinterpret historical setters as detector events.
    pub fn from_frozen_controlled(
        config: PresentationConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Self> {
        control.checkpoint()?;
        let theme_compatibility = if let Some(binding) = config.theme_compatibility().recipe() {
            let mut fields = Vec::new();
            for field in config.theme_compatibility().mermaid_fields() {
                control.checkpoint()?;
                let mut paths = Vec::new();
                for path in field.shared_assignment_paths().iter() {
                    control.checkpoint()?;
                    paths.push(ThemeCompatibilityPathOwnership {
                        path: Arc::clone(path),
                        owned: true,
                    });
                }
                fields.push(ThemeCompatibilityFieldOwnership {
                    opaque_id: Arc::from(field.opaque_id()),
                    kind: field.kind(),
                    paths,
                });
            }
            Some(ThemeCompatibilityState::Frozen(Arc::new(
                ThemeCompatibilityOwnership {
                    binding: binding.clone(),
                    fields,
                    family_paint_inputs: config.family_paint_inputs(),
                },
            )))
        } else {
            None
        };
        control.checkpoint()?;
        Ok(Self {
            inputs: config.as_config().clone(),
            theme_compatibility,
            source_theme_palette: config.palette_paint_authority()
                == PalettePaintAuthority::SelectedPalette,
            explicit_config_paths: Arc::clone(config.shared_explicit_config_paths()),
            mutation_paths: Arc::new(BTreeMap::new()),
            mutation_revision: 0,
        })
    }

    /// Freezes checked immutable presentation data and discards all source mutation state.
    pub fn into_presentation_config_controlled(
        mut self,
        policy: &InputResourcePolicy,
        control: &OperationControl,
    ) -> OperationControlResult<std::result::Result<PresentationConfig, PresentationConfigError>>
    {
        control.checkpoint()?;
        self.freeze_theme_compatibility();
        let (recipe, fields, family_paint_inputs) = match &self.theme_compatibility {
            Some(ThemeCompatibilityState::Frozen(ownership)) => {
                let fields = match ownership.freeze_fields_controlled(policy, control)? {
                    Ok(fields) => fields,
                    Err(error) => return Ok(Err(error)),
                };
                (
                    Some(ownership.binding.clone()),
                    fields,
                    ownership.family_paint_inputs,
                )
            }
            Some(ThemeCompatibilityState::Tracking(_)) | None => (None, Arc::from([]), None),
        };
        let palette = if self.source_theme_palette {
            PalettePaintAuthority::SelectedPalette
        } else {
            PalettePaintAuthority::PropertyInputs
        };
        let parts = FrozenPresentationConfigParts::new(self.inputs)
            .with_explicit_config_paths(self.explicit_config_paths)
            .with_theme_compatibility(recipe, fields)
            .with_palette_paint_authority(palette)
            .with_family_paint_inputs(family_paint_inputs);
        PresentationConfig::try_from_frozen_parts_controlled(parts, policy, control)
    }

    #[cfg(test)]
    pub(crate) fn gitgraph_paint_inputs(&self) -> Option<GitGraphPaintInputs> {
        match &self.theme_compatibility {
            Some(ThemeCompatibilityState::Frozen(ownership)) => match ownership.family_paint_inputs
            {
                Some(FamilyPaintInputs::GitGraph(inputs)) => Some(inputs),
                _ => None,
            },
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn er_paint_inputs(&self) -> Option<ErPaintInputs> {
        match &self.theme_compatibility {
            Some(ThemeCompatibilityState::Frozen(ownership)) => match ownership.family_paint_inputs
            {
                Some(FamilyPaintInputs::Er(inputs)) => Some(inputs),
                _ => None,
            },
            _ => None,
        }
    }

    pub(crate) fn adopt_tracking_theme_compatibility_from(&mut self, source: &mut Self) {
        let Some(state @ ThemeCompatibilityState::Tracking(_)) = source.theme_compatibility.take()
        else {
            return;
        };
        self.theme_compatibility = Some(state);
    }

    pub(crate) fn retain_theme_compatibility_paths_applied_after(
        &mut self,
        target: &Self,
        before: &Self,
    ) {
        let Some(ThemeCompatibilityState::Tracking(ownership)) = self.theme_compatibility.as_mut()
        else {
            return;
        };
        for field in &mut Arc::make_mut(ownership).fields {
            for path in &mut field.paths {
                if path.owned && !target.path_was_mutated_after(before, &path.path) {
                    path.owned = false;
                }
            }
        }
    }

    pub(crate) fn retain_normalized_theme_compatibility_variables(
        &mut self,
        normalized: &Map<String, Value>,
    ) {
        let Some(ThemeCompatibilityState::Tracking(ownership)) = self.theme_compatibility.as_mut()
        else {
            return;
        };
        for field in &mut Arc::make_mut(ownership).fields {
            for path in &mut field.paths {
                let Some(key) = path.path.strip_prefix("themeVariables.") else {
                    continue;
                };
                if path.owned && !normalized.contains_key(key) {
                    path.owned = false;
                }
            }
        }
    }

    pub(crate) fn freeze_theme_compatibility(&mut self) {
        let Some(ThemeCompatibilityState::Tracking(ownership)) = self.theme_compatibility.as_ref()
        else {
            return;
        };
        self.theme_compatibility = Some(ThemeCompatibilityState::Frozen(Arc::clone(ownership)));
    }
    #[cfg(test)]
    pub(crate) fn config_path_overrides_typed_default(&self, dotted_path: &str) -> bool {
        self.explicit_config_owns_path(dotted_path)
            || matches!(
                self.theme_compatibility.as_ref(),
                Some(ThemeCompatibilityState::Frozen(ownership))
                    if ownership.fields
                        .iter()
                        .flat_map(|field| field.paths.iter())
                        .any(|candidate| candidate.owned && dotted_paths_overlap(&candidate.path, dotted_path))
            )
    }

    /// Paint from an authored theme includes its derived palette, not just raw variables.
    /// Typography and geometry continue to use their property-specific input ownership.
    #[cfg(test)]
    pub(crate) fn config_paint_overrides_typed_default(&self, dotted_path: &str) -> bool {
        self.config_path_overrides_typed_default(dotted_path)
            || (self.source_theme_palette
                && dotted_path.starts_with("themeVariables.")
                && config_value_at(self.as_value(), dotted_path).is_some())
    }

    pub(crate) fn set_source_theme_palette(&mut self, source_selected: bool) {
        self.source_theme_palette = source_selected;
    }

    pub(crate) fn theme_variable_has_calculation_owner(&self, variable: &str) -> bool {
        let source_path = format!("themeVariables.{variable}");
        if self.explicit_config_owns_path(&source_path) {
            return true;
        }
        // Theme calculation precedes freezing. Read the same surviving physical inputs that
        // propagation can extend, without changing the renderer's frozen-admission predicate.
        match self.theme_compatibility.as_ref() {
            Some(
                ThemeCompatibilityState::Tracking(ownership)
                | ThemeCompatibilityState::Frozen(ownership),
            ) => ownership.fields.iter().any(|field| {
                field
                    .paths
                    .iter()
                    .any(|path| path.owned && path.path.as_ref() == source_path)
            }),
            None => false,
        }
    }

    pub(crate) fn has_theme_variable_calculation_owners(&self) -> bool {
        if self.explicit_config_paths.contains("")
            || self.explicit_config_paths.contains("themeVariables")
            || self
                .explicit_config_paths
                .range::<str, _>((
                    std::ops::Bound::Included("themeVariables."),
                    std::ops::Bound::Unbounded,
                ))
                .next()
                .is_some_and(|path| path.starts_with("themeVariables."))
        {
            return true;
        }
        match self.theme_compatibility.as_ref() {
            Some(
                ThemeCompatibilityState::Tracking(ownership)
                | ThemeCompatibilityState::Frozen(ownership),
            ) => ownership.fields.iter().any(|field| {
                field
                    .paths
                    .iter()
                    .any(|path| path.owned && path.path.starts_with("themeVariables."))
            }),
            None => false,
        }
    }

    pub(crate) fn propagate_theme_variable_ownership(&mut self, source: &str, target: &str) {
        let source_path = format!("themeVariables.{source}");
        let target_path: Arc<str> = Arc::from(format!("themeVariables.{target}"));

        // Derived ownership follows exact calculation inputs. A parent `themeVariables` owner
        // must not implicitly claim every variable produced by a theme program.
        if self.explicit_config_paths.contains(source_path.as_str()) {
            Arc::make_mut(&mut self.explicit_config_paths).insert(Arc::clone(&target_path));
        }

        match self.theme_compatibility.as_mut() {
            Some(ThemeCompatibilityState::Tracking(ownership)) => {
                for field in &mut Arc::make_mut(ownership).fields {
                    let owns_source = field
                        .paths
                        .iter()
                        .any(|path| path.owned && path.path.as_ref() == source_path.as_str());
                    if owns_source
                        && !field
                            .paths
                            .iter()
                            .any(|path| path.path.as_ref() == target_path.as_ref())
                    {
                        field.paths.push(ThemeCompatibilityPathOwnership {
                            path: Arc::clone(&target_path),
                            owned: true,
                        });
                    }
                }
            }
            Some(ThemeCompatibilityState::Frozen(ownership)) => {
                for field in &mut Arc::make_mut(ownership).fields {
                    if !field
                        .paths
                        .iter()
                        .any(|path| path.owned && path.path.as_ref() == source_path.as_str())
                    {
                        continue;
                    }
                    // The old frozen representation had already removed shadowed paths. An
                    // actual later calculation could therefore restore this derived target.
                    if let Some(path) = field.paths.iter_mut().find(|path| path.path == target_path)
                    {
                        path.owned = true;
                    } else {
                        field.paths.push(ThemeCompatibilityPathOwnership {
                            path: Arc::clone(&target_path),
                            owned: true,
                        });
                    }
                }
            }
            None => {}
        }
    }

    pub(crate) fn path_was_mutated_after(&self, before: &Self, dotted_path: &str) -> bool {
        self.mutation_paths.iter().any(|(candidate, revision)| {
            before
                .mutation_paths
                .get(candidate)
                .is_none_or(|before_revision| revision.revision > before_revision.revision)
                && dotted_paths_overlap(candidate.as_ref(), dotted_path)
        })
    }

    pub(crate) fn explicit_config_owns_path(&self, dotted_path: &str) -> bool {
        if self.explicit_config_paths.is_empty() {
            return false;
        }
        if dotted_path.is_empty() {
            return !self.explicit_config_paths.is_empty();
        }
        let owns = |path: &str| {
            #[cfg(test)]
            super::record_config_work(|work| work.explicit_path_candidates += 1);
            self.explicit_config_paths.contains(path)
        };
        if owns("") || owns(dotted_path) {
            return true;
        }
        for (index, _) in dotted_path.match_indices('.') {
            if owns(&dotted_path[..index]) {
                return true;
            }
        }
        // Descendants are contiguous in lexical order. Starting at the dotted prefix
        // avoids unrelated keys such as `a-ignored` hiding an owned `a.child`.
        let prefix = format!("{dotted_path}.");
        #[cfg(test)]
        super::record_config_work(|work| work.explicit_path_candidates += 1);
        self.explicit_config_paths
            .range::<str, _>((
                std::ops::Bound::Included(prefix.as_str()),
                std::ops::Bound::Unbounded,
            ))
            .next()
            .is_some_and(|candidate| candidate.starts_with(&prefix))
    }

    fn replace_detector_value(&mut self, path: &str, value: Option<&Value>) {
        let segments = if path.is_empty() {
            Vec::new()
        } else {
            path.split('.').collect()
        };
        self.replace_detector_segments(&segments, value);
    }

    fn replace_detector_segments(&mut self, path: &[&str], value: Option<&Value>) {
        let Some((key, parents)) = path.split_last() else {
            if let Some(value) = value {
                replace_value_nonrecursive(
                    self.inputs.as_value_mut(),
                    clone_value_nonrecursive(value),
                );
            }
            return;
        };
        if let Some(value) = value {
            let mut parent = self.inputs.as_value_mut();
            for segment in parents {
                if !parent.is_object() {
                    replace_value_nonrecursive(parent, Value::Object(Map::new()));
                }
                parent = parent
                    .as_object_mut()
                    .expect("object parent")
                    .entry((*segment).to_owned())
                    .or_insert(Value::Null);
            }
            if !parent.is_object() {
                replace_value_nonrecursive(parent, Value::Object(Map::new()));
            }
            if let Some(removed) = parent
                .as_object_mut()
                .expect("object parent")
                .insert((*key).to_owned(), clone_value_nonrecursive(value))
            {
                drop_value_nonrecursive(removed);
            }
        } else {
            let parent = parents
                .iter()
                .try_fold(self.inputs.as_value_mut(), |value, segment| {
                    value.as_object_mut()?.get_mut(*segment)
                });
            if let Some(parent) = parent.and_then(Value::as_object_mut)
                && let Some(removed) = parent.remove(*key)
            {
                drop_value_nonrecursive(removed);
            }
        }
    }

    fn mark_explicit_config_path(&mut self, dotted_path: &str) {
        Arc::make_mut(&mut self.explicit_config_paths).insert(Arc::from(dotted_path));
    }

    pub fn as_value_mut(&mut self) -> &mut Value {
        self.shadow_theme_compatibility_path("");
        self.record_mutation("", true);
        self.inputs.as_value_mut()
    }

    pub fn set_value(&mut self, dotted_path: &str, value: Value) {
        self.shadow_theme_compatibility_path(dotted_path);
        self.record_mutation(dotted_path, true);
        self.inputs.set_value(dotted_path, value);
    }

    pub(crate) fn set_value_explicit(&mut self, dotted_path: &str, value: Value) {
        self.shadow_theme_compatibility_path(dotted_path);
        self.record_mutation(dotted_path, false);
        self.inputs.set_value(dotted_path, value);
        self.mark_explicit_config_path(dotted_path);
    }

    pub(crate) fn set_value_preserving_theme_compatibility(
        &mut self,
        dotted_path: &str,
        value: Value,
    ) {
        self.record_mutation(dotted_path, false);
        self.inputs.set_value(dotted_path, value);
    }

    pub(crate) fn commit_legacy_theme_variables(&mut self, variables: Map<String, Value>) {
        // Legacy resolution used to publish normalized inputs before the final variables.
        // Preserve both framework mutation revisions for detector replay, but publish only the
        // complete result after every fallible calculation has succeeded.
        self.record_mutation("themeVariables", false);
        self.set_value_preserving_theme_compatibility("themeVariables", Value::Object(variables));
    }

    pub fn deep_merge(&mut self, other: &Value) {
        self.deep_merge_with_explicit_ownership(other, false);
    }

    pub(crate) fn deep_merge_explicit(&mut self, other: &Value) {
        self.deep_merge_with_explicit_ownership(other, true);
    }

    fn deep_merge_with_explicit_ownership(&mut self, other: &Value, explicit: bool) {
        let mut mutation_paths = BTreeSet::new();
        self.inputs
            .merge_with_mutation_observer(other, &mut |segments| {
                mutation_paths.insert(segments.join("."));
            });
        for path in mutation_paths {
            self.shadow_theme_compatibility_path(&path);
            self.record_mutation(&path, !explicit);
            if explicit {
                self.mark_explicit_config_path(&path);
            }
        }
    }

    pub(crate) fn mirror_legacy_font_family(&mut self) {
        mirror_legacy_font_family_into_theme_variables_value(self.inputs.as_value_mut());
    }

    pub(crate) fn seal_detector_mutations(&mut self) {
        // A completed nested operation carries its existing explicit metadata, not setter events
        // that an enclosing detector can reinterpret as its own assignments.
        if self
            .mutation_paths
            .values()
            .any(|stamp| stamp.detector_epoch != 0)
        {
            for stamp in Arc::make_mut(&mut self.mutation_paths).values_mut() {
                stamp.detector_epoch = 0;
            }
        }
    }

    fn record_mutation(&mut self, dotted_path: &str, public_assignment: bool) {
        self.mutation_revision = self
            .mutation_revision
            .checked_add(1)
            .expect("configuration mutation revision exhausted");
        Arc::make_mut(&mut self.mutation_paths).insert(
            Arc::from(dotted_path),
            MutationStamp {
                detector_epoch: if public_assignment {
                    DETECTOR_EPOCH.load(Ordering::Relaxed)
                } else {
                    0
                },
                revision: self.mutation_revision,
            },
        );
    }

    fn shadow_theme_compatibility_path(&mut self, dotted_path: &str) {
        match self.theme_compatibility.as_mut() {
            Some(ThemeCompatibilityState::Tracking(ownership)) => {
                Arc::make_mut(ownership).shadow_path(dotted_path);
            }
            Some(ThemeCompatibilityState::Frozen(_)) => {
                self.theme_compatibility = None;
            }
            None => {}
        }
    }
}

pub(crate) fn apply_hardened_site_policy(config: &mut SourcePresentationConfig) {
    config.set_value(
        "secure",
        Value::Array(
            super::HARDENED_SECURE_KEYS
                .iter()
                .map(|key| Value::String((*key).to_string()))
                .collect(),
        ),
    );
}

pub(crate) struct DetectorConfigCheckpoint {
    before: SourcePresentationConfig,
    epoch: u64,
}

#[derive(Debug)]
pub(crate) struct DetectorConfigChanges {
    before: Option<SourcePresentationConfig>,
    after: SourcePresentationConfig,
    explicit_paths: Vec<Arc<str>>,
    transferred_explicit_ownership: bool,
}

impl DetectorConfigCheckpoint {
    pub(crate) fn new(config: &SourcePresentationConfig) -> Self {
        let epoch = DETECTOR_EPOCH
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |epoch| {
                epoch.checked_add(1)
            })
            .expect("detector mutation epoch exhausted")
            + 1;
        Self {
            before: config.clone(),
            epoch,
        }
    }

    pub(crate) fn finish(
        self,
        after: &mut SourcePresentationConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Option<DetectorConfigChanges>> {
        let transferred_explicit_ownership =
            self.before.explicit_config_paths != after.explicit_config_paths;
        let explicit_paths = after
            .mutation_paths
            .iter()
            .filter(|(_, stamp)| stamp.detector_epoch >= self.epoch)
            .map(|(path, _)| Arc::clone(path))
            .collect::<Vec<_>>();
        if !explicit_paths.is_empty() {
            Arc::make_mut(&mut after.explicit_config_paths).extend(explicit_paths.iter().cloned());
        }
        let values_changed = !std::ptr::eq(self.before.inputs.as_value(), after.inputs.as_value())
            && !config_values_equal(self.before.as_value(), after.as_value(), Some(control))?;
        if !values_changed && !transferred_explicit_ownership && explicit_paths.is_empty() {
            return Ok(None);
        }
        Ok(Some(DetectorConfigChanges {
            before: values_changed.then_some(self.before),
            after: after.clone(),
            explicit_paths,
            transferred_explicit_ownership,
        }))
    }
}

impl DetectorConfigChanges {
    #[cfg(test)]
    pub(crate) fn explicit_paths(&self) -> &[Arc<str>] {
        &self.explicit_paths
    }

    pub(crate) fn claimed_paths(&self) -> impl Iterator<Item = &Arc<str>> {
        // Completed nested operations seal their setter events, but replacement still transfers
        // their explicit owners. Ordinary initialized config must not become detector claims.
        self.explicit_paths
            .iter()
            .filter(|_| !self.transferred_explicit_ownership)
            .chain(
                self.transferred_explicit_ownership
                    .then_some(&self.after.explicit_config_paths)
                    .into_iter()
                    .flat_map(|paths| paths.iter()),
            )
    }

    pub(crate) fn value_changed_at(&self, path: &str) -> bool {
        let Some(before) = &self.before else {
            return false;
        };
        match (
            config_value_at(before.as_value(), path),
            config_value_at(self.after.as_value(), path),
        ) {
            (Some(before), Some(after)) => !config_values_equal(before, after, None)
                .expect("comparison without operation control cannot cancel"),
            (None, None) => false,
            _ => true,
        }
    }

    pub(crate) fn replay(
        &self,
        target: &mut SourcePresentationConfig,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        // Whole-config replacement transfers its explicit metadata. Materialization must not
        // resurrect owners from the pre-detection host/source config; appearance is reapplied later.
        if self.transferred_explicit_ownership {
            target.explicit_config_paths = Arc::clone(&self.after.explicit_config_paths);
        }
        // Values alone do not acquire explicit ownership, but a changed compatibility binding
        // cannot retain its previous owner. Inspect exact bound paths, not structural parents.
        if let Some(ThemeCompatibilityState::Tracking(ownership)) = &mut target.theme_compatibility
        {
            for field in &mut Arc::make_mut(ownership).fields {
                for path in &mut field.paths {
                    control.checkpoint()?;
                    if path.owned && self.value_changed_at(&path.path) {
                        path.owned = false;
                    }
                }
            }
        }
        if let Some(before) = &self.before {
            enum Frame<'a> {
                Visit(Option<&'a Value>, Option<&'a Value>),
                Object {
                    before: &'a Map<String, Value>,
                    after: &'a Map<String, Value>,
                    before_entries: serde_json::map::Iter<'a>,
                    after_entries: serde_json::map::Iter<'a>,
                },
                LeaveChild,
            }
            let mut path = Vec::new();
            let mut pending = vec![Frame::Visit(
                Some(before.as_value()),
                Some(self.after.as_value()),
            )];
            while let Some(frame) = pending.pop() {
                control.checkpoint()?;
                #[cfg(test)]
                record_config_work(|work| {
                    work.detector_replay_steps += 1;
                    work.detector_replay_peak_frames =
                        work.detector_replay_peak_frames.max(pending.len() + 1);
                    work.detector_replay_peak_path_segments =
                        work.detector_replay_peak_path_segments.max(path.len());
                });
                match frame {
                    Frame::Visit(Some(Value::Object(before)), Some(Value::Object(after))) => {
                        pending.push(Frame::Object {
                            before,
                            after,
                            before_entries: before.iter(),
                            after_entries: after.iter(),
                        });
                    }
                    Frame::Visit(Some(before), Some(after))
                        if config_values_equal(before, after, Some(control))? => {}
                    Frame::Visit(None, None) => {}
                    Frame::Visit(_, after) => target.replace_detector_segments(&path, after),
                    Frame::Object {
                        before,
                        after,
                        mut before_entries,
                        mut after_entries,
                    } => {
                        let child = before_entries
                            .next()
                            .map(|(key, value)| (key.as_str(), Some(value), after.get(key)))
                            .or_else(|| {
                                after_entries
                                    .find(|(key, _)| !before.contains_key(*key))
                                    .map(|(key, value)| (key.as_str(), None, Some(value)))
                            });
                        if let Some((key, before_child, after_child)) = child {
                            pending.push(Frame::Object {
                                before,
                                after,
                                before_entries,
                                after_entries,
                            });
                            path.push(key);
                            pending.push(Frame::LeaveChild);
                            pending.push(Frame::Visit(before_child, after_child));
                        }
                    }
                    Frame::LeaveChild => {
                        path.pop();
                    }
                }
            }
        }
        // Rebuild both newly assigned and transferred explicit values, even when they matched
        // the old initialized theme. Sealed nested assignments are owners, not new events.
        for path in self.claimed_paths() {
            control.checkpoint()?;
            target.shadow_theme_compatibility_path(path);
            target.record_mutation(path, false);
            target.mark_explicit_config_path(path);
            target.replace_detector_value(path, config_value_at(self.after.as_value(), path));
        }
        Ok(())
    }
}

fn config_value_at<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(root);
    }
    path.split('.')
        .try_fold(root, |value, segment| value.as_object()?.get(segment))
}

#[derive(Debug, Clone)]
enum ThemeCompatibilityState {
    Tracking(Arc<ThemeCompatibilityOwnership>),
    // This source phase flag retains the exact source ownership graph. The canonical immutable
    // field representation is built once at the controlled data-export boundary.
    Frozen(Arc<ThemeCompatibilityOwnership>),
}

#[derive(Debug, Clone)]
struct ThemeCompatibilityOwnership {
    binding: ThemeCompatibilityRecipe,
    fields: Vec<ThemeCompatibilityFieldOwnership>,
    family_paint_inputs: Option<FamilyPaintInputs>,
}

#[derive(Debug, Clone)]
struct ThemeCompatibilityFieldOwnership {
    opaque_id: Arc<str>,
    kind: ThemeCompatibilityFieldKind,
    paths: Vec<ThemeCompatibilityPathOwnership>,
}

#[derive(Debug, Clone)]
struct ThemeCompatibilityPathOwnership {
    path: Arc<str>,
    owned: bool,
}

impl ThemeCompatibilityOwnership {
    fn from_config(binding: ThemeCompatibilityRecipe, config: &Value) -> Self {
        let mut fields = Vec::new();
        let Some(root) = config.as_object() else {
            return Self {
                binding,
                fields,
                family_paint_inputs: None,
            };
        };

        if root.contains_key("theme") {
            fields.push(ThemeCompatibilityFieldOwnership::new(
                ThemeCompatibilityFieldKind::Theme,
                ["theme"],
            ));
        }

        let variables = root.get("themeVariables").and_then(Value::as_object);
        if root.contains_key("darkMode")
            || variables.is_some_and(|variables| variables.contains_key("darkMode"))
        {
            fields.push(ThemeCompatibilityFieldOwnership::new(
                ThemeCompatibilityFieldKind::DarkMode,
                ["darkMode", "themeVariables.darkMode"],
            ));
        }

        if let Some(variables) = variables {
            fields.extend(
                variables
                    .keys()
                    .filter(|key| key.as_str() != "darkMode")
                    .map(|key| {
                        ThemeCompatibilityFieldOwnership::new(
                            ThemeCompatibilityFieldKind::Variable,
                            [format!("themeVariables.{key}")],
                        )
                    }),
            );
        }

        Self {
            binding,
            fields,
            family_paint_inputs: None,
        }
    }

    fn shadow_path(&mut self, dotted_path: &str) {
        for field in &mut self.fields {
            for path in &mut field.paths {
                if path.owned && dotted_paths_overlap(&path.path, dotted_path) {
                    path.owned = false;
                }
            }
        }
    }

    fn freeze_fields_controlled(
        &self,
        policy: &InputResourcePolicy,
        control: &OperationControl,
    ) -> OperationControlResult<
        std::result::Result<Arc<[ThemeCompatibilityField]>, PresentationConfigError>,
    > {
        // Bound the complete source ownership walk before creating per-field sorting indexes or
        // cloning path handles. Shadowed paths still cost work even though they are not exported.
        let mut work = ModelComplexity::default();
        for field in &self.fields {
            control.checkpoint()?;
            let Some(items) = work.items.checked_add(1) else {
                return Ok(Err(PresentationConfigError::SizeOverflow));
            };
            let Some(text_bytes) = work.text_bytes.checked_add(field.opaque_id.len()) else {
                return Ok(Err(PresentationConfigError::SizeOverflow));
            };
            work = ModelComplexity::new(items, text_bytes, 0);
            if let Err(error) = policy.check_model_complexity(work) {
                return Ok(Err(error.into()));
            }
            for path in &field.paths {
                control.checkpoint()?;
                let Some(items) = work.items.checked_add(1) else {
                    return Ok(Err(PresentationConfigError::SizeOverflow));
                };
                let Some(text_bytes) = work.text_bytes.checked_add(path.path.len()) else {
                    return Ok(Err(PresentationConfigError::SizeOverflow));
                };
                work = ModelComplexity::new(items, text_bytes, 0);
                if let Err(error) = policy.check_model_complexity(work) {
                    return Ok(Err(error.into()));
                }
            }
        }

        let mut fields = Vec::new();
        for field in &self.fields {
            control.checkpoint()?;
            let mut surviving_paths = BTreeSet::new();
            for path in &field.paths {
                control.checkpoint()?;
                if path.owned {
                    surviving_paths.insert(Arc::clone(&path.path));
                }
            }
            if surviving_paths.is_empty() {
                continue;
            }
            let paths = surviving_paths.into_iter().collect::<Vec<_>>().into();
            let field = match ThemeCompatibilityField::try_new_controlled(
                Arc::clone(&field.opaque_id),
                field.kind,
                paths,
                policy,
                control,
            )? {
                Ok(field) => field,
                Err(error) => return Ok(Err(error)),
            };
            fields.push(field);
        }
        control.checkpoint()?;
        Ok(Ok(fields.into()))
    }

    #[cfg(test)]
    fn freeze_fields(&self) -> Arc<[ThemeCompatibilityField]> {
        self.freeze_fields_controlled(
            &source_config_construction_policy(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap()
    }
}

impl ThemeCompatibilityFieldOwnership {
    fn new<I, P>(kind: ThemeCompatibilityFieldKind, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<Arc<str>>,
    {
        let paths = paths.into_iter().map(Into::into).collect::<Vec<Arc<str>>>();
        let primary_path = paths
            .first()
            .expect("theme compatibility conceptual fields must own at least one path");
        let opaque_id = match kind {
            ThemeCompatibilityFieldKind::Theme => Arc::from("mermaid.theme"),
            ThemeCompatibilityFieldKind::DarkMode => Arc::from("mermaid.darkMode"),
            ThemeCompatibilityFieldKind::Variable => Arc::from(format!("mermaid.{primary_path}")),
        };
        Self {
            opaque_id,
            kind,
            paths: paths
                .into_iter()
                .map(|path| ThemeCompatibilityPathOwnership { path, owned: true })
                .collect(),
        }
    }
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

/// Source parse entrypoints do not accept a model admission policy. Keep their existing lack of
/// an implicit config ceiling; construction remains iterative, checked, and cancellable. Hosts
/// supplying frozen data use the explicit policy accepted by the model constructor.
pub(crate) fn source_config_construction_policy() -> InputResourcePolicy {
    InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
}

pub(crate) fn presentation_config_error(error: PresentationConfigError) -> crate::Error {
    crate::InternalFailure::new(format!(
        "invalid frozen presentation configuration: {error}"
    ))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn freeze(config: SourcePresentationConfig) -> PresentationConfig {
        config
            .into_presentation_config_controlled(
                &source_config_construction_policy(),
                &OperationControl::new(),
            )
            .unwrap()
            .unwrap()
    }

    #[test]
    fn frozen_export_and_import_preserve_derived_residuals_without_replaying_recipe_history() {
        let recipe = ThemeCompatibilityRecipe::try_new(
            [0x73; 32],
            MermaidConfig::from_value(json!({"themeVariables": {"primaryColor": "#123456"}})),
        )
        .unwrap();
        let mut source = SourcePresentationConfig::from_theme_compatibility_recipe(recipe.clone());
        source.set_value_preserving_theme_compatibility("themeVariables.mainBkg", json!("#123456"));
        source.propagate_theme_variable_ownership("primaryColor", "mainBkg");
        let checkpoint = DetectorConfigCheckpoint::new(&source);
        source.set_value("themeVariables.primaryColor", json!("#123456"));
        checkpoint
            .finish(&mut source, &OperationControl::new())
            .unwrap();
        source.set_source_theme_palette(true);
        let (frozen, work) = measure_config_work(|| freeze(source));
        assert_eq!(work.json_cow_copies, 0, "{work:?}");
        assert_eq!(frozen.theme_compatibility().recipe(), Some(&recipe));
        assert_eq!(frozen.theme_compatibility().mermaid_residual_count(), 1);
        assert!(frozen.explicit_config_owns_path("themeVariables.primaryColor"));
        assert!(
            !frozen
                .theme_compatibility()
                .mermaid_residual_path_survives("themeVariables.primaryColor")
        );
        assert!(
            frozen
                .theme_compatibility()
                .mermaid_residual_path_survives("themeVariables.mainBkg")
        );

        let mut imported = SourcePresentationConfig::from_frozen_controlled(
            frozen.clone(),
            &OperationControl::new(),
        )
        .unwrap();
        assert!(imported.mutation_paths.is_empty());
        assert_eq!(imported.mutation_revision, 0);
        assert!(!imported.has_tracking_theme_compatibility());
        let checkpoint = DetectorConfigCheckpoint::new(&imported);
        assert!(
            checkpoint
                .finish(&mut imported, &OperationControl::new())
                .unwrap()
                .is_none()
        );
        assert_eq!(freeze(imported), frozen);
    }

    #[test]
    fn frozen_recalculation_can_restore_a_previously_shadowed_derived_target() {
        let recipe = ThemeCompatibilityRecipe::try_new(
            [0x74; 32],
            MermaidConfig::from_value(json!({"themeVariables": {"primaryColor": "#123456"}})),
        )
        .unwrap();
        let mut source = SourcePresentationConfig::from_theme_compatibility_recipe(recipe);
        source.set_value_preserving_theme_compatibility("themeVariables.mainBkg", json!("#123456"));
        source.propagate_theme_variable_ownership("primaryColor", "mainBkg");
        source.set_value("themeVariables.mainBkg", json!("#abcdef"));
        source.freeze_theme_compatibility();
        source.set_value_preserving_theme_compatibility("themeVariables.mainBkg", json!("#123456"));
        source.propagate_theme_variable_ownership("primaryColor", "mainBkg");
        let frozen = freeze(source);
        assert!(
            frozen
                .theme_compatibility()
                .mermaid_residual_path_survives("themeVariables.mainBkg")
        );
    }

    #[test]
    fn frozen_import_transfers_explicit_owners_without_inventing_detector_setters() {
        let mut source = SourcePresentationConfig::from_value(json!({"same": 7}));
        source.set_value_explicit("same", json!(7));
        let frozen = freeze(source);
        let original = SourcePresentationConfig::from_value(json!({"same": 7}));
        let checkpoint = DetectorConfigCheckpoint::new(&original);
        let mut replacement =
            SourcePresentationConfig::from_frozen_controlled(frozen, &OperationControl::new())
                .unwrap();
        let changes = checkpoint
            .finish(&mut replacement, &OperationControl::new())
            .unwrap()
            .unwrap();
        assert!(changes.explicit_paths().is_empty());
        assert!(!changes.value_changed_at("same"));
        assert_eq!(
            changes
                .claimed_paths()
                .map(AsRef::as_ref)
                .collect::<Vec<&str>>(),
            ["same"]
        );
    }

    #[test]
    fn cancelled_source_freeze_never_publishes_a_shared_config() {
        let recipe = ThemeCompatibilityRecipe::try_new(
            [0x75; 32],
            MermaidConfig::from_value(json!({"themeVariables": {"primaryColor": "#123456"}})),
        )
        .unwrap();
        let source = SourcePresentationConfig::from_theme_compatibility_recipe(recipe);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        assert!(
            source
                .into_presentation_config_controlled(&source_config_construction_policy(), &control)
                .is_err()
        );
        let control = OperationControl::new();
        control.cancel();
        assert!(
            SourcePresentationConfig::from_frozen_controlled(
                PresentationConfig::empty_object(),
                &control
            )
            .is_err()
        );
    }

    #[test]
    fn same_value_public_setter_keeps_detector_claims_and_shadows_without_json_copy() {
        let binding = ThemeCompatibilityRecipe::try_new(
            [0x71; 32],
            MermaidConfig::from_value(json!({
                "theme": "base",
                "themeVariables": {"primaryColor": "#123456", "secondaryColor": "#abcdef"},
            })),
        )
        .unwrap();
        let original = SourcePresentationConfig::from_theme_compatibility_recipe(binding);
        let mut working = original.clone();
        let checkpoint = DetectorConfigCheckpoint::new(&working);
        let (_, work) = measure_config_work(|| {
            working.set_value("themeVariables.primaryColor", json!("#123456"));
        });
        let changes = checkpoint
            .finish(&mut working, &OperationControl::new())
            .unwrap()
            .unwrap();
        assert_eq!(
            changes.explicit_paths(),
            &[Arc::<str>::from("themeVariables.primaryColor")]
        );
        assert!(!changes.value_changed_at("themeVariables.primaryColor"));
        assert!(working.explicit_config_owns_path("themeVariables.primaryColor"));

        let mut replayed = original.clone();
        replayed.set_value_preserving_theme_compatibility(
            "themeVariables.primaryColor",
            json!("#654321"),
        );
        changes
            .replay(&mut replayed, &OperationControl::new())
            .unwrap();
        assert_eq!(
            replayed.get_str("themeVariables.primaryColor"),
            Some("#123456")
        );
        assert!(replayed.explicit_config_owns_path("themeVariables.primaryColor"));
        working.freeze_theme_compatibility();
        let surviving = working.mermaid_compatibility_fields().unwrap();
        assert!(!surviving.iter().any(|field| {
            field
                .shared_assignment_paths()
                .as_ref()
                .iter()
                .any(|path| path.as_ref() == "themeVariables.primaryColor")
        }));
        assert!(surviving.iter().any(|field| {
            field
                .shared_assignment_paths()
                .as_ref()
                .iter()
                .any(|path| path.as_ref() == "themeVariables.secondaryColor")
        }));
        assert_eq!(working.as_value(), original.as_value());
        assert!(original.has_tracking_theme_compatibility());
        assert!(std::ptr::eq(
            working.inputs.as_value(),
            original.inputs.as_value()
        ));
        assert_eq!(work.json_cow_copies, 0, "{work:?}");
    }

    #[test]
    fn same_value_explicit_and_framework_setters_preserve_their_distinct_authority() {
        let binding = ThemeCompatibilityRecipe::try_new(
            [0x72; 32],
            MermaidConfig::from_value(json!({
                "theme": "base", "themeVariables": {"primaryColor": "#123456"},
            })),
        )
        .unwrap();
        let original = SourcePresentationConfig::from_theme_compatibility_recipe(binding);
        let mut explicit = original.clone();
        let mut framework = original.clone();
        let (_, work) = measure_config_work(|| {
            explicit.set_value_explicit("themeVariables.primaryColor", json!("#123456"));
            framework.set_value_preserving_theme_compatibility(
                "themeVariables.primaryColor",
                json!("#123456"),
            );
        });
        assert!(explicit.explicit_config_owns_path("themeVariables.primaryColor"));
        assert!(!framework.explicit_config_owns_path("themeVariables.primaryColor"));
        assert!(explicit.path_was_mutated_after(&original, "themeVariables.primaryColor"));
        assert!(framework.path_was_mutated_after(&original, "themeVariables.primaryColor"));
        explicit.freeze_theme_compatibility();
        framework.freeze_theme_compatibility();
        assert!(framework.config_path_overrides_typed_default("themeVariables.primaryColor"));
        assert_eq!(explicit.mermaid_compatibility_residual_count(), 1);
        assert_eq!(framework.mermaid_compatibility_residual_count(), 2);
        assert!(std::ptr::eq(
            explicit.inputs.as_value(),
            original.inputs.as_value()
        ));
        assert!(std::ptr::eq(
            framework.inputs.as_value(),
            original.inputs.as_value()
        ));
        assert_eq!(work.json_cow_copies, 0, "{work:?}");
    }

    #[test]
    fn same_value_setter_keeps_literal_empty_keys_and_empty_path_segments() {
        let original = SourcePresentationConfig::from_value(json!({
            "": 3, "a": {"": {"b": [1, 2]}}, "dot.key": 7, "dot": {"key": 8},
        }));
        let mut working = original.clone();
        let (_, work) = measure_config_work(|| {
            working.set_value("", json!(3));
            working.set_value("a..b", json!([1, 2]));
            working.set_value("dot.key", json!(8));
        });
        assert_eq!(working.as_value(), original.as_value());
        assert_eq!(working.as_value()["dot.key"], json!(7));
        assert!(working.path_was_mutated_after(&original, ""));
        assert!(working.path_was_mutated_after(&original, "a..b"));
        assert!(std::ptr::eq(
            working.inputs.as_value(),
            original.inputs.as_value()
        ));
        assert_eq!(work.json_cow_copies, 0, "{work:?}");

        for root in [json!(3), json!({"a": null}), json!({"a": [1]})] {
            let original = SourcePresentationConfig::from_value(root);
            let mut coerced = original.clone();
            let path = if original.as_value().is_object() {
                "a.b"
            } else {
                ""
            };
            let (_, work) = measure_config_work(|| coerced.set_value(path, Value::Null));
            let expected = if path.is_empty() {
                json!({"": null})
            } else {
                json!({"a": {"b": null}})
            };
            assert_eq!(coerced.as_value(), &expected);
            assert_eq!(work.json_cow_copies, 1, "{work:?}");
            assert_ne!(coerced.as_value(), original.as_value());
        }
    }

    #[test]
    fn detector_checkpoint_distinguishes_old_clone_setters_from_historical_journal() {
        let mut old = SourcePresentationConfig::from_value(json!({"custom": 7}));
        old.set_value("custom", json!(7));
        let mut before = old.clone();
        before.set_value("custom", json!(7));
        let checkpoint = DetectorConfigCheckpoint::new(&before);
        old.set_value("custom", json!(7));
        assert_eq!(old.mutation_revision, before.mutation_revision);
        let changes = checkpoint
            .finish(&mut old, &OperationControl::new())
            .unwrap()
            .unwrap();
        assert_eq!(
            changes
                .explicit_paths()
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<&str>>(),
            ["custom"]
        );
        assert!(!changes.value_changed_at("custom"));
        assert!(old.explicit_config_owns_path("custom"));

        let checkpoint = DetectorConfigCheckpoint::new(&before);
        assert!(
            checkpoint
                .finish(&mut before, &OperationControl::new())
                .unwrap()
                .is_none()
        );

        let same_without_journal = SourcePresentationConfig::from_value(json!({"custom": 7}));
        let checkpoint = DetectorConfigCheckpoint::new(&same_without_journal);
        assert!(
            checkpoint
                .finish(&mut before, &OperationControl::new())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn detector_epoch_survives_nested_detection_and_thread_handoff() {
        let mut config = SourcePresentationConfig::from_value(json!({"custom": 7}));
        let outer = DetectorConfigCheckpoint::new(&config);
        let inner = DetectorConfigCheckpoint::new(&config);
        config.set_value("custom", json!(7));
        assert!(
            inner
                .finish(&mut config, &OperationControl::new())
                .unwrap()
                .is_some()
        );
        assert!(
            outer
                .finish(&mut config, &OperationControl::new())
                .unwrap()
                .is_some()
        );

        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        {
            let checkpoint = DetectorConfigCheckpoint::new(&config);
            let mut config = std::thread::spawn(move || {
                config.set_value("custom", json!(7));
                config
            })
            .join()
            .unwrap();
            assert!(
                checkpoint
                    .finish(&mut config, &OperationControl::new())
                    .unwrap()
                    .is_some()
            );
        }
    }

    #[test]
    fn detector_checkpoint_does_not_promote_framework_or_completed_operation_events() {
        let mut config = SourcePresentationConfig::from_value(json!({"custom": 7}));
        let checkpoint = DetectorConfigCheckpoint::new(&config);
        config.set_value_preserving_theme_compatibility("custom", json!(7));
        assert!(
            checkpoint
                .finish(&mut config, &OperationControl::new())
                .unwrap()
                .is_none()
        );

        let checkpoint = DetectorConfigCheckpoint::new(&config);
        config.set_value("custom", json!(7));
        config.seal_detector_mutations();
        assert!(
            checkpoint
                .finish(&mut config, &OperationControl::new())
                .unwrap()
                .is_none()
        );

        let checkpoint = DetectorConfigCheckpoint::new(&config);
        config.set_value("custom", json!(7));
        let changes = checkpoint
            .finish(&mut config, &OperationControl::new())
            .unwrap()
            .unwrap();
        assert_eq!(changes.explicit_paths().len(), 1);
    }

    #[test]
    fn legacy_theme_commit_preserves_framework_mutation_order_and_detector_replay() {
        let raw = SourcePresentationConfig::from_value(json!({
            "themeVariables": {"nodeBkg": null, "primaryColor": "#123456"},
            "custom": 7,
        }));
        let normalized = json!({"primaryColor": "#123456"});
        let resolved = json!({"primaryColor": "#123456", "nodeBkg": "#123456"});
        let mut reference = raw.clone();
        let reference_checkpoint = DetectorConfigCheckpoint::new(&reference);
        reference.set_value_preserving_theme_compatibility("themeVariables", normalized);
        reference.set_value_preserving_theme_compatibility("themeVariables", resolved.clone());
        reference.set_value("custom", json!(7));
        let reference_changes = reference_checkpoint
            .finish(&mut reference, &OperationControl::new())
            .unwrap()
            .unwrap();

        let mut committed = raw.clone();
        let committed_checkpoint = DetectorConfigCheckpoint::new(&committed);
        committed.commit_legacy_theme_variables(resolved.as_object().unwrap().clone());
        committed.set_value("custom", json!(7));
        let committed_changes = committed_checkpoint
            .finish(&mut committed, &OperationControl::new())
            .unwrap()
            .unwrap();

        assert_eq!(committed.as_value(), reference.as_value());
        assert_eq!(committed.mutation_revision, reference.mutation_revision);
        assert_eq!(
            committed.mutation_paths["themeVariables"],
            reference.mutation_paths["themeVariables"]
        );
        assert_eq!(
            committed_changes.explicit_paths(),
            reference_changes.explicit_paths()
        );
        assert_eq!(
            committed_changes.explicit_paths(),
            &[Arc::<str>::from("custom")]
        );

        let mut expected_target = raw.clone();
        let mut actual_target = raw;
        reference_changes
            .replay(&mut expected_target, &OperationControl::new())
            .unwrap();
        committed_changes
            .replay(&mut actual_target, &OperationControl::new())
            .unwrap();
        assert_eq!(actual_target.as_value(), expected_target.as_value());
        assert_eq!(
            actual_target.explicit_config_paths,
            expected_target.explicit_config_paths
        );
        assert!(actual_target.explicit_config_owns_path("custom"));
        assert!(!actual_target.explicit_config_owns_path("themeVariables.nodeBkg"));
    }

    #[test]
    fn replacement_replay_handles_literal_keys_deletions_and_only_actual_value_changes() {
        let before = SourcePresentationConfig::from_value(json!({
            "nested": {"deleted": 1}, "same": 2, "literal.key": 3,
        }));
        let checkpoint = DetectorConfigCheckpoint::new(&before);
        let mut after = SourcePresentationConfig::from_value(json!({"same": 2, "literal.key": 4}));
        let changes = checkpoint
            .finish(&mut after, &OperationControl::new())
            .unwrap()
            .unwrap();
        assert!(changes.value_changed_at("nested.deleted"));
        assert!(!changes.value_changed_at("nested.previouslyMissing"));
        assert!(!changes.value_changed_at("same"));
        let mut target = SourcePresentationConfig::from_value(json!({
            "nested": {"deleted": 1}, "same": 99, "literal.key": 3,
        }));
        changes
            .replay(&mut target, &OperationControl::new())
            .unwrap();
        assert_eq!(target.as_value(), &json!({"same": 99, "literal.key": 4}));
        assert!(!target.explicit_config_owns_path("nested"));
        assert!(!target.explicit_config_owns_path("literal.key"));

        let control = OperationControl::new();
        control.cancel();
        assert!(changes.replay(&mut target, &control).is_err());
    }

    #[test]
    fn detector_replay_deep_chain_and_comb_keep_linear_traversal_storage() {
        fn nested(depth: usize, comb: bool, leaf: i64) -> Value {
            let mut value = Value::from(leaf);
            for _ in 0..depth {
                let mut object = Map::new();
                object.insert("child".to_owned(), value);
                if comb {
                    object.insert("sibling".to_owned(), Value::from(7));
                }
                value = Value::Object(object);
            }
            value
        }

        for comb in [false, true] {
            let depth = 20_000;
            let before = SourcePresentationConfig::from_value(nested(depth, comb, 1));
            let mut target = SourcePresentationConfig::from_value(nested(depth, comb, 1));
            let mut after = SourcePresentationConfig::from_value(nested(depth, comb, 2));
            let checkpoint = DetectorConfigCheckpoint::new(&before);
            let changes = checkpoint
                .finish(&mut after, &OperationControl::new())
                .unwrap()
                .unwrap();
            let (result, work) =
                measure_config_work(|| changes.replay(&mut target, &OperationControl::new()));
            result.unwrap();
            assert!(config_values_equal(target.as_value(), after.as_value(), None).unwrap());
            assert!(work.detector_replay_steps <= 9 * depth + 3, "{work:?}");
            assert!(
                work.detector_replay_peak_frames <= 2 * depth + 2,
                "{work:?}"
            );
            assert!(work.detector_replay_peak_path_segments <= depth, "{work:?}");

            let control = OperationControl::new();
            control.cancel_after_checkpoints(20);
            let (result, work) = measure_config_work(|| changes.replay(&mut target, &control));
            assert!(result.is_err());
            assert!(work.detector_replay_steps <= 20, "{work:?}");
        }
    }

    #[test]
    fn dotted_path_overlap_is_symmetric_for_parent_and_child_paths() {
        assert!(dotted_paths_overlap(
            "themeVariables",
            "themeVariables.darkMode"
        ));
        assert!(dotted_paths_overlap(
            "themeVariables.darkMode",
            "themeVariables"
        ));
        assert!(!dotted_paths_overlap(
            "themeVariables.darkMode",
            "themeVariables.useGradient"
        ));
    }

    #[test]
    fn dark_mode_ownership_remains_one_concept_when_one_mirrored_path_survives() {
        let compatibility = json!({
            "darkMode": true,
            "themeVariables": {
                "darkMode": true
            }
        });
        let binding = ThemeCompatibilityRecipe::try_new(
            [0; 32],
            MermaidConfig::from_value(clone_value_nonrecursive(&compatibility)),
        )
        .expect("valid mirrored dark-mode compatibility");
        let mut ownership = ThemeCompatibilityOwnership::from_config(binding, &compatibility);

        ownership.shadow_path("darkMode");

        let fields = ownership.freeze_fields();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].kind(), ThemeCompatibilityFieldKind::DarkMode);
        assert_eq!(
            fields[0].shared_assignment_paths().as_ref(),
            &[Arc::<str>::from("themeVariables.darkMode")]
        );
    }

    #[test]
    fn explicit_ownership_uses_effective_assign_with_depth_paths() {
        let mut config = SourcePresentationConfig::from_value(json!({
            "flowchart": {
                "nodeSpacing": 50
            },
            "retainedScalar": "site"
        }));

        config.deep_merge_explicit(&json!({
            "flowchart": {
                "nodeSpacing": 50
            },
            "retainedScalar": {
                "child": true
            }
        }));

        assert!(config.explicit_config_owns_path("flowchart.nodeSpacing"));
        assert!(config.explicit_config_owns_path("flowchart"));
        assert!(config.explicit_config_owns_path("flowchart.nodeSpacing.unmaterializedDescendant"));
        assert!(!config.explicit_config_owns_path("flowchart.rankSpacing"));
        assert!(!config.explicit_config_owns_path("retainedScalar.child"));
    }

    #[test]
    fn explicit_ownership_lookup_preserves_dotted_path_boundaries() {
        let paths = [
            "", "a", "a.", "a.b", "a..b", "a-b", "a/b", "a.b.c", "ab", "é.葉", "é",
        ];
        for owner in paths {
            let mut config = SourcePresentationConfig::empty_object();
            Arc::make_mut(&mut config.explicit_config_paths).insert(Arc::from(owner));
            for query in paths {
                assert_eq!(
                    config.explicit_config_owns_path(query),
                    dotted_paths_overlap(owner, query),
                    "owner={owner:?}, query={query:?}"
                );
            }
        }
        let empty = SourcePresentationConfig::empty_object();
        assert!(!empty.explicit_config_owns_path(""));
        let mut config = SourcePresentationConfig::empty_object();
        Arc::make_mut(&mut config.explicit_config_paths)
            .extend([Arc::from("a-ignored"), Arc::from("a.child")]);
        assert!(config.explicit_config_owns_path("a"));
    }

    #[test]
    fn explicit_ownership_lookup_does_not_scan_unrelated_paths() {
        for count in [100, 1_000, 10_000] {
            let mut config = SourcePresentationConfig::empty_object();
            Arc::make_mut(&mut config.explicit_config_paths)
                .extend((0..count).map(|index| Arc::from(format!("junk{index}"))));
            let (_, work) = measure_config_work(|| {
                for _ in 0..100 {
                    assert!(!config.explicit_config_owns_path("themeVariables.cScale1"));
                }
            });
            assert!(
                work.explicit_path_candidates <= 400,
                "unrelated paths must not multiply each ownership lookup: {count}: {work:?}"
            );
        }
    }
}
