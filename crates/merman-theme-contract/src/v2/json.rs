use std::{fmt, marker::PhantomData};

use serde::de::{Deserialize, Deserializer, MapAccess, Visitor, value::MapAccessDeserializer};

use super::{ThemeDocumentV2, ThemeError};

pub(super) fn parse(bytes: &[u8]) -> Result<ThemeDocumentV2, ThemeError> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let document = serde_path_to_error::deserialize(&mut decoder).map_err(|error| {
        let path = error.path().to_string();
        ThemeError::json(path, normalize_duplicate_field(error.into_inner()))
    })?;
    decoder
        .end()
        .map_err(|error| ThemeError::json(".".into(), error))?;
    Ok(document)
}

// Serde's derived struct visitor also accepts positional arrays. Theme documents use objects.
pub(super) struct Object<T>(T);

impl<T> Object<T> {
    pub(super) fn into_inner(self) -> T {
        self.0
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
            type Value = Object<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(Object)
            }
        }

        deserializer.deserialize_map(ObjectVisitor(PhantomData))
    }
}

pub(super) fn object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    Object::deserialize(deserializer).map(Object::into_inner)
}

pub(super) fn present_object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    object(deserializer).map(Some)
}

pub(super) fn objects<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    Vec::<Object<T>>::deserialize(deserializer)
        .map(|objects| objects.into_iter().map(Object::into_inner).collect())
}

fn normalize_duplicate_field(error: serde_json::Error) -> serde_json::Error {
    if error.is_data() {
        let message = error.to_string();
        if let Some(field_and_location) = message.strip_prefix("duplicate field ") {
            // Keep the public diagnostic wording. serde_json's custom error constructor
            // recovers the line and column from its own display suffix.
            return serde::de::Error::custom(format_args!("duplicate key {field_and_location}"));
        }
    }
    error
}
