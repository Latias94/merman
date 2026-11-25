use super::{
    MermaidConfig, config_values_equal, drop_value_nonrecursive,
    mirror_legacy_font_family_into_theme_variables_value, replace_value_nonrecursive,
};
use crate::{OperationControl, OperationControlResult};
use merman_model::json::clone_value_nonrecursive;
use merman_model::presentation::{
    FrozenPresentationConfigParts, PresentationConfig, PresentationConfigError,
};
use merman_model::resources::{InputResourcePolicy, ResourceProfile};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::{Arc, OnceLock};

/// Mutable Mermaid configuration used by source parsing and custom detectors.
#[derive(Clone)]
pub struct SourcePresentationConfig {
    inputs: MermaidConfig,
    authored_paths: Arc<BTreeSet<Arc<str>>>,
    detection: Option<Arc<DetectionBaseline>>,
    site_appearance: Option<Arc<SiteAppearance>>,
}

/// An appearance that depends only on the engine's site configuration and diagram namespace.
/// Its exact projection is validated on the first successful freeze that reuses these inputs.
#[derive(Debug)]
pub(crate) struct SiteAppearance {
    inputs: MermaidConfig,
    authored_paths: Arc<BTreeSet<Arc<str>>>,
    frozen: OnceLock<PresentationConfig>,
}

impl SiteAppearance {
    pub(crate) fn new(mut config: SourcePresentationConfig) -> Self {
        config.finish_detection();
        Self {
            inputs: config.inputs,
            authored_paths: config.authored_paths,
            frozen: OnceLock::new(),
        }
    }

    pub(crate) fn fork(self: &Arc<Self>) -> SourcePresentationConfig {
        SourcePresentationConfig {
            inputs: self.inputs.clone(),
            authored_paths: Arc::clone(&self.authored_paths),
            detection: None,
            site_appearance: Some(Arc::clone(self)),
        }
    }
}

pub(crate) struct DetectionBaseline {
    values: MermaidConfig,
    authored_paths: Arc<BTreeSet<Arc<str>>>,
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

    /// Starts source construction with raw configuration values.
    pub fn from_config(inputs: MermaidConfig) -> Self {
        Self {
            inputs,
            authored_paths: Arc::default(),
            detection: None,
            site_appearance: None,
        }
    }

    pub fn as_config(&self) -> &MermaidConfig {
        &self.inputs
    }

    pub fn as_value(&self) -> &Value {
        self.inputs.as_value()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.inputs.get(key)
    }

    pub fn get_path(&self, path: &[&str]) -> Option<&Value> {
        self.inputs.get_path(path)
    }

