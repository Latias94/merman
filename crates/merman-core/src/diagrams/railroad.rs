use crate::sanitize::sanitize_text;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RailroadDialect {
    Ir,
    Ebnf,
    Abnf,
    Peg,
}

impl RailroadDialect {
    fn diagram_type(self) -> &'static str {
        match self {
            Self::Ir => "railroad",
            Self::Ebnf => "railroadEbnf",
            Self::Abnf => "railroadAbnf",
            Self::Peg => "railroadPeg",
        }
    }

    fn header(self) -> &'static str {
        match self {
            Self::Ir => "railroad-beta",
            Self::Ebnf => "railroad-ebnf-beta",
            Self::Abnf => "railroad-abnf-beta",
            Self::Peg => "railroad-peg-beta",
        }
    }

    fn common_detail_prefix(self) -> &'static str {
        match self {
            Self::Ir => "railroad",
            Self::Ebnf => "railroad ebnf",
            Self::Abnf => "railroad abnf",
            Self::Peg => "railroad peg",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RailroadDiagramModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub rules: Vec<RailroadRuleModel>,
    #[serde(skip_serializing)]
    title_span: Option<SourceSpan>,
    #[serde(skip_serializing)]
    acc_title_span: Option<SourceSpan>,
    #[serde(skip_serializing)]
    acc_descr_span: Option<SourceSpan>,
}

impl RailroadDiagramModel {
    fn new() -> Self {
        Self {
            acc_title: None,
            acc_descr: None,
            title: None,
            rules: Vec::new(),
            title_span: None,
            acc_title_span: None,
            acc_descr_span: None,
        }
    }

    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
        for rule in &mut self.rules {
            rule.name = sanitize_text(&rule.name, config);
            sanitize_ast_node(&mut rule.definition, config);
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RailroadRuleModel {
    pub name: String,
    pub definition: RailroadAstNode,
    #[serde(skip_serializing)]
    name_span: SourceSpan,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum RailroadAstNode {
    #[serde(rename = "terminal")]
    Terminal {
        value: String,
        #[serde(skip_serializing)]
        span: SourceSpan,
        #[serde(skip_serializing)]
        selection: SourceSpan,
    },
    #[serde(rename = "nonterminal")]
    NonTerminal {
        name: String,
        #[serde(skip_serializing)]
        span: SourceSpan,
        #[serde(skip_serializing)]
        selection: SourceSpan,
    },
    #[serde(rename = "sequence")]
    Sequence {
        elements: Vec<RailroadAstNode>,
        #[serde(skip_serializing)]
        span: SourceSpan,
    },
    #[serde(rename = "choice")]
    Choice {
        alternatives: Vec<RailroadAstNode>,
        #[serde(skip_serializing)]
        span: SourceSpan,
    },
    #[serde(rename = "optional")]
    Optional {
        element: Box<RailroadAstNode>,
        #[serde(skip_serializing)]
        span: SourceSpan,
    },
    #[serde(rename = "repetition")]
    Repetition {
        element: Box<RailroadAstNode>,
        min: u64,
        max: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        separator: Option<Box<RailroadAstNode>>,
        #[serde(skip_serializing)]
        span: SourceSpan,
    },
    #[serde(rename = "special")]
    Special {
        text: String,
        #[serde(skip_serializing)]
        span: SourceSpan,
        #[serde(skip_serializing)]
        selection: SourceSpan,
    },
}

impl RailroadAstNode {
    fn span(&self) -> SourceSpan {
        match self {
            Self::Terminal { span, .. }
            | Self::NonTerminal { span, .. }
            | Self::Sequence { span, .. }
            | Self::Choice { span, .. }
            | Self::Optional { span, .. }
            | Self::Repetition { span, .. }
            | Self::Special { span, .. } => *span,
        }
    }

    fn selection(&self) -> SourceSpan {
        match self {
            Self::Terminal { selection, .. }
            | Self::NonTerminal { selection, .. }
            | Self::Special { selection, .. } => *selection,
            Self::Sequence { span, .. }
            | Self::Choice { span, .. }
            | Self::Optional { span, .. }
            | Self::Repetition { span, .. } => *span,
        }
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
    Common(CommonFieldKind, String),
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

pub fn parse_railroad(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_railroad_for_dialect(code, meta, RailroadDialect::Ir)
}

pub fn parse_railroad_ebnf(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_railroad_for_dialect(code, meta, RailroadDialect::Ebnf)
}

pub fn parse_railroad_abnf(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_railroad_for_dialect(code, meta, RailroadDialect::Abnf)
}

pub fn parse_railroad_peg(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_railroad_for_dialect(code, meta, RailroadDialect::Peg)
}

pub fn parse_railroad_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    parse_railroad_editor_facts_for_dialect(code, meta, RailroadDialect::Ir)
}

pub fn parse_railroad_ebnf_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    parse_railroad_editor_facts_for_dialect(code, meta, RailroadDialect::Ebnf)
}

pub fn parse_railroad_abnf_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    parse_railroad_editor_facts_for_dialect(code, meta, RailroadDialect::Abnf)
}

pub fn parse_railroad_peg_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    parse_railroad_editor_facts_for_dialect(code, meta, RailroadDialect::Peg)
}

fn parse_railroad_for_dialect(
    code: &str,
    meta: &ParseMetadata,
    dialect: RailroadDialect,
) -> Result<Value> {
    let mut model = parse_railroad_model(code, meta, dialect)?;
    model.sanitize_common_db_fields(&meta.effective_config);

    Ok(json!({
        "type": meta.diagram_type,
        "title": model.title,
        "accTitle": model.acc_title,
        "accDescr": model.acc_descr,
        "rules": model.rules,
    }))
}

fn parse_railroad_editor_facts_for_dialect(
    code: &str,
    meta: &ParseMetadata,
    dialect: RailroadDialect,
) -> EditorSemanticFacts {
    match parse_railroad_model(code, meta, dialect) {
        Ok(model) => editor_facts_from_model(&model, dialect),
        Err(err) => {
            let mut facts = scan_editor_facts_lossy(code, dialect);
            facts.mark_recovered_from_parse_error(
                format!(
                    "{} parser recovered after parse error: {err}",
                    dialect.diagram_type()
                ),
                Some(SourceSpan::new(0, code.len())),
            );
            facts
        }
    }
}

fn parse_railroad_model(
    code: &str,
    meta: &ParseMetadata,
    dialect: RailroadDialect,
) -> Result<RailroadDiagramModel> {
    let tokens = Lexer::new(code, dialect, meta.diagram_type.as_str()).tokenize()?;
    RailroadParser::new(tokens, code.len(), meta.diagram_type.as_str(), dialect).parse()
}

fn sanitize_ast_node(node: &mut RailroadAstNode, config: &crate::MermaidConfig) {
    match node {
        RailroadAstNode::Terminal { value, .. } => *value = sanitize_text(value, config),
        RailroadAstNode::NonTerminal { name, .. } => *name = sanitize_text(name, config),
        RailroadAstNode::Sequence { elements, .. } => {
            for element in elements {
                sanitize_ast_node(element, config);
            }
        }
        RailroadAstNode::Choice { alternatives, .. } => {
            for alternative in alternatives {
                sanitize_ast_node(alternative, config);
            }
        }
        RailroadAstNode::Optional { element, .. } => sanitize_ast_node(element, config),
        RailroadAstNode::Repetition {
            element, separator, ..
        } => {
            sanitize_ast_node(element, config);
            if let Some(separator) = separator {
                sanitize_ast_node(separator, config);
            }
        }
        RailroadAstNode::Special { text, .. } => *text = sanitize_text(text, config),
    }
}

struct RailroadParser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    input_len: usize,
    diagram_type: &'a str,
    dialect: RailroadDialect,
}

