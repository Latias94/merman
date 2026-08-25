use crate::SourceSpan;

mod actor;
mod scanner;

use scanner::SequenceScanner;

#[cfg(test)]
use actor::HALF_ARROW_TYPES;

#[derive(Debug, Clone)]
pub(crate) enum Tok {
    Newline,

    SequenceDiagram,
    Participant,
    ActorKw,
    Create,
    Destroy,
    As,

    Box,
    End,

    Loop,
    Rect,
    Opt,
    Alt,
    Else,
    Par,
    ParOver,
    And,
    Critical,
    Option,
    Break,

    Note,
    LeftOf,
    RightOf,
    Over,

    Links,
    Link,
    Properties,
    Details,

    Autonumber,
    Off,

    Activate,
    Deactivate,

    Comma,
    Plus,
    Minus,
    Central,

    Num(f64),
    Actor(String),
    Text(String),
    RestOfLine(String),
    SignalType(i32),
    Config(String),

    Title(String),
    CompatTitle(String),
    AccTitle(String),
    AccDescr(String),
    AccDescrMultiline(String),
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub(crate) struct LexError {
    pub message: String,
    pub span: SourceSpan,
}

impl crate::error::ParseErrorSourceSpan for LexError {
    fn source_span(&self) -> Option<crate::SourceSpan> {
        Some(self.span)
    }
}

pub(super) struct Lexer<'input> {
    scanner: SequenceScanner<'input>,
}

impl<'input> Lexer<'input> {
    pub(super) fn new(input: &'input str) -> Self {
        Self {
            scanner: SequenceScanner::new(input),
        }
    }

    pub(super) fn position(&self) -> usize {
        self.scanner.position()
    }
}

impl Iterator for Lexer<'_> {
    type Item = std::result::Result<(usize, Tok, usize), LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.scanner.next_token()
    }
}

#[cfg(test)]
mod tests {
    use super::{HALF_ARROW_TYPES, Lexer, Tok};

    fn token_trace(input: &str) -> Vec<(usize, &'static str, usize)> {
        Lexer::new(input)
            .map(|event| {
                let (start, token, end) = event.expect("sequence token");
                let name = match token {
                    Tok::AccDescrMultiline(_) => "accDescrMultiline",
                    Tok::Newline => "newline",
                    Tok::Participant => "participant",
                    _ => "other",
                };
                (start, name, end)
            })
            .collect()
    }

    fn boundary_after_accessibility(input: &str) -> (usize, &'static str, usize) {
        let trace = token_trace(input);
        let accessibility = trace
            .iter()
            .position(|(_, name, _)| *name == "accDescrMultiline")
            .expect("multiline accessibility token");
        trace[accessibility + 1]
    }

    #[test]
    fn multiline_accessibility_only_synthesizes_required_statement_boundaries() {
        let ordinary = "sequenceDiagram\naccDescr {desc}\nparticipant A";
        let physical_newline = ordinary.find("}\n").expect("closing brace") + 1;
        assert_eq!(
            boundary_after_accessibility(ordinary),
            (physical_newline, "newline", physical_newline + 1)
        );

        let same_line = "sequenceDiagram\naccDescr {desc} participant A";
        let closing = same_line.find('}').expect("closing brace") + 1;
        assert_eq!(
            boundary_after_accessibility(same_line),
            (closing, "newline", closing)
        );

        let eof = "sequenceDiagram\naccDescr {desc}";
        assert_eq!(
            boundary_after_accessibility(eof),
            (eof.len(), "newline", eof.len())
        );
    }

    #[test]
    fn lexes_all_upstream_half_arrow_variants() {
        for (arrow, expected_type) in HALF_ARROW_TYPES {
            for input in [
                format!("A {arrow} B: message"),
                format!("A{arrow}B: message"),
                format!("顧客{arrow}サーバー: message"),
            ] {
                let signal_types: Vec<_> = Lexer::new(&input)
                    .map(|event| event.expect("sequence token").1)
                    .filter_map(|token| match token {
                        Tok::SignalType(signal_type) => Some(signal_type),
                        _ => None,
                    })
                    .collect();

                assert_eq!(signal_types, vec![expected_type], "{input}");
            }
        }
    }
}
