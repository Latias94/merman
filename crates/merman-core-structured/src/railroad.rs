//! Railroad syntax and rule recovery for IR, EBNF, ABNF, and PEG.

use merman_model::families::railroad::{
    RailroadAstNode, RailroadDiagramModel, RailroadRepeatBound, RailroadRuleModel,
};
use merman_model::{
    MAX_DIAGRAM_NESTING_DEPTH, OperationControl, OperationControlResult, SourceSpan,
};

type Result<T> = std::result::Result<T, RailroadParseDiagnostic>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailroadDialect {
    Ir,
    Ebnf,
    Abnf,
    Peg,
}

impl RailroadDialect {
    fn is_identifier_start(self, ch: char) -> bool {
        ch.is_ascii_alphabetic() || (self != Self::Abnf && ch == '_')
    }

    fn is_identifier_continue(self, ch: char) -> bool {
        ch.is_ascii_alphanumeric() || ch == '-' || (self != Self::Abnf && ch == '_')
    }

    fn header(self) -> &'static str {
        match self {
            Self::Ir => "railroad-beta",
            Self::Ebnf => "railroad-ebnf-beta",
            Self::Abnf => "railroad-abnf-beta",
            Self::Peg => "railroad-peg-beta",
        }
    }
}

#[cfg(feature = "editor-facts")]
pub fn is_valid_rule_identifier(candidate: &str, dialect: RailroadDialect) -> bool {
    let mut chars = candidate.chars();
    chars.next().is_some_and(|first| {
        dialect.is_identifier_start(first) && chars.all(|ch| dialect.is_identifier_continue(ch))
    }) && railroad_rule_name_conflict(candidate, dialect).is_none()
}

/// A syntax failure in the byte coordinates of the supplied Railroad source.
#[derive(Debug)]
pub enum RailroadParseDiagnostic {
    Exact { span: SourceSpan, message: String },
    InsertionPoint { offset: usize, message: String },
    EbnfFallback { message: String },
}

impl RailroadParseDiagnostic {
    fn exact(message: impl Into<String>, span: SourceSpan) -> Self {
        Self::Exact {
            span,
            message: message.into(),
        }
    }

    fn insertion_point(message: impl Into<String>, offset: usize) -> Self {
        Self::InsertionPoint {
            offset,
            message: message.into(),
        }
    }

    fn start(&self) -> usize {
        match self {
            Self::Exact { span, .. } => span.start,
            Self::InsertionPoint { offset, .. } => *offset,
            Self::EbnfFallback { .. } => usize::MAX,
        }
    }
}

/// Syntax model, including successfully parsed rules after a recoverable error.
pub struct RailroadParseOutcome {
    pub model: RailroadDiagramModel,
    #[cfg(feature = "editor-facts")]
    pub trace: Option<RailroadParserTrace>,
    pub diagnostic: Option<RailroadParseDiagnostic>,
}

#[derive(Default)]
#[cfg(feature = "editor-facts")]
pub struct RailroadParserTrace {
    pub title_span: Option<SourceSpan>,
    pub acc_title_span: Option<SourceSpan>,
    pub acc_descr_span: Option<SourceSpan>,
    pub rule_name_spans: Vec<SourceSpan>,
    pub peg_predicates: Vec<RailroadPegPredicateTrace>,
}

#[cfg(feature = "editor-facts")]
pub struct RailroadPegPredicateTrace {
    pub span: SourceSpan,
    pub inner: RailroadAstNode,
}

/// Parses all tokens and retains the first syntax diagnostic and the recovered model.
///
/// Trace capture is available with the `editor-facts` feature.
pub fn parse_recovering(
    code: &str,
    dialect: RailroadDialect,
    capture_trace: bool,
    control: &OperationControl,
) -> OperationControlResult<RailroadParseOutcome> {
    #[cfg(not(feature = "editor-facts"))]
    let _ = capture_trace;

    let lexer = Lexer::new(code, dialect).tokenize_recovering(control)?;
    control.checkpoint()?;
    let mut parsed = RailroadParser::new(
        lexer.tokens,
        code.len(),
        dialect,
        #[cfg(feature = "editor-facts")]
        capture_trace,
    )
    .parse_recovering(control)?;
    parsed.diagnostic = earliest_railroad_error(lexer.first_error, parsed.diagnostic);
    Ok(parsed)
}

fn earliest_railroad_error(
    lexer: Option<RailroadParseDiagnostic>,
    parser: Option<RailroadParseDiagnostic>,
) -> Option<RailroadParseDiagnostic> {
    match (lexer, parser) {
        (Some(lexer), Some(parser)) => {
            if lexer.start() <= parser.start() {
                Some(lexer)
            } else {
                Some(parser)
            }
        }
        (Some(error), None) | (None, Some(error)) => Some(error),
        (None, None) => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommonFieldKind {
    Title,
    AccTitle,
    AccDescr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Ident(String),
    String(String),
    SpecialSequence(String),
    NumVal(String),
    Repeat(String),
    Number(String),
    Common(ParsedCommonField),
    Symbol(char),
    ColonColonEq,
    LeftArrow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    span: SourceSpan,
    selection: SourceSpan,
}

struct RailroadLexerOutcome {
    tokens: Vec<Token>,
    first_error: Option<RailroadParseDiagnostic>,
}

fn is_railroad_ir_keyword(value: &str) -> bool {
    matches!(
        value,
        "sequence"
            | "choice"
            | "optional"
            | "oneOrMore"
            | "zeroOrMore"
            | "terminal"
            | "nonterminal"
            | "special"
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RailroadRuleNameConflict {
    TitleToken,
    HeaderKeyword,
    IrConstructor,
}

fn railroad_rule_name_conflict(
    candidate: &str,
    dialect: RailroadDialect,
) -> Option<RailroadRuleNameConflict> {
    if candidate.starts_with("title") {
        return Some(RailroadRuleNameConflict::TitleToken);
    }
    if candidate == dialect.header() {
        return Some(RailroadRuleNameConflict::HeaderKeyword);
    }
    if dialect == RailroadDialect::Ir && is_railroad_ir_keyword(candidate) {
        return Some(RailroadRuleNameConflict::IrConstructor);
    }
    None
}

struct RailroadParser {
    tokens: Vec<Token>,
    pos: usize,
    input_len: usize,
    dialect: RailroadDialect,
    #[cfg(feature = "editor-facts")]
    trace: Option<RailroadParserTrace>,
}

/// Tracks owned AST depth independently from parser/group depth. A leaf has depth zero,
/// preserving the established boundary of 256 enclosing nodes.
struct RailroadConstructedNode {
    node: RailroadAstNode,
    depth: usize,
}

impl RailroadConstructedNode {
    fn leaf(node: RailroadAstNode) -> Self {
        Self { node, depth: 0 }
    }

    fn span(&self) -> SourceSpan {
        self.node.span()
    }

    fn selection(&self) -> SourceSpan {
        self.node.selection()
    }
}

#[derive(Clone, Copy)]
enum RailroadIrExpressionFrameKind {
    Optional,
    OneOrMore,
    ZeroOrMore,
    Sequence,
    Choice,
}

impl RailroadIrExpressionFrameKind {
    fn is_variadic(self) -> bool {
        matches!(self, Self::Sequence | Self::Choice)
    }

    fn name(self) -> &'static str {
        match self {
            Self::Optional => "optional",
            Self::OneOrMore => "oneOrMore",
            Self::ZeroOrMore => "zeroOrMore",
            Self::Sequence => "sequence",
            Self::Choice => "choice",
        }
    }
}

struct RailroadIrExpressionFrame {
    kind: RailroadIrExpressionFrameKind,
    start: usize,
    nesting_depth: usize,
    elements: Vec<RailroadConstructedNode>,
}

#[derive(Clone, Copy)]
enum RailroadEbnfFrameKind {
    Root,
    Group { start: usize },
    Optional { start: usize },
    Repetition { start: usize },
}

impl RailroadEbnfFrameKind {
    fn closing_symbol(self) -> Option<char> {
        match self {
            Self::Root => None,
            Self::Group { .. } => Some(')'),
            Self::Optional { .. } => Some(']'),
            Self::Repetition { .. } => Some('}'),
        }
    }

    fn closing_message(self) -> &'static str {
        match self {
            Self::Root => "expected EBNF expression",
            Self::Group { .. } => "expected ')' after EBNF group",
            Self::Optional { .. } => "expected ']' after EBNF optional group",
            Self::Repetition { .. } => "expected '}' after EBNF repetition",
        }
    }
}

enum RailroadEbnfFrameState {
    ExpectPrimary,
    Pending(RailroadConstructedNode),
    ExpectExceptionRhs {
        left: RailroadConstructedNode,
        dash_span: SourceSpan,
    },
}

struct RailroadEbnfFrame {
    kind: RailroadEbnfFrameKind,
    nesting_depth: usize,
    alternatives: Vec<Vec<RailroadConstructedNode>>,
    sequence: Vec<RailroadConstructedNode>,
    state: RailroadEbnfFrameState,
}

#[derive(Clone, Copy)]
enum RailroadAbnfFrameKind {
    Root,
    Group { start: usize },
    Optional { start: usize },
}

impl RailroadAbnfFrameKind {
    fn closing_symbol(self) -> Option<char> {
        match self {
            Self::Root => None,
            Self::Group { .. } => Some(')'),
            Self::Optional { .. } => Some(']'),
        }
    }

    fn closing_message(self) -> &'static str {
        match self {
            Self::Root => "expected ABNF element",
            Self::Group { .. } => "expected ')' after ABNF group",
            Self::Optional { .. } => "expected ']' after ABNF optional group",
        }
    }
}

struct RailroadAbnfFrame {
    kind: RailroadAbnfFrameKind,
    nesting_depth: usize,
    alternatives: Vec<Vec<RailroadConstructedNode>>,
    sequence: Vec<RailroadConstructedNode>,
    pending_repeat: Option<SpannedText>,
}

#[derive(Clone, Copy)]
enum RailroadPegFrameKind {
    Root,
    Group { start: usize },
}

impl RailroadPegFrameKind {
    fn closing_symbol(self) -> Option<char> {
        match self {
            Self::Root => None,
            Self::Group { .. } => Some(')'),
        }
    }

    fn closing_message(self) -> &'static str {
        match self {
            Self::Root => "expected PEG expression",
            Self::Group { .. } => "expected ')' after PEG group",
        }
    }
}