impl<'a> RailroadParser<'a> {
    fn new(
        tokens: Vec<Token>,
        input_len: usize,
        diagram_type: &'a str,
        dialect: RailroadDialect,
    ) -> Self {
        Self {
            tokens,
            pos: 0,
            input_len,
            diagram_type,
            dialect,
        }
    }

    fn parse(mut self) -> Result<RailroadDiagramModel> {
        let mut model = RailroadDiagramModel::new();
        self.expect_header()?;

        while let Some(field) = self.take_common_field()? {
            match field.kind {
                CommonFieldKind::Title => {
                    model.title = Some(field.value);
                    model.title_span = Some(field.span);
                }
                CommonFieldKind::AccTitle => {
                    model.acc_title = Some(field.value);
                    model.acc_title_span = Some(field.span);
                }
                CommonFieldKind::AccDescr => {
                    model.acc_descr = Some(field.value);
                    model.acc_descr_span = Some(field.span);
                }
            }
        }

        while !self.is_eof() {
            model.rules.push(self.parse_rule()?);
        }

        Ok(model)
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
        if !value.eq_ignore_ascii_case(self.dialect.header()) {
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
        let TokenKind::Common(kind, value) = &token.kind else {
            return Ok(None);
        };
        let out = SpannedCommonField {
            kind: *kind,
            value: value.clone(),
            span: token.selection,
        };
        self.pos += 1;
        Ok(Some(out))
    }

    fn parse_rule(&mut self) -> Result<RailroadRuleModel> {
        let name = self.expect_ident("expected railroad rule name")?;
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
            RailroadDialect::Ir => self.parse_ir_expression()?,
            RailroadDialect::Ebnf => self.parse_ebnf_choice()?,
            RailroadDialect::Abnf => self.parse_abnf_alternation()?,
            RailroadDialect::Peg => self.parse_peg_ordered_choice()?,
        };
        self.expect_symbol(';', "expected ';' after railroad rule definition")?;

        Ok(RailroadRuleModel {
            name: name.text,
            definition,
            name_span: name.selection,
        })
    }

