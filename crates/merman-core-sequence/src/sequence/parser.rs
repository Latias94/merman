//! Predictive parser over the Sequence lexer's spanned events.

use super::ast::Action;
use super::lexer::{LexError, SequenceLexicalEvent as LexicalEvent, Tok};
use merman_core_preprocess::scan::TokenStream;
use merman_model::families::sequence::{
    LINETYPE_ALT_ELSE, LINETYPE_ALT_END, LINETYPE_ALT_START, LINETYPE_BREAK_END,
    LINETYPE_BREAK_START, LINETYPE_CENTRAL_CONNECTION, LINETYPE_CENTRAL_CONNECTION_DUAL,
    LINETYPE_CENTRAL_CONNECTION_REVERSE, LINETYPE_CRITICAL_END, LINETYPE_CRITICAL_OPTION,
    LINETYPE_CRITICAL_START, LINETYPE_LOOP_END, LINETYPE_LOOP_START, LINETYPE_OPT_END,
    LINETYPE_OPT_START, LINETYPE_PAR_AND, LINETYPE_PAR_END, LINETYPE_PAR_OVER_START,
    LINETYPE_PAR_START, LINETYPE_RECT_END, LINETYPE_RECT_START, PLACEMENT_LEFT_OF, PLACEMENT_OVER,
    PLACEMENT_RIGHT_OF,
};
use merman_model::{OperationCancelled, OperationControl, OperationControlResult, SourceSpan};

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

// Keep expected terminal names in the original grammar's diagnostic order.
terminals! {
    Newline => "Newline",
    SequenceDiagram => "\"sequenceDiagram\"",
    Participant => "\"participant\"",
    ActorKw => "\"actor\"",
    Create => "\"create\"",
    Destroy => "\"destroy\"",
    As => "\"as\"",
    Box => "\"box\"",
    End => "\"end\"",
    Loop => "\"loop\"",
    Rect => "\"rect\"",
    Opt => "\"opt\"",
    Alt => "\"alt\"",
    Else => "\"else\"",
    Par => "\"par\"",
    ParOver => "\"par_over\"",
    And => "\"and\"",
    Critical => "\"critical\"",
    Option => "\"option\"",
    Break => "\"break\"",
    Note => "\"note\"",
    LeftOf => "\"left_of\"",
    RightOf => "\"right_of\"",
    Over => "\"over\"",
    Links => "\"links\"",
    Link => "\"link\"",
    Properties => "\"properties\"",
    Details => "\"details\"",
    Autonumber => "\"autonumber\"",
    Off => "\"off\"",
    Activate => "\"activate\"",
    Deactivate => "\"deactivate\"",
    Comma => "Comma",
    Plus => "Plus",
    Minus => "Minus",
    Central => "Central",
    Num(_) => "Num",
    Actor(_) => "Actor",
    Text(_) => "Text",
    RestOfLine(_) => "RestOfLine",
    SignalType(_) => "SignalType",
    Config(_) => "Config",
    Title(_) => "Title",
    CompatTitle(_) => "CompatTitle",
    AccTitle(_) => "AccTitle",
    AccDescr(_) => "AccDescr",
    AccDescrMultiline(_) => "AccDescrMultiline",
}

use Terminal::{
    AccDescr, AccDescrMultiline, AccTitle, Activate, Actor, ActorKw, Alt, And, As, Autonumber, Box,
    Break, Central, Comma, CompatTitle, Config, Create, Critical, Deactivate, Destroy, Details,
    Else, End, LeftOf, Link, Links, Loop, Minus, Newline, Note, Num, Off, Opt,
    Option as OptionBranch, Over, Par, ParOver, Participant, Plus, Properties, Rect, RestOfLine,
    RightOf, SequenceDiagram, SignalType, Text, Title,
};

/// The terminals accepted at a grammar error, in diagnostic order.
#[derive(Debug, Clone, Copy, Default)]
pub struct SequenceExpectedTokens(u64);

type Expected = SequenceExpectedTokens;

impl SequenceExpectedTokens {
    const fn one(terminal: Terminal) -> Self {
        Self(1 << terminal as u8)
    }

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
        self.union(Self::one(terminal))
    }

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    const fn without(self, terminal: Terminal) -> Self {
        Self(self.0 & !Self::one(terminal).0)
    }

    const fn contains(self, terminal: Terminal) -> bool {
        self.0 & Self::one(terminal).0 != 0
    }

    /// Iterates the original grammar's terminal names.
    pub fn names(self) -> impl Iterator<Item = &'static str> {
        TERMINAL_NAMES
            .iter()
            .copied()
            .enumerate()
            .filter_map(move |(index, name)| (self.0 & (1 << index) != 0).then_some(name))
    }
}

