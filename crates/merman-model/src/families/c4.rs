//! Canonical C4 data and layout vocabulary.

#[cfg(feature = "compat-json")]
pub mod compat;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum C4Text {
    Wrapped { text: Value },
    String(String),
    Value(Value),
}

impl Default for C4Text {
    fn default() -> Self {
        Self::String(String::new())
    }
}

impl C4Text {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Wrapped { text } => text.as_str().unwrap_or(""),
            Self::String(s) => s.as_str(),
            Self::Value(v) => v.as_str().unwrap_or(""),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct C4LayoutConfig {
    /// JavaScript Number after `parseInt` and the database's minimum-one check.
    #[serde(default, rename = "c4ShapeInRow", with = "crate::number::serde_number")]
    pub c4_shape_in_row: f64,
    /// JavaScript Number after `parseInt` and the database's minimum-one check.
    #[serde(
        default,
        rename = "c4BoundaryInRow",
        with = "crate::number::serde_number"
    )]
    pub c4_boundary_in_row: f64,
}

/// A relationship offset as stored by Mermaid's C4 database.
///
/// Named offsets in the color argument slots remain strings; offsets in the offset slots
/// have already passed through `parseInt`. The renderer coerces both again before use.
/// Canonical JSON wraps nonfinite Numbers as `{"number":"NaN"}` (or either infinity tag)
/// so literal string offsets retain their meaning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum C4RelationOffset {
    Number(#[serde(with = "relation_offset_number")] f64),
    String(String),
}

/// Parses the integer prefix consumed by C4's database and relationship renderer.
///
/// This is JavaScript `parseInt` with an omitted radix: only decimal and `0x` hexadecimal
/// prefixes are recognized. Results retain binary64 rounding, signed zero, and overflow.
pub fn parse_integer_prefix(input: &str) -> Option<f64> {
    let input = input.trim_start_matches(crate::text::is_ecmascript_whitespace);
    let (negative, unsigned) = match input.as_bytes().first() {
        Some(b'-') => (true, &input[1..]),
        Some(b'+') => (false, &input[1..]),
        _ => (false, input),
    };
    let (hexadecimal, digits) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
        .map_or((false, unsigned), |digits| (true, digits));
    let digit_count = digits
        .bytes()
        .take_while(|byte| {
            if hexadecimal {
                byte.is_ascii_hexdigit()
            } else {
                byte.is_ascii_digit()
            }
        })
        .count();
    if digit_count == 0 {
        return None;
    }

    let digits = &digits[..digit_count];
    let magnitude = if hexadecimal {
        crate::number::parse_power_of_two_digits(digits, 16)?
    } else {
        digits.parse::<f64>().ok()?
    };
    Some(if negative { -magnitude } else { magnitude })
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct C4Shape {
    pub alias: String,
    #[serde(default, rename = "parentBoundary")]
    pub parent_boundary: String,
    #[serde(default, rename = "typeC4Shape")]
    pub type_c4_shape: C4Text,
    #[serde(default)]
    pub label: C4Text,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Value>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub ty: Option<C4Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub techn: Option<C4Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descr: Option<C4Text>,
    #[serde(default, rename = "bgColor", skip_serializing_if = "Option::is_none")]
    pub bg_color: Option<String>,
    #[serde(
        default,
        rename = "borderColor",
        skip_serializing_if = "Option::is_none"
    )]
    pub border_color: Option<String>,
    #[serde(default, rename = "fontColor", skip_serializing_if = "Option::is_none")]
    pub font_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadowing: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<Value>,
    #[serde(
        default,
        rename = "legendText",
        skip_serializing_if = "Option::is_none"
    )]
    pub legend_text: Option<Value>,
    #[serde(
        default,
        rename = "legendSprite",
        skip_serializing_if = "Option::is_none"
    )]
    pub legend_sprite: Option<Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct C4Boundary {
    pub alias: String,
    #[serde(default, rename = "parentBoundary")]
    pub parent_boundary: String,
    #[serde(default)]
    pub label: C4Text,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub ty: Option<C4Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descr: Option<C4Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Value>,
    #[serde(default, rename = "nodeType", skip_serializing_if = "Option::is_none")]
    pub node_type: Option<String>,
    #[serde(default, rename = "bgColor", skip_serializing_if = "Option::is_none")]
    pub bg_color: Option<String>,
    #[serde(
        default,
        rename = "borderColor",
        skip_serializing_if = "Option::is_none"
    )]
    pub border_color: Option<String>,
    #[serde(default, rename = "fontColor", skip_serializing_if = "Option::is_none")]
    pub font_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadowing: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub techn: Option<Value>,
    #[serde(
        default,
        rename = "legendText",
        skip_serializing_if = "Option::is_none"
    )]
    pub legend_text: Option<Value>,
    #[serde(
        default,
        rename = "legendSprite",
        skip_serializing_if = "Option::is_none"
    )]
    pub legend_sprite: Option<Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct C4Relation {
    #[serde(rename = "from")]
    pub from_alias: String,
    #[serde(rename = "to")]
    pub to_alias: String,
    #[serde(rename = "type")]
    pub rel_type: String,
    #[serde(default)]
    pub label: C4Text,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub techn: Option<C4Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descr: Option<C4Text>,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Value>,
    #[serde(default, rename = "offsetX", skip_serializing_if = "Option::is_none")]
    pub offset_x: Option<C4RelationOffset>,
    #[serde(default, rename = "offsetY", skip_serializing_if = "Option::is_none")]
    pub offset_y: Option<C4RelationOffset>,
    #[serde(default, rename = "lineColor", skip_serializing_if = "Option::is_none")]
    pub line_color: Option<String>,
    #[serde(default, rename = "textColor", skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct C4DiagramModel {
    #[serde(default, rename = "c4Type")]
    pub c4_type: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default)]
    pub layout: C4LayoutConfig,
    #[serde(default)]
    pub shapes: Vec<C4Shape>,
    #[serde(default)]
    pub boundaries: Vec<C4Boundary>,
    #[serde(default)]
    pub rels: Vec<C4Relation>,
}

mod relation_offset_number {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    enum NonfiniteNumber {
        #[serde(rename = "number")]
        Number(#[serde(with = "crate::number::serde_number")] f64),
    }

    pub fn serialize<S: serde::Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        if value.is_finite() {
            value.serialize(serializer)
        } else {
            NonfiniteNumber::Number(*value).serialize(serializer)
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Number {
            Finite(f64),
            Nonfinite(NonfiniteNumber),
        }

        match Number::deserialize(deserializer)? {
            Number::Finite(value) => Ok(value),
            Number::Nonfinite(NonfiniteNumber::Number(value)) if !value.is_finite() => Ok(value),
            Number::Nonfinite(_) => Err(serde::de::Error::custom(
                "expected a nonfinite number in an offset number object",
            )),
        }
    }
}