    fn parse_ir_expression(&mut self) -> Result<RailroadAstNode> {
        let function = self.expect_ident("expected railroad expression")?;
        self.expect_symbol('(', "expected '(' after railroad expression name")?;

        let node = match function.text.as_str() {
            "terminal" => {
                let value = self.expect_string("expected string argument for terminal")?;
                self.expect_symbol(')', "expected ')' after terminal argument")?;
                RailroadAstNode::Terminal {
                    value: value.text,
                    span: value.span,
                    selection: value.selection,
                }
            }
            "nonterminal" => {
                let name = self.expect_string("expected string argument for nonterminal")?;
                self.expect_symbol(')', "expected ')' after nonterminal argument")?;
                RailroadAstNode::NonTerminal {
                    name: name.text,
                    span: name.span,
                    selection: name.selection,
                }
            }
            "special" => {
                let text = self.expect_string("expected string argument for special")?;
                self.expect_symbol(')', "expected ')' after special argument")?;
                RailroadAstNode::Special {
                    text: text.text,
                    span: text.span,
                    selection: text.selection,
                }
            }
            "optional" => {
                let start = function.span.start;
                let element = self.parse_ir_expression()?;
                let end = self.expect_symbol(')', "expected ')' after optional argument")?;
                RailroadAstNode::Optional {
                    element: Box::new(element),
                    span: SourceSpan::new(start, end.span.end),
                }
            }
            "oneOrMore" => {
                let start = function.span.start;
                let element = self.parse_ir_expression()?;
                let end = self.expect_symbol(')', "expected ')' after oneOrMore argument")?;
                RailroadAstNode::Repetition {
                    element: Box::new(element),
                    min: 1,
                    max: None,
                    separator: None,
                    span: SourceSpan::new(start, end.span.end),
                }
            }
            "zeroOrMore" => {
                let start = function.span.start;
                let element = self.parse_ir_expression()?;
                let end = self.expect_symbol(')', "expected ')' after zeroOrMore argument")?;
                RailroadAstNode::Repetition {
                    element: Box::new(element),
                    min: 0,
                    max: None,
                    separator: None,
                    span: SourceSpan::new(start, end.span.end),
                }
            }
            "sequence" => {
                let start = function.span.start;
                let elements = self.parse_ir_expression_list("sequence")?;
                let end = self.expect_symbol(')', "expected ')' after sequence arguments")?;
                collapse_sequence(elements, SourceSpan::new(start, end.span.end))
            }
            "choice" => {
                let start = function.span.start;
                let alternatives = self.parse_ir_expression_list("choice")?;
                let end = self.expect_symbol(')', "expected ')' after choice arguments")?;
                collapse_choice(alternatives, SourceSpan::new(start, end.span.end))
            }
            _ => {
                return Err(self.error_at_span(
                    function.span,
                    format!("unsupported railroad expression: {}", function.text),
                ));
            }
        };

        Ok(node)
    }

    fn parse_ir_expression_list(&mut self, function: &str) -> Result<Vec<RailroadAstNode>> {
        if self.check_symbol(')') {
            return Err(self.error_at_current(format!("{function} requires at least one argument")));
        }

        let mut elements = vec![self.parse_ir_expression()?];
        while self.take_symbol(',') {
            elements.push(self.parse_ir_expression()?);
        }
        Ok(elements)
    }

    fn parse_ebnf_choice(&mut self) -> Result<RailroadAstNode> {
        let first = self.parse_ebnf_sequence()?;
        let start = first.span().start;
        let mut end = first.span().end;
        let mut alternatives = vec![first];

        while self.take_symbol('|') {
            let next = self.parse_ebnf_sequence()?;
            end = next.span().end;
            alternatives.push(next);
        }

        Ok(collapse_choice(alternatives, SourceSpan::new(start, end)))
    }

    fn parse_ebnf_sequence(&mut self) -> Result<RailroadAstNode> {
        if !self.is_ebnf_primary_start() {
            return Err(self.error_at_current("expected EBNF expression"));
        }

        let first = self.parse_ebnf_term()?;
        let start = first.span().start;
        let mut end = first.span().end;
        let mut elements = vec![first];

        loop {
            if self.take_symbol(',') && !self.is_ebnf_primary_start() {
                return Err(self.error_at_current("expected EBNF term after ','"));
            }
            if !self.is_ebnf_primary_start() {
                break;
            }
            let term = self.parse_ebnf_term()?;
            end = term.span().end;
            elements.push(term);
        }

        Ok(collapse_sequence(elements, SourceSpan::new(start, end)))
    }

