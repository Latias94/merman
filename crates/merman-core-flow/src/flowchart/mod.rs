//! Flowchart lexical events, source grammar actions, and semantic model construction.

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
mod accessibility;
mod ast;
mod lex;
mod lexer;
mod lexer_iter;
mod link;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
mod parser;
#[cfg(feature = "editor-facts")]
mod shape_value;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
mod source;
mod text;
mod tokens;

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub use accessibility::FlowchartAccessibility;
#[cfg(all(
    feature = "editor-facts",
    any(feature = "diagram-flowchart", feature = "diagram-swimlane")
))]
pub use accessibility::{FlowchartAccessibilityDirective, FlowchartAccessibilityStatement};
pub use ast::{
    ClassAssignStmt, ClassDefStmt, ClickAction, ClickStmt, LinkStylePos, LinkStyleStmt, StyleStmt,
};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub use ast::{
    EdgeAction, FlowchartAst, NodeAction, ShapeData, StatementEvent, Stmt, SubgraphBlock,
};
#[cfg(feature = "editor-facts")]
pub use ast::{FlowchartClickEditorEvidence, FlowchartDirectiveEditorEvidence};
use lexer::Lexer;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub use lexer::Lexer as FlowchartLexer;
#[cfg(feature = "editor-facts")]
pub use lexer::node_rename_text;
use link::{destruct_end_link, destruct_labeled_end_link, destruct_start_link};
use merman_model::families::flowchart::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub use parser::{FlowchartExpectedTokens, FlowchartSyntaxDiagnostic, parse_flowchart};
#[cfg(feature = "editor-facts")]
pub use shape_value::shape_value_expected_span;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub use text::trim_flowdb_label_text;
use text::{is_ecmascript_trim_char, parse_edge_label_text, parse_label_text};
pub use tokens::LexError as FlowchartLexicalError;
pub use tokens::{
    ArrowToken, DirectionStatementToken, LabeledText, LinkToken, NodeLabelToken, SubgraphHeader,
    TitleKind,
};
#[cfg(feature = "editor-facts")]
pub use tokens::{FlowchartExpectationKind, FlowchartLexicalExpectation};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub use tokens::{FlowchartLexicalEvent, Tok as FlowchartToken};
use tokens::{LexError, Tok};

/// The presentation statements whose lexical rules are shared by Flowchart and Agentflow.
#[cfg(feature = "diagram-agentflow")]
#[derive(Debug, Clone)]
pub enum PresentationStatement {
    Style(StyleStmt),
    ClassDef(ClassDefStmt),
    ClassAssign(ClassAssignStmt),
    Click(ClickStmt),
    LinkStyle(LinkStyleStmt),
}

/// Lexes a presentation statement using byte offsets relative to `source`.
///
/// The family applies the statement's effects in its own database replay.
#[cfg(feature = "diagram-agentflow")]
pub fn lex_presentation_statement(
    source: &str,
) -> Option<Result<(usize, PresentationStatement, usize), FlowchartLexicalError>> {
    let mut lexer = Lexer::new(source);
    let event = lexer
        .lex_style_stmt()
        .or_else(|| lexer.lex_classdef_stmt())
        .or_else(|| lexer.lex_class_assign_stmt())
        .or_else(|| lexer.lex_click_stmt())
        .or_else(|| lexer.lex_link_style_stmt())?;
    Some(event.map(|(start, token, end)| {
        let statement = match token {
            Tok::StyleStmt(style) => PresentationStatement::Style(style),
            Tok::ClassDefStmt(class) => PresentationStatement::ClassDef(class),
            Tok::ClassAssignStmt(class) => PresentationStatement::ClassAssign(class),
            Tok::ClickStmt(click) => PresentationStatement::Click(click),
            Tok::LinkStyleStmt(style) => PresentationStatement::LinkStyle(style),
            _ => unreachable!("presentation lexer returns only presentation tokens"),
        };
        (start, statement, end)
    }))
}

#[cfg(feature = "editor-facts")]
pub fn is_valid_editor_node_id(candidate: &str) -> bool {
    // A name is a fragment. The edited document supplies the actual following delimiter.
    let probe = [candidate, ";"].concat();
    let mut lexer = Lexer::new(&probe);
    matches!(
        (lexer.next(), lexer.next()),
        (Some(Ok((0, Tok::Id(_), end))), Some(Ok((_, Tok::Sep, _)))) if end == candidate.len()
    )
}
