//! Shared Mermaid configuration values with iterative ownership and pinned merge behavior.

mod theme_id;
pub use theme_id::{MermaidThemeId, MermaidThemeIdParseError};

use crate::json::estimated_value_owned_heap_bytes;
use crate::json::{
    ManagedJson, clone_object_nonrecursive_controlled, clone_value_nonrecursive,
    clone_value_nonrecursive_controlled, drop_value_nonrecursive,
};
#[cfg(feature = "presentation-data")]
use crate::resources::ModelComplexity;
use crate::{OperationControl, OperationControlResult};
use serde_json::{Map, Value};
use std::fmt;
use std::ops::ControlFlow;
use std::sync::{Arc, OnceLock};

#[derive(Clone)]
pub struct MermaidConfig {
    #[cfg(test)]
    before_value_release: Option<tests::BeforeValueRelease>,
    value: Arc<ConfigValue>,
    share_sections_on_write: bool,
}

#[derive(Clone)]
struct ConfigValue {
    storage: ConfigStorage,
    #[cfg(feature = "presentation-data")]
    construction_complexity: OnceLock<ModelComplexity>,
}

#[derive(Clone)]
enum ConfigStorage {
    Json(ManagedJson),
    Sections(SectionOverrides),
}

/// The base is a flat object. A request copies a section only when it writes to that section.
/// Full JSON is a compatibility projection and is cached only after a consumer asks for it.
struct SectionOverrides {
    base: Arc<ConfigValue>,
    changed: ManagedJson,
    flattened: OnceLock<ManagedJson>,
}

impl Clone for SectionOverrides {
    fn clone(&self) -> Self {
        Self {
            base: Arc::clone(&self.base),
            changed: self.changed.clone(),
            flattened: OnceLock::new(),
        }
    }
}

impl SectionOverrides {
    fn base_object(&self) -> &Map<String, Value> {
        let ConfigStorage::Json(base) = &self.base.storage else {
            unreachable!("config sections use one flat base")
        };
        base.as_value()
            .as_object()
            .expect("config base is an object")
    }

    fn changed_object(&self) -> &Map<String, Value> {
        self.changed
            .as_value()
            .as_object()
            .expect("changed sections form an object")
    }

    fn edited_object(&mut self) -> &mut Map<String, Value> {
        self.flattened.take();
        self.changed
            .as_value_mut()
            .as_object_mut()
            .expect("changed sections form an object")
    }

    fn insert(&mut self, key: &str, value: Value) {
        if let Some(old) = self.edited_object().insert(key.to_owned(), value) {
            drop_value_nonrecursive(old);
        }
    }

    fn edit(&mut self, key: &str, default: impl FnOnce() -> Value) -> &mut Value {
        if !self.changed_object().contains_key(key) {
            let value = self
                .base_object()
                .get(key)
                .map_or_else(default, clone_value_nonrecursive);
            self.insert(key, value);
        }
        self.edited_object()
            .get_mut(key)
            .expect("editable config section")
    }

    fn entries(&self) -> SectionEntries<'_> {
        let base = self.base_object();
        let changed = self.changed_object();
        SectionEntries {
            base,
            changed,
            base_entries: base.iter(),
            changed_entries: changed.iter(),
        }
    }

    fn flatten(&self) -> ManagedJson {
        let object = self
            .entries()
            .map(|(key, value)| (key.clone(), clone_value_nonrecursive(value)))
            .collect();
        ManagedJson::from_value(Value::Object(object))
    }
}

#[derive(Clone)]
pub(crate) struct SectionEntries<'a> {
    base: &'a Map<String, Value>,
    changed: &'a Map<String, Value>,
    base_entries: serde_json::map::Iter<'a>,
    changed_entries: serde_json::map::Iter<'a>,
}

impl<'a> Iterator for SectionEntries<'a> {
    type Item = (&'a String, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((key, original)) = self.base_entries.next() {
            return Some((key, self.changed.get(key).unwrap_or(original)));
        }
        self.changed_entries
            .find(|(key, _)| !self.base.contains_key(key.as_str()))
    }
}

impl ConfigValue {
    fn json(&self) -> &ManagedJson {
        match &self.storage {
            ConfigStorage::Json(json) => json,
            ConfigStorage::Sections(sections) => {
                sections.flattened.get_or_init(|| sections.flatten())
            }
        }
    }
}

impl From<ManagedJson> for ConfigValue {
    fn from(json: ManagedJson) -> Self {
        Self {
            storage: ConfigStorage::Json(json),
            #[cfg(feature = "presentation-data")]
            construction_complexity: OnceLock::new(),
        }
    }
}

impl fmt::Debug for MermaidConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("MermaidConfig")
            .field(self.value.json())
            .finish()
    }
}

impl PartialEq for MermaidConfig {
    fn eq(&self, other: &Self) -> bool {
        self.shares_storage_with(other) || self.value.json() == other.value.json()
    }
}

impl Eq for MermaidConfig {}

impl Default for MermaidConfig {
    fn default() -> Self {
        Self::empty_object()
    }
}

impl MermaidConfig {
    pub fn empty_object() -> Self {
        Self::from_value(Value::Object(Map::new()))
    }

    pub fn from_value(value: Value) -> Self {
        Self {
            #[cfg(test)]
            before_value_release: None,
            value: Arc::new(ManagedJson::from_value(value).into()),
            share_sections_on_write: false,
        }
    }

    pub fn as_value(&self) -> &Value {
        self.value.json().as_value()
    }

    /// Takes an independent compatibility projection without retaining a second JSON tree.
    pub fn to_managed_json(&self) -> ManagedJson {
        match &self.value.storage {
            ConfigStorage::Json(json) => json.clone(),
            ConfigStorage::Sections(sections) => sections
                .flattened
                .get()
                .map_or_else(|| sections.flatten(), Clone::clone),
        }
    }