    fn parse_ebnf_term(&mut self) -> Result<RailroadAstNode> {
        let mut node = self.parse_ebnf_primary()?;

        loop {
            if self.take_symbol('?') {
                let span = SourceSpan::new(node.span().start, self.previous_end());
                node = RailroadAstNode::Optional {
                    element: Box::new(node),
                    span,
                };
                continue;
            }
            if self.take_symbol('*') {
                let span = SourceSpan::new(node.span().start, self.previous_end());
                node = RailroadAstNode::Repetition {
                    element: Box::new(node),
                    min: 0,
                    max: None,
                    separator: None,
                    span,
                };
                continue;
            }
            if self.take_symbol('+') {
                let span = SourceSpan::new(node.span().start, self.previous_end());
                node = RailroadAstNode::Repetition {
                    element: Box::new(node),
                    min: 1,
                    max: None,
                    separator: None,
                    span,
                };
                continue;
            }
            if self.take_symbol('-') {
                let op_span = self.previous_span();
                let except = self.parse_ebnf_primary()?;
                let span = SourceSpan::new(node.span().start, except.span().end);
                let dash = RailroadAstNode::Terminal {
                    value: "-".to_string(),
                    span: op_span,
                    selection: op_span,
                };
                node = RailroadAstNode::Sequence {
                    elements: vec![node, dash, except],
                    span,
                };
                continue;
            }
            break;
        }

        Ok(node)
    }

    fn parse_ebnf_primary(&mut self) -> Result<RailroadAstNode> {
        if let Some(value) = self.take_string() {
            return Ok(RailroadAstNode::Terminal {
                value: value.text,
                span: value.span,
                selection: value.selection,
            });
        }
        if let Some(value) = self.take_special_sequence() {
            return Ok(RailroadAstNode::Special {
                text: value.text,
                span: value.span,
                selection: value.selection,
            });
        }
        if let Some(value) = self.take_ident() {
            return Ok(RailroadAstNode::NonTerminal {
                name: value.text,
                span: value.span,
                selection: value.selection,
            });
        }
        if self.take_symbol('(') {
            let start = self.previous_span().start;
            let element = self.parse_ebnf_choice()?;
            let end = self.expect_symbol(')', "expected ')' after EBNF group")?;
            let span = SourceSpan::new(start, end.span.end);
            return Ok(with_outer_span(element, span));
        }
        if self.take_symbol('[') {
            let start = self.previous_span().start;
            let element = self.parse_ebnf_choice()?;
            let end = self.expect_symbol(']', "expected ']' after EBNF optional group")?;
            return Ok(RailroadAstNode::Optional {
                element: Box::new(element),
                span: SourceSpan::new(start, end.span.end),
            });
        }
        if self.take_symbol('{') {
            let start = self.previous_span().start;
            let element = self.parse_ebnf_choice()?;
            let end = self.expect_symbol('}', "expected '}' after EBNF repetition")?;
            return Ok(RailroadAstNode::Repetition {
                element: Box::new(element),
                min: 0,
                max: None,
                separator: None,
                span: SourceSpan::new(start, end.span.end),
            });
        }

        Err(self.error_at_current("expected EBNF primary"))
    }

    fn parse_abnf_alternation(&mut self) -> Result<RailroadAstNode> {
        let first = self.parse_abnf_concatenation()?;
        let start = first.span().start;
        let mut end = first.span().end;
        let mut alternatives = vec![first];

        while self.take_symbol('/') {
            let next = self.parse_abnf_concatenation()?;
            end = next.span().end;
            alternatives.push(next);
        }

        Ok(collapse_choice(alternatives, SourceSpan::new(start, end)))
    }

    fn parse_abnf_concatenation(&mut self) -> Result<RailroadAstNode> {
        if !self.is_abnf_element_start() {
            return Err(self.error_at_current("expected ABNF element"));
        }

        let first = self.parse_abnf_element()?;
        let start = first.span().start;
        let mut end = first.span().end;
        let mut elements = vec![first];

        while self.is_abnf_element_start() {
            let element = self.parse_abnf_element()?;
            end = element.span().end;
            elements.push(element);
        }

        Ok(collapse_sequence(elements, SourceSpan::new(start, end)))
    }

    fn parse_abnf_element(&mut self) -> Result<RailroadAstNode> {
        let repeat = self.take_abnf_repeat();
        let primary = self.parse_abnf_primary()?;
        let Some(repeat) = repeat else {
            return Ok(primary);
        };

        let (min, max) = parse_abnf_repeat_bounds(&repeat.text);
        let span = SourceSpan::new(repeat.span.start, primary.span().end);
        if min == 0 && max == Some(1) {
            return Ok(RailroadAstNode::Optional {
                element: Box::new(primary),
                span,
            });
        }

        Ok(RailroadAstNode::Repetition {
            element: Box::new(primary),
            min,
            max,
            separator: None,
            span,
        })
    }

    fn parse_abnf_primary(&mut self) -> Result<RailroadAstNode> {
        if let Some(value) = self.take_string() {
            return Ok(RailroadAstNode::Terminal {
                value: value.text,
                span: value.span,
                selection: value.selection,
            });
        }
        if let Some(value) = self.take_num_val() {
            return Ok(RailroadAstNode::Terminal {
                value: value.text,
                span: value.span,
                selection: value.selection,
            });
        }
        if let Some(value) = self.take_ident() {
            return Ok(RailroadAstNode::NonTerminal {
                name: value.text,
                span: value.span,
                selection: value.selection,
            });
        }
        if self.take_symbol('(') {
            let start = self.previous_span().start;
            let element = self.parse_abnf_alternation()?;
            let end = self.expect_symbol(')', "expected ')' after ABNF group")?;
            let span = SourceSpan::new(start, end.span.end);
            return Ok(with_outer_span(element, span));
        }
        if self.take_symbol('[') {
            let start = self.previous_span().start;
            let element = self.parse_abnf_alternation()?;
            let end = self.expect_symbol(']', "expected ']' after ABNF optional group")?;
            return Ok(RailroadAstNode::Optional {
                element: Box::new(element),
                span: SourceSpan::new(start, end.span.end),
            });
        }

        Err(self.error_at_current("expected ABNF primary"))
    }

