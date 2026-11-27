//! Decode source objects with literal keys while retaining each format's scalar rules.

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use std::fmt;

pub(crate) fn parse_json5_value(input: &str) -> Option<Value> {
    json5::from_str::<LiteralJson5>(input)
        .ok()
        .map(|value| value.0)
}

struct LiteralJson5(Value);

impl<'de> Deserialize<'de> for LiteralJson5 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(Json5Visitor).map(Self)
    }
}

struct Json5Visitor;

impl<'de> Visitor<'de> for Json5Visitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON5 value")
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
        // Ordinary directives have always mapped JSON5 non-finite numbers to null.
        Ok(value.into())
    }

    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_string<E>(self, value: String) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(LiteralJson5(value)) = sequence.next_element()? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        // Value's default visitor classifies the first key as a possible Serde marker.
        while let Some((key, LiteralJson5(value))) = object.next_entry()? {
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
