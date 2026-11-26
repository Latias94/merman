//! Predictive Class grammar over the existing normalized lexical events.

use super::ast::{Action, ActionArena, ActionList, ClassIdList, Relation, RelationData};
use super::lexer::{ClassLexicalEvent, LexError, Tok};
use crate::scan::TokenStream;
use merman_model::families::class::{
    LINE_DOTTED, LINE_SOLID, REL_AGGREGATION, REL_COMPOSITION, REL_DEPENDENCY, REL_EXTENSION,
    REL_LOLLIPOP, REL_NONE,
};
use merman_model::{OperationCancelled, OperationControl, OperationControlResult, SourceSpan};

type Token = (usize, Tok, usize);
type NamespaceItem = (ActionList, ClassIdList);

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

// Diagnostic order follows the grammar's terminal declarations.
terminals! {
    Newline => "Newline",
    ClassDiagram => "\"classDiagram\"",
    Direction(_) => "Direction",
    ClassKw => "\"class\"",
    NamespaceKw => "\"namespace\"",
    Note => "\"note\"",
    NoteFor => "\"note_for\"",
    CssClass => "\"cssClass\"",
    StyleKw => "\"style\"",
    ClassDefKw => "\"classDef\"",
    ClickKw => "\"click\"",
    LinkKw => "\"link\"",
    CallbackKw => "\"callback\"",
    HrefKw => "\"href\"",
    CallKw => "\"call\"",
    StructStart => "StructStart",
    StructStop => "StructStop",
    SquareStart => "SquareStart",
    SquareStop => "SquareStop",
    AnnotationStart => "AnnotationStart",
    AnnotationStop => "AnnotationStop",
    StyleSeparator => "StyleSeparator",
    Comma => "Comma",
    Ext => "Ext",
    Dep => "Dep",
    Comp => "Comp",
    Agg => "Agg",
    Lollipop => "Lollipop",
    Line => "Line",
    DottedLine => "DottedLine",
    Label(_) => "Label",
    Str(_) => "Str",
    Name(_) => "Name",
    Member(_) => "Member",
    RestOfLine(_) => "RestOfLine",
    LinkTarget(_) => "LinkTarget",
    CallbackName(_) => "CallbackName",
    CallbackArgs(_) => "CallbackArgs",
    AccTitle(_) => "AccTitle",
    AccDescr(_) => "AccDescr",
    AccDescrMultiline(_) => "AccDescrMultiline",
}

use Terminal::*;

/// The grammar terminals accepted at a failing position, in grammar order.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClassExpectedTokens(u64);

use ClassExpectedTokens as Expected;

impl ClassExpectedTokens {
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

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    const fn contains(self, terminal: Terminal) -> bool {
        self.0 & (1 << terminal as u8) != 0
    }

    pub fn names(self) -> impl Iterator<Item = &'static str> {
        TERMINAL_NAMES
            .iter()
            .copied()
            .enumerate()
            .filter_map(move |(index, name)| (self.0 & (1 << index) != 0).then_some(name))
    }
}

const EMPTY: Expected = Expected(0);
const NAMESPACE_ITEMS: Expected = Expected::new(&[ClassKw, NamespaceKw, Note, NoteFor, StructStop]);
const ROOT_ITEMS: Expected = Expected::new(&[
    Direction,
    ClassKw,
    NamespaceKw,
    Note,
    NoteFor,
    CssClass,
    StyleKw,
    ClassDefKw,
    ClickKw,
    LinkKw,
    CallbackKw,
    AnnotationStart,
    Name,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
]);
const ROOT_END: Expected = Expected::new(&[Newline]);
const NAMESPACE_END: Expected = ROOT_END.with(StructStop);
const LINE: Expected = Expected::new(&[Line, DottedLine]);
const RELATION_TYPE: Expected = Expected::new(&[Ext, Dep, Comp, Agg, Lollipop]);
const RELATION_START: Expected = RELATION_TYPE.union(LINE);
const ENDPOINT: Expected = Expected::new(&[Str, Name]);

/// The first lexical or grammar error in the supplied source's byte coordinates.
#[derive(Debug)]
pub enum ClassSyntaxDiagnostic {
    Lexical(LexError),
    UnexpectedToken {
        token: Tok,
        span: SourceSpan,
        expected: ClassExpectedTokens,
    },
    UnexpectedEof {
        offset: usize,
        expected: ClassExpectedTokens,
    },
}

