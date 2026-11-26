use super::{ClassAssignStmt, ClassDefStmt, ClickStmt, LinkStyleStmt, StyleStmt};
use merman_model::SourceSpan;
use merman_model::families::flowchart::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility};

pub type FlowchartLexicalEvent = Result<(usize, Tok, usize), LexError>;

#[derive(Debug, Clone)]
#[cfg_attr(
    not(any(feature = "diagram-flowchart", feature = "diagram-swimlane")),
    allow(
        dead_code,
        reason = "Agentflow shares presentation lexing but does not consume Flowchart grammar and recovery payloads."
    )
)]
pub enum Tok {
    KwGraph,
    KwFlowchart,
    KwFlowchartElk,
    KwSwimlane,
    KwSubgraph,
    KwEnd,

    Sep,
    AccessibilityValue(String),
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
#[cfg_attr(
    not(any(feature = "diagram-flowchart", feature = "diagram-swimlane")),
    allow(
        dead_code,
        reason = "Agentflow shares presentation lexing but does not consume Flowchart grammar and recovery payloads."
    )
)]
pub struct ArrowToken {
    pub link: LinkToken,
    pub recovery_error: Option<LexError>,
}

#[derive(Debug, Clone)]
#[cfg_attr(
    not(any(feature = "diagram-flowchart", feature = "diagram-swimlane")),
    allow(
        dead_code,
        reason = "Agentflow shares presentation lexing but does not consume Flowchart grammar and recovery payloads."
    )
)]
pub struct NodeLabelToken {
    pub shape: String,
    pub text: LabeledText,
    #[cfg(feature = "editor-facts")]
    pub trigger_span: Option<SourceSpan>,
    /// Strict parser error represented by this editor-recovery token.
    ///
    /// The combined semantic path lexes once. An incomplete label therefore carries both the
    /// token needed for editor facts and the error projected to the strict parser.
    #[cfg(any(test, feature = "editor-facts"))]
    pub recovery_error: Option<LexError>,
}

#[derive(Debug, Clone)]
#[cfg_attr(
    not(any(feature = "diagram-flowchart", feature = "diagram-swimlane")),
    allow(
        dead_code,
        reason = "Agentflow shares presentation lexing but does not consume Flowchart grammar and recovery payloads."
    )
)]
pub struct DirectionStatementToken {
    pub direction: String,
    #[cfg(any(test, feature = "editor-facts"))]
    pub completion_span: Option<SourceSpan>,
    #[cfg(any(test, feature = "editor-facts"))]
    pub recovery_error: Option<LexError>,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct LexError {
    pub message: String,
    pub span: Option<SourceSpan>,
    #[cfg(feature = "editor-facts")]
    pub expected_syntax: Vec<FlowchartLexicalExpectation>,
    #[cfg(feature = "editor-facts")]
    pub directive_prefix: Option<&'static str>,
}

impl LexError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            #[cfg(feature = "editor-facts")]
            expected_syntax: Vec::new(),
            #[cfg(feature = "editor-facts")]
            directive_prefix: None,
        }
    }

    pub fn with_span(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
            #[cfg(feature = "editor-facts")]
            expected_syntax: Vec::new(),
            #[cfg(feature = "editor-facts")]
            directive_prefix: None,
        }
    }

    #[cfg(feature = "editor-facts")]
    pub fn expecting(mut self, kind: FlowchartExpectationKind, span: SourceSpan) -> Self {
        let expected = FlowchartLexicalExpectation::new(kind, span);
        if !self.expected_syntax.contains(&expected) {
            self.expected_syntax.push(expected);
        }
        self
    }

    #[cfg(feature = "editor-facts")]
    pub fn in_directive(mut self, prefix: &'static str, span: SourceSpan) -> Self {
        self.directive_prefix = Some(prefix);
        self.expecting(FlowchartExpectationKind::Directive, span)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(
    not(any(feature = "diagram-flowchart", feature = "diagram-swimlane")),
    allow(
        dead_code,
        reason = "Agentflow shares presentation lexing but does not consume Flowchart grammar and recovery payloads."
    )
)]
pub struct LinkToken {
    pub end: String,
    pub start_marker: FlowEdgeMarker,
    pub end_marker: FlowEdgeMarker,
    pub stroke_kind: FlowEdgeStroke,
    pub visibility: FlowEdgeVisibility,
    pub length: usize,
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
impl LinkToken {
    /// Reads the marker at the start of the final operator captured by the lexer.
    pub fn final_operator_start_marker(&self) -> FlowEdgeMarker {
        super::link::operator_start_marker(&self.end)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TitleKind {
    Text,
    String,
    Markdown,
}

#[derive(Debug, Clone)]
pub struct LabeledText {
    pub text: String,
    pub kind: TitleKind,
    pub span: Option<SourceSpan>,
    pub selection: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgraphHeader {
    pub raw_id: String,
    pub header_span: Option<SourceSpan>,
    pub raw_id_span: Option<SourceSpan>,
    pub raw_title: String,
    pub title_kind: TitleKind,
    pub id_equals_title: bool,
}

impl Default for SubgraphHeader {
    fn default() -> Self {
        Self {
            raw_id: String::new(),
            header_span: None,
            raw_id_span: None,
            raw_title: String::new(),
            title_kind: TitleKind::Text,
            id_equals_title: true,
        }
    }
}

impl TitleKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::String => "string",
            Self::Markdown => "markdown",
        }
    }
}

/// A lexical slot with no editor policy or source remapping applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "editor-facts")]
pub struct FlowchartLexicalExpectation {
    pub kind: FlowchartExpectationKind,
    pub span: SourceSpan,
}

#[cfg(feature = "editor-facts")]
impl FlowchartLexicalExpectation {
    pub(super) const fn new(kind: FlowchartExpectationKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "editor-facts")]
pub enum FlowchartExpectationKind {
    Directive,
    NodeIdentifier,
    StyleValue,
    ClassName,
    IdList,
    InteractionAction,
    Payload,
    ShapeValue,
    DirectionValue,
    Operator,
}
