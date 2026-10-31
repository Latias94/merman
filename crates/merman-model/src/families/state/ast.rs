use crate::SourceSpan;

use super::StateDocumentId;

#[derive(Debug, Clone)]
pub struct StateStatementNote {
    pub position: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct StateStatementClick {
    pub id: String,
    pub url: String,
    pub tooltip: String,
}

#[derive(Debug, Clone)]
pub struct StateStatementState {
    pub id: String,
    pub id_span: Option<SourceSpan>,
    pub ty: String,
    pub description: Option<String>,
    pub descriptions: Vec<String>,
    pub doc: Option<StateDocumentId>,
    pub note: Option<StateStatementNote>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
    pub start: Option<bool>,
}

impl StateStatementState {
    pub fn new(id: String) -> Self {
        Self {
            id,
            id_span: None,
            ty: "default".to_string(),
            description: None,
            descriptions: Vec::new(),
            doc: None,
            note: None,
            classes: Vec::new(),
            styles: Vec::new(),
            text_styles: Vec::new(),
            start: None,
        }
    }

    pub fn new_typed(id: String, ty: &str) -> Self {
        Self {
            ty: ty.to_string(),
            ..Self::new(id)
        }
    }
}

#[derive(Debug, Clone)]
pub struct StateStatementRelation {
    pub state1: StateStatementState,
    pub state2: StateStatementState,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub enum StateStatement {
    Noop,
    State(StateStatementState),
    Relation(Box<StateStatementRelation>),
    ClassDef { id: String, classes: String },
    ApplyClass { ids: String, class_name: String },
    Style { ids: String, styles: String },
    Direction(String),
    AccTitle(String),
    AccDescr(String),
    Click(StateStatementClick),
}
