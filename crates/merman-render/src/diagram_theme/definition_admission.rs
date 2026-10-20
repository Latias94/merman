//! Bounded input admission shared by editable designs and concrete specifications.

use std::borrow::Cow;
use std::collections::HashSet;
use std::{fmt, io};

use merman_theme_contract::{ThemeMaterializationDiagnosticV1, ThemeMaterializationErrorV1};
use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};

use super::{ThemeResourceLimitExceeded, ThemeResourcePolicy};

const MAX_JSON_DEPTH: usize = 32;
const MAX_JSON_OBJECT_MEMBERS: usize = 1_024;
const MAX_JSON_ARRAY_ITEMS: usize = 1_024;
const MAX_TOTAL_COLLECTION_ITEMS: usize = 65_536;
const MAX_JSON_STRING_BYTES: usize = 64 * 1_024;
const PREFLIGHT_SENTINEL: &str = "theme design JSON preflight failed";

/// A bounded wire serialization failed before semantic decoding.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(super) enum ThemeInputEncodingError {
    #[error("typed theme input cannot be encoded for bounded admission")]
    InvalidEncoding,
    #[error(transparent)]
    ResourceLimit(#[from] ThemeResourceLimitExceeded),
}

pub(super) fn encoded_bytes_contract_error(
    error: ThemeResourceLimitExceeded,
) -> ThemeMaterializationErrorV1 {
    let limit = if error.limit == "theme_encoded_bytes_hard_cap" {
        "theme_encoded_bytes_hard_cap"
    } else {
        "max_theme_encoded_bytes"
    };
    ThemeMaterializationErrorV1::from_diagnostic(
        ThemeMaterializationDiagnosticV1::resource_limit_exceeded(
            "",
            limit,
            error.max.saturating_add(1),
            error.max,
            "theme input exceeds the caller-owned resource policy",
        ),
    )
}

fn invalid_json_contract_error(
    reason: &'static str,
    message: &'static str,
) -> ThemeMaterializationErrorV1 {
    ThemeMaterializationErrorV1::from_diagnostic(
        ThemeMaterializationDiagnosticV1::invalid_definition_json(reason, message),
    )
}

pub(super) fn check_typed_input_encoded_bytes(
    resources: &ThemeResourcePolicy,
    input: &impl serde::Serialize,
) -> Result<(), ThemeInputEncodingError> {
    measure_typed_input_encoded_bytes(resources, input).map(|_| ())
}

pub(super) fn measure_typed_input_encoded_bytes(
    resources: &ThemeResourcePolicy,
    input: &impl serde::Serialize,
) -> Result<usize, ThemeInputEncodingError> {
    let mut writer = BoundedCountingWriter::new(resources);
    let encoded = serde_json::to_writer(&mut writer, input);
    if let Some(error) = writer.take_resource_error() {
        return Err(ThemeInputEncodingError::ResourceLimit(error));
    }
    encoded.map_err(|_| ThemeInputEncodingError::InvalidEncoding)?;
    Ok(writer.bytes)
}

struct BoundedCountingWriter<'a> {
    resources: &'a ThemeResourcePolicy,
    bytes: usize,
    resource_error: Option<ThemeResourceLimitExceeded>,
}

impl<'a> BoundedCountingWriter<'a> {
    const fn new(resources: &'a ThemeResourcePolicy) -> Self {
        Self {
            resources,
            bytes: 0,
            resource_error: None,
        }
    }

    fn take_resource_error(&mut self) -> Option<ThemeResourceLimitExceeded> {
        self.resource_error.take()
    }
}

impl io::Write for BoundedCountingWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.resource_error.is_some() {
            return Err(io::Error::other("theme input encoded-byte limit exceeded"));
        }
        let projected = self.bytes.saturating_add(buffer.len());
        if let Err(error) = self.resources.check_theme_encoded_bytes(projected) {
            self.resource_error = Some(error);
            return Err(io::Error::other("theme input encoded-byte limit exceeded"));
        }
        self.bytes = projected;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct PreflightState {
    path: String,
    path_frozen: bool,
    collection_items: usize,
    failure: Option<ThemeMaterializationErrorV1>,
}