struct RailroadPegFrame {
    kind: RailroadPegFrameKind,
    nesting_depth: usize,
    alternatives: Vec<Vec<RailroadConstructedNode>>,
    sequence: Vec<RailroadConstructedNode>,
    pending_prefix: Option<(char, SourceSpan)>,
}

impl RailroadParser {
    fn new(
        tokens: Vec<Token>,
        input_len: usize,
        dialect: RailroadDialect,
        #[cfg(feature = "editor-facts")] capture_trace: bool,
    ) -> Self {
        Self {
            tokens,
            pos: 0,
            input_len,
            dialect,
            #[cfg(feature = "editor-facts")]
            trace: capture_trace.then(RailroadParserTrace::default),
        }
    }

    fn parse_recovering(
        mut self,
        control: &OperationControl,
    ) -> OperationControlResult<RailroadParseOutcome> {
        control.checkpoint()?;
        let mut model = RailroadDiagramModel {
            acc_title: None,
            acc_descr: None,
            title: None,
            rules: Vec::new(),
        };
        let mut first_error = None;
        if let Err(error) = self.expect_header() {
            return Ok(RailroadParseOutcome {
                model,
                #[cfg(feature = "editor-facts")]
                trace: self.trace,
                diagnostic: Some(error),
            });
        }

        while let Ok(Some(field)) = self.take_common_field() {
            control.checkpoint()?;
            match field.kind {
                CommonFieldKind::Title => {
                    model.title = Some(field.value);
                    #[cfg(feature = "editor-facts")]
                    if let Some(trace) = &mut self.trace {
                        trace.title_span = Some(field.span);
                    }
                }
                CommonFieldKind::AccTitle => {
                    model.acc_title = Some(field.value);
                    #[cfg(feature = "editor-facts")]
                    if let Some(trace) = &mut self.trace {
                        trace.acc_title_span = Some(field.span);
                    }
                }
                CommonFieldKind::AccDescr => {
                    model.acc_descr = Some(field.value);
                    #[cfg(feature = "editor-facts")]
                    if let Some(trace) = &mut self.trace {
                        trace.acc_descr_span = Some(field.span);
                    }
                }
            }
        }

        while !self.is_eof() {
            control.checkpoint()?;
            let checkpoint = self.pos;
            match self.parse_rule() {
                Ok(rule) => model.rules.push(rule),
                Err(error) => {
                    first_error.get_or_insert(error);
                    self.recover_to_next_rule(checkpoint, control)?;
                }
            }
        }

        control.checkpoint()?;
        Ok(RailroadParseOutcome {
            model,
            #[cfg(feature = "editor-facts")]
            trace: self.trace,
            diagnostic: first_error,
        })
    }

    fn recover_to_next_rule(
        &mut self,
        checkpoint: usize,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let start = checkpoint.saturating_add(1).min(self.tokens.len());
        for index in start..self.tokens.len().saturating_sub(1) {
            if (index - start).is_multiple_of(128) {
                control.checkpoint()?;
            }
            if matches!(self.tokens[index].kind, TokenKind::Ident(_))
                && self.assignment_follows(index + 1)
            {
                self.pos = index;
                return Ok(());
            }
        }
        self.pos = self.tokens.len();
        Ok(())
    }

    fn assignment_follows(&self, index: usize) -> bool {
        self.tokens
            .get(index)
            .is_some_and(|token| match self.dialect {
                RailroadDialect::Peg => matches!(token.kind, TokenKind::LeftArrow),
                RailroadDialect::Ebnf => {
                    matches!(token.kind, TokenKind::Symbol('=') | TokenKind::ColonColonEq)
                }
                RailroadDialect::Ir | RailroadDialect::Abnf => {
                    matches!(token.kind, TokenKind::Symbol('='))
                }
            })
    }

    fn expect_header(&mut self) -> Result<()> {
        let token = self.take().ok_or_else(|| {
            self.error_at_current(format!("expected {} header", self.dialect.header()))
        })?;
        let TokenKind::Ident(value) = &token.kind else {
            return Err(
                self.error_at_token(&token, format!("expected {} header", self.dialect.header()))
            );
        };
        if value != self.dialect.header() {
            return Err(
                self.error_at_token(&token, format!("expected {} header", self.dialect.header()))
            );
        }
        Ok(())
    }

    fn take_common_field(&mut self) -> Result<Option<SpannedCommonField>> {
        let Some(token) = self.peek() else {
            return Ok(None);
        };
        let TokenKind::Common(field) = &token.kind else {
            return Ok(None);
        };
        let out = SpannedCommonField {
            kind: field.kind,
            value: field.value.text.clone(),
            #[cfg(feature = "editor-facts")]
            span: field.value.selection,
        };
        self.pos += 1;
        Ok(Some(out))
    }

    fn parse_rule(&mut self) -> Result<RailroadRuleModel> {
        let name = self.expect_rule_name()?;
        match self.dialect {
            RailroadDialect::Peg => self.expect_left_arrow("expected '<-' after PEG rule name")?,
            RailroadDialect::Ebnf => {
                if !(self.take_symbol('=') || self.take_colon_colon_eq()) {
                    return Err(self.error_at_current("expected '=' or '::=' after EBNF rule name"));
                }
            }
            RailroadDialect::Ir | RailroadDialect::Abnf => {
                self.expect_symbol('=', "expected '=' after railroad rule name")?;
            }
        }

        let definition = match self.dialect {
            RailroadDialect::Ir => self.parse_ir_expression(0)?,
            RailroadDialect::Ebnf => self.parse_ebnf_choice(0)?,
            RailroadDialect::Abnf => self.parse_abnf_alternation(0)?,
            RailroadDialect::Peg => self.parse_peg_ordered_choice(0)?,
        };
        self.expect_symbol(';', "expected ';' after railroad rule definition")?;

        #[cfg(feature = "editor-facts")]
        if let Some(trace) = &mut self.trace {
            trace.rule_name_spans.push(name.selection);
        }
        Ok(RailroadRuleModel {
            name: name.text,
            definition: definition.node,
        })
    }

    fn parse_ir_expression(&mut self, nesting_depth: usize) -> Result<RailroadConstructedNode> {
        let mut frames: Vec<RailroadIrExpressionFrame> = Vec::new();
        let mut pending = None;

        loop {
            if let Some(node) = pending.take() {
                let Some(frame) = frames.last_mut() else {
                    return Ok(node);
                };
                frame.elements.push(node);

                if frame.kind.is_variadic() && self.take_symbol(',') {
                    continue;
                }

                let frame = frames.pop().expect("non-empty Railroad IR frame stack");
                pending = Some(self.finish_ir_expression_frame(frame)?);
                continue;
            }

            let expression_depth = frames
                .last()
                .map(|frame: &RailroadIrExpressionFrame| frame.nesting_depth.saturating_add(1))
                .unwrap_or(nesting_depth);
            self.ensure_nesting_depth(expression_depth)?;

            let function = self.expect_ident("expected railroad expression")?;
            self.expect_symbol('(', "expected '(' after railroad expression name")?;
            let kind = match function.text.as_str() {
                "optional" => Some(RailroadIrExpressionFrameKind::Optional),
                "oneOrMore" => Some(RailroadIrExpressionFrameKind::OneOrMore),
                "zeroOrMore" => Some(RailroadIrExpressionFrameKind::ZeroOrMore),
                "sequence" => Some(RailroadIrExpressionFrameKind::Sequence),
                "choice" => Some(RailroadIrExpressionFrameKind::Choice),
                "terminal" => {
                    let value = self.expect_string("expected string argument for terminal")?;
                    self.expect_symbol(')', "expected ')' after terminal argument")?;
                    pending = Some(RailroadConstructedNode::leaf(RailroadAstNode::Terminal {
                        value: value.text,
                        span: value.span,
                        selection: value.selection,
                    }));
                    None
                }
                "nonterminal" => {
                    let name = self.expect_string("expected string argument for nonterminal")?;
                    self.expect_symbol(')', "expected ')' after nonterminal argument")?;
                    pending = Some(RailroadConstructedNode::leaf(
                        RailroadAstNode::NonTerminal {
                            name: name.text,
                            span: name.span,
                            selection: name.selection,
                        },
                    ));
                    None
                }
                "special" => {
                    let text = self.expect_string("expected string argument for special")?;
                    self.expect_symbol(')', "expected ')' after special argument")?;
                    pending = Some(RailroadConstructedNode::leaf(RailroadAstNode::Special {
                        text: text.text,
                        span: text.span,
                        selection: text.selection,
                    }));
                    None
                }
                _ => {
                    return Err(self.error_at_span(
                        function.span,
                        format!("unsupported railroad expression: {}", function.text),
                    ));
                }
            };

            if let Some(kind) = kind {
                if kind.is_variadic() && self.check_symbol(')') {
                    return Err(self.error_at_current(format!(
                        "{} requires at least one argument",
                        kind.name()
                    )));
                }
                frames.push(RailroadIrExpressionFrame {
                    kind,
                    start: function.span.start,
                    nesting_depth: expression_depth,
                    elements: Vec::new(),
                });
            }
        }
    }

