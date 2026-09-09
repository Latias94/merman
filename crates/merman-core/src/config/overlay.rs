use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;
use std::sync::Arc;

use serde_json::Value;

use crate::{OperationControl, OperationControlResult};

use super::{MermaidConfig, PostDetectionDefaultPaths, ThemeCompatibilityState};

const MAX_OVERLAY_FAMILIES: usize = 64;
const MAX_FAMILY_NAME_BYTES: usize = 64;
const MAX_CONTRIBUTIONS_PER_FAMILY: usize = 128;
const MAX_CONTRIBUTION_ID_BYTES: usize = 128;
const MAX_ASSIGNMENTS_PER_CONTRIBUTION: usize = 512;
const MAX_ASSIGNMENTS_PER_FAMILY: usize = 1024;
const MAX_ASSIGNMENTS_PER_OVERLAY: usize = 4096;
const MAX_ASSIGNMENT_DEPTH: usize = 16;
const MAX_ASSIGNMENT_KEY_BYTES: usize = 128;
const MAX_ASSIGNMENT_STRING_BYTES: usize = 4 * 1024;
const MAX_RETAINED_BYTES_PER_FAMILY: usize = 3 * 1024 * 1024;
const MAX_RETAINED_BYTES_PER_OVERLAY: usize = 4 * 1024 * 1024;

