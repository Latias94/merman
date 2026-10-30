//! Canonical Usecase records and lossless JavaScript JSON data codecs.

#[cfg(feature = "compat-json")]
pub mod compat;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub mod json_string;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UsecaseNodeKind {
    Actor,
    UseCaseEllipse,
    UseCaseRect,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UsecaseLabelType {
    #[default]
    Text,
    Markdown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UsecaseActorType {
    #[default]
    Normal,
    Hollow,
    Awesome,
    Icon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseNode {
    pub id: String,
    pub label: String,
    pub label_type: UsecaseLabelType,
    pub kind: UsecaseNodeKind,
    pub actor_type: UsecaseActorType,
    pub icon: Option<String>,
    pub parent_id: Option<String>,
    pub business: bool,
    pub stereotype: Option<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseBoundary {
    pub id: String,
    pub label: String,
    pub label_type: UsecaseLabelType,
    pub members: Vec<String>,
    pub package: bool,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UsecaseRelationshipType {
    Association,
    Include,
    Extend,
    Generalization,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "u8", into = "u8")]
pub enum UsecaseArrowType {
    Point,
    ReversedPoint,
    Line,
    Circle,
    Cross,
    ReversedCircle,
    ReversedCross,
}

impl From<UsecaseArrowType> for u8 {
    fn from(value: UsecaseArrowType) -> Self {
        value as u8
    }
}
impl TryFrom<u8> for UsecaseArrowType {
    type Error = &'static str;
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Point),
            1 => Ok(Self::ReversedPoint),
            2 => Ok(Self::Line),
            3 => Ok(Self::Circle),
            4 => Ok(Self::Cross),
            5 => Ok(Self::ReversedCircle),
            6 => Ok(Self::ReversedCross),
            _ => Err("invalid Usecase arrow type"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UsecaseAnimation {
    Fast,
    Slow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseRelationship {
    pub id: String,
    pub explicit_id: bool,
    pub source: String,
    pub target: String,
    pub relationship_type: UsecaseRelationshipType,
    /// Upstream ARROW_TYPE: point, reversed point, line, circle, cross,
    /// reversed circle, and reversed cross, respectively.
    pub arrow_type: UsecaseArrowType,
    pub label: Option<String>,
    pub label_type: Option<UsecaseLabelType>,
    pub dotted: bool,
    pub minlen: usize,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub animate: bool,
    pub animation: Option<UsecaseAnimation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseNote {
    pub id: String,
    pub target: String,
    pub label: String,
    pub label_type: UsecaseLabelType,
}

/// JavaScript JSON numbers whose binary64 value overflows. Compatibility JSON
/// serializes these as null, while Usecase table labels retain their sign.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UsecaseJsonInfinity {
    Positive,
    Negative,
}

/// Lossless JSON string storage for values containing unpaired UTF-16 surrogates.
/// In this mode every object key and string value is a complete canonical JSON
/// string literal. Decode explicitly; these strings are not ordinary display text.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum UsecaseJsonStringEncoding {
    #[serde(rename = "json-utf16")]
    JsonUtf16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsecaseJsonNode {
    pub id: String,
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub string_encoding: Option<UsecaseJsonStringEncoding>,
    /// Source property order, keyed by RFC 6901 JSON pointers, including the root.
    pub property_order: BTreeMap<String, Vec<String>>,
    /// Non-finite table values keyed by RFC 6901 JSON pointers. Their entries in
    /// `value` are null, matching JavaScript JSON.stringify serialization.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub non_finite_numbers: BTreeMap<String, UsecaseJsonInfinity>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

impl UsecaseJsonNode {
    /// Returns UTF-8 display text, replacing each unpaired surrogate only at the
    /// rendering boundary, as Mermaid CLI's TextEncoder does for SVG output.
    pub fn display_string<'a>(
        &self,
        value: &'a str,
    ) -> std::result::Result<std::borrow::Cow<'a, str>, serde_json::Error> {
        if self.string_encoding.is_none() {
            return Ok(std::borrow::Cow::Borrowed(value));
        }
        Ok(std::borrow::Cow::Owned(String::from_utf16_lossy(
            &json_string::code_units(value)?,
        )))
    }

    /// Returns the original JavaScript UTF-16 code units without replacement.
    pub fn string_code_units(
        &self,
        value: &str,
    ) -> std::result::Result<Vec<u16>, serde_json::Error> {
        if self.string_encoding.is_none() {
            return Ok(value.encode_utf16().collect());
        }
        json_string::code_units(value)
    }

    /// Exports ordinary JavaScript JSON, restoring encoded keys and strings.
    /// Non-finite values remain null, matching JSON.stringify. Property order
    /// follows the source-order metadata; malformed encoded strings return errors.
    pub fn to_javascript_json(&self) -> std::result::Result<String, serde_json::Error> {
        fn write_value(
            node: &UsecaseJsonNode,
            value: &Value,
            pointer: &str,
            depth: usize,
            output: &mut String,
        ) -> std::result::Result<(), serde_json::Error> {
            if depth > 128 {
                return Err(<serde_json::Error as serde::de::Error>::custom(
                    "Usecase JSON nesting depth exceeds 128",
                ));
            }
            match value {
                Value::String(value) => {
                    if node.string_encoding.is_some() {
                        output.push_str(&json_string::canonical(value)?);
                    } else {
                        output.push_str(&serde_json::to_string(value)?);
                    }
                }
                Value::Array(values) => {
                    output.push('[');
                    for (index, value) in values.iter().enumerate() {
                        if index > 0 {
                            output.push(',');
                        }
                        write_value(
                            node,
                            value,
                            &format!("{pointer}/{index}"),
                            depth + 1,
                            output,
                        )?;
                    }
                    output.push(']');
                }
                Value::Object(values) => {
                    output.push('{');
                    let order = node.property_order.get(pointer);
                    let mut seen = std::collections::HashSet::with_capacity(values.len());
                    let mut first = true;
                    for key in order.into_iter().flatten().chain(values.keys()) {
                        let Some(value) = values.get(key) else {
                            continue;
                        };
                        if !seen.insert(key) {
                            continue;
                        }
                        if !first {
                            output.push(',');
                        }
                        first = false;
                        if node.string_encoding.is_some() {
                            output.push_str(&json_string::canonical(key)?);
                        } else {
                            output.push_str(&serde_json::to_string(key)?);
                        }
                        output.push(':');
                        let escaped = key.replace('~', "~0").replace('/', "~1");
                        write_value(
                            node,
                            value,
                            &format!("{pointer}/{escaped}"),
                            depth + 1,
                            output,
                        )?;
                    }
                    output.push('}');
                }
                _ => output.push_str(&serde_json::to_string(value)?),
            }
            Ok(())
        }
        let mut output = String::new();
        write_value(self, &self.value, "", 0, &mut output)?;
        Ok(output)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsecaseClassDef {
    pub id: String,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Resolved use cases, actors, boundaries, relationships, and source-authored JSON records.
pub struct UsecaseDiagramModel {
    pub direction: String,
    pub nodes: Vec<UsecaseNode>,
    pub boundaries: Vec<UsecaseBoundary>,
    pub relationships: Vec<UsecaseRelationship>,
    pub notes: Vec<UsecaseNote>,
    pub json_nodes: Vec<UsecaseJsonNode>,
    pub class_defs: Vec<UsecaseClassDef>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_description: Option<String>,
}
