//! ZenUML grammar, participant resolution, and semantic model construction.
//!
//! The companion's `sequenceLexer.g4` and `sequenceParser.g4` define the syntax.

mod ast;
mod lexer;
mod parser;
mod semantic;

use merman_model::families::zenuml::ZenumlDiagramModel;
use merman_model::{OperationControl, OperationControlResult};

pub use ast::SyntaxDiagnostic;
#[cfg(feature = "editor-facts")]
pub use editor::{OccurrenceKind, SourceEvent, SourceObserver};

pub struct ParsedZenuml {
    pub model: ZenumlDiagramModel,
    pub first_diagnostic: Option<SyntaxDiagnostic>,
}

pub fn parse_source_controlled<'a>(
    code: &str,
    #[cfg(feature = "editor-facts")] observer: Option<SourceObserver<'a>>,
    control: &'a OperationControl,
) -> OperationControlResult<ParsedZenuml> {
    control.checkpoint()?;
    let tokens = lexer::lex_controlled(code, control)?;
    let parsed = parser::parse_controlled(code, &tokens, control)?;
    control.checkpoint()?;
    let built = semantic::build_controlled(
        parsed,
        #[cfg(feature = "editor-facts")]
        observer,
        control,
    )?;
    control.checkpoint()?;
    Ok(ParsedZenuml {
        model: built.model,
        first_diagnostic: built.diagnostics.into_iter().next(),
    })
}

#[cfg(feature = "editor-facts")]
mod editor {
    use merman_model::SourceSpan;

    use super::SyntaxDiagnostic;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum OccurrenceKind {
        Participant,
        ParticipantReference,
        Group,
        String,
        Class,
        Variable,
        Function,
    }

    pub enum SourceEvent<'a> {
        TitleDirective,
        Occurrence {
            kind: OccurrenceKind,
            text: &'a str,
            detail: &'a str,
            span: SourceSpan,
        },
        Diagnostic(&'a SyntaxDiagnostic),
    }

    pub struct SourceObserver<'a> {
        capture: bool,
        event: &'a mut dyn FnMut(SourceEvent<'_>),
    }

    impl<'a> SourceObserver<'a> {
        pub fn new(capture: bool, event: &'a mut dyn FnMut(SourceEvent<'_>)) -> Self {
            Self { capture, event }
        }

        pub(super) fn diagnostic(&mut self, diagnostic: &SyntaxDiagnostic) {
            (self.event)(SourceEvent::Diagnostic(diagnostic));
        }

        pub(super) fn capture(&mut self, event: SourceEvent<'_>) {
            if self.capture {
                (self.event)(event);
            }
        }

        pub(super) fn captures_rich(&self) -> bool {
            self.capture
        }
    }
}