pub(super) fn normalize_post_detection_default_paths(
    existing: &PostDetectionDefaultPaths,
    family: &str,
    paths: &[&str],
) -> Result<PostDetectionDefaultPaths, ConfigOverlayError> {
    validate_name(family, MAX_FAMILY_NAME_BYTES, ConfigOverlayField::Family)?;
    if paths.len() > MAX_ASSIGNMENTS_PER_FAMILY {
        return Err(ConfigOverlayError::LimitExceeded {
            field: ConfigOverlayField::Assignments,
        });
    }
    let mut normalized = existing
        .get(family)
        .into_iter()
        .flat_map(|paths| paths.iter().cloned())
        .collect::<BTreeSet<_>>();
    for path in paths {
        for (depth, key) in path.split('.').enumerate() {
            if depth >= MAX_ASSIGNMENT_DEPTH {
                return Err(ConfigOverlayError::LimitExceeded {
                    field: ConfigOverlayField::AssignmentPath,
                });
            }
            validate_assignment_key(key)?;
        }
        normalized.insert(Arc::from(*path));
        if normalized.len() > MAX_ASSIGNMENTS_PER_FAMILY {
            return Err(ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::Assignments,
            });
        }
    }
    if normalized.is_empty() {
        return Ok(existing.clone());
    }
    if !existing.contains_key(family) && existing.len() >= MAX_OVERLAY_FAMILIES {
        return Err(ConfigOverlayError::LimitExceeded {
            field: ConfigOverlayField::Families,
        });
    }
    let total_paths = existing.values().map(|paths| paths.len()).sum::<usize>()
        - existing.get(family).map_or(0, |paths| paths.len())
        + normalized.len();
    if total_paths > MAX_ASSIGNMENTS_PER_OVERLAY {
        return Err(ConfigOverlayError::LimitExceeded {
            field: ConfigOverlayField::Assignments,
        });
    }
    let mut requests = existing.clone();
    requests.insert(
        Arc::from(family),
        normalized.into_iter().collect::<Vec<_>>().into(),
    );
    Ok(requests)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ConfigOverlayProvenance {
    host: Arc<[ConfigOverlayContributionProvenance]>,
    fallback: Arc<[ConfigOverlayContributionProvenance]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigOverlayContributionProvenance {
    opaque_id: Arc<str>,
    surviving_assignment_paths: Arc<[Arc<str>]>,
}

impl ConfigOverlayContributionProvenance {
    pub(crate) fn opaque_id(&self) -> &str {
        &self.opaque_id
    }

    pub(crate) fn surviving_assignment_paths(&self) -> impl ExactSizeIterator<Item = &str> {
        self.surviving_assignment_paths.iter().map(AsRef::as_ref)
    }
}

impl ConfigOverlayProvenance {
    #[cfg(test)]
    pub(crate) fn contribution_ids(&self) -> impl Iterator<Item = &str> {
        self.host
            .iter()
            .chain(self.fallback.iter())
            .map(ConfigOverlayContributionProvenance::opaque_id)
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, opaque_id: &str) -> bool {
        self.contribution_ids()
            .any(|candidate| candidate == opaque_id)
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.host.is_empty() && self.fallback.is_empty()
    }

    /// Returns contributions from the low-priority compatibility lane only.
    #[cfg(test)]
    pub(crate) fn fallback_contribution_ids(&self) -> impl ExactSizeIterator<Item = &str> {
        self.fallback
            .iter()
            .map(ConfigOverlayContributionProvenance::opaque_id)
    }

    pub(crate) fn fallback_contributions(
        &self,
    ) -> impl ExactSizeIterator<Item = &ConfigOverlayContributionProvenance> {
        self.fallback.iter()
    }

    fn from_lanes(
        host: Vec<ConfigOverlayContributionProvenance>,
        fallback: Vec<ConfigOverlayContributionProvenance>,
    ) -> Self {
        Self {
            host: host.into(),
            fallback: fallback.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigOverlayLane {
    Host,
    Fallback,
}

#[derive(Debug, Clone)]
pub(crate) struct ConfigOverlayContribution {
    opaque_id: Arc<str>,
    assignments: Arc<[ConfigOverlayAssignment]>,
    retained_bytes: usize,
}

impl ConfigOverlayContribution {
    pub(crate) fn new(
        opaque_id: impl Into<String>,
        patch: MermaidConfig,
    ) -> Result<Self, ConfigOverlayError> {
        let opaque_id = opaque_id.into();
        validate_name(
            &opaque_id,
            MAX_CONTRIBUTION_ID_BYTES,
            ConfigOverlayField::ContributionId,
        )?;
        let assignments = flatten_patch(patch.as_value())?;
        if assignments.is_empty() {
            return Err(ConfigOverlayError::EmptyPatch);
        }
        if assignments
            .iter()
            .any(|assignment| is_render_family_selector_path(&assignment.path))
        {
            return Err(ConfigOverlayError::InvalidValue {
                field: ConfigOverlayField::AssignmentPath,
            });
        }
        let retained_bytes = assignments.iter().try_fold(
            size_of::<Self>() + opaque_id.len(),
            |retained_bytes, assignment| {
                retained_bytes
                    .checked_add(assignment.retained_bytes())
                    .ok_or(ConfigOverlayError::LimitExceeded {
                        field: ConfigOverlayField::RetainedBytes,
                    })
            },
        )?;
        Ok(Self {
            opaque_id: Arc::from(opaque_id),
            assignments: assignments.into(),
            retained_bytes,
        })
    }

    pub(crate) fn opaque_id(&self) -> &str {
        &self.opaque_id
    }

    pub(crate) fn assignments(&self) -> impl ExactSizeIterator<Item = (&str, &Value)> + '_ {
        self.assignments
            .iter()
            .map(|assignment| (assignment.path.as_ref(), assignment.value.as_ref()))
    }

    fn assignment_count(&self) -> usize {
        self.assignments.len()
    }
}

#[derive(Debug, Clone, Default)]
struct FamilyConfigOverlay {
    contributions: Vec<ConfigOverlayContribution>,
    assignment_count: usize,
    retained_bytes: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PostDetectionConfigOverlay {
    families: BTreeMap<String, FamilyConfigOverlay>,
    assignment_count: usize,
    retained_bytes: usize,
}

/// Lazily supplies one bounded fallback overlay after the final render family is known.
///
/// Providers are called at most once per parse operation and only for the selected family. The
/// returned overlay is still applied by Merman, so host precedence, resource limits, provenance,
/// and cancellation semantics remain owned by the core pipeline.
pub(crate) trait PostDetectionConfigOverlayProvider: std::fmt::Debug + Send + Sync {
    fn overlay_for_family(
        &self,
        family: &str,
        control: &OperationControl,
    ) -> OperationControlResult<
        Result<Option<Arc<PostDetectionConfigOverlay>>, PostDetectionConfigOverlayProviderError>,
    >;
}

impl PostDetectionConfigOverlay {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(crate) fn with_family_contribution(
        mut self,
        family: impl Into<String>,
        contribution: ConfigOverlayContribution,
    ) -> Result<Self, ConfigOverlayError> {
        self.try_push_family_contribution(family, contribution)?;
        Ok(self)
    }

    pub(crate) fn try_push_family_contribution(
        &mut self,
        family: impl Into<String>,
        contribution: ConfigOverlayContribution,
    ) -> Result<(), ConfigOverlayError> {
        let family = family.into();
        validate_name(&family, MAX_FAMILY_NAME_BYTES, ConfigOverlayField::Family)?;
        let is_new_family = !self.families.contains_key(&family);
        if is_new_family && self.families.len() >= MAX_OVERLAY_FAMILIES {
            return Err(ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::Families,
            });
        }

        let existing = self.families.get(&family);
        if existing
            .is_some_and(|existing| existing.contributions.len() >= MAX_CONTRIBUTIONS_PER_FAMILY)
        {
            return Err(ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::Contributions,
            });
        }
        if existing.is_some_and(|existing| {
            existing
                .contributions
                .iter()
                .any(|candidate| candidate.opaque_id == contribution.opaque_id)
        }) {
            return Err(ConfigOverlayError::DuplicateContributionId {
                family,
                opaque_id: contribution.opaque_id().to_string(),
            });
        }
        if let Some(path) = existing.and_then(|existing| {
            existing
                .contributions
                .iter()
                .find_map(|candidate| conflicting_assignment_path(candidate, &contribution))
        }) {
            return Err(ConfigOverlayError::DuplicateAssignment { family, path });
        }

        let family_assignment_count = existing
            .map_or(0, |existing| existing.assignment_count)
            .checked_add(contribution.assignment_count())
            .ok_or(ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::Assignments,
            })?;
        if family_assignment_count > MAX_ASSIGNMENTS_PER_FAMILY {
            return Err(ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::Assignments,
            });
        }
        let assignment_count = self
            .assignment_count
            .checked_add(contribution.assignment_count())
            .ok_or(ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::Assignments,
            })?;
        if assignment_count > MAX_ASSIGNMENTS_PER_OVERLAY {
            return Err(ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::Assignments,
            });
        }

        let family_entry_bytes = usize::from(is_new_family)
            .checked_mul(size_of::<FamilyConfigOverlay>() + family.len())
            .ok_or(ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::RetainedBytes,
            })?;
        let family_retained_bytes = existing
            .map_or(0, |existing| existing.retained_bytes)
            .checked_add(contribution.retained_bytes)
            .and_then(|bytes| bytes.checked_add(family_entry_bytes))
            .ok_or(ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::RetainedBytes,
            })?;
        if family_retained_bytes > MAX_RETAINED_BYTES_PER_FAMILY {
            return Err(ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::RetainedBytes,
            });
        }
        let retained_bytes = self
            .retained_bytes
            .checked_add(contribution.retained_bytes)
            .and_then(|bytes| bytes.checked_add(family_entry_bytes))
            .ok_or(ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::RetainedBytes,
            })?;
        if retained_bytes > MAX_RETAINED_BYTES_PER_OVERLAY {
            return Err(ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::RetainedBytes,
            });
        }

        let family_overlay = self.families.entry(family).or_default();
        family_overlay.assignment_count = family_assignment_count;
        family_overlay.retained_bytes = family_retained_bytes;
        family_overlay.contributions.push(contribution);
        self.assignment_count = assignment_count;
        self.retained_bytes = retained_bytes;
        Ok(())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.families
            .values()
            .all(|family| family.contributions.is_empty())
    }

    #[cfg(test)]
    pub(crate) fn apply_family_controlled(
        &self,
        family: &str,
        explicit_site_config: &MermaidConfig,
        explicit_source_config: &MermaidConfig,
        config_before_detection: &MermaidConfig,
        effective_config: &mut MermaidConfig,
        application: &mut ConfigOverlayApplication,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        self.apply_family_controlled_in_lane(
            family,
            explicit_site_config,
            explicit_source_config,
            config_before_detection,
            effective_config,
            application,
            ConfigOverlayLane::Host,
            control,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_family_controlled_in_lane(
        &self,
        family: &str,
        explicit_site_config: &MermaidConfig,
        explicit_source_config: &MermaidConfig,
        config_before_detection: &MermaidConfig,
        effective_config: &mut MermaidConfig,
        application: &mut ConfigOverlayApplication,
        lane: ConfigOverlayLane,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        control.checkpoint()?;
        let Some(family_overlay) = self.families.get(family) else {
            return Ok(());
        };

        let defaults = PostDetectionConfigDefaults {
            explicit_site_config,
            explicit_source_config,
            config_before_detection,
        };
        for contribution in &family_overlay.contributions {
            control.checkpoint()?;
            let mut applied_assignments = Vec::new();
            for assignment in contribution.assignments.iter() {
                control.checkpoint()?;
                if defaults.blocks_path(effective_config, application, &assignment.path) {
                    continue;
                }
                if lane == ConfigOverlayLane::Host {
                    effective_config
                        .set_value_explicit(&assignment.path, assignment.value.as_ref().clone());
                } else {
                    effective_config.set_value(&assignment.path, assignment.value.as_ref().clone());
                }
                application.claim_path(Arc::clone(&assignment.path));
                applied_assignments.push(AppliedOverlayAssignment {
                    path: Arc::clone(&assignment.path),
                    value: Arc::clone(&assignment.value),
                });
            }
            application.record_contribution(lane, &contribution.opaque_id, applied_assignments);
        }
        Ok(())
    }
}

/// Borrowed inputs that determine whether a post-detection default may fill a path.
///
/// Unlike typed-default ownership, this checks surviving raw input values, not ownership
/// propagated through theme calculations. The effective config and prior claims stay live
/// so each assignment observes earlier host/fallback writes without retaining a snapshot.
struct PostDetectionConfigDefaults<'a> {
    explicit_site_config: &'a MermaidConfig,
    explicit_source_config: &'a MermaidConfig,
    config_before_detection: &'a MermaidConfig,
}

