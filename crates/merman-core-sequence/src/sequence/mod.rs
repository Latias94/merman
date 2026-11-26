//! Sequence lexical syntax and grammar actions.

mod ast;
mod lexer;
mod parser;

use lexer::Lexer;
use merman_core_preprocess::scan::collect_lexical_events;
use merman_model::{OperationControl, OperationControlResult};

pub use ast::Action as SequenceAction;
pub use lexer::{LexError as SequenceLexicalError, SequenceLexicalEvent, Tok as SequenceToken};
pub use parser::{SequenceExpectedTokens, SequenceSyntaxDiagnostic};

/// A single lexical stream, including events after recoverable lexer errors.
pub struct SequenceSyntax {
    events: Vec<SequenceLexicalEvent>,
}

impl SequenceSyntax {
    /// Collects the events used by grammar validation and editor projection.
    pub fn lex(code: &str, control: &OperationControl) -> OperationControlResult<Self> {
        let events = collect_lexical_events(&mut Lexer::new(code), control)?;
        Ok(Self { events })
    }

    /// Borrows the events for editor projection before grammar validation.
    #[cfg(feature = "editor-facts")]
    pub fn events(&self) -> &[SequenceLexicalEvent] {
        &self.events
    }

    /// Consumes the lexical stream and returns the flat actions for database replay.
    pub fn into_actions(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Vec<SequenceAction>, SequenceSyntaxDiagnostic>> {
        control.checkpoint()?;
        parser::parse(self.events.into_iter(), control)
    }
}

#[cfg(test)]
mod tests;
