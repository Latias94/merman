//! State's predictive grammar over spanned lexical events.

use super::{
    LexError, StateDocumentScratch, StateStatement, StateStatementNote, StateStatementState, Tok,
};
use crate::diagrams::scan::{TokenStream, is_ecmascript_whitespace};
use crate::parse_diagnostic::{unexpected_eof_message, unexpected_token_message};
use crate::{
    OperationCancelled, OperationControl, OperationControlResult, ParseDiagnostic,
    ParseDiagnosticSpanKind, SourceSpan,
};
use merman_model::families::state::{StateStatementClick, StateStatementRelation};

type Token = (usize, Tok, usize);
pub(super) type LexicalEvent = Result<Token, LexError>;

macro_rules! terminals {
    ($($variant:ident $(($payload:tt))? => $name:literal),+ $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        enum Terminal { $($variant),+ }

        const TERMINAL_NAMES: &[&str] = &[$($name),+];

        impl Terminal {
            const fn of(token: &Tok) -> Self {
                match token {
                    $(Tok::$variant $(($payload))? => Self::$variant),+
                }
            }
        }
    };
}

// Expected terminals appear in grammar order in public diagnostics.
terminals! {
    Newline => "Newline",
    Sd => "\"stateDiagram\"",
    Id(_) => "Id",
    StyledId(_) => "StyledId",
    EdgeState => "EdgeState",
    Descr(_) => "Descr",
    Arrow => "\"-->\"",
    StructStart => "\"{\"",
    StructStop => "\"}\"",
    As => "\"as\"",
    Note => "Note",
    LeftOf => "LeftOf",
    RightOf => "RightOf",
    NoteText(_) => "NoteTextTok",
    StateDescr(_) => "StateDescr",
    CompositState(_) => "CompositState",
    Fork(_) => "Fork",
    Join(_) => "Join",
    Choice(_) => "Choice",
    Concurrent => "Concurrent",
    HideEmptyDescription => "HideEmptyDescription",
    ScaleWidth => "ScaleWidth",
    ClassDef => "ClassDef",
    ClassDefId(_) => "ClassDefId",
    ClassDefStyleOpts(_) => "ClassDefStyleOpts",
    Class => "Class",
    ClassEntityIds(_) => "ClassEntityIds",
    StyleClass(_) => "StyleClass",
    Style => "Style",
    StyleIds(_) => "StyleIds",
    StyleDefStyleOpts(_) => "StyleDefStyleOpts",
    Direction(_) => "Direction",
    AccTitle(_) => "AccTitle",
    AccDescr(_) => "AccDescr",
    AccDescrMultiline(_) => "AccDescrMultiline",
    Click => "Click",
    Href => "Href",
    StringLit(_) => "StringLit",
}

#[derive(Clone, Copy, Default)]
struct Expected(u64);

impl Expected {
    const fn new(terminals: &[Terminal]) -> Self {
        let mut expected = Self(0);
        let mut index = 0;
        while index < terminals.len() {
            expected = expected.with(terminals[index]);
            index += 1;
        }
        expected
    }

    const fn with(self, terminal: Terminal) -> Self {
        Self(self.0 | (1 << terminal as u8))
    }

    const fn contains(self, terminal: Terminal) -> bool {
        self.0 & (1 << terminal as u8) != 0
    }

    fn names(self) -> impl Iterator<Item = &'static str> {
        TERMINAL_NAMES
            .iter()
            .copied()
            .enumerate()
            .filter_map(move |(index, name)| (self.0 & (1 << index) != 0).then_some(name))
    }
}

use Terminal::*;

const PRELUDE: Expected = Expected::new(&[Newline, Sd]);
const STATE_ID: Expected = Expected::new(&[Id, StyledId, EdgeState]);
const ITEM: Expected = Expected::new(&[
    Newline,
    Id,
    StyledId,
    EdgeState,
    Note,
    StateDescr,
    CompositState,
    Fork,
    Join,
    Choice,
    Concurrent,
    HideEmptyDescription,
    ScaleWidth,
    ClassDef,
    Class,
    Style,
    Direction,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Click,
]);