const DOCUMENT: Expected = Expected::new(&[
    Newline,
    Participant,
    ActorKw,
    Create,
    Destroy,
    Box,
    Loop,
    Rect,
    Opt,
    Alt,
    Par,
    ParOver,
    Critical,
    Break,
    Note,
    Links,
    Link,
    Properties,
    Details,
    Autonumber,
    Activate,
    Deactivate,
    Actor,
    Title,
    CompatTitle,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
]);
const BOX_BODY: Expected = Expected::new(&[Newline, Participant, ActorKw, Destroy, End]);
const DECLARATION: Expected = Expected::new(&[Participant, ActorKw]);

#[derive(Clone, Copy)]
struct Fragment {
    start: i32,
    end: i32,
    branch: Option<(Terminal, i32)>,
}

impl Fragment {
    const fn new(start: i32, end: i32) -> Self {
        Self {
            start,
            end,
            branch: None,
        }
    }

    const fn branched(start: i32, end: i32, branch: Terminal, signal_type: i32) -> Self {
        Self {
            start,
            end,
            branch: Some((branch, signal_type)),
        }
    }

    const fn after_head(head: Terminal) -> Self {
        match head {
            Loop => Self::new(LINETYPE_LOOP_START, LINETYPE_LOOP_END),
            Rect => Self::new(LINETYPE_RECT_START, LINETYPE_RECT_END),
            Opt => Self::new(LINETYPE_OPT_START, LINETYPE_OPT_END),
            Break => Self::new(LINETYPE_BREAK_START, LINETYPE_BREAK_END),
            Alt => Self::branched(
                LINETYPE_ALT_START,
                LINETYPE_ALT_END,
                Else,
                LINETYPE_ALT_ELSE,
            ),
            Par => Self::branched(LINETYPE_PAR_START, LINETYPE_PAR_END, And, LINETYPE_PAR_AND),
            ParOver => Self::branched(
                LINETYPE_PAR_OVER_START,
                LINETYPE_PAR_END,
                And,
                LINETYPE_PAR_AND,
            ),
            Critical => Self::branched(
                LINETYPE_CRITICAL_START,
                LINETYPE_CRITICAL_END,
                OptionBranch,
                LINETYPE_CRITICAL_OPTION,
            ),
            _ => unreachable!(),
        }
    }
}

enum Frame {
    Box,
    Fragment(Fragment),
}

/// The first lexical or grammar error in the supplied input's byte coordinates.
#[derive(Debug)]
pub enum SequenceSyntaxDiagnostic {
    Lexical(LexError),
    UnexpectedToken {
        token: Tok,
        span: SourceSpan,
        expected: SequenceExpectedTokens,
    },
    UnexpectedEof {
        offset: usize,
        expected: SequenceExpectedTokens,
    },
}

enum Failure {
    Syntax(SequenceSyntaxDiagnostic),
    Cancelled(OperationCancelled),
}

impl From<OperationCancelled> for Failure {
    fn from(error: OperationCancelled) -> Self {
        Self::Cancelled(error)
    }
}

type ParseResult<T> = Result<T, Failure>;

pub(super) fn parse(
    events: impl Iterator<Item = LexicalEvent>,
    control: &OperationControl,
) -> OperationControlResult<Result<Vec<Action>, SequenceSyntaxDiagnostic>> {
    let mut parser = Parser {
        input: TokenStream::new(events, control),
        actions: Vec::new(),
        control,
    };
    let outcome = parser.document();
    control.checkpoint()?;
    match outcome {
        Ok(()) => Ok(Ok(parser.actions)),
        Err(Failure::Syntax(diagnostic)) => Ok(Err(diagnostic)),
        Err(Failure::Cancelled(error)) => Err(error),
    }
}

struct Parser<'a, I: Iterator> {
    input: TokenStream<'a, I>,
    actions: Vec<Action>,
    control: &'a OperationControl,
}