impl PostDetectionConfigDefaults<'_> {
    fn blocks_path(
        &self,
        effective_config: &MermaidConfig,
        application: &ConfigOverlayApplication,
        dotted_path: &str,
    ) -> bool {
        application.claims_path(dotted_path)
            || owns_path(self.explicit_site_config, effective_config, dotted_path)
            || owns_path(self.explicit_source_config, effective_config, dotted_path)
            || effective_config.path_was_mutated_after(self.config_before_detection, dotted_path)
            || value_at_path(self.config_before_detection, dotted_path)
                != value_at_path(effective_config, dotted_path)
    }
}

impl MermaidConfig {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn capture_post_detection_default_decisions(
        &mut self,
        family: &str,
        explicit_site_config: &MermaidConfig,
        explicit_source_config: &MermaidConfig,
        config_before_detection: &MermaidConfig,
        application: &ConfigOverlayApplication,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let Some(ThemeCompatibilityState::Tracking(ownership)) = self.theme_compatibility.as_ref()
        else {
            return Ok(());
        };
        let Some(paths) = ownership.binding.post_detection_default_paths.get(family) else {
            return Ok(());
        };
        let defaults = PostDetectionConfigDefaults {
            explicit_site_config,
            explicit_source_config,
            config_before_detection,
        };
        let mut decisions = Vec::with_capacity(paths.len());
        for path in paths.iter() {
            control.checkpoint()?;
            decisions.push((
                Arc::clone(path),
                defaults.blocks_path(self, application, path),
            ));
        }
        if let Some(ThemeCompatibilityState::Tracking(ownership)) =
            self.theme_compatibility.as_mut()
        {
            Arc::make_mut(ownership).post_detection_defaults = Some(decisions.into());
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub(crate) struct ConfigOverlayApplication {
    claimed_paths: BTreeSet<Arc<str>>,
    contributions: Vec<AppliedOverlayContribution>,
}

impl ConfigOverlayApplication {
    pub(crate) fn finalize(&self, effective_config: &MermaidConfig) -> ConfigOverlayProvenance {
        let mut surviving_host = Vec::new();
        let mut surviving_fallback = Vec::new();
        for contribution in &self.contributions {
            let surviving_assignment_paths = contribution
                .assignments
                .iter()
                .filter(|assignment| {
                    value_at_path(effective_config, &assignment.path)
                        == Some(assignment.value.as_ref())
                })
                .map(|assignment| Arc::clone(&assignment.path))
                .collect::<Vec<_>>();
            if surviving_assignment_paths.is_empty() {
                continue;
            }
            let surviving = match contribution.lane {
                ConfigOverlayLane::Host => &mut surviving_host,
                ConfigOverlayLane::Fallback => &mut surviving_fallback,
            };
            record_surviving_contribution(
                surviving,
                &contribution.opaque_id,
                surviving_assignment_paths,
            );
        }
        ConfigOverlayProvenance::from_lanes(surviving_host, surviving_fallback)
    }

    fn claims_path(&self, path: &str) -> bool {
        self.claimed_paths
            .iter()
            .any(|claimed| dotted_paths_overlap(claimed, path))
    }

    fn claim_path(&mut self, path: Arc<str>) {
        self.claimed_paths.insert(path);
    }

    fn record_contribution(
        &mut self,
        lane: ConfigOverlayLane,
        opaque_id: &Arc<str>,
        assignments: Vec<AppliedOverlayAssignment>,
    ) {
        if assignments.is_empty() {
            return;
        }
        self.contributions.push(AppliedOverlayContribution {
            lane,
            opaque_id: Arc::clone(opaque_id),
            assignments,
        });
    }
}

fn record_surviving_contribution(
    contributions: &mut Vec<ConfigOverlayContributionProvenance>,
    opaque_id: &Arc<str>,
    surviving_assignment_paths: Vec<Arc<str>>,
) {
    if let Some(existing) = contributions
        .iter_mut()
        .find(|candidate| candidate.opaque_id.as_ref() == opaque_id.as_ref())
    {
        let mut merged_paths = existing.surviving_assignment_paths.to_vec();
        for path in surviving_assignment_paths {
            if !merged_paths.contains(&path) {
                merged_paths.push(path);
            }
        }
        existing.surviving_assignment_paths = merged_paths.into();
        return;
    }
    contributions.push(ConfigOverlayContributionProvenance {
        opaque_id: Arc::clone(opaque_id),
        surviving_assignment_paths: surviving_assignment_paths.into(),
    });
}

#[derive(Debug)]
struct AppliedOverlayContribution {
    lane: ConfigOverlayLane,
    opaque_id: Arc<str>,
    assignments: Vec<AppliedOverlayAssignment>,
}

#[derive(Debug)]
struct AppliedOverlayAssignment {
    path: Arc<str>,
    value: Arc<Value>,
}

#[derive(Debug, Clone)]
struct ConfigOverlayAssignment {
    path: Arc<str>,
    value: Arc<Value>,
}

impl ConfigOverlayAssignment {
    fn retained_bytes(&self) -> usize {
        size_of::<Self>() + self.path.len() + scalar_retained_bytes(&self.value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigOverlayField {
    Family,
    Families,
    ContributionId,
    Contributions,
    Assignments,
    RetainedBytes,
    Patch,
    AssignmentPath,
    AssignmentValue,
}

impl ConfigOverlayField {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Family => "family",
            Self::Families => "families",
            Self::ContributionId => "contribution_id",
            Self::Contributions => "contributions",
            Self::Assignments => "assignments",
            Self::RetainedBytes => "retained_bytes",
            Self::Patch => "patch",
            Self::AssignmentPath => "assignment_path",
            Self::AssignmentValue => "assignment_value",
        }
    }
}

impl std::fmt::Display for ConfigOverlayField {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ConfigOverlayError {
    #[error("config overlay field `{field}` is invalid")]
    InvalidValue { field: ConfigOverlayField },
    #[error("config overlay field `{field}` exceeds its implementation limit")]
    LimitExceeded { field: ConfigOverlayField },
    #[error("config overlay patch must contain at least one scalar assignment")]
    EmptyPatch,
    #[error("duplicate config overlay contribution `{opaque_id}` for family `{family}`")]
    DuplicateContributionId { family: String, opaque_id: String },
    #[error("duplicate config overlay assignment `{path}` for family `{family}`")]
    DuplicateAssignment { family: String, path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PostDetectionConfigOverlayProviderError {
    #[error(transparent)]
    Overlay(#[from] ConfigOverlayError),
    #[error("config overlay provider failed for family `{family}`: {message}")]
    Provider { family: String, message: String },
}

fn validate_name(
    value: &str,
    max_bytes: usize,
    field: ConfigOverlayField,
) -> Result<(), ConfigOverlayError> {
    if value.is_empty()
        || value.len() > max_bytes
        || value.chars().any(char::is_control)
        || value.trim() != value
    {
        return Err(ConfigOverlayError::InvalidValue { field });
    }
    Ok(())
}

fn flatten_patch(value: &Value) -> Result<Vec<ConfigOverlayAssignment>, ConfigOverlayError> {
    let Value::Object(root) = value else {
        return Err(ConfigOverlayError::InvalidValue {
            field: ConfigOverlayField::Patch,
        });
    };
    let mut assignments = Vec::new();
    let mut seen_paths = BTreeSet::new();
    let mut path = Vec::<&str>::new();
    flatten_object(root, &mut path, &mut assignments, &mut seen_paths)?;
    Ok(assignments)
}

fn validate_assignment_key(key: &str) -> Result<(), ConfigOverlayError> {
    if key.is_empty()
        || key.len() > MAX_ASSIGNMENT_KEY_BYTES
        || key.contains('.')
        || key.chars().any(char::is_control)
    {
        return Err(ConfigOverlayError::InvalidValue {
            field: ConfigOverlayField::AssignmentPath,
        });
    }
    Ok(())
}

fn flatten_object<'a>(
    object: &'a serde_json::Map<String, Value>,
    path: &mut Vec<&'a str>,
    assignments: &mut Vec<ConfigOverlayAssignment>,
    seen_paths: &mut BTreeSet<String>,
) -> Result<(), ConfigOverlayError> {
    if path.len() >= MAX_ASSIGNMENT_DEPTH {
        return Err(ConfigOverlayError::LimitExceeded {
            field: ConfigOverlayField::AssignmentPath,
        });
    }
    for (key, value) in object {
        validate_assignment_key(key)?;
        path.push(key);
        match value {
            Value::Object(nested) if !nested.is_empty() => {
                flatten_object(nested, path, assignments, seen_paths)?;
            }
            Value::String(value) if value.len() <= MAX_ASSIGNMENT_STRING_BYTES => {
                push_assignment(path, Value::String(value.clone()), assignments, seen_paths)?;
            }
            Value::Number(_) | Value::Bool(_) => {
                push_assignment(path, value.clone(), assignments, seen_paths)?;
            }
            _ => {
                return Err(ConfigOverlayError::InvalidValue {
                    field: ConfigOverlayField::AssignmentValue,
                });
            }
        }
        path.pop();
    }
    Ok(())
}

fn push_assignment(
    path: &[&str],
    value: Value,
    assignments: &mut Vec<ConfigOverlayAssignment>,
    seen_paths: &mut BTreeSet<String>,
) -> Result<(), ConfigOverlayError> {
    if assignments.len() >= MAX_ASSIGNMENTS_PER_CONTRIBUTION {
        return Err(ConfigOverlayError::LimitExceeded {
            field: ConfigOverlayField::Patch,
        });
    }
    let path = path.join(".");
    if !seen_paths.insert(path.clone()) {
        return Err(ConfigOverlayError::DuplicateAssignment {
            family: String::new(),
            path,
        });
    }
    assignments.push(ConfigOverlayAssignment {
        path: Arc::from(path),
        value: Arc::new(value),
    });
    Ok(())
}

fn conflicting_assignment_path(
    existing: &ConfigOverlayContribution,
    candidate: &ConfigOverlayContribution,
) -> Option<String> {
    existing.assignments.iter().find_map(|existing_assignment| {
        candidate
            .assignments
            .iter()
            .find_map(|candidate_assignment| {
                dotted_paths_overlap(&existing_assignment.path, &candidate_assignment.path)
                    .then(|| candidate_assignment.path.to_string())
            })
    })
}

fn owns_path(
    explicit_config: &MermaidConfig,
    effective_config: &MermaidConfig,
    dotted_path: &str,
) -> bool {
    let mut explicit_current = explicit_config.as_value();
    let mut effective_current = effective_config.as_value();
    for segment in dotted_path.split('.') {
        let Some(explicit_object) = explicit_current.as_object() else {
            // A scalar/array parent only owns a descendant when assignWithDepth actually left that
            // parent in the effective config. Type-conflicting assignments are ignored upstream.
            return explicit_current == effective_current;
        };
        let Some(explicit_next) = explicit_object.get(segment) else {
            return false;
        };
        let Some(effective_next) = effective_current
            .as_object()
            .and_then(|object| object.get(segment))
        else {
            return false;
        };
        explicit_current = explicit_next;
        effective_current = effective_next;
    }
    explicit_current == effective_current
}

fn value_at_path<'a>(config: &'a MermaidConfig, dotted_path: &str) -> Option<&'a Value> {
    let mut current = config.as_value();
    for segment in dotted_path.split('.') {
        current = current.as_object()?.get(segment)?;
    }
    Some(current)
}

fn dotted_paths_overlap(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('.'))
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn is_render_family_selector_path(path: &str) -> bool {
    path.split('.')
        .any(|segment| matches!(segment, "layout" | "defaultRenderer"))
}

fn scalar_retained_bytes(value: &Value) -> usize {
    match value {
        Value::String(value) => value.len(),
        Value::Number(value) => size_of::<serde_json::Number>() + value.to_string().len(),
        Value::Bool(_) => size_of::<bool>(),
        Value::Null | Value::Array(_) | Value::Object(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, json};

    use super::*;

    fn contribution(id: &str, patch: Value) -> ConfigOverlayContribution {
        ConfigOverlayContribution::new(id, MermaidConfig::from_value(patch)).unwrap()
    }

    fn apply(
        overlay: &PostDetectionConfigOverlay,
        explicit_site: &MermaidConfig,
        explicit_source: &MermaidConfig,
        before_detect: &MermaidConfig,
        effective: &mut MermaidConfig,
    ) -> ConfigOverlayProvenance {
        let mut application = ConfigOverlayApplication::default();
        overlay
            .apply_family_controlled(
                "flowchart",
                explicit_site,
                explicit_source,
                before_detect,
                effective,
                &mut application,
                &OperationControl::new(),
            )
            .unwrap();
        application.finalize(effective)
    }

    #[test]
    fn family_overlay_only_fills_paths_without_higher_priority_ownership() {
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                contribution(
                    "legacy.flowchart.node.fill",
                    json!({"themeVariables": {"primaryColor": "#123456"}}),
                ),
            )
            .unwrap()
            .with_family_contribution(
                "flowchart",
                contribution(
                    "legacy.flowchart.line",
                    json!({"themeVariables": {"lineColor": "#abcdef"}}),
                ),
            )
            .unwrap();
        let explicit_site = MermaidConfig::from_value(json!({
            "themeVariables": {"primaryColor": "#fedcba"}
        }));
        let explicit_source = MermaidConfig::empty_object();
        let before_detect = MermaidConfig::from_value(json!({
            "themeVariables": {
                "primaryColor": "#fedcba",
                "lineColor": "#333333"
            }
        }));
        let mut effective = before_detect.clone();

        let provenance = apply(
            &overlay,
            &explicit_site,
            &explicit_source,
            &before_detect,
            &mut effective,
        );

        assert_eq!(
            effective.get_str("themeVariables.primaryColor"),
            Some("#fedcba")
        );
        assert_eq!(
            effective.get_str("themeVariables.lineColor"),
            Some("#abcdef")
        );
        assert!(!provenance.contains("legacy.flowchart.node.fill"));
        assert!(provenance.contains("legacy.flowchart.line"));
    }

    #[test]
    fn post_detection_defaults_distinguish_raw_inputs_from_derived_ownership() {
        let explicit = MermaidConfig::from_value(json!({
            "themeVariables": {"primaryColor": "#123456"}
        }));
        let empty = MermaidConfig::empty_object();
        let mut before_detect = MermaidConfig::from_value(json!({"theme": "base"}));
        before_detect.deep_merge_explicit(explicit.as_value());
        crate::theme::apply_theme_defaults(&mut before_detect).unwrap();
        assert!(before_detect.explicit_config_owns_path("themeVariables.primaryBorderColor"));
        assert!(before_detect.config_path_overrides_typed_default("themeVariables.mainBkg"));
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                contribution(
                    "post-detection.node",
                    json!({"themeVariables": {
                        "primaryColor": "#abcdef",
                        "primaryBorderColor": "#abcdef",
                        "mainBkg": "#abcdef"
                    }}),
                ),
            )
            .unwrap();

        for (site, source) in [(&explicit, &empty), (&empty, &explicit)] {
            let mut effective = before_detect.clone();
            let provenance = apply(&overlay, site, source, &before_detect, &mut effective);
            assert_eq!(
                effective.get_str("themeVariables.primaryColor"),
                Some("#123456")
            );
            assert_eq!(
                effective.get_str("themeVariables.primaryBorderColor"),
                Some("#abcdef")
            );
            assert_eq!(effective.get_str("themeVariables.mainBkg"), Some("#abcdef"));
            assert!(provenance.contains("post-detection.node"));
        }
    }

    #[test]
    fn normalized_raw_values_do_not_block_post_detection_defaults() {
        let explicit = MermaidConfig::from_value(json!({
            "themeVariables": {"primaryColor": null}
        }));
        let empty = MermaidConfig::empty_object();
        let mut before_detect = MermaidConfig::from_value(json!({"theme": "base"}));
        before_detect.set_value_explicit("themeVariables.primaryColor", Value::Null);
        crate::theme::apply_theme_defaults(&mut before_detect).unwrap();
        assert!(
            before_detect
                .get_str("themeVariables.primaryColor")
                .is_some()
        );
        assert!(before_detect.explicit_config_owns_path("themeVariables.primaryColor"));
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                contribution(
                    "post-detection.node",
                    json!({"themeVariables": {"primaryColor": "#abcdef"}}),
                ),
            )
            .unwrap();

        for (site, source) in [(&explicit, &empty), (&empty, &explicit)] {
            let mut effective = before_detect.clone();
            let provenance = apply(&overlay, site, source, &before_detect, &mut effective);
            assert_eq!(
                effective.get_str("themeVariables.primaryColor"),
                Some("#abcdef")
            );
            assert!(provenance.contains("post-detection.node"));
        }
    }

    #[test]
    fn prior_claims_block_overlapping_defaults_without_value_changes() {
        let empty = MermaidConfig::empty_object();
        let before_detect = MermaidConfig::from_value(json!({
            "custom": {"child": "same"}, "sibling": "same"
        }));
        let defaults = PostDetectionConfigDefaults {
            explicit_site_config: &empty,
            explicit_source_config: &empty,
            config_before_detection: &before_detect,
        };
        let mut application = ConfigOverlayApplication::default();
        assert!(!defaults.blocks_path(&before_detect, &application, "custom.child"));
        application.claim_path(Arc::from("custom.child"));
        assert!(defaults.blocks_path(&before_detect, &application, "custom.child"));
        assert!(defaults.blocks_path(&before_detect, &application, "custom"));
        assert!(!defaults.blocks_path(&before_detect, &application, "sibling"));
        application.claim_path(Arc::from("custom"));
        assert!(defaults.blocks_path(&before_detect, &application, "custom.other"));
    }

    #[test]
    fn same_value_host_assignment_still_blocks_fallback() {
        let make_overlay = |id, color| {
            PostDetectionConfigOverlay::new()
                .with_family_contribution(
                    "flowchart",
                    contribution(id, json!({"themeVariables": {"lineColor": color}})),
                )
                .unwrap()
        };
        let host = make_overlay("host.line", "#333333");
        let fallback = make_overlay("fallback.line", "#abcdef");
        let empty = MermaidConfig::empty_object();
        let before_detect = MermaidConfig::from_value(json!({
            "themeVariables": {"lineColor": "#333333"}
        }));
        let mut effective = before_detect.clone();
        let mut application = ConfigOverlayApplication::default();
        for (overlay, lane) in [
            (&host, ConfigOverlayLane::Host),
            (&fallback, ConfigOverlayLane::Fallback),
        ] {
            overlay
                .apply_family_controlled_in_lane(
                    "flowchart",
                    &empty,
                    &empty,
                    &before_detect,
                    &mut effective,
                    &mut application,
                    lane,
                    &OperationControl::new(),
                )
                .unwrap();
        }
        let provenance = application.finalize(&effective);
        assert_eq!(
            effective.get_str("themeVariables.lineColor"),
            Some("#333333")
        );
        assert!(provenance.contains("host.line"));
        assert!(!provenance.contains("fallback.line"));
    }

    #[test]
    fn ignored_type_conflicts_do_not_claim_overlay_paths() {
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                contribution(
                    "legacy.flowchart.node-spacing",
                    json!({"flowchart": {"nodeSpacing": 72}}),
                ),
            )
            .unwrap();
        let explicit_site = MermaidConfig::from_value(json!({"flowchart": "ignored"}));
        let explicit_source = MermaidConfig::empty_object();
        let before_detect = MermaidConfig::from_value(json!({
            "flowchart": {"nodeSpacing": 50}
        }));
        let mut effective = before_detect.clone();

        let provenance = apply(
            &overlay,
            &explicit_site,
            &explicit_source,
            &before_detect,
            &mut effective,
        );

        assert_eq!(effective.as_value()["flowchart"]["nodeSpacing"], json!(72));
        assert!(provenance.contains("legacy.flowchart.node-spacing"));
    }

    #[test]
    fn surviving_scalar_parent_still_claims_descendant_overlay_paths() {
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                contribution("legacy.custom.child", json!({"custom": {"child": 72}})),
            )
            .unwrap();
        let explicit_site = MermaidConfig::from_value(json!({"custom": "owned"}));
        let explicit_source = MermaidConfig::empty_object();
        let before_detect = MermaidConfig::from_value(json!({"custom": "owned"}));
        let mut effective = before_detect.clone();

        let provenance = apply(
            &overlay,
            &explicit_site,
            &explicit_source,
            &before_detect,
            &mut effective,
        );

        assert_eq!(effective.as_value()["custom"], json!("owned"));
        assert!(provenance.is_empty());
    }

    #[test]
    fn detector_mutation_owns_the_changed_path() {
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                contribution(
                    "legacy.flowchart.line",
                    json!({"themeVariables": {"lineColor": "#123456"}}),
                ),
            )
            .unwrap();
        let explicit = MermaidConfig::empty_object();
        let before_detect = MermaidConfig::from_value(json!({
            "themeVariables": {"lineColor": "#333333"}
        }));
        let mut effective = before_detect.clone();
        effective.set_value("themeVariables.lineColor", json!("#abcdef"));

        let provenance = apply(
            &overlay,
            &explicit,
            &explicit,
            &before_detect,
            &mut effective,
        );

        assert_eq!(
            effective.get_str("themeVariables.lineColor"),
            Some("#abcdef")
        );
        assert!(provenance.is_empty());
    }

