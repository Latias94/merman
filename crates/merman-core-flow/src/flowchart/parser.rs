//! Predictive grammar shared by Flowchart and Swimlane.

use super::ast::{EdgeAction, NodeAction, StatementArena, StatementListBuilder};
use super::{
    FlowchartAst, FlowchartLexicalEvent, LabeledText, LexError, LinkToken, NodeLabelToken, Stmt,
    SubgraphBlock, SubgraphHeader, Tok,
};
use merman_core_preprocess::scan::TokenStream;
use merman_model::{OperationCancelled, OperationControl, OperationControlResult, SourceSpan};

type Token = (usize, Tok, usize);

macro_rules! terminals {
    ($($variant:ident $(($payload:tt))? => $name:literal),+ $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        enum Terminal { $($variant),+ }

        const TERMINAL_NAMES: &[&str] = &[$($name),+];

        impl Terminal {
            const fn variant_name(self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($variant)),+
                }
            }

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

/// Terminals accepted at a failing position, in Mermaid-compatible order.
#[derive(Debug, Clone, Copy, Default)]
pub struct FlowchartExpectedTokens(u32);

use FlowchartExpectedTokens as Expected;

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

    pub fn names(self) -> impl Iterator<Item = &'static str> {
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

/// A failure in parser-input byte coordinates. Core assigns public wording and maps the spans.
#[derive(Debug)]
pub enum FlowchartSyntaxDiagnostic {
    User {
        message: String,
        span: Option<SourceSpan>,
        recovery_span: SourceSpan,
    },
    UnexpectedToken {
        token: &'static str,
        span: SourceSpan,
        expected: FlowchartExpectedTokens,
    },
    UnexpectedEof {
        offset: usize,
        expected: FlowchartExpectedTokens,
    },
}

enum Failure {
    Syntax(FlowchartSyntaxDiagnostic),
    Cancelled(OperationCancelled),
}

impl From<OperationCancelled> for Failure {
    fn from(error: OperationCancelled) -> Self {
        Self::Cancelled(error)
    }
}

type ParseResult<T> = Result<T, Failure>;

/// Parses one strict lexical stream with the resolved maxEdges limit.
pub fn parse_flowchart(
    events: impl Iterator<Item = FlowchartLexicalEvent>,
    control: &OperationControl,
    max_edges: usize,
    source_len: usize,
) -> OperationControlResult<Result<FlowchartAst, FlowchartSyntaxDiagnostic>> {
    let outcome = Parser {
        input: TokenStream::new(events, control),
        statements: StatementArena::default(),
        control,
        remaining_edges: max_edges,
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
    remaining_edges: usize,
    source_len: usize,
}

impl<I: Iterator<Item = FlowchartLexicalEvent>> Parser<'_, I> {
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

    fn failed(&self, event: Option<FlowchartLexicalEvent>, expected: Expected) -> Failure {
        Failure::Syntax(match event {
            Some(Ok((start, token, end))) => FlowchartSyntaxDiagnostic::UnexpectedToken {
                token: Terminal::of(&token).variant_name(),
                span: SourceSpan::new(start, end),
                expected,
            },
            Some(Err(error)) => return self.user_error(error),
            None => FlowchartSyntaxDiagnostic::UnexpectedEof {
                offset: self.input.previous_end(),
                expected,
            },
        })
    }

    fn user_error(&self, error: LexError) -> Failure {
        Failure::Syntax(FlowchartSyntaxDiagnostic::User {
            message: error.message,
            span: error.span,
            recovery_span: SourceSpan::new(self.source_len, self.source_len),
        })
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

    fn node_group(&mut self, first: NodeAction, nested: bool) -> ParseResult<Vec<NodeAction>> {
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
    ) -> ParseResult<NodeAction> {
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

        let (shape, label) = label
            .map_or((None, None), |NodeLabelToken { shape, text, .. }| {
                (Some(shape), Some(text))
            });
        Ok(NodeAction {
            id,
            id_span: Some(span),
            label,
            shape,
            shape_data,
            classes,
        })
    }

    fn chain(&mut self, first: Vec<NodeAction>, nested: bool) -> ParseResult<Stmt> {
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

    fn expand_edges(
        &mut self,
        from: &[NodeAction],
        segment: &EdgeSegment,
    ) -> ParseResult<Vec<EdgeAction>> {
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
        let remaining = self.remaining_edges;
        if requested > remaining {
            return Err(self.user_error(flowchart_edge_limit_error(
                format!("Flowchart edge limit exceeded: this edge group would create {requested} edges, but only {remaining} of the maxEdges budget remain"),
                span,
            )));
        }
        self.remaining_edges = remaining - requested;

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
                edges.push(EdgeAction {
                    from: source.id.clone(),
                    to: target.id.clone(),
                    id,
                    link: segment.link.clone(),
                    label: segment.label.clone(),
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
    target: Vec<NodeAction>,
}

fn flowchart_edge_limit_error(message: impl Into<String>, span: Option<SourceSpan>) -> LexError {
    let message = message.into();
    match span {
        Some(span) => LexError::with_span(message, span),
        None => LexError::new(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flowchart::FlowchartLexer;

    fn error(source: &str, max_edges: usize) -> FlowchartSyntaxDiagnostic {
        let control = OperationControl::new();
        let lexer = FlowchartLexer::new(source).with_control(&control);
        parse_flowchart(lexer, &control, max_edges, source.len())
            .unwrap()
            .unwrap_err()
    }

    #[test]
    fn malformed_later_segment_wins_over_the_edge_budget_for_the_same_chain() {
        let source = "flowchart TB\na & b --> c & d -->";
        let FlowchartSyntaxDiagnostic::UnexpectedEof { offset, expected } = error(source, 0) else {
            panic!("expected the last segment's missing target");
        };
        assert_eq!(offset, source.len());
        assert_eq!(expected.names().collect::<Vec<_>>(), ["Id", "EdgeLabel"]);
    }

    #[test]
    fn finished_chain_charges_its_budget_before_the_next_statement() {
        let source = "flowchart TB\na & b --> c & d\nend";
        let FlowchartSyntaxDiagnostic::User {
            message,
            span,
            recovery_span,
        } = error(source, 3)
        else {
            panic!("expected the completed edge chain to exceed maxEdges first");
        };
        assert_eq!(
            message,
            "Flowchart edge limit exceeded: this edge group would create 4 edges, but only 3 of the maxEdges budget remain"
        );
        let start = "flowchart TB\n".len();
        assert_eq!(span, Some(SourceSpan::new(start, start + 1)));
        assert_eq!(recovery_span, SourceSpan::new(source.len(), source.len()));
    }
}