impl<I: Iterator<Item = LexicalEvent>> Parser<'_, I> {
    fn peek(&mut self) -> ParseResult<Option<Terminal>> {
        match self.input.peek()? {
            Some(Ok((_, token, _))) => Ok(Some(Terminal::of(token))),
            Some(Err(error)) => Err(Self::lexical_error(error.clone())),
            None => Ok(None),
        }
    }

    fn accept(&mut self, terminal: Terminal) -> ParseResult<bool> {
        if self.peek()? == Some(terminal) {
            self.input.bump()?;
            return Ok(true);
        }
        Ok(false)
    }

    fn require(&mut self, expected: Expected) -> ParseResult<Tok> {
        match self.input.bump()? {
            Some(Ok((_, token, _))) if expected.contains(Terminal::of(&token)) => Ok(token),
            event => Err(self.failure(event, expected)),
        }
    }

    fn failure(&self, event: Option<LexicalEvent>, expected: Expected) -> Failure {
        match event {
            Some(Ok((start, token, end))) => {
                Failure::Syntax(SequenceSyntaxDiagnostic::UnexpectedToken {
                    token,
                    span: SourceSpan::new(start, end),
                    expected,
                })
            }
            Some(Err(error)) => Self::lexical_error(error),
            None => Failure::Syntax(SequenceSyntaxDiagnostic::UnexpectedEof {
                offset: self.input.previous_end(),
                expected,
            }),
        }
    }

    fn lexical_error(error: LexError) -> Failure {
        Failure::Syntax(SequenceSyntaxDiagnostic::Lexical(error))
    }

    fn text(&mut self, terminal: Terminal) -> ParseResult<String> {
        match self.require(Expected::one(terminal))? {
            Tok::Actor(text) | Tok::Text(text) | Tok::RestOfLine(text) | Tok::Config(text) => {
                Ok(text)
            }
            _ => unreachable!("Sequence text must be an actor, text, line tail, or config"),
        }
    }

    fn emit(&mut self, action: Action) -> ParseResult<()> {
        // The old carrier checked about twice per action as it allocated and linked records.
        if self.actions.len().is_multiple_of(64) {
            self.control.checkpoint()?;
        }
        self.actions.push(action);
        Ok(())
    }

    fn document(&mut self) -> ParseResult<()> {
        let mut leading_newline = false;
        while self.accept(Newline)? {
            leading_newline = true;
        }
        let header = if leading_newline
            && matches!(self.peek()?, Some(Participant | ActorKw | Destroy | End))
        {
            Expected::one(SequenceDiagram)
        } else {
            Expected::new(&[Newline, SequenceDiagram])
        };
        self.require(header)?;

        // Statements emit flat actions eagerly. Only a complete document reaches DB replay.
        let mut frames = Vec::new();
        let mut body_at_entry = true;
        loop {
            let next = self.peek()?;
            let expected = match frames.last() {
                None if next.is_none() => return Ok(()),
                None => DOCUMENT,
                Some(Frame::Box) if !body_at_entry && next == Some(SequenceDiagram) => {
                    // The old grammar reduces extra box newlines before a second header.
                    BOX_BODY.without(Newline)
                }
                Some(Frame::Box) => BOX_BODY,
                Some(Frame::Fragment(fragment)) => {
                    let closing = match fragment.branch {
                        Some((keyword, _)) => Expected::new(&[End, keyword]),
                        None => Expected::one(End),
                    };
                    // A nonempty Doc reduces before the old parser rejects a foreign branch.
                    if !body_at_entry
                        && next.is_some_and(|keyword| {
                            matches!(keyword, Else | And | OptionBranch)
                                && !closing.contains(keyword)
                        })
                    {
                        closing
                    } else {
                        DOCUMENT.union(closing)
                    }
                }
            };

            body_at_entry = match self.require(expected)? {
                Tok::Newline => false,
                Tok::End => {
                    match frames.pop().expect("Sequence end requires an open block") {
                        Frame::Box => self.emit(Action::BoxEnd)?,
                        Frame::Fragment(fragment) => self.emit(Action::ControlSignal {
                            signal_type: fragment.end,
                            text: None,
                        })?,
                    }
                    false
                }
                Tok::Box => {
                    let header = self.text(RestOfLine)?;
                    self.require(Expected::one(Newline))?;
                    self.emit(Action::BoxStart { header })?;
                    frames.push(Frame::Box);
                    true
                }
                Tok::Loop
                | Tok::Rect
                | Tok::Opt
                | Tok::Alt
                | Tok::Par
                | Tok::ParOver
                | Tok::Critical
                | Tok::Break => {
                    let fragment = Fragment::after_head(next.expect("Sequence fragment head"));
                    let text = self.text(RestOfLine)?;
                    self.emit(Action::ControlSignal {
                        signal_type: fragment.start,
                        text: Some(text),
                    })?;
                    frames.push(Frame::Fragment(fragment));
                    true
                }
                Tok::Else | Tok::And | Tok::Option => {
                    let Some(Frame::Fragment(Fragment {
                        branch: Some((_, signal_type)),
                        ..
                    })) = frames.last()
                    else {
                        unreachable!("Sequence branch requires a matching open fragment");
                    };
                    let text = self.text(RestOfLine)?;
                    self.emit(Action::ControlSignal {
                        signal_type: *signal_type,
                        text: Some(text),
                    })?;
                    true
                }
                token => {
                    let expected_tail = self.line_statement(token)?;
                    self.line_end(frames.is_empty(), expected_tail)?;
                    matches!(frames.last(), Some(Frame::Box))
                }
            };
        }
    }

    fn line_end(&mut self, allow_eof: bool, expected_tail: Expected) -> ParseResult<()> {
        // Only a top-level ordinary statement can finish at EOF. Completed blocks resume
        // directly in the parent body and can be followed by another token on the same line.
        match self.peek()? {
            Some(Newline) => {
                self.require(Expected::one(Newline))?;
                Ok(())
            }
            None if allow_eof => Ok(()),
            next => {
                let expected = if next.is_none() {
                    Expected::one(Newline)
                } else {
                    expected_tail.with(Newline)
                };
                let event = self.input.bump()?;
                Err(self.failure(event, expected))
            }
        }
    }

    fn line_statement(&mut self, head: Tok) -> ParseResult<Expected> {
        let action = match head {
            Tok::Participant | Tok::ActorKw => return self.participant(head, false),
            Tok::Create => {
                let kind = self.require(DECLARATION)?;
                return self.participant(kind, true);
            }
            Tok::Destroy => Action::DestroyParticipant {
                id: self.text(Actor)?,
            },
            Tok::Actor(from) => {
                self.signal(from)?;
                return Ok(Expected::default());
            }
            Tok::Note => {
                self.note()?;
                return Ok(Expected::default());
            }
            Tok::Links | Tok::Link | Tok::Properties | Tok::Details => {
                let actor = self.text(Actor)?;
                let text = self.text(Text)?;
                self.emit(Action::EnsureParticipant { id: actor.clone() })?;
                match head {
                    Tok::Links => Action::AddLinks { actor, text },
                    Tok::Link => Action::AddLink { actor, text },
                    Tok::Properties => Action::AddProperties { actor, text },
                    Tok::Details => Action::AddDetails { actor, text },
                    _ => unreachable!(),
                }
            }
            Tok::Title(text) | Tok::CompatTitle(text) => Action::SetTitle(text),
            Tok::AccTitle(text) => Action::SetAccTitle(text),
            Tok::AccDescr(text) | Tok::AccDescrMultiline(text) => Action::SetAccDescr(text),
            Tok::Autonumber => return self.autonumber(),
            Tok::Activate => Action::ActiveStart {
                actor: self.text(Actor)?,
            },
            Tok::Deactivate => Action::ActiveEnd {
                actor: self.text(Actor)?,
            },
            _ => unreachable!("Sequence line statement requires a statement head"),
        };
        self.emit(action)?;
        Ok(Expected::default())
    }

    fn participant(&mut self, kind: Tok, create: bool) -> ParseResult<Expected> {
        let draw = match kind {
            Tok::Participant => "participant",
            Tok::ActorKw => "actor",
            _ => unreachable!("Sequence participant declaration requires participant or actor"),
        }
        .to_string();
        let id = self.text(Actor)?;
        let config = if self.peek()? == Some(Config) {
            Some(self.text(Config)?)
        } else {
            None
        };
        let (description, expected) = if self.accept(As)? {
            (Some(self.text(RestOfLine)?), Expected::default())
        } else if config.is_some() {
            (None, Expected::one(As))
        } else {
            (None, Expected::new(&[As, Config]))
        };
        self.emit(if create {
            Action::CreateParticipant {
                id,
                description,
                draw,
                config,
            }
        } else {
            Action::AddParticipant {
                id,
                description,
                draw,
                config,
            }
        })?;
        Ok(expected)
    }

    fn signal(&mut self, from: String) -> ParseResult<()> {
        let (source_central, signal_type) =
            match self.require(Expected::new(&[Central, SignalType]))? {
                Tok::SignalType(signal_type) => (false, signal_type),
                Tok::Central => match self.require(Expected::one(SignalType))? {
                    Tok::SignalType(signal_type) => (true, signal_type),
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            };
        let expected = if source_central {
            Expected::new(&[Central, Actor])
        } else {
            Expected::new(&[Plus, Minus, Central, Actor])
        };
        let (to, activate, target_central, follow) = match self.require(expected)? {
            Tok::Actor(actor) => (actor, false, false, None),
            Tok::Plus => {
                let actor = self.text(Actor)?;
                let follow = Action::ActiveStart {
                    actor: actor.clone(),
                };
                (actor, true, false, Some(follow))
            }
            Tok::Minus => (
                self.text(Actor)?,
                false,
                false,
                Some(Action::ActiveEnd {
                    actor: from.clone(),
                }),
            ),
            Tok::Central => {
                let actor = self.text(Actor)?;
                let follow = Action::CentralConnection {
                    actor: actor.clone(),
                };
                (actor, true, true, Some(follow))
            }
            _ => unreachable!("Sequence signal requires a target or a target modifier"),
        };
        let text = self.text(Text)?;
        let central_connection = match (source_central, target_central) {
            (false, false) => 0,
            (false, true) => LINETYPE_CENTRAL_CONNECTION,
            (true, false) => LINETYPE_CENTRAL_CONNECTION_REVERSE,
            (true, true) => LINETYPE_CENTRAL_CONNECTION_DUAL,
        };
        let source_follow = source_central.then(|| Action::CentralConnectionReverse {
            actor: from.clone(),
        });
        self.emit(Action::EnsureParticipant { id: from.clone() })?;
        self.emit(Action::EnsureParticipant { id: to.clone() })?;
        self.emit(Action::AddMessage {
            from,
            to,
            signal_type,
            text,
            activate,
            central_connection,
        })?;
        for action in [follow, source_follow].into_iter().flatten() {
            self.emit(action)?;
        }
        Ok(())
    }

    fn note(&mut self) -> ParseResult<()> {
        let placement = match self.require(Expected::new(&[LeftOf, RightOf, Over]))? {
            Tok::LeftOf => PLACEMENT_LEFT_OF,
            Tok::RightOf => PLACEMENT_RIGHT_OF,
            Tok::Over => PLACEMENT_OVER,
            _ => unreachable!(),
        };
        let first = self.text(Actor)?;
        let (second, text) = if placement == PLACEMENT_OVER {
            match self.require(Expected::new(&[Comma, Text]))? {
                Tok::Comma => (Some(self.text(Actor)?), self.text(Text)?),
                Tok::Text(text) => (None, text),
                _ => unreachable!(),
            }
        } else {
            (None, self.text(Text)?)
        };
        self.emit(Action::EnsureParticipant { id: first.clone() })?;
        let actors = if let Some(second) = second {
            self.emit(Action::EnsureParticipant { id: second.clone() })?;
            vec![first, second]
        } else if placement == PLACEMENT_OVER {
            vec![first.clone(), first]
        } else {
            vec![first]
        };
        self.emit(Action::AddNote {
            actors,
            placement,
            text,
        })
    }

    fn autonumber(&mut self) -> ParseResult<Expected> {
        let (start, step, visible, expected) = match self.peek()? {
            Some(Off) => {
                self.require(Expected::one(Off))?;
                (None, None, false, Expected::default())
            }
            Some(Num) => {
                let start = self.number()?;
                if self.peek()? == Some(Num) {
                    (Some(start), Some(self.number()?), true, Expected::default())
                } else {
                    (Some(start), Some(1.0), true, Expected::one(Num))
                }
            }
            _ => (None, None, true, Expected::new(&[Num, Off])),
        };
        self.emit(Action::Autonumber {
            start,
            step,
            visible,
        })?;
        Ok(expected)
    }

    fn number(&mut self) -> ParseResult<f64> {
        match self.require(Expected::one(Num))? {
            Tok::Num(number) => Ok(number),
            _ => unreachable!(),
        }
    }
}
