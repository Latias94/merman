use serde_json::{Map, Value};
use std::sync::Arc;

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

    pub fn as_value_mut(&mut self) -> &mut Value {
        Arc::make_mut(&mut self.0)
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
        let root_value = Arc::make_mut(&mut self.0);
        // Be defensive: callers can construct `MermaidConfig` from any JSON value via
        // `from_value`. Mermaid configs are objects; if we see a non-object here, coerce it
        // to an object so this API never panics on user input.
        if !root_value.is_object() {
            *root_value = Value::Object(Map::new());
        }

        let Value::Object(root) = root_value else {
            return;
        };
        let mut cur: &mut Map<String, Value> = root;
        let mut segments = dotted_path.split('.').peekable();
        while let Some(seg) = segments.next() {
            if segments.peek().is_none() {
                cur.insert(seg.to_string(), value);
                return;
            }
            let slot = cur.entry(seg).or_insert_with(|| Value::Object(Map::new()));
            if !slot.is_object() {
                *slot = Value::Object(Map::new());
            }
            let Some(next) = slot.as_object_mut() else {
                return;
            };
            cur = next;
        }
    }

    pub fn deep_merge(&mut self, other: &Value) {
        let Value::Object(m) = other else {
            let base = Arc::make_mut(&mut self.0);
            deep_merge_value(base, other);
            return;
        };
        if m.is_empty() {
            return;
        }
        let base = Arc::make_mut(&mut self.0);
        deep_merge_value(base, other);
    }
}

fn deep_merge_value(base: &mut Value, incoming: &Value) {
    match (base, incoming) {
        (Value::Object(base_map), Value::Object(in_map)) => {
            for (key, in_value) in in_map {
                match base_map.get_mut(key) {
                    Some(base_value) => deep_merge_value(base_value, in_value),
                    None => {
                        base_map.insert(key.clone(), in_value.clone());
                    }
                }
            }
        }
        (base_slot, in_value) => {
            *base_slot = in_value.clone();
        }
    }
}
