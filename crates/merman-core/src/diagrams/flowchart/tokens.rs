use super::{
    ClassAssignStmt, ClassDefStmt, ClickStmt, LabeledText, LinkStyleStmt, LinkToken, StyleStmt,
    SubgraphHeader,
};
use crate::{SourceSpan, error::ParseErrorSourceSpan};

#[derive(Debug, Clone)]
pub(crate) enum Tok {
    KwGraph,
    KwFlowchart,
    KwFlowchartElk,
    KwSwimlane,
    KwSubgraph,
    KwEnd,

    Sep,
    Amp,
    StyleSep,
    NodeLabel(NodeLabelToken),

    Direction(String),
    DirectionStmt(String),
    Id(String),
    Arrow(LinkToken),
    EdgeLabel(LabeledText),
    SubgraphHeader(SubgraphHeader),

    StyleStmt(StyleStmt),
    ClassDefStmt(ClassDefStmt),
    ClassAssignStmt(ClassAssignStmt),
    ClickStmt(ClickStmt),
    LinkStyleStmt(LinkStyleStmt),

    EdgeId(String),
    ShapeData(String),
}

#[derive(Debug, Clone)]
pub(crate) struct NodeLabelToken {
    pub shape: String,
    pub text: LabeledText,
    pub trigger_span: Option<SourceSpan>,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub(crate) struct LexError {
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl LexError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }

    pub(crate) fn with_span(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
        }
    }
}

impl ParseErrorSourceSpan for LexError {
    fn source_span(&self) -> Option<SourceSpan> {
        self.span
    }
}
