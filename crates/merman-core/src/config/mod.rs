use crate::{OperationControl, OperationControlResult};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::mem::size_of;
use std::sync::Arc;

pub(crate) const HARDENED_SECURE_KEYS: &[&str] = &[
    "secure",
    "securityLevel",
    "startOnLoad",
    "maxTextSize",
    "suppressErrorRendering",
    "maxEdges",
    "fontFamily",
    "altFontFamily",
    "themeCSS",
    "themeVariables",
];

pub(crate) fn apply_hardened_site_policy(config: &mut MermaidConfig) {
    config.set_value(
        "secure",
        Value::Array(
            HARDENED_SECURE_KEYS
                .iter()
                .map(|key| Value::String((*key).to_string()))
                .collect(),
        ),
    );
}

#[derive(Debug, Clone, PartialEq)]
pub struct MermaidConfig(Arc<Value>);

impl Default for MermaidConfig {
    fn default() -> Self {
        Self::empty_object()
    }
}

impl MermaidConfig {
    pub fn empty_object() -> Self {
        Self(Arc::new(Value::Object(Map::new())))
    }

    pub fn from_value(value: Value) -> Self {
        Self(Arc::new(value))
    }

    pub fn as_value(&self) -> &Value {
        self.0.as_ref()
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

    pub(crate) fn is_empty_object(&self) -> bool {
        matches!(self.as_value(), Value::Object(map) if map.is_empty())
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    #[cfg(test)]
    pub(crate) fn estimated_owned_heap_bytes(&self) -> usize {
        estimated_value_owned_heap_bytes(self.as_value())
    }

    pub fn as_value_mut(&mut self) -> &mut Value {
        self.value_mut()
    }

    pub fn get_str(&self, dotted_path: &str) -> Option<&str> {
        let mut cur: &Value = self.0.as_ref();
        for segment in dotted_path.split('.') {
            cur = cur.as_object()?.get(segment)?;
        }
        cur.as_str()
    }

    pub fn get_bool(&self, dotted_path: &str) -> Option<bool> {
        let mut cur: &Value = self.0.as_ref();
        for segment in dotted_path.split('.') {
            cur = cur.as_object()?.get(segment)?;
        }
        cur.as_bool()
    }

    pub fn set_value(&mut self, dotted_path: &str, value: Value) {
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
        let Value::Object(m) = other else {
            let base = self.value_mut();
            deep_merge_value(base, other);
            return;
        };
        if m.is_empty() {
            return;
        }
        let base = self.value_mut();
        deep_merge_value(base, other);
    }

    pub(crate) fn secure_filtered_overrides(&self, overrides: &MermaidConfig) -> MermaidConfig {
        let mut filtered = clone_value_nonrecursive(overrides.as_value());
        remove_secure_keys_recursive(self.as_value(), &mut filtered);
        MermaidConfig::from_value(filtered)
    }

    fn value_mut(&mut self) -> &mut Value {
        if Arc::strong_count(&self.0) != 1 || Arc::weak_count(&self.0) != 0 {
            self.0 = Arc::new(clone_value_nonrecursive(self.0.as_ref()));
        }
        Arc::make_mut(&mut self.0)
    }
}

impl Drop for MermaidConfig {
    fn drop(&mut self) {
        if let Some(value) = Arc::get_mut(&mut self.0) {
            let old = std::mem::replace(value, Value::Null);
            drop_value_nonrecursive(old);
        }
    }
}

fn remove_secure_keys_recursive(site_config: &Value, overrides: &mut Value) {
    let secure_keys = site_config
        .get("secure")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut stack = vec![overrides];
    while let Some(current) = stack.pop() {
        match current {
            Value::Object(map) => {
                if let Some(old) = map.remove("secure") {
                    drop_value_nonrecursive(old);
                }
                for key in &secure_keys {
                    if let Some(old) = map.remove(*key) {
                        drop_value_nonrecursive(old);
                    }
                }
                for child in map.values_mut().rev() {
                    stack.push(child);
                }
            }
            Value::Array(items) => {
                for child in items.iter_mut().rev() {
                    stack.push(child);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
}

pub(crate) fn mirror_legacy_font_family_into_theme_variables(config: &mut MermaidConfig) {
    let value = config.value_mut();
    mirror_legacy_font_family_into_theme_variables_value(value);
}

pub(crate) fn mirror_legacy_font_family_into_theme_variables_value(value: &mut Value) {
    let Some(root) = value.as_object_mut() else {
        return;
    };
    let Some(font_family) = root
        .get("fontFamily")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
    else {
        return;
    };

    let has_theme_font_family = root
        .get("themeVariables")
        .and_then(Value::as_object)
        .and_then(|theme_variables| theme_variables.get("fontFamily"))
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    if has_theme_font_family {
        return;
    }

    let theme_variables = root
        .entry("themeVariables")
        .or_insert_with(|| Value::Object(Map::new()));
    if !theme_variables.is_object() {
        replace_value_nonrecursive(theme_variables, Value::Object(Map::new()));
    }
    if let Some(theme_variables) = theme_variables.as_object_mut()
        && let Some(old) =
            theme_variables.insert("fontFamily".to_string(), Value::String(font_family))
    {
        drop_value_nonrecursive(old);
    }
}

fn deep_merge_value(base: &mut Value, incoming: &Value) {
    // Mermaid 11.16.1 uses `assignWithDepth(dst, src)` with its default depth of two for site,
    // frontmatter, and directive configuration. Keep that bounded traversal instead of turning
    // configuration merging into an unbounded recursive deep merge.
    assign_with_depth(base, incoming, 2);
}

fn assign_with_depth(destination: &mut Value, source: &Value, depth: usize) {
    if let Value::Array(source_items) = source {
        match destination {
            Value::Array(destination_items) => merge_arrays(destination_items, source_items),
            Value::Object(_) => merge_array_of_sources(destination, source_items, depth),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
        return;
    }

    if destination.is_null() {
        // Mermaid 11.16.1 treats a null destination as an absent value and returns the source.
        // Mutating the slot in place is the JSON equivalent of the caller assigning that return
        // value back to the parent property.
        replace_value_nonrecursive(destination, clone_value_nonrecursive(source));
        return;
    }

    if source.is_null() {
        // `assignWithDepth` ignores null-valued object properties when the destination is present.
        return;
    }

    if depth == 0 {
        object_assign(destination, source);
        return;
    }

    let (Value::Object(destination_map), Value::Object(source_map)) = (destination, source) else {
        return;
    };

    for (key, source_child) in source_map {
        if is_non_null_js_object(source_child) {
            match destination_map.get_mut(key) {
                Some(destination_child) if is_js_object(destination_child) => {
                    assign_with_depth(destination_child, source_child, depth - 1);
                }
                Some(_) => {}
                None => {
                    destination_map.insert(key.clone(), empty_container_for(source_child));
                    if let Some(destination_child) = destination_map.get_mut(key) {
                        assign_with_depth(destination_child, source_child, depth - 1);
                    }
                }
            }
        } else if !source_child.is_null()
            && destination_map
                .get(key)
                .is_none_or(|value| !is_js_object(value))
        {
            insert_cloned(destination_map, key, source_child);
        }
    }
}

fn merge_array_of_sources(destination: &mut Value, source_items: &[Value], depth: usize) {
    let mut stack = source_items.iter().rev().collect::<Vec<_>>();
    while let Some(source) = stack.pop() {
        if let Value::Array(items) = source {
            stack.extend(items.iter().rev());
        } else {
            assign_with_depth(destination, source, depth);
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

fn object_assign(destination: &mut Value, source: &Value) {
    match (destination, source) {
        (Value::Object(destination_map), Value::Object(source_map)) => {
            for (key, source_child) in source_map {
                insert_cloned(destination_map, key, source_child);
            }
        }
        // JavaScript arrays can own named properties, while JSON arrays cannot represent them.
        // Numeric config keys are not part of Mermaid's public config shape, so leave this
        // unrepresentable object-to-array case unchanged.
        (Value::Array(_), Value::Object(_)) => {}
        (destination, source) => {
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

pub(crate) fn replace_value_nonrecursive(slot: &mut Value, value: Value) {
    let old = std::mem::replace(slot, value);
    drop_value_nonrecursive(old);
}

pub(crate) fn clone_value_nonrecursive(value: &Value) -> Value {
    let control = OperationControl::new();
    clone_value_nonrecursive_with_control(value, &control)
        .expect("a private operation control cannot be cancelled")
}

pub(crate) fn clone_value_nonrecursive_with_control(
    value: &Value,
    control: &OperationControl,
) -> OperationControlResult<Value> {
    clone_value_nonrecursive_controlled(value, usize::MAX, usize::MAX, control)
        .map(|value| value.expect("unbounded config cloning cannot exceed its budget"))
}

fn clone_value_nonrecursive_controlled(
    value: &Value,
    max_retained_bytes: usize,
    max_nesting_depth: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<Value>> {
    let mut cloned: HashMap<*const Value, Value> = HashMap::new();
    let mut stack = vec![(value, false, 0usize)];
    let mut retained_bytes = 0usize;
    let mut visited_nodes = 0usize;

    while let Some((current, visited, depth)) = stack.pop() {
        if visited_nodes.is_multiple_of(64)
            && let Err(cancelled) = control.checkpoint()
        {
            drop_cloned_values(cloned);
            return Err(cancelled);
        }
        visited_nodes = visited_nodes.saturating_add(1);
        let current_ptr = std::ptr::from_ref(current);
        if visited {
            retained_bytes = retained_bytes.saturating_add(value_clone_weight(current));
            if retained_bytes > max_retained_bytes {
                drop_cloned_values(cloned);
                return Ok(None);
            }
            let value = match current {
                Value::Null => Value::Null,
                Value::Bool(v) => Value::Bool(*v),
                Value::Number(v) => Value::Number(v.clone()),
                Value::String(v) => Value::String(v.clone()),
                Value::Array(items) => {
                    let mut out = Vec::with_capacity(items.len());
                    for (index, item) in items.iter().enumerate() {
                        if index.is_multiple_of(64)
                            && let Err(cancelled) = control.checkpoint()
                        {
                            drop_value_nonrecursive(Value::Array(out));
                            drop_cloned_values(cloned);
                            return Err(cancelled);
                        }
                        if let Some(value) = cloned.remove(&std::ptr::from_ref(item)) {
                            out.push(value);
                        }
                    }
                    Value::Array(out)
                }
                Value::Object(entries) => {
                    let mut out = Map::new();
                    for (index, (key, child)) in entries.iter().enumerate() {
                        if index.is_multiple_of(64)
                            && let Err(cancelled) = control.checkpoint()
                        {
                            drop_value_nonrecursive(Value::Object(out));
                            drop_cloned_values(cloned);
                            return Err(cancelled);
                        }
                        if let Some(value) = cloned.remove(&std::ptr::from_ref(child)) {
                            out.insert(key.clone(), value);
                        }
                    }
                    Value::Object(out)
                }
            };
            cloned.insert(current_ptr, value);
        } else {
            let has_children = matches!(current, Value::Array(items) if !items.is_empty())
                || matches!(current, Value::Object(entries) if !entries.is_empty());
            if has_children && depth >= max_nesting_depth {
                drop_cloned_values(cloned);
                return Ok(None);
            }
            let structural_weight = value_structural_weight(current);
            retained_bytes = retained_bytes.saturating_add(structural_weight);
            if retained_bytes > max_retained_bytes {
                drop_cloned_values(cloned);
                return Ok(None);
            }
            stack.push((current, true, depth));
            match current {
                Value::Array(items) => {
                    for item in items.iter().rev() {
                        stack.push((item, false, depth.saturating_add(1)));
                    }
                }
                Value::Object(entries) => {
                    for child in entries.values().rev() {
                        stack.push((child, false, depth.saturating_add(1)));
                    }
                }
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
            }
        }
    }

    if let Err(cancelled) = control.checkpoint() {
        drop_cloned_values(cloned);
        return Err(cancelled);
    }
    Ok(Some(
        cloned
            .remove(&std::ptr::from_ref(value))
            .unwrap_or(Value::Null),
    ))
}

fn value_structural_weight(value: &Value) -> usize {
    match value {
        Value::Array(items) => items.len().saturating_mul(size_of::<Value>()),
        Value::Object(entries) => entries.iter().fold(
            entries.len().saturating_mul(
                size_of::<String>()
                    .saturating_add(size_of::<Value>())
                    .saturating_add(size_of::<usize>().saturating_mul(4)),
            ),
            |weight, (key, _)| weight.saturating_add(key.len()),
        ),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
    }
}

fn value_clone_weight(value: &Value) -> usize {
    size_of::<Value>().saturating_add(match value {
        Value::String(value) => value.len(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => 0,
    })
}

#[cfg(test)]
fn estimated_value_owned_heap_bytes(value: &Value) -> usize {
    let mut retained_bytes = 0usize;
    let mut stack = vec![value];
    while let Some(current) = stack.pop() {
        retained_bytes = retained_bytes
            .saturating_add(value_structural_weight(current))
            .saturating_add(value_clone_weight(current));
        match current {
            Value::Array(items) => stack.extend(items),
            Value::Object(entries) => stack.extend(entries.values()),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    retained_bytes
}

fn drop_cloned_values(cloned: HashMap<*const Value, Value>) {
    for value in cloned.into_values() {
        drop_value_nonrecursive(value);
    }
}

pub(crate) fn drop_value_nonrecursive(value: Value) {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            Value::Array(items) => {
                stack.extend(items);
            }
            Value::Object(entries) => {
                stack.extend(entries.into_values());
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mirror_legacy_font_family_populates_missing_theme_variable() {
        let mut cfg = MermaidConfig::from_value(json!({
            "fontFamily": "Courier"
        }));

        mirror_legacy_font_family_into_theme_variables(&mut cfg);

        assert_eq!(cfg.get_str("themeVariables.fontFamily"), Some("Courier"));
    }

    #[test]
    fn mirror_legacy_font_family_preserves_explicit_theme_variable() {
        let mut cfg = MermaidConfig::from_value(json!({
            "fontFamily": "Courier",
            "themeVariables": {
                "fontFamily": "Inter"
            }
        }));

        mirror_legacy_font_family_into_theme_variables(&mut cfg);

        assert_eq!(cfg.get_str("themeVariables.fontFamily"), Some("Inter"));
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

    #[test]
    fn clone_on_write_handles_deep_config_with_small_stack() {
        const DEPTH: usize = 2_048;
        let value = deep_config_value(DEPTH);
        let handle = std::thread::Builder::new()
            .name("mermaid-config-deep-clone-on-write".to_string())
            .stack_size(64 * 1024)
            .spawn(move || {
                let original = MermaidConfig::from_value(value);
                let mut cloned = original.clone();
                cloned.set_value("theme", Value::String("default".to_string()));
                assert_eq!(cloned.get_str("theme"), Some("default"));
            })
            .expect("spawn deep config clone-on-write test");
        handle
            .join()
            .expect("deep config clone-on-write should finish without stack overflow");
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
    fn upstream_secure_key_list_matches_mermaid_runtime() {
        let upstream = crate::generated::upstream_default_config();
        let secure = upstream
            .as_value()
            .get("secure")
            .and_then(Value::as_array)
            .expect("upstream secure array")
            .iter()
            .map(|value| value.as_str().expect("secure key string"))
            .collect::<Vec<_>>();

        assert_eq!(
            secure,
            [
                "secure",
                "securityLevel",
                "startOnLoad",
                "maxTextSize",
                "suppressErrorRendering",
                "maxEdges"
            ]
        );
    }

    #[test]
    fn default_site_config_applies_hardened_secure_policy() {
        let default = crate::generated::default_site_config();
        let secure = default
            .as_value()
            .get("secure")
            .and_then(Value::as_array)
            .expect("hardened secure array")
            .iter()
            .map(|value| value.as_str().expect("secure key string"))
            .collect::<Vec<_>>();

        assert_eq!(secure, HARDENED_SECURE_KEYS);
    }

    #[test]
    fn secure_filtered_overrides_removes_default_secure_keys_recursively() {
        let site_config = crate::generated::default_site_config();
        let overrides = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "fontFamily": "diagram-font",
            "flowchart": {
                "securityLevel": "sandbox",
                "htmlLabels": false,
                "nested": [
                    {
                        "securityLevel": "loose",
                        "shape": "rect"
                    }
                ]
            }
        }));

        let filtered = site_config.secure_filtered_overrides(&overrides);

        assert!(filtered.get_str("fontFamily").is_none());
        assert_eq!(filtered.get_bool("flowchart.htmlLabels"), Some(false));
        assert_eq!(
            filtered.as_value()["flowchart"]["nested"][0]["shape"],
            json!("rect")
        );
        assert!(filtered.get_str("securityLevel").is_none());
        assert!(filtered.get_str("flowchart.securityLevel").is_none());
        assert!(
            filtered.as_value()["flowchart"]["nested"][0]
                .get("securityLevel")
                .is_none()
        );
    }

    #[test]
    fn secure_filtered_overrides_keeps_the_merged_secure_policy() {
        let mut site_config = crate::generated::default_site_config();
        site_config.deep_merge(&json!({
            "secure": ["fontSize"]
        }));
        let overrides = MermaidConfig::from_value(json!({
            "secure": ["theme"],
            "fontSize": 99,
            "securityLevel": "loose",
            "theme": "dark"
        }));

        let filtered = site_config.secure_filtered_overrides(&overrides);

        assert!(filtered.as_value().get("secure").is_none());
        assert!(filtered.as_value().get("fontSize").is_none());
        assert!(filtered.get_str("securityLevel").is_none());
        assert_eq!(filtered.get_str("theme"), Some("dark"));
    }
}