    pub(crate) fn with_shared_sections(mut self) -> Self {
        self.inputs = self.inputs.with_shared_sections();
        self
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

    /// Resumes source construction with the configuration's authored paths.
    pub fn from_frozen_controlled(
        config: PresentationConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Self> {
        control.checkpoint()?;
        Ok(Self {
            inputs: config.as_config().clone(),
            authored_paths: Arc::clone(config.shared_explicit_config_paths()),
            detection: None,
            site_appearance: None,
        })
    }

    /// Freezes the effective values and authored paths for render consumers.
    pub fn into_presentation_config_controlled(
        mut self,
        policy: &InputResourcePolicy,
        control: &OperationControl,
    ) -> OperationControlResult<Result<PresentationConfig, PresentationConfigError>> {
        control.checkpoint()?;
        self.finish_detection();
        // Both shared identities must survive source and detector edits. The memo uses the
        // ordinary source parse policy. Arbitrary host policies still check the same data.
        let site = self.site_appearance.take().filter(|site| {
            self.inputs.shares_storage_with(&site.inputs)
                && Arc::ptr_eq(&self.authored_paths, &site.authored_paths)
                && *policy == source_config_construction_policy()
        });
        if let Some(frozen) = site.as_ref().and_then(|site| site.frozen.get()) {
            return Ok(Ok(frozen.clone()));
        }
        #[cfg(test)]
        super::record_config_work(|work| {
            work.authored_path_validations += self.authored_paths.len()
        });
        let parts = FrozenPresentationConfigParts::new(self.inputs)
            .with_explicit_config_paths(self.authored_paths);
        let result = PresentationConfig::try_from_frozen_parts_controlled(parts, policy, control)?;
        if let (Some(site), Ok(frozen)) = (site, &result) {
            let _ = site.frozen.set(frozen.clone());
        }
        Ok(result)
    }

    #[cfg(test)]
    pub(crate) fn explicit_config_owns_path(&self, dotted_path: &str) -> bool {
        self.authored_paths
            .iter()
            .chain(
                self.detection
                    .iter()
                    .flat_map(|baseline| baseline.authored_paths.iter()),
            )
            .any(|candidate| {
                candidate.is_empty()
                    || dotted_path.is_empty()
                    || candidate.as_ref() == dotted_path
                    || candidate
                        .strip_prefix(dotted_path)
                        .is_some_and(|suffix| suffix.starts_with('.'))
                    || dotted_path
                        .strip_prefix(candidate.as_ref())
                        .is_some_and(|suffix| suffix.starts_with('.'))
            })
    }

    pub fn as_value_mut(&mut self) -> &mut Value {
        self.claim_path("");
        self.inputs.as_value_mut()
    }

    pub fn set_value(&mut self, dotted_path: &str, value: Value) {
        self.claim_path(dotted_path);
        self.inputs.set_value(dotted_path, value);
    }

    pub(crate) fn set_computed_value(&mut self, dotted_path: &str, value: Value) {
        self.inputs.set_value(dotted_path, value);
    }

    pub fn deep_merge(&mut self, other: &Value) {
        let paths = &mut self.authored_paths;
        self.inputs
            .merge_with_mutation_observer(other, &mut |segments| {
                Arc::make_mut(paths).insert(Arc::from(segments.join(".")));
            });
    }

    pub(crate) fn mirror_legacy_font_family(&mut self) {
        mirror_legacy_font_family_into_theme_variables_value(self.inputs.as_value_mut());
    }

    fn claim_path(&mut self, dotted_path: &str) {
        if !self.authored_paths.contains(dotted_path) {
            Arc::make_mut(&mut self.authored_paths).insert(Arc::from(dotted_path));
        }
    }

    /// Records the input seen by the detectors so an alternate Mermaid palette can replay edits.
    pub(crate) fn begin_detection(&mut self) -> Arc<DetectionBaseline> {
        let baseline = Arc::new(DetectionBaseline {
            values: self.inputs.clone(),
            authored_paths: std::mem::take(&mut self.authored_paths),
        });
        self.detection = Some(Arc::clone(&baseline));
        baseline
    }

    fn retained_authored_paths(&self) -> Arc<BTreeSet<Arc<str>>> {
        let Some(baseline) = self.detection.as_ref() else {
            return Arc::clone(&self.authored_paths);
        };
        if baseline.authored_paths.is_empty() {
            return Arc::clone(&self.authored_paths);
        }
        let mut paths = Arc::clone(&baseline.authored_paths);
        if !self.authored_paths.is_empty() {
            Arc::make_mut(&mut paths).extend(self.authored_paths.iter().cloned());
        }
        paths
    }

    pub(crate) fn finish_detection(&mut self) {
        self.authored_paths = self.retained_authored_paths();
        self.detection = None;
    }

    /// Reapplies custom detector values when selecting a different Mermaid palette.
    pub(crate) fn replay_detection(
        &mut self,
        baseline: &Arc<DetectionBaseline>,
        detected: &Self,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let after = detected.as_config();
        let assigned_paths = if detected
            .detection
            .as_ref()
            .is_some_and(|operation| Arc::ptr_eq(operation, baseline))
        {
            Arc::clone(&detected.authored_paths)
        } else {
            let replacement_paths = detected.retained_authored_paths();
            self.authored_paths = Arc::clone(&replacement_paths);
            replacement_paths
        };
        let before = &baseline.values;
        if !before.shares_storage_with(after)
            && !config_values_equal(before.as_value(), after.as_value(), Some(control))?
        {
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
                Some(after.as_value()),
            )];
            while let Some(frame) = pending.pop() {
                control.checkpoint()?;
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
                    Frame::Visit(_, after) => self.replace_detector_segments(&path, after),
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
        for path in assigned_paths.iter() {
            control.checkpoint()?;
            self.claim_path(path);
            let value = if path.is_empty() {
                Some(after.as_value())
            } else {
                after.get_dotted_path(path)
            };
            self.replace_detector_value(path, value);
        }
        Ok(())
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
        // String equality is exact. Replay records explicit claims separately and continues
        // to replace containers so their contents and key order follow the detector.
        if let Some(Value::String(replacement)) = value
            && if path.is_empty() {
                self.as_value().as_str() == Some(replacement)
            } else {
                self.get_path(path).and_then(Value::as_str) == Some(replacement)
            }
        {
            return;
        }
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
}

pub(crate) fn apply_hardened_site_policy(config: &mut SourcePresentationConfig) {
    config.set_computed_value(
        "secure",
        Value::Array(
            super::HARDENED_SECURE_KEYS
                .iter()
                .map(|key| Value::String((*key).to_string()))
                .collect(),
        ),
    );
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
    use merman_model::resources::InputResourceLimitId;
    use serde_json::json;

    #[test]
    fn frozen_site_appearance_rechecks_ownership_and_host_limits() {
        let mut base = SourcePresentationConfig::from_value(json!({
            "look": "neo", "theme": "dark",
        }));
        base.set_value("look", json!("neo"));
        let site = Arc::new(SiteAppearance::new(base));
        let control = OperationControl::new();
        let source_policy = source_config_construction_policy();
        let baseline = site
            .fork()
            .into_presentation_config_controlled(&source_policy, &control)
            .unwrap()
            .unwrap();
        assert!(site.frozen.get().is_some());

        let limited = InputResourcePolicy::default()
            .with_limit(InputResourceLimitId::MaxModelItems, 3)
            .unwrap();
        let result = site
            .fork()
            .into_presentation_config_controlled(&limited, &control)
            .unwrap();
        assert!(
            matches!(result, Err(PresentationConfigError::ResourceLimit(error))
            if error.limit == "max_model_items" && error.actual == 4)
        );

        let mut reassigned = site.fork();
        reassigned.set_value("theme", json!("dark"));
        let reassigned = reassigned
            .into_presentation_config_controlled(&source_policy, &control)
            .unwrap()
            .unwrap();
        assert!(
            reassigned
                .as_config()
                .shares_storage_with(baseline.as_config())
        );
        assert!(reassigned.explicit_config_owns_path("theme"));
        assert!(!baseline.explicit_config_owns_path("theme"));

        control.cancel();
        assert!(
            site.fork()
                .into_presentation_config_controlled(&source_policy, &control)
                .is_err()
        );
    }

    #[test]
    fn detector_replay_keeps_literal_keys_and_leaves_unedited_values_in_the_new_palette() {
        let mut before = SourcePresentationConfig::from_value(json!({
            "nested": {"deleted": 1}, "same": 2, "literal.key": 3,
        }));
        let baseline = before.begin_detection();
        let after = SourcePresentationConfig::from_value(json!({"same": 2, "literal.key": 4}));
        let mut target = SourcePresentationConfig::from_value(json!({
            "nested": {"deleted": 1}, "same": 99, "literal.key": 3,
        }));
        target
            .replay_detection(&baseline, &after, &OperationControl::new())
            .unwrap();
        assert_eq!(target.as_value(), &json!({"same": 99, "literal.key": 4}));
        assert!(!target.explicit_config_owns_path("nested"));
        assert!(!target.explicit_config_owns_path("literal.key"));

        let control = OperationControl::new();
        control.cancel();
        assert!(
            target
                .replay_detection(&baseline, &after, &control)
                .is_err()
        );
    }

    #[test]
    fn replacing_config_with_another_detection_keeps_its_own_assignments() {
        let mut outer = SourcePresentationConfig::empty_object();
        outer.set_value("choice", json!("outer"));
        let baseline = outer.begin_detection();
        let mut inner = SourcePresentationConfig::empty_object();
        inner.set_value("choice", json!("inner"));
        let _ = inner.begin_detection();
        inner.set_value("custom", json!(true));

        let mut target = SourcePresentationConfig::empty_object();
        target.set_value("choice", json!("new-palette"));
        target
            .replay_detection(&baseline, &inner, &OperationControl::new())
            .unwrap();
        assert_eq!(
            target.as_value(),
            &json!({"choice": "inner", "custom": true})
        );
        assert!(target.explicit_config_owns_path("choice"));
        assert!(target.explicit_config_owns_path("custom"));
    }
}
