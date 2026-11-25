//! Predictive parser over the ER lexer's spanned events.

use super::{
    Action, ErActionArena, ErActionList, ErLexicalError, ErLexicalEvent, ErParsedActions, ErToken,
};
use crate::scan::TokenStream;
use merman_model::families::er::{ErAttribute, ErRelSpec};
use merman_model::{OperationCancelled, OperationControl, OperationControlResult, SourceSpan};

macro_rules! terminals {
    ($($terminal:ident $(($payload:tt))? => $name:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        enum Terminal {
            $($terminal),+
        }

        const TERMINAL_NAMES: &[&str] = &[$($name),+];

        impl Terminal {
            const fn of(token: &ErToken) -> Self {
                match token {
                    $(ErToken::$terminal $(($payload))? => Self::$terminal),+
                }
            }
        }
    };
}

// Keep the grammar's terminal order in diagnostics.
terminals! {
    Newline => "Newline",
    ErDiagram => "\"erDiagram\"",
    Name(_) => "Name",
    Str(_) => "Str",
    IdList(_) => "IdList",
    RestOfLine(_) => "RestOfLine",
    AccTitle(_) => "AccTitle",
    AccDescr(_) => "AccDescr",
    AccDescrMultiline(_) => "AccDescrMultiline",
    Direction(_) => "Direction",
    BlockStart => "BlockStart",
    BlockStop => "BlockStop",
    SquareStart => "SquareStart",
    SquareStop => "SquareStop",
    StyleSeparator => "StyleSeparator",
    Colon => "Colon",
    Comma => "Comma",
    Question => "Question",
    StyleKw => "\"style\"",
    ClassDefKw => "\"classDef\"",
    ClassKw => "\"class\"",
    SubgraphKw => "\"subgraph\"",
    EndKw => "\"end\"",
    ZeroOrOne => "ZeroOrOne",
    ZeroOrMore => "ZeroOrMore",
    OneOrMore => "OneOrMore",
    OnlyOne => "OnlyOne",
    MdParent => "MdParent",
    Identifying => "Identifying",
    NonIdentifying => "NonIdentifying",
    AttrWord(_) => "AttrWord",
    AttrKey(_) => "AttrKey",
    Comment(_) => "Comment",
}

/// Grammar terminals accepted at the failing position.
#[derive(Debug, Clone, Copy)]
pub struct ErExpectedTokens(u64);

impl ErExpectedTokens {
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

    /// Terminal names in grammar order.
    pub fn names(self) -> impl Iterator<Item = &'static str> {
        TERMINAL_NAMES
            .iter()
            .copied()
            .enumerate()
            .filter_map(move |(index, name)| (self.0 & (1 << index) != 0).then_some(name))
    }
}

use Terminal::{
    AccDescr, AccDescrMultiline, AccTitle, AttrKey, AttrWord, BlockStart, BlockStop, ClassDefKw,
    ClassKw, Colon, Comma, Comment, Direction, EndKw, ErDiagram, IdList, Identifying, MdParent,
    Name, Newline, NonIdentifying, OneOrMore, OnlyOne, Question, RestOfLine, SquareStart,
    SquareStop, Str, StyleKw, StyleSeparator, SubgraphKw, ZeroOrMore, ZeroOrOne,
};

const STATEMENT_START: ErExpectedTokens = ErExpectedTokens::new(&[
    Name,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Direction,
    StyleKw,
    ClassDefKw,
    ClassKw,
    SubgraphKw,
]);
const NAME_OR_STR: ErExpectedTokens = ErExpectedTokens::new(&[Name, Str]);
const CARDINALITY: ErExpectedTokens =
    ErExpectedTokens::new(&[ZeroOrOne, ZeroOrMore, OneOrMore, OnlyOne, MdParent]);
const RELATION_KIND: ErExpectedTokens = ErExpectedTokens::new(&[Identifying, NonIdentifying]);
const ATTRIBUTE_BOUNDARY: ErExpectedTokens = ErExpectedTokens::new(&[BlockStop, AttrWord]);

/// The first lexing or grammar error in ER input byte coordinates.
#[derive(Debug)]
pub enum ErSyntaxDiagnostic {
    Lexical(ErLexicalError),
    UnexpectedToken {
        token: ErToken,
        span: SourceSpan,
        expected: ErExpectedTokens,
    },
    UnexpectedEof {
        offset: usize,
        expected: ErExpectedTokens,
    },
}