    /// Enables copy-on-write at the root section boundary for an operation-local config fork.
    #[must_use]
    pub fn with_shared_sections(mut self) -> Self {
        self.share_sections_on_write = true;
        self
    }

    /// Finds one root member without constructing a full JSON projection.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match &self.value.storage {
            ConfigStorage::Json(json) => json.as_value().get(key),
            ConfigStorage::Sections(sections) => sections
                .changed_object()
                .get(key)
                .or_else(|| sections.base_object().get(key)),
        }
    }

    pub fn get_path(&self, path: &[&str]) -> Option<&Value> {
        let (first, rest) = path.split_first()?;
        rest.iter()
            .try_fold(self.get(first)?, |value, segment| value.get(*segment))
    }

    pub fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }

    pub fn get_dotted_path(&self, dotted_path: &str) -> Option<&Value> {
        let mut segments = dotted_path.split('.');
        let first = segments.next()?;
        segments.try_fold(self.get(first)?, |value, segment| value.get(segment))
    }

    #[cfg(feature = "presentation-data")]
    pub(crate) fn construction_complexity(&self) -> &OnceLock<ModelComplexity> {
        &self.value.construction_complexity
    }

    pub(crate) fn section_entries(&self) -> Option<(usize, SectionEntries<'_>)> {
        let ConfigStorage::Sections(sections) = &self.value.storage else {
            return None;
        };
        let entries = sections.entries();
        let additions = entries
            .changed
            .keys()
            .filter(|key| !entries.base.contains_key(key.as_str()))
            .count();
        Some((entries.base.len() + additions, entries))
    }

    /// Clones this config without recursion while enforcing a retained-size and nesting budget.
    ///
    /// `Ok(None)` means the owned value would exceed either budget. Cancellation remains distinct
    /// from budget rejection so callers can abandon an enclosing operation immediately.
    pub fn clone_value_bounded_controlled(
        &self,
        max_retained_bytes: usize,
        max_nesting_depth: usize,
        control: &OperationControl,
    ) -> OperationControlResult<Option<Value>> {
        control.checkpoint()?;
        if let Some((len, entries)) = self.section_entries() {
            return clone_object_nonrecursive_controlled(
                len,
                entries,
                max_retained_bytes,
                max_nesting_depth,
                control,
            );
        }
        clone_value_nonrecursive_controlled(
            self.as_value(),
            max_retained_bytes,
            max_nesting_depth,
            control,
        )
    }

    pub fn is_empty_object(&self) -> bool {
        match &self.value.storage {
            ConfigStorage::Json(json) => {
                matches!(json.as_value(), Value::Object(map) if map.is_empty())
            }
            ConfigStorage::Sections(sections) => {
                sections.base_object().is_empty() && sections.changed_object().is_empty()
            }
        }
    }

    fn is_object(&self) -> bool {
        match &self.value.storage {
            ConfigStorage::Json(json) => json.as_value().is_object(),
            ConfigStorage::Sections(_) => true,
        }
    }

    /// Estimates owned JSON allocation weight for retained-evidence budgeting.
    pub fn estimated_owned_heap_bytes(&self) -> usize {
        estimated_value_owned_heap_bytes(self.as_value())
    }

    pub fn as_value_mut(&mut self) -> &mut Value {
        self.value_mut()
    }

    pub fn get_str(&self, dotted_path: &str) -> Option<&str> {
        self.get_dotted_path(dotted_path)?.as_str()
    }

    pub fn get_bool(&self, dotted_path: &str) -> Option<bool> {
        self.get_dotted_path(dotted_path)?.as_bool()
    }

    pub fn set_value(&mut self, dotted_path: &str, value: Value) {
        self.set_value_raw(dotted_path, value);
    }

    fn set_value_raw(&mut self, dotted_path: &str, value: Value) {
        let current = self.get_dotted_path(dotted_path);
        if current.is_some_and(|current| {
            if current.is_object() || current.is_array() || value.is_object() || value.is_array() {
                values_equal_controlled(current, &value, None)
                    .expect("comparison without operation control cannot cancel")
            } else {
                current == &value
            }
        }) {
            drop_value_nonrecursive(value);
            return;
        }
        if let Some(sections) = self.sections_mut() {
            if let Some((root, tail)) = dotted_path.split_once('.') {
                Self::set_in_object(sections.edit(root, || Value::Null), tail, value);
            } else {
                sections.insert(dotted_path, value);
            }
            return;
        }
        Self::set_in_object(self.value_mut(), dotted_path, value);
    }

    fn set_in_object(root_value: &mut Value, dotted_path: &str, value: Value) {
        // Be defensive: callers can construct `MermaidConfig` from any JSON value via
        // `from_value`. Mermaid configs are objects; if we see a non-object here, coerce it
        // to an object so this API never panics on user input.
        if !root_value.is_object() {
            replace_value_nonrecursive(root_value, Value::Object(Map::new()));
        }

        let Value::Object(root) = root_value else {
            return;
        };
        let mut cur: &mut Map<String, Value> = root;
        let mut segments = dotted_path.split('.').peekable();
        while let Some(seg) = segments.next() {
            if segments.peek().is_none() {
                if let Some(old) = cur.insert(seg.to_string(), value) {
                    drop_value_nonrecursive(old);
                }
                return;
            }
            let slot = cur.entry(seg).or_insert_with(|| Value::Object(Map::new()));
            if !slot.is_object() {
                replace_value_nonrecursive(slot, Value::Object(Map::new()));
            }
            let Some(next) = slot.as_object_mut() else {
                return;
            };
            cur = next;
        }
    }

    pub fn deep_merge(&mut self, other: &Value) {
        self.merge_with_mutation_observer(other, &mut |_| {});
    }

    /// Merges Mermaid values while reporting accepted assignments in encounter order.
    ///
    /// The observer receives path segments during this call and cannot retain a borrowed path.
    /// Source owners use these events without changing the value container's merge semantics.
    pub fn merge_with_mutation_observer<F>(&mut self, other: &Value, record: &mut F)
    where
        F: FnMut(&[&str]),
    {
        if matches!(other, Value::Object(map) if map.is_empty()) {
            return;
        }
        if self.share_sections_on_write && self.is_object() {
            match other {
                Value::Object(updates) => self.merge_sections(updates, record),
                Value::Array(items) => {
                    let _ = visit_array_updates(items, |update| {
                        if let Value::Object(updates) = update {
                            self.merge_sections(updates, record);
                        }
                        ControlFlow::Continue(())
                    });
                }
                _ => {}
            }
            return;
        }
        let mut path = Vec::new();
        let mut reported_prefix = 0usize;
        let first_write =
            MergeTarget::Shared(self.as_value()).apply(other, 2, &mut path, &mut |path| {
                record(path);
                reported_prefix += 1;
            });
        if first_write.is_break() {
            let result =
                MergeTarget::Writable(self.value_mut()).apply(other, 2, &mut path, &mut |path| {
                    if reported_prefix == 0 {
                        record(path);
                    } else {
                        reported_prefix -= 1;
                    }
                });
            debug_assert!(result.is_continue() && reported_prefix == 0);
        }
    }

    fn sections_mut(&mut self) -> Option<&mut SectionOverrides> {
        if !self.share_sections_on_write {
            return None;
        }
        if matches!(self.value.storage, ConfigStorage::Json(_)) {
            if !self.is_object() || Arc::get_mut(&mut self.value).is_some() {
                return None;
            }
            self.value = Arc::new(ConfigValue {
                storage: ConfigStorage::Sections(SectionOverrides {
                    base: Arc::clone(&self.value),
                    changed: ManagedJson::from_value(Value::Object(Map::new())),
                    flattened: OnceLock::new(),
                }),
                #[cfg(feature = "presentation-data")]
                construction_complexity: OnceLock::new(),
            });
        }
        let value = Arc::make_mut(&mut self.value);
        #[cfg(feature = "presentation-data")]
        value.construction_complexity.take();
        let ConfigStorage::Sections(sections) = &mut value.storage else {
            unreachable!("shared object fork has section storage")
        };
        Some(sections)
    }

    fn edit_section(
        &mut self,
        key: &str,
        default: impl FnOnce() -> Value,
        edit: impl FnOnce(&mut Value),
    ) {
        if let Some(sections) = self.sections_mut() {
            edit(sections.edit(key, default));
        } else {
            let object = self
                .value_mut()
                .as_object_mut()
                .expect("config is an object");
            edit(object.entry(key).or_insert_with(default));
        }
    }

    fn merge_sections(&mut self, updates: &Map<String, Value>, record: &mut impl FnMut(&[&str])) {
        let mut path = Vec::new();
        for (key, update) in updates {
            let destination = self.get(key);
            match merge_member_action(destination, update) {
                MergeMember::Ignore => {}
                MergeMember::Assign => {
                    record(&[key]);
                    if !destination.is_some_and(|value| unchanged_scalar(value, update)) {
                        let replacement = clone_value_nonrecursive(update);
                        if let Some(sections) = self.sections_mut() {
                            sections.insert(key, replacement);
                        } else {
                            self.edit_section(
                                key,
                                || Value::Null,
                                |slot| replace_value_nonrecursive(slot, replacement),
                            );
                        }
                    }
                }
                MergeMember::Merge => {
                    path.push(key.as_str());
                    let mut reported = 0usize;
                    let needs_write = destination.is_none_or(|value| {
                        MergeTarget::Shared(value)
                            .apply(update, 1, &mut path, &mut |path| {
                                record(path);
                                reported += 1;
                            })
                            .is_break()
                    });
                    if needs_write {
                        self.edit_section(
                            key,
                            || empty_container_for(update),
                            |section| {
                                let result = MergeTarget::Writable(section).apply(
                                    update,
                                    1,
                                    &mut path,
                                    &mut |path| {
                                        if reported == 0 {
                                            record(path);
                                        } else {
                                            reported -= 1;
                                        }
                                    },
                                );
                                debug_assert!(result.is_continue() && reported == 0);
                            },
                        );
                    }
                    path.pop();
                }
            }
        }
    }

    fn value_mut(&mut self) -> &mut Value {
        if let Some(value) = Arc::get_mut(&mut self.value) {
            if let ConfigStorage::Sections(sections) = &mut value.storage {
                let json = sections.flattened.take().unwrap_or_else(|| {
                    #[cfg(any(test, feature = "test-support"))]
                    test_support::record_cow_copy();
                    sections.flatten()
                });
                value.storage = ConfigStorage::Json(json);
            }
        } else {
            #[cfg(any(test, feature = "test-support"))]
            test_support::record_cow_copy();
            self.value = Arc::new(self.to_managed_json().into());
        }
        let value = Arc::make_mut(&mut self.value);
        #[cfg(feature = "presentation-data")]
        value.construction_complexity.take();
        let ConfigStorage::Json(json) = &mut value.storage else {
            unreachable!("config is flat before exposing raw mutation")
        };
        json.as_value_mut()
    }
}