impl PreflightState {
    fn fail<E: de::Error>(&mut self, failure: ThemeMaterializationErrorV1) -> E {
        self.failure.get_or_insert(failure);
        E::custom(PREFLIGHT_SENTINEL)
    }

    fn bound<E: de::Error>(
        &mut self,
        limit: &'static str,
        actual: usize,
        max: usize,
    ) -> Result<(), E> {
        if actual <= max {
            return Ok(());
        }
        let path = if limit == "max_theme_definition_collection_items" {
            ""
        } else {
            &self.path
        };
        let error = ThemeMaterializationErrorV1::from_diagnostic(
            ThemeMaterializationDiagnosticV1::resource_limit_exceeded(
                path,
                limit,
                max.saturating_add(1),
                max,
                "theme design JSON exceeds a bounded structural limit",
            ),
        );
        Err(self.fail(error))
    }

    fn string<E: de::Error>(&mut self, value: &str) -> Result<(), E> {
        self.bound(
            "max_theme_definition_string_bytes",
            value.len(),
            MAX_JSON_STRING_BYTES,
        )
    }

    fn item<E: de::Error>(&mut self) -> Result<(), E> {
        self.collection_items = self.collection_items.saturating_add(1);
        self.bound(
            "max_theme_definition_collection_items",
            self.collection_items,
            MAX_TOTAL_COLLECTION_ITEMS,
        )
    }

    fn push_key(&mut self, key: &str) -> (usize, bool) {
        let previous = (self.path.len(), self.path_frozen);
        if self.path_frozen {
            return previous;
        }
        let escaped_len = key.len().saturating_add(
            key.bytes()
                .filter(|byte| matches!(*byte, b'~' | b'/'))
                .count(),
        );
        if self
            .path
            .len()
            .saturating_add(1)
            .saturating_add(escaped_len)
            > ThemeMaterializationDiagnosticV1::MAX_PATH_BYTES
        {
            self.path_frozen = true;
            return previous;
        }
        self.path.push('/');
        for character in key.chars() {
            match character {
                '~' => self.path.push_str("~0"),
                '/' => self.path.push_str("~1"),
                _ => self.path.push(character),
            }
        }
        previous
    }

    fn push_index(&mut self, index: usize) -> (usize, bool) {
        let previous = (self.path.len(), self.path_frozen);
        if !self.path_frozen {
            let segment = format!("/{index}");
            if self.path.len().saturating_add(segment.len())
                > ThemeMaterializationDiagnosticV1::MAX_PATH_BYTES
            {
                self.path_frozen = true;
            } else {
                self.path.push_str(&segment);
            }
        }
        previous
    }

    fn restore_path(&mut self, previous: (usize, bool)) {
        self.path.truncate(previous.0);
        self.path_frozen = previous.1;
    }
}

/// Checks structure without interpreting token types or materialization semantics.
/// Duplicate keys are rejected before map normalization can erase their provenance.
pub(super) fn preflight_json(bytes: &[u8]) -> Result<(), ThemeMaterializationErrorV1> {
    let mut state = PreflightState::default();
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let parsed = JsonValueSeed {
        state: &mut state,
        parent_depth: 0,
    }
    .deserialize(&mut decoder);
    if parsed.is_err() {
        return Err(state.failure.unwrap_or_else(|| {
            invalid_json_contract_error("malformed-json", "theme design JSON is malformed")
        }));
    }
    decoder.end().map_err(|_| {
        invalid_json_contract_error("malformed-json", "theme design JSON is malformed")
    })
}

struct JsonValueSeed<'a> {
    state: &'a mut PreflightState,
    parent_depth: usize,
}

impl<'de> DeserializeSeed<'de> for JsonValueSeed<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(JsonValueVisitor {
            state: self.state,
            parent_depth: self.parent_depth,
        })
    }
}

struct JsonValueVisitor<'a> {
    state: &'a mut PreflightState,
    parent_depth: usize,
}