    fn parse_peg_ordered_choice(&mut self) -> Result<RailroadAstNode> {
        let first = self.parse_peg_sequence()?;
        let start = first.span().start;
        let mut end = first.span().end;
        let mut alternatives = vec![first];

        while self.take_symbol('/') {
            let next = self.parse_peg_sequence()?;
            end = next.span().end;
            alternatives.push(next);
        }

        Ok(collapse_choice(alternatives, SourceSpan::new(start, end)))
    }

    fn parse_peg_sequence(&mut self) -> Result<RailroadAstNode> {
        if !self.is_peg_prefix_start() {
            return Err(self.error_at_current("expected PEG expression"));
        }

        let first = self.parse_peg_prefix()?;
        let start = first.span().start;
        let mut end = first.span().end;
        let mut elements = vec![first];

        while self.is_peg_prefix_start() {
            let element = self.parse_peg_prefix()?;
            end = element.span().end;
            elements.push(element);
        }

        Ok(collapse_sequence(elements, SourceSpan::new(start, end)))
    }

    fn parse_peg_prefix(&mut self) -> Result<RailroadAstNode> {
        let prefix = if self.take_symbol('&') {
            Some(('&', self.previous_span()))
        } else if self.take_symbol('!') {
            Some(('!', self.previous_span()))
        } else {
            None
        };
        let suffix = self.parse_peg_suffix()?;

        let Some((operator, span)) = prefix else {
            return Ok(suffix);
        };

        let label = format!("{operator}{}", node_to_label(&suffix));
        Ok(RailroadAstNode::Special {
            text: label,
            span: SourceSpan::new(span.start, suffix.span().end),
            selection: SourceSpan::new(span.start, suffix.selection().end),
        })
    }

    fn parse_peg_suffix(&mut self) -> Result<RailroadAstNode> {
        let primary = self.parse_peg_primary()?;
        if self.take_symbol('?') {
            return Ok(RailroadAstNode::Optional {
                span: SourceSpan::new(primary.span().start, self.previous_end()),
                element: Box::new(primary),
            });
        }
        if self.take_symbol('*') {
            return Ok(RailroadAstNode::Repetition {
                span: SourceSpan::new(primary.span().start, self.previous_end()),
                element: Box::new(primary),
                min: 0,
                max: None,
                separator: None,
            });
        }
        if self.take_symbol('+') {
            return Ok(RailroadAstNode::Repetition {
                span: SourceSpan::new(primary.span().start, self.previous_end()),
                element: Box::new(primary),
                min: 1,
                max: None,
                separator: None,
            });
        }
        Ok(primary)
    }

