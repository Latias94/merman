//! Literal JSON5 values and source-backed key evidence.

use crate::SourceConfigPath;
use merman_model::json::drop_value_nonrecursive;
use serde::Deserialize;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

pub(super) struct Json5ConfigCapture {
    pub(super) value: Option<Value>,
    pub(super) keys: Vec<Json5ConfigKeyEvidence>,
    pub rewrite_safe: bool,
}

#[derive(Debug, Clone)]
pub struct Json5ConfigKeyEvidence {
    pub path: SourceConfigPath,
    pub span: Option<std::ops::Range<usize>>,
    pub rewrite_safe: bool,
}

pub(super) fn parse_json5_config(input: &str) -> Json5ConfigCapture {
    let mut capture = Json5CaptureState {
        input,
        keys: Vec::new(),
        rewrite_safe: true,
    };
    let mut deserializer = json5::Deserializer::from_str(input);
    let value = Json5ValueSeed {
        capture: &mut capture,
        path: SourceConfigPath::root(),
        capture_keys: true,
    }
    .deserialize(&mut deserializer)
    .ok();

    let value = match value {
        Some(value) => match IgnoredAny::deserialize(&mut deserializer) {
            Err(error) if error.code() == Some(json5::ErrorCode::EofParsingValue) => Some(value),
            Ok(_) | Err(_) => {
                drop_value_nonrecursive(value);
                capture.rewrite_safe = false;
                None
            }
        },
        None => {
            capture.rewrite_safe = false;
            None
        }
    };

    Json5ConfigCapture {
        value,
        keys: capture.keys,
        rewrite_safe: capture.rewrite_safe,
    }
}

struct Json5CaptureState<'source> {
    input: &'source str,
    keys: Vec<Json5ConfigKeyEvidence>,
    rewrite_safe: bool,
}

struct Json5ValueSeed<'capture, 'source> {
    capture: &'capture mut Json5CaptureState<'source>,
    path: SourceConfigPath,
    capture_keys: bool,
}

impl<'de> DeserializeSeed<'de> for Json5ValueSeed<'_, '_> {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(Json5ValueVisitor {
            capture: self.capture,
            path: self.path,
            capture_keys: self.capture_keys,
        })
    }
}

struct Json5ValueVisitor<'capture, 'source> {
    capture: &'capture mut Json5CaptureState<'source>,
    path: SourceConfigPath,
    capture_keys: bool,
}

impl<'de> Visitor<'de> for Json5ValueVisitor<'_, '_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON5 value")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_i128<E>(self, value: i128) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Number::from_i128(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("JSON5 integer is outside serde_json's supported range"))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Number::from_u128(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("JSON5 integer is outside serde_json's supported range"))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("JSON5 float is not representable as serde_json"))
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
        Ok(Value::String(value.to_string()))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Value::String(value.to_string()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0));
        while let Some(value) = sequence.next_element_seed(Json5ValueSeed {
            capture: self.capture,
            path: SourceConfigPath::root(),
            // Mermaid config paths do not traverse array indices. Emitting an object's keys with
            // the parent object path would flatten the array and create false diagnostics.
            capture_keys: false,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::with_capacity(object.size_hint().unwrap_or(0));
        while let Some(key) = object.next_key_seed(Json5KeySeed {
            input: self.capture.input,
        })? {
            let path = if self.capture_keys {
                self.path.child(key.name.clone())
            } else {
                SourceConfigPath::root()
            };
            if self.capture_keys {
                self.capture.keys.push(Json5ConfigKeyEvidence {
                    path: path.clone(),
                    span: key.span.clone(),
                    rewrite_safe: key.rewrite_safe,
                });
            }
            let value = object.next_value_seed(Json5ValueSeed {
                capture: self.capture,
                path,
                capture_keys: self.capture_keys,
            })?;
            if let Some(replaced) = values.insert(key.name, value) {
                drop_value_nonrecursive(replaced);
            }
        }
        Ok(Value::Object(values))
    }
}

struct Json5KeySeed<'source> {
    input: &'source str,
}

struct Json5Key {
    name: String,
    span: Option<std::ops::Range<usize>>,
    rewrite_safe: bool,
}

impl<'de> DeserializeSeed<'de> for Json5KeySeed<'_> {
    type Value = Json5Key;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_identifier(Json5KeyVisitor { input: self.input })
    }
}

struct Json5KeyVisitor<'source> {
    input: &'source str,
}

impl<'de> Visitor<'de> for Json5KeyVisitor<'_> {
    type Value = Json5Key;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON5 object key")
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
        let span = borrowed_subslice_range(self.input, value);
        Ok(Json5Key {
            name: value.to_string(),
            rewrite_safe: span.is_some(),
            span,
        })
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Json5Key {
            name: value.to_string(),
            span: None,
            rewrite_safe: false,
        })
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Json5Key {
            name: value,
            span: None,
            rewrite_safe: false,
        })
    }
}

fn borrowed_subslice_range(source: &str, subslice: &str) -> Option<std::ops::Range<usize>> {
    let source_start = source.as_ptr() as usize;
    let source_end = source_start.checked_add(source.len())?;
    let subslice_start = subslice.as_ptr() as usize;
    let subslice_end = subslice_start.checked_add(subslice.len())?;
    if subslice_start < source_start || subslice_end > source_end {
        return None;
    }
    let start = subslice_start - source_start;
    let end = start.checked_add(subslice.len())?;
    source.get(start..end).map(|_| start..end)
}