pub fn values_equal_controlled(
    left: &Value,
    right: &Value,
    control: Option<&OperationControl>,
) -> OperationControlResult<bool> {
    let mut pending = vec![(left, right)];
    while let Some((left, right)) = pending.pop() {
        if let Some(control) = control {
            control.checkpoint()?;
        }
        match (left, right) {
            (Value::Object(left), Value::Object(right)) if left.len() == right.len() => {
                for (key, left) in left {
                    let Some(right) = right.get(key) else {
                        return Ok(false);
                    };
                    pending.push((left, right));
                }
            }
            (Value::Array(left), Value::Array(right)) if left.len() == right.len() => {
                pending.extend(left.iter().zip(right));
            }
            (Value::Object(_) | Value::Array(_), _) | (_, Value::Object(_) | Value::Array(_)) => {
                return Ok(false);
            }
            _ if left != right => return Ok(false),
            _ => {}
        }
    }
    Ok(true)
}

/// The shared traversal stops at the first write. A writable traversal then replays its
/// unchanged prefix and continues in the detached tree. Both descend by borrowing each
/// child directly, keeping ancestor keys out of the inner object and array loops.
enum MergeTarget<'value> {
    Shared(&'value Value),
    Writable(&'value mut Value),
}

enum MergeMember {
    Ignore,
    Assign,
    Merge,
}

fn merge_member_action(destination: Option<&Value>, source: &Value) -> MergeMember {
    if source.is_null() {
        MergeMember::Ignore
    } else if is_non_null_js_object(source) {
        if destination.is_none_or(is_js_object) {
            MergeMember::Merge
        } else {
            MergeMember::Ignore
        }
    } else if destination.is_none_or(|value| !is_js_object(value)) {
        MergeMember::Assign
    } else {
        MergeMember::Ignore
    }
}

fn visit_array_updates<'source>(
    source_items: &'source [Value],
    mut visit: impl FnMut(&'source Value) -> ControlFlow<()>,
) -> ControlFlow<()> {
    let mut pending = source_items.iter().rev().collect::<Vec<_>>();
    while let Some(source) = pending.pop() {
        if let Value::Array(items) = source {
            pending.extend(items.iter().rev());
        } else {
            visit(source)?;
        }
    }
    ControlFlow::Continue(())
}

impl MergeTarget<'_> {
    fn value(&self) -> &Value {
        match self {
            Self::Shared(value) => value,
            Self::Writable(value) => value,
        }
    }

    fn child(&mut self, key: &str) -> MergeTarget<'_> {
        match self {
            Self::Shared(value) => MergeTarget::Shared(value.get(key).expect("merge child exists")),
            Self::Writable(value) => {
                MergeTarget::Writable(value.get_mut(key).expect("merge child exists"))
            }
        }
    }

    fn object_mut(&mut self) -> ControlFlow<(), &mut Map<String, Value>> {
        match self {
            Self::Shared(_) => ControlFlow::Break(()),
            Self::Writable(value) => {
                ControlFlow::Continue(value.as_object_mut().expect("merge parent is an object"))
            }
        }
    }

    fn apply<'source>(
        &mut self,
        source: &'source Value,
        depth: usize,
        path: &mut Vec<&'source str>,
        record: &mut impl FnMut(&[&str]),
    ) -> ControlFlow<()> {
        if let Value::Array(source_items) = source {
            match self.value() {
                Value::Array(_) if !source_items.is_empty() => {
                    record(path);
                    return self.append_array_items(source_items);
                }
                Value::Object(_) => {
                    return visit_array_updates(source_items, |source| {
                        self.apply(source, depth, path, record)
                    });
                }
                _ => {}
            }
            return ControlFlow::Continue(());
        }

        if self.value().is_null() {
            return self.replace(source, path, record);
        }
        if source.is_null() {
            return ControlFlow::Continue(());
        }
        if depth == 0 {
            return self.shallow_assign(source, path, record);
        }

        let (Value::Object(_), Value::Object(source_map)) = (self.value(), source) else {
            return ControlFlow::Continue(());
        };
        for (key, source_child) in source_map {
            let destination_child = self.value().get(key);
            match merge_member_action(destination_child, source_child) {
                MergeMember::Ignore => {}
                MergeMember::Assign => self.assign_member(key, source_child, path, record)?,
                MergeMember::Merge => {
                    if destination_child.is_none() {
                        self.object_mut()?
                            .insert(key.clone(), empty_container_for(source_child));
                    }
                    path.push(key);
                    let result = self.child(key).apply(source_child, depth - 1, path, record);
                    path.pop();
                    result?;
                }
            }
        }
        ControlFlow::Continue(())
    }

    fn append_array_items(&mut self, source: &[Value]) -> ControlFlow<()> {
        match self {
            Self::Shared(Value::Array(destination)) => {
                if source.iter().any(|item| !array_contains(destination, item)) {
                    return ControlFlow::Break(());
                }
            }
            Self::Writable(Value::Array(destination)) => {
                for item in source {
                    if !array_contains(destination, item) {
                        destination.push(clone_value_nonrecursive(item));
                    }
                }
            }
            _ => unreachable!("array merge destination"),
        }
        ControlFlow::Continue(())
    }

    fn shallow_assign<'source>(
        &mut self,
        source: &'source Value,
        path: &mut Vec<&'source str>,
        record: &mut impl FnMut(&[&str]),
    ) -> ControlFlow<()> {
        match (self.value(), source) {
            (Value::Object(_), Value::Object(source_map)) => {
                for (key, source_child) in source_map {
                    self.assign_member(key, source_child, path, record)?;
                }
                ControlFlow::Continue(())
            }
            // JavaScript arrays can own named properties, while JSON arrays cannot represent them.
            (Value::Array(_), Value::Object(_)) => ControlFlow::Continue(()),
            _ => self.replace(source, path, record),
        }
    }

    fn replace(
        &mut self,
        source: &Value,
        path: &[&str],
        record: &mut impl FnMut(&[&str]),
    ) -> ControlFlow<()> {
        record(path);
        match self {
            Self::Shared(value) if unchanged_scalar(value, source) => {}
            Self::Shared(_) => return ControlFlow::Break(()),
            Self::Writable(value) if !unchanged_scalar(value, source) => {
                replace_value_nonrecursive(value, clone_value_nonrecursive(source));
            }
            Self::Writable(_) => {}
        }
        ControlFlow::Continue(())
    }

    fn assign_member<'source>(
        &mut self,
        key: &'source str,
        source: &Value,
        path: &mut Vec<&'source str>,
        record: &mut impl FnMut(&[&str]),
    ) -> ControlFlow<()> {
        path.push(key);
        record(path);
        path.pop();
        if self
            .value()
            .get(key)
            .is_some_and(|current| unchanged_scalar(current, source))
        {
            return ControlFlow::Continue(());
        }
        if let Some(previous) = self
            .object_mut()?
            .insert(key.to_owned(), clone_value_nonrecursive(source))
        {
            drop_value_nonrecursive(previous);
        }
        ControlFlow::Continue(())
    }
}