#[derive(Debug)]
enum Failure {
    Syntax(ErSyntaxDiagnostic),
    Cancelled(OperationCancelled),
}

impl From<OperationCancelled> for Failure {
    fn from(cancelled: OperationCancelled) -> Self {
        Self::Cancelled(cancelled)
    }
}

type ParseResult<T> = std::result::Result<T, Failure>;

pub(super) fn parse(
    events: impl Iterator<Item = ErLexicalEvent>,
    control: &OperationControl,
) -> OperationControlResult<ErParsedActions> {
    let mut parser = Parser::new(events, control);
    let outcome = parser.document();
    control.checkpoint()?;
    match outcome {
        Ok(actions) => Ok(Ok((parser.arena, actions))),
        Err(Failure::Syntax(error)) => Ok(Err(error)),
        Err(Failure::Cancelled(error)) => Err(error),
    }
}

struct SubgraphFrame {
    id: String,
    title: String,
    outer: ErActionList,
}

struct Parser<'a, I: Iterator> {
    input: TokenStream<'a, I>,
    arena: ErActionArena,
}

impl<'a, I: Iterator<Item = ErLexicalEvent>> Parser<'a, I> {
    fn new(events: I, control: &'a OperationControl) -> Self {
        Self {
            input: TokenStream::new(events, control),
            arena: ErActionArena::default(),
        }
    }

    fn peek(&mut self) -> ParseResult<Option<Terminal>> {
        match self.input.peek()? {
            Some(Ok((_, token, _))) => Ok(Some(Terminal::of(token))),
            Some(Err(error)) => Err(Failure::Syntax(ErSyntaxDiagnostic::Lexical(error.clone()))),
            None => Ok(None),
        }
    }

    fn accept(&mut self, terminal: Terminal) -> ParseResult<bool> {
        let matches = self.peek()? == Some(terminal);
        if matches {
            let _ = self.input.bump()?;
        }
        Ok(matches)
    }

    fn require(&mut self, expected: ErExpectedTokens) -> ParseResult<ErToken> {
        match self.input.bump()? {
            Some(Ok((_, token, _))) if expected.contains(Terminal::of(&token)) => Ok(token),
            event => Err(self.failure(event, expected)),
        }
    }

    fn unexpected<T>(&mut self, expected: ErExpectedTokens) -> ParseResult<T> {
        let event = self.input.bump()?;
        Err(self.failure(event, expected))
    }

    fn failure(&self, event: Option<ErLexicalEvent>, expected: ErExpectedTokens) -> Failure {
        Failure::Syntax(match event {
            Some(Ok((start, token, end))) => ErSyntaxDiagnostic::UnexpectedToken {
                token,
                span: SourceSpan::new(start, end),
                expected,
            },
            Some(Err(error)) => ErSyntaxDiagnostic::Lexical(error),
            None => ErSyntaxDiagnostic::UnexpectedEof {
                offset: self.input.previous_end(),
                expected,
            },
        })
    }

    fn text(&mut self, expected: ErExpectedTokens) -> ParseResult<String> {
        match self.require(expected)? {
            ErToken::Name(text)
            | ErToken::Str(text)
            | ErToken::RestOfLine(text)
            | ErToken::AttrWord(text)
            | ErToken::AttrKey(text)
            | ErToken::Comment(text) => Ok(text),
            _ => unreachable!("ER text expectation contains only string tokens"),
        }
    }

    fn id_list(&mut self) -> ParseResult<Vec<String>> {
        match self.require(ErExpectedTokens::one(IdList))? {
            ErToken::IdList(ids) => Ok(ids.into_names()),
            _ => unreachable!("ER id list expectation contains only IdList"),
        }
    }

    fn inline_classes(&mut self) -> ParseResult<Option<Vec<String>>> {
        if self.accept(StyleSeparator)? {
            return self.id_list().map(Some);
        }
        Ok(None)
    }

