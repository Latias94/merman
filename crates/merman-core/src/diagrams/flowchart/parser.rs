//! Predictive grammar shared by Flowchart and Swimlane.

use super::ast::StatementListBuilder;
use super::{
    Edge, FlowNodeProvenance, FlowNodeSyntax, FlowchartAst, LabeledText, LexError, LinkToken, Node,
    NodeLabelToken, StatementArena, Stmt, SubgraphBlock, SubgraphHeader, TitleKind, Tok,
    flowchart_edge_limit_error,
};
use crate::diagrams::scan::TokenStream;
use crate::parse_diagnostic::{unexpected_eof_message, unexpected_token_message};
use crate::{
    OperationCancelled, OperationControl, OperationControlResult, ParseDiagnostic,
    ParseDiagnosticSpanKind, SourceSpan,
};
use std::cell::Cell;

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

// The ordered expectations are part of the existing diagnostic text.
terminals! {
    KwGraph => "\"graph\"",
    KwFlowchart => "\"flowchart\"",
    KwFlowchartElk => "\"flowchart-elk\"",
    KwSwimlane => "\"swimlane-beta\"",
    KwSubgraph => "\"subgraph\"",
    KwEnd => "\"end\"",
    Sep => "Sep",
    AccessibilityValue(_) => "AccessibilityValue",
    Amp => "Amp",
    StyleSep => "StyleSep",
    Direction(_) => "Direction",
    DirectionStmt(_) => "DirectionStmt",
    Id(_) => "Id",
    NodeLabel(_) => "NodeLabel",
    Arrow(_) => "Arrow",
    EdgeLabel(_) => "EdgeLabel",
    SubgraphHeader(_) => "SubgraphHeader",
    StyleStmt(_) => "StyleStmt",
    ClassDefStmt(_) => "ClassDefStmt",
    ClassAssignStmt(_) => "ClassAssignStmt",
    ClickStmt(_) => "ClickStmt",
    LinkStyleStmt(_) => "LinkStyleStmt",
    EdgeId(_) => "EdgeId",
    ShapeData(_) => "ShapeData",
}

use Terminal::{
    AccessibilityValue, Amp, Arrow, ClassAssignStmt, ClassDefStmt, ClickStmt, Direction,
    DirectionStmt, EdgeId, EdgeLabel, Id, KwEnd, KwFlowchart, KwFlowchartElk, KwGraph, KwSubgraph,
    KwSwimlane, LinkStyleStmt, NodeLabel, Sep, ShapeData, StyleSep, StyleStmt,
    SubgraphHeader as SubgraphHeaderToken,
};

#[derive(Clone, Copy, Default)]
struct Expected(u32);

impl Expected {
    const fn new(terminals: &[Terminal]) -> Self {
        let mut result = Self(0);
        let mut index = 0;
        while index < terminals.len() {
            result = result.with(terminals[index]);
            index += 1;
        }
        result
    }

    const fn one(terminal: Terminal) -> Self {
        Self(1 << terminal as u8)
    }

    const fn with(self, terminal: Terminal) -> Self {
        self.union(Self::one(terminal))
    }

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    const fn contains(self, terminal: Terminal) -> bool {
        self.0 & Self::one(terminal).0 != 0
    }

    fn names(self) -> impl Iterator<Item = &'static str> {
        TERMINAL_NAMES
            .iter()
            .copied()
            .enumerate()
            .filter_map(move |(index, name)| (self.0 & (1 << index) != 0).then_some(name))
    }
}

const SEPARATORS: Expected = Expected::new(&[Sep, AccessibilityValue]);
const HEADERS: Expected = Expected::new(&[KwGraph, KwFlowchart, KwFlowchartElk, KwSwimlane]);
const STATEMENTS: Expected = Expected::new(&[
    KwSubgraph,
    DirectionStmt,
    Id,
    StyleStmt,
    ClassDefStmt,
    ClassAssignStmt,
    ClickStmt,
    LinkStyleStmt,
]);
const BODY: Expected = STATEMENTS.union(SEPARATORS);
const CONTINUE_GROUP: Expected = Expected::new(&[Amp, Arrow, EdgeId]);