fn array_contains(destination: &[Value], item: &Value) -> bool {
    is_json_primitive(item)
        && destination
            .iter()
            .any(|present| same_json_primitive(present, item))
}

fn unchanged_scalar(destination: &Value, source: &Value) -> bool {
    match (destination, source) {
        (Value::Number(old), Value::Number(new)) => {
            old == new
                && (!old.is_f64()
                    || old.as_f64().map(f64::to_bits) == new.as_f64().map(f64::to_bits))
        }
        (Value::Null | Value::Bool(_) | Value::String(_), _) => destination == source,
        _ => false,
    }
}

fn empty_container_for(value: &Value) -> Value {
    if value.is_array() {
        Value::Array(Vec::new())
    } else {
        Value::Object(Map::new())
    }
}

fn is_js_object(value: &Value) -> bool {
    matches!(value, Value::Null | Value::Array(_) | Value::Object(_))
}

fn is_non_null_js_object(value: &Value) -> bool {
    matches!(value, Value::Array(_) | Value::Object(_))
}

fn is_json_primitive(value: &Value) -> bool {
    matches!(
        value,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
    )
}

fn same_json_primitive(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Number(left), Value::Number(right)) => left.as_f64() == right.as_f64(),
        (Value::String(left), Value::String(right)) => left == right,
        _ => false,
    }
}

