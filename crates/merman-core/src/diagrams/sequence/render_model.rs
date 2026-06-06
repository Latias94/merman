use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::collections::BTreeMap;

use super::{PLACEMENT_LEFT_OF, PLACEMENT_OVER, PLACEMENT_RIGHT_OF};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceDiagramRenderModel {
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "actorOrder")]
    pub actor_order: Vec<String>,
    pub actors: BTreeMap<String, SequenceActor>,
    #[serde(default)]
    pub boxes: Vec<SequenceBox>,
    pub messages: Vec<SequenceMessage>,
    #[serde(default)]
    pub notes: Vec<SequenceNote>,
    #[serde(rename = "createdActors", default)]
    pub created_actors: BTreeMap<String, usize>,
    #[serde(rename = "destroyedActors", default)]
    pub destroyed_actors: BTreeMap<String, usize>,
}

impl SequenceDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }

    pub(crate) fn to_compat_json(&self, diagram_type: &str) -> Value {
        let Value::Object(mut typed) =
            serde_json::to_value(self).expect("sequence render model must serialize")
        else {
            unreachable!("sequence render model must serialize to a JSON object");
        };

        let mut root = serde_json::Map::with_capacity(12);
        root.insert("type".to_string(), Value::String(diagram_type.to_string()));
        for key in [
            "title",
            "accTitle",
            "accDescr",
            "actorOrder",
            "actors",
            "messages",
            "notes",
            "boxes",
            "createdActors",
            "destroyedActors",
        ] {
            root.insert(
                key.to_string(),
                typed.remove(key).unwrap_or_else(|| {
                    panic!("sequence render model serialization missing field {key}")
                }),
            );
        }

        let mut placement = serde_json::Map::with_capacity(3);
        placement.insert(
            "leftOf".to_string(),
            Value::Number(PLACEMENT_LEFT_OF.into()),
        );
        placement.insert(
            "rightOf".to_string(),
            Value::Number(PLACEMENT_RIGHT_OF.into()),
        );
        placement.insert("over".to_string(), Value::Number(PLACEMENT_OVER.into()));
        let mut constants = serde_json::Map::with_capacity(1);
        constants.insert("placement".to_string(), Value::Object(placement));
        root.insert("constants".to_string(), Value::Object(constants));

        Value::Object(root)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceActor {
    #[serde(default)]
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub actor_type: String,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default)]
    pub links: serde_json::Map<String, Value>,
    #[serde(default)]
    pub properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceMessage {
    pub id: String,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(rename = "type")]
    pub message_type: i32,
    pub message: SequenceMessagePayload,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default)]
    pub activate: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<i32>,
    #[serde(
        rename = "centralConnection",
        default,
        skip_serializing_if = "is_zero_i32"
    )]
    pub central_connection: i32,
}

impl SequenceMessage {
    pub fn message_text(&self) -> &str {
        self.message.as_text()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SequenceMessagePayload {
    Text(String),
    Autonumber(SequenceAutonumber),
}

impl SequenceMessagePayload {
    pub fn as_text(&self) -> &str {
        match self {
            Self::Text(text) => text,
            Self::Autonumber(_) => "",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceAutonumber {
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_sequence_number"
    )]
    pub start: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_sequence_number"
    )]
    pub step: Option<f64>,
    #[serde(default = "default_true")]
    pub visible: bool,
}

fn serialize_optional_sequence_number<S>(
    value: &Option<f64>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serialize_sequence_number(*value, serializer),
        None => serializer.serialize_none(),
    }
}

fn serialize_sequence_number<S>(value: f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if value.is_finite()
        && value.fract() == 0.0
        && value >= i64::MIN as f64
        && value <= i64::MAX as f64
    {
        serializer.serialize_i64(value as i64)
    } else {
        serializer.serialize_f64(value)
    }
}

fn default_true() -> bool {
    true
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceBox {
    #[serde(rename = "actorKeys")]
    pub actor_keys: Vec<String>,
    pub fill: String,
    pub name: Option<String>,
    #[serde(default)]
    pub wrap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceNote {
    pub actor: Value,
    pub message: String,
    pub placement: i32,
    #[serde(default)]
    pub wrap: bool,
}