    fn finish_ir_expression_frame(
        &mut self,
        mut frame: RailroadIrExpressionFrame,
    ) -> Result<RailroadConstructedNode> {
        let end = self.expect_symbol(
            ')',
            format!("expected ')' after {} arguments", frame.kind.name()),
        )?;
        let span = SourceSpan::new(frame.start, end.span.end);
        if matches!(frame.kind, RailroadIrExpressionFrameKind::Sequence) {
            return self.collapse_sequence(frame.elements, span);
        }
        if matches!(frame.kind, RailroadIrExpressionFrameKind::Choice) {
            return self.collapse_choice(frame.elements, span);
        }
        let depth = self.next_constructed_depth(frame.elements.iter().map(|node| node.depth))?;
        let element = Box::new(
            frame
                .elements
                .pop()
                .expect("unary Railroad IR frame has one argument")
                .node,
        );
        let node = match frame.kind {
            RailroadIrExpressionFrameKind::Optional => RailroadAstNode::Optional { element, span },
            RailroadIrExpressionFrameKind::OneOrMore => RailroadAstNode::Repetition {
                element,
                min: RailroadRepeatBound::ONE,
                max: RailroadRepeatBound::INFINITY,
                separator: None,
                span,
            },
            RailroadIrExpressionFrameKind::ZeroOrMore => RailroadAstNode::Repetition {
                element,
                min: RailroadRepeatBound::ZERO,
                max: RailroadRepeatBound::INFINITY,
                separator: None,
                span,
            },
            RailroadIrExpressionFrameKind::Sequence | RailroadIrExpressionFrameKind::Choice => {
                unreachable!("variadic Railroad IR frames collapse before unary wrapping")
            }
        };
        Ok(RailroadConstructedNode { node, depth })
    }

    fn parse_ebnf_choice(&mut self, nesting_depth: usize) -> Result<RailroadConstructedNode> {
        let mut frames = vec![RailroadEbnfFrame {
            kind: RailroadEbnfFrameKind::Root,
            nesting_depth,
            alternatives: Vec::new(),
            sequence: Vec::new(),
            state: RailroadEbnfFrameState::ExpectPrimary,
        }];
        let mut completed = None;

        loop {
            if let Some(node) = completed.take() {
                let frame = frames
                    .last_mut()
                    .expect("Railroad EBNF parent frame exists for completed group");
                self.accept_ebnf_primary(frame, node)?;
                continue;
            }

            let state_is_pending = matches!(
                &frames
                    .last()
                    .expect("Railroad EBNF frame stack is non-empty")
                    .state,
                RailroadEbnfFrameState::Pending(_)
            );

            if state_is_pending {
                let wrapped_depth =
                    if self.check_symbol('?') || self.check_symbol('*') || self.check_symbol('+') {
                        let frame = frames.last().expect("Railroad EBNF frame exists");
                        let RailroadEbnfFrameState::Pending(node) = &frame.state else {
                            unreachable!("pending Railroad EBNF state was checked above")
                        };
                        self.next_constructed_depth([node.depth])?
                    } else {
                        0
                    };
                if self.take_symbol('?') {
                    let end = self.previous_end();
                    let frame = frames.last_mut().expect("Railroad EBNF frame exists");
                    let node = Self::take_ebnf_pending(frame)?;
                    frame.state = RailroadEbnfFrameState::Pending(RailroadConstructedNode {
                        depth: wrapped_depth,
                        node: RailroadAstNode::Optional {
                            span: SourceSpan::new(node.span().start, end),
                            element: Box::new(node.node),
                        },
                    });
                    continue;
                }
                if self.take_symbol('*') {
                    let end = self.previous_end();
                    let frame = frames.last_mut().expect("Railroad EBNF frame exists");
                    let node = Self::take_ebnf_pending(frame)?;
                    frame.state = RailroadEbnfFrameState::Pending(RailroadConstructedNode {
                        depth: wrapped_depth,
                        node: RailroadAstNode::Repetition {
                            span: SourceSpan::new(node.span().start, end),
                            element: Box::new(node.node),
                            min: RailroadRepeatBound::ZERO,
                            max: RailroadRepeatBound::INFINITY,
                            separator: None,
                        },
                    });
                    continue;
                }
                if self.take_symbol('+') {
                    let end = self.previous_end();
                    let frame = frames.last_mut().expect("Railroad EBNF frame exists");
                    let node = Self::take_ebnf_pending(frame)?;
                    frame.state = RailroadEbnfFrameState::Pending(RailroadConstructedNode {
                        depth: wrapped_depth,
                        node: RailroadAstNode::Repetition {
                            span: SourceSpan::new(node.span().start, end),
                            element: Box::new(node.node),
                            min: RailroadRepeatBound::ONE,
                            max: RailroadRepeatBound::INFINITY,
                            separator: None,
                        },
                    });
                    continue;
                }
                if self.take_symbol('-') {
                    let dash_span = self.previous_span();
                    let frame = frames.last_mut().expect("Railroad EBNF frame exists");
                    let left = Self::take_ebnf_pending(frame)?;
                    frame.state = RailroadEbnfFrameState::ExpectExceptionRhs { left, dash_span };
                    continue;
                }
                if self.take_symbol(',') {
                    let frame = frames.last_mut().expect("Railroad EBNF frame exists");
                    let node = Self::take_ebnf_pending(frame)?;
                    frame.sequence.push(node);
                    continue;
                }
                if self.take_symbol('|') {
                    let frame = frames.last_mut().expect("Railroad EBNF frame exists");
                    let node = Self::take_ebnf_pending(frame)?;
                    frame.sequence.push(node);
                    frame.alternatives.push(std::mem::take(&mut frame.sequence));
                    continue;
                }

                let kind = frames.last().expect("Railroad EBNF frame exists").kind;
                if let Some(closing) = kind.closing_symbol() {
                    if self.check_symbol(closing) {
                        let end = self.expect_symbol(closing, kind.closing_message())?;
                        let frame = frames.pop().expect("Railroad EBNF frame exists");
                        completed = Some(self.finish_ebnf_frame(frame, Some(end.span))?);
                        continue;
                    }
                } else if self.check_symbol(';') || self.is_eof() {
                    let frame = frames.pop().expect("Railroad EBNF root frame exists");
                    return self.finish_ebnf_frame(frame, None);
                }

                if self.is_ebnf_primary_start() {
                    let frame = frames.last_mut().expect("Railroad EBNF frame exists");
                    let node = Self::take_ebnf_pending(frame)?;
                    frame.sequence.push(node);
                    continue;
                }

                return Err(self.error_at_current(kind.closing_message()));
            }

            let (frame_depth, expects_exception_rhs) = {
                let frame = frames
                    .last()
                    .expect("Railroad EBNF frame stack is non-empty");
                (
                    frame.nesting_depth,
                    matches!(
                        &frame.state,
                        RailroadEbnfFrameState::ExpectExceptionRhs { .. }
                    ),
                )
            };
            self.ensure_nesting_depth(frame_depth)?;

            if self.take_symbol('(') {
                let start = self.previous_span().start;
                frames.push(RailroadEbnfFrame {
                    kind: RailroadEbnfFrameKind::Group { start },
                    nesting_depth: self.next_nesting_depth(frame_depth)?,
                    alternatives: Vec::new(),
                    sequence: Vec::new(),
                    state: RailroadEbnfFrameState::ExpectPrimary,
                });
                continue;
            }
            if self.take_symbol('[') {
                let start = self.previous_span().start;
                frames.push(RailroadEbnfFrame {
                    kind: RailroadEbnfFrameKind::Optional { start },
                    nesting_depth: self.next_nesting_depth(frame_depth)?,
                    alternatives: Vec::new(),
                    sequence: Vec::new(),
                    state: RailroadEbnfFrameState::ExpectPrimary,
                });
                continue;
            }
            if self.take_symbol('{') {
                let start = self.previous_span().start;
                frames.push(RailroadEbnfFrame {
                    kind: RailroadEbnfFrameKind::Repetition { start },
                    nesting_depth: self.next_nesting_depth(frame_depth)?,
                    alternatives: Vec::new(),
                    sequence: Vec::new(),
                    state: RailroadEbnfFrameState::ExpectPrimary,
                });
                continue;
            }

            let node = if let Some(value) = self.take_string() {
                RailroadAstNode::Terminal {
                    value: value.text,
                    span: value.span,
                    selection: value.selection,
                }
            } else if let Some(value) = self.take_special_sequence() {
                RailroadAstNode::Special {
                    text: value.text,
                    span: value.span,
                    selection: value.selection,
                }
            } else if let Some(value) = self.take_ident() {
                RailroadAstNode::NonTerminal {
                    name: value.text,
                    span: value.span,
                    selection: value.selection,
                }
            } else if expects_exception_rhs {
                return Err(self.error_at_current("expected EBNF primary"));
            } else {
                return Err(self.error_at_current("expected EBNF expression"));
            };
            let frame = frames.last_mut().expect("Railroad EBNF frame exists");
            self.accept_ebnf_primary(frame, RailroadConstructedNode::leaf(node))?;
        }
    }

    fn accept_ebnf_primary(
        &self,
        frame: &mut RailroadEbnfFrame,
        node: RailroadConstructedNode,
    ) -> Result<()> {
        let state = std::mem::replace(&mut frame.state, RailroadEbnfFrameState::ExpectPrimary);
        frame.state = match state {
            RailroadEbnfFrameState::ExpectPrimary => RailroadEbnfFrameState::Pending(node),
            RailroadEbnfFrameState::ExpectExceptionRhs { left, dash_span } => {
                let depth = self.next_constructed_depth([left.depth, node.depth])?;
                let span = SourceSpan::new(left.span().start, node.span().end);
                RailroadEbnfFrameState::Pending(RailroadConstructedNode {
                    depth,
                    node: RailroadAstNode::Sequence {
                        elements: vec![
                            left.node,
                            RailroadAstNode::Terminal {
                                value: "-".to_string(),
                                span: dash_span,
                                selection: dash_span,
                            },
                            node.node,
                        ],
                        span,
                    },
                })
            }
            RailroadEbnfFrameState::Pending(_) => {
                unreachable!("Railroad EBNF accepts a primary only after committing the prior term")
            }
        };
        Ok(())
    }

