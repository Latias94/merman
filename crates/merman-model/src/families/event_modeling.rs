#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventModelingFrame {
    pub name: String,
    #[serde(rename = "frameKind")]
    pub frame_kind: String,
    #[serde(rename = "modelEntityType")]
    pub model_entity_type: String,
    #[serde(rename = "entityIdentifier")]
    pub entity_identifier: String,
    #[serde(default, rename = "sourceFrames")]
    pub source_frames: Vec<String>,
    #[serde(default, rename = "dataInlineValue")]
    pub data_inline_value: Option<String>,
    #[serde(default, rename = "dataReference")]
    pub data_reference: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventModelingDataEntity {
    pub name: String,
    #[serde(rename = "dataBlockValue")]
    pub data_block_value: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
/// Domain frames and data entities constructed by the Event Modeling grammar.
pub struct EventModelingDiagramModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub frames: Vec<EventModelingFrame>,
    #[serde(default, rename = "dataEntities")]
    pub data_entities: Vec<EventModelingDataEntity>,
}
