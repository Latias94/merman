//! Class syntax, database replay, and canonical model construction.

mod ast;
mod db;
mod lexer;
mod parser;

use crate::scan::collect_lexical_events;
use ast::ActionArena;
use db::ClassDb;
use lexer::{Lexer, Tok};
use merman_model::{
    MermaidConfig, OperationControl, OperationControlResult, families::class::ClassDiagramModel,
    warnings::DiagramWarningFact,
};

pub use ast::{
    Action as ClassAction, Relation as ClassRelation, RelationData as ClassRelationData,
};
pub use lexer::{ClassLexicalEvent, LexError as ClassLexicalError, Tok as ClassToken};
#[cfg(feature = "editor-facts")]
pub use lexer::{
    ClassLexicalExpectation, editor_name_needs_separator, editor_name_requires_backticks,
};
pub use parser::{ClassExpectedTokens, ClassSyntaxDiagnostic};

/// A Class grammar failure in preprocessed source coordinates or a database failure.
#[derive(Debug)]
pub enum ClassModelError {
    Grammar(ClassSyntaxDiagnostic),
    Db(String),
}

/// A normalized lexical stream, including events after recoverable lexical errors.
pub struct ClassSyntax {
    events: Vec<ClassLexicalEvent>,
}

impl ClassSyntax {
    /// Lexes once, normalizing adjacent names before grammar and editor use.
    pub fn lex(code: &str, control: &OperationControl) -> OperationControlResult<Self> {
        let events = collect_lexical_events(&mut Lexer::new(code), control)?;
        let mut combined: Vec<ClassLexicalEvent> = Vec::with_capacity(events.len());
        let mut can_prefix_name = false;
        let mut in_annotation = false;
        for (index, event) in events.into_iter().enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            let next_can_prefix = !in_annotation
                && match &event {
                    Ok((start, Tok::Name(_), end)) => !code[*start..*end].contains(['`', '~']),
                    _ => false,
                };
            match &event {
                Ok((_, Tok::AnnotationStart, _)) => in_annotation = true,
                Ok((_, Tok::AnnotationStop | Tok::Newline, _)) => in_annotation = false,
                _ => {}
            }
            // Only alphaNumToken can prefix another className. A quoted or generic name
            // completes the production; annotations retain their single terminal as well.
            match (combined.last_mut(), event) {
                (
                    Some(Ok((_, Tok::Name(previous), previous_end))),
                    Ok((_, Tok::Name(next), end)),
                ) if can_prefix_name => {
                    previous.push_str(&next);
                    *previous_end = end;
                }
                (_, event) => combined.push(event),
            }
            can_prefix_name = next_can_prefix;
        }
        control.checkpoint()?;
        Ok(Self { events: combined })
    }

    /// Borrows the normalized events for editor projection before grammar validation.
    #[cfg(feature = "editor-facts")]
    pub fn events(&self) -> &[ClassLexicalEvent] {
        &self.events
    }

    /// Consumes the observed syntax, validates the grammar, and replays the database actions.
    /// A warning sink requests ignored-statement warnings and retains them in replay order.
    pub fn into_model(
        self,
        config: &MermaidConfig,
        diagram_type: &str,
        mut warning_facts: Option<&mut Vec<DiagramWarningFact>>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ClassDiagramModel, ClassModelError>> {
        let actions = match self.into_actions(warning_facts.is_some(), control)? {
            Ok(actions) => actions,
            Err(diagnostic) => return Ok(Err(ClassModelError::Grammar(diagnostic))),
        };

        let mut db = ClassDb::new(config);
        for (index, action) in actions.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Err(message) = db.apply(action, warning_facts.as_deref_mut()) {
                return Ok(Err(ClassModelError::Db(message)));
            }
        }

        control.checkpoint()?;
        Ok(Ok(db.into_semantic_model(diagram_type)))
    }

    /// Constructs the flat actions consumed by the Class database.
    ///
    /// The caller requests ignored-statement spans when collecting warnings.
    pub fn into_actions(
        self,
        capture_ignored_statements: bool,
        control: &OperationControl,
    ) -> OperationControlResult<
        Result<impl Iterator<Item = ClassAction> + use<>, ClassSyntaxDiagnostic>,
    > {
        let mut arena = ActionArena::default();
        arena.capture_ignored_statements = capture_ignored_statements;
        Ok(parser::parse(&mut arena, self.events.into_iter(), control)?
            .map(|actions| arena.into_actions(actions)))
    }
}

#[cfg(test)]
mod tests;