    fn take_ebnf_pending(frame: &mut RailroadEbnfFrame) -> Result<RailroadConstructedNode> {
        match std::mem::replace(&mut frame.state, RailroadEbnfFrameState::ExpectPrimary) {
            RailroadEbnfFrameState::Pending(node) => Ok(node),
            RailroadEbnfFrameState::ExpectPrimary
            | RailroadEbnfFrameState::ExpectExceptionRhs { .. } => {
                Err(RailroadParseDiagnostic::EbnfFallback {
                    message: "internal EBNF parser state expected a completed term".to_string(),
                })
            }
        }
    }

    fn finish_ebnf_frame(
        &mut self,
        mut frame: RailroadEbnfFrame,
        closing_span: Option<SourceSpan>,
    ) -> Result<RailroadConstructedNode> {
        let node = Self::take_ebnf_pending(&mut frame)?;
        frame.sequence.push(node);
        frame.alternatives.push(std::mem::take(&mut frame.sequence));
        let alternatives = frame
            .alternatives
            .into_iter()
            .map(|sequence| self.collapse_nonempty_sequence(sequence))
            .collect::<Result<Vec<_>>>()?;
        let element = self.collapse_nonempty_choice(alternatives)?;

        match frame.kind {
            RailroadEbnfFrameKind::Root => Ok(element),
            RailroadEbnfFrameKind::Group { start } => {
                let end = closing_span.expect("EBNF group has a closing span");
                Ok(with_outer_span(element, SourceSpan::new(start, end.end)))
            }
            RailroadEbnfFrameKind::Optional { start } => {
                let end = closing_span.expect("EBNF optional group has a closing span");
                let depth = self.next_constructed_depth([element.depth])?;
                Ok(RailroadConstructedNode {
                    depth,
                    node: RailroadAstNode::Optional {
                        element: Box::new(element.node),
                        span: SourceSpan::new(start, end.end),
                    },
                })
            }
            RailroadEbnfFrameKind::Repetition { start } => {
                let end = closing_span.expect("EBNF repetition has a closing span");
                let depth = self.next_constructed_depth([element.depth])?;
                Ok(RailroadConstructedNode {
                    depth,
                    node: RailroadAstNode::Repetition {
                        element: Box::new(element.node),
                        min: RailroadRepeatBound::ZERO,
                        max: RailroadRepeatBound::INFINITY,
                        separator: None,
                        span: SourceSpan::new(start, end.end),
                    },
                })
            }
        }
    }

    fn parse_abnf_alternation(&mut self, nesting_depth: usize) -> Result<RailroadConstructedNode> {
        let mut frames = vec![RailroadAbnfFrame {
            kind: RailroadAbnfFrameKind::Root,
            nesting_depth,
            alternatives: Vec::new(),
            sequence: Vec::new(),
            pending_repeat: None,
        }];
        let mut completed = None;

        loop {
            if let Some(node) = completed.take() {
                let frame = frames
                    .last_mut()
                    .expect("Railroad ABNF parent frame exists for completed group");
                self.append_abnf_element(frame, node)?;
                continue;
            }

            let (kind, frame_depth, has_elements, has_pending_repeat) = {
                let frame = frames
                    .last()
                    .expect("Railroad ABNF frame stack is non-empty");
                (
                    frame.kind,
                    frame.nesting_depth,
                    !frame.sequence.is_empty(),
                    frame.pending_repeat.is_some(),
                )
            };

            if let Some(closing) = kind.closing_symbol() {
                if self.check_symbol(closing) {
                    if !has_elements || has_pending_repeat {
                        return Err(self.error_at_current("expected ABNF element"));
                    }
                    let end = self.expect_symbol(closing, kind.closing_message())?;
                    let frame = frames.pop().expect("Railroad ABNF frame exists");
                    completed = Some(self.finish_abnf_frame(frame, Some(end.span))?);
                    continue;
                }
            } else if self.check_symbol(';') || self.is_eof() {
                if !has_elements || has_pending_repeat {
                    return Err(self.error_at_current("expected ABNF element"));
                }
                let frame = frames.pop().expect("Railroad ABNF root frame exists");
                return self.finish_abnf_frame(frame, None);
            }

            if self.take_symbol('/') {
                if !has_elements || has_pending_repeat {
                    return Err(self.error_at_current("expected ABNF element"));
                }
                let frame = frames.last_mut().expect("Railroad ABNF frame exists");
                frame.alternatives.push(std::mem::take(&mut frame.sequence));
                continue;
            }

            if !self.is_abnf_element_start() {
                if kind.closing_symbol().is_none() && has_elements && !has_pending_repeat {
                    let frame = frames.pop().expect("Railroad ABNF root frame exists");
                    return self.finish_abnf_frame(frame, None);
                }
                return Err(self.error_at_current(kind.closing_message()));
            }

            self.ensure_nesting_depth(frame_depth)?;
            let repeat = self.take_abnf_repeat();
            if self.take_symbol('(') {
                let start = self.previous_span().start;
                frames
                    .last_mut()
                    .expect("Railroad ABNF frame exists")
                    .pending_repeat = repeat;
                frames.push(RailroadAbnfFrame {
                    kind: RailroadAbnfFrameKind::Group { start },
                    nesting_depth: self.next_nesting_depth(frame_depth)?,
                    alternatives: Vec::new(),
                    sequence: Vec::new(),
                    pending_repeat: None,
                });
                continue;
            }
            if self.take_symbol('[') {
                let start = self.previous_span().start;
                frames
                    .last_mut()
                    .expect("Railroad ABNF frame exists")
                    .pending_repeat = repeat;
                frames.push(RailroadAbnfFrame {
                    kind: RailroadAbnfFrameKind::Optional { start },
                    nesting_depth: self.next_nesting_depth(frame_depth)?,
                    alternatives: Vec::new(),
                    sequence: Vec::new(),
                    pending_repeat: None,
                });
                continue;
            }

            let primary = if let Some(value) = self.take_string() {
                RailroadAstNode::Terminal {
                    value: value.text,
                    span: value.span,
                    selection: value.selection,
                }
            } else if let Some(value) = self.take_num_val() {
                RailroadAstNode::Terminal {
                    value: value.text,
                    span: value.span,
                    selection: value.selection,
                }
            } else if let Some(value) = self.take_ident() {
                RailroadAstNode::NonTerminal {
                    name: value.text,
                    span: value.span,
                    selection: value.selection,
                }
            } else {
                return Err(self.error_at_current("expected ABNF primary"));
            };
            let frame = frames.last_mut().expect("Railroad ABNF frame exists");
            frame.pending_repeat = repeat;
            self.append_abnf_element(frame, RailroadConstructedNode::leaf(primary))?;
        }
    }

    fn append_abnf_element(
        &self,
        frame: &mut RailroadAbnfFrame,
        primary: RailroadConstructedNode,
    ) -> Result<()> {
        let repeat = frame.pending_repeat.take();
        let element = self.apply_abnf_repeat(repeat, primary)?;
        frame.sequence.push(element);
        Ok(())
    }

    fn apply_abnf_repeat(
        &self,
        repeat: Option<SpannedText>,
        primary: RailroadConstructedNode,
    ) -> Result<RailroadConstructedNode> {
        let Some(repeat) = repeat else {
            return Ok(primary);
        };
        let (min, max) = parse_abnf_repeat_bounds(&repeat.text)
            .ok_or_else(|| self.error_at_span(repeat.span, "invalid ABNF repetition bound"))?;
        let span = SourceSpan::new(repeat.span.start, primary.span().end);
        let depth = self.next_constructed_depth([primary.depth])?;
        if min.is_zero() && max.is_one() {
            return Ok(RailroadConstructedNode {
                depth,
                node: RailroadAstNode::Optional {
                    element: Box::new(primary.node),
                    span,
                },
            });
        }
        Ok(RailroadConstructedNode {
            depth,
            node: RailroadAstNode::Repetition {
                element: Box::new(primary.node),
                min,
                max,
                separator: None,
                span,
            },
        })
    }

    fn finish_abnf_frame(
        &mut self,
        mut frame: RailroadAbnfFrame,
        closing_span: Option<SourceSpan>,
    ) -> Result<RailroadConstructedNode> {
        frame.alternatives.push(std::mem::take(&mut frame.sequence));
        let alternatives = frame
            .alternatives
            .into_iter()
            .map(|sequence| self.collapse_nonempty_sequence(sequence))
            .collect::<Result<Vec<_>>>()?;
        let element = self.collapse_nonempty_choice(alternatives)?;

        match frame.kind {
            RailroadAbnfFrameKind::Root => Ok(element),
            RailroadAbnfFrameKind::Group { start } => {
                let end = closing_span.expect("ABNF group has a closing span");
                Ok(with_outer_span(element, SourceSpan::new(start, end.end)))
            }
            RailroadAbnfFrameKind::Optional { start } => {
                let end = closing_span.expect("ABNF optional group has a closing span");
                let depth = self.next_constructed_depth([element.depth])?;
                Ok(RailroadConstructedNode {
                    depth,
                    node: RailroadAstNode::Optional {
                        element: Box::new(element.node),
                        span: SourceSpan::new(start, end.end),
                    },
                })
            }
        }
    }