    fn document(&mut self) -> ParseResult<ErActionList> {
        let mut leading_newline = false;
        while self.accept(Newline)? {
            leading_newline = true;
        }
        let mut header = ErExpectedTokens::new(&[Newline, ErDiagram]);
        // Preserve the reported expectations after the old grammar reduces its leading newlines.
        if leading_newline
            && self
                .peek()?
                .is_none_or(|next| STATEMENT_START.with(EndKw).contains(next))
        {
            header = header.without(Newline);
        }
        self.require(header)?;

        let mut frames = Vec::<SubgraphFrame>::new();
        let mut body = ErActionList::default();
        let mut skipped_newline = false;
        loop {
            let next = self.peek()?;
            let at_root = frames.is_empty();
            if at_root && next.is_none() {
                return Ok(body);
            }
            let mut follow = STATEMENT_START.with(Newline);
            if !at_root {
                follow = follow.with(EndKw);
            }
            // Reducing a non-empty separator list can expose a narrower expected set.
            let expected = if skipped_newline
                && match next {
                    Some(ErDiagram) => true,
                    Some(EndKw) => at_root,
                    None => !at_root,
                    _ => false,
                } {
                follow.without(Newline)
            } else {
                follow
            };
            match self.require(expected)? {
                ErToken::Newline => {
                    skipped_newline = true;
                    continue;
                }
                ErToken::SubgraphKw => {
                    let (id, title) = self.subgraph_header()?;
                    frames.push(SubgraphFrame {
                        id,
                        title,
                        outer: std::mem::take(&mut body),
                    });
                }
                ErToken::EndKw => {
                    let frame = frames.pop().expect("ER end requires an open subgraph");
                    let subgraph = self.arena.push(Action::AddSubgraph {
                        id: frame.id,
                        title: frame.title,
                        label_type: "markdown".to_string(),
                        body,
                    });
                    body = self.arena.append(frame.outer, subgraph);
                }
                ErToken::Name(name) => {
                    let statement = self.named_statement(name, follow)?;
                    body = self.arena.append(body, statement);
                }
                token => {
                    let statement = self.payload_statement(token)?;
                    let statement = self.arena.push(statement);
                    body = self.arena.append(body, statement);
                }
            }
            skipped_newline = false;
        }
    }

    fn subgraph_header(&mut self) -> ParseResult<(String, String)> {
        let id = self.text(NAME_OR_STR)?;
        match self.require(ErExpectedTokens::new(&[Newline, SquareStart]))? {
            ErToken::Newline => Ok((id.clone(), id)),
            ErToken::SquareStart => {
                let mut title = self.text(NAME_OR_STR)?;
                loop {
                    match self.require(NAME_OR_STR.with(SquareStop))? {
                        ErToken::Name(part) | ErToken::Str(part) => {
                            title.push(' ');
                            title.push_str(&part);
                        }
                        ErToken::SquareStop => break,
                        _ => unreachable!("ER subgraph title expects text or its closing bracket"),
                    }
                }
                self.require(ErExpectedTokens::one(Newline))?;
                Ok((id, title))
            }
            _ => unreachable!("ER subgraph header expects a line ending or title"),
        }
    }

    fn payload_statement(&mut self, token: ErToken) -> ParseResult<Action> {
        Ok(match token {
            ErToken::AccTitle(title) => Action::SetAccTitle(title),
            ErToken::AccDescr(description) | ErToken::AccDescrMultiline(description) => {
                Action::SetAccDescr(description)
            }
            ErToken::Direction(direction) => Action::SetDirection(direction.value),
            ErToken::StyleKw => Action::AddCssStyles {
                entities: self.id_list()?,
                raw: self.text(ErExpectedTokens::one(RestOfLine))?,
            },
            ErToken::ClassDefKw => Action::AddClassDef {
                classes: self.id_list()?,
                raw: self.text(ErExpectedTokens::one(RestOfLine))?,
            },
            ErToken::ClassKw => Action::SetClass {
                entities: self.id_list()?,
                classes: self.id_list()?,
            },
            _ => unreachable!("ER statement starts are checked by document"),
        })
    }

    fn named_statement(
        &mut self,
        name: String,
        follow: ErExpectedTokens,
    ) -> ParseResult<ErActionList> {
        let alias = if self.accept(SquareStart)? {
            let alias = self.text(ErExpectedTokens::one(Name))?;
            self.require(ErExpectedTokens::one(SquareStop))?;
            Some(alias)
        } else {
            None
        };
        let classes = self.inline_classes()?;
        let attributes = match self.peek()? {
            Some(next) if alias.is_none() && CARDINALITY.contains(next) => {
                return self.relationship(name, classes);
            }
            Some(BlockStart) => {
                self.require(ErExpectedTokens::one(BlockStart))?;
                Some(self.attributes()?)
            }
            // EOF and end reduce a complete entity before the document checks the boundary.
            None | Some(EndKw) => None,
            Some(next) if follow.contains(next) => None,
            Some(_) => {
                let mut expected = follow.with(BlockStart);
                if classes.is_none() {
                    expected = expected.with(StyleSeparator);
                    if alias.is_none() {
                        expected = expected.with(SquareStart);
                    }
                }
                if alias.is_none() {
                    expected = expected.union(CARDINALITY);
                }
                return self.unexpected(expected);
            }
        };
        let attribute_action = attributes.map(|attributes| Action::AddAttributes {
            entity: name.clone(),
            attributes,
        });
        let class_action = classes.map(|classes| Action::SetClass {
            entities: vec![name.clone()],
            classes,
        });
        let entity = self.arena.push(Action::AddEntity { name, alias });
        Ok(self
            .arena
            .append_actions(entity, attribute_action.into_iter().chain(class_action)))
    }

