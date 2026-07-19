use super::ast::*;
use super::lexer::{Keyword, Token, TokenKind};
use crate::diagrams::langium_common::{LangiumCommonField, parse_langium_common};
use crate::{MAX_DIAGRAM_NESTING_DEPTH, SourceSpan};

pub(super) fn parse(source: &str, tokens: Vec<Token>) -> ParsedSyntax {
    Parser::new(source, tokens).parse()
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    cursor: usize,
    diagnostics: Vec<SyntaxDiagnostic>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, tokens: Vec<Token>) -> Self {
        Self {
            source,
            tokens,
            cursor: 0,
            diagnostics: Vec::new(),
        }
    }

    fn parse(mut self) -> ParsedSyntax {
        self.skip_newlines();
        let header = self.take_name();
        match header {
            Some(header) if header.value.eq_ignore_ascii_case("zenuml") => {}
            Some(header) => self.error("expected `zenuml` header", header.span),
            None => self.error("expected `zenuml` header", self.insertion_span()),
        }
        self.finish_header_line();

        let mut leading_comment = self.take_leading_comments();
        let title = if self.at_keyword(Keyword::Title) {
            leading_comment = None;
            Some(self.parse_title())
        } else {
            None
        };
        let mut acc_title = None;
        let mut acc_descr = None;
        loop {
            self.skip_newlines();
            let offset = self.peek().span.start;
            let Some(common) = parse_langium_common(self.source, offset) else {
                break;
            };
            let value = SpannedText::new(common.fact.value, common.fact.value_span);
            match common.fact.field {
                LangiumCommonField::AccTitle => acc_title = Some(value),
                LangiumCommonField::AccDescr => acc_descr = Some(value),
                LangiumCommonField::Title => break,
            }
            self.advance_to_offset(offset + common.consumed);
            if let Some(diagnostic) = common.diagnostic {
                self.error(diagnostic.message, diagnostic.span);
            }
        }

        let mut participants = Vec::new();
        let mut groups = Vec::new();
        let mut starter = None;

        loop {
            self.skip_newlines();
            if leading_comment.is_none() {
                leading_comment = self.take_leading_comments();
            }
            if self.at_keyword(Keyword::Group) {
                groups.push(self.parse_group(leading_comment.take()));
                continue;
            }
            if matches!(self.peek_kind(), TokenKind::StarterAnnotation) {
                starter = Some(self.parse_starter());
                leading_comment = None;
                break;
            }
            if self.looks_like_participant_line() {
                if let Some(participant) = self.parse_participant(leading_comment.take()) {
                    participants.push(participant);
                }
                continue;
            }
            break;
        }

        let statements = self.parse_block(0, false, leading_comment.take());
        let document = SyntaxDocument {
            title,
            acc_title,
            acc_descr,
            participants,
            groups,
            starter,
            statements,
        };
        ParsedSyntax {
            document,
            diagnostics: self.diagnostics,
            tokens: self.tokens,
        }
    }

    fn parse_title(&mut self) -> SpannedText {
        let keyword = self.bump().clone();
        let start = keyword.span.end;
        let end = self.line_end_offset();
        self.consume_to_line_end();
        trimmed_text(self.source, start, end)
            .unwrap_or_else(|| SpannedText::new(String::new(), SourceSpan::new(end, end)))
    }

    fn advance_to_offset(&mut self, offset: usize) {
        while !self.at_eof() && self.peek().span.start < offset {
            self.bump();
        }
    }

    fn parse_group(&mut self, _comment: Option<SpannedText>) -> GroupSyntax {
        let start = self.bump().span.start;
        let name = self.take_name();
        let mut participants = Vec::new();
        if self
            .consume_simple(TokenKindDiscriminant::OpenBrace)
            .is_some()
        {
            loop {
                self.skip_newlines();
                let comment = self.take_leading_comments();
                if self
                    .consume_simple(TokenKindDiscriminant::CloseBrace)
                    .is_some()
                {
                    break;
                }
                if self.at_eof() {
                    self.error(
                        "unterminated ZenUML group; expected `}`",
                        self.insertion_span(),
                    );
                    break;
                }
                if self.looks_like_participant_line() {
                    if let Some(participant) = self.parse_participant(comment) {
                        participants.push(participant);
                    }
                } else {
                    let span = self.current_line_span();
                    self.error("expected participant declaration in ZenUML group", span);
                    self.synchronize_statement();
                }
            }
        } else {
            self.consume_to_line_end();
        }
        let end = self.previous_end().max(start);
        GroupSyntax {
            name,
            participants,
            span: SourceSpan::new(start, end),
        }
    }

    fn parse_starter(&mut self) -> SpannedText {
        let annotation = self.bump().clone();
        if self
            .consume_simple(TokenKindDiscriminant::OpenParen)
            .is_none()
        {
            self.consume_to_line_end();
            return SpannedText::new("_STARTER_", annotation.span);
        }
        let starter = self.take_name().unwrap_or_else(|| {
            self.error(
                "expected participant name in `@Starter(...)`",
                self.insertion_span(),
            );
            SpannedText::new("_STARTER_", self.insertion_span())
        });
        if self
            .consume_simple(TokenKindDiscriminant::CloseParen)
            .is_none()
        {
            self.error("expected `)` after ZenUML starter", self.insertion_span());
        }
        self.consume_to_line_end();
        starter
    }

    fn parse_participant(&mut self, comment: Option<SpannedText>) -> Option<ParticipantSyntax> {
        let start = self.peek().span.start;
        let participant_type = match self.peek().clone() {
            Token {
                kind: TokenKind::Annotation(value),
                span,
            } => {
                self.bump();
                Some(SpannedText::new(value, span))
            }
            _ => None,
        };
        let stereotype = if matches!(self.peek_kind(), TokenKind::StereotypeOpen) {
            self.bump();
            let value = self.take_name();
            if matches!(self.peek_kind(), TokenKind::StereotypeClose)
                || matches!(self.peek_kind(), TokenKind::Operator(value) if value == ">")
            {
                self.bump();
            }
            value
        } else {
            None
        };
        let emoji = if self
            .consume_simple(TokenKindDiscriminant::OpenBracket)
            .is_some()
        {
            let value = self.take_name();
            if self
                .consume_simple(TokenKindDiscriminant::CloseBracket)
                .is_none()
            {
                self.error(
                    "expected `]` after ZenUML participant emoji",
                    self.insertion_span(),
                );
            }
            value
        } else {
            None
        };
        let Some(name) = self.take_name() else {
            let span = self.current_line_span();
            self.error("expected ZenUML participant name", span);
            self.synchronize_statement();
            return None;
        };
        let width = match self.peek_kind() {
            TokenKind::Integer(value) => {
                let parsed = value.parse().ok();
                self.bump();
                parsed
            }
            _ => None,
        };
        let label = if self.at_keyword(Keyword::As) {
            self.bump();
            self.take_name()
        } else {
            None
        };
        let color = match self.peek().clone() {
            Token {
                kind: TokenKind::Color(value),
                span,
            } => {
                self.bump();
                Some(SpannedText::new(value, span))
            }
            _ => None,
        };
        if !self.at_line_end() && !matches!(self.peek_kind(), TokenKind::CloseBrace) {
            self.error(
                "unexpected token after ZenUML participant declaration",
                self.peek().span,
            );
        }
        self.consume_to_line_end();
        Some(ParticipantSyntax {
            name,
            label,
            participant_type,
            stereotype,
            emoji,
            width,
            color,
            comment,
            span: SourceSpan::new(start, self.previous_end().max(start)),
        })
    }

    fn parse_block(
        &mut self,
        depth: usize,
        stop_at_close: bool,
        mut pending_comment: Option<SpannedText>,
    ) -> Vec<StatementSyntax> {
        if depth > MAX_DIAGRAM_NESTING_DEPTH {
            self.error(
                format!("ZenUML nesting depth exceeds {MAX_DIAGRAM_NESTING_DEPTH}"),
                self.peek().span,
            );
            self.skip_balanced_block();
            return Vec::new();
        }

        let mut statements = Vec::new();
        loop {
            self.skip_newlines();
            if pending_comment.is_none() {
                pending_comment = self.take_leading_comments();
            }
            if stop_at_close && matches!(self.peek_kind(), TokenKind::CloseBrace) {
                self.bump();
                break;
            }
            if self.at_eof() {
                if stop_at_close {
                    self.error(
                        "unterminated ZenUML block; expected `}`",
                        self.insertion_span(),
                    );
                }
                break;
            }
            if matches!(self.peek_kind(), TokenKind::CloseBrace) {
                let span = self.bump().span;
                self.error("unexpected `}` in ZenUML document", span);
                continue;
            }
            let statement_start = self.cursor;
            if let Some(statement) = self.parse_statement(depth, pending_comment.take()) {
                statements.push(statement);
            } else if self.cursor == statement_start
                || !self.tokens[..self.cursor].last().is_some_and(|token| {
                    matches!(token.kind, TokenKind::Newline | TokenKind::Semicolon)
                })
            {
                self.synchronize_statement();
            }
        }
        statements
    }

    fn parse_statement(
        &mut self,
        depth: usize,
        comment: Option<SpannedText>,
    ) -> Option<StatementSyntax> {
        match self.peek_kind() {
            TokenKind::Keyword(Keyword::If) => self.parse_alternative(depth, comment),
            TokenKind::Keyword(Keyword::Try) => self.parse_try(depth, comment),
            TokenKind::Keyword(Keyword::Par) => {
                self.parse_single_fragment(depth, comment, FragmentKindSyntax::Parallel, true)
            }
            TokenKind::Keyword(Keyword::Opt) => {
                self.parse_single_fragment(depth, comment, FragmentKindSyntax::Optional, true)
            }
            TokenKind::Keyword(Keyword::Critical) => {
                self.parse_single_fragment(depth, comment, FragmentKindSyntax::Critical, true)
            }
            TokenKind::Keyword(Keyword::Section) | TokenKind::OpenBrace => {
                self.parse_single_fragment(depth, comment, FragmentKindSyntax::Section, true)
            }
            TokenKind::Keyword(Keyword::While) => {
                self.parse_single_fragment(depth, comment, FragmentKindSyntax::Loop, true)
            }
            TokenKind::Keyword(Keyword::Ref) => self.parse_reference(comment),
            TokenKind::Keyword(Keyword::Return) | TokenKind::ReturnAnnotation => {
                self.parse_return_statement(comment)
            }
            TokenKind::Divider(_) => self.parse_divider(comment),
            _ => self.parse_message_or_creation(depth, comment),
        }
    }

    fn parse_single_fragment(
        &mut self,
        depth: usize,
        comment: Option<SpannedText>,
        kind: FragmentKindSyntax,
        block_optional: bool,
    ) -> Option<StatementSyntax> {
        let start = self.peek().span.start;
        let anonymous = matches!(self.peek_kind(), TokenKind::OpenBrace);
        if !anonymous {
            self.bump();
        }
        let label = if matches!(self.peek_kind(), TokenKind::OpenParen) {
            self.take_parenthesized_text()
        } else {
            None
        };
        let statements = if self
            .consume_simple(TokenKindDiscriminant::OpenBrace)
            .is_some()
        {
            self.parse_block(depth + 1, true, None)
        } else if block_optional {
            self.consume_to_line_end();
            Vec::new()
        } else {
            self.error("expected `{` after ZenUML fragment", self.insertion_span());
            Vec::new()
        };
        let end = self.previous_end().max(start);
        let section = FragmentSectionSyntax {
            label: label.clone(),
            statements,
            span: SourceSpan::new(start, end),
        };
        Some(StatementSyntax {
            kind: StatementKindSyntax::Fragment(FragmentSyntax {
                kind,
                label,
                sections: vec![section],
            }),
            comment,
            span: SourceSpan::new(start, end),
        })
    }

    fn parse_alternative(
        &mut self,
        depth: usize,
        comment: Option<SpannedText>,
    ) -> Option<StatementSyntax> {
        let start = self.bump().span.start;
        let label = self.take_parenthesized_text();
        let first = self.parse_fragment_section(depth, label.clone(), "if")?;
        let mut sections = vec![first];

        loop {
            self.skip_newlines();
            if !self.at_keyword(Keyword::Else) {
                break;
            }
            let branch_start = self.bump().span.start;
            let branch_label = if self.at_keyword(Keyword::If) {
                self.bump();
                self.take_parenthesized_text()
            } else {
                Some(SpannedText::new(
                    "else",
                    SourceSpan::new(branch_start, self.previous_end()),
                ))
            };
            let Some(mut section) = self.parse_fragment_section(depth, branch_label, "else") else {
                break;
            };
            section.span.start = branch_start;
            sections.push(section);
        }
        let end = self.previous_end().max(start);
        Some(StatementSyntax {
            kind: StatementKindSyntax::Fragment(FragmentSyntax {
                kind: FragmentKindSyntax::Alternative,
                label,
                sections,
            }),
            comment,
            span: SourceSpan::new(start, end),
        })
    }

    fn parse_try(&mut self, depth: usize, comment: Option<SpannedText>) -> Option<StatementSyntax> {
        let start = self.bump().span.start;
        let try_label = SpannedText::new("try", SourceSpan::new(start, self.previous_end()));
        let first = self.parse_fragment_section(depth, Some(try_label), "try")?;
        let mut sections = vec![first];
        loop {
            self.skip_newlines();
            let (name, branch_start) = if self.at_keyword(Keyword::Catch) {
                ("catch", self.bump().span.start)
            } else if self.at_keyword(Keyword::Finally) {
                ("finally", self.bump().span.start)
            } else {
                break;
            };
            let details = if name == "catch" && matches!(self.peek_kind(), TokenKind::OpenParen) {
                self.take_parenthesized_text()
            } else {
                None
            };
            let label = details.map_or_else(
                || SpannedText::new(name, SourceSpan::new(branch_start, self.previous_end())),
                |details| {
                    SpannedText::new(
                        format!("{name} {}", details.value),
                        SourceSpan::new(branch_start, details.span.end),
                    )
                },
            );
            let Some(mut section) = self.parse_fragment_section(depth, Some(label), name) else {
                break;
            };
            section.span.start = branch_start;
            sections.push(section);
            if name == "finally" {
                break;
            }
        }
        let end = self.previous_end().max(start);
        Some(StatementSyntax {
            kind: StatementKindSyntax::Fragment(FragmentSyntax {
                kind: FragmentKindSyntax::TryCatchFinally,
                label: None,
                sections,
            }),
            comment,
            span: SourceSpan::new(start, end),
        })
    }

    fn parse_fragment_section(
        &mut self,
        depth: usize,
        label: Option<SpannedText>,
        context: &str,
    ) -> Option<FragmentSectionSyntax> {
        let start = label
            .as_ref()
            .map_or(self.peek().span.start, |label| label.span.start);
        if self
            .consume_simple(TokenKindDiscriminant::OpenBrace)
            .is_none()
        {
            // The selected 3.47.8 grammar requires a block for if/else/tcf. Keep editor recovery
            // local so later top-level statements still produce facts.
            self.error(
                format!("expected `{{` after ZenUML {context}"),
                self.insertion_span(),
            );
            self.consume_to_line_end();
            return Some(FragmentSectionSyntax {
                label,
                statements: Vec::new(),
                span: SourceSpan::new(start, self.previous_end().max(start)),
            });
        }
        let statements = self.parse_block(depth + 1, true, None);
        Some(FragmentSectionSyntax {
            label,
            statements,
            span: SourceSpan::new(start, self.previous_end().max(start)),
        })
    }

    fn parse_reference(&mut self, comment: Option<SpannedText>) -> Option<StatementSyntax> {
        let start = self.bump().span.start;
        let mut participants = Vec::new();
        if self
            .consume_simple(TokenKindDiscriminant::OpenParen)
            .is_none()
        {
            self.error("expected `(` after ZenUML `ref`", self.insertion_span());
        } else {
            loop {
                if let Some(name) = self.take_name() {
                    participants.push(name);
                }
                if self.consume_simple(TokenKindDiscriminant::Comma).is_some() {
                    continue;
                }
                break;
            }
            if self
                .consume_simple(TokenKindDiscriminant::CloseParen)
                .is_none()
            {
                self.error("expected `)` after ZenUML reference", self.insertion_span());
            }
        }
        self.consume_simple(TokenKindDiscriminant::Semicolon);
        self.consume_to_line_end();
        let end = self.previous_end().max(start);
        Some(StatementSyntax {
            kind: StatementKindSyntax::Reference(ReferenceSyntax {
                participants,
                span: SourceSpan::new(start, end),
            }),
            comment,
            span: SourceSpan::new(start, end),
        })
    }

    fn parse_return_statement(&mut self, comment: Option<SpannedText>) -> Option<StatementSyntax> {
        let start = self.bump().span.start;
        let return_annotation = matches!(
            self.tokens[self.cursor.saturating_sub(1)].kind,
            TokenKind::ReturnAnnotation
        );
        let head = self.take_statement_head();
        let end = head
            .last()
            .map_or(self.previous_end(), |token| token.span.end);
        let has_return_arrow = head
            .iter()
            .any(|token| matches!(token.kind, TokenKind::ReturnArrow));
        let return_syntax = if return_annotation || has_return_arrow {
            self.classify_return_head(&head)
        } else {
            ReturnSyntax {
                from: None,
                from_emoji: None,
                to: None,
                to_emoji: None,
                value: spanned_from_tokens(self.source, &head),
            }
        };
        self.consume_statement_terminator();
        Some(StatementSyntax {
            kind: StatementKindSyntax::Return(return_syntax),
            comment,
            span: SourceSpan::new(start, end.max(start)),
        })
    }

    fn classify_return_head(&mut self, head: &[Token]) -> ReturnSyntax {
        let arrow = head
            .iter()
            .position(|token| matches!(token.kind, TokenKind::ReturnArrow | TokenKind::Arrow));
        let colon = head
            .iter()
            .position(|token| matches!(token.kind, TokenKind::Colon));
        let to_end = colon.unwrap_or(head.len());
        let (from, from_emoji, to, to_emoji) = if let Some(arrow) = arrow {
            let (from, from_emoji) = last_endpoint(&head[..arrow]);
            let (to, to_emoji) = first_endpoint(&head[arrow + 1..to_end]);
            (from, from_emoji, to, to_emoji)
        } else if colon.is_some() {
            let (to, to_emoji) = first_endpoint(&head[..to_end]);
            (None, None, to, to_emoji)
        } else {
            (None, None, None, None)
        };
        let value = colon.and_then(|colon| spanned_from_tokens(self.source, &head[colon + 1..]));
        if arrow.is_none() && colon.is_none() {
            return ReturnSyntax {
                from: None,
                from_emoji: None,
                to: None,
                to_emoji: None,
                value: spanned_from_tokens(self.source, head),
            };
        }
        ReturnSyntax {
            from,
            from_emoji,
            to,
            to_emoji,
            value,
        }
    }

    fn parse_divider(&mut self, comment: Option<SpannedText>) -> Option<StatementSyntax> {
        let token = self.bump().clone();
        let TokenKind::Divider(value) = token.kind else {
            return None;
        };
        let label_value = value
            .trim_matches(|character: char| character == '=' || character.is_whitespace())
            .to_string();
        let label = SpannedText::new(label_value, token.span);
        self.consume_to_line_end();
        Some(StatementSyntax {
            kind: StatementKindSyntax::Divider(label),
            comment,
            span: token.span,
        })
    }

    fn parse_message_or_creation(
        &mut self,
        depth: usize,
        comment: Option<SpannedText>,
    ) -> Option<StatementSyntax> {
        let start = self.peek().span.start;
        let head = self.take_statement_head();
        if head.is_empty() {
            self.error("expected ZenUML statement", self.peek().span);
            return None;
        }
        let body = if self
            .consume_simple(TokenKindDiscriminant::OpenBrace)
            .is_some()
        {
            self.parse_block(depth + 1, true, None)
        } else {
            Vec::new()
        };
        self.consume_statement_terminator();
        let end = self.previous_end().max(start);

        let assignment_index = top_level_position(&head, |kind| matches!(kind, TokenKind::Assign));
        let assignment = assignment_index.and_then(|index| last_name(&head[..index]));
        let body_start = assignment_index.map_or(0, |index| index + 1);
        if head
            .get(body_start)
            .is_some_and(|token| matches!(token.kind, TokenKind::Keyword(Keyword::New)))
        {
            return self.creation_statement(
                start,
                end,
                comment,
                assignment,
                &head[body_start..],
                body,
            );
        }

        if top_level_position(&head, |kind| matches!(kind, TokenKind::ReturnArrow)).is_some() {
            return Some(StatementSyntax {
                kind: StatementKindSyntax::Return(self.classify_return_head(&head[body_start..])),
                comment,
                span: SourceSpan::new(start, end),
            });
        }

        let arrow = top_level_position(&head, |kind| matches!(kind, TokenKind::Arrow));
        let colon = top_level_position(&head, |kind| matches!(kind, TokenKind::Colon));
        if let Some(colon) = colon {
            let to_start = arrow.map_or(body_start, |index| index + 1);
            let (from, from_emoji) = arrow
                .map(|index| last_endpoint(&head[body_start..index]))
                .unwrap_or_default();
            let (to, to_emoji) = first_endpoint(&head[to_start..colon]);
            let signature =
                spanned_from_tokens(self.source, &head[colon + 1..]).unwrap_or_else(|| {
                    SpannedText::new(
                        String::new(),
                        SourceSpan::new(head[colon].span.end, head[colon].span.end),
                    )
                });
            return Some(StatementSyntax {
                kind: StatementKindSyntax::Message(MessageSyntax {
                    from,
                    from_emoji,
                    to,
                    to_emoji,
                    signature,
                    assignment,
                    style: MessageStyleSyntax::Asynchronous,
                    body,
                }),
                comment,
                span: SourceSpan::new(start, end),
            });
        }

        let dot = top_level_position(&head[body_start..], |kind| matches!(kind, TokenKind::Dot))
            .map(|index| body_start + index);
        let (from, from_emoji, to, to_emoji, signature) = if let Some(dot) = dot {
            let arrow = arrow.filter(|arrow| *arrow < dot);
            let to_start = arrow.map_or(body_start, |arrow| arrow + 1);
            let (from, from_emoji) = arrow
                .map(|arrow| last_endpoint(&head[body_start..arrow]))
                .unwrap_or_default();
            let (to, to_emoji) = last_endpoint(&head[to_start..dot]);
            let signature = spanned_from_tokens(self.source, &head[dot + 1..]);
            (from, from_emoji, to, to_emoji, signature)
        } else if is_function_like(&head[body_start..]) {
            (
                None,
                None,
                None,
                None,
                spanned_from_tokens(self.source, &head[body_start..]),
            )
        } else if let Some(arrow) = arrow {
            let (from, from_emoji) = last_endpoint(&head[body_start..arrow]);
            let (to, to_emoji) = first_endpoint(&head[arrow + 1..]);
            (
                from,
                from_emoji,
                to,
                to_emoji,
                Some(SpannedText::new(
                    String::new(),
                    SourceSpan::new(head[arrow].span.end, head[arrow].span.end),
                )),
            )
        } else {
            self.error("unsupported ZenUML statement", SourceSpan::new(start, end));
            return None;
        };
        let Some(signature) = signature else {
            self.error(
                "expected ZenUML message signature",
                SourceSpan::new(start, end),
            );
            return None;
        };
        Some(StatementSyntax {
            kind: StatementKindSyntax::Message(MessageSyntax {
                from,
                from_emoji,
                to,
                to_emoji,
                signature,
                assignment,
                style: MessageStyleSyntax::Synchronous,
                body,
            }),
            comment,
            span: SourceSpan::new(start, end),
        })
    }

    fn creation_statement(
        &mut self,
        start: usize,
        end: usize,
        comment: Option<SpannedText>,
        assignment: Option<SpannedText>,
        creation: &[Token],
        body: Vec<StatementSyntax>,
    ) -> Option<StatementSyntax> {
        let Some(constructor) = first_name(&creation[1..]) else {
            self.error(
                "expected constructor after ZenUML `new`",
                SourceSpan::new(start, end),
            );
            return None;
        };
        let signature =
            spanned_from_tokens(self.source, &creation[1..]).unwrap_or_else(|| constructor.clone());
        Some(StatementSyntax {
            kind: StatementKindSyntax::Creation(CreationSyntax {
                constructor,
                assignment,
                signature,
                body,
            }),
            comment,
            span: SourceSpan::new(start, end),
        })
    }

    fn take_statement_head(&mut self) -> Vec<Token> {
        let mut out = Vec::new();
        let mut parens = 0usize;
        let mut brackets = 0usize;
        while !self.at_eof() {
            match self.peek_kind() {
                TokenKind::OpenParen => parens += 1,
                TokenKind::CloseParen => parens = parens.saturating_sub(1),
                TokenKind::OpenBracket => brackets += 1,
                TokenKind::CloseBracket => brackets = brackets.saturating_sub(1),
                TokenKind::OpenBrace if parens == 0 && brackets == 0 => break,
                TokenKind::CloseBrace if parens == 0 && brackets == 0 => break,
                TokenKind::Semicolon | TokenKind::Newline if parens == 0 && brackets == 0 => break,
                _ => {}
            }
            out.push(self.bump().clone());
        }
        if parens > 0 {
            self.error(
                "unterminated ZenUML invocation; expected `)`",
                self.insertion_span(),
            );
        }
        if brackets > 0 {
            self.error(
                "unterminated ZenUML emoji; expected `]`",
                self.insertion_span(),
            );
        }
        out
    }

    fn take_parenthesized_text(&mut self) -> Option<SpannedText> {
        let open = self.consume_simple(TokenKindDiscriminant::OpenParen)?;
        let content_start = open.span.end;
        let mut depth = 1usize;
        let mut content_end = content_start;
        while !self.at_eof() {
            match self.peek_kind() {
                TokenKind::OpenParen => {
                    depth += 1;
                    content_end = self.bump().span.end;
                }
                TokenKind::CloseParen => {
                    depth -= 1;
                    if depth == 0 {
                        let close_start = self.bump().span.start;
                        return trimmed_text(self.source, content_start, close_start).or_else(
                            || {
                                Some(SpannedText::new(
                                    String::new(),
                                    SourceSpan::new(content_start, content_start),
                                ))
                            },
                        );
                    }
                    content_end = self.bump().span.end;
                }
                TokenKind::OpenBrace | TokenKind::Newline if depth == 1 => {
                    // 3.47.8 accepts a missing closing parenthesis during editor recovery.
                    return trimmed_text(self.source, content_start, content_end).or_else(|| {
                        Some(SpannedText::new(
                            String::new(),
                            SourceSpan::new(content_start, content_start),
                        ))
                    });
                }
                _ => content_end = self.bump().span.end,
            }
        }
        self.error(
            "unterminated ZenUML expression; expected `)`",
            self.insertion_span(),
        );
        trimmed_text(self.source, content_start, content_end)
    }

    fn take_leading_comments(&mut self) -> Option<SpannedText> {
        self.skip_newlines();
        let mut values = Vec::new();
        let mut start = None;
        let mut end = 0usize;
        while let Token {
            kind: TokenKind::Comment(value),
            ..
        } = self.peek().clone()
        {
            let token = self.bump().clone();
            start.get_or_insert(token.span.start);
            end = token.span.end;
            values.push(value);
            self.skip_newlines();
        }
        start.map(|start| SpannedText::new(values.join("\n"), SourceSpan::new(start, end)))
    }

    fn take_name(&mut self) -> Option<SpannedText> {
        let token = self.peek().clone();
        let value = match token.kind {
            TokenKind::Identifier(value) => value,
            TokenKind::StringLiteral { value, .. } => value,
            _ => return None,
        };
        self.cursor += 1;
        Some(SpannedText::new(value, token.span))
    }

    fn looks_like_participant_line(&self) -> bool {
        if !matches!(
            self.peek_kind(),
            TokenKind::Annotation(_)
                | TokenKind::StereotypeOpen
                | TokenKind::OpenBracket
                | TokenKind::Identifier(_)
                | TokenKind::StringLiteral { .. }
        ) {
            return false;
        }
        let mut cursor = self.cursor;
        while cursor < self.tokens.len() {
            match &self.tokens[cursor].kind {
                TokenKind::Newline | TokenKind::Eof | TokenKind::CloseBrace => return true,
                TokenKind::Dot
                | TokenKind::Arrow
                | TokenKind::ReturnArrow
                | TokenKind::Colon
                | TokenKind::Assign
                | TokenKind::OpenParen => return false,
                _ => cursor += 1,
            }
        }
        true
    }

    fn finish_header_line(&mut self) {
        if !self.at_line_end() {
            self.error(
                "unexpected content after `zenuml` header",
                self.current_line_span(),
            );
        }
        self.consume_to_line_end();
    }

    fn consume_statement_terminator(&mut self) {
        self.consume_simple(TokenKindDiscriminant::Semicolon);
        if matches!(self.peek_kind(), TokenKind::Newline) {
            self.bump();
        }
    }

    fn consume_to_line_end(&mut self) {
        while !self.at_line_end() && !self.at_eof() {
            self.bump();
        }
        if matches!(self.peek_kind(), TokenKind::Newline) {
            self.bump();
        }
    }

    fn synchronize_statement(&mut self) {
        let mut braces = 0usize;
        while !self.at_eof() {
            match self.peek_kind() {
                TokenKind::OpenBrace => {
                    braces += 1;
                    self.bump();
                }
                TokenKind::CloseBrace if braces > 0 => {
                    braces -= 1;
                    self.bump();
                }
                TokenKind::CloseBrace => break,
                TokenKind::Newline | TokenKind::Semicolon if braces == 0 => {
                    self.bump();
                    break;
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    fn skip_balanced_block(&mut self) {
        let mut depth = 0usize;
        while !self.at_eof() {
            match self.peek_kind() {
                TokenKind::OpenBrace => depth += 1,
                TokenKind::CloseBrace if depth == 0 => {
                    self.bump();
                    break;
                }
                TokenKind::CloseBrace => depth -= 1,
                _ => {}
            }
            self.bump();
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek_kind(), TokenKind::Newline) {
            self.bump();
        }
    }

    fn at_line_end(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Newline | TokenKind::Eof | TokenKind::CloseBrace
        )
    }

    fn at_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.peek_kind(), TokenKind::Keyword(found) if *found == keyword)
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn consume_simple(&mut self, expected: TokenKindDiscriminant) -> Option<Token> {
        if expected.matches(self.peek_kind()) {
            Some(self.bump().clone())
        } else {
            None
        }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.cursor.min(self.tokens.len().saturating_sub(1))]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn bump(&mut self) -> &Token {
        let index = self.cursor.min(self.tokens.len().saturating_sub(1));
        if self.cursor + 1 < self.tokens.len() {
            self.cursor += 1;
        }
        &self.tokens[index]
    }

    fn previous_end(&self) -> usize {
        self.tokens
            .get(self.cursor.saturating_sub(1))
            .map_or(0, |token| token.span.end)
    }

    fn insertion_span(&self) -> SourceSpan {
        SourceSpan::new(self.peek().span.start, self.peek().span.start)
    }

    fn current_line_span(&self) -> SourceSpan {
        let start = self.peek().span.start;
        SourceSpan::new(start, self.line_end_offset().max(start))
    }

    fn line_end_offset(&self) -> usize {
        self.tokens[self.cursor..]
            .iter()
            .find(|token| matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
            .map_or(self.source.len(), |token| token.span.start)
    }

    fn error(&mut self, message: impl Into<String>, span: SourceSpan) {
        self.diagnostics.push(SyntaxDiagnostic {
            message: message.into(),
            span,
        });
    }
}

#[derive(Clone, Copy)]
enum TokenKindDiscriminant {
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    Comma,
    Semicolon,
}

impl TokenKindDiscriminant {
    fn matches(self, kind: &TokenKind) -> bool {
        matches!(
            (self, kind),
            (Self::OpenParen, TokenKind::OpenParen)
                | (Self::CloseParen, TokenKind::CloseParen)
                | (Self::OpenBrace, TokenKind::OpenBrace)
                | (Self::CloseBrace, TokenKind::CloseBrace)
                | (Self::OpenBracket, TokenKind::OpenBracket)
                | (Self::CloseBracket, TokenKind::CloseBracket)
                | (Self::Comma, TokenKind::Comma)
                | (Self::Semicolon, TokenKind::Semicolon)
        )
    }
}

fn first_name(tokens: &[Token]) -> Option<SpannedText> {
    tokens.iter().find_map(token_name)
}

fn last_name(tokens: &[Token]) -> Option<SpannedText> {
    tokens.iter().rev().find_map(token_name)
}

fn first_endpoint(tokens: &[Token]) -> (Option<SpannedText>, Option<SpannedText>) {
    let (emoji, after_emoji) = bracket_emoji(tokens);
    let name = after_emoji
        .and_then(|index| first_name(&tokens[index..]))
        .or_else(|| first_name(tokens));
    (name, emoji)
}

fn last_endpoint(tokens: &[Token]) -> (Option<SpannedText>, Option<SpannedText>) {
    let (emoji, after_emoji) = bracket_emoji(tokens);
    let name = after_emoji
        .and_then(|index| last_name(&tokens[index..]))
        .or_else(|| last_name(tokens));
    (name, emoji)
}

fn bracket_emoji(tokens: &[Token]) -> (Option<SpannedText>, Option<usize>) {
    let Some(open) = tokens
        .iter()
        .position(|token| matches!(token.kind, TokenKind::OpenBracket))
    else {
        return (None, None);
    };
    let Some(relative_close) = tokens[open + 1..]
        .iter()
        .position(|token| matches!(token.kind, TokenKind::CloseBracket))
    else {
        return (None, None);
    };
    let close = open + 1 + relative_close;
    (first_name(&tokens[open + 1..close]), Some(close + 1))
}

fn token_name(token: &Token) -> Option<SpannedText> {
    match &token.kind {
        TokenKind::Identifier(value) | TokenKind::StringLiteral { value, .. } => {
            Some(SpannedText::new(value.clone(), token.span))
        }
        _ => None,
    }
}

fn is_function_like(tokens: &[Token]) -> bool {
    first_name(tokens).is_some()
        && tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::OpenParen))
}