enum Failure {
    Syntax(ClassSyntaxDiagnostic),
    Cancelled(OperationCancelled),
}

impl From<OperationCancelled> for Failure {
    fn from(error: OperationCancelled) -> Self {
        Self::Cancelled(error)
    }
}

type ParseResult<T> = Result<T, Failure>;

pub(super) fn parse(
    arena: &mut ActionArena,
    events: impl Iterator<Item = ClassLexicalEvent>,
    control: &OperationControl,
) -> OperationControlResult<Result<ActionList, ClassSyntaxDiagnostic>> {
    control.checkpoint()?;
    let outcome = Parser {
        arena,
        input: TokenStream::new(events, control),
        control,
    }
    .document();
    control.checkpoint()?;
    match outcome {
        Ok(actions) => Ok(Ok(actions)),
        Err(Failure::Syntax(error)) => Ok(Err(error)),
        Err(Failure::Cancelled(error)) => Err(error),
    }
}

struct NamespaceFrame {
    name: String,
    label: Option<String>,
    items: Vec<NamespaceItem>,
}

struct Parser<'a, 'control, I: Iterator> {
    arena: &'a mut ActionArena,
    input: TokenStream<'control, I>,
    control: &'control OperationControl,
}

impl<I: Iterator<Item = ClassLexicalEvent>> Parser<'_, '_, I> {
    fn document(&mut self) -> ParseResult<ActionList> {
        let leading_newlines = self.skip_newlines()?;
        let mut header = Expected::new(&[Newline, ClassDiagram]);
        // The old parser shares this Newlines reduction with namespace item lists.
        if leading_newlines
            && self
                .peek()?
                .is_some_and(|token| NAMESPACE_ITEMS.contains(token))
        {
            header = Expected::new(&[ClassDiagram]);
        }
        self.require(header)?;
        self.require(ROOT_END)?;

        let mut root = Vec::new();
        let mut namespaces: Vec<NamespaceFrame> = Vec::new();
        let mut token = self.require(ROOT_ITEMS)?;
        loop {
            let item = match token.1 {
                Tok::NamespaceKw => {
                    namespaces.push(self.open_namespace()?);
                    token = self.next_namespace_item(false)?;
                    continue;
                }
                Tok::StructStop => {
                    let frame = namespaces
                        .pop()
                        .expect("namespace body owns its closing token");
                    let body = frame
                        .items
                        .into_iter()
                        .rev()
                        .try_fold(NamespaceItem::default(), |rest, item| {
                            self.arena.concat_namespace_items(item, rest, self.control)
                        })?;
                    self.finish(!namespaces.is_empty(), EMPTY)?;
                    (
                        self.arena
                            .namespace(frame.name, frame.label, body, self.control)?,
                        ClassIdList::default(),
                    )
                }
                _ => self.statement(token, !namespaces.is_empty())?,
            };

            if let Some(namespace) = namespaces.last_mut() {
                namespace.items.push(item);
                token = self.next_namespace_item(true)?;
            } else {
                root.push(item.0);
                if !self.accept(Newline)? || self.peek()?.is_none() {
                    break;
                }
                token = self.require(ROOT_ITEMS)?;
            }
        }
        Ok(root
            .into_iter()
            .rev()
            .try_fold(ActionList::default(), |rest, item| {
                self.arena.concat(item, rest, self.control)
            })?)
    }

    fn open_namespace(&mut self) -> ParseResult<NamespaceFrame> {
        let name = self.text(Name)?;
        let label = match self.require(Expected::new(&[SquareStart, StructStart]))?.1 {
            Tok::SquareStart => {
                let label = self.class_label()?;
                self.require(Expected::new(&[StructStart]))?;
                Some(label)
            }
            Tok::StructStart => None,
            _ => unreachable!("namespace label or body"),
        };
        Ok(NamespaceFrame {
            name,
            label,
            items: Vec::new(),
        })
    }

