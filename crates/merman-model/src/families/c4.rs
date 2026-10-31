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
    #[serde(default, rename = "c4ShapeInRow")]
    pub c4_shape_in_row: i64,
    #[serde(default, rename = "c4BoundaryInRow")]
    pub c4_boundary_in_row: i64,
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
    pub offset_x: Option<i64>,
    #[serde(default, rename = "offsetY", skip_serializing_if = "Option::is_none")]
    pub offset_y: Option<i64>,
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