    fn parse_peg_primary(&mut self) -> Result<RailroadAstNode> {
        if let Some(value) = self.take_string() {
            return Ok(RailroadAstNode::Terminal {
                value: value.text,
                span: value.span,
                selection: value.selection,
            });
        }
        if let Some(value) = self.take_ident() {
            return Ok(RailroadAstNode::NonTerminal {
                name: value.text,
                span: value.span,
                selection: value.selection,
            });
        }
        if self.take_symbol('(') {
            let start = self.previous_span().start;
            let element = self.parse_peg_ordered_choice()?;
            let end = self.expect_symbol(')', "expected ')' after PEG group")?;
            let span = SourceSpan::new(start, end.span.end);
            return Ok(with_outer_span(element, span));
        }
        if self.take_symbol('.') {
            let span = self.previous_span();
            return Ok(RailroadAstNode::Special {
                text: ".".to_string(),
                span,
                selection: span,
            });
        }

        Err(self.error_at_current("expected PEG primary"))
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

    fn error_at_current(&self, message: impl Into<String>) -> Error {
        if let Some(token) = self.peek() {
            self.error_at_token(token, message)
        } else {
            Error::diagram_parse_insertion_point(self.diagram_type, message, self.input_len)
        }
    }

    fn error_at_token(&self, token: &Token, message: impl Into<String>) -> Error {
        Error::diagram_parse_exact(self.diagram_type, message, token.span)
    }

    fn error_at_span(&self, span: SourceSpan, message: impl Into<String>) -> Error {
        Error::diagram_parse_exact(self.diagram_type, message, span)
    }
}

#[derive(Debug, Clone)]
struct SpannedCommonField {
    kind: CommonFieldKind,
    value: String,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    span: SourceSpan,
    selection: SourceSpan,
}

struct Lexer<'a> {
    input: &'a str,
    dialect: RailroadDialect,
    diagram_type: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str, dialect: RailroadDialect, diagram_type: &'a str) -> Self {
        Self {
            input,
            dialect,
            diagram_type,
            pos: 0,
        }
    }

    fn tokenize(mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        while self.skip_trivia()? {
            if self.is_eof() {
                break;
            }
            if let Some(token) = self.take_common_field()? {
                tokens.push(token);
                continue;
            }
            if let Some(token) = self.take_string()? {
                tokens.push(token);
                continue;
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
            return Err(Error::diagram_parse_exact(
                self.diagram_type,
                format!("unexpected railroad token `{ch}`"),
                SourceSpan::new(start, start + ch.len_utf8()),
            ));
        }

        Ok(tokens)
    }

    fn skip_trivia(&mut self) -> Result<bool> {
        loop {
            let before = self.pos;
            while self.current_char().is_some_and(char::is_whitespace) {
                self.advance_char();
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
                self.skip_until("*/", "unterminated railroad block comment")?;
                continue;
            }
            if self.dialect == RailroadDialect::Ebnf && self.starts_with("(*") {
                self.skip_until("*)", "unterminated EBNF comment")?;
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
            if self.starts_with("---") && self.at_line_start_after_indent() {
                if let Some(close_rel) = self.remaining()["---".len()..].find("\n---") {
                    self.pos += "---".len() + close_rel + "\n---".len();
                    continue;
                }
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
        let line_end = self.input[token_start..]
            .find(['\r', '\n'])
            .map(|rel| token_start + rel)
            .unwrap_or(self.input.len());
        let line = &self.input[token_start..line_end];

        if let Some(field) = parse_common_field_line(line, token_start, self.dialect) {
            self.pos = line_end;
            return Ok(Some(Token {
                kind: TokenKind::Common(field.kind, field.value.text),
                span: SourceSpan::new(token_start, line_end),
                selection: field.value.selection,
            }));
        }

        if line.trim_start().starts_with("accDescr") && line.contains('{') && !line.contains('}') {
            if let Some(end_rel) = self.input[line_end..].find('}') {
                let end = line_end + end_rel + 1;
                let full = &self.input[token_start..end];
                if let Some(field) = parse_common_field_block(full, token_start) {
                    self.pos = end;
                    return Ok(Some(Token {
                        kind: TokenKind::Common(field.kind, field.value.text),
                        span: SourceSpan::new(token_start, end),
                        selection: field.value.selection,
                    }));
                }
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

        Err(Error::diagram_parse_insertion_point(
            self.diagram_type,
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
        let valid_start = match self.dialect {
            RailroadDialect::Abnf => first.is_ascii_alphabetic(),
            RailroadDialect::Ir | RailroadDialect::Ebnf | RailroadDialect::Peg => {
                first.is_ascii_alphabetic() || first == '_'
            }
        };
        if !valid_start {
            return None;
        }

        self.advance_char();
        while let Some(ch) = self.current_char() {
            let valid = match self.dialect {
                RailroadDialect::Abnf => ch.is_ascii_alphanumeric() || ch == '-',
                RailroadDialect::Ir | RailroadDialect::Ebnf | RailroadDialect::Peg => {
                    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
                }
            };
            if !valid {
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
            return Err(Error::diagram_parse_insertion_point(
                self.diagram_type,
                message,
                start,
            ));
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

    fn at_line_start_after_indent(&self) -> bool {
        let line_start = self.input[..self.pos]
            .rfind(['\r', '\n'])
            .map(|idx| idx + 1)
            .unwrap_or(0);
        self.input[line_start..self.pos].trim().is_empty()
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

#[derive(Debug, Clone)]
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
    let rest = trimmed.strip_prefix("accDescr")?.trim_start();
    let rest_start = block_start
        + leading
        + "accDescr".len()
        + (trimmed.strip_prefix("accDescr")?.len() - rest.len());
    let body = rest.strip_prefix('{')?;
    let end = body.find('}')?;
    let raw = &body[..end];
    let value = normalize_multiline_common(raw);
    let raw_trimmed = raw.trim();
    let value_rel = if raw_trimmed.is_empty() {
        rest_start + rest.find('{')? + 1
    } else {
        block_start + block.find(raw_trimmed)?
    };
    Some(ParsedCommonField {
        kind: CommonFieldKind::AccDescr,
        value: SpannedText {
            text: value,
            span: SourceSpan::new(value_rel, value_rel + raw_trimmed.len()),
            selection: SourceSpan::new(value_rel, value_rel + raw_trimmed.len()),
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
    if trimmed == "title" {
        let offset = line_start + leading + "title".len();
        return Some(ParsedCommonField {
            kind: CommonFieldKind::Title,
            value: SpannedText {
                text: String::new(),
                span: SourceSpan::new(offset, offset),
                selection: SourceSpan::new(offset, offset),
            },
        });
    }
    let rest = trimmed.strip_prefix("title")?;
    let ws = rest.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let raw = rest.trim();
    let collapsed = collapse_common_spaces(raw);
    let value = decode_wrapped_quoted_title(&collapsed, dialect).unwrap_or(collapsed);
    let value_rel = line.find(raw)?;
    Some(ParsedCommonField {
        kind: CommonFieldKind::Title,
        value: SpannedText {
            text: value,
            span: SourceSpan::new(line_start + value_rel, line_start + value_rel + raw.len()),
            selection: SourceSpan::new(line_start + value_rel, line_start + value_rel + raw.len()),
        },
    })
}

fn parse_acc_title_spanned(line: &str, line_start: usize) -> Option<ParsedCommonField> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("accTitle")?.trim_start();
    let raw = rest.strip_prefix(':')?.trim();
    let value = collapse_common_spaces(raw);
    let value_rel = if raw.is_empty() {
        line.len()
    } else {
        line.find(raw)?
    };
    Some(ParsedCommonField {
        kind: CommonFieldKind::AccTitle,
        value: SpannedText {
            text: value,
            span: SourceSpan::new(line_start + value_rel, line_start + value_rel + raw.len()),
            selection: SourceSpan::new(line_start + value_rel, line_start + value_rel + raw.len()),
        },
    })
}

fn parse_acc_descr_spanned(line: &str, line_start: usize) -> Option<ParsedCommonField> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("accDescr")?.trim_start();
    let (raw, value_rel) = if let Some(value) = rest.strip_prefix(':') {
        let raw = value.trim();
        let rel = if raw.is_empty() {
            line.len()
        } else {
            line.find(raw)?
        };
        (raw, rel)
    } else {
        let body = rest.strip_prefix('{')?;
        let end = body.find('}')?;
        let raw = body[..end].trim();
        let rel = if raw.is_empty() {
            line.find('{')? + 1
        } else {
            line.find(raw)?
        };
        (raw, rel)
    };
    Some(ParsedCommonField {
        kind: CommonFieldKind::AccDescr,
        value: SpannedText {
            text: collapse_common_spaces(raw),
            span: SourceSpan::new(line_start + value_rel, line_start + value_rel + raw.len()),
            selection: SourceSpan::new(line_start + value_rel, line_start + value_rel + raw.len()),
        },
    })
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
    let mut lines: Vec<_> = value.lines().map(|line| line.trim()).collect();
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
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

fn parse_abnf_repeat_bounds(repeat: &str) -> (u64, Option<u64>) {
    if let Some((min, max)) = repeat.split_once('*') {
        let min = if min.is_empty() {
            0
        } else {
            min.parse().unwrap_or(0)
        };
        let max = if max.is_empty() {
            None
        } else {
            Some(max.parse().unwrap_or(min))
        };
        return (min, max);
    }

    let exact = repeat.parse().unwrap_or(1);
    (exact, Some(exact))
}

fn collapse_sequence(elements: Vec<RailroadAstNode>, span: SourceSpan) -> RailroadAstNode {
    if elements.len() == 1 {
        elements.into_iter().next().expect("one element")
    } else {
        RailroadAstNode::Sequence { elements, span }
    }
}

fn collapse_choice(alternatives: Vec<RailroadAstNode>, span: SourceSpan) -> RailroadAstNode {
    if alternatives.len() == 1 {
        alternatives.into_iter().next().expect("one alternative")
    } else {
        RailroadAstNode::Choice { alternatives, span }
    }
}

fn with_outer_span(mut node: RailroadAstNode, span: SourceSpan) -> RailroadAstNode {
    match &mut node {
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

fn editor_facts_from_model(
    model: &RailroadDiagramModel,
    dialect: RailroadDialect,
) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let detail_prefix = dialect.common_detail_prefix();

    if let (Some(title), Some(span)) = (&model.title, model.title_span) {
        facts.push_directive_prefix("title");
        push_payload_fact(
            &mut facts,
            title.clone(),
            span,
            format!("{detail_prefix} title"),
        );
    }
    if let (Some(acc_title), Some(span)) = (&model.acc_title, model.acc_title_span) {
        facts.push_directive_prefix("accTitle");
        push_payload_fact(
            &mut facts,
            acc_title.clone(),
            span,
            format!("{detail_prefix} accessibility title"),
        );
    }
    if let (Some(acc_descr), Some(span)) = (&model.acc_descr, model.acc_descr_span) {
        facts.push_directive_prefix("accDescr");
        push_payload_fact(
            &mut facts,
            acc_descr.clone(),
            span,
            format!("{detail_prefix} accessibility description"),
        );
    }

    for rule in &model.rules {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::NodeIdentifier,
            rule.name_span,
        ));
        facts.push_symbol(EditorSemanticSymbol::new(
            rule.name.clone(),
            Some(format!("{detail_prefix} rule")),
            EditorSemanticKind::Function,
            rule.name_span,
            rule.name_span,
        ));
        push_ast_facts(&mut facts, &rule.definition, detail_prefix);
    }

    facts
}

fn push_ast_facts(facts: &mut EditorSemanticFacts, node: &RailroadAstNode, detail_prefix: &str) {
    match node {
        RailroadAstNode::Terminal {
            value, selection, ..
        } => {
            push_payload_fact(
                facts,
                value.clone(),
                *selection,
                format!("{detail_prefix} terminal"),
            );
        }
        RailroadAstNode::NonTerminal {
            name,
            span,
            selection,
        } => {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::NodeIdentifier,
                *selection,
            ));
            facts.push_symbol(EditorSemanticSymbol::new(
                name.clone(),
                Some(format!("{detail_prefix} nonterminal reference")),
                EditorSemanticKind::Function,
                *span,
                *selection,
            ));
        }
        RailroadAstNode::Special {
            text, selection, ..
        } => {
            push_payload_fact(
                facts,
                text.clone(),
                *selection,
                format!("{detail_prefix} special"),
            );
        }
        RailroadAstNode::Sequence { elements, .. } => {
            for element in elements {
                push_ast_facts(facts, element, detail_prefix);
            }
        }
        RailroadAstNode::Choice { alternatives, .. } => {
            for alternative in alternatives {
                push_ast_facts(facts, alternative, detail_prefix);
            }
        }
        RailroadAstNode::Optional { element, .. } => push_ast_facts(facts, element, detail_prefix),
        RailroadAstNode::Repetition {
            element, separator, ..
        } => {
            push_ast_facts(facts, element, detail_prefix);
            if let Some(separator) = separator {
                push_ast_facts(facts, separator, detail_prefix);
            }
        }
    }
}

fn push_payload_fact(
    facts: &mut EditorSemanticFacts,
    value: String,
    selection: SourceSpan,
    detail: String,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        value,
        Some(detail),
        EditorSemanticKind::String,
        selection,
        selection,
    ));
}

fn scan_editor_facts_lossy(code: &str, dialect: RailroadDialect) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let Ok(tokens) = Lexer::new(code, dialect, dialect.diagram_type()).tokenize() else {
        return facts;
    };
    let detail_prefix = dialect.common_detail_prefix();

    let mut after_header = false;
    let mut prev_ident: Option<Token> = None;
    for token in tokens {
        match &token.kind {
            TokenKind::Ident(value) if value.eq_ignore_ascii_case(dialect.header()) => {
                after_header = true;
                prev_ident = None;
            }
            TokenKind::Common(kind, value) => {
                let prefix = match kind {
                    CommonFieldKind::Title => "title",
                    CommonFieldKind::AccTitle => "accTitle",
                    CommonFieldKind::AccDescr => "accDescr",
                };
                facts.push_directive_prefix(prefix);
                push_payload_fact(
                    &mut facts,
                    value.clone(),
                    token.selection,
                    format!("{detail_prefix} {prefix}"),
                );
                prev_ident = None;
            }
            TokenKind::Ident(_) if after_header => {
                prev_ident = Some(token);
            }
            TokenKind::Symbol('=') | TokenKind::ColonColonEq | TokenKind::LeftArrow => {
                if let Some(name) = prev_ident.take() {
                    if let TokenKind::Ident(value) = name.kind {
                        facts.push_expected_syntax(EditorExpectedSyntax::new(
                            EditorExpectedSyntaxKind::NodeIdentifier,
                            name.selection,
                        ));
                        facts.push_symbol(EditorSemanticSymbol::new(
                            value,
                            Some(format!("{detail_prefix} rule")),
                            EditorSemanticKind::Function,
                            name.span,
                            name.selection,
                        ));
                    }
                }
            }
            TokenKind::String(value)
            | TokenKind::SpecialSequence(value)
            | TokenKind::NumVal(value) => {
                push_payload_fact(
                    &mut facts,
                    value.clone(),
                    token.selection,
                    format!("{detail_prefix} literal"),
                );
                prev_ident = None;
            }
            _ => prev_ident = None,
        }
    }

    facts.mark_recovered();
    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_escaped_strings_for_non_abnf_dialects() {
        let mut pos = 0usize;
        let value = take_quoted_string_in(r#""a\n\t\"b\"""#, &mut pos, 0, RailroadDialect::Ir)
            .unwrap()
            .unwrap();
        assert_eq!(value.text, "a\n\t\"b\"");
    }

    #[test]
    fn abnf_strings_slice_without_escape_decoding() {
        let mut pos = 0usize;
        let value = take_quoted_string_in(r#""a\n""#, &mut pos, 0, RailroadDialect::Abnf)
            .unwrap()
            .unwrap();
        assert_eq!(value.text, r#"a\n"#);
    }

    #[test]
    fn parses_repeat_bounds() {
        assert_eq!(parse_abnf_repeat_bounds("*"), (0, None));
        assert_eq!(parse_abnf_repeat_bounds("1*"), (1, None));
        assert_eq!(parse_abnf_repeat_bounds("*2"), (0, Some(2)));
        assert_eq!(parse_abnf_repeat_bounds("1*2"), (1, Some(2)));
        assert_eq!(parse_abnf_repeat_bounds("3"), (3, Some(3)));
    }
}