    fn next_namespace_item(&mut self, after_statement: bool) -> ParseResult<Token> {
        if after_statement {
            let separator = self.require(NAMESPACE_END)?;
            if matches!(separator.1, Tok::StructStop) {
                return Ok(separator);
            }
        }
        let reduced_newlines = self.skip_newlines()?;
        // Newlines1 has already consumed its first separator above. The diagnostic
        // reduction applies after a newline in the optional Newlines tail.
        let expected = if reduced_newlines && self.peek()? == Some(ClassDiagram) {
            NAMESPACE_ITEMS
        } else {
            NAMESPACE_ITEMS.with(Newline)
        };
        self.require(expected)
    }

    fn statement(
        &mut self,
        (start, token, end): Token,
        nested: bool,
    ) -> ParseResult<NamespaceItem> {
        let action = match token {
            Tok::ClassKw => return self.class(nested),
            Tok::ClassDefKw => return self.class_def(),
            Tok::ClickKw | Tok::LinkKw | Tok::CallbackKw => {
                return self.interaction(Terminal::of(&token));
            }
            Tok::Name(id) => return self.named_statement(id, SourceSpan::new(start, end)),
            Tok::Direction(direction) => Action::SetDirection(direction),
            Tok::AccTitle(title) => Action::SetAccTitle(title),
            Tok::AccDescr(text) | Tok::AccDescrMultiline(text) => Action::SetAccDescr(text),
            Tok::AnnotationStart => {
                let annotation = self.annotation()?;
                Action::AddAnnotation {
                    id: self.text(Name)?,
                    annotation,
                }
            }
            Tok::Note | Tok::NoteFor => Action::AddNote {
                class_id: if matches!(token, Tok::NoteFor) {
                    Some(self.text(Name)?)
                } else {
                    None
                },
                text: self.text(Str)?,
            },
            Tok::CssClass => Action::SetCssClass {
                ids: self.text(Str)?,
                css_class: self.text(Name)?,
            },
            Tok::StyleKw => Action::SetCssStyle {
                id: self.text(Name)?,
                raw: self.text(RestOfLine)?,
            },
            _ => unreachable!("Class statement start has been checked"),
        };
        self.finish(nested, EMPTY)?;
        self.actions([action])
    }

    fn class(&mut self, nested: bool) -> ParseResult<NamespaceItem> {
        let id = self.text(Name)?;
        let mut allowed_suffix =
            Expected::new(&[SquareStart, AnnotationStart, StyleSeparator, StructStart]);
        let label = if self.accept(SquareStart)? {
            allowed_suffix = Expected::new(&[AnnotationStart, StyleSeparator, StructStart]);
            Some(self.class_label()?)
        } else {
            None
        };
        let annotation = if self.accept(AnnotationStart)? {
            allowed_suffix = Expected::new(&[StyleSeparator, StructStart]);
            Some(self.annotation()?)
        } else {
            None
        };
        let css = if self.accept(StyleSeparator)? {
            allowed_suffix = Expected::new(&[StructStart]);
            Some(self.text(Name)?)
        } else {
            None
        };
        let members = if self.accept(StructStart)? {
            allowed_suffix = EMPTY;
            let mut members = Vec::new();
            loop {
                match self.require(Expected::new(&[Member, StructStop]))?.1 {
                    Tok::Member(member) => members.push(member),
                    Tok::StructStop => break,
                    _ => unreachable!("Class body members or closing delimiter"),
                }
            }
            // ClassDb::add_members reverses the grammar's member list on replay.
            members.reverse();
            Some(members)
        } else {
            None
        };
        self.finish(nested, allowed_suffix)?;

        let list = self.arena.push_actions(
            [
                Some(Action::AddClass { id: id.clone() }),
                label.map(|label| Action::SetClassLabel {
                    id: id.clone(),
                    label,
                }),
                annotation.map(|annotation| Action::AddAnnotation {
                    id: id.clone(),
                    annotation,
                }),
                css.map(|css_class| Action::SetCssClass {
                    ids: id.clone(),
                    css_class,
                }),
                members.map(|members| Action::AddMembers {
                    id: id.clone(),
                    members,
                }),
            ]
            .into_iter()
            .flatten(),
            self.control,
        )?;
        let ids = if nested {
            self.arena.push_class_id(id, self.control)?
        } else {
            ClassIdList::default()
        };
        Ok((list, ids))
    }