    fn parse_peg_ordered_choice(
        &mut self,
        nesting_depth: usize,
    ) -> Result<RailroadConstructedNode> {
        let mut frames = vec![RailroadPegFrame {
            kind: RailroadPegFrameKind::Root,
            nesting_depth,
            alternatives: Vec::new(),
            sequence: Vec::new(),
            pending_prefix: None,
        }];
        let mut completed = None;

        loop {
            if let Some(node) = completed.take() {
                let frame = frames
                    .last_mut()
                    .expect("Railroad PEG parent frame exists for completed group");
                self.append_peg_element(frame, node)?;
                continue;
            }

            let (kind, frame_depth, has_elements, has_pending_prefix) = {
                let frame = frames
                    .last()
                    .expect("Railroad PEG frame stack is non-empty");
                (
                    frame.kind,
                    frame.nesting_depth,
                    !frame.sequence.is_empty(),
                    frame.pending_prefix.is_some(),
                )
            };

            if let Some(closing) = kind.closing_symbol() {
                if self.check_symbol(closing) {
                    if !has_elements || has_pending_prefix {
                        return Err(self.error_at_current("expected PEG expression"));
                    }
                    let end = self.expect_symbol(closing, kind.closing_message())?;
                    let frame = frames.pop().expect("Railroad PEG frame exists");
                    completed = Some(self.finish_peg_frame(frame, Some(end.span))?);
                    continue;
                }
            } else if self.check_symbol(';') || self.is_eof() {
                if !has_elements || has_pending_prefix {
                    return Err(self.error_at_current("expected PEG expression"));
                }
                let frame = frames.pop().expect("Railroad PEG root frame exists");
                return self.finish_peg_frame(frame, None);
            }

            if self.take_symbol('/') {
                if !has_elements || has_pending_prefix {
                    return Err(self.error_at_current("expected PEG expression"));
                }
                let frame = frames.last_mut().expect("Railroad PEG frame exists");
                frame.alternatives.push(std::mem::take(&mut frame.sequence));
                continue;
            }

            if !self.is_peg_prefix_start() {
                if kind.closing_symbol().is_none() && has_elements && !has_pending_prefix {
                    let frame = frames.pop().expect("Railroad PEG root frame exists");
                    return self.finish_peg_frame(frame, None);
                }
                return Err(self.error_at_current(kind.closing_message()));
            }

            self.ensure_nesting_depth(frame_depth)?;
            let prefix = if self.take_symbol('&') {
                Some(('&', self.previous_span()))
            } else if self.take_symbol('!') {
                Some(('!', self.previous_span()))
            } else {
                None
            };

            if self.take_symbol('(') {
                let start = self.previous_span().start;
                frames
                    .last_mut()
                    .expect("Railroad PEG frame exists")
                    .pending_prefix = prefix;
                frames.push(RailroadPegFrame {
                    kind: RailroadPegFrameKind::Group { start },
                    nesting_depth: self.next_nesting_depth(frame_depth)?,
                    alternatives: Vec::new(),
                    sequence: Vec::new(),
                    pending_prefix: None,
                });
                continue;
            }

            let primary = if let Some(value) = self.take_string() {
                RailroadAstNode::Terminal {
                    value: value.text,
                    span: value.span,
                    selection: value.selection,
                }
            } else if let Some(value) = self.take_ident() {
                RailroadAstNode::NonTerminal {
                    name: value.text,
                    span: value.span,
                    selection: value.selection,
                }
            } else if self.take_symbol('.') {
                let span = self.previous_span();
                RailroadAstNode::Special {
                    text: ".".to_string(),
                    span,
                    selection: span,
                }
            } else {
                return Err(self.error_at_current("expected PEG primary"));
            };
            let frame = frames.last_mut().expect("Railroad PEG frame exists");
            frame.pending_prefix = prefix;
            self.append_peg_element(frame, RailroadConstructedNode::leaf(primary))?;
        }
    }

    fn append_peg_element(
        &mut self,
        frame: &mut RailroadPegFrame,
        primary: RailroadConstructedNode,
    ) -> Result<()> {
        let suffix = self.apply_peg_suffix(primary)?;
        let element = self.apply_peg_prefix(frame.pending_prefix.take(), suffix);
        frame.sequence.push(element);
        Ok(())
    }

    fn apply_peg_suffix(
        &mut self,
        primary: RailroadConstructedNode,
    ) -> Result<RailroadConstructedNode> {
        let depth = if self.check_symbol('?') || self.check_symbol('*') || self.check_symbol('+') {
            self.next_constructed_depth([primary.depth])?
        } else {
            return Ok(primary);
        };
        if self.take_symbol('?') {
            return Ok(RailroadConstructedNode {
                depth,
                node: RailroadAstNode::Optional {
                    span: SourceSpan::new(primary.span().start, self.previous_end()),
                    element: Box::new(primary.node),
                },
            });
        }
        if self.take_symbol('*') {
            return Ok(RailroadConstructedNode {
                depth,
                node: RailroadAstNode::Repetition {
                    span: SourceSpan::new(primary.span().start, self.previous_end()),
                    element: Box::new(primary.node),
                    min: RailroadRepeatBound::ZERO,
                    max: RailroadRepeatBound::INFINITY,
                    separator: None,
                },
            });
        }
        if self.take_symbol('+') {
            return Ok(RailroadConstructedNode {
                depth,
                node: RailroadAstNode::Repetition {
                    span: SourceSpan::new(primary.span().start, self.previous_end()),
                    element: Box::new(primary.node),
                    min: RailroadRepeatBound::ONE,
                    max: RailroadRepeatBound::INFINITY,
                    separator: None,
                },
            });
        }
        unreachable!("a supported Railroad PEG suffix was checked before wrapping")
    }

    fn apply_peg_prefix(
        &mut self,
        prefix: Option<(char, SourceSpan)>,
        suffix: RailroadConstructedNode,
    ) -> RailroadConstructedNode {
        let Some((operator, span)) = prefix else {
            return suffix;
        };
        let label = format!("{operator}{}", node_to_label(&suffix.node));
        let predicate_span = SourceSpan::new(span.start, suffix.span().end);
        #[cfg(feature = "editor-facts")]
        if let Some(trace) = &mut self.trace {
            trace.peg_predicates.push(RailroadPegPredicateTrace {
                span: predicate_span,
                inner: suffix.node.clone(),
            });
        }
        RailroadConstructedNode::leaf(RailroadAstNode::Special {
            text: label,
            span: predicate_span,
            selection: SourceSpan::new(span.start, suffix.selection().end),
        })
    }

    fn finish_peg_frame(
        &mut self,
        mut frame: RailroadPegFrame,
        closing_span: Option<SourceSpan>,
    ) -> Result<RailroadConstructedNode> {
        frame.alternatives.push(std::mem::take(&mut frame.sequence));
        let alternatives = frame
            .alternatives
            .into_iter()
            .map(|sequence| self.collapse_nonempty_sequence(sequence))
            .collect::<Result<Vec<_>>>()?;
        let element = self.collapse_nonempty_choice(alternatives)?;

        match frame.kind {
            RailroadPegFrameKind::Root => Ok(element),
            RailroadPegFrameKind::Group { start } => {
                let end = closing_span.expect("PEG group has a closing span");
                Ok(with_outer_span(element, SourceSpan::new(start, end.end)))
            }
        }
    }

    fn is_ebnf_primary_start(&self) -> bool {
        matches!(
            self.peek().map(|token| &token.kind),
            Some(TokenKind::String(_))
                | Some(TokenKind::SpecialSequence(_))
                | Some(TokenKind::Ident(_))
                | Some(TokenKind::Symbol('(' | '[' | '{'))
        )
    }

    fn is_abnf_element_start(&self) -> bool {
        matches!(
            self.peek().map(|token| &token.kind),
            Some(TokenKind::Repeat(_))
                | Some(TokenKind::Number(_))
                | Some(TokenKind::String(_))
                | Some(TokenKind::NumVal(_))
                | Some(TokenKind::Ident(_))
                | Some(TokenKind::Symbol('(' | '['))
        )
    }

    fn is_peg_prefix_start(&self) -> bool {
        matches!(
            self.peek().map(|token| &token.kind),
            Some(TokenKind::Symbol('&' | '!'))
                | Some(TokenKind::String(_))
                | Some(TokenKind::Ident(_))
                | Some(TokenKind::Symbol('(' | '.'))
        )
    }

    fn take_abnf_repeat(&mut self) -> Option<SpannedText> {
        let token = self.peek()?;
        match &token.kind {
            TokenKind::Repeat(value) | TokenKind::Number(value) => {
                let out = SpannedText {
                    text: value.clone(),
                    span: token.span,
                    selection: token.selection,
                };
                self.pos += 1;
                Some(out)
            }
            _ => None,
        }
    }

    fn expect_ident(&mut self, message: impl Into<String>) -> Result<SpannedText> {
        self.take_ident()
            .ok_or_else(|| self.error_at_current(message))
    }

    fn expect_rule_name(&mut self) -> Result<SpannedText> {
        let name = self.expect_ident("expected railroad rule name")?;
        if let Some(conflict) = railroad_rule_name_conflict(&name.text, self.dialect) {
            let reason = match conflict {
                RailroadRuleNameConflict::TitleToken => "is tokenized as railroad title metadata",
                RailroadRuleNameConflict::HeaderKeyword => "is the railroad dialect header",
                RailroadRuleNameConflict::IrConstructor => "is reserved for railroad expressions",
            };
            return Err(self.error_at_span(name.span, format!("`{}` {reason}", name.text)));
        }
        Ok(name)
    }

    fn ensure_nesting_depth(&self, nesting_depth: usize) -> Result<()> {
        if nesting_depth > MAX_DIAGRAM_NESTING_DEPTH {
            return Err(self.error_at_current(format!(
                "railroad nesting depth exceeds {MAX_DIAGRAM_NESTING_DEPTH}"
            )));
        }
        Ok(())
    }

    fn next_nesting_depth(&self, nesting_depth: usize) -> Result<usize> {
        let next = nesting_depth.saturating_add(1);
        self.ensure_nesting_depth(next)?;
        Ok(next)
    }

    fn next_constructed_depth(&self, depths: impl IntoIterator<Item = usize>) -> Result<usize> {
        let next = depths.into_iter().max().unwrap_or(0).saturating_add(1);
        self.ensure_nesting_depth(next)?;
        Ok(next)
    }

    fn collapse_sequence(
        &self,
        elements: Vec<RailroadConstructedNode>,
        span: SourceSpan,
    ) -> Result<RailroadConstructedNode> {
        if elements.len() == 1 {
            return Ok(elements.into_iter().next().expect("one element"));
        }
        let depth = self.next_constructed_depth(elements.iter().map(|node| node.depth))?;
        Ok(RailroadConstructedNode {
            depth,
            node: RailroadAstNode::Sequence {
                elements: elements.into_iter().map(|node| node.node).collect(),
                span,
            },
        })
    }

