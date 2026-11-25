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

#[cfg(feature = "diagram-sequence")]
pub(crate) use strict::parse_json_object;

#[cfg(feature = "diagram-sequence")]
mod strict {
    use super::{Deserialize, Map, Value};
    use indexmap::IndexMap;

    pub(crate) fn parse_json_object(input: &str) -> Option<IndexMap<String, Value>> {
        let mut cursor = JsonCursor { remaining: input };
        let mut object = IndexMap::new();
        cursor.object(0, |key, value| {
            object.insert(key, value);
        })?;
        cursor.skip_whitespace();
        cursor.remaining.is_empty().then_some(object)
    }

    struct JsonCursor<'source> {
        remaining: &'source str,
    }

    impl<'source> JsonCursor<'source> {
        // Match serde_json's default limit of 127 nested containers.
        const MAX_CONTAINER_DEPTH: usize = 127;

        fn value(&mut self, depth: usize) -> Option<Value> {
            self.skip_whitespace();
            match self.remaining.as_bytes().first()? {
                b'{' | b'[' if depth == Self::MAX_CONTAINER_DEPTH => None,
                b'{' => {
                    let mut values = Map::new();
                    self.object(depth, |key, value| {
                        values.insert(key, value);
                    })?;
                    Some(Value::Object(values))
                }
                b'[' => {
                    self.remaining = &self.remaining[1..];
                    let mut values = Vec::new();
                    self.items(']', |cursor| {
                        values.push(cursor.value(depth + 1)?);
                        Some(())
                    })?;
                    Some(Value::Array(values))
                }
                _ => self.scalar(),
            }
        }

        fn object(&mut self, depth: usize, mut member: impl FnMut(String, Value)) -> Option<()> {
            if !self.take('{') {
                return None;
            }
            self.items('}', |cursor| {
                let key = cursor.scalar::<String>()?;
                if !cursor.take(':') {
                    return None;
                }
                member(key, cursor.value(depth + 1)?);
                Some(())
            })
        }

        fn scalar<T: Deserialize<'source>>(&mut self) -> Option<T> {
            // Limit Value deserialization to scalars. This also lets serde_json decode
            // numbers in its native representation when arbitrary_precision is unified.
            let mut stream = serde_json::Deserializer::from_str(self.remaining).into_iter::<T>();
            let value = stream.next()?.ok()?;
            self.remaining = &self.remaining[stream.byte_offset()..];
            Some(value)
        }

        fn items(
            &mut self,
            close: char,
            mut item: impl FnMut(&mut Self) -> Option<()>,
        ) -> Option<()> {
            if self.take(close) {
                return Some(());
            }
            loop {
                item(self)?;
                if self.take(close) {
                    return Some(());
                }
                if !self.take(',') {
                    return None;
                }
            }
        }

        fn take(&mut self, token: char) -> bool {
            self.skip_whitespace();
            let Some(remaining) = self.remaining.strip_prefix(token) else {
                return false;
            };
            self.remaining = remaining;
            true
        }

        fn skip_whitespace(&mut self) {
            self.remaining = self.remaining.trim_start_matches([' ', '\t', '\r', '\n']);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn literal_objects_keep_strict_json_syntax_and_depth_limit() {
            for source in [
                r#"{key: 1}"#,
                r#"{"key": 0x10}"#,
                r#"{"key": [1,]}"#,
                r#"{"key": "\ud800"}"#,
                r#"{"key": 1,}"#,
                r#"{"key": 1} {}"#,
            ] {
                assert!(parse_json_object(source).is_none(), "{source}");
            }
            for (arrays, accepted) in [(126, true), (127, false)] {
                let source = format!(r#"{{"key":{}0{}}}"#, "[".repeat(arrays), "]".repeat(arrays));
                assert_eq!(parse_json_object(&source).is_some(), accepted, "{arrays}");
            }
        }
    }
}