    fn class_label(&mut self) -> ParseResult<String> {
        let label = self.text(Str)?;
        self.require(Expected::new(&[SquareStop]))?;
        Ok(label)
    }

    fn annotation(&mut self) -> ParseResult<String> {
        let annotation = self.text(Name)?;
        self.require(Expected::new(&[AnnotationStop]))?;
        Ok(annotation)
    }

    fn class_def(&mut self) -> ParseResult<NamespaceItem> {
        let mut ids = Vec::new();
        loop {
            let id = self.text(Name)?;
            if !matches!(self.peek()?, Some(Comma | RestOfLine)) {
                let token = self.bump()?;
                return Err(self.unexpected(token, Expected::new(&[Comma, RestOfLine])));
            }
            self.arena.checkpoint(self.control)?;
            ids.push(id);
            if !self.accept(Comma)? {
                break;
            }
        }
        let raw = self.text(RestOfLine)?;
        self.finish(false, EMPTY)?;
        self.actions(ids.into_iter().map(|id| Action::DefineClass {
            id,
            raw: raw.clone(),
        }))
    }

    fn named_statement(&mut self, id: String, span: SourceSpan) -> ParseResult<NamespaceItem> {
        match self.peek()? {
            Some(Label) => {
                let member = self.text(Label)?;
                self.finish(false, EMPTY)?;
                self.actions([Action::AddMember { id, member }])
            }
            Some(terminal) if terminal == Str || RELATION_START.contains(terminal) => {
                let relation = self.relation(id)?;
                self.actions([Action::AddRelation { data: relation }])
            }
            _ => {
                self.finish(false, RELATION_START.with(Label).with(Str))?;
                if self.arena.capture_ignored_statements {
                    self.actions([Action::WarnIgnoredStatement { span }])
                } else {
                    Ok(NamespaceItem::default())
                }
            }
        }
    }

    fn relation(&mut self, id1: String) -> ParseResult<RelationData> {
        let relation_title1 = self.optional_text(Str)?;
        let mut line = self.require(RELATION_START)?.1;
        let mut type1 = REL_NONE;
        if RELATION_TYPE.contains(Terminal::of(&line)) {
            type1 = relation_type(line);
            line = self.require(LINE)?.1;
        }
        let line_type = if matches!(line, Tok::Line) {
            LINE_SOLID
        } else {
            LINE_DOTTED
        };

        let mut endpoint = self.require(RELATION_TYPE.union(ENDPOINT))?.1;
        let mut type2 = REL_NONE;
        if RELATION_TYPE.contains(Terminal::of(&endpoint)) {
            type2 = relation_type(endpoint);
            endpoint = self.require(ENDPOINT)?.1;
        }
        let (relation_title2, id2) = match endpoint {
            Tok::Name(id) => (None, id),
            Tok::Str(title) => (Some(title), self.text(Name)?),
            _ => unreachable!("right Class relation endpoint"),
        };
        let title = self.optional_text(Label)?;
        self.finish(
            false,
            if title.is_none() {
                EMPTY.with(Label)
            } else {
                EMPTY
            },
        )?;
        Ok(RelationData {
            id1,
            id2,
            relation: Relation {
                type1,
                type2,
                line_type,
            },
            relation_title1,
            relation_title2,
            title,
        })
    }

    fn interaction(&mut self, directive: Terminal) -> ParseResult<NamespaceItem> {
        let id = self.text(Name)?;
        let operation = if directive == ClickKw {
            Terminal::of(&self.require(Expected::new(&[HrefKw, CallKw]))?.1)
        } else {
            directive
        };

        let (action, tooltip) = match operation {
            LinkKw | HrefKw => {
                let url = self.text(Str)?;
                let tooltip = self.optional_text(Str)?;
                let target = self.optional_text(LinkTarget)?;
                let suffix = match (&tooltip, &target) {
                    (_, Some(_)) => EMPTY,
                    (Some(_), None) => EMPTY.with(LinkTarget),
                    (None, None) => Expected::new(&[Str, LinkTarget]),
                };
                self.finish(false, suffix)?;
                (
                    Action::SetLink {
                        id: id.clone(),
                        url,
                        target,
                    },
                    tooltip,
                )
            }
            CallKw | CallbackKw => {
                let call = operation == CallKw;
                let function = self.text(if call { CallbackName } else { Str })?;
                let args = if call {
                    self.optional_text(CallbackArgs)?
                } else {
                    None
                };
                let tooltip = self.optional_text(Str)?;
                let suffix = if tooltip.is_some() {
                    EMPTY
                } else if call && args.is_none() {
                    Expected::new(&[Str, CallbackArgs])
                } else {
                    EMPTY.with(Str)
                };
                self.finish(false, suffix)?;
                (
                    Action::SetClickEvent {
                        id: id.clone(),
                        function,
                        args,
                    },
                    tooltip,
                )
            }
            _ => unreachable!("Class interaction operation"),
        };
        self.actions(
            [
                Some(action),
                tooltip.map(|tooltip| Action::SetTooltip { id, tooltip }),
            ]
            .into_iter()
            .flatten(),
        )
    }