impl<'de> Visitor<'de> for JsonValueVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one bounded JSON value")
    }

    fn visit_bool<E>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<(), E> {
        self.state.string(value)
    }
    fn visit_borrowed_str<E: de::Error>(self, value: &'de str) -> Result<(), E> {
        self.state.string(value)
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<(), E> {
        self.state.string(&value)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        let depth = self.parent_depth.saturating_add(1);
        self.state
            .bound("max_theme_definition_json_depth", depth, MAX_JSON_DEPTH)?;
        let mut count = 0usize;
        while sequence
            .next_element_seed(JsonSequenceItemSeed {
                state: &mut *self.state,
                parent_depth: depth,
                index: count,
            })?
            .is_some()
        {
            count += 1;
        }
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let depth = self.parent_depth.saturating_add(1);
        self.state
            .bound("max_theme_definition_json_depth", depth, MAX_JSON_DEPTH)?;
        let mut seen = HashSet::<Cow<'de, str>>::new();
        while let Some(key) = map.next_key_seed(JsonKeySeed {
            state: &mut *self.state,
        })? {
            self.state.bound(
                "max_theme_definition_object_members",
                seen.len().saturating_add(1),
                MAX_JSON_OBJECT_MEMBERS,
            )?;
            if !seen.insert(key.clone()) {
                return Err(self.state.fail(invalid_json_contract_error(
                    "duplicate-object-key",
                    "theme design JSON contains a duplicate object key",
                )));
            }
            self.state.item()?;
            let previous = self.state.push_key(&key);
            let result = map.next_value_seed(JsonValueSeed {
                state: &mut *self.state,
                parent_depth: depth,
            });
            self.state.restore_path(previous);
            result?;
        }
        Ok(())
    }
}

struct JsonSequenceItemSeed<'a> {
    state: &'a mut PreflightState,
    parent_depth: usize,
    index: usize,
}

impl<'de> DeserializeSeed<'de> for JsonSequenceItemSeed<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        self.state.bound(
            "max_theme_json_array_items",
            self.index.saturating_add(1),
            MAX_JSON_ARRAY_ITEMS,
        )?;
        self.state.item()?;
        let previous = self.state.push_index(self.index);
        let result = JsonValueSeed {
            state: &mut *self.state,
            parent_depth: self.parent_depth,
        }
        .deserialize(deserializer);
        self.state.restore_path(previous);
        result
    }
}

struct JsonKeySeed<'a> {
    state: &'a mut PreflightState,
}

impl<'de> DeserializeSeed<'de> for JsonKeySeed<'_> {
    type Value = Cow<'de, str>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_str(JsonKeyVisitor { state: self.state })
    }
}

struct JsonKeyVisitor<'a> {
    state: &'a mut PreflightState,
}

