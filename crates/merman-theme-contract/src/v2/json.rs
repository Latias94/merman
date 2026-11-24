use std::collections::HashSet;
use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use super::{ThemeDocumentV2, ThemeError};

pub(super) fn parse(bytes: &[u8]) -> Result<ThemeDocumentV2, ThemeError> {
    // Untagged enums buffer maps during deserialization, so check keys on the raw stream.
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    serde_path_to_error::deserialize::<_, UniqueKeys>(&mut decoder)
        .map_err(|error| ThemeError::json(error.path().to_string(), error.into_inner()))?;
    decoder
        .end()
        .map_err(|error| ThemeError::json(".".into(), error))?;

    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    serde_path_to_error::deserialize(&mut decoder)
        .map_err(|error| ThemeError::json(error.path().to_string(), error.into_inner()))
}

struct UniqueKeys;

impl<'de> Deserialize<'de> for UniqueKeys {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(Self)
    }
}

impl<'de> Visitor<'de> for UniqueKeys {
    type Value = Self;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self, A::Error> {
        while sequence.next_element::<Self>()?.is_some() {}
        Ok(self)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self, A::Error> {
        let mut keys = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if keys.contains(&key) {
                return Err(de::Error::custom(format!("duplicate key `{key}`")));
            }
            keys.insert(key);
            map.next_value::<Self>()?;
        }
        Ok(self)
    }
}
