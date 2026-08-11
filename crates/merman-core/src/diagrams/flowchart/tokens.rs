use super::{
    ClassAssignStmt, ClassDefStmt, ClickStmt, FlowchartLexemeComponent, LabeledText, LinkStyleStmt,
    LinkToken, StyleStmt, SubgraphHeader,
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
    DirectionStmt(DirectionStatementToken),
    Id(String),
    Arrow(ArrowToken),
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
pub(crate) struct ArrowToken {
    pub link: LinkToken,
    pub lexeme_components: Vec<FlowchartLexemeComponent>,
    pub recovery_error: Option<LexError>,
}

#[derive(Debug, Clone)]
pub(crate) struct NodeLabelToken {
    pub shape: String,
    pub text: LabeledText,
    pub trigger_span: Option<SourceSpan>,
    pub lexeme_components: Vec<FlowchartLexemeComponent>,
    /// Strict parser error represented by this editor-recovery token.
    ///
    /// The combined semantic path lexes once. An incomplete label therefore carries both the
    /// token needed for editor facts and the error projected to the strict LALRPOP parser.
    pub recovery_error: Option<LexError>,
}

#[derive(Debug, Clone)]
pub(crate) struct DirectionStatementToken {
    pub direction: String,
    pub selection: SourceSpan,
    pub lexeme_components: Vec<FlowchartLexemeComponent>,
    pub recovery_error: Option<LexError>,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub(crate) struct LexError {
    pub message: String,
    pub span: Option<SourceSpan>,
    pub expected_syntax: Vec<crate::EditorExpectedSyntax>,
    pub directive_prefix: Option<&'static str>,
}

impl LexError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            expected_syntax: Vec::new(),
            directive_prefix: None,
        }
    }

    pub(crate) fn with_span(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
            expected_syntax: Vec::new(),
            directive_prefix: None,
        }
    }

    pub(crate) fn expecting(
        mut self,
        kind: crate::EditorExpectedSyntaxKind,
        span: SourceSpan,
    ) -> Self {
        let expected = crate::EditorExpectedSyntax::new(kind, span);
        if !self.expected_syntax.contains(&expected) {
            self.expected_syntax.push(expected);
        }
        self
    }

    pub(crate) fn in_directive(mut self, prefix: &'static str, span: SourceSpan) -> Self {
        self.directive_prefix = Some(prefix);
        self.expecting(crate::EditorExpectedSyntaxKind::Directive, span)
    }
}

impl ParseErrorSourceSpan for LexError {
    fn source_span(&self) -> Option<SourceSpan> {
        self.span
    }
}
