use crate::{Error, MermaidConfig, ParseMetadata, Result, SourceSpan};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlDiagramRenderModel {
    pub title: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_source: Option<String>,
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub group_id: Option<String>,
    pub explicit: bool,
    /// True for an explicit starter or the renderer-owned default starter synthesized when the
    /// selected ZenUML participant collector observes an empty context or a missing sender.
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
    pub id: Option<String>,
    pub participant_names: Vec<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlStatement {
    pub id: String,
    pub comment: Option<String>,
    pub span: SourceSpan,
    #[serde(flatten)]
    pub kind: ZenumlStatementKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ZenumlStatementKind {
    Message {
        explicit_from: Option<String>,
        resolved_from: Option<String>,
        resolved_to: Option<String>,
        label: String,
        assignment: Option<String>,
        style: ZenumlMessageStyle,
        body: Vec<ZenumlStatement>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body_comment: Option<String>,
    },
    Creation {
        resolved_from: Option<String>,
        resolved_to: String,
        constructor: String,
        parameters: String,
        assignment: Option<String>,
        label: String,
        body: Vec<ZenumlStatement>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body_comment: Option<String>,
    },
    Return {
        explicit_from: Option<String>,
        resolved_from: Option<String>,
        explicit_to: Option<String>,
        resolved_to: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_comment: Option<String>,
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
    if let Some(Value::Array(participants)) = object.get_mut("participants") {
        for participant in participants {
            let Value::Object(participant) = participant else {
                continue;
            };
            let width = participant
                .remove("widthSource")
                .and_then(|width| width.as_str().map(project_js_integer))
                .flatten()
                .and_then(serde_json::Number::from_f64)
                .map_or(Value::Null, Value::Number);
            participant.insert("width".to_string(), width);
        }
    }
    object.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    Ok(Value::Object(object))
}

fn project_js_integer(source: &str) -> Option<f64> {
    let parsed = source.parse::<f64>().ok()?;
    (parsed != 0.0).then_some(parsed)
}
