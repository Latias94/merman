//! State syntax and flat statement construction.

mod lexer;
mod parser;

use crate::scan::collect_lexical_events;
use lexer::Lexer;
use merman_model::families::state::{StateDocumentBuilder, StateDocumentId};
use merman_model::{OperationControl, OperationControlResult};

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
}

#[cfg(test)]
mod tests;