    fn actions(&mut self, actions: impl IntoIterator<Item = Action>) -> ParseResult<NamespaceItem> {
        Ok((
            self.arena.push_actions(actions, self.control)?,
            ClassIdList::default(),
        ))
    }

    fn finish(&mut self, nested: bool, suffix: Expected) -> ParseResult<()> {
        match self.peek()? {
            Some(Newline) => Ok(()),
            Some(StructStop) if nested => Ok(()),
            None if !nested => Ok(()),
            _ => {
                let token = self.bump()?;
                let follow = if nested { NAMESPACE_END } else { ROOT_END };
                Err(self.unexpected(token, follow.union(suffix)))
            }
        }
    }

    fn skip_newlines(&mut self) -> ParseResult<bool> {
        let mut skipped = false;
        while self.accept(Newline)? {
            skipped = true;
        }
        Ok(skipped)
    }

    fn optional_text(&mut self, expected: Terminal) -> ParseResult<Option<String>> {
        if self.peek()? == Some(expected) {
            self.text(expected).map(Some)
        } else {
            Ok(None)
        }
    }

    fn text(&mut self, expected: Terminal) -> ParseResult<String> {
        match self.require(EMPTY.with(expected))?.1 {
            Tok::Name(text)
            | Tok::Str(text)
            | Tok::Label(text)
            | Tok::RestOfLine(text)
            | Tok::LinkTarget(text)
            | Tok::CallbackName(text)
            | Tok::CallbackArgs(text) => Ok(text),
            _ => unreachable!("Class text production expects a string token"),
        }
    }

    fn accept(&mut self, terminal: Terminal) -> ParseResult<bool> {
        let matched = self.peek()? == Some(terminal);
        if matched {
            self.bump()?;
        }
        Ok(matched)
    }

    fn peek(&mut self) -> ParseResult<Option<Terminal>> {
        match self.input.peek()? {
            Some(Ok((_, token, _))) => Ok(Some(Terminal::of(token))),
            Some(Err(error)) => Err(Failure::Syntax(ClassSyntaxDiagnostic::Lexical(
                error.clone(),
            ))),
            None => Ok(None),
        }
    }

    fn bump(&mut self) -> ParseResult<Option<Token>> {
        self.input
            .bump()?
            .transpose()
            .map_err(|error| Failure::Syntax(ClassSyntaxDiagnostic::Lexical(error)))
    }

    fn require(&mut self, expected: Expected) -> ParseResult<Token> {
        match self.bump()? {
            Some(token) if expected.contains(Terminal::of(&token.1)) => Ok(token),
            token => Err(self.unexpected(token, expected)),
        }
    }

    fn unexpected(&self, token: Option<Token>, expected: Expected) -> Failure {
        Failure::Syntax(match token {
            Some((start, token, end)) => ClassSyntaxDiagnostic::UnexpectedToken {
                token,
                span: SourceSpan::new(start, end),
                expected,
            },
            None => ClassSyntaxDiagnostic::UnexpectedEof {
                offset: self.input.previous_end(),
                expected,
            },
        })
    }
}

fn relation_type(token: Tok) -> i32 {
    match token {
        Tok::Ext => REL_EXTENSION,
        Tok::Dep => REL_DEPENDENCY,
        Tok::Comp => REL_COMPOSITION,
        Tok::Agg => REL_AGGREGATION,
        Tok::Lollipop => REL_LOLLIPOP,
        _ => unreachable!("Class relation marker"),
    }
}