    #[test]
    fn repeated_and_raw_mutations_own_paths_even_when_the_value_is_unchanged() {
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                contribution(
                    "legacy.flowchart.line",
                    json!({"themeVariables": {"lineColor": "#123456"}}),
                ),
            )
            .unwrap();
        let explicit = MermaidConfig::empty_object();

        let mut before_detect = MermaidConfig::empty_object();
        before_detect.set_value("themeVariables.lineColor", json!("#333333"));
        let mut repeated = before_detect.clone();
        repeated.set_value("themeVariables.lineColor", json!("#333333"));
        assert!(
            apply(
                &overlay,
                &explicit,
                &explicit,
                &before_detect,
                &mut repeated,
            )
            .is_empty()
        );

        let mut raw = before_detect.clone();
        raw.as_value_mut()["themeVariables"]["lineColor"] = json!("#333333");
        assert!(apply(&overlay, &explicit, &explicit, &before_detect, &mut raw,).is_empty());
    }

    #[test]
    fn detector_parent_mutation_owns_descendant_overlay_paths() {
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                contribution(
                    "legacy.flowchart.primary",
                    json!({"themeVariables": {"primaryColor": "#123456"}}),
                ),
            )
            .unwrap();
        let explicit = MermaidConfig::empty_object();
        let before_detect = MermaidConfig::from_value(json!({
            "themeVariables": {"primaryColor": "#ececff"}
        }));
        let mut effective = before_detect.clone();
        effective.set_value("themeVariables", json!({"primaryColor": "#ececff"}));

        let provenance = apply(
            &overlay,
            &explicit,
            &explicit,
            &before_detect,
            &mut effective,
        );

        assert_eq!(
            effective.get_str("themeVariables.primaryColor"),
            Some("#ececff")
        );
        assert!(provenance.is_empty());
    }

    #[test]
    fn finalization_drops_contributions_without_a_surviving_assignment() {
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                contribution(
                    "legacy.flowchart.line",
                    json!({"themeVariables": {"lineColor": "#123456"}}),
                ),
            )
            .unwrap();
        let explicit = MermaidConfig::empty_object();
        let before_detect = MermaidConfig::from_value(json!({
            "themeVariables": {"lineColor": "#333333"}
        }));
        let mut effective = before_detect.clone();
        let mut application = ConfigOverlayApplication::default();
        overlay
            .apply_family_controlled(
                "flowchart",
                &explicit,
                &explicit,
                &before_detect,
                &mut effective,
                &mut application,
                &OperationControl::new(),
            )
            .unwrap();
        effective.set_value("themeVariables.lineColor", json!("#abcdef"));

        assert!(application.finalize(&effective).is_empty());
    }

    #[test]
    fn finalization_records_only_the_assignment_paths_that_still_survive() {
        let opaque_id = "legacy.flowchart.typography";
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                contribution(
                    opaque_id,
                    json!({
                        "fontFamily": "TypedFont,sans-serif",
                        "themeVariables": {
                            "fontFamily": "TypedFont,sans-serif",
                            "fontSize": "24px"
                        }
                    }),
                ),
            )
            .unwrap();
        let explicit = MermaidConfig::empty_object();
        let before_detect = MermaidConfig::empty_object();
        let mut effective = before_detect.clone();
        let mut application = ConfigOverlayApplication::default();
        overlay
            .apply_family_controlled_in_lane(
                "flowchart",
                &explicit,
                &explicit,
                &before_detect,
                &mut effective,
                &mut application,
                ConfigOverlayLane::Fallback,
                &OperationControl::new(),
            )
            .unwrap();

        effective.set_value("themeVariables.fontSize", json!("16px"));
        let provenance = application.finalize(&effective);
        let contribution = provenance
            .fallback_contributions()
            .next()
            .expect("partially surviving fallback contribution");

        assert_eq!(contribution.opaque_id(), opaque_id);
        assert_eq!(
            contribution
                .surviving_assignment_paths()
                .collect::<Vec<_>>(),
            ["fontFamily", "themeVariables.fontFamily"]
        );
        assert_eq!(provenance.fallback_contributions().len(), 1);

        effective.set_overlay_provenance(provenance);
        assert!(effective.fallback_overlay_owns_path("fontFamily"));
        assert!(effective.fallback_overlay_owns_path("themeVariables.fontFamily"));
        assert!(!effective.fallback_overlay_owns_path("themeVariables.fontSize"));
        assert!(!effective.fallback_overlay_owns_path("themeVariables"));
        assert!(!effective.config_path_overrides_typed_default("fontFamily"));
        assert!(!effective.config_path_overrides_typed_default("themeVariables.fontFamily"));
    }

    #[test]
    fn fallback_ownership_excludes_host_overlay_assignments() {
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                contribution(
                    "host.flowchart.typography",
                    json!({"themeVariables": {"fontFamily": "HostFont,sans-serif"}}),
                ),
            )
            .unwrap();
        let explicit = MermaidConfig::empty_object();
        let before_detect = MermaidConfig::empty_object();
        let mut effective = before_detect.clone();
        let mut application = ConfigOverlayApplication::default();
        overlay
            .apply_family_controlled(
                "flowchart",
                &explicit,
                &explicit,
                &before_detect,
                &mut effective,
                &mut application,
                &OperationControl::new(),
            )
            .unwrap();

        effective.set_overlay_provenance(application.finalize(&effective));
        assert!(!effective.fallback_overlay_owns_path("themeVariables.fontFamily"));
        assert!(effective.config_path_overrides_typed_default("themeVariables.fontFamily"));
    }

    #[test]
    fn failed_family_contribution_push_preserves_prior_overlay_state() {
        let mut overlay = PostDetectionConfigOverlay::new();
        overlay
            .try_push_family_contribution(
                "flowchart",
                contribution(
                    "legacy.flowchart.primary",
                    json!({"themeVariables": {"primaryColor": "#123456"}}),
                ),
            )
            .expect("first contribution is valid");

        let error = overlay
            .try_push_family_contribution(
                "flowchart",
                contribution(
                    "legacy.flowchart.conflict",
                    json!({"themeVariables": {"primaryColor": "#abcdef"}}),
                ),
            )
            .expect_err("overlapping assignments must fail closed");
        assert_eq!(
            error,
            ConfigOverlayError::DuplicateAssignment {
                family: "flowchart".to_string(),
                path: "themeVariables.primaryColor".to_string(),
            }
        );

        let explicit = MermaidConfig::empty_object();
        let before_detect = MermaidConfig::empty_object();
        let mut effective = before_detect.clone();
        let provenance = apply(
            &overlay,
            &explicit,
            &explicit,
            &before_detect,
            &mut effective,
        );

        assert_eq!(
            effective.get_str("themeVariables.primaryColor"),
            Some("#123456")
        );
        assert!(provenance.contains("legacy.flowchart.primary"));
        assert!(!provenance.contains("legacy.flowchart.conflict"));
    }

    #[test]
    fn selected_family_and_overlay_assignment_limits_are_aggregate() {
        let wide_contribution = |id: &str, namespace: &str, count: usize| {
            let values = (0..count)
                .map(|index| (format!("k{index}"), json!(index)))
                .collect::<Map<_, _>>();
            contribution(id, json!({namespace: values}))
        };

        let family_limited = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                wide_contribution("one", "one", MAX_ASSIGNMENTS_PER_CONTRIBUTION),
            )
            .unwrap()
            .with_family_contribution(
                "flowchart",
                wide_contribution("two", "two", MAX_ASSIGNMENTS_PER_CONTRIBUTION),
            )
            .unwrap()
            .with_family_contribution("flowchart", wide_contribution("three", "three", 1))
            .unwrap_err();
        assert_eq!(
            family_limited,
            ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::Assignments
            }
        );

        let mut overlay = PostDetectionConfigOverlay::new();
        for index in 0..(MAX_ASSIGNMENTS_PER_OVERLAY / MAX_ASSIGNMENTS_PER_CONTRIBUTION) {
            overlay = overlay
                .with_family_contribution(
                    format!("family{index}"),
                    wide_contribution(
                        &format!("contribution{index}"),
                        &format!("n{index}"),
                        MAX_ASSIGNMENTS_PER_CONTRIBUTION,
                    ),
                )
                .unwrap();
        }
        let error = overlay
            .with_family_contribution("overflow", wide_contribution("overflow", "overflow", 1))
            .unwrap_err();
        assert_eq!(
            error,
            ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::Assignments
            }
        );
    }

    #[test]
    fn overlay_retained_bytes_are_bounded_across_families() {
        let large = "x".repeat(MAX_ASSIGNMENT_STRING_BYTES);
        let wide_contribution = |id: &str, namespace: &str| {
            let values = (0..MAX_ASSIGNMENTS_PER_CONTRIBUTION)
                .map(|index| (format!("k{index}"), json!(large)))
                .collect::<Map<_, _>>();
            contribution(id, json!({namespace: values}))
        };
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution("family1", wide_contribution("one", "one"))
            .unwrap();
        let error = overlay
            .with_family_contribution("family2", wide_contribution("two", "two"))
            .unwrap_err();
        assert_eq!(
            error,
            ConfigOverlayError::LimitExceeded {
                field: ConfigOverlayField::RetainedBytes
            }
        );
    }

    #[test]
    fn capturing_default_decisions_is_selected_family_only_and_cancellable() {
        let binding =
            super::super::ThemeParseBinding::try_new([0x5a; 32], MermaidConfig::empty_object())
                .unwrap()
                .try_with_post_detection_default_paths("flowchart", &["a", "b", "c"])
                .unwrap();
        let mut config = MermaidConfig::from_theme_parse_binding(binding.clone());
        let empty = MermaidConfig::empty_object();
        let application = ConfigOverlayApplication::default();
        let cancelled = OperationControl::new();
        cancelled.cancel();
        assert!(
            config
                .capture_post_detection_default_decisions(
                    "sequence",
                    &empty,
                    &empty,
                    &empty,
                    &application,
                    &cancelled,
                )
                .is_ok()
        );
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        assert!(
            config
                .capture_post_detection_default_decisions(
                    "flowchart",
                    &empty,
                    &empty,
                    &empty,
                    &application,
                    &control,
                )
                .is_err()
        );
        config.freeze_theme_compatibility();
        assert_eq!(config.post_detection_default_blocked("a"), None);

        let mut config = MermaidConfig::from_theme_parse_binding(binding);
        config
            .capture_post_detection_default_decisions(
                "flowchart",
                &empty,
                &empty,
                &empty,
                &application,
                &OperationControl::new(),
            )
            .unwrap();
        assert_eq!(config.post_detection_default_blocked("a"), None);
        config.freeze_theme_compatibility();
        assert_eq!(config.post_detection_default_blocked("a"), Some(false));
    }

    #[test]
    fn applying_selected_family_work_observes_parse_control() {
        let overlay = PostDetectionConfigOverlay::new()
            .with_family_contribution(
                "flowchart",
                contribution("legacy.flowchart", json!({"a": 1, "b": 2, "c": 3})),
            )
            .unwrap();
        let explicit = MermaidConfig::empty_object();
        let before_detect = MermaidConfig::empty_object();
        let mut effective = before_detect.clone();
        let mut application = ConfigOverlayApplication::default();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);

        assert!(
            overlay
                .apply_family_controlled(
                    "flowchart",
                    &explicit,
                    &explicit,
                    &before_detect,
                    &mut effective,
                    &mut application,
                    &control,
                )
                .is_err()
        );
    }
}
