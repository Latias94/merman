use crate::{MermaidConfig, SourceSpan};
use serde::Deserialize;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;
#[cfg(test)]
use std::mem::size_of;

/// Mermaid source construct that supplied one configuration key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceConfigOrigin {
    Frontmatter,
    Directive { directive_index: usize },
}

/// Source-backed evidence for one Mermaid directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDirectiveEvidence {
    keyword: String,
    full_span: SourceSpan,
    keyword_span: SourceSpan,
    order: usize,
    complete: bool,
    rewrite_safe: bool,
}

impl SourceDirectiveEvidence {
    pub(super) fn new(
        keyword: String,
        full_span: SourceSpan,
        keyword_span: SourceSpan,
        order: usize,
        complete: bool,
        rewrite_safe: bool,
    ) -> Self {
        Self {
            keyword,
            full_span,
            keyword_span,
            order,
            complete,
            rewrite_safe,
        }
    }

    pub fn keyword(&self) -> &str {
        &self.keyword
    }

    pub const fn full_span(&self) -> SourceSpan {
        self.full_span
    }

    pub const fn keyword_span(&self) -> SourceSpan {
        self.keyword_span
    }

    pub const fn order(&self) -> usize {
        self.order
    }

    pub const fn complete(&self) -> bool {
        self.complete
    }

    pub const fn rewrite_safe(&self) -> bool {
        self.rewrite_safe
    }

    #[cfg(test)]
    fn estimated_owned_heap_bytes(&self) -> usize {
        self.keyword.capacity()
    }
}

/// Source-backed evidence for one source-addressable configuration key accepted by the owning
/// YAML or JSON5 parser.
///
/// A parser may accept a key whose decoded name cannot be mapped back to one contiguous source
/// span (for example, an escaped JSON5 key). Such keys remain in the parsed configuration value
/// but are intentionally absent from this collection rather than receiving a guessed span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceConfigKeyEvidence {
    origin: SourceConfigOrigin,
    path: Box<[String]>,
    span: SourceSpan,
    order: usize,
    rewrite_safe: bool,
}

impl SourceConfigKeyEvidence {
    pub(super) fn new(
        origin: SourceConfigOrigin,
        path: Vec<String>,
        span: SourceSpan,
        order: usize,
        rewrite_safe: bool,
    ) -> Self {
        Self {
            origin,
            path: path.into_boxed_slice(),
            span,
            order,
            rewrite_safe,
        }
    }

    pub const fn origin(&self) -> SourceConfigOrigin {
        self.origin
    }

    pub fn path(&self) -> &[String] {
        &self.path
    }

    pub fn matches_path(&self, expected: &[&str]) -> bool {
        self.path.len() == expected.len()
            && self
                .path
                .iter()
                .map(String::as_str)
                .zip(expected.iter().copied())
                .all(|(actual, expected)| actual == expected)
    }

    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    pub const fn order(&self) -> usize {
        self.order
    }

    pub const fn rewrite_safe(&self) -> bool {
        self.rewrite_safe
    }

    #[cfg(test)]
    fn estimated_owned_heap_bytes(&self) -> usize {
        self.path.iter().fold(
            self.path.len().saturating_mul(size_of::<String>()),
            |weight, segment| weight.saturating_add(segment.capacity()),
        )
    }
}

/// Source-backed frontmatter boundary and rewrite-safety evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct FrontmatterSourceEvidence {
    full_span: SourceSpan,
    body_span: SourceSpan,
    indent: String,
    has_config: bool,
    rewrite_safe: bool,
    fields: Option<MermaidConfig>,
}

impl FrontmatterSourceEvidence {
    pub(super) fn new(
        full_span: SourceSpan,
        body_span: SourceSpan,
        indent: String,
        has_config: bool,
        rewrite_safe: bool,
        fields: Option<MermaidConfig>,
    ) -> Self {
        Self {
            full_span,
            body_span,
            indent,
            has_config,
            rewrite_safe,
            fields,
        }
    }

    pub const fn full_span(&self) -> SourceSpan {
        self.full_span
    }

    pub const fn body_span(&self) -> SourceSpan {
        self.body_span
    }

    pub fn indent(&self) -> &str {
        &self.indent
    }

    pub const fn has_config(&self) -> bool {
        self.has_config
    }

    pub const fn rewrite_safe(&self) -> bool {
        self.rewrite_safe
    }

    /// Parsed top-level frontmatter fields retained from the owning preprocessing operation.
    pub fn fields(&self) -> Option<&MermaidConfig> {
        self.fields.as_ref()
    }

    #[cfg(test)]
    fn estimated_owned_heap_bytes(&self) -> usize {
        self.indent.capacity().saturating_add(
            self.fields
                .as_ref()
                .map_or(0, MermaidConfig::estimated_owned_heap_bytes),
        )
    }
}

