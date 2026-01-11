use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct MermaidConfig(Value);

impl Default for MermaidConfig {
    fn default() -> Self {
        Self::empty_object()
    }
}

impl MermaidConfig {
    pub fn empty_object() -> Self {
        Self(Value::Object(Map::new()))
    }

    pub fn from_value(value: Value) -> Self {
        Self(value)
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }

    pub fn as_value_mut(&mut self) -> &mut Value {
        &mut self.0
    }

    pub fn get_str(&self, dotted_path: &str) -> Option<&str> {
        let mut cur = &self.0;
        for segment in dotted_path.split('.') {
            cur = cur.as_object()?.get(segment)?;
        }
        cur.as_str()
    }

    pub fn get_bool(&self, dotted_path: &str) -> Option<bool> {
        let mut cur = &self.0;
        for segment in dotted_path.split('.') {
            cur = cur.as_object()?.get(segment)?;
        }
        cur.as_bool()
    }

    pub fn set_value(&mut self, dotted_path: &str, value: Value) {
        let mut cur = self.0.as_object_mut().unwrap();
        let mut segments = dotted_path.split('.').peekable();
        while let Some(seg) = segments.next() {
            if segments.peek().is_none() {
                cur.insert(seg.to_string(), value);
                return;
            }
            cur = cur
                .entry(seg)
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .unwrap();
        }
    }

    pub fn deep_merge(&mut self, other: &Value) {
        deep_merge_value(&mut self.0, other);
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