/// The strict diagnostic and the editor's recovery anchor can differ for lexer errors.
#[derive(Debug)]
pub(super) struct SyntaxError {
    pub(super) diagnostic: ParseDiagnostic,
    #[cfg(feature = "editor-facts")]
    pub(super) recovery_span: SourceSpan,
}

impl SyntaxError {
    fn new(
        message: String,
        span: SourceSpan,
        kind: ParseDiagnosticSpanKind,
        _recovery_span: SourceSpan,
    ) -> Self {
        Self {
            diagnostic: ParseDiagnostic::new(message).with_span(span, kind),
            #[cfg(feature = "editor-facts")]
            recovery_span: _recovery_span,
        }
    }
}

enum Failure {
    Syntax(SyntaxError),
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
    edge_budget: &Cell<usize>,
    source_len: usize,
) -> OperationControlResult<Result<FlowchartAst, SyntaxError>> {
    let outcome = Parser {
        input: TokenStream::new(events, control),
        statements: StatementArena::default(),
        control,
        edge_budget,
        source_len,
    }
    .document();
    control.checkpoint()?;
    match outcome {
        Ok(ast) => Ok(Ok(ast)),
        Err(Failure::Syntax(error)) => Ok(Err(error)),
        Err(Failure::Cancelled(error)) => Err(error),
    }
}

#[derive(Clone, Copy, Default)]
enum BodyPosition {
    #[default]
    Start,
    AfterSeparator,
    AfterStatement,
}

#[derive(Default)]
struct Body {
    statements: StatementListBuilder,
    position: BodyPosition,
}

impl Body {
    fn expected(&self, next: Option<Terminal>, nested: bool) -> Expected {
        match (self.position, next, nested) {
            // These lookaheads completed the old statement list before failing at its boundary.
            (BodyPosition::AfterSeparator | BodyPosition::AfterStatement, None, true) => {
                Expected::one(KwEnd)
            }
            (BodyPosition::AfterSeparator | BodyPosition::AfterStatement, Some(KwEnd), false) => {
                Expected::default()
            }
            (position, _, _) => {
                let follow = match position {
                    BodyPosition::Start | BodyPosition::AfterSeparator => BODY,
                    BodyPosition::AfterStatement => SEPARATORS,
                };
                if nested { follow.with(KwEnd) } else { follow }
            }
        }
    }

    fn push(
        &mut self,
        statement: Stmt,
        arena: &mut StatementArena,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        self.position = BodyPosition::AfterStatement;
        self.statements.push(statement, arena, control)
    }
}

struct SubgraphFrame {
    header: SubgraphHeader,
    outer: Body,
}

struct Parser<'a, I: Iterator> {
    input: TokenStream<'a, I>,
    statements: StatementArena,
    control: &'a OperationControl,
    edge_budget: &'a Cell<usize>,
    source_len: usize,
}

