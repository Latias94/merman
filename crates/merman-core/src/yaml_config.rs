use granit_parser::{Event, Parser, ScalarStyle, Tag};
use serde_json::{Map, Number, Value};
use std::collections::HashMap;

pub(crate) fn parse_yaml_value(input: &str, max_nesting_depth: usize) -> Result<Value, String> {
    let mut builder = YamlValueBuilder::new(max_nesting_depth);

    for event in Parser::new_from_str(input) {
        let (event, _) = event.map_err(|e| e.to_string())?;
        builder.on_event(event)?;
    }

    builder.finish()
}

struct YamlValueBuilder {
    stack: Vec<Frame>,
    root: Option<Value>,
    anchors: HashMap<usize, Value>,
    max_nesting_depth: usize,
}

impl YamlValueBuilder {
    fn new(max_nesting_depth: usize) -> Self {
        Self {
            stack: Vec::new(),
            root: None,
            anchors: HashMap::new(),
            max_nesting_depth,
        }
    }

    fn on_event(&mut self, event: Event<'_>) -> Result<(), String> {
        match event {
            Event::StreamStart
            | Event::StreamEnd
            | Event::DocumentStart(_)
            | Event::DocumentEnd
            | Event::Comment(_, _)
            | Event::Nothing => Ok(()),
            Event::Alias(anchor_id) => {
                let role = self.reserve_role()?;
                let value = self
                    .anchors
                    .get(&anchor_id)
                    .map(crate::config::clone_value_nonrecursive)
                    .ok_or_else(|| "unsupported forward YAML alias".to_string())?;
                self.complete_value(value, role, 0)
            }
            Event::Scalar(raw, style, anchor_id, tag) => {
                let role = self.reserve_role()?;
                let value = scalar_to_value(raw.as_ref(), style, tag.as_deref())?;
                self.complete_value(value, role, anchor_id)
            }
            Event::SequenceStart(_, anchor_id, _) => {
                let role = self.reserve_role()?;
                self.push_frame(Frame {
                    container: Container::Sequence(Vec::new()),
                    role,
                    anchor_id,
                })
            }
            Event::MappingStart(_, anchor_id, _) => {
                let role = self.reserve_role()?;
                self.push_frame(Frame {
                    container: Container::Mapping {
                        map: Map::new(),
                        pending_key: None,
                    },
                    role,
                    anchor_id,
                })
            }
            end_event @ (Event::SequenceEnd | Event::MappingEnd) => {
                let frame = self
                    .stack
                    .pop()
                    .ok_or_else(|| "unexpected YAML collection end".to_string())?;
                let value = match (end_event, frame.container) {
                    (Event::SequenceEnd, Container::Sequence(items)) => Value::Array(items),
                    (
                        Event::MappingEnd,
                        Container::Mapping {
                            map,
                            pending_key: _,
                        },
                    ) => Value::Object(map),
                    (Event::SequenceEnd, other) | (Event::MappingEnd, other) => {
                        drop_container_nonrecursive(other);
                        return Err("mismatched YAML collection end".to_string());
                    }
                    _ => unreachable!(),
                };
                self.complete_value(value, frame.role, frame.anchor_id)
            }
        }
    }

    fn push_frame(&mut self, frame: Frame) -> Result<(), String> {
        if self.stack.len() >= self.max_nesting_depth {
            return Err(format!(
                "config nesting exceeds {} levels",
                self.max_nesting_depth
            ));
        }
        self.stack.push(frame);
        Ok(())
    }

    fn reserve_role(&mut self) -> Result<Role, String> {
        let Some(parent) = self.stack.last_mut() else {
            if self.root.is_some() {
                return Err("multiple YAML documents are not supported".to_string());
            }
            return Ok(Role::Root);
        };

        match &mut parent.container {
            Container::Sequence(_) => Ok(Role::SequenceItem),
            Container::Mapping { pending_key, .. } => match pending_key.take() {
                Some(key) => Ok(Role::MappingValue(key)),
                None => Ok(Role::MappingKey),
            },
        }
    }

