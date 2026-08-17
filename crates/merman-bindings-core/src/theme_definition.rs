use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;

use merman::diagram_theme::ThemeMaterializer;
use merman::svg::{DiagramTheme, DiagramThemeCompiler, ThemeResourcePolicy};
use merman_theme_contract::ThemeDefinitionV1;
use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};

use crate::common::{BindingError, BindingStatus};

const MAX_JSON_DEPTH: usize = 32;
const MAX_JSON_OBJECT_MEMBERS: usize = 64;
const MAX_JSON_ARRAY_ITEMS: usize = 1_024;
const MAX_JSON_TOTAL_ENTRIES: usize = 65_536;
const MAX_JSON_STRING_BYTES: usize = 64 * 1_024;
const PREFLIGHT_SENTINEL: &str = "theme definition JSON preflight failed";

/// Compiles one versioned authoring definition with the general binding resource profile.
pub fn compile_theme_definition_json(bytes: &[u8]) -> Result<DiagramTheme, BindingError> {
    let compiler =
        DiagramThemeCompiler::new().with_resource_policy(ThemeResourcePolicy::for_profile(
            merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
        ));
    compile_theme_definition_json_with(&compiler, bytes)
}

/// Compiles one versioned authoring definition under a caller-owned compiler policy.
pub fn compile_theme_definition_json_with(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<DiagramTheme, BindingError> {
    compiler
        .check_encoded_input_bytes(bytes.len())
        .map_err(crate::theme::theme_resource_error)?;
    preflight_json(bytes)?;
    let definition = serde_json::from_slice::<ThemeDefinitionV1>(bytes).map_err(|error| {
        invalid_definition_json(format!(
            "definition does not match ThemeDefinitionV1: {error}"
        ))
    })?;
    let materialized = ThemeMaterializer::new()
        .materialize_theme(&definition)
        .map_err(|error| {
            BindingError::new(
                BindingStatus::InvalidArgument,
                format!("invalid theme definition: {error}"),
            )
        })?;
    compiler
        .compile_spec_wire(materialized.into_spec())
        .map_err(crate::theme::theme_compile_error)
}

#[derive(Debug, Clone, Copy)]
enum PreflightFailure {
    NestingDepth,
    ObjectMembers,
    ArrayItems,
    TotalEntries,
    StringBytes,
    DuplicateObjectKey,
}

impl fmt::Display for PreflightFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NestingDepth => "nesting depth limit exceeded",
            Self::ObjectMembers => "object member limit exceeded",
            Self::ArrayItems => "array item limit exceeded",
            Self::TotalEntries => "total collection entry limit exceeded",
            Self::StringBytes => "decoded string byte limit exceeded",
            Self::DuplicateObjectKey => "duplicate object key",
        })
    }
}

struct PreflightState {
    total_entries: usize,
    failure: Option<PreflightFailure>,
}

impl PreflightState {
    const fn new() -> Self {
        Self {
            total_entries: 0,
            failure: None,
        }
    }

    fn fail<E: de::Error>(&mut self, failure: PreflightFailure) -> E {
        if self.failure.is_none() {
            self.failure = Some(failure);
        }
        E::custom(PREFLIGHT_SENTINEL)
    }

    fn check_depth<E: de::Error>(&mut self, depth: usize) -> Result<(), E> {
        if depth > MAX_JSON_DEPTH {
            return Err(self.fail(PreflightFailure::NestingDepth));
        }
        Ok(())
    }

    fn check_object_members<E: de::Error>(&mut self, members: usize) -> Result<(), E> {
        if members > MAX_JSON_OBJECT_MEMBERS {
            return Err(self.fail(PreflightFailure::ObjectMembers));
        }
        Ok(())
    }

    fn check_array_items<E: de::Error>(&mut self, items: usize) -> Result<(), E> {
        if items > MAX_JSON_ARRAY_ITEMS {
            return Err(self.fail(PreflightFailure::ArrayItems));
        }
        Ok(())
    }