enum Failure {
    Syntax(ParseDiagnostic),
    Cancelled(OperationCancelled),
}

impl From<OperationCancelled> for Failure {
    fn from(cancelled: OperationCancelled) -> Self {
        Self::Cancelled(cancelled)
    }
}

type ParseResult<T> = Result<T, Failure>;

pub(super) fn parse(
    events: impl Iterator<Item = LexicalEvent>,
    source_len: usize,
    control: &OperationControl,
) -> OperationControlResult<Result<StateDocumentScratch, ParseDiagnostic>> {
    let outcome = Parser {
        input: TokenStream::new(events, control),
        source_len,
    }
    .document();
    control.checkpoint()?;
    match outcome {
        Ok(document) => Ok(Ok(document)),
        Err(Failure::Syntax(diagnostic)) => Ok(Err(diagnostic)),
        Err(Failure::Cancelled(cancelled)) => Err(cancelled),
    }
}

struct BlockFrame {
    state: StateStatementState,
    outer: Vec<StateStatement>,
}

struct Parser<'a, I: Iterator> {
    input: TokenStream<'a, I>,
    source_len: usize,
}

impl<I: Iterator<Item = LexicalEvent>> Parser<'_, I> {
    fn document(&mut self) -> ParseResult<StateDocumentScratch> {
        loop {
            match self.bump()? {
                Some((_, Tok::Sd, _)) => break,
                Some((_, Tok::Newline, _)) => {}
                token => return Err(self.unexpected(token, PRELUDE)),
            }
        }

        let mut document = StateDocumentScratch::default();
        let mut items = Vec::new();
        let mut frames: Vec<BlockFrame> = Vec::new();
        let mut expected = ITEM;
        let mut at_list_start = true;

        loop {
            let Some((start, token, end)) = self.bump()? else {
                if !frames.is_empty() {
                    // After a completed item, EOF diagnoses the missing close. An empty
                    // open block still admits its first item as well.
                    if at_list_start {
                        return Err(self.unexpected(None, expected));
                    }
                    return Err(self.unexpected(None, Expected::new(&[StructStop])));
                }
                document.root = Some(document.push(items));
                return Ok(document);
            };
            let starts_item = if frames.is_empty() {
                ITEM
            } else {
                ITEM.with(StructStop)
            };
            let previous = std::mem::replace(&mut expected, starts_item);
            let was_at_list_start = std::mem::replace(&mut at_list_start, false);
            let span = SourceSpan::new(start, end);
            let statement = match token {
                Tok::Newline => continue,
                Tok::StructStop => {
                    let Some(BlockFrame { mut state, outer }) = frames.pop() else {
                        // At the root, a close after a completed item is an extra token.
                        let expected = if was_at_list_start {
                            ITEM
                        } else {
                            Expected::default()
                        };
                        return Err(self.unexpected(Some((start, token, end)), expected));
                    };
                    state.doc = Some(document.push(items));
                    items = outer;
                    if frames.is_empty() {
                        expected = ITEM;
                    }
                    StateStatement::State(state)
                }
                Tok::Id(_) | Tok::StyledId(_) | Tok::EdgeState => {
                    let mut state = state_id(start, token, end);
                    if self.accept(Arrow)? {
                        let (start, token, end) = self.require(STATE_ID)?;
                        let state2 = state_id(start, token, end);
                        let description = self.description()?;
                        if description.is_none() {
                            expected = expected.with(Descr);
                        }
                        StateStatement::Relation(Box::new(StateStatementRelation {
                            state1: state,
                            state2,
                            description,
                        }))
                    } else {
                        state.description = self.description()?;
                        if state.description.is_none() {
                            expected = expected.with(Arrow).with(Descr);
                        }
                        StateStatement::State(state)
                    }
                }
                Tok::CompositState(id) => {
                    if self.accept(StructStart)? {
                        let mut state = StateStatementState::new(id);
                        state.id_span = Some(span);
                        frames.push(BlockFrame {
                            state,
                            outer: std::mem::take(&mut items),
                        });
                        expected = ITEM.with(StructStop);
                        at_list_start = true;
                        continue;
                    }
                    expected = expected.with(StructStart);
                    StateStatement::Noop
                }
                Tok::StateDescr(description) => {
                    self.require(Expected::new(&[As]))?;
                    let (start, Tok::Id(id), _) = self.require(Expected::new(&[Id]))? else {
                        unreachable!("State aliases require a plain identifier")
                    };
                    let composite = self.accept(StructStart)?;
                    let state = alias(start, id, description, composite);
                    if composite {
                        frames.push(BlockFrame {
                            state,
                            outer: std::mem::take(&mut items),
                        });
                        expected = ITEM.with(StructStop);
                        at_list_start = true;
                        continue;
                    }
                    expected = expected.with(StructStart);
                    StateStatement::State(state)
                }
                Tok::Fork(id) => typed_state(start, id, "fork"),
                Tok::Join(id) => typed_state(start, id, "join"),
                Tok::Choice(id) => typed_state(start, id, "choice"),
                Tok::Concurrent => StateStatement::State(StateStatementState::new_typed(
                    "__divider__".to_string(),
                    "divider",
                )),
                Tok::Note => self.note()?,
                Tok::Click => self.click()?,
                Tok::HideEmptyDescription | Tok::ScaleWidth => StateStatement::Noop,
                Tok::ClassDef => StateStatement::ClassDef {
                    id: self.text(ClassDefId)?,
                    classes: self.text(ClassDefStyleOpts)?,
                },
                Tok::Style => StateStatement::Style {
                    ids: self.text(StyleIds)?,
                    styles: self.text(StyleDefStyleOpts)?,
                },
                Tok::Class => StateStatement::ApplyClass {
                    ids: self.text(ClassEntityIds)?,
                    class_name: self.text(StyleClass)?,
                },
                Tok::Direction(direction) => StateStatement::Direction(direction.value),
                Tok::AccTitle(title) => StateStatement::AccTitle(title),
                Tok::AccDescr(text) | Tok::AccDescrMultiline(text) => {
                    StateStatement::AccDescr(text)
                }
                _ => return Err(self.unexpected(Some((start, token, end)), previous)),
            };
            items.push(statement);
        }
    }

    fn note(&mut self) -> ParseResult<StateStatement> {
        let position = match self.require(Expected::new(&[LeftOf, RightOf, NoteText]))?.1 {
            Tok::LeftOf => "left of",
            Tok::RightOf => "right of",
            Tok::NoteText(_) => {
                // Mermaid admits floating note syntax without adding a note to the model.
                self.require(Expected::new(&[As]))?;
                self.require(Expected::new(&[Id]))?;
                return Ok(StateStatement::Noop);
            }
            _ => unreachable!("State note position or floating text"),
        };
        let (start, token, end) = self.require(Expected::new(&[Id]))?;
        let mut state = state_id(start, token, end);
        state.note = Some(StateStatementNote {
            position: Some(position.to_string()),
            text: self.text(NoteText)?,
        });
        Ok(StateStatement::State(state))
    }

    fn click(&mut self) -> ParseResult<StateStatement> {
        let (start, token, end) = self.require(STATE_ID)?;
        let id = state_id(start, token, end).id;
        let (url, tooltip) = match self.require(Expected::new(&[Href, StringLit]))?.1 {
            Tok::Href => (self.text(StringLit)?, String::new()),
            Tok::StringLit(url) => (url, self.text(StringLit)?),
            _ => unreachable!("State click link or href keyword"),
        };
        Ok(StateStatement::Click(StateStatementClick {
            id,
            url,
            tooltip,
        }))
    }

    fn description(&mut self) -> ParseResult<Option<String>> {
        if self.peek()? == Some(Descr) {
            self.text(Descr).map(Some)
        } else {
            Ok(None)
        }
    }

    fn text(&mut self, terminal: Terminal) -> ParseResult<String> {
        match self.require(Expected::new(&[terminal]))?.1 {
            Tok::Descr(text)
            | Tok::NoteText(text)
            | Tok::ClassDefId(text)
            | Tok::ClassDefStyleOpts(text)
            | Tok::ClassEntityIds(text)
            | Tok::StyleClass(text)
            | Tok::StyleIds(text)
            | Tok::StyleDefStyleOpts(text)
            | Tok::StringLit(text) => Ok(text),
            _ => unreachable!("State text expectations accept only string tokens"),
        }
    }

    fn peek(&mut self) -> ParseResult<Option<Terminal>> {
        match self.input.peek()? {
            Some(Ok((_, token, _))) => Ok(Some(Terminal::of(token))),
            Some(Err(error)) => Err(lexical_error(error.clone(), self.source_len)),
            None => Ok(None),
        }
    }

    fn accept(&mut self, terminal: Terminal) -> ParseResult<bool> {
        let accepted = self.peek()? == Some(terminal);
        if accepted {
            self.bump()?;
        }
        Ok(accepted)
    }

    fn bump(&mut self) -> ParseResult<Option<Token>> {
        self.input
            .bump()?
            .transpose()
            .map_err(|error| lexical_error(error, self.source_len))
    }

    fn require(&mut self, expected: Expected) -> ParseResult<Token> {
        match self.bump()? {
            Some(token) if expected.contains(Terminal::of(&token.1)) => Ok(token),
            token => Err(self.unexpected(token, expected)),
        }
    }

    fn unexpected(&self, token: Option<Token>, expected: Expected) -> Failure {
        use ParseDiagnosticSpanKind::{Exact, InsertionPoint};

        let diagnostic = match token {
            Some((start, token, end)) => {
                let kind =
                    if matches!(token, Tok::Newline) && start == end && end == self.source_len {
                        InsertionPoint
                    } else {
                        Exact
                    };
                ParseDiagnostic::new(unexpected_token_message(&token, expected.names()))
                    .with_span(SourceSpan::new(start, end), kind)
            }
            None => {
                let end = self.input.previous_end();
                ParseDiagnostic::new(unexpected_eof_message(expected.names()))
                    .with_span(SourceSpan::new(end, end), InsertionPoint)
            }
        };
        Failure::Syntax(diagnostic)
    }
}