impl<'de> Visitor<'de> for JsonKeyVisitor<'_> {
    type Value = Cow<'de, str>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one bounded JSON object key")
    }

    fn visit_borrowed_str<E: de::Error>(self, value: &'de str) -> Result<Self::Value, E> {
        self.state.string(value)?;
        Ok(Cow::Borrowed(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        self.state.string(value)?;
        Ok(Cow::Owned(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        self.state.string(&value)?;
        Ok(Cow::Owned(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn preflight_accepts_a_full_design_token_library_without_legacy_field_interpretation() {
        let tokens = (0..256)
            .map(|index| {
                (
                    format!("color.{index}"),
                    json!({"type":"color","value":"#123456"}),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        preflight_json(&serde_json::to_vec(&json!({"tokens":tokens})).unwrap()).unwrap();
    }

    #[test]
    fn preflight_rejects_duplicate_keys_before_normalization() {
        let error = preflight_json(
            br##"{"tokens":{"ink":{"type":"color","value":"#000","value":"#fff"}}}"##,
        )
        .unwrap_err();
        assert_eq!(error.diagnostic().reason_id(), Some("duplicate-object-key"));
    }

    #[test]
    fn structural_limits_accept_the_boundary_and_reject_the_next_item() {
        let members = (0..MAX_JSON_OBJECT_MEMBERS)
            .map(|index| (index.to_string(), serde_json::Value::Null))
            .collect::<serde_json::Map<_, _>>();
        let mut object = serde_json::Value::Object(members);
        preflight_json(&serde_json::to_vec(&object).unwrap()).unwrap();
        object["excess"] = serde_json::Value::Null;
        let error = preflight_json(&serde_json::to_vec(&object).unwrap()).unwrap_err();
        assert_eq!(
            error.diagnostic().limit_id(),
            Some("max_theme_definition_object_members")
        );
        assert_eq!(
            error.diagnostic().actual(),
            Some((MAX_JSON_OBJECT_MEMBERS + 1) as u64)
        );

        let mut aggregate = vec![vec![0; 1024]; 63];
        aggregate.push(vec![0; 960]);
        preflight_json(&serde_json::to_vec(&aggregate).unwrap()).unwrap();
        aggregate[63].push(0);
        let error = preflight_json(&serde_json::to_vec(&aggregate).unwrap()).unwrap_err();
        assert_eq!(error.diagnostic().path(), "");
        assert_eq!(
            error.diagnostic().limit_id(),
            Some("max_theme_definition_collection_items")
        );
        assert_eq!(
            error.diagnostic().actual(),
            Some((MAX_TOTAL_COLLECTION_ITEMS + 1) as u64)
        );

        let exact = format!(
            "{}0{}",
            "[".repeat(MAX_JSON_DEPTH),
            "]".repeat(MAX_JSON_DEPTH)
        );
        preflight_json(exact.as_bytes()).unwrap();
        let error = preflight_json(format!("[{exact}]").as_bytes()).unwrap_err();
        assert_eq!(
            error.diagnostic().limit_id(),
            Some("max_theme_definition_json_depth")
        );
        assert_eq!(
            error.diagnostic().actual(),
            Some((MAX_JSON_DEPTH + 1) as u64)
        );
    }

    #[test]
    fn preflight_preserves_bounded_escaped_paths_and_restores_siblings() {
        let input = json!({"future":{"x".repeat(505):["short"]},"z/z~":["x".repeat(MAX_JSON_STRING_BYTES + 1)]});
        let error = preflight_json(&serde_json::to_vec(&input).unwrap()).unwrap_err();
        assert_eq!(error.diagnostic().path(), "/z~1z~0/0");
        assert_eq!(
            error.diagnostic().limit_id(),
            Some("max_theme_definition_string_bytes")
        );
        assert!(input.pointer(error.diagnostic().path()).is_some());
    }

    #[test]
    fn preflight_refuses_the_first_excess_array_item_before_decoding_it() {
        let entries = std::iter::repeat_n("0", MAX_JSON_ARRAY_ITEMS)
            .collect::<Vec<_>>()
            .join(",");
        let input = format!("[{entries},{{\"invalid\":]");
        let error = preflight_json(input.as_bytes()).unwrap_err();
        assert_eq!(
            error.diagnostic().limit_id(),
            Some("max_theme_json_array_items")
        );
        assert_eq!(
            error.diagnostic().actual(),
            Some((MAX_JSON_ARRAY_ITEMS + 1) as u64)
        );
    }

    #[test]
    fn typed_encoded_admission_retains_the_caller_resource_error() {
        let resources = ThemeResourcePolicy::default()
            .with_limit(super::super::ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
            .unwrap();
        let error = check_typed_input_encoded_bytes(&resources, &json!({})).unwrap_err();
        let ThemeInputEncodingError::ResourceLimit(error) = error else {
            panic!("resource error required");
        };
        let diagnostic = encoded_bytes_contract_error(error);
        assert_eq!(
            diagnostic.diagnostic().limit_id(),
            Some("max_theme_encoded_bytes")
        );
        assert_eq!(diagnostic.diagnostic().actual(), Some(2));
        assert_eq!(diagnostic.diagnostic().max(), Some(1));
    }
}