    fn charge_entry<E: de::Error>(&mut self) -> Result<(), E> {
        self.total_entries = self.total_entries.saturating_add(1);
        if self.total_entries > MAX_JSON_TOTAL_ENTRIES {
            return Err(self.fail(PreflightFailure::TotalEntries));
        }
        Ok(())
    }

    fn check_string<E: de::Error>(&mut self, value: &str) -> Result<(), E> {
        if value.len() > MAX_JSON_STRING_BYTES {
            return Err(self.fail(PreflightFailure::StringBytes));
        }
        Ok(())
    }
}

fn preflight_json(bytes: &[u8]) -> Result<(), BindingError> {
    let mut state = PreflightState::new();
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let result = JsonValueSeed {
        state: &mut state,
        parent_depth: 0,
    }
    .deserialize(&mut deserializer);
    if let Err(error) = result {
        return Err(match state.failure {
            Some(failure) => invalid_definition_json(failure.to_string()),
            None => invalid_definition_json(format!("invalid JSON: {error}")),
        });
    }
    deserializer
        .end()
        .map_err(|error| invalid_definition_json(format!("invalid JSON: {error}")))
}

fn invalid_definition_json(message: impl Into<String>) -> BindingError {
    BindingError::new(
        BindingStatus::OptionsJsonError,
        format!("invalid theme definition JSON: {}", message.into()),
    )
}

struct JsonValueSeed<'a> {
    state: &'a mut PreflightState,
    parent_depth: usize,
}

impl<'de> DeserializeSeed<'de> for JsonValueSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
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

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string(value)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string(value)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string(&value)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let depth = self.parent_depth.saturating_add(1);
        self.state.check_depth(depth)?;
        let mut item_count = 0usize;
        loop {
            let next_count = item_count.saturating_add(1);
            let Some(()) = sequence.next_element_seed(JsonSequenceElementSeed {
                state: &mut *self.state,
                parent_depth: depth,
                item_count: next_count,
            })?
            else {
                break;
            };
            item_count = next_count;
        }
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let depth = self.parent_depth.saturating_add(1);
        self.state.check_depth(depth)?;
        let mut member_count = 0usize;
        let mut seen = HashSet::<Cow<'de, str>>::new();
        while let Some(key) = map.next_key_seed(JsonKeySeed {
            state: &mut *self.state,
        })? {
            member_count = member_count.saturating_add(1);
            self.state.check_object_members(member_count)?;
            self.state.charge_entry()?;
            if !seen.insert(key) {
                return Err(self.state.fail(PreflightFailure::DuplicateObjectKey));
            }
            map.next_value_seed(JsonValueSeed {
                state: &mut *self.state,
                parent_depth: depth,
            })?;
        }
        Ok(())
    }
}

struct JsonSequenceElementSeed<'a> {
    state: &'a mut PreflightState,
    parent_depth: usize,
    item_count: usize,
}

impl<'de> DeserializeSeed<'de> for JsonSequenceElementSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.state.check_array_items(self.item_count)?;
        self.state.charge_entry()?;
        JsonValueSeed {
            state: self.state,
            parent_depth: self.parent_depth,
        }
        .deserialize(deserializer)
    }
}

struct JsonKeySeed<'a> {
    state: &'a mut PreflightState,
}

impl<'de> DeserializeSeed<'de> for JsonKeySeed<'_> {
    type Value = Cow<'de, str>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(JsonKeyVisitor { state: self.state })
    }
}

struct JsonKeyVisitor<'a> {
    state: &'a mut PreflightState,
}

impl<'de> Visitor<'de> for JsonKeyVisitor<'_> {
    type Value = Cow<'de, str>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded JSON object key")
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string(value)?;
        Ok(Cow::Borrowed(value))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string(value)?;
        Ok(Cow::Owned(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string(&value)?;
        Ok(Cow::Owned(value))
    }
}