    fn collapse_choice(
        &self,
        alternatives: Vec<RailroadConstructedNode>,
        span: SourceSpan,
    ) -> Result<RailroadConstructedNode> {
        if alternatives.len() == 1 {
            return Ok(alternatives.into_iter().next().expect("one alternative"));
        }
        let depth = self.next_constructed_depth(alternatives.iter().map(|node| node.depth))?;
        Ok(RailroadConstructedNode {
            depth,
            node: RailroadAstNode::Choice {
                alternatives: alternatives.into_iter().map(|node| node.node).collect(),
                span,
            },
        })
    }

    fn collapse_nonempty_sequence(
        &self,
        elements: Vec<RailroadConstructedNode>,
    ) -> Result<RailroadConstructedNode> {
        let start = elements
            .first()
            .expect("Railroad sequence has an element")
            .span()
            .start;
        let end = elements
            .last()
            .expect("Railroad sequence has an element")
            .span()
            .end;
        self.collapse_sequence(elements, SourceSpan::new(start, end))
    }

    fn collapse_nonempty_choice(
        &self,
        alternatives: Vec<RailroadConstructedNode>,
    ) -> Result<RailroadConstructedNode> {
        let start = alternatives
            .first()
            .expect("Railroad choice has an alternative")
            .span()
            .start;
        let end = alternatives
            .last()
            .expect("Railroad choice has an alternative")
            .span()
            .end;
        self.collapse_choice(alternatives, SourceSpan::new(start, end))
    }

    fn take_ident(&mut self) -> Option<SpannedText> {
        let token = self.peek()?;
        let TokenKind::Ident(value) = &token.kind else {
            return None;
        };
        let out = SpannedText {
            text: value.clone(),
            span: token.span,
            selection: token.selection,
        };
        self.pos += 1;
        Some(out)
    }

    fn expect_string(&mut self, message: impl Into<String>) -> Result<SpannedText> {
        self.take_string()
            .ok_or_else(|| self.error_at_current(message))
    }

    fn take_string(&mut self) -> Option<SpannedText> {
        let token = self.peek()?;
        let TokenKind::String(value) = &token.kind else {
            return None;
        };
        let out = SpannedText {
            text: value.clone(),
            span: token.span,
            selection: token.selection,
        };
        self.pos += 1;
        Some(out)
    }

    fn take_special_sequence(&mut self) -> Option<SpannedText> {
        let token = self.peek()?;
        let TokenKind::SpecialSequence(value) = &token.kind else {
            return None;
        };
        let out = SpannedText {
            text: value.clone(),
            span: token.span,
            selection: token.selection,
        };
        self.pos += 1;
        Some(out)
    }

    fn take_num_val(&mut self) -> Option<SpannedText> {
        let token = self.peek()?;
        let TokenKind::NumVal(value) = &token.kind else {
            return None;
        };
        let out = SpannedText {
            text: value.clone(),
            span: token.span,
            selection: token.selection,
        };
        self.pos += 1;
        Some(out)
    }

    fn expect_symbol(&mut self, symbol: char, message: impl Into<String>) -> Result<Token> {
        if self.take_symbol(symbol) {
            Ok(self.previous_token().expect("previous token").clone())
        } else {
            Err(self.error_at_current(message))
        }
    }

    fn take_symbol(&mut self, symbol: char) -> bool {
        if self.check_symbol(symbol) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn check_symbol(&self, symbol: char) -> bool {
        matches!(
            self.peek().map(|token| &token.kind),
            Some(TokenKind::Symbol(ch)) if *ch == symbol
        )
    }

    fn take_colon_colon_eq(&mut self) -> bool {
        if matches!(
            self.peek().map(|token| &token.kind),
            Some(TokenKind::ColonColonEq)
        ) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_left_arrow(&mut self, message: impl Into<String>) -> Result<()> {
        if matches!(
            self.peek().map(|token| &token.kind),
            Some(TokenKind::LeftArrow)
        ) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error_at_current(message))
        }
    }

    fn take(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos)?.clone();
        self.pos += 1;
        Some(token)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn previous_token(&self) -> Option<&Token> {
        self.pos.checked_sub(1).and_then(|pos| self.tokens.get(pos))
    }

    fn previous_span(&self) -> SourceSpan {
        self.previous_token()
            .map(|token| token.span)
            .unwrap_or_else(|| SourceSpan::new(self.input_len, self.input_len))
    }

    fn previous_end(&self) -> usize {
        self.previous_span().end
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn error_at_current(&self, message: impl Into<String>) -> RailroadParseDiagnostic {
        if let Some(token) = self.peek() {
            self.error_at_token(token, message)
        } else {
            RailroadParseDiagnostic::insertion_point(message, self.input_len)
        }
    }

    fn error_at_token(&self, token: &Token, message: impl Into<String>) -> RailroadParseDiagnostic {
        RailroadParseDiagnostic::exact(message, token.span)
    }

    fn error_at_span(
        &self,
        span: SourceSpan,
        message: impl Into<String>,
    ) -> RailroadParseDiagnostic {
        RailroadParseDiagnostic::exact(message, span)
    }
}

#[derive(Debug, Clone)]
struct SpannedCommonField {
    kind: CommonFieldKind,
    value: String,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpannedText {
    text: String,
    span: SourceSpan,
    selection: SourceSpan,
}

struct Lexer<'a> {
    input: &'a str,
    dialect: RailroadDialect,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str, dialect: RailroadDialect) -> Self {
        Self {
            input,
            dialect,
            pos: 0,
        }
    }

    fn tokenize_recovering(
        mut self,
        control: &OperationControl,
    ) -> OperationControlResult<RailroadLexerOutcome> {
        let mut tokens = Vec::new();
        let mut first_error = None;
        loop {
            control.checkpoint()?;
            match self.skip_trivia(control) {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => {
                    first_error.get_or_insert(error);
                    if self.is_eof() {
                        break;
                    }
                    self.advance_char();
                    continue;
                }
            }
            if self.is_eof() {
                break;
            }
            match self.take_common_field() {
                Ok(Some(token)) => {
                    tokens.push(token);
                    continue;
                }
                Ok(None) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                    if self.is_eof() {
                        break;
                    }
                    self.advance_char();
                    continue;
                }
            }
            match self.take_string() {
                Ok(Some(token)) => {
                    tokens.push(token);
                    continue;
                }
                Ok(None) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                    if self.is_eof() {
                        break;
                    }
                    self.advance_char();
                    continue;
                }
            }
            if self.dialect == RailroadDialect::Ebnf
                && let Some(token) = self.take_ebnf_special_sequence()
            {
                tokens.push(token);
                continue;
            }
            if self.dialect == RailroadDialect::Abnf {
                if let Some(token) = self.take_abnf_num_val() {
                    tokens.push(token);
                    continue;
                }
                if let Some(token) = self.take_abnf_repeat_or_number() {
                    tokens.push(token);
                    continue;
                }
            }
            if let Some(token) = self.take_identifier() {
                tokens.push(token);
                continue;
            }
            if let Some(token) = self.take_compound_symbol() {
                tokens.push(token);
                continue;
            }
            if let Some(token) = self.take_single_symbol() {
                tokens.push(token);
                continue;
            }