    fn relationship(
        &mut self,
        a: String,
        left_classes: Option<Vec<String>>,
    ) -> ParseResult<ErActionList> {
        let card_b = self.cardinality()?;
        let rel_type = match self.require(RELATION_KIND)? {
            ErToken::Identifying => "IDENTIFYING".to_string(),
            ErToken::NonIdentifying => "NON_IDENTIFYING".to_string(),
            _ => unreachable!("ER relationship kind is identifying or non-identifying"),
        };
        let card_a = self.cardinality()?;
        let b = self.text(ErExpectedTokens::one(Name))?;
        let right_classes = match self.require(ErExpectedTokens::new(&[StyleSeparator, Colon]))? {
            ErToken::StyleSeparator => {
                let classes = self.id_list()?;
                self.require(ErExpectedTokens::one(Colon))?;
                Some(classes)
            }
            ErToken::Colon => None,
            _ => unreachable!("ER right endpoint expects classes or the role separator"),
        };
        let role = self.text(NAME_OR_STR)?;
        let left_class = left_classes.map(|classes| Action::SetClass {
            entities: vec![a.clone()],
            classes,
        });
        let right_class = right_classes.map(|classes| Action::SetClass {
            entities: vec![b.clone()],
            classes,
        });
        let relation = self.arena.push(Action::AddRelationship {
            a,
            role,
            b,
            spec: ErRelSpec {
                card_a,
                card_b,
                rel_type,
            },
        });
        Ok(self
            .arena
            .append_actions(relation, left_class.into_iter().chain(right_class)))
    }

    fn cardinality(&mut self) -> ParseResult<String> {
        Ok(match self.require(CARDINALITY)? {
            ErToken::ZeroOrOne => "ZERO_OR_ONE",
            ErToken::ZeroOrMore => "ZERO_OR_MORE",
            ErToken::OneOrMore => "ONE_OR_MORE",
            ErToken::OnlyOne => "ONLY_ONE",
            ErToken::MdParent => "MD_PARENT",
            _ => unreachable!("ER cardinality expectation contains only cardinalities"),
        }
        .to_string())
    }

    fn attributes(&mut self) -> ParseResult<Vec<ErAttribute>> {
        let mut attributes = Vec::new();
        loop {
            let ty = match self.require(ATTRIBUTE_BOUNDARY)? {
                ErToken::BlockStop => return Ok(attributes),
                ErToken::AttrWord(word) => word,
                _ => unreachable!("ER attribute block expects a type or its closing brace"),
            };
            attributes.push(self.attribute(ty)?);
        }
    }

    fn attribute(&mut self, mut ty: String) -> ParseResult<ErAttribute> {
        let name = match self.require(ErExpectedTokens::new(&[Question, AttrWord]))? {
            ErToken::Question => {
                ty.push('?');
                self.text(ErExpectedTokens::one(AttrWord))?
            }
            ErToken::AttrWord(word) => word,
            _ => unreachable!("ER attribute type expects a nullable suffix or a name"),
        };
        let mut keys = Vec::new();
        let mut comment_or_boundary = ATTRIBUTE_BOUNDARY.with(Comment).with(AttrKey);
        if self.peek()? == Some(AttrKey) {
            keys.push(self.text(ErExpectedTokens::one(AttrKey))?);
            while self.accept(Comma)? {
                keys.push(self.text(ErExpectedTokens::one(AttrKey))?);
            }
            comment_or_boundary = ATTRIBUTE_BOUNDARY.with(Comment).with(Comma);
        }
        let comment = match self.peek()? {
            Some(Comment) => self.text(ErExpectedTokens::one(Comment))?,
            Some(next) if ATTRIBUTE_BOUNDARY.contains(next) => String::new(),
            _ => return self.unexpected(comment_or_boundary),
        };
        Ok(ErAttribute {
            ty,
            name,
            keys,
            comment,
        })
    }
}