/// Compact source-configuration evidence retained by an editor parse operation.
///
/// The evidence owns no copy of the Mermaid source and no YAML/JSON5 syntax tree. Paths and spans
/// are emitted by the same parser operation that materializes Mermaid configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceConfigEvidence {
    frontmatter: Option<FrontmatterSourceEvidence>,
    directives: Vec<SourceDirectiveEvidence>,
    keys: Vec<SourceConfigKeyEvidence>,
    config_insert_span: SourceSpan,
    rewrite_safe: bool,
}

impl Default for SourceConfigEvidence {
    fn default() -> Self {
        Self::empty()
    }
}

impl SourceConfigEvidence {
    pub(super) fn empty() -> Self {
        Self {
            frontmatter: None,
            directives: Vec::new(),
            keys: Vec::new(),
            config_insert_span: SourceSpan::new(0, 0),
            rewrite_safe: true,
        }
    }

    pub fn frontmatter(&self) -> Option<&FrontmatterSourceEvidence> {
        self.frontmatter.as_ref()
    }

    pub fn directives(&self) -> &[SourceDirectiveEvidence] {
        &self.directives
    }

    pub fn keys(&self) -> &[SourceConfigKeyEvidence] {
        &self.keys
    }

    /// Original-source insertion point for a new frontmatter `config` block.
    pub const fn config_insert_span(&self) -> SourceSpan {
        self.config_insert_span
    }

    pub const fn rewrite_safe(&self) -> bool {
        self.rewrite_safe
    }

    /// Estimated owned heap storage, excluding the source text that the spans address.
    #[cfg(test)]
    pub(crate) fn estimated_owned_heap_bytes(&self) -> usize {
        let directive_storage = self
            .directives
            .capacity()
            .saturating_mul(size_of::<SourceDirectiveEvidence>());
        let key_storage = self
            .keys
            .capacity()
            .saturating_mul(size_of::<SourceConfigKeyEvidence>());
        let payload = self
            .directives
            .iter()
            .map(SourceDirectiveEvidence::estimated_owned_heap_bytes)
            .chain(
                self.keys
                    .iter()
                    .map(SourceConfigKeyEvidence::estimated_owned_heap_bytes),
            )
            .fold(0usize, usize::saturating_add);
        let frontmatter = self
            .frontmatter
            .as_ref()
            .map_or(0, FrontmatterSourceEvidence::estimated_owned_heap_bytes);
        directive_storage
            .saturating_add(key_storage)
            .saturating_add(payload)
            .saturating_add(frontmatter)
    }

    pub(super) fn set_frontmatter(&mut self, frontmatter: FrontmatterSourceEvidence) {
        // Adding a missing `config` block is an insertion and preserves the existing YAML bytes.
        // Parser losslessness is required only when an existing config forces full frontmatter
        // materialization and replacement.
        if frontmatter.has_config {
            self.rewrite_safe &= frontmatter.rewrite_safe;
        }
        self.frontmatter = Some(frontmatter);
    }

    pub(super) fn set_config_insert_span(&mut self, span: SourceSpan) {
        self.config_insert_span = span;
    }

    pub(super) fn push_directive(&mut self, directive: SourceDirectiveEvidence) -> usize {
        let index = self.directives.len();
        if matches!(directive.keyword(), "init" | "initialize") {
            self.rewrite_safe &= directive.rewrite_safe;
        }
        self.directives.push(directive);
        index
    }

    pub(super) fn push_key(&mut self, key: SourceConfigKeyEvidence) {
        self.keys.push(key);
    }

    pub(super) fn mark_rewrite_unsafe(&mut self) {
        self.rewrite_safe = false;
    }
}

pub(super) struct Json5ConfigCapture {
    pub(super) value: Option<Value>,
    pub(super) keys: Vec<Json5ConfigKeyEvidence>,
    pub(super) rewrite_safe: bool,
}

#[derive(Debug, Clone)]
pub(super) struct Json5ConfigKeyEvidence {
    pub(super) path: Vec<String>,
    pub(super) span: Option<std::ops::Range<usize>>,
    pub(super) rewrite_safe: bool,
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
        path: Vec::new(),
        capture_keys: true,
    }
    .deserialize(&mut deserializer)
    .ok();

    let value = match value {
        Some(value) => match IgnoredAny::deserialize(&mut deserializer) {
            Err(error) if error.code() == Some(json5::ErrorCode::EofParsingValue) => Some(value),
            Ok(_) | Err(_) => {
                crate::config::drop_value_nonrecursive(value);
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
    path: Vec<String>,
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
    path: Vec<String>,
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
            path: Vec::new(),
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
            let mut path = if self.capture_keys {
                self.path.clone()
            } else {
                Vec::new()
            };
            if self.capture_keys {
                path.push(key.name.clone());
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
                crate::config::drop_value_nonrecursive(replaced);
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