            let start = self.pos;
            let ch = self.current_char().expect("not eof");
            first_error.get_or_insert_with(|| {
                RailroadParseDiagnostic::exact(
                    format!("unexpected railroad token `{ch}`"),
                    SourceSpan::new(start, start + ch.len_utf8()),
                )
            });
            self.advance_char();
        }

        control.checkpoint()?;
        Ok(RailroadLexerOutcome {
            tokens,
            first_error,
        })
    }

    fn skip_trivia(&mut self, control: &OperationControl) -> Result<bool> {
        loop {
            let before = self.pos;
            let mut last_checkpoint = self.pos;
            while self.current_char().is_some_and(char::is_whitespace) {
                self.advance_char();
                if self.pos.saturating_sub(last_checkpoint) >= 4096 {
                    if control.is_cancelled() {
                        return Ok(false);
                    }
                    last_checkpoint = self.pos;
                }
            }
            if control.is_cancelled() {
                return Ok(false);
            }

            if self.starts_with("%%{") {
                if let Some(end) = self.remaining().find("}%%") {
                    self.pos += end + "}%%".len();
                    continue;
                }
                self.pos = self.input.len();
                continue;
            }
            if self.starts_with("%%") {
                self.skip_to_line_end();
                continue;
            }
            if matches!(self.dialect, RailroadDialect::Ir | RailroadDialect::Ebnf)
                && self.starts_with("/*")
            {
                let result = self.skip_until("*/", "unterminated railroad block comment");
                result?;
                continue;
            }
            if self.dialect == RailroadDialect::Ebnf && self.starts_with("(*") {
                let result = self.skip_until("*)", "unterminated EBNF comment");
                result?;
                continue;
            }
            if self.dialect == RailroadDialect::Peg && self.starts_with("#") {
                self.skip_to_line_end();
                continue;
            }
            if self.dialect == RailroadDialect::Abnf && self.abnf_semicolon_starts_comment() {
                self.skip_to_line_end();
                continue;
            }
            if self.skip_yaml_fence() {
                continue;
            }

            if self.pos == before {
                break;
            }
        }
        Ok(!self.is_eof())
    }

    fn take_common_field(&mut self) -> Result<Option<Token>> {
        let start = self.pos;
        let rest = self.remaining();
        let trimmed = rest.trim_start_matches([' ', '\t']);
        let leading = rest.len() - trimmed.len();
        if leading > 0 && rest[..leading].contains(['\r', '\n']) {
            return Ok(None);
        }
        let token_start = start + leading;
        let title_end = token_start + "title".len();
        if self.input[token_start..].starts_with("title")
            && self.input[title_end..]
                .chars()
                .next()
                .is_some_and(|ch| !matches!(ch, ' ' | '\t' | '\r' | '\n'))
        {
            self.pos = title_end;
            let span = SourceSpan::new(token_start, title_end);
            return Ok(Some(Token {
                kind: TokenKind::Common(ParsedCommonField {
                    kind: CommonFieldKind::Title,
                    value: SpannedText {
                        text: String::new(),
                        span: SourceSpan::new(title_end, title_end),
                        selection: SourceSpan::new(title_end, title_end),
                    },
                }),
                span,
                selection: SourceSpan::new(title_end, title_end),
            }));
        }
        let line_end = self.input[token_start..]
            .find(['\r', '\n'])
            .map(|rel| token_start + rel)
            .unwrap_or(self.input.len());
        let line = &self.input[token_start..line_end];

        if let Some(field) = parse_common_field_line(line, token_start, self.dialect) {
            self.pos = line_end;
            let selection = field.value.selection;
            return Ok(Some(Token {
                kind: TokenKind::Common(field),
                span: SourceSpan::new(token_start, line_end),
                selection,
            }));
        }

        if line.trim_start().starts_with("accDescr")
            && line.contains('{')
            && !line.contains('}')
            && let Some(end_rel) = self.input[line_end..].find('}')
        {
            let end = line_end + end_rel + 1;
            let full = &self.input[token_start..end];
            if let Some(field) = parse_common_field_block(full, token_start) {
                self.pos = end;
                let selection = field.value.selection;
                return Ok(Some(Token {
                    kind: TokenKind::Common(field),
                    span: SourceSpan::new(token_start, end),
                    selection,
                }));
            }
        }

        Ok(None)
    }

    fn take_string(&mut self) -> Result<Option<Token>> {
        let Some(quote) = self.current_char() else {
            return Ok(None);
        };
        if !matches!(quote, '"' | '\'') {
            return Ok(None);
        }
        if self.dialect == RailroadDialect::Abnf && quote != '"' {
            return Ok(None);
        }

        let start = self.pos;
        self.advance_char();
        let content_start = self.pos;
        let mut value = String::new();
        let mut escaped = false;
        while let Some(ch) = self.current_char() {
            let ch_start = self.pos;
            self.advance_char();
            if self.dialect != RailroadDialect::Abnf && is_railroad_string_line_terminator(ch) {
                return Err(RailroadParseDiagnostic::exact(
                    "physical line breaks are not allowed in railroad string literals",
                    SourceSpan::new(ch_start, self.pos),
                ));
            }
            if self.dialect != RailroadDialect::Abnf && escaped {
                match ch {
                    'n' => value.push('\n'),
                    'r' => value.push('\r'),
                    't' => value.push('\t'),
                    other => value.push(other),
                }
                escaped = false;
                continue;
            }
            if self.dialect != RailroadDialect::Abnf && ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                return Ok(Some(Token {
                    kind: TokenKind::String(value),
                    span: SourceSpan::new(start, self.pos),
                    selection: SourceSpan::new(content_start, ch_start),
                }));
            }
            value.push(ch);
        }

        Err(RailroadParseDiagnostic::insertion_point(
            "unterminated railroad string literal",
            start,
        ))
    }

    fn take_ebnf_special_sequence(&mut self) -> Option<Token> {
        if !self.starts_with("?") {
            return None;
        }
        let start = self.pos;
        let rest = &self.remaining()[1..];
        let end_rel = rest.find('?')?;
        let content = &rest[..end_rel];
        if content.contains(';') || content.trim().is_empty() {
            return None;
        }
        self.pos += 1 + end_rel + 1;
        Some(Token {
            kind: TokenKind::SpecialSequence(content.trim().to_string()),
            span: SourceSpan::new(start, self.pos),
            selection: SourceSpan::new(start + 1, start + 1 + content.len()),
        })
    }

    fn take_abnf_num_val(&mut self) -> Option<Token> {
        if !self.starts_with("%") {
            return None;
        }
        let start = self.pos;
        let bytes = self.input.as_bytes();
        let mut pos = start + 1;
        let base = *bytes.get(pos)?;
        if !matches!(base, b'x' | b'X' | b'd' | b'D' | b'b' | b'B') {
            return None;
        }
        pos += 1;
        let digits_start = pos;
        while pos < bytes.len() && bytes[pos].is_ascii_hexdigit() {
            pos += 1;
        }
        if pos == digits_start {
            return None;
        }
        while pos < bytes.len() && matches!(bytes[pos], b'-' | b'.') {
            pos += 1;
            let chunk_start = pos;
            while pos < bytes.len() && bytes[pos].is_ascii_hexdigit() {
                pos += 1;
            }
            if pos == chunk_start {
                return None;
            }
        }
        self.pos = pos;
        Some(Token {
            kind: TokenKind::NumVal(self.input[start..pos].to_string()),
            span: SourceSpan::new(start, pos),
            selection: SourceSpan::new(start, pos),
        })
    }

    fn take_abnf_repeat_or_number(&mut self) -> Option<Token> {
        let start = self.pos;
        let bytes = self.input.as_bytes();
        let mut pos = start;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }

        if pos < bytes.len() && bytes[pos] == b'*' {
            pos += 1;
            while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
            self.pos = pos;
            return Some(Token {
                kind: TokenKind::Repeat(self.input[start..pos].to_string()),
                span: SourceSpan::new(start, pos),
                selection: SourceSpan::new(start, pos),
            });
        }

        if pos > start {
            self.pos = pos;
            return Some(Token {
                kind: TokenKind::Number(self.input[start..pos].to_string()),
                span: SourceSpan::new(start, pos),
                selection: SourceSpan::new(start, pos),
            });
        }

        if bytes.get(start) == Some(&b'*') {
            self.pos += 1;
            while self.pos < bytes.len() && bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
            return Some(Token {
                kind: TokenKind::Repeat(self.input[start..self.pos].to_string()),
                span: SourceSpan::new(start, self.pos),
                selection: SourceSpan::new(start, self.pos),
            });
        }

        None
    }

    fn take_identifier(&mut self) -> Option<Token> {
        let start = self.pos;
        let first = self.current_char()?;
        if !self.dialect.is_identifier_start(first) {
            return None;
        }

        self.advance_char();
        while let Some(ch) = self.current_char() {
            if !self.dialect.is_identifier_continue(ch) {
                break;
            }
            self.advance_char();
        }

        Some(Token {
            kind: TokenKind::Ident(self.input[start..self.pos].to_string()),
            span: SourceSpan::new(start, self.pos),
            selection: SourceSpan::new(start, self.pos),
        })
    }

    fn take_compound_symbol(&mut self) -> Option<Token> {
        let start = self.pos;
        if self.starts_with("::=") {
            self.pos += 3;
            return Some(Token {
                kind: TokenKind::ColonColonEq,
                span: SourceSpan::new(start, self.pos),
                selection: SourceSpan::new(start, self.pos),
            });
        }
        if self.starts_with("<-") {
            self.pos += 2;
            return Some(Token {
                kind: TokenKind::LeftArrow,
                span: SourceSpan::new(start, self.pos),
                selection: SourceSpan::new(start, self.pos),
            });
        }
        None
    }

    fn take_single_symbol(&mut self) -> Option<Token> {
        let ch = self.current_char()?;
        let symbols = match self.dialect {
            RailroadDialect::Ir => "=;,()",
            RailroadDialect::Ebnf => "=;,|()[]{}?*+-",
            RailroadDialect::Abnf => "=;/()[]",
            RailroadDialect::Peg => ";/()&!?.*+",
        };
        if !symbols.contains(ch) {
            return None;
        }

        let start = self.pos;
        self.advance_char();
        Some(Token {
            kind: TokenKind::Symbol(ch),
            span: SourceSpan::new(start, self.pos),
            selection: SourceSpan::new(start, self.pos),
        })
    }

    fn skip_until(&mut self, needle: &str, message: &'static str) -> Result<()> {
        let start = self.pos;
        let Some(end) = self.remaining().find(needle) else {
            self.pos = self.input.len();
            return Err(RailroadParseDiagnostic::insertion_point(message, start));
        };
        self.pos += end + needle.len();
        Ok(())
    }

    fn skip_to_line_end(&mut self) {
        let rel = self
            .remaining()
            .find(['\r', '\n'])
            .unwrap_or(self.remaining().len());
        self.pos += rel;
    }

    fn abnf_semicolon_starts_comment(&self) -> bool {
        if !self.starts_with(";") {
            return false;
        }
        let line_start = self.input[..self.pos]
            .rfind(['\r', '\n'])
            .map(|idx| idx + 1)
            .unwrap_or(0);
        self.input[line_start..self.pos].trim().is_empty()
    }

    fn skip_yaml_fence(&mut self) -> bool {
        if !self.starts_with("---") || !self.at_line_start_after_indent() {
            return false;
        }

        let mut cursor = self.pos + "---".len();
        while self.input[cursor..]
            .chars()
            .next()
            .is_some_and(|ch| matches!(ch, ' ' | '\t'))
        {
            cursor += self.input[cursor..]
                .chars()
                .next()
                .expect("checked above")
                .len_utf8();
        }
        if self.input[cursor..].starts_with("\r\n") {
            cursor += 2;
        } else if self.input[cursor..].starts_with('\n') {
            cursor += 1;
        } else {
            return false;
        }

        loop {
            let line_end = self.input[cursor..]
                .find(['\r', '\n'])
                .map(|relative| cursor + relative)
                .unwrap_or(self.input.len());
            if self.input[cursor..].starts_with("---") {
                let marker_end = cursor + "---".len();
                if self.input[marker_end..]
                    .chars()
                    .next()
                    .is_none_or(|ch| ch.is_whitespace())
                {
                    self.pos = marker_end;
                    return true;
                }
            }
            if line_end == self.input.len() {
                return false;
            }
            cursor = line_end
                + if self.input[line_end..].starts_with("\r\n") {
                    2
                } else {
                    1
                };
        }
    }

    fn at_line_start_after_indent(&self) -> bool {
        let line_start = self.input[..self.pos]
            .rfind(['\r', '\n'])
            .map(|idx| idx + 1)
            .unwrap_or(0);
        self.input[line_start..self.pos]
            .chars()
            .all(|ch| matches!(ch, ' ' | '\t'))
    }

    fn starts_with(&self, literal: &str) -> bool {
        self.remaining().starts_with(literal)
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn current_char(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn advance_char(&mut self) {
        if let Some(ch) = self.current_char() {
            self.pos += ch.len_utf8();
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedCommonField {
    kind: CommonFieldKind,
    value: SpannedText,
}

fn parse_common_field_line(
    line: &str,
    line_start: usize,
    dialect: RailroadDialect,
) -> Option<ParsedCommonField> {
    let stripped = strip_inline_comment_aware(line);
    parse_title_spanned(stripped, line_start, dialect)
        .or_else(|| parse_acc_title_spanned(stripped, line_start))
        .or_else(|| parse_acc_descr_spanned(stripped, line_start))
}

fn parse_common_field_block(block: &str, block_start: usize) -> Option<ParsedCommonField> {
    let trimmed = block.trim_start();
    let leading = block.len() - trimmed.len();
    let keyword_start = block_start + leading;
    let keyword_end = keyword_start + "accDescr".len();
    let after_keyword = trimmed.strip_prefix("accDescr")?;
    let rest = after_keyword.trim_start();
    let rest_start = keyword_end + (after_keyword.len() - rest.len());
    let body = rest.strip_prefix('{')?;
    let opening = SourceSpan::new(rest_start, rest_start + 1);
    let close_rel = body.find('}')?;
    let raw = &body[..close_rel];
    let (_, value_span) = trimmed_value_span(raw, opening.end);
    Some(ParsedCommonField {
        kind: CommonFieldKind::AccDescr,
        value: SpannedText {
            text: normalize_multiline_common(raw),
            span: value_span,
            selection: value_span,
        },
    })
}

fn parse_title_spanned(
    line: &str,
    line_start: usize,
    dialect: RailroadDialect,
) -> Option<ParsedCommonField> {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let keyword_start = line_start + leading;
    let keyword_end = keyword_start + "title".len();
    if trimmed == "title" {
        return Some(ParsedCommonField {
            kind: CommonFieldKind::Title,
            value: SpannedText {
                text: String::new(),
                span: SourceSpan::new(keyword_end, keyword_end),
                selection: SourceSpan::new(keyword_end, keyword_end),
            },
        });
    }
    let rest = trimmed.strip_prefix("title")?;
    let ws = rest.chars().next()?;
    if !matches!(ws, ' ' | '\t') {
        return None;
    }
    let (raw, raw_span) = trimmed_value_span(rest, keyword_end);
    let collapsed = collapse_common_spaces(raw);
    let decoded = decode_wrapped_quoted_title(&collapsed, dialect);
    let (value, selection) = if let Some(value) = decoded {
        let opening = SourceSpan::new(raw_span.start, raw_span.start + 1);
        let closing = SourceSpan::new(raw_span.end - 1, raw_span.end);
        (value, SourceSpan::new(opening.end, closing.start))
    } else {
        (collapsed, raw_span)
    };
    Some(ParsedCommonField {
        kind: CommonFieldKind::Title,
        value: SpannedText {
            text: value,
            span: raw_span,
            selection,
        },
    })
}

fn parse_acc_title_spanned(line: &str, line_start: usize) -> Option<ParsedCommonField> {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let keyword_start = line_start + leading;
    let keyword_end = keyword_start + "accTitle".len();
    let after_keyword = trimmed.strip_prefix("accTitle")?;
    let rest = after_keyword.trim_start_matches([' ', '\t']);
    let colon_start = keyword_end + (after_keyword.len() - rest.len());
    let value_region = rest.strip_prefix(':')?;
    let (raw, value_span) = trimmed_value_span(value_region, colon_start + 1);
    Some(ParsedCommonField {
        kind: CommonFieldKind::AccTitle,
        value: SpannedText {
            text: collapse_common_spaces(raw),
            span: value_span,
            selection: value_span,
        },
    })
}

fn parse_acc_descr_spanned(line: &str, line_start: usize) -> Option<ParsedCommonField> {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let keyword_start = line_start + leading;
    let keyword_end = keyword_start + "accDescr".len();
    let after_keyword = trimmed.strip_prefix("accDescr")?;
    let rest = after_keyword.trim_start_matches([' ', '\t']);
    let delimiter_start = keyword_end + (after_keyword.len() - rest.len());
    let (raw, value_span) = if let Some(value_region) = rest.strip_prefix(':') {
        let (raw, value_span) = trimmed_value_span(value_region, delimiter_start + 1);
        (raw, value_span)
    } else {
        let body = rest.strip_prefix('{')?;
        let close_rel = body.find('}')?;
        let (raw, value_span) = trimmed_value_span(&body[..close_rel], delimiter_start + 1);
        (raw, value_span)
    };
    Some(ParsedCommonField {
        kind: CommonFieldKind::AccDescr,
        value: SpannedText {
            text: collapse_common_spaces(raw),
            span: value_span,
            selection: value_span,
        },
    })
}

fn trimmed_value_span(value: &str, base_offset: usize) -> (&str, SourceSpan) {
    let trimmed_start = value.trim_start();
    if trimmed_start.is_empty() {
        let insertion = base_offset + value.len();
        return ("", SourceSpan::new(insertion, insertion));
    }
    let leading = value.len() - trimmed_start.len();
    let trimmed = trimmed_start.trim_end();
    let start = base_offset + leading;
    (trimmed, SourceSpan::new(start, start + trimmed.len()))
}

fn collapse_common_spaces(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut previous_space = false;
    for ch in value.chars() {
        if matches!(ch, ' ' | '\t') {
            if !previous_space {
                out.push(' ');
                previous_space = true;
            }
        } else {
            out.push(ch);
            previous_space = false;
        }
    }
    out
}

fn normalize_multiline_common(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for line in value
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !normalized.is_empty() {
            normalized.push('\n');
        }
        normalized.push_str(line);
    }
    normalized
}

fn decode_wrapped_quoted_title(value: &str, dialect: RailroadDialect) -> Option<String> {
    if !((value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\'')))
    {
        return None;
    }
    if dialect == RailroadDialect::Abnf {
        return Some(value[1..value.len() - 1].to_string());
    }
    decode_escaped_quoted_string(value).map(|text| text.text)
}

fn is_railroad_string_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn decode_escaped_quoted_string(value: &str) -> Option<SpannedText> {
    let mut pos = 0usize;
    take_quoted_string_in(value, &mut pos, 0, RailroadDialect::Ir)
        .ok()
        .flatten()
        .filter(|_| pos == value.len())
}

fn take_quoted_string_in(
    input: &str,
    pos: &mut usize,
    base_offset: usize,
    dialect: RailroadDialect,
) -> Result<Option<SpannedText>> {
    let Some(quote) = input[*pos..].chars().next() else {
        return Ok(None);
    };
    if !matches!(quote, '"' | '\'') {
        return Ok(None);
    }
    if dialect == RailroadDialect::Abnf && quote != '"' {
        return Ok(None);
    }

    let start = base_offset + *pos;
    *pos += quote.len_utf8();
    let content_start = base_offset + *pos;
    let mut text = String::new();
    let mut escaped = false;
    while let Some(ch) = input[*pos..].chars().next() {
        let ch_start = base_offset + *pos;
        *pos += ch.len_utf8();
        if dialect != RailroadDialect::Abnf && escaped {
            match ch {
                'n' => text.push('\n'),
                'r' => text.push('\r'),
                't' => text.push('\t'),
                other => text.push(other),
            }
            escaped = false;
            continue;
        }
        if dialect != RailroadDialect::Abnf && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Ok(Some(SpannedText {
                text,
                span: SourceSpan::new(start, base_offset + *pos),
                selection: SourceSpan::new(content_start, ch_start),
            }));
        }
        text.push(ch);
    }
    Ok(None)
}

fn strip_inline_comment_aware(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut iter = line.char_indices().peekable();
    while let Some((idx, ch)) = iter.next() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_single || in_double => escaped = true,
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '%' if !in_single
                && !in_double
                && iter.peek().is_some_and(|(_, next)| *next == '%') =>
            {
                return &line[..idx];
            }
            _ => {}
        }
    }
    line
}