pub fn replace_value_nonrecursive(slot: &mut Value, value: Value) {
    let old = std::mem::replace(slot, value);
    drop_value_nonrecursive(old);
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use std::cell::RefCell;

    thread_local! {
        static COW_COPIES: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn record_cow_copy() {
        COW_COPIES.with(|scopes| {
            for copies in scopes.borrow_mut().iter_mut() {
                *copies += 1;
            }
        });
    }

    /// Counts actual copy-on-write clones within a test operation, including nested scopes.
    pub fn measure_cow_copies<T>(operation: impl FnOnce() -> T) -> (T, usize) {
        struct Scope;
        impl Drop for Scope {
            fn drop(&mut self) {
                COW_COPIES.with(|scopes| {
                    scopes.borrow_mut().pop();
                });
            }
        }
        COW_COPIES.with(|scopes| scopes.borrow_mut().push(0));
        let scope = Scope;
        let result = operation();
        let copies = COW_COPIES.with(|scopes| *scopes.borrow().last().expect("active COW scope"));
        drop(scope);
        (result, copies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::process::{Command, Stdio};
    use std::sync::{Barrier, mpsc};
    use std::time::{Duration, Instant};

    #[derive(Clone)]
    pub(super) struct BeforeValueRelease {
        ready: Arc<Barrier>,
        release: Arc<Barrier>,
    }

    impl Drop for BeforeValueRelease {
        fn drop(&mut self) {
            self.ready.wait();
            self.release.wait();
        }
    }

    #[test]
    fn deep_merge_ignores_null_source_values() {
        let mut config = MermaidConfig::from_value(json!({
            "theme": "default",
            "flowchart": {
                "htmlLabels": true
            }
        }));

        config.deep_merge(&json!({
            "theme": null,
            "flowchart": {
                "htmlLabels": null,
                "curve": "basis"
            },
            "newValue": null
        }));

        assert_eq!(
            config.as_value(),
            &json!({
                "theme": "default",
                "flowchart": {
                    "htmlLabels": true,
                    "curve": "basis"
                }
            })
        );

        config.deep_merge(&Value::Null);
        assert_eq!(config.get_str("theme"), Some("default"));
    }

    #[test]
    fn section_writes_preserve_order_and_merge_events_without_flattening() {
        let baseline = MermaidConfig::from_value(json!({
            "first": "unchanged",
            "flowchart": { "spacing": 50, "labels": {"type": "html"} },
            "secure": ["startOnLoad"],
            "empty": {},
            "last": 0.0,
        }));
        let source = json!({
            "newSection": {},
            "first": "unchanged",
            "flowchart": {"spacing": 72, "labels": {"mode": "wrap", "x": null}},
            "secure": ["startOnLoad", "themeCSS"],
            "empty": false,
            "last": -0.0,
            "custom.a": {},
        });
        let merge = |mut config: MermaidConfig| {
            let mut paths = Vec::new();
            config.merge_with_mutation_observer(&source, &mut |path| paths.push(path.join(".")));
            config.set_value("flowchart.labels.type", json!("text"));
            config.set_value("inserted", json!(true));
            (config, paths)
        };

        let (flat, flat_paths) = merge(baseline.clone());
        let (layered, layered_paths) = merge(baseline.clone().with_shared_sections());
        let ConfigStorage::Sections(sections) = &layered.value.storage else {
            panic!("expected a section fork");
        };
        assert!(sections.flattened.get().is_none());
        assert_eq!(layered_paths, flat_paths);
        assert!(std::ptr::eq(
            layered.get("empty").unwrap(),
            baseline.get("empty").unwrap()
        ));
        assert_eq!(layered.get_str("flowchart.labels.type"), Some("text"));
        assert_eq!(
            serde_json::to_vec(&layered.to_managed_json()).unwrap(),
            serde_json::to_vec(&flat.to_managed_json()).unwrap()
        );
        assert!(sections.flattened.get().is_none());
    }

    #[test]
    fn section_merges_keep_empty_and_ordered_array_sources_lazy() {
        let base = MermaidConfig::from_value(json!({
            "large": "x".repeat(32 * 1_024),
            "flowchart": {"nodeSpacing": 50},
        }));
        let mut flat = base.clone();
        let mut layered = base.clone().with_shared_sections();
        flat.set_value("flowchart.rankSpacing", json!(61));
        layered.set_value("flowchart.rankSpacing", json!(61));
        for source in [Value::Null, json!([]), json!([[null, []], true, {}])] {
            layered.merge_with_mutation_observer(&source, &mut |_| {
                panic!("an empty merge should report no assignments")
            });
        }

        let updates = json!([
            {"flowchart": {"nodeSpacing": 70}},
            [{"enabled": true}, {"flowchart": {"nodeSpacing": 72, "rankSpacing": 61}}],
        ]);
        let mut flat_paths = Vec::new();
        flat.merge_with_mutation_observer(&updates, &mut |path| flat_paths.push(path.join(".")));
        let mut layered_paths = Vec::new();
        layered.merge_with_mutation_observer(&updates, &mut |path| {
            layered_paths.push(path.join("."));
        });
        assert_eq!(layered_paths, flat_paths);
        assert_eq!(layered.to_managed_json(), flat.to_managed_json());
        let ConfigStorage::Sections(sections) = &layered.value.storage else {
            panic!("expected a section fork");
        };
        assert!(sections.flattened.get().is_none());
    }

    #[test]
    fn unique_section_owner_reuses_its_cached_json_for_raw_mutation() {
        let base = MermaidConfig::from_value(json!({"large": "x".repeat(32 * 1_024), "tiny": 0}));
        let mut config = base.clone().with_shared_sections();
        config.set_value("tiny", json!(1));
        let cached_buffer = config.as_value()["large"].as_str().unwrap().as_ptr();
        let (_, copies) = test_support::measure_cow_copies(|| {
            config.as_value_mut()["tiny"] = json!(2);
        });

        assert_eq!(copies, 0);
        assert_eq!(
            config.as_value()["large"].as_str().unwrap().as_ptr(),
            cached_buffer
        );
        assert_eq!(config.get("tiny"), Some(&json!(2)));
        assert_eq!(base.get("tiny"), Some(&json!(0)));
    }

    #[test]
    fn deep_merge_empty_object_is_a_noop_for_a_null_root() {
        let mut config = MermaidConfig::from_value(Value::Null);

        config.deep_merge(&json!({}));

        assert_eq!(config.as_value(), &Value::Null);
    }

    #[test]
    fn merge_preserves_shared_json_and_reports_same_value_assignments() {
        let original = MermaidConfig::from_value(json!({
            "pie": {"textPosition": 0.75},
            "list": [1, null],
            "scalar": true
        }));
        let mut config = original.clone();
        let mut observed = Vec::new();
        let (_, copies) = test_support::measure_cow_copies(|| {
            config.merge_with_mutation_observer(&json!({"pie": {}}), &mut |_| {
                panic!("an existing empty section emits no assignments")
            });
            config.merge_with_mutation_observer(
                &json!({
                    "pie": {"textPosition": 0.75},
                    "list": [1.0, null],
                    "scalar": {"ignored": true}
                }),
                &mut |path| observed.push(path.join(".")),
            );
        });

        assert_eq!(observed, ["pie.textPosition", "list"]);
        assert_eq!(copies, 0);
        assert!(Arc::ptr_eq(&original.value, &config.value));
    }

    #[test]
    fn merge_copies_on_insertion_and_preserves_number_representation() {
        let original = MermaidConfig::from_value(json!({"same": true, "zero": -0.0, "one": 1}));
        let mut config = original.clone();
        let mut observed = Vec::new();
        let (_, copies) = test_support::measure_cow_copies(|| {
            config.merge_with_mutation_observer(
                &json!({"same": true, "added": {}, "zero": 0.0, "one": 1.0}),
                &mut |path| observed.push(path.join(".")),
            );
        });

        assert_eq!(copies, 1);
        assert_eq!(observed, ["same", "zero", "one"]);
        assert_eq!(
            original.as_value().to_string(),
            r#"{"same":true,"zero":-0.0,"one":1}"#
        );
        assert_eq!(
            config.as_value().to_string(),
            r#"{"same":true,"zero":0.0,"one":1.0,"added":{}}"#
        );
    }

    #[test]
    fn deep_merge_preserves_null_values_inside_arrays() {
        let mut config = MermaidConfig::from_value(json!({
            "values": ["old"]
        }));

        config.deep_merge(&json!({
            "values": [1, null, 2]
        }));

        assert_eq!(config.as_value()["values"], json!(["old", 1, null, 2]));
    }

    #[test]
    fn deep_merge_preserves_null_values_at_the_depth_boundary() {
        let mut config = MermaidConfig::default();

        config.deep_merge(&json!({
            "flowchart": {
                "curve": "basis",
                "nested": {
                    "ignored": null,
                    "kept": true
                },
                "values": [1, null, { "insideArray": null }]
            }
        }));

        assert_eq!(
            config.as_value()["flowchart"],
            json!({
                "curve": "basis",
                "nested": {
                    "ignored": null,
                    "kept": true
                },
                "values": [1, null, { "insideArray": null }]
            })
        );
    }

    #[test]
    fn deep_merge_deduplicates_primitives_in_new_arrays() {
        let mut config = MermaidConfig::default();

        config.deep_merge(&json!({
            "values": [1, 1.0, null, null, { "value": 1 }, { "value": 1 }]
        }));

        assert_eq!(
            config.as_value()["values"],
            json!([1, null, { "value": 1 }, { "value": 1 }])
        );
    }

    #[test]
    fn deep_merge_does_not_clobber_dissimilar_types_but_replaces_null_destinations() {
        let mut config = MermaidConfig::from_value(json!({
            "object": { "kept": true },
            "scalar": "kept",
            "nullObject": null,
            "nullScalar": null
        }));

        config.deep_merge(&json!({
            "object": "ignored",
            "scalar": { "ignored": true },
            "nullObject": { "accepted": true },
            "nullScalar": "accepted"
        }));

        assert_eq!(
            config.as_value(),
            &json!({
                "object": { "kept": true },
                "scalar": "kept",
                "nullObject": { "accepted": true },
                "nullScalar": null
            })
        );

        let mut root_null = MermaidConfig::from_value(Value::Null);
        root_null.deep_merge(&json!({ "accepted": true }));
        assert_eq!(root_null.as_value(), &json!({ "accepted": true }));
    }

    #[test]
    fn deep_merge_uses_object_assign_at_the_default_depth_boundary() {
        let mut config = MermaidConfig::from_value(json!({
            "bar": {
                "bar": {
                    "foo": {
                        "message": "old",
                        "willBe": "clobbered"
                    },
                    "preservedSibling": true
                }
            }
        }));

        config.deep_merge(&json!({
            "bar": {
                "bar": {
                    "foo": {
                        "message": "new"
                    }
                }
            }
        }));

        assert_eq!(
            config.as_value(),
            &json!({
                "bar": {
                    "bar": {
                        "foo": {
                            "message": "new"
                        },
                        "preservedSibling": true
                    }
                }
            })
        );
    }

    #[test]
    fn deep_merge_replaces_nested_null_at_the_depth_boundary() {
        let mut config = MermaidConfig::from_value(json!({
            "outer": {
                "inner": {
                    "slot": null
                }
            }
        }));

        config.deep_merge(&json!({
            "outer": {
                "inner": {
                    "slot": { "accepted": true }
                }
            }
        }));

        assert_eq!(
            config.as_value()["outer"]["inner"]["slot"],
            json!({ "accepted": true })
        );
    }

    fn deep_config_value(depth: usize) -> Value {
        let mut value = Value::String("leaf".to_string());
        for idx in (0..depth).rev() {
            let mut map = Map::new();
            map.insert(format!("k{idx}"), value);
            value = Value::Object(map);
        }
        value
    }

    fn isolated_config_lifecycle(test_name: &str, operation: fn()) {
        const CHILD_ENV: &str = "MERMAN_CONFIG_LIFECYCLE_CHILD";
        if std::env::var(CHILD_ENV).ok().as_deref() == Some(test_name) {
            std::thread::Builder::new()
                .name(test_name.to_string())
                .stack_size(64 * 1024)
                .spawn(operation)
                .expect("spawn small-stack config lifecycle test")
                .join()
                .expect("config lifecycle should complete without stack overflow");
            println!("config-lifecycle-completed={test_name}");
            return;
        }

        let mut child = Command::new(std::env::current_exe().expect("current test executable"))
            .args(["--exact", test_name, "--nocapture"])
            .env(CHILD_ENV, test_name)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn isolated config lifecycle test");
        let started = Instant::now();
        loop {
            if child
                .try_wait()
                .expect("poll config lifecycle child")
                .is_some()
            {
                break;
            }
            if started.elapsed() >= Duration::from_secs(30) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("config lifecycle child timed out: {test_name}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = child
            .wait_with_output()
            .expect("collect config lifecycle child");
        assert!(
            output.status.success(),
            "{test_name}: {}; stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains(&format!("config-lifecycle-completed={test_name}")),
            "the isolated lifecycle test must actually execute"
        );
    }

    #[test]
    fn clone_on_write_handles_deep_config_with_small_stack() {
        isolated_config_lifecycle(
            "config::tests::clone_on_write_handles_deep_config_with_small_stack",
            || {
                let original = MermaidConfig::from_value(deep_config_value(2_048));
                let mut cloned = original.clone();
                assert!(Arc::ptr_eq(&original.value, &cloned.value));
                let (_, copies) = test_support::measure_cow_copies(|| {
                    cloned.set_value("theme", Value::String("default".to_string()));
                    cloned.set_value("look", Value::String("classic".to_string()));
                });
                assert_eq!(copies, 1);
                assert!(!Arc::ptr_eq(&original.value, &cloned.value));
                assert_eq!(original.get_str("theme"), None);
                assert_eq!(cloned.get_str("theme"), Some("default"));
            },
        );
    }

    #[test]
    fn independent_deep_configs_compare_by_value_on_a_small_stack() {
        isolated_config_lifecycle(
            "config::tests::independent_deep_configs_compare_by_value_on_a_small_stack",
            || {
                const DEPTH: usize = 2_048;
                let left = MermaidConfig::from_value(deep_config_value(DEPTH));
                let mut right = MermaidConfig::from_value(deep_config_value(DEPTH));
                assert!(!Arc::ptr_eq(&left.value, &right.value));
                assert_eq!(left, right);
                let leaf_path = (0..DEPTH)
                    .map(|index| format!("k{index}"))
                    .collect::<Vec<_>>()
                    .join(".");
                right.set_value(&leaf_path, json!("changed"));
                assert_ne!(left, right);
            },
        );
    }

    #[test]
    fn deep_config_debug_is_iterative_on_a_small_stack() {
        isolated_config_lifecycle(
            "config::tests::deep_config_debug_is_iterative_on_a_small_stack",
            || {
                const DEPTH: usize = 2_048;
                let config = MermaidConfig::from_value(deep_config_value(DEPTH));
                let mut expected = String::from("MermaidConfig(");
                for index in 0..DEPTH {
                    expected.push_str(&format!("{{\"k{index}\":"));
                }
                expected.push_str("\"leaf\"");
                expected.extend(std::iter::repeat_n('}', DEPTH));
                expected.push(')');
                assert_eq!(format!("{config:?}"), expected);
            },
        );
    }

    #[test]
    fn shared_deep_config_final_release_is_safe_in_both_thread_orders() {
        isolated_config_lifecycle(
            "config::tests::shared_deep_config_final_release_is_safe_in_both_thread_orders",
            || {
                for last_index in 0..2 {
                    let config = MermaidConfig::from_value(deep_config_value(2_048));
                    let ready = Arc::new(Barrier::new(3));
                    let (released_tx, released_rx) = mpsc::channel();
                    let mut releases = Vec::new();
                    let mut workers = Vec::new();
                    let configs = [config.clone(), config];
                    assert_eq!(Arc::strong_count(&configs[0].value), 2);
                    for (index, mut config) in configs.into_iter().enumerate() {
                        let release = Arc::new(Barrier::new(2));
                        releases.push(release.clone());
                        config.before_value_release = Some(BeforeValueRelease {
                            ready: ready.clone(),
                            release,
                        });
                        let released_tx = released_tx.clone();
                        workers.push(
                            std::thread::Builder::new()
                                .stack_size(64 * 1024)
                                .spawn(move || {
                                    drop(config);
                                    released_tx.send(index).expect("report completed release");
                                })
                                .expect("spawn config release worker"),
                        );
                    }
                    ready.wait();
                    let first_index = 1 - last_index;
                    releases[first_index].wait();
                    assert_eq!(
                        released_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
                        first_index
                    );
                    releases[last_index].wait();
                    assert_eq!(
                        released_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
                        last_index
                    );
                    for worker in workers {
                        worker.join().expect("config release should complete");
                    }
                }
            },
        );
    }

    #[test]
    fn shared_clone_preserves_cow_counts_mutation_observation_and_value_equality() {
        let original = MermaidConfig::from_value(json!({
            "flowchart": { "htmlLabels": true },
            "theme": "default",
            "values": [1]
        }));
        let (mut cloned, copies) = test_support::measure_cow_copies(|| {
            let mut cloned = original.clone();
            cloned.set_value("theme", json!("default"));
            cloned.deep_merge(&json!({}));
            cloned
        });
        assert_eq!(copies, 0);
        assert!(Arc::ptr_eq(&original.value, &cloned.value));
        let mut observed = Vec::new();
        let (_, copies) = test_support::measure_cow_copies(|| {
            cloned.merge_with_mutation_observer(
                &json!({
                    "flowchart": { "htmlLabels": false },
                    "theme": "dark",
                    "values": [1, 2]
                }),
                &mut |path| observed.push(path.join(".")),
            );
            cloned.set_value("theme", json!("forest"));
        });
        assert_eq!(copies, 1);
        assert_eq!(observed, ["flowchart.htmlLabels", "theme", "values"]);
        assert_eq!(original.get_str("theme"), Some("default"));
        assert_eq!(original.get_bool("flowchart.htmlLabels"), Some(true));
        assert_eq!(original.as_value()["values"], json!([1]));
        let independent = MermaidConfig::from_value(json!({
            "values": [1, 2],
            "flowchart": { "htmlLabels": false },
            "theme": "forest"
        }));
        assert_eq!(cloned, independent);
    }

    #[test]
    fn bounded_controlled_config_clone_preserves_values_within_budget() {
        let config = MermaidConfig::from_value(json!({
            "theme": "dark",
            "flowchart": { "htmlLabels": false },
        }));
        let control = OperationControl::new();

        let cloned = config
            .clone_value_bounded_controlled(64 * 1024, 16, &control)
            .expect("active control")
            .expect("small config fits the materialization budget");

        assert_eq!(&cloned, config.as_value());
    }

    #[test]
    fn bounded_controlled_config_clone_rejects_weight_and_depth_before_cloning() {
        let oversized = MermaidConfig::from_value(json!({ "payload": "x".repeat(4 * 1024) }));
        let deep = MermaidConfig::from_value(deep_config_value(8));
        let control = OperationControl::new();

        assert!(
            oversized
                .clone_value_bounded_controlled(1_024, 16, &control)
                .expect("active control")
                .is_none()
        );
        assert!(
            deep.clone_value_bounded_controlled(64 * 1024, 4, &control)
                .expect("active control")
                .is_none()
        );
    }

    #[test]
    fn bounded_controlled_config_clone_observes_cancellation() {
        let config = MermaidConfig::from_value(json!({ "theme": "dark" }));
        let control = OperationControl::new();
        control.cancel();

        assert!(matches!(
            config.clone_value_bounded_controlled(64 * 1024, 16, &control),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn bounded_section_clone_checks_limits_without_materializing_the_projection() {
        let base = MermaidConfig::from_value(json!({
            "before": "x".repeat(2 * 1_024 * 1_024),
            "section": {"nested": ["value"]},
            "after": false,
        }));
        let mut config = base.clone().with_shared_sections();
        config.set_value("section.new", json!(7));
        config.set_value("appended", json!(true));
        let ConfigStorage::Sections(sections) = &config.value.storage else {
            panic!("expected a section fork");
        };

        let control = OperationControl::new();
        let expected = config.to_managed_json();
        let bytes = estimated_value_owned_heap_bytes(expected.as_value());
        for (limit, depth) in [(1_024, 16), (bytes - 1, 16), (bytes, 2)] {
            assert!(
                config
                    .clone_value_bounded_controlled(limit, depth, &control)
                    .unwrap()
                    .is_none()
            );
        }
        let cloned = config
            .clone_value_bounded_controlled(bytes, 3, &control)
            .unwrap()
            .map(ManagedJson::from_value)
            .unwrap();
        assert_eq!(
            serde_json::to_vec(&cloned).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );

        control.cancel();
        assert!(
            config
                .clone_value_bounded_controlled(bytes, 3, &control)
                .is_err()
        );
        assert!(sections.flattened.get().is_none());
    }
}
