use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::Value;

use crate::config::values_equal_controlled;
use crate::resources::InputResourcePolicy;
use crate::{MermaidConfig, OperationControl, OperationControlResult};

use super::{PresentationConfigError, validation::ConfigConstructionBudget};

/// Unvalidated frozen declarations, accepted only by the checked config constructor.
///
/// Use this to preserve authored settings when projecting an effective configuration.
#[derive(Debug, Clone)]
pub struct FrozenPresentationConfigParts {
    values: MermaidConfig,
    authored_paths: Arc<BTreeSet<Arc<str>>>,
}

impl FrozenPresentationConfigParts {
    pub fn new(values: MermaidConfig) -> Self {
        Self {
            values,
            authored_paths: Arc::default(),
        }
    }

    /// Retains exact dotted ownership spellings, including empty segments and the root owner.
    ///
    /// Paths describe assignments, so they need not resolve to a present scalar. A parent scalar
    /// can own a queried descendant, and source assignments can outlive a later value removal.
    /// Literal dotted or empty keys retain the source contract's dotted-path interpretation.
    pub fn with_explicit_config_paths(mut self, paths: Arc<BTreeSet<Arc<str>>>) -> Self {
        self.authored_paths = paths;
        self
    }
}

/// Immutable resolved configuration with explicit output ownership declarations.
///
/// Equality includes the paths assigned by the caller or diagram source.
#[derive(Debug, Clone)]
pub struct PresentationConfig {
    values: MermaidConfig,
    authored_paths: Arc<BTreeSet<Arc<str>>>,
}

impl PartialEq for PresentationConfig {
    fn eq(&self, other: &Self) -> bool {
        self.authored_paths == other.authored_paths
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
            authored_paths: Arc::default(),
        }
    }

    /// Freezes ordinary caller-authored values, owning their provided leaves and containers.
    ///
    /// Empty root objects assign nothing. Other empty containers, arrays, and scalars are actual
    /// supplied values. The construction policy bounds the values and the paths derived from them.
    pub fn from_config_controlled(
        values: MermaidConfig,
        policy: &InputResourcePolicy,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Self, PresentationConfigError>> {
        let mut budget = ConfigConstructionBudget::new(policy, control);
        if let Err(error) = budget.charge_config(&values)? {
            return Ok(Err(error));
        }
        let authored_paths = match authored_paths(values.as_value(), &mut budget)? {
            Ok(paths) => paths,
            Err(error) => return Ok(Err(error)),
        };
        control.checkpoint()?;
        Ok(Ok(Self {
            values,
            authored_paths: Arc::new(authored_paths),
        }))
    }

    /// Checks the construction budget for the values and the declared authored paths.
    pub fn try_from_frozen_parts_controlled(
        parts: FrozenPresentationConfigParts,
        policy: &InputResourcePolicy,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Self, PresentationConfigError>> {
        let mut budget = ConfigConstructionBudget::new(policy, control);
        if let Err(error) = budget.charge_config(&parts.values)? {
            return Ok(Err(error));
        }
        for path in parts.authored_paths.iter() {
            if let Err(error) = budget.charge(1, path.len(), 0)? {
                return Ok(Err(error));
            }
        }
        control.checkpoint()?;
        Ok(Ok(Self {
            values: parts.values,
            authored_paths: parts.authored_paths,
        }))
    }

    pub fn as_config(&self) -> &MermaidConfig {
        &self.values
    }

    pub fn as_value(&self) -> &Value {
        self.values.as_value()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    pub fn get_path(&self, path: &[&str]) -> Option<&Value> {
        self.values.get_path(path)
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

    pub fn explicit_config_paths(&self) -> impl ExactSizeIterator<Item = &str> {
        self.authored_paths.iter().map(AsRef::as_ref)
    }

    pub fn shared_explicit_config_paths(&self) -> &Arc<BTreeSet<Arc<str>>> {
        &self.authored_paths
    }

    /// Clones only shared handles when exporting the exact frozen declarations for revalidation.
    pub fn frozen_parts(&self) -> FrozenPresentationConfigParts {
        FrozenPresentationConfigParts {
            values: self.values.clone(),
            authored_paths: Arc::clone(&self.authored_paths),
        }
    }

    /// Copies the retained root sections and ownership using the caller's cancellation control.
    /// Returns `None` when none of the supplied keys is present.
    #[doc(hidden)]
    pub fn without_root_keys_controlled(
        &self,
        keys: &[&str],
        control: &OperationControl,
    ) -> OperationControlResult<Option<Self>> {
        let Some(values) = self.values.without_root_keys_controlled(keys, control)? else {
            return Ok(None);
        };
        let mut authored_paths = BTreeSet::new();
        for path in self.authored_paths.iter() {
            control.checkpoint()?;
            if path.is_empty() {
                let object = values.as_value().as_object().expect("retained root object");
                for key in object.keys() {
                    control.checkpoint()?;
                    authored_paths.insert(Arc::from(key.as_str()));
                }
            } else if !keys.contains(&path.split_once('.').map_or(path, |(root, _)| root)) {
                authored_paths.insert(Arc::clone(path));
            }
        }
        Ok(Some(Self {
            values,
            authored_paths: Arc::new(authored_paths),
        }))
    }

    /// Alias for [`Self::explicit_config_owns_path`] during the renderer migration.
    pub fn config_paint_overrides_typed_default(&self, dotted_path: &str) -> bool {
        self.explicit_config_owns_path(dotted_path)
    }

    /// Resolves exact/ancestor/descendant ownership without scanning unrelated declarations.
    pub fn explicit_config_owns_path(&self, dotted_path: &str) -> bool {
        authored_path_overlaps(&self.authored_paths, dotted_path)
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

fn authored_path_overlaps(paths: &BTreeSet<Arc<str>>, dotted_path: &str) -> bool {
    if paths.is_empty() {
        return false;
    }
    if dotted_path.is_empty() || paths.contains("") || paths.contains(dotted_path) {
        return true;
    }
    for (index, _) in dotted_path.match_indices('.') {
        if paths.contains(&dotted_path[..index]) {
            return true;
        }
    }
    let prefix = format!("{dotted_path}.");
    paths
        .range::<str, _>((
            std::ops::Bound::Included(prefix.as_str()),
            std::ops::Bound::Unbounded,
        ))
        .next()
        .is_some_and(|candidate| candidate.starts_with(&prefix))
}