fn parse_abnf_repeat_bounds(repeat: &str) -> Option<(RailroadRepeatBound, RailroadRepeatBound)> {
    let parse_bound = |bound: &str| {
        bound
            .parse::<f64>()
            .ok()
            .and_then(|value| RailroadRepeatBound::try_from(value).ok())
    };

    if let Some((min, max)) = repeat.split_once('*') {
        let min = if min.is_empty() {
            RailroadRepeatBound::ZERO
        } else {
            parse_bound(min)?
        };
        let max = if max.is_empty() {
            RailroadRepeatBound::INFINITY
        } else {
            parse_bound(max)?
        };
        return Some((min, max));
    }

    let exact = parse_bound(repeat)?;
    Some((exact, exact))
}

fn with_outer_span(mut node: RailroadConstructedNode, span: SourceSpan) -> RailroadConstructedNode {
    match &mut node.node {
        RailroadAstNode::Terminal { span: inner, .. }
        | RailroadAstNode::NonTerminal { span: inner, .. }
        | RailroadAstNode::Sequence { span: inner, .. }
        | RailroadAstNode::Choice { span: inner, .. }
        | RailroadAstNode::Optional { span: inner, .. }
        | RailroadAstNode::Repetition { span: inner, .. }
        | RailroadAstNode::Special { span: inner, .. } => *inner = span,
    }
    node
}

fn node_to_label(node: &RailroadAstNode) -> String {
    match node {
        RailroadAstNode::Terminal { value, .. } => format!("\"{value}\""),
        RailroadAstNode::NonTerminal { name, .. } => name.clone(),
        RailroadAstNode::Special { text, .. } => text.clone(),
        _ => "(...)".to_string(),
    }
}