impl<I: Iterator<Item = LexicalEvent>> Parser<'_, I> {
    fn peek(&mut self) -> ParseResult<Option<Terminal>> {
        match self.input.peek()? {
            Some(Ok((_, token, _))) => Ok(Some(Terminal::of(token))),
            Some(Err(error)) => {
                let error = error.clone();
                Err(self.user_error(error))
            }
            None => Ok(None),
        }
    }

    fn require(&mut self, expected: Expected) -> ParseResult<Token> {
        match self.input.bump()? {
            Some(Ok(token)) if expected.contains(Terminal::of(&token.1)) => Ok(token),
            event => Err(self.failed(event, expected)),
        }
    }

    fn unexpected<T>(&mut self, expected: Expected) -> ParseResult<T> {
        let event = self.input.bump()?;
        Err(self.failed(event, expected))
    }

    fn failed(&self, event: Option<LexicalEvent>, expected: Expected) -> Failure {
        use ParseDiagnosticSpanKind::{Exact, InsertionPoint};
        let (message, span, kind) = match event {
            Some(Ok((start, token, end))) => (
                unexpected_token_message(&token, expected.names()),
                SourceSpan::new(start, end),
                Exact,
            ),
            Some(Err(error)) => return self.user_error(error),
            None => {
                let offset = self.input.previous_end();
                (
                    unexpected_eof_message(expected.names()),
                    SourceSpan::new(offset, offset),
                    InsertionPoint,
                )
            }
        };
        Failure::Syntax(SyntaxError::new(message, span, kind, span))
    }

    fn user_error(&self, error: LexError) -> Failure {
        use ParseDiagnosticSpanKind::{Exact, Fallback};
        let recovery = SourceSpan::new(self.source_len, self.source_len);
        let (span, kind) = error
            .span
            .map_or((recovery, Fallback), |span| (span, Exact));
        Failure::Syntax(SyntaxError::new(error.message, span, kind, recovery))
    }

    fn header(&mut self) -> ParseResult<(String, Option<String>, SourceSpan)> {
        let (start, token, mut end) = loop {
            let token = self.require(HEADERS.union(SEPARATORS))?;
            if HEADERS.contains(Terminal::of(&token.1)) {
                break token;
            }
        };
        let keyword = match token {
            Tok::KwGraph => "graph",
            Tok::KwFlowchart => "flowchart",
            Tok::KwFlowchartElk => "flowchart-elk",
            Tok::KwSwimlane => "swimlane-beta",
            _ => unreachable!("Flowchart header requires a diagram keyword"),
        };
        let direction = match self.peek()? {
            Some(Direction) => {
                let (_, Tok::Direction(direction), direction_end) =
                    self.require(Expected::one(Direction))?
                else {
                    unreachable!();
                };
                end = direction_end;
                Some(direction)
            }
            Some(next) if !BODY.contains(next) => return self.unexpected(BODY.with(Direction)),
            _ => None,
        };
        Ok((keyword.to_string(), direction, SourceSpan::new(start, end)))
    }

    fn document(mut self) -> ParseResult<FlowchartAst> {
        let (keyword, direction, header_span) = self.header()?;
        let mut frames = Vec::new();
        let mut body = Body::default();
        loop {
            let next = self.peek()?;
            let nested = !frames.is_empty();
            if next.is_none() && !nested {
                return Ok(FlowchartAst {
                    keyword,
                    direction,
                    header_span,
                    statements: self.statements,
                    root: body.statements.finish(),
                });
            }
            let (start, token, end) = self.require(body.expected(next, nested))?;
            let statement = match token {
                Tok::Sep => {
                    body.position = BodyPosition::AfterSeparator;
                    continue;
                }
                Tok::AccessibilityValue(value) => {
                    body.push(
                        Stmt::AccessibilityValue(value),
                        &mut self.statements,
                        self.control,
                    )?;
                    body.position = BodyPosition::AfterSeparator;
                    continue;
                }
                Tok::KwSubgraph => {
                    let header = self.subgraph_header()?;
                    frames.push(SubgraphFrame {
                        header,
                        outer: std::mem::take(&mut body),
                    });
                    continue;
                }
                Tok::KwEnd => {
                    let frame = frames
                        .pop()
                        .expect("Flowchart end requires an open subgraph");
                    let statements = body.statements.finish();
                    body = frame.outer;
                    Stmt::Subgraph(SubgraphBlock {
                        header: frame.header,
                        statements,
                    })
                }
                Tok::Id(id) => self.node_statement(id, SourceSpan::new(start, end), nested)?,
                Tok::DirectionStmt(direction) => Stmt::Direction(direction.direction),
                Tok::StyleStmt(style) => Stmt::Style(style),
                Tok::ClassDefStmt(class) => Stmt::ClassDef(class),
                Tok::ClassAssignStmt(class) => Stmt::ClassAssign(class),
                Tok::ClickStmt(click) => Stmt::Click(click),
                Tok::LinkStyleStmt(style) => Stmt::LinkStyle(style),
                _ => unreachable!("Flowchart body expects a statement or a separator"),
            };
            body.push(statement, &mut self.statements, self.control)?;
        }
    }

    fn subgraph_header(&mut self) -> ParseResult<SubgraphHeader> {
        let (_, token, _) = self.require(Expected::new(&[Sep, SubgraphHeaderToken]))?;
        Ok(match token {
            Tok::Sep => SubgraphHeader::default(),
            Tok::SubgraphHeader(header) => {
                self.require(Expected::one(Sep))?;
                header
            }
            _ => unreachable!(),
        })
    }

    fn node_statement(&mut self, id: String, span: SourceSpan, nested: bool) -> ParseResult<Stmt> {
        let first = self.node(id, span, nested, true)?;
        match self.peek()? {
            Some(Amp | Arrow | EdgeId) => {
                let first = self.node_group(first, nested)?;
                self.chain(first, nested)
            }
            _ if first.label.is_none() && first.shape_data.is_some() => Ok(Stmt::ShapeData {
                target: first.id,
                target_span: first.id_span,
                yaml: first.shape_data.expect("shape data checked above"),
            }),
            _ => Ok(Stmt::Node(Box::new(first))),
        }
    }

    fn node_group(&mut self, first: Node, nested: bool) -> ParseResult<Vec<Node>> {
        let mut nodes = vec![first];
        while self.peek()? == Some(Amp) {
            self.require(Expected::one(Amp))?;
            let (start, Tok::Id(id), end) = self.require(Expected::one(Id))? else {
                unreachable!();
            };
            nodes.push(self.node(id, SourceSpan::new(start, end), nested, false)?);
        }
        Ok(nodes)
    }

    fn node(
        &mut self,
        id: String,
        span: SourceSpan,
        nested: bool,
        statement_head: bool,
    ) -> ParseResult<Node> {
        let mut next = self.peek()?;
        let label = if next == Some(NodeLabel) {
            let (_, Tok::NodeLabel(label), _) = self.require(Expected::one(NodeLabel))? else {
                unreachable!();
            };
            next = self.peek()?;
            Some(label)
        } else {
            None
        };
        let shape_data = if next == Some(ShapeData) {
            let (_, Tok::ShapeData(data), _) = self.require(Expected::one(ShapeData))? else {
                unreachable!();
            };
            next = self.peek()?;
            Some(data)
        } else {
            None
        };
        let classes = if next == Some(StyleSep) {
            self.require(Expected::one(StyleSep))?;
            let (_, Tok::Id(class), _) = self.require(Expected::one(Id))? else {
                unreachable!();
            };
            next = self.peek()?;
            vec![class]
        } else {
            Vec::new()
        };

        let mut expected = CONTINUE_GROUP;
        // Bare shape data is a statement. Adding an inline class to it requires a group or edge.
        let needs_group =
            statement_head && label.is_none() && shape_data.is_some() && !classes.is_empty();
        if !needs_group {
            expected = expected.union(SEPARATORS);
            if nested {
                expected = expected.with(KwEnd);
            }
        }
        if classes.is_empty() {
            expected = expected.with(StyleSep);
            if shape_data.is_none() {
                expected = expected.with(ShapeData);
                if label.is_none() {
                    expected = expected.with(NodeLabel);
                }
            }
        }
        // Both end and EOF can reduce a complete node or chain before the body checks scope.
        if !next.map_or(!needs_group, |token| {
            expected.contains(token) || (!needs_group && token == KwEnd)
        }) {
            return self.unexpected(expected);
        }

        let explicit = label.is_some() || shape_data.is_some() || !classes.is_empty();
        let (shape, label, label_type, label_span, label_selection) = match label {
            Some(NodeLabelToken { shape, text, .. }) => (
                Some(shape),
                Some(text.text),
                text.kind,
                text.span,
                text.selection,
            ),
            None => (None, None, TitleKind::Text, None, None),
        };
        Ok(Node {
            id,
            provenance: FlowNodeProvenance::Authored,
            syntax: if explicit {
                FlowNodeSyntax::ExplicitDefinition
            } else {
                FlowNodeSyntax::BareReference
            },
            id_span: Some(span),
            label,
            label_type,
            label_span,
            label_selection,
            shape,
            shape_data,
            icon: None,
            form: None,
            pos: None,
            img: None,
            constraint: None,
            asset_width: None,
            asset_height: None,
            styles: Vec::new(),
            classes,
            link: None,
            link_target: None,
            have_callback: false,
        })
    }

    fn chain(&mut self, first: Vec<Node>, nested: bool) -> ParseResult<Stmt> {
        let mut segments = Vec::new();
        while matches!(self.peek()?, Some(Arrow | EdgeId)) {
            segments.push(self.edge_segment(nested)?);
        }

        // Charge and expand only once the entire chain can reduce. A malformed later segment
        // therefore wins over maxEdges on an earlier segment, as it did in the old grammar.
        let mut node_groups = vec![first];
        let mut edge_groups = Vec::with_capacity(segments.len());
        for segment in segments {
            let previous = node_groups
                .last()
                .expect("Flowchart chain starts with a node group");
            edge_groups.push(self.expand_edges(previous, &segment)?);
            node_groups.push(segment.target);
        }
        Ok(Stmt::Chain {
            node_groups,
            edge_groups,
        })
    }

    fn edge_segment(&mut self, nested: bool) -> ParseResult<EdgeSegment> {
        let id = if self.peek()? == Some(EdgeId) {
            let (_, Tok::EdgeId(id), _) = self.require(Expected::one(EdgeId))? else {
                unreachable!();
            };
            Some(id)
        } else {
            None
        };
        let (_, Tok::Arrow(arrow), _) = self.require(Expected::one(Arrow))? else {
            unreachable!();
        };
        let first = self.require(Expected::new(&[Id, EdgeLabel]))?;
        let (label, (start, token, end)) = match first {
            (_, Tok::EdgeLabel(label), _) => (Some(label), self.require(Expected::one(Id))?),
            first => (None, first),
        };
        let Tok::Id(target) = token else {
            unreachable!()
        };
        let first = self.node(target, SourceSpan::new(start, end), nested, false)?;
        Ok(EdgeSegment {
            id,
            link: arrow.link,
            label,
            target: self.node_group(first, nested)?,
        })
    }

    fn expand_edges(&mut self, from: &[Node], segment: &EdgeSegment) -> ParseResult<Vec<Edge>> {
        let to = &segment.target;
        let span = from
            .first()
            .and_then(|node| node.id_span)
            .or_else(|| to.first().and_then(|node| node.id_span));
        let requested = from.len().checked_mul(to.len()).ok_or_else(|| {
            self.user_error(flowchart_edge_limit_error(
                "Flowchart edge limit exceeded: edge-group expansion exceeds the maxEdges budget",
                span,
            ))
        })?;
        let remaining = self.edge_budget.get();
        if requested > remaining {
            return Err(self.user_error(flowchart_edge_limit_error(
                format!("Flowchart edge limit exceeded: this edge group would create {requested} edges, but only {remaining} of the maxEdges budget remain"),
                span,
            )));
        }
        self.edge_budget.set(remaining - requested);

        let mut edges = Vec::with_capacity(requested);
        for source in from {
            for target in to {
                self.statements.checkpoint(self.control)?;
                // The old expansion matches IDs here, including repeated endpoints.
                let id = if source.id == from[from.len() - 1].id && target.id == to[0].id {
                    segment.id.clone()
                } else {
                    None
                };
                let label = segment.label.as_ref();
                edges.push(Edge {
                    from: source.id.clone(),
                    to: target.id.clone(),
                    id,
                    link: segment.link.clone(),
                    label: label.map(|label| label.text.clone()),
                    label_type: label.map_or(TitleKind::Text, |label| label.kind.clone()),
                    label_span: label.and_then(|label| label.span),
                    label_selection: label.and_then(|label| label.selection),
                    style: Vec::new(),
                    classes: Vec::new(),
                    interpolate: None,
                    is_user_defined_id: false,
                    animate: None,
                    animation: None,
                    step: None,
                });
            }
        }
        Ok(edges)
    }
}

struct EdgeSegment {
    id: Option<String>,
    link: LinkToken,
    label: Option<LabeledText>,
    target: Vec<Node>,
}
