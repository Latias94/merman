//! Shared Mermaid configuration values with iterative ownership and pinned merge behavior.

mod theme_id;
pub use theme_id::{MermaidThemeId, MermaidThemeIdParseError};

use crate::json::estimated_value_owned_heap_bytes;
use crate::json::{
    ManagedJson, clone_value_nonrecursive, clone_value_nonrecursive_controlled,
    drop_value_nonrecursive,
};
use crate::{OperationControl, OperationControlResult};
use serde_json::{Map, Value};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct MermaidConfig {
    #[cfg(test)]
    before_value_release: Option<tests::BeforeValueRelease>,
    value: Arc<ManagedJson>,
}

impl fmt::Debug for MermaidConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("MermaidConfig")
            .field(&self.value)
            .finish()
    }
}

impl PartialEq for MermaidConfig {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
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
            value: Arc::new(ManagedJson::from_value(value)),
        }
    }

    pub fn as_value(&self) -> &Value {
        self.value.as_value()
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
        clone_value_nonrecursive_controlled(
            self.as_value(),
            max_retained_bytes,
            max_nesting_depth,
            control,
        )
    }

    pub fn is_empty_object(&self) -> bool {
        matches!(self.as_value(), Value::Object(map) if map.is_empty())
    }

    /// Estimates owned JSON allocation weight for retained-evidence budgeting.
    pub fn estimated_owned_heap_bytes(&self) -> usize {
        estimated_value_owned_heap_bytes(self.as_value())
    }

    pub fn as_value_mut(&mut self) -> &mut Value {
        self.value_mut()
    }

    pub fn get_str(&self, dotted_path: &str) -> Option<&str> {
        let mut cur = self.as_value();
        for segment in dotted_path.split('.') {
            cur = cur.as_object()?.get(segment)?;
        }
        cur.as_str()
    }

    pub fn get_bool(&self, dotted_path: &str) -> Option<bool> {
        let mut cur = self.as_value();
        for segment in dotted_path.split('.') {
            cur = cur.as_object()?.get(segment)?;
        }
        cur.as_bool()
    }

    pub fn set_value(&mut self, dotted_path: &str, value: Value) {
        self.set_value_raw(dotted_path, value);
    }

    fn set_value_raw(&mut self, dotted_path: &str, value: Value) {
        let current = dotted_path
            .split('.')
            .try_fold(self.as_value(), |parent, segment| {
                parent.as_object()?.get(segment)
            });
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
        let root_value = self.value_mut();
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

    /// Merges Mermaid values while reporting each actual mutation path in encounter order.
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
        let mut path = Vec::new();
        assign_with_depth_recording(self.value_mut(), other, 2, &mut path, record);
    }

    fn value_mut(&mut self) -> &mut Value {
        if Arc::strong_count(&self.value) != 1 || Arc::weak_count(&self.value) != 0 {
            #[cfg(any(test, feature = "test-support"))]
            test_support::record_cow_copy();
            self.value = Arc::new(self.value.as_ref().clone());
        }
        Arc::make_mut(&mut self.value).as_value_mut()
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

fn assign_with_depth_recording<'a, F>(
    destination: &mut Value,
    source: &'a Value,
    depth: usize,
    path: &mut Vec<&'a str>,
    record_mutation: &mut F,
) where
    F: FnMut(&[&str]),
{
    if let Value::Array(source_items) = source {
        match destination {
            Value::Array(destination_items) => {
                if !source_items.is_empty() {
                    record_mutation(path);
                }
                merge_arrays(destination_items, source_items);
            }
            Value::Object(_) => merge_array_of_sources_recording(
                destination,
                source_items,
                depth,
                path,
                record_mutation,
            ),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
        return;
    }

    if destination.is_null() {
        // Mermaid 11.16.1 treats a null destination as an absent value and returns the source.
        // Mutating the slot in place is the JSON equivalent of the caller assigning that return
        // value back to the parent property.
        record_mutation(path);
        replace_value_nonrecursive(destination, clone_value_nonrecursive(source));
        return;
    }

    if source.is_null() {
        // `assignWithDepth` ignores null-valued object properties when the destination is present.
        return;
    }

    if depth == 0 {
        object_assign_recording(destination, source, path, record_mutation);
        return;
    }

    let (Value::Object(destination_map), Value::Object(source_map)) = (destination, source) else {
        return;
    };

    for (key, source_child) in source_map {
        if is_non_null_js_object(source_child) {
            match destination_map.get_mut(key) {
                Some(destination_child) if is_js_object(destination_child) => {
                    path.push(key);
                    assign_with_depth_recording(
                        destination_child,
                        source_child,
                        depth - 1,
                        path,
                        record_mutation,
                    );
                    path.pop();
                }
                Some(_) => {}
                None => {
                    destination_map.insert(key.clone(), empty_container_for(source_child));
                    if let Some(destination_child) = destination_map.get_mut(key) {
                        path.push(key);
                        assign_with_depth_recording(
                            destination_child,
                            source_child,
                            depth - 1,
                            path,
                            record_mutation,
                        );
                        path.pop();
                    }
                }
            }
        } else if !source_child.is_null()
            && destination_map
                .get(key)
                .is_none_or(|value| !is_js_object(value))
        {
            path.push(key);
            record_mutation(path);
            path.pop();
            insert_cloned(destination_map, key, source_child);
        }
    }
}

fn merge_array_of_sources_recording<'a, F>(
    destination: &mut Value,
    source_items: &'a [Value],
    depth: usize,
    path: &mut Vec<&'a str>,
    record_mutation: &mut F,
) where
    F: FnMut(&[&str]),
{
    let mut stack = source_items.iter().rev().collect::<Vec<_>>();
    while let Some(source) = stack.pop() {
        if let Value::Array(items) = source {
            stack.extend(items.iter().rev());
        } else {
            assign_with_depth_recording(destination, source, depth, path, record_mutation);
        }
    }
}

fn merge_arrays(destination: &mut Vec<Value>, source: &[Value]) {
    for source_item in source {
        let already_present = is_json_primitive(source_item)
            && destination
                .iter()
                .any(|destination_item| same_json_primitive(destination_item, source_item));
        if !already_present {
            destination.push(clone_value_nonrecursive(source_item));
        }
    }
}

fn object_assign_recording<'a, F>(
    destination: &mut Value,
    source: &'a Value,
    path: &mut Vec<&'a str>,
    record_mutation: &mut F,
) where
    F: FnMut(&[&str]),
{
    match (destination, source) {
        (Value::Object(destination_map), Value::Object(source_map)) => {
            for (key, source_child) in source_map {
                path.push(key);
                record_mutation(path);
                path.pop();
                insert_cloned(destination_map, key, source_child);
            }
        }
        // JavaScript arrays can own named properties, while JSON arrays cannot represent them.
        // Numeric config keys are not part of Mermaid's public config shape, so leave this
        // unrepresentable object-to-array case unchanged.
        (Value::Array(_), Value::Object(_)) => {}
        (destination, source) => {
            record_mutation(path);
            replace_value_nonrecursive(destination, clone_value_nonrecursive(source));
        }
    }
}

fn insert_cloned(destination: &mut Map<String, Value>, key: &str, source: &Value) {
    if let Some(previous) = destination.insert(key.to_string(), clone_value_nonrecursive(source)) {
        drop_value_nonrecursive(previous);
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
    fn deep_merge_empty_object_is_a_noop_for_a_null_root() {
        let mut config = MermaidConfig::from_value(Value::Null);

        config.deep_merge(&json!({}));

        assert_eq!(config.as_value(), &Value::Null);
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
}
