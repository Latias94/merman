#[cfg(feature = "compat-json")]
pub mod compat;

use crate::json::ManagedJson;
use crate::{OperationControl, OperationControlResult, SourceSpan};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Canonical ZenUML participants, authored statements, and resolved message ownership.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlDiagramModel {
    pub title: Option<String>,
    pub starter: Option<String>,
    pub participants: Vec<ZenumlParticipant>,
    pub groups: Vec<ZenumlGroup>,
    pub statements: Vec<ZenumlStatement>,
}

impl Serialize for ZenumlDiagramModel {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let projected = model_to_typed_json_controlled(self, &OperationControl::new())
            .expect("a private projection control cannot be cancelled")
            .map_err(serde::ser::Error::custom)?;
        projected.serialize(serializer)
    }
}

impl ZenumlDiagramModel {
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

/// Projects typed serde fields iteratively while retaining managed JSON ownership.
///
/// The operation control observes cancellation during traversal. The returned value retains
/// iterative clone, disposal, and JSON export behavior, including for partial projections.
pub fn model_to_typed_json_controlled(
    model: &ZenumlDiagramModel,
    control: &OperationControl,
) -> OperationControlResult<serde_json::Result<ManagedJson>> {
    enum Task<'a> {
        Statements(&'a [ZenumlStatement]),
        Statement(&'a ZenumlStatement),
        Sections(&'a [ZenumlFragmentSection]),
        Section(&'a ZenumlFragmentSection),
        FinishArray(usize),
        FinishObject(ManagedJson, &'static str),
    }

    let mut root = ManagedJson::from_value(json!({
        "title": model.title,
        "starter": model.starter,
        "participants": [],
        "groups": [],
        "statements": [],
    }));
    for (index, participant) in model.participants.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let value = match serde_json::to_value(participant) {
            Ok(value) => value,
            Err(error) => return Ok(Err(error)),
        };
        root.as_value_mut()["participants"]
            .as_array_mut()
            .unwrap()
            .push(value);
    }
    for (index, group) in model.groups.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let value = match serde_json::to_value(group) {
            Ok(value) => value,
            Err(error) => return Ok(Err(error)),
        };
        root.as_value_mut()["groups"]
            .as_array_mut()
            .unwrap()
            .push(value);
    }

    let mut tasks = vec![Task::Statements(&model.statements)];
    let mut completed = Vec::<ManagedJson>::new();
    while let Some(task) = tasks.pop() {
        control.checkpoint()?;
        match task {
            Task::Statements(statements) => {
                tasks.push(Task::FinishArray(statements.len()));
                tasks.extend(statements.iter().rev().map(Task::Statement));
            }
            Task::Sections(sections) => {
                tasks.push(Task::FinishArray(sections.len()));
                tasks.extend(sections.iter().rev().map(Task::Section));
            }
            Task::Statement(statement) => {
                let object = ManagedJson::from_value(statement_to_value_shallow(statement));
                match &statement.kind {
                    ZenumlStatementKind::Message { body, .. }
                    | ZenumlStatementKind::Creation { body, .. } => {
                        tasks.push(Task::FinishObject(object, "body"));
                        tasks.push(Task::Statements(body));
                    }
                    ZenumlStatementKind::Fragment { sections, .. } => {
                        tasks.push(Task::FinishObject(object, "sections"));
                        tasks.push(Task::Sections(sections));
                    }
                    _ => completed.push(object),
                }
            }
            Task::Section(section) => {
                let mut object = json!({ "label": section.label, "statements": [] });
                if let Some(comment) = &section.body_comment {
                    object
                        .as_object_mut()
                        .unwrap()
                        .insert("bodyComment".into(), json!(comment));
                }
                object
                    .as_object_mut()
                    .unwrap()
                    .insert("span".into(), json!(section.span));
                tasks.push(Task::FinishObject(
                    ManagedJson::from_value(object),
                    "statements",
                ));
                tasks.push(Task::Statements(&section.statements));
            }
            Task::FinishArray(count) => {
                let children = completed.drain(completed.len() - count..);
                let mut array = ManagedJson::from_value(Value::Array(Vec::with_capacity(count)));
                for (index, child) in children.enumerate() {
                    if index % 128 == 0 {
                        control.checkpoint()?;
                    }
                    array
                        .as_value_mut()
                        .as_array_mut()
                        .unwrap()
                        .push(child.into_unmanaged_value());
                }
                completed.push(array);
            }
            Task::FinishObject(mut object, field) => {
                let child = completed
                    .pop()
                    .expect("child projection completes before its owner");
                object
                    .as_value_mut()
                    .as_object_mut()
                    .unwrap()
                    .insert(field.into(), child.into_unmanaged_value());
                completed.push(object);
            }
        }
    }
    let statements = completed
        .pop()
        .expect("root statement projection completes last");
    root.as_value_mut()
        .as_object_mut()
        .unwrap()
        .insert("statements".into(), statements.into_unmanaged_value());
    control.checkpoint()?;
    Ok(Ok(root))
}

fn statement_to_value_shallow(statement: &ZenumlStatement) -> Value {
    let value = match &statement.kind {
        ZenumlStatementKind::Message {
            explicit_from,
            resolved_from,
            resolved_to,
            label,
            assignment,
            style,
            body_comment,
            ..
        } => {
            let mut value = json!({ "kind": "message", "explicit_from": explicit_from, "resolved_from": resolved_from, "resolved_to": resolved_to, "label": label, "assignment": assignment, "style": style, "body": [] });
            if let Some(comment) = body_comment {
                value
                    .as_object_mut()
                    .unwrap()
                    .insert("body_comment".into(), json!(comment));
            }
            value
        }
        ZenumlStatementKind::Creation {
            resolved_from,
            resolved_to,
            constructor,
            parameters,
            assignment,
            label,
            body_comment,
            ..
        } => {
            let mut value = json!({ "kind": "creation", "resolved_from": resolved_from, "resolved_to": resolved_to, "constructor": constructor, "parameters": parameters, "assignment": assignment, "label": label, "body": [] });
            if let Some(comment) = body_comment {
                value
                    .as_object_mut()
                    .unwrap()
                    .insert("body_comment".into(), json!(comment));
            }
            value
        }
        ZenumlStatementKind::Return {
            explicit_from,
            resolved_from,
            explicit_to,
            resolved_to,
            label,
        } => {
            json!({ "kind": "return", "explicit_from": explicit_from, "resolved_from": resolved_from, "explicit_to": explicit_to, "resolved_to": resolved_to, "label": label })
        }
        ZenumlStatementKind::Fragment {
            fragment_kind,
            label,
            ..
        } => {
            json!({ "kind": "fragment", "fragment_kind": fragment_kind, "label": label, "sections": [] })
        }
        ZenumlStatementKind::Reference {
            participants,
            label,
        } => json!({ "kind": "reference", "participants": participants, "label": label }),
        ZenumlStatementKind::Divider { label } => json!({ "kind": "divider", "label": label }),
    };
    let mut object = Map::new();
    object.insert("id".into(), json!(statement.id));
    object.insert("comment".into(), json!(statement.comment));
    object.insert("span".into(), json!(statement.span));
    let Value::Object(fields) = value else {
        unreachable!("statement fields form an object")
    };
    object.extend(fields);
    Value::Object(object)
}
