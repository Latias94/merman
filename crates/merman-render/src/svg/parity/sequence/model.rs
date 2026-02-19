use super::super::*;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct SequenceSvgActor {
    pub(super) description: String,
    #[serde(rename = "type")]
    pub(super) actor_type: String,
    #[serde(default)]
    pub(super) wrap: bool,
    #[serde(default)]
    pub(super) links: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub(super) properties: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct SequenceSvgMessage {
    pub(super) id: String,
    #[serde(default)]
    pub(super) from: Option<String>,
    #[serde(default)]
    pub(super) to: Option<String>,
    #[serde(rename = "type")]
    pub(super) message_type: i32,
    pub(super) message: serde_json::Value,
    #[serde(default)]
    pub(super) wrap: bool,
    #[serde(default)]
    pub(super) activate: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct SequenceSvgModel {
    #[serde(rename = "accTitle")]
    pub(super) acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub(super) acc_descr: Option<String>,
    pub(super) title: Option<String>,
    #[serde(rename = "actorOrder")]
    pub(super) actor_order: Vec<String>,
    pub(super) actors: std::collections::BTreeMap<String, SequenceSvgActor>,
    #[serde(default)]
    pub(super) boxes: Vec<SequenceSvgBox>,
    pub(super) messages: Vec<SequenceSvgMessage>,
    #[serde(default)]
    #[allow(dead_code)]
    pub(super) notes: Vec<SequenceSvgNote>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct SequenceSvgBox {
    #[serde(rename = "actorKeys")]
    pub(super) actor_keys: Vec<String>,
    pub(super) fill: String,
    pub(super) name: Option<String>,
    #[allow(dead_code)]
    pub(super) wrap: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct SequenceSvgNote {
    #[allow(dead_code)]
    pub(super) actor: serde_json::Value,
    #[allow(dead_code)]
    pub(super) message: String,
    #[allow(dead_code)]
    pub(super) placement: i32,
    #[allow(dead_code)]
    pub(super) wrap: bool,
}