    fn complete_value(&mut self, value: Value, role: Role, anchor_id: usize) -> Result<(), String> {
        if anchor_id != 0 {
            self.anchors
                .insert(anchor_id, crate::config::clone_value_nonrecursive(&value));
        }

        match role {
            Role::Root => {
                self.root = Some(value);
                Ok(())
            }
            Role::SequenceItem => {
                let Some(Frame {
                    container: Container::Sequence(items),
                    ..
                }) = self.stack.last_mut()
                else {
                    crate::config::drop_value_nonrecursive(value);
                    return Err("YAML sequence item had no parent sequence".to_string());
                };
                items.push(value);
                Ok(())
            }
            Role::MappingKey => {
                let key = value_to_mapping_key(value);
                let Some(Frame {
                    container: Container::Mapping { pending_key, .. },
                    ..
                }) = self.stack.last_mut()
                else {
                    return Err("YAML mapping key had no parent mapping".to_string());
                };
                *pending_key = Some(key);
                Ok(())
            }
            Role::MappingValue(key) => {
                let Some(Frame {
                    container: Container::Mapping { map, .. },
                    ..
                }) = self.stack.last_mut()
                else {
                    crate::config::drop_value_nonrecursive(value);
                    return Err("YAML mapping value had no parent mapping".to_string());
                };
                match key {
                    MappingKey::String(key) => {
                        if map.contains_key(&key) {
                            crate::config::drop_value_nonrecursive(value);
                            return Err("duplicated mapping key".to_string());
                        }
                        map.insert(key, value);
                    }
                    MappingKey::Ignored => crate::config::drop_value_nonrecursive(value),
                }
                Ok(())
            }
        }
    }

    fn finish(mut self) -> Result<Value, String> {
        if !self.stack.is_empty() {
            return Err("incomplete YAML document".to_string());
        }

        for (_, value) in self.anchors.drain() {
            crate::config::drop_value_nonrecursive(value);
        }

        Ok(self.root.take().unwrap_or(Value::Null))
    }
}

struct Frame {
    container: Container,
    role: Role,
    anchor_id: usize,
}

enum Container {
    Sequence(Vec<Value>),
    Mapping {
        map: Map<String, Value>,
        pending_key: Option<MappingKey>,
    },
}

enum Role {
    Root,
    SequenceItem,
    MappingKey,
    MappingValue(MappingKey),
}

enum MappingKey {
    String(String),
    Ignored,
}

fn drop_container_nonrecursive(container: Container) {
    match container {
        Container::Sequence(items) => {
            for item in items {
                crate::config::drop_value_nonrecursive(item);
            }
        }
        Container::Mapping {
            map,
            pending_key: _,
        } => {
            for (_, value) in map {
                crate::config::drop_value_nonrecursive(value);
            }
        }
    }
}

fn value_to_mapping_key(value: Value) -> MappingKey {
    match value {
        Value::String(key) => MappingKey::String(key),
        Value::Number(key) => MappingKey::String(key.to_string()),
        Value::Bool(true) => MappingKey::String("true".to_string()),
        Value::Bool(false) => MappingKey::String("false".to_string()),
        Value::Null => MappingKey::String("null".to_string()),
        Value::Array(_) | Value::Object(_) => {
            crate::config::drop_value_nonrecursive(value);
            MappingKey::Ignored
        }
    }
}

fn scalar_to_value(raw: &str, style: ScalarStyle, tag: Option<&Tag>) -> Result<Value, String> {
    if let Some(core_suffix) = tag.and_then(Tag::core_suffix) {
        return scalar_to_tagged_value(raw, core_suffix);
    }

    if style != ScalarStyle::Plain {
        return Ok(Value::String(raw.to_string()));
    }

    if is_yaml_null(raw) {
        return Ok(Value::Null);
    }
    if let Some(value) = parse_yaml_bool(raw) {
        return Ok(Value::Bool(value));
    }
    if let Some(number) = parse_yaml_int(raw) {
        return Ok(Value::Number(number));
    }
    if let Some(number) = parse_yaml_float(raw) {
        return Ok(Value::Number(number));
    }

    Ok(Value::String(raw.to_string()))
}

fn scalar_to_tagged_value(raw: &str, core_suffix: &str) -> Result<Value, String> {
    match core_suffix {
        "str" => Ok(Value::String(raw.to_string())),
        "null" if is_yaml_null(raw) || raw.is_empty() => Ok(Value::Null),
        "bool" => parse_yaml_bool(raw)
            .map(Value::Bool)
            .ok_or_else(|| format!("invalid YAML bool scalar: {raw:?}")),
        "int" => parse_yaml_int(raw)
            .map(Value::Number)
            .ok_or_else(|| format!("invalid YAML integer scalar: {raw:?}")),
        "float" => parse_yaml_float(raw)
            .map(Value::Number)
            .ok_or_else(|| format!("invalid YAML float scalar: {raw:?}")),
        _ => Ok(Value::String(raw.to_string())),
    }
}

fn is_yaml_null(raw: &str) -> bool {
    matches!(raw, "" | "~" | "null" | "Null" | "NULL")
}

