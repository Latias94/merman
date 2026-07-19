use crate::SourceSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Keyword {
    As,
    Catch,
    Critical,
    Else,
    False,
    Finally,
    Group,
    If,
    In,
    New,
    Nil,
    Opt,
    Par,
    Ref,
    Return,
    Section,
    Title,
    True,
    Try,
    While,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TokenKind {
    Keyword(Keyword),
    Identifier(String),
    StringLiteral { value: String, closed: bool },
    Integer(String),
    Number(String),
    Color(String),
    Annotation(String),
    ReturnAnnotation,
    StarterAnnotation,
    Comment(String),
    Divider(String),
    EventPayload(String),
    Modifier,
    Newline,
    Colon,
    Semicolon,
    Comma,
    Assign,
    Dot,
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    StereotypeOpen,
    StereotypeClose,
    Arrow,
    ReturnArrow,
    Operator(String),
    Other(char),
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Token {
    pub(super) kind: TokenKind,
    pub(super) span: SourceSpan,
}

impl Token {
    fn new(kind: TokenKind, start: usize, end: usize) -> Self {
        Self {
            kind,
            span: SourceSpan::new(start, end),
        }
    }
}

pub(super) fn lex(source: &str) -> Vec<Token> {
    Lexer::new(source)
        .filter(|token| !matches!(token.kind, TokenKind::Modifier))
        .collect()
}

struct Lexer<'a> {
    source: &'a str,
    offset: usize,
    line_start: usize,
    title_allowed: bool,
    event_mode: bool,
    emitted_eof: bool,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            offset: 0,
            line_start: 0,
            title_allowed: true,
            event_mode: false,
            emitted_eof: false,
        }
    }

    fn rest(&self) -> &'a str {
        &self.source[self.offset..]
    }

    fn starts_with(&self, value: &str) -> bool {
        self.rest().starts_with(value)
    }

    fn bump_char(&mut self) -> Option<char> {
        let ch = self.rest().chars().next()?;
        self.offset += ch.len_utf8();
        Some(ch)
    }

    fn skip_horizontal_space(&mut self) {
        while self
            .rest()
            .chars()
            .next()
            .is_some_and(|ch| matches!(ch, ' ' | '\t'))
        {
            self.bump_char();
        }
    }

    fn word(&mut self) -> Token {
        let start = self.offset;
        self.bump_char();
        while self
            .rest()
            .chars()
            .next()
            .is_some_and(is_identifier_continue)
        {
            self.bump_char();
        }
        let value = &self.source[start..self.offset];
        let kind = if matches!(value, "const" | "readonly" | "static" | "await") {
            TokenKind::Modifier
        } else {
            keyword(value, self.title_allowed, self.source, self.offset)
                .map(TokenKind::Keyword)
                .unwrap_or_else(|| TokenKind::Identifier(value.to_string()))
        };
        if !matches!(kind, TokenKind::Keyword(Keyword::Title)) && value != "zenuml" {
            self.title_allowed = false;
        }
        Token::new(kind, start, self.offset)
    }

    fn number(&mut self) -> Token {
        let start = self.offset;
        while self
            .rest()
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            self.offset += 1;
        }
        let mut is_number = false;
        if self.starts_with(".") {
            is_number = true;
            self.offset += 1;
            while self
                .rest()
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_digit())
            {
                self.offset += 1;
            }
        }
        while self
            .rest()
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic())
        {
            is_number = true;
            self.offset += 1;
        }
        let value = self.source[start..self.offset].to_string();
        let kind = if is_number {
            TokenKind::Number(value)
        } else {
            TokenKind::Integer(value)
        };
        self.title_allowed = false;
        Token::new(kind, start, self.offset)
    }

    fn string(&mut self) -> Token {
        let start = self.offset;
        self.offset += 1;
        let mut value = String::new();
        let mut closed = false;
        while self.offset < self.source.len() {
            if self.starts_with("\"\"") {
                value.push('"');
                self.offset += 2;
                continue;
            }
            let Some(ch) = self.rest().chars().next() else {
                break;
            };
            if ch == '"' {
                self.offset += 1;
                closed = true;
                break;
            }
            if matches!(ch, '\r' | '\n') {
                break;
            }
            value.push(ch);
            self.offset += ch.len_utf8();
        }
        self.title_allowed = false;
        Token::new(
            TokenKind::StringLiteral { value, closed },
            start,
            self.offset,
        )
    }

    fn annotation(&mut self) -> Token {
        let start = self.offset;
        self.offset += 1;
        while self
            .rest()
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            self.offset += 1;
        }
        let value = &self.source[start..self.offset];
        let kind = if value.eq_ignore_ascii_case("@starter") {
            TokenKind::StarterAnnotation
        } else if value.eq_ignore_ascii_case("@return") || value.eq_ignore_ascii_case("@reply") {
            TokenKind::ReturnAnnotation
        } else {
            TokenKind::Annotation(value.trim_start_matches('@').to_string())
        };
        self.title_allowed = false;
        Token::new(kind, start, self.offset)
    }

    fn color(&mut self) -> Token {
        let start = self.offset;
        self.offset += 1;
        while self
            .rest()
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_hexdigit())
        {
            self.offset += 1;
        }
        self.title_allowed = false;
        Token::new(
            TokenKind::Color(self.source[start..self.offset].to_string()),
            start,
            self.offset,
        )
    }

    fn comment(&mut self) -> Token {
        let start = self.offset;
        self.offset += 2;
        let body_start = self.offset;
        while self
            .rest()
            .chars()
            .next()
            .is_some_and(|ch| !matches!(ch, '\r' | '\n'))
        {
            self.bump_char();
        }
        let value = self.source[body_start..self.offset].trim().to_string();
        Token::new(TokenKind::Comment(value), start, self.offset)
    }

    fn divider(&mut self, start: usize) -> Token {
        while self
            .rest()
            .chars()
            .next()
            .is_some_and(|ch| !matches!(ch, '\r' | '\n'))
        {
            self.bump_char();
        }
        self.title_allowed = false;
        Token::new(
            TokenKind::Divider(self.source[start..self.offset].trim().to_string()),
            start,
            self.offset,
        )
    }

    fn event_payload(&mut self) -> Option<Token> {
        let start = self.offset;
        while self
            .rest()
            .chars()
            .next()
            .is_some_and(|ch| !matches!(ch, '\r' | '\n'))
        {
            self.bump_char();
        }
        (start < self.offset).then(|| {
            Token::new(
                TokenKind::EventPayload(self.source[start..self.offset].trim().to_string()),
                start,
                self.offset,
            )
        })
    }

    fn fixed(&mut self, width: usize, kind: TokenKind) -> Token {
        let start = self.offset;
        self.offset += width;
        self.title_allowed = false;
        Token::new(kind, start, self.offset)
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        if self.event_mode {
            self.event_mode = false;
            if let Some(payload) = self.event_payload() {
                return Some(payload);
            }
        }
        if self.offset >= self.source.len() {
            if self.emitted_eof {
                return None;
            }
            self.emitted_eof = true;
            return Some(Token::new(TokenKind::Eof, self.offset, self.offset));
        }

        let before_space = self.offset;
        self.skip_horizontal_space();
        if self.offset >= self.source.len() {
            return self.next();
        }
        if before_space == self.line_start && self.starts_with("==") {
            return Some(self.divider(before_space));
        }

        let start = self.offset;
        let ch = self.rest().chars().next()?;
        if matches!(ch, '\r' | '\n') {
            if ch == '\r' && self.starts_with("\r\n") {
                self.offset += 2;
            } else {
                self.bump_char();
            }
            self.line_start = self.offset;
            return Some(Token::new(TokenKind::Newline, start, self.offset));
        }
        if self.starts_with("//") {
            return Some(self.comment());
        }
        if is_identifier_start(ch) {
            return Some(self.word());
        }
        if ch.is_ascii_digit() {
            return Some(self.number());
        }
        if ch == '"' {
            return Some(self.string());
        }
        if ch == '@' {
            return Some(self.annotation());
        }
        if ch == '#' {
            return Some(self.color());
        }

        for (text, kind) in [
            ("-->", TokenKind::ReturnArrow),
            ("->", TokenKind::Arrow),
            ("<<", TokenKind::StereotypeOpen),
            (">>", TokenKind::StereotypeClose),
            (">=", TokenKind::Operator(">=".to_string())),
            ("<=", TokenKind::Operator("<=".to_string())),
            ("==", TokenKind::Operator("==".to_string())),
            ("!=", TokenKind::Operator("!=".to_string())),
            ("&&", TokenKind::Operator("&&".to_string())),
            ("||", TokenKind::Operator("||".to_string())),
        ] {
            if self.starts_with(text) {
                return Some(self.fixed(text.len(), kind));
            }
        }

        let token = match ch {
            ':' => {
                let token = self.fixed(1, TokenKind::Colon);
                self.event_mode = true;
                return Some(token);
            }
            ';' => self.fixed(1, TokenKind::Semicolon),
            ',' => self.fixed(1, TokenKind::Comma),
            '=' => self.fixed(1, TokenKind::Assign),
            '.' => self.fixed(1, TokenKind::Dot),
            '(' => self.fixed(1, TokenKind::OpenParen),
            ')' => self.fixed(1, TokenKind::CloseParen),
            '{' => self.fixed(1, TokenKind::OpenBrace),
            '}' => self.fixed(1, TokenKind::CloseBrace),
            '[' => self.fixed(1, TokenKind::OpenBracket),
            ']' => self.fixed(1, TokenKind::CloseBracket),
            '+' | '-' | '*' | '/' | '%' | '^' | '!' | '>' | '<' => {
                self.bump_char();
                self.title_allowed = false;
                Token::new(TokenKind::Operator(ch.to_string()), start, self.offset)
            }
            _ => {
                self.bump_char();
                self.title_allowed = false;
                Token::new(TokenKind::Other(ch), start, self.offset)
            }
        };
        Some(token)
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

fn keyword(value: &str, title_allowed: bool, source: &str, after_word: usize) -> Option<Keyword> {
    Some(match value {
        "as" => Keyword::As,
        "catch" => Keyword::Catch,
        "critical" => Keyword::Critical,
        "else" => Keyword::Else,
        "false" => Keyword::False,
        "finally" => Keyword::Finally,
        "group" => Keyword::Group,
        "if" => Keyword::If,
        "in" => Keyword::In,
        "new" => Keyword::New,
        "nil" | "null" => Keyword::Nil,
        "opt" => Keyword::Opt,
        "par" => Keyword::Par,
        "ref" => Keyword::Ref,
        "return" => Keyword::Return,
        "section" | "frame" => Keyword::Section,
        "true" => Keyword::True,
        "try" => Keyword::Try,
        "while" | "for" | "foreach" | "forEach" | "loop" => Keyword::While,
        "title" if title_allowed && title_right_edge_is_directive(source, after_word) => {
            Keyword::Title
        }
        _ => return None,
    })
}

fn title_right_edge_is_directive(source: &str, mut offset: usize) -> bool {
    while source[offset..]
        .chars()
        .next()
        .is_some_and(|ch| matches!(ch, ' ' | '\t'))
    {
        offset += source[offset..].chars().next().map_or(0, char::len_utf8);
    }
    !source[offset..]
        .chars()
        .next()
        .is_some_and(|ch| matches!(ch, '.' | '(' | '='))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_predicate_distinguishes_directive_from_method() {
        let directive = lex("title Order Service\n");
        assert!(matches!(
            directive[0].kind,
            TokenKind::Keyword(Keyword::Title)
        ));

        let method = lex("title.render()\n");
        assert!(matches!(method[0].kind, TokenKind::Identifier(_)));
    }

    #[test]
    fn unicode_names_and_unclosed_strings_are_lexed_without_regex() {
        let tokens = lex("客户.创建(\"订单\n");
        assert!(matches!(tokens[0].kind, TokenKind::Identifier(ref v) if v == "客户"));
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::StringLiteral { closed: false, .. }))
        );
    }

    #[test]
    fn oracle_modifier_channel_is_absent_from_parser_tokens() {
        let tokens = lex("const result = await Service.call()\n");
        assert!(
            !tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Modifier))
        );
        assert!(matches!(tokens[0].kind, TokenKind::Identifier(ref value) if value == "result"));
    }
}
