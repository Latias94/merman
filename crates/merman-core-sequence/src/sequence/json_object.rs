use indexmap::IndexMap;
use serde::de::Deserialize;
use serde_json::{Map, Value};

pub(super) fn parse_json_object(input: &str) -> Option<IndexMap<String, Value>> {
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

    fn items(&mut self, close: char, mut item: impl FnMut(&mut Self) -> Option<()>) -> Option<()> {
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