fn lexical_error(error: LexError, source_len: usize) -> Failure {
    let (span, kind) = match error.span {
        Some(span) => (span, ParseDiagnosticSpanKind::Exact),
        None => (
            SourceSpan::new(source_len, source_len),
            ParseDiagnosticSpanKind::Fallback,
        ),
    };
    Failure::Syntax(ParseDiagnostic::new(error.message).with_span(span, kind))
}

fn state_id(start: usize, token: Tok, end: usize) -> StateStatementState {
    let (id, class) = match token {
        Tok::Id(id) => (id, None),
        Tok::EdgeState => ("[*]".to_string(), None),
        Tok::StyledId((id, class)) => (id, Some(class)),
        _ => unreachable!("State identifier tokens"),
    };
    let mut state = StateStatementState::new(id);
    state.id_span = Some(SourceSpan::new(start, end));
    if let Some(class) = class {
        state.id_span = Some(SourceSpan::new(start, start + state.id.len()));
        state.classes.push(class);
    }
    state
}

fn typed_state(start: usize, id: String, kind: &str) -> StateStatement {
    let mut state = StateStatementState::new_typed(id, kind);
    state.id_span = Some(SourceSpan::new(start, start + state.id.len()));
    StateStatement::State(state)
}

fn alias(start: usize, id: String, description: String, composite: bool) -> StateStatementState {
    let mut state = StateStatementState::new(id);
    state.description = Some(
        description
            .trim_matches(is_ecmascript_whitespace)
            .to_string(),
    );
    // The plain alias can annotate its identifier with a colon; a composite keeps the full ID.
    if !composite {
        let mut parts = state.id.split(':');
        let name = parts.next().unwrap_or_default();
        if let Some(extra) = parts.next() {
            let extra = extra.trim_matches(is_ecmascript_whitespace);
            if !extra.is_empty() {
                state.descriptions.push(extra.to_string());
            }
            state.id.truncate(name.len());
        }
    }
    state.id_span = Some(SourceSpan::new(start, start + state.id.len()));
    state
}