fn top_level_position(
    tokens: &[Token],
    mut predicate: impl FnMut(&TokenKind) -> bool,
) -> Option<usize> {
    let mut parens = 0usize;
    let mut brackets = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        if parens == 0 && brackets == 0 && predicate(&token.kind) {
            return Some(index);
        }
        match token.kind {
            TokenKind::OpenParen => parens += 1,
            TokenKind::CloseParen => parens = parens.saturating_sub(1),
            TokenKind::OpenBracket => brackets += 1,
            TokenKind::CloseBracket => brackets = brackets.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn spanned_from_tokens(source: &str, tokens: &[Token]) -> Option<SpannedText> {
    let first = tokens.first()?;
    let last = tokens.last()?;
    trimmed_text(source, first.span.start, last.span.end)
}

fn trimmed_text(source: &str, start: usize, end: usize) -> Option<SpannedText> {
    let raw = source.get(start..end)?;
    let trimmed_start = raw.len().saturating_sub(raw.trim_start().len());
    let trimmed_end = raw.trim_end().len();
    (trimmed_start <= trimmed_end).then(|| {
        SpannedText::new(
            raw[trimmed_start..trimmed_end].to_string(),
            SourceSpan::new(start + trimmed_start, start + trimmed_end),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_source(source: &str) -> ParsedSyntax {
        parse(source, super::super::lexer::lex(source))
    }

    #[test]
    fn parses_nested_official_grammar_without_line_translation() {
        let parsed = parse_source(
            "zenuml\n@Actor Client #FFEBE6\n@Starter(Client)\nService.call(x) {\n  if(x != null) {\n    Worker.run(x)\n  }\n}\n",
        );
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.document.participants.len(), 1);
        assert_eq!(parsed.document.statements.len(), 1);
    }

    #[test]
    fn recovers_after_invalid_statement() {
        let parsed = parse_source("zenuml\n@Starter(A)\nA.call()\n?\nB.call()\n");
        assert!(!parsed.diagnostics.is_empty());
        assert_eq!(parsed.document.statements.len(), 2);
    }
}
