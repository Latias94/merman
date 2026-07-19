use crate::{Error, MermaidConfig, ParseMetadata, Result, SourceSpan};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlDiagramRenderModel {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub starter: Option<String>,
    pub participants: Vec<ZenumlParticipant>,
    pub groups: Vec<ZenumlGroup>,
    pub statements: Vec<ZenumlStatement>,
}

impl ZenumlDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, _config: &MermaidConfig) {
        // ZenUML text stays as data throughout the typed pipeline. The SVG emitter escapes every
        // text and attribute position, so an HTML sanitizer must not rewrite language semantics.
    }

    pub fn participant(&self, name: &str) -> Option<&ZenumlParticipant> {
        self.participants
            .iter()
            .find(|participant| participant.name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlParticipant {
    pub name: String,
    pub label: Option<String>,
    pub participant_type: Option<String>,
    pub stereotype: Option<String>,
    pub emoji: Option<String>,
    pub width: Option<u32>,
    pub color: Option<String>,
    pub group_id: Option<String>,
    pub explicit: bool,
    pub is_starter: bool,
    pub declaration_span: Option<SourceSpan>,
    pub occurrences: Vec<SourceSpan>,
}

impl ZenumlParticipant {
    pub fn display_name(&self) -> &str {
        self.label.as_deref().unwrap_or(self.name.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlGroup {
    pub id: String,
    pub participant_names: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlStatement {
    pub id: String,
    pub number: String,
    pub comment: Option<String>,
    pub span: SourceSpan,
    #[serde(flatten)]
    pub kind: ZenumlStatementKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ZenumlStatementKind {
    Message {
        from: String,
        to: String,
        label: String,
        assignment: Option<String>,
        style: ZenumlMessageStyle,
        body: Vec<ZenumlStatement>,
    },
    Creation {
        from: String,
        to: String,
        constructor: String,
        assignment: Option<String>,
        label: String,
        body: Vec<ZenumlStatement>,
    },
    Return {
        from: String,
        to: String,
        label: String,
    },
    Fragment {
        fragment_kind: ZenumlFragmentKind,
        label: Option<String>,
        sections: Vec<ZenumlFragmentSection>,
    },
    Reference {
        participants: Vec<String>,
        label: String,
    },
    Divider {
        label: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZenumlMessageStyle {
    Synchronous,
    Asynchronous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZenumlFragmentKind {
    Loop,
    Alternative,
    Parallel,
    Optional,
    Critical,
    Section,
    TryCatchFinally,
}

impl ZenumlFragmentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Loop => "loop",
            Self::Alternative => "alt",
            Self::Parallel => "par",
            Self::Optional => "opt",
            Self::Critical => "critical",
            Self::Section => "section",
            Self::TryCatchFinally => "tcf",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlFragmentSection {
    pub label: Option<String>,
    pub statements: Vec<ZenumlStatement>,
    pub span: SourceSpan,
}

pub(crate) fn render_model_to_compat_json(
    model: &ZenumlDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let mut object = match serde_json::to_value(model).map_err(|error| {
        Error::diagram_parse_fallback(meta.diagram_type.clone(), error.to_string())
    })? {
        Value::Object(object) => object,
        _ => Map::new(),
    };
    object.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    Ok(Value::Object(object))
}
