use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

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
    #[serde(default)]
    pub placement: Option<i32>,
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

    pub(crate) fn into_value(self) -> Value {
        match self {
            Self::Text(text) => Value::String(text),
            Self::Autonumber(v) => {
                serde_json::to_value(v).expect("sequence autonumber payload must serialize")
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceAutonumber {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<i64>,
    #[serde(default = "default_true")]
    pub visible: bool,
}

fn default_true() -> bool {
    true
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
