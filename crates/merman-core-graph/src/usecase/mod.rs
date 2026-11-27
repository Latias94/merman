//! Native Mermaid 12 Usecase grammar and transactional semantic construction.
//!
//! Grammar, declaration merging, and deferred resolution follow the pinned upstream
//! `usecase.parser.ts`, `usecase.visitor.ts`, and `usecaseModelBuilder.ts`.

use merman_model::families::usecase::{
    UsecaseActorType, UsecaseAnimation, UsecaseArrowType, UsecaseBoundary, UsecaseClassDef,
    UsecaseDiagramModel, UsecaseJsonInfinity, UsecaseJsonNode, UsecaseJsonStringEncoding,
    UsecaseLabelType, UsecaseNode, UsecaseNodeKind, UsecaseNote, UsecaseRelationship,
    UsecaseRelationshipType,
};
use merman_model::{OperationControl, OperationControlResult, SourceSpan};
use serde_json::Value;

#[cfg(feature = "editor-facts")]
mod editor;
mod json_string;
mod lexer;
mod ordered_json;
mod parse;
mod resolve;

#[cfg(feature = "editor-facts")]
pub use editor::{
    UsecaseBinding, UsecaseBindings, UsecaseEditorObserver, UsecaseOccurrence,
    UsecaseOccurrenceKind, UsecaseOccurrenceRole,
};

/// Whether an unquoted spelling is one complete identifier in the pinned grammar.
#[cfg(feature = "editor-facts")]
pub fn is_valid_editor_identifier(candidate: &str) -> bool {
    if candidate.is_empty()
        || is_forbidden_statement_identifier(candidate)
        || !candidate
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return false;
    }
    let Ok(Ok(tokens)) = lexer::lex(candidate, &OperationControl::new()) else {
        return false;
    };
    tokens.len() == 2 && tokens[0].kind == lexer::TokenKind::Identifier
}

fn is_forbidden_statement_identifier(candidate: &str) -> bool {
    [
        "allowmixing",
        "newpage",
        "package",
        "rectangle",
        "skinparam",
    ]
    .iter()
    .any(|keyword| candidate.eq_ignore_ascii_case(keyword))
}

#[derive(Debug)]
pub struct ParseIssue {
    pub message: String,
    pub span: SourceSpan,
}

impl ParseIssue {
    fn new(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

/// Parses one preprocessed source after the caller's entry checkpoint.
///
/// Occurrences and issues refer to byte offsets in `code`. Rich observation starts after
/// lexing and resolves names from the completed portion of the draft before model construction.
#[cfg_attr(
    not(feature = "editor-facts"),
    allow(
        clippy::needless_lifetimes,
        reason = "The editor observer shares the parser borrow when enabled."
    )
)]
pub fn parse_usecase_source_controlled<'a>(
    code: &'a str,
    title: Option<&str>,
    #[cfg(feature = "editor-facts")] observer: Option<&'a mut dyn UsecaseEditorObserver>,
    control: &'a OperationControl,
) -> OperationControlResult<Result<UsecaseDiagramModel, ParseIssue>> {
    let tokens = match lexer::lex(code, control)? {
        Ok(tokens) => tokens,
        Err(issue) => return Ok(Err(issue)),
    };
    let mut parser = parse::Parser::new(
        code,
        tokens,
        #[cfg(feature = "editor-facts")]
        observer,
        control,
    );
    let parsed = parser.parse()?;
    #[cfg(feature = "editor-facts")]
    parser.finalize_editor_facts()?;
    if let Err(issue) = parsed {
        return Ok(Err(issue));
    }
    resolve::resolve(&parser.draft, title, control)
}