fn parse_yaml_bool(raw: &str) -> Option<bool> {
    match raw {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

fn parse_yaml_int(raw: &str) -> Option<Number> {
    let cleaned = raw.replace('_', "");
    let (negative, body) = match cleaned.as_bytes().first()? {
        b'-' => (true, &cleaned[1..]),
        b'+' => (false, &cleaned[1..]),
        _ => (false, cleaned.as_str()),
    };
    if body.is_empty() {
        return None;
    }

    if body == "0" {
        return Some(Number::from(0));
    }

    if let Some(digits) = body.strip_prefix("0b").or_else(|| body.strip_prefix("0B")) {
        return parse_nondecimal_int(digits, 2, negative);
    }
    if let Some(digits) = body.strip_prefix("0o").or_else(|| body.strip_prefix("0O")) {
        return parse_nondecimal_int(digits, 8, negative);
    }
    if let Some(digits) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        return parse_nondecimal_int(digits, 16, negative);
    }

    if !body.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    parse_decimal_int(body, negative)
}

fn parse_yaml_float(raw: &str) -> Option<Number> {
    let cleaned = raw.replace('_', "");
    let (negative, body) = match cleaned.as_bytes().first()? {
        b'-' => (true, &cleaned[1..]),
        b'+' => (false, &cleaned[1..]),
        _ => (false, cleaned.as_str()),
    };
    if body.is_empty() {
        return None;
    }

    let value = match body {
        ".inf" | ".Inf" | ".INF" => Some(f64::INFINITY),
        ".nan" | ".NaN" | ".NAN" => Some(f64::NAN),
        _ => {
            if !is_yaml_float_body(body) {
                return None;
            }
            body.parse::<f64>().ok()
        }
    }?;

    let value = if negative { -value } else { value };
    Number::from_f64(value)
}

fn parse_decimal_int(body: &str, negative: bool) -> Option<Number> {
    let unsigned = body.parse::<u128>().ok()?;
    if negative {
        let signed = i128::try_from(unsigned).ok()?.checked_neg()?;
        if let Ok(value) = i64::try_from(signed) {
            Some(Number::from(value))
        } else {
            Number::from_f64(signed as f64)
        }
    } else if let Ok(value) = u64::try_from(unsigned) {
        Some(Number::from(value))
    } else {
        Number::from_f64(unsigned as f64)
    }
}

fn parse_nondecimal_int(digits: &str, radix: u32, negative: bool) -> Option<Number> {
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_digit(radix)) {
        return None;
    }
    let unsigned = u128::from_str_radix(digits, radix).ok()?;
    if negative {
        let signed = i128::try_from(unsigned).ok()?.checked_neg()?;
        if let Ok(value) = i64::try_from(signed) {
            Some(Number::from(value))
        } else {
            Number::from_f64(signed as f64)
        }
    } else if let Ok(value) = u64::try_from(unsigned) {
        Some(Number::from(value))
    } else {
        Number::from_f64(unsigned as f64)
    }
}

fn is_yaml_float_body(body: &str) -> bool {
    let mut chars = body.chars().peekable();
    let mut saw_digit = false;

    let mut int_digits = 0usize;
    while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
        chars.next();
        saw_digit = true;
        int_digits += 1;
    }

    let mut frac_digits = 0usize;
    if chars.peek() == Some(&'.') {
        chars.next();
        while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            chars.next();
            saw_digit = true;
            frac_digits += 1;
        }
        if int_digits == 0 && frac_digits == 0 {
            return false;
        }
    } else if int_digits == 0 && chars.peek() != Some(&'.') {
        return false;
    }

    if let Some(&ch) = chars.peek()
        && (ch == 'e' || ch == 'E')
    {
        chars.next();
        if matches!(chars.peek(), Some('+') | Some('-')) {
            chars.next();
        }
        let mut exp_digits = 0usize;
        while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            chars.next();
            exp_digits += 1;
        }
        if exp_digits == 0 {
            return false;
        }
        saw_digit = true;
    }

    saw_digit && chars.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_nested_yaml_without_recursion() {
        let value = parse_yaml_value(
            r#"
config:
  theme: base
  flowchart:
    htmlLabels: true
  values: [1, 0x10, false, null]
"#,
            16,
        )
        .expect("yaml parses");

        assert_eq!(
            value,
            json!({
                "config": {
                    "theme": "base",
                    "flowchart": {
                        "htmlLabels": true
                    },
                    "values": [1, 16, false, null]
                }
            })
        );
    }

    #[test]
    fn ignores_complex_mapping_keys() {
        let value = parse_yaml_value(
            r#"
? [non, string, key]
: ignored
plain: retained
"#,
            16,
        )
        .expect("yaml parses");

        assert_eq!(value, json!({ "plain": "retained" }));
    }
}
