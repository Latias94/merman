//! State syntax, database replay, and model construction.

mod db;
mod document;
mod lexer;
mod parser;
#[cfg(feature = "presentation")]
mod render_model;

use crate::scan::collect_lexical_events;
use lexer::Lexer;
use merman_model::families::state::{StateDocumentBuilder, StateDocumentId};
use merman_model::{OperationControl, OperationControlResult};

pub use db::{StateModelError, StateSource};
pub use lexer::{StateDirection, StateLexicalError, StateLexicalEvent, StateToken};
pub use parser::{StateExpectedTokens, StateSyntaxDiagnostic};

/// Flat statement construction data for the State database to transform and validate.
#[derive(Debug)]
pub struct StateParsedDocument {
    pub builder: StateDocumentBuilder,
    pub root: StateDocumentId,
}

/// A single lexed State input, including events after recoverable lexer errors.
pub struct StateSyntax {
    events: Vec<StateLexicalEvent>,
    source_len: usize,
}

impl StateSyntax {
    /// Lexes State input, checking the same operation as subsequent parsing.
    pub fn lex(code: &str, control: &OperationControl) -> OperationControlResult<Self> {
        let events = collect_lexical_events(&mut Lexer::new(code), control)?;
        Ok(Self {
            events,
            source_len: code.len(),
        })
    }

    /// Exposes the original event stream for editor projection before grammar validation.
    #[cfg(feature = "editor-facts")]
    pub fn events(&self) -> &[StateLexicalEvent] {
        &self.events
    }

    /// Consumes the events and returns the flat documents and root allocated by the grammar.
    pub fn into_document(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<StateParsedDocument, StateSyntaxDiagnostic>> {
        control.checkpoint()?;
        parser::parse(self.events.into_iter(), self.source_len, control)
    }

    /// Consumes the same syntax and constructs the database after grammar validation.
    pub fn into_source(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<StateSource, StateSyntaxDiagnostic>> {
        match self.into_document(control)? {
            Ok(document) => Ok(Ok(StateSource::from_document(document, control)?)),
            Err(diagnostic) => Ok(Err(diagnostic)),
        }
    }
}

#[cfg(test)]
mod tests;
