use super::{
    ArrowToken, DirectionStatementToken, FlowchartLexemeComponent, LabeledText, LexError,
    LinkToken, NodeLabelToken, SubgraphHeader, TitleKind, Tok, destruct_end_link,
    destruct_start_link, is_ecmascript_trim_char, lex, parse_label_text,
};
use crate::{EditorLexemeKind, SourceSpan};
use std::collections::VecDeque;

fn prepend_statement_keyword(
    components: &mut Vec<FlowchartLexemeComponent>,
    start: usize,
    end: usize,
) {
    components.insert(
        0,
        FlowchartLexemeComponent::new(EditorLexemeKind::Keyword, SourceSpan::new(start, end)),
    );
}

fn skip_ecmascript_whitespace(input: &str, mut pos: usize) -> usize {
    debug_assert!(input.is_char_boundary(pos));
    while pos < input.len() {
        let ch = input[pos..]
            .chars()
            .next()
            .expect("position before input end must contain a character");
        if !is_ecmascript_trim_char(ch) {
            break;
        }
        pos += ch.len_utf8();
    }
    pos
}

fn find_pipe_label_end(input: &str, mut pos: usize) -> Option<usize> {
    #[derive(Clone, Copy)]
    enum State {
        Text,
        String,
        MarkdownString,
    }

    let mut state = State::Text;
    while pos < input.len() {
        let rest = &input[pos..];
        match state {
            State::Text => {
                if rest.starts_with('|') {
                    return Some(pos);
                }
                if rest.starts_with("\"`") {
                    state = State::MarkdownString;
                    pos += 2;
                    continue;
                }
                if rest.starts_with('"') {
                    state = State::String;
                    pos += 1;
                    continue;
                }
            }
            State::String => {
                if rest.starts_with('"') {
                    state = State::Text;
                    pos += 1;
                    continue;
                }
            }
            State::MarkdownString => {
                if rest.starts_with("`\"") {
                    state = State::Text;
                    pos += 2;
                    continue;
                }
            }
        }

        let ch = rest
            .chars()
            .next()
            .expect("pipe label scan position must contain a character");
        pos += ch.len_utf8();
    }
    None
}

pub(super) struct Lexer<'input> {
    pub(super) input: &'input str,
    pub(super) pos: usize,
    pub(super) pending: VecDeque<std::result::Result<(usize, Tok, usize), LexError>>,
    pub(super) allow_header_direction: bool,
    pub(super) recover_partial_node_labels: bool,
}

impl<'input> Lexer<'input> {
    pub(super) fn normalize_direction_token(dir: &str) -> &str {
        if dir == "TD" { "TB" } else { dir }
    }

    pub(super) fn new(input: &'input str) -> Self {
        Self {
            input,
            pos: 0,
            pending: VecDeque::new(),
            allow_header_direction: false,
            recover_partial_node_labels: false,
        }
    }

    pub(super) fn recovering(input: &'input str) -> Self {
        Self {
            recover_partial_node_labels: true,
            ..Self::new(input)
        }
    }

    pub(super) fn bump(&mut self) -> Option<u8> {
        if self.pos >= self.input.len() {
            return None;
        }
        let b = self.input.as_bytes()[self.pos];
        self.pos += 1;
        Some(b)
    }

    pub(super) fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    pub(super) fn peek2(&self) -> Option<[u8; 2]> {
        if self.pos + 1 >= self.input.len() {
            return None;
        }
        Some([
            self.input.as_bytes()[self.pos],
            self.input.as_bytes()[self.pos + 1],
        ])
    }

    pub(super) fn starts_with_kw(&self, kw: &str) -> bool {
        let rest = &self.input[self.pos..];
        if !rest.starts_with(kw) {
            return false;
        }
        let after = self.pos + kw.len();
        if after >= self.input.len() {
            return true;
        }
        let b = self.input.as_bytes()[after];
        !b.is_ascii_alphanumeric() && b != b'_' && b != b'-'
    }

    pub(super) fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == b'\r' {
                self.pos += 1;
                continue;
            }
            break;
        }
    }

    pub(super) fn lex_sep(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        match self.peek()? {
            b'\n' => {
                let bytes = self.input.as_bytes();
                let mut look = self.pos + 1;
                while look < bytes.len() {
                    match bytes[look] {
                        b' ' | b'\t' | b'\r' => look += 1,
                        _ => break,
                    }
                }
                if look < bytes.len() {
                    let is_linkish = match bytes[look] {
                        b'~' => {
                            look + 2 < bytes.len()
                                && bytes[look + 1] == b'~'
                                && bytes[look + 2] == b'~'
                        }
                        b'=' => look + 1 < bytes.len() && bytes[look + 1] == b'=',
                        b'-' => {
                            look + 1 < bytes.len()
                                && (bytes[look + 1] == b'-' || bytes[look + 1] == b'.')
                        }
                        b'o' | b'x' | b'<' => {
                            look + 2 < bytes.len()
                                && ((bytes[look + 1] == b'-'
                                    && (bytes[look + 2] == b'-' || bytes[look + 2] == b'.'))
                                    || (bytes[look + 1] == b'=' && bytes[look + 2] == b'='))
                        }
                        _ => false,
                    };
                    if is_linkish {
                        self.pos = look;
                        return None;
                    }
                }

                self.pos += 1;
                Some((start, Tok::Sep, self.pos))
            }
            b';' => {
                self.pos += 1;
                Some((start, Tok::Sep, self.pos))
            }
            _ => None,
        }
    }

    pub(super) fn lex_comment(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let Some([b'%', b'%']) = self.peek2() else {
            return None;
        };
        // Consume until newline or EOF. If newline exists, emit Sep to keep statement boundaries.
        self.pos += 2;
        while let Some(b) = self.peek() {
            if b == b'\n' {
                self.pos += 1;
                return Some((start, Tok::Sep, self.pos));
            }
            self.pos += 1;
        }
        None
    }

    pub(super) fn lex_direction(&mut self) -> Option<(usize, Tok, usize)> {
        if !self.allow_header_direction {
            return None;
        }
        let start = self.pos;
        let rest = &self.input[self.pos..];
        for d in ["TB", "TD", "BT", "LR", "RL"] {
            if rest.starts_with(d) {
                let after = self.pos + d.len();
                if after < self.input.len() {
                    let b = self.input.as_bytes()[after];
                    if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                        continue;
                    }
                }
                self.pos = after;
                self.allow_header_direction = false;
                let d = Self::normalize_direction_token(d);
                return Some((start, Tok::Direction(d.to_string()), self.pos));
            }
        }

        if let Some(&b) = rest.as_bytes().first() {
            let mapped = match b {
                b'>' => Some("LR"),
                b'<' => Some("RL"),
                b'^' => Some("BT"),
                b'v' => Some("TB"),
                _ => None,
            };
            if let Some(d) = mapped {
                let after = self.pos + 1;
                if after < self.input.len() {
                    let next = self.input.as_bytes()[after];
                    if next.is_ascii_alphanumeric() || next == b'_' || next == b'-' {
                        return None;
                    }
                }
                self.pos = after;
                self.allow_header_direction = false;
                return Some((start, Tok::Direction(d.to_string()), self.pos));
            }
        }

        None
    }

    pub(super) fn lex_direction_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("direction") {
            return None;
        }
        self.pos += "direction".len();
        let keyword_end = self.pos;
        self.skip_ws();

        let direction_start = self.pos;
        while let Some(b) = self.peek() {
            if b.is_ascii_whitespace() || b == b';' {
                break;
            }
            self.pos += 1;
        }
        let direction_end = self.pos;
        while let Some(b) = self.peek() {
            if b == b'\n' || b == b';' {
                break;
            }
            self.pos += 1;
        }
        let statement_end = self.pos;
        let direction = &self.input[direction_start..direction_end];
        let selection = SourceSpan::new(direction_start, direction_end);
        let mut lexeme_components = vec![FlowchartLexemeComponent::new(
            EditorLexemeKind::Keyword,
            SourceSpan::new(start, keyword_end),
        )];
        if selection.start < selection.end {
            lexeme_components.push(FlowchartLexemeComponent::new(
                EditorLexemeKind::Literal,
                selection,
            ));
        }

        let Some(dir) = ["TB", "TD", "BT", "LR", "RL"]
            .into_iter()
            .find(|candidate| *candidate == direction)
        else {
            let error = LexError::with_span("invalid flowchart direction", selection)
                .expecting(crate::EditorExpectedSyntaxKind::DirectionValue, selection);
            if self.recover_partial_node_labels {
                return Some(Ok((
                    start,
                    Tok::DirectionStmt(DirectionStatementToken {
                        direction: String::new(),
                        selection,
                        lexeme_components,
                        recovery_error: Some(error),
                    }),
                    statement_end,
                )));
            }
            return Some(Err(error));
        };

        Some(Ok((
            start,
            Tok::DirectionStmt(DirectionStatementToken {
                direction: dir.to_string(),
                selection,
                lexeme_components,
                recovery_error: None,
            }),
            statement_end,
        )))
    }

    pub(super) fn capture_to_stmt_end(&mut self) -> (usize, String, usize) {
        let start = self.pos;
        let mut in_double_quote = false;
        let mut in_single_quote = false;
        let mut escaped = false;
        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];
            if in_double_quote {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_double_quote = false;
                }
                self.pos += 1;
                continue;
            }
            if in_single_quote {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'\'' {
                    in_single_quote = false;
                }
                self.pos += 1;
                continue;
            }

            if b == b'"' {
                in_double_quote = true;
                self.pos += 1;
                continue;
            }
            if b == b'\'' {
                in_single_quote = true;
                self.pos += 1;
                continue;
            }

            if b == b'\n' || b == b';' {
                break;
            }
            self.pos += 1;
        }
        (start, self.input[start..self.pos].to_string(), self.pos)
    }

    pub(super) fn capture_to_stmt_end_from(&mut self, start: usize) -> (usize, String, usize) {
        self.pos = start;
        self.capture_to_stmt_end()
    }

    pub(super) fn capture_recovery_to_stmt_end_from(
        &mut self,
        start: usize,
    ) -> (usize, String, usize) {
        self.pos = start;
        while self.pos < self.input.len() {
            match self.input.as_bytes()[self.pos] {
                b'\n' | b';' => break,
                _ => self.pos += 1,
            }
        }
        (start, self.input[start..self.pos].to_string(), self.pos)
    }

    pub(super) fn lex_style_sep(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.input[self.pos..].starts_with(":::") {
            self.pos += 3;
            return Some((start, Tok::StyleSep, self.pos));
        }
        None
    }

    pub(super) fn lex_shape_data(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.input[self.pos..].starts_with("@{") {
            return None;
        }
        self.pos += 2;

        // Mermaid's Jison lexer has dedicated states for shapeData strings:
        // - it allows `}` inside double-quoted strings
        // - it rewrites `\n\s*` inside double-quoted strings to `<br/>`
        //
        // We mimic that behavior here while returning a single `ShapeData` token.
        let bytes = self.input.as_bytes();
        let mut out = String::new();
        let mut segment_start = self.pos;
        let mut in_string = false;

        while self.pos < self.input.len() {
            let b = bytes[self.pos];
            if !in_string {
                if b == b'"' {
                    out.push_str(&self.input[segment_start..self.pos + 1]);
                    self.pos += 1;
                    segment_start = self.pos;
                    in_string = true;
                    continue;
                }
                if b == b'}' {
                    out.push_str(&self.input[segment_start..self.pos]);
                    self.pos += 1;
                    return Some(Ok((start, Tok::ShapeData(out), self.pos)));
                }
                self.pos += 1;
                continue;
            }

            if b == b'"' {
                out.push_str(&self.input[segment_start..self.pos + 1]);
                self.pos += 1;
                segment_start = self.pos;
                in_string = false;
                continue;
            }

            if b == b'\n' {
                out.push_str(&self.input[segment_start..self.pos]);
                out.push_str("<br/>");
                self.pos += 1;
                while self.pos < self.input.len() {
                    match bytes[self.pos] {
                        b' ' | b'\t' | b'\r' => self.pos += 1,
                        _ => break,
                    }
                }
                segment_start = self.pos;
                continue;
            }

            self.pos += 1;
        }

        out.push_str(&self.input[segment_start..self.pos]);
        let span = SourceSpan::new(start, self.pos);
        let expected = super::shape_value_expected_span(self.input, start, self.pos)
            .unwrap_or(SourceSpan::new(self.pos, self.pos));
        Some(Err(LexError::with_span(
            "Unterminated shape data (missing `}`)",
            span,
        )
        .expecting(
            crate::EditorExpectedSyntaxKind::ShapeValue,
            expected,
        )))
    }

    pub(super) fn lex_edge_id(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let bytes = self.input.as_bytes();
        if start >= bytes.len() {
            return None;
        }
        let first = bytes[start];
        if !first.is_ascii_alphanumeric() && first != b'_' {
            return None;
        }
        let mut i = start;
        while i < bytes.len() {
            let b = bytes[i];
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                i += 1;
                continue;
            }
            break;
        }
        if i >= bytes.len() || bytes[i] != b'@' {
            return None;
        }
        let next = bytes.get(i + 1).copied();
        if matches!(next, Some(b'{') | Some(b'"')) {
            return None;
        }
        self.pos = i + 1;
        let id = self.input[start..i].to_string();
        Some((start, Tok::EdgeId(id), self.pos))
    }

    pub(super) fn lex_style_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("style") {
            return None;
        }
        self.pos += "style".len();
        let keyword_end = self.pos;
        self.skip_ws();
        let (rest_start, rest, end) = self.capture_to_stmt_end();
        match lex::parse_style_stmt(&rest) {
            Ok(mut stmt) => {
                lex::attach_style_stmt_spans(&mut stmt, &rest, rest_start);
                prepend_statement_keyword(&mut stmt.lexeme_components, start, keyword_end);
                Some(Ok((start, Tok::StyleStmt(stmt), end)))
            }
            Err(e) => Some(Err(e)),
        }
    }

    pub(super) fn lex_classdef_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("classDef") {
            return None;
        }
        self.pos += "classDef".len();
        let keyword_end = self.pos;
        self.skip_ws();
        let (rest_start, rest, end) = self.capture_to_stmt_end();
        match lex::parse_classdef_stmt(&rest) {
            Ok(mut stmt) => {
                lex::attach_classdef_stmt_spans(&mut stmt, &rest, rest_start);
                prepend_statement_keyword(&mut stmt.lexeme_components, start, keyword_end);
                Some(Ok((start, Tok::ClassDefStmt(stmt), end)))
            }
            Err(e) => Some(Err(e)),
        }
    }

    pub(super) fn lex_class_assign_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("class") {
            return None;
        }
        self.pos += "class".len();
        let keyword_end = self.pos;
        self.skip_ws();
        let (rest_start, rest, end) = self.capture_to_stmt_end();
        match lex::parse_class_assign_stmt(&rest) {
            Ok(mut stmt) => {
                lex::attach_class_assign_stmt_spans(&mut stmt, &rest, rest_start);
                prepend_statement_keyword(&mut stmt.lexeme_components, start, keyword_end);
                Some(Ok((start, Tok::ClassAssignStmt(stmt), end)))
            }
            Err(e) => Some(Err(e)),
        }
    }

    pub(super) fn lex_click_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("click") {
            return None;
        }
        self.pos += "click".len();
        let keyword_end = self.pos;
        self.skip_ws();
        let (rest_start, rest, end) = self.capture_to_stmt_end();
        match lex::parse_click_stmt(&rest, rest_start) {
            Ok(mut stmt) => {
                prepend_statement_keyword(&mut stmt.lexeme_components, start, keyword_end);
                Some(Ok((start, Tok::ClickStmt(stmt), end)))
            }
            Err(e) => Some(Err(e)),
        }
    }

    pub(super) fn lex_link_style_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("linkStyle") {
            return None;
        }
        self.pos += "linkStyle".len();
        let keyword_end = self.pos;
        self.skip_ws();
        let (rest_start, rest, end) = self.capture_to_stmt_end();
        match lex::parse_link_style_stmt(&rest, rest_start) {
            Ok(mut stmt) => {
                prepend_statement_keyword(&mut stmt.lexeme_components, start, keyword_end);
                Some(Ok((start, Tok::LinkStyleStmt(stmt), end)))
            }
            Err(e) => Some(Err(e)),
        }
    }

    pub(super) fn lex_subgraph_header_after_keyword(
        &mut self,
        keyword_start: usize,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        // Match Mermaid's flowchart parser behavior: it consumes a single "SPACE" token after the
        // `subgraph` keyword, while any additional whitespace becomes part of the subgraph header
        // token (`textNoTags`). This affects whether `FlowDB.addSubGraph(...)` decides to auto-generate
        // a `subGraphN` id.
        //
        // Example:
        // - `subgraph main`   -> header text has no whitespace, id stays `main`
        // - `subgraph  main`  -> header text begins with whitespace, id becomes `subGraphN`
        let rest = &self.input[self.pos..];
        if rest.starts_with('\n') || rest.starts_with("\r\n") || rest.starts_with(';') {
            return None;
        }
        if let Some(ch) = rest.chars().next()
            && is_ecmascript_trim_char(ch)
        {
            self.pos += ch.len_utf8();
        }

        let start = self.pos;
        if start >= self.input.len() {
            return None;
        }
        match self.input.as_bytes()[start] {
            b'\n' | b'\r' | b';' => return None,
            _ => {}
        }

        let mut in_quote = false;
        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];
            if in_quote {
                if b == b'"' {
                    in_quote = false;
                }
                self.pos += 1;
                continue;
            }
            if b == b'"' {
                in_quote = true;
                self.pos += 1;
                continue;
            }
            if b == b'\n' || b == b'\r' || b == b';' || b == b'[' {
                break;
            }
            self.pos += 1;
        }

        let raw_id_end = self.pos;
        let raw_id = self.input[start..raw_id_end].to_string();
        let mut raw_title = raw_id.clone();
        let mut title_kind = TitleKind::Text;
        let mut id_equals_title = true;
        let mut lexeme_components = Vec::new();
        let (trimmed_id, trimmed_id_span) = trimmed_slice_with_span(self.input, start, raw_id_end);
        if !trimmed_id.is_empty() {
            lexeme_components.push(FlowchartLexemeComponent::new(
                EditorLexemeKind::Identifier,
                trimmed_id_span,
            ));
        }

        if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'[' {
            id_equals_title = false;
            let open_start = self.pos;
            self.pos += 1;
            lexeme_components.push(FlowchartLexemeComponent::new(
                EditorLexemeKind::Delimiter,
                SourceSpan::new(open_start, self.pos),
            ));
            let title_start = self.pos;
            in_quote = false;
            while self.pos < self.input.len() {
                let b = self.input.as_bytes()[self.pos];
                if in_quote {
                    if b == b'"' {
                        in_quote = false;
                    }
                    self.pos += 1;
                    continue;
                }
                if b == b'"' {
                    in_quote = true;
                    self.pos += 1;
                    continue;
                }
                if b == b']' {
                    break;
                }
                if b == b'\n' || b == b'\r' {
                    break;
                }
                self.pos += 1;
            }
            raw_title = self.input[title_start..self.pos].to_string();
            let (trimmed_title, trimmed_title_span) =
                trimmed_slice_with_span(self.input, title_start, self.pos);
            if !trimmed_title.is_empty() {
                lexeme_components.push(FlowchartLexemeComponent::new(
                    EditorLexemeKind::String,
                    trimmed_title_span,
                ));
            }
            let parsed_title = match lex::parse_node_label_text(&raw_title) {
                Ok(parsed) => parsed,
                Err(error) => {
                    return Some(Err(LexError::with_span(
                        error.message,
                        SourceSpan::new(title_start, self.pos),
                    )));
                }
            };
            title_kind = parsed_title.kind;
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b']' {
                let close_start = self.pos;
                self.pos += 1;
                lexeme_components.push(FlowchartLexemeComponent::new(
                    EditorLexemeKind::Delimiter,
                    SourceSpan::new(close_start, self.pos),
                ));
            }
        } else if raw_id.contains('"') && !(raw_id.starts_with('"') && raw_id.ends_with('"')) {
            return Some(Err(LexError::with_span(
                "Invalid subgraph header: quoted strings cannot be mixed with unquoted text",
                SourceSpan::new(start, raw_id_end),
            )));
        }

        Some(Ok((
            start,
            Tok::SubgraphHeader(SubgraphHeader {
                raw_id,
                header_span: Some(SourceSpan::new(keyword_start, self.pos)),
                raw_id_span: Some(SourceSpan::new(start, raw_id_end)),
                raw_title,
                title_kind,
                id_equals_title,
                lexeme_components,
            }),
            self.pos,
        )))
    }

    pub(super) fn lex_amp(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.peek()? != b'&' {
            return None;
        }
        self.pos += 1;
        Some((start, Tok::Amp, self.pos))
    }

    pub(super) fn lex_id(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let bytes = self.input.as_bytes();
        if start >= bytes.len() {
            return None;
        }
        let first = bytes[start];
        if !first.is_ascii_alphanumeric() && first != b'_' {
            return None;
        }
        self.pos += 1;

        while self.pos < bytes.len() {
            if self.pos + 1 < bytes.len()
                && (bytes[self.pos] == b'-' && bytes[self.pos + 1] == b'-'
                    || bytes[self.pos] == b'=' && bytes[self.pos + 1] == b'=')
            {
                break;
            }
            let b = bytes[self.pos];
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
                continue;
            }
            if b == b'-' {
                if self.pos + 1 < bytes.len() && bytes[self.pos + 1] == b'-' {
                    break;
                }
                // Dotted edges start with `-.` (e.g. `A-.->B`). Avoid consuming the link start as
                // part of the id while still allowing ids like `subcontainer-child`.
                if self.pos + 1 < bytes.len() && bytes[self.pos + 1] == b'.' {
                    break;
                }
                self.pos += 1;
                continue;
            }
            if b == b'.' {
                // Allow dots inside ids (Mermaid supports nodes like `P1.5`), but avoid consuming
                // the `.` that starts a dotted link token like `.->` when it is directly adjacent
                // to an id (e.g. `A.->B`).
                if self.pos + 1 < bytes.len() && bytes[self.pos + 1] == b'-' {
                    break;
                }
                self.pos += 1;
                continue;
            }
            break;
        }

        if self.pos <= start {
            return None;
        }

        let id = self.input[start..self.pos].to_string();
        Some((start, Tok::Id(id), self.pos))
    }

    pub(super) fn lex_arrow_and_label(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        let bytes = self.input.as_bytes();

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum LinkFamily {
            Normal,
            Thick,
            Dotted,
            Invisible,
        }

        struct LinkEndMatch {
            label_end: usize,
            match_end: usize,
            operator: String,
            operator_span: SourceSpan,
        }

        struct StartLinkMatch {
            family: LinkFamily,
            operator: String,
            operator_span: SourceSpan,
            match_end: usize,
        }

        let match_link_end = |pos: usize, family: LinkFamily| -> Option<LinkEndMatch> {
            let len = bytes.len();
            let label_end = pos;
            let operator_start = skip_ecmascript_whitespace(self.input, pos);
            if operator_start >= len {
                return None;
            }

            let mut cur = operator_start;
            let start_marker = bytes[cur];
            if matches!(start_marker, b'x' | b'o' | b'<') {
                cur += 1;
                if cur >= len {
                    return None;
                }
            }

            match family {
                LinkFamily::Invisible => {
                    cur = operator_start;
                    let mut tildes = 0usize;
                    while cur < len && bytes[cur] == b'~' {
                        tildes += 1;
                        cur += 1;
                    }
                    if tildes < 3 {
                        return None;
                    }
                }
                LinkFamily::Normal => {
                    let hyphen_start = cur;
                    while cur < len && bytes[cur] == b'-' {
                        cur += 1;
                    }
                    let hyphens = cur - hyphen_start;
                    if hyphens < 2 {
                        return None;
                    }
                    if cur < len {
                        match bytes[cur] {
                            b'x' | b'o' | b'>' => {
                                cur += 1;
                            }
                            _ => {
                                // Open-ended edge: `--+` + `-` requires at least 3 hyphens total.
                                if hyphens < 3 {
                                    return None;
                                }
                            }
                        }
                    } else if hyphens < 3 {
                        return None;
                    }
                }
                LinkFamily::Thick => {
                    let eq_start = cur;
                    while cur < len && bytes[cur] == b'=' {
                        cur += 1;
                    }
                    let eqs = cur - eq_start;
                    if eqs < 2 {
                        return None;
                    }
                    if cur < len {
                        match bytes[cur] {
                            b'x' | b'o' | b'>' => {
                                cur += 1;
                            }
                            _ => {
                                // Open-ended edge: `==+` + `=` requires at least 3 '=' total.
                                if eqs < 3 {
                                    return None;
                                }
                            }
                        }
                    } else if eqs < 3 {
                        return None;
                    }
                }
                LinkFamily::Dotted => {
                    if cur < len && bytes[cur] == b'-' {
                        cur += 1;
                    }
                    let mut dots = 0usize;
                    while cur < len && bytes[cur] == b'.' {
                        dots += 1;
                        cur += 1;
                    }
                    if dots == 0 {
                        return None;
                    }
                    if cur >= len || bytes[cur] != b'-' {
                        return None;
                    }
                    cur += 1;
                    if cur < len && matches!(bytes[cur], b'x' | b'o' | b'>') {
                        cur += 1;
                    }
                }
            }

            let operator_end = cur;
            let match_end = skip_ecmascript_whitespace(self.input, operator_end);
            Some(LinkEndMatch {
                label_end,
                match_end,
                operator: self.input[operator_start..operator_end].to_string(),
                operator_span: SourceSpan::new(operator_start, operator_end),
            })
        };

        let compute_link =
            |end: String, start: Option<String>| -> std::result::Result<LinkToken, LexError> {
                let (end_type, stroke, length) = destruct_end_link(&end);
                let mut edge_type = end_type;

                if let Some(start_str) = start.as_deref() {
                    let (start_type, start_stroke) = destruct_start_link(start_str);
                    if start_stroke != stroke.as_str() {
                        return Err(LexError::new(
                            "Invalid link: stroke mismatch between start and end".to_string(),
                        ));
                    }

                    if start_type == "arrow_open" {
                        edge_type = edge_type.clone();
                    } else {
                        if start_type != edge_type.as_str() {
                            return Err(LexError::new(
                                "Invalid link: start/end arrowhead mismatch".to_string(),
                            ));
                        }
                        edge_type = format!("double_{start_type}");
                    }

                    if edge_type == "double_arrow" {
                        edge_type = "double_arrow_point".to_string();
                    }
                }

                Ok(LinkToken {
                    end,
                    edge_type,
                    stroke,
                    length,
                })
            };

        // 1) Prefer full LINK tokens, matching their source-order priority before START_LINK.
        let families = [
            LinkFamily::Invisible,
            LinkFamily::Thick,
            LinkFamily::Normal,
            LinkFamily::Dotted,
        ];
        for family in families {
            if let Some(link_match) = match_link_end(self.pos, family) {
                self.pos = link_match.match_end;
                let arrow_end = link_match.match_end;
                let link = match compute_link(link_match.operator, None) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                let arrow = ArrowToken {
                    link,
                    lexeme_components: vec![FlowchartLexemeComponent::new(
                        EditorLexemeKind::Operator,
                        link_match.operator_span,
                    )],
                    recovery_error: None,
                };

                // Optional pipe label: `A--x|label|B` or `A --> |label| B`.
                let pipe_pos = self.pos;
                if pipe_pos < self.input.len() && bytes[pipe_pos] == b'|' {
                    self.pos = pipe_pos + 1;
                    let label_start = self.pos;
                    if let Some(label_end) = find_pipe_label_end(self.input, label_start) {
                        self.pos = label_end;
                        let raw = &self.input[label_start..self.pos];
                        let raw_span = SourceSpan::new(label_start, self.pos);
                        let parsed = match lex::parse_node_label_text(raw) {
                            Ok(parsed) => parsed,
                            Err(error) => {
                                self.pos += 1;
                                return Some(Err(LexError::with_span(error.message, raw_span)));
                            }
                        };
                        self.pos += 1;
                        let token_span = SourceSpan::new(pipe_pos, self.pos);
                        let mut label = labeled_text_with_spans(
                            self.input,
                            LabeledText {
                                text: parsed.text,
                                kind: parsed.kind,
                                span: None,
                                selection: None,
                                lexeme_components: Vec::new(),
                            },
                            token_span,
                            raw_span,
                        );
                        label.lexeme_components = label_lexeme_components(
                            token_span,
                            SourceSpan::new(label_start, label_end),
                            raw_span,
                        );
                        self.pending
                            .push_back(Ok((pipe_pos, Tok::EdgeLabel(label), self.pos)));
                    } else {
                        self.pos = self.input.len();
                        let error = LexError::with_span(
                            "Unterminated flowchart pipe edge label",
                            SourceSpan::new(pipe_pos, self.pos),
                        )
                        .expecting(
                            crate::EditorExpectedSyntaxKind::Payload,
                            SourceSpan::new(self.pos, self.pos),
                        );
                        if self.recover_partial_node_labels {
                            let mut arrow = arrow;
                            arrow.recovery_error = Some(error);
                            return Some(Ok((start, Tok::Arrow(arrow), self.pos)));
                        }
                        return Some(Err(error));
                    }
                }

                return Some(Ok((start, Tok::Arrow(arrow), arrow_end)));
            }
        }

        // 2) START_LINK + edgeText + LINK (new notation): A-- text -->B
        let parse_start_link = |pos: usize| -> Option<StartLinkMatch> {
            let len = bytes.len();
            let operator_start = skip_ecmascript_whitespace(self.input, pos);
            if operator_start >= len {
                return None;
            }
            let mut cur = operator_start;
            if matches!(bytes[cur], b'x' | b'o' | b'<') {
                cur += 1;
                if cur >= len {
                    return None;
                }
            }

            if cur + 1 < len && bytes[cur] == b'-' && bytes[cur + 1] == b'-' {
                cur += 2;
                return Some(StartLinkMatch {
                    family: LinkFamily::Normal,
                    operator: self.input[operator_start..cur].to_string(),
                    operator_span: SourceSpan::new(operator_start, cur),
                    match_end: skip_ecmascript_whitespace(self.input, cur),
                });
            }
            if cur + 1 < len && bytes[cur] == b'=' && bytes[cur + 1] == b'=' {
                cur += 2;
                return Some(StartLinkMatch {
                    family: LinkFamily::Thick,
                    operator: self.input[operator_start..cur].to_string(),
                    operator_span: SourceSpan::new(operator_start, cur),
                    match_end: skip_ecmascript_whitespace(self.input, cur),
                });
            }
            if cur + 1 < len && bytes[cur] == b'-' && bytes[cur + 1] == b'.' {
                cur += 2;
                return Some(StartLinkMatch {
                    family: LinkFamily::Dotted,
                    operator: self.input[operator_start..cur].to_string(),
                    operator_span: SourceSpan::new(operator_start, cur),
                    match_end: skip_ecmascript_whitespace(self.input, cur),
                });
            }
            None
        };

        let Some(start_match) = parse_start_link(self.pos) else {
            let operator_start = skip_ecmascript_whitespace(self.input, self.pos);
            if self.input[operator_start..].starts_with("->") {
                self.pos = operator_start + 2;
                let selection = SourceSpan::new(operator_start, self.pos);
                return Some(Err(LexError::with_span(
                    "incomplete flowchart edge operator",
                    selection,
                )
                .expecting(crate::EditorExpectedSyntaxKind::Operator, selection)));
            }
            return None;
        };
        let family = start_match.family;
        let after_start = start_match.match_end;
        let edge_text_start = after_start;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum EdgeTextState {
            Plain,
            String,
            MarkdownString,
        }

        let arrow_token =
            |link: LinkToken, end_span: SourceSpan, recovery_error: Option<LexError>| ArrowToken {
                link,
                lexeme_components: vec![
                    FlowchartLexemeComponent::new(
                        EditorLexemeKind::Operator,
                        start_match.operator_span,
                    ),
                    FlowchartLexemeComponent::new(EditorLexemeKind::Operator, end_span),
                ],
                recovery_error,
            };

        let mut scan = edge_text_start;
        let mut state = EdgeTextState::Plain;
        while scan < self.input.len() {
            let rest = &self.input[scan..];
            match state {
                EdgeTextState::Plain => {
                    if rest.starts_with("\"`") {
                        state = EdgeTextState::MarkdownString;
                        scan += 2;
                        continue;
                    }
                    if rest.starts_with('"') {
                        state = EdgeTextState::String;
                        scan += 1;
                        continue;
                    }
                    let whitespace_end = skip_ecmascript_whitespace(self.input, scan);
                    if let Some(link_match) = match_link_end(scan, family) {
                        let match_start = link_match.label_end;
                        let match_end = link_match.match_end;
                        let raw_text = &self.input[edge_text_start..match_start];
                        let raw_span = SourceSpan::new(edge_text_start, match_start);
                        self.pos = match_end;

                        let parsed = match lex::parse_edge_text(raw_text) {
                            Ok(parsed) => parsed,
                            Err(error) => {
                                let error = LexError::with_span(error.message, raw_span)
                                    .expecting(crate::EditorExpectedSyntaxKind::Payload, raw_span);
                                if self.recover_partial_node_labels
                                    && let Ok(link) = compute_link(
                                        link_match.operator,
                                        Some(start_match.operator.clone()),
                                    )
                                {
                                    let arrow =
                                        arrow_token(link, link_match.operator_span, Some(error));
                                    return Some(Ok((start, Tok::Arrow(arrow), match_end)));
                                }
                                return Some(Err(error));
                            }
                        };
                        let link = match compute_link(
                            link_match.operator,
                            Some(start_match.operator.clone()),
                        ) {
                            Ok(v) => v,
                            Err(e) => return Some(Err(e)),
                        };
                        let arrow = arrow_token(link, link_match.operator_span, None);

                        let label = labeled_text_with_spans(
                            self.input,
                            LabeledText {
                                text: parsed.text,
                                kind: parsed.kind,
                                span: None,
                                selection: None,
                                lexeme_components: Vec::new(),
                            },
                            SourceSpan::new(edge_text_start, match_start),
                            raw_span,
                        );
                        self.pending.push_back(Ok((
                            edge_text_start,
                            Tok::EdgeLabel(label),
                            match_end,
                        )));
                        return Some(Ok((start, Tok::Arrow(arrow), after_start)));
                    }

                    if whitespace_end != scan {
                        // The terminator probe already inspected this whole whitespace run. Skip
                        // it once instead of probing every suffix, which would make long internal
                        // whitespace in an edge label quadratic.
                        scan = whitespace_end;
                        continue;
                    }

                    let invalid_edge_text = match family {
                        LinkFamily::Normal => rest.starts_with("--"),
                        LinkFamily::Thick => rest.starts_with('='),
                        LinkFamily::Dotted => rest.starts_with('.'),
                        LinkFamily::Invisible => false,
                    };
                    if invalid_edge_text {
                        let ch = rest
                            .chars()
                            .next()
                            .expect("edge label scan position must contain a character");
                        let error_end = scan + ch.len_utf8();
                        self.pos = error_end;
                        return Some(Err(LexError::with_span(
                            "Invalid character sequence in flowchart edge label",
                            SourceSpan::new(scan, error_end),
                        )));
                    }
                }
                EdgeTextState::String => {
                    if rest.starts_with('"') {
                        state = EdgeTextState::Plain;
                        scan += 1;
                        continue;
                    }
                }
                EdgeTextState::MarkdownString => {
                    if rest.starts_with("`\"") {
                        state = EdgeTextState::Plain;
                        scan += 2;
                        continue;
                    }
                    if rest.starts_with(['`', '"']) {
                        let ch = rest
                            .chars()
                            .next()
                            .expect("edge label scan position must contain a character");
                        let error_end = scan + ch.len_utf8();
                        self.pos = error_end;
                        return Some(Err(LexError::with_span(
                            "Invalid Markdown string in flowchart edge label",
                            SourceSpan::new(scan, error_end),
                        )));
                    }
                }
            }
            let ch = self.input[scan..]
                .chars()
                .next()
                .expect("edge label scan position must contain a character");
            scan += ch.len_utf8();
        }

        self.pos = self.input.len();
        Some(Err(LexError::with_span(
            "Unterminated edge label (missing link terminator)",
            SourceSpan::new(edge_text_start, self.pos),
        )
        .expecting(
            crate::EditorExpectedSyntaxKind::Operator,
            SourceSpan::new(start, after_start),
        )))
    }

    pub(super) fn lex_node_label(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        let rest = &self.input[self.pos..];

        if rest.starts_with("[\\") {
            let open = "[\\";
            let content_start = self.pos + open.len();
            let end_slash = lex::find_unquoted_delim(self.input, content_start, "/]");
            let end_backslash = lex::find_unquoted_delim(self.input, content_start, "\\]");

            let (end_start, close, shape) = match (end_slash, end_backslash) {
                (None, None) => {
                    if self.recover_partial_node_labels {
                        let (raw_start, raw, token_end) =
                            self.capture_recovery_to_stmt_end_from(content_start);
                        let token = build_partial_node_label_token_from_raw(
                            self.input,
                            "inv_trapezoid",
                            SourceSpan::new(start, token_end),
                            SourceSpan::new(content_start, token_end),
                            &raw,
                            SourceSpan::new(raw_start, token_end),
                            PartialNodeLabelRecovery {
                                trigger_span: Some(SourceSpan::new(start, content_start)),
                                error: LexError::with_span(
                                    "Unterminated node label (missing `/]` or `\\]`)",
                                    SourceSpan::new(start, token_end),
                                ),
                            },
                        );
                        self.pos = token_end;
                        return Some(Ok((start, token, self.pos)));
                    }
                    let (_, _, token_end) = self.capture_to_stmt_end_from(content_start);
                    self.pos = token_end;
                    return Some(Err(LexError::with_span(
                        "Unterminated node label (missing `/]` or `\\]`)",
                        SourceSpan::new(start, token_end),
                    )));
                }
                (Some(p), None) => (p, "/]", "inv_trapezoid"),
                (None, Some(p)) => (p, "\\]", "lean_left"),
                (Some(a), Some(b)) => {
                    if a <= b {
                        (a, "/]", "inv_trapezoid")
                    } else {
                        (b, "\\]", "lean_left")
                    }
                }
            };

            let token_end = end_start + close.len();
            let token = match build_node_label_token(
                self.input,
                shape,
                SourceSpan::new(start, token_end),
                SourceSpan::new(content_start, end_start),
                None,
            ) {
                Ok(v) => v,
                Err(e) => {
                    self.pos = token_end;
                    return Some(Err(e));
                }
            };
            self.pos = token_end;
            return Some(Ok((start, token, self.pos)));
        }

        if rest.starts_with("[/") {
            let open = "[/";
            let content_start = self.pos + open.len();
            let end_slash = lex::find_unquoted_delim(self.input, content_start, "/]");
            let end_backslash = lex::find_unquoted_delim(self.input, content_start, "\\]");

            let (end_start, close, shape) = match (end_slash, end_backslash) {
                (None, None) => {
                    if self.recover_partial_node_labels {
                        let (raw_start, raw, token_end) =
                            self.capture_recovery_to_stmt_end_from(content_start);
                        let token = build_partial_node_label_token_from_raw(
                            self.input,
                            "lean_right",
                            SourceSpan::new(start, token_end),
                            SourceSpan::new(content_start, token_end),
                            &raw,
                            SourceSpan::new(raw_start, token_end),
                            PartialNodeLabelRecovery {
                                trigger_span: Some(SourceSpan::new(start, content_start)),
                                error: LexError::with_span(
                                    "Unterminated node label (missing `/]` or `\\]`)",
                                    SourceSpan::new(start, token_end),
                                ),
                            },
                        );
                        self.pos = token_end;
                        return Some(Ok((start, token, self.pos)));
                    }
                    let (_, _, token_end) = self.capture_to_stmt_end_from(content_start);
                    self.pos = token_end;
                    return Some(Err(LexError::with_span(
                        "Unterminated node label (missing `/]` or `\\]`)",
                        SourceSpan::new(start, token_end),
                    )));
                }
                (Some(p), None) => (p, "/]", "lean_right"),
                (None, Some(p)) => (p, "\\]", "trapezoid"),
                (Some(a), Some(b)) => {
                    if a <= b {
                        (a, "/]", "lean_right")
                    } else {
                        (b, "\\]", "trapezoid")
                    }
                }
            };

            let token_end = end_start + close.len();
            let token = match build_node_label_token(
                self.input,
                shape,
                SourceSpan::new(start, token_end),
                SourceSpan::new(content_start, end_start),
                None,
            ) {
                Ok(v) => v,
                Err(e) => {
                    self.pos = token_end;
                    return Some(Err(e));
                }
            };
            self.pos = token_end;
            return Some(Ok((start, token, self.pos)));
        }

        let candidates: [(&str, &str, &str); 8] = [
            ("(((", ")))", "doublecircle"),
            ("{{", "}}", "hexagon"),
            ("[[", "]]", "subroutine"),
            ("(-", "-)", "ellipse"),
            ("([", "])", "stadium"),
            ("[(", ")]", "cylinder"),
            ("((", "))", "circle"),
            (">", "]", "odd"),
        ];

        for (open, close, shape) in candidates {
            if !rest.starts_with(open) {
                continue;
            }
            let content_start = self.pos + open.len();
            let token = if let Some(end_start) =
                lex::find_unquoted_delim(self.input, content_start, close)
            {
                let token_end = end_start + close.len();
                let token = match build_node_label_token(
                    self.input,
                    shape,
                    SourceSpan::new(start, token_end),
                    SourceSpan::new(content_start, end_start),
                    None,
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        self.pos = token_end;
                        return Some(Err(e));
                    }
                };
                self.pos = token_end;
                token
            } else {
                if !self.recover_partial_node_labels {
                    let (_, _, token_end) = self.capture_to_stmt_end_from(content_start);
                    self.pos = token_end;
                    return Some(Err(LexError::with_span(
                        format!("Unterminated node label (missing `{close}`)"),
                        SourceSpan::new(start, token_end),
                    )));
                }
                let (raw_start, raw, token_end) =
                    self.capture_recovery_to_stmt_end_from(content_start);
                let token = build_partial_node_label_token_from_raw(
                    self.input,
                    shape,
                    SourceSpan::new(start, token_end),
                    SourceSpan::new(content_start, token_end),
                    &raw,
                    SourceSpan::new(raw_start, token_end),
                    PartialNodeLabelRecovery {
                        trigger_span: Some(SourceSpan::new(start, content_start)),
                        error: LexError::with_span(
                            format!("Unterminated node label (missing `{close}`)"),
                            SourceSpan::new(start, token_end),
                        ),
                    },
                );
                self.pos = token_end;
                token
            };
            return Some(Ok((start, token, self.pos)));
        }

        if rest.starts_with("[") {
            let content_start = self.pos + 1;
            let token =
                if let Some(end_start) = lex::find_unquoted_delim(self.input, content_start, "]") {
                    let token_end = end_start + 1;
                    let raw = &self.input[content_start..end_start];
                    let raw_span = SourceSpan::new(content_start, end_start);
                    let (shape, label_raw, label_offset) = lex::parse_rect_border_label(raw);
                    let label_span = SourceSpan::new(
                        raw_span.start + label_offset,
                        raw_span.start + label_offset + label_raw.len(),
                    );
                    let token = match build_node_label_token_from_raw(
                        self.input,
                        shape,
                        SourceSpan::new(start, token_end),
                        SourceSpan::new(content_start, end_start),
                        label_raw,
                        label_span,
                        None,
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            self.pos = token_end;
                            return Some(Err(e));
                        }
                    };
                    self.pos = token_end;
                    token
                } else {
                    if !self.recover_partial_node_labels {
                        let (_, _, token_end) = self.capture_to_stmt_end_from(content_start);
                        self.pos = token_end;
                        return Some(Err(LexError::with_span(
                            "Unterminated node label (missing `]`)",
                            SourceSpan::new(start, token_end),
                        )));
                    }
                    let (raw_start, raw, token_end) =
                        self.capture_recovery_to_stmt_end_from(content_start);
                    let (shape, label_raw, label_offset) = lex::parse_rect_border_label(&raw);
                    let label_span = SourceSpan::new(
                        raw_start + label_offset,
                        raw_start + label_offset + label_raw.len(),
                    );
                    let token = build_partial_node_label_token_from_raw(
                        self.input,
                        shape,
                        SourceSpan::new(start, token_end),
                        SourceSpan::new(content_start, token_end),
                        label_raw,
                        label_span,
                        PartialNodeLabelRecovery {
                            trigger_span: Some(SourceSpan::new(start, content_start)),
                            error: LexError::with_span(
                                "Unterminated node label (missing `]`)",
                                SourceSpan::new(start, token_end),
                            ),
                        },
                    );
                    self.pos = token_end;
                    token
                };
            return Some(Ok((start, token, self.pos)));
        }

        if rest.starts_with("{") {
            let content_start = self.pos + 1;
            let token =
                if let Some(end_start) = lex::find_unquoted_delim(self.input, content_start, "}") {
                    let token_end = end_start + 1;
                    let token = match build_node_label_token(
                        self.input,
                        "diamond",
                        SourceSpan::new(start, token_end),
                        SourceSpan::new(content_start, end_start),
                        None,
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            self.pos = token_end;
                            return Some(Err(e));
                        }
                    };
                    self.pos = token_end;
                    token
                } else {
                    if !self.recover_partial_node_labels {
                        let (_, _, token_end) = self.capture_to_stmt_end_from(content_start);
                        self.pos = token_end;
                        return Some(Err(LexError::with_span(
                            "Unterminated node label (missing `}`)",
                            SourceSpan::new(start, token_end),
                        )));
                    }
                    let (raw_start, raw, token_end) =
                        self.capture_recovery_to_stmt_end_from(content_start);
                    let token = build_partial_node_label_token_from_raw(
                        self.input,
                        "diamond",
                        SourceSpan::new(start, token_end),
                        SourceSpan::new(content_start, token_end),
                        &raw,
                        SourceSpan::new(raw_start, token_end),
                        PartialNodeLabelRecovery {
                            trigger_span: Some(SourceSpan::new(start, content_start)),
                            error: LexError::with_span(
                                "Unterminated node label (missing `}`)",
                                SourceSpan::new(start, token_end),
                            ),
                        },
                    );
                    self.pos = token_end;
                    token
                };
            return Some(Ok((start, token, self.pos)));
        }

        if rest.starts_with("(") {
            let content_start = self.pos + 1;
            let token =
                if let Some(end_start) = lex::find_unquoted_delim(self.input, content_start, ")") {
                    let token_end = end_start + 1;
                    let token = match build_node_label_token(
                        self.input,
                        "round",
                        SourceSpan::new(start, token_end),
                        SourceSpan::new(content_start, end_start),
                        None,
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            self.pos = token_end;
                            return Some(Err(e));
                        }
                    };
                    self.pos = token_end;
                    token
                } else {
                    if !self.recover_partial_node_labels {
                        let (_, _, token_end) = self.capture_to_stmt_end_from(content_start);
                        self.pos = token_end;
                        return Some(Err(LexError::with_span(
                            "Unterminated node label (missing `)`)",
                            SourceSpan::new(start, token_end),
                        )));
                    }
                    let (raw_start, raw, token_end) =
                        self.capture_recovery_to_stmt_end_from(content_start);
                    let token = build_partial_node_label_token_from_raw(
                        self.input,
                        "round",
                        SourceSpan::new(start, token_end),
                        SourceSpan::new(content_start, token_end),
                        &raw,
                        SourceSpan::new(raw_start, token_end),
                        PartialNodeLabelRecovery {
                            trigger_span: Some(SourceSpan::new(start, content_start)),
                            error: LexError::with_span(
                                "Unterminated node label (missing `)`)",
                                SourceSpan::new(start, token_end),
                            ),
                        },
                    );
                    self.pos = token_end;
                    token
                };
            return Some(Ok((start, token, self.pos)));
        }

        None
    }
}

fn build_node_label_token(
    input: &str,
    shape: &str,
    token_span: SourceSpan,
    content_span: SourceSpan,
    trigger_span: Option<SourceSpan>,
) -> std::result::Result<Tok, LexError> {
    let raw = &input[content_span.start..content_span.end];
    let raw_span = content_span;
    build_node_label_token_from_raw(
        input,
        shape,
        token_span,
        content_span,
        raw,
        raw_span,
        trigger_span,
    )
}

fn build_node_label_token_from_raw(
    input: &str,
    shape: &str,
    token_span: SourceSpan,
    content_span: SourceSpan,
    raw: &str,
    raw_span: SourceSpan,
    trigger_span: Option<SourceSpan>,
) -> std::result::Result<Tok, LexError> {
    let text = lex::parse_node_label_text(raw)?;
    Ok(Tok::NodeLabel(NodeLabelToken {
        shape: shape.to_string(),
        text: labeled_text_with_spans(input, text, token_span, raw_span),
        trigger_span,
        lexeme_components: label_lexeme_components(token_span, content_span, raw_span),
        recovery_error: None,
    }))
}

struct PartialNodeLabelRecovery {
    trigger_span: Option<SourceSpan>,
    error: LexError,
}

fn build_partial_node_label_token_from_raw(
    input: &str,
    shape: &str,
    token_span: SourceSpan,
    content_span: SourceSpan,
    raw: &str,
    raw_span: SourceSpan,
    recovery: PartialNodeLabelRecovery,
) -> Tok {
    let (text, kind) = parse_label_text(raw);
    Tok::NodeLabel(NodeLabelToken {
        shape: shape.to_string(),
        text: labeled_text_with_spans(
            input,
            LabeledText {
                text,
                kind,
                span: None,
                selection: None,
                lexeme_components: Vec::new(),
            },
            token_span,
            raw_span,
        ),
        trigger_span: recovery.trigger_span,
        lexeme_components: label_lexeme_components(token_span, content_span, raw_span),
        recovery_error: Some(recovery.error),
    })
}

fn label_lexeme_components(
    token_span: SourceSpan,
    content_span: SourceSpan,
    value_span: SourceSpan,
) -> Vec<FlowchartLexemeComponent> {
    let mut components = Vec::with_capacity(3);
    if token_span.start < content_span.start {
        components.push(FlowchartLexemeComponent::new(
            EditorLexemeKind::Delimiter,
            SourceSpan::new(token_span.start, content_span.start),
        ));
    }
    if value_span.start < value_span.end {
        components.push(FlowchartLexemeComponent::new(
            EditorLexemeKind::String,
            value_span,
        ));
    }
    if content_span.end < token_span.end {
        components.push(FlowchartLexemeComponent::new(
            EditorLexemeKind::Delimiter,
            SourceSpan::new(content_span.end, token_span.end),
        ));
    }
    components
}

fn labeled_text_with_spans(
    input: &str,
    mut text: LabeledText,
    token_span: SourceSpan,
    content_span: SourceSpan,
) -> LabeledText {
    text.span = Some(token_span);
    text.selection = label_value_selection(input, content_span, &text.text).or(Some(content_span));
    if text.lexeme_components.is_empty() && content_span.start < content_span.end {
        text.lexeme_components.push(FlowchartLexemeComponent::new(
            EditorLexemeKind::String,
            content_span,
        ));
    }
    text
}

fn label_value_selection(input: &str, content_span: SourceSpan, value: &str) -> Option<SourceSpan> {
    if value.is_empty() {
        return None;
    }
    let slice = input.get(content_span.start..content_span.end)?;
    let relative_start = slice.find(value)?;
    Some(SourceSpan::new(
        content_span.start + relative_start,
        content_span.start + relative_start + value.len(),
    ))
}

fn trimmed_slice_with_span(input: &str, start: usize, end: usize) -> (&str, SourceSpan) {
    let slice = &input[start..end];
    let leading = slice
        .len()
        .saturating_sub(slice.trim_start_matches(is_ecmascript_trim_char).len());
    let text = &slice[leading..];
    let trimmed_len = text.trim_end_matches(is_ecmascript_trim_char).len();
    let start = start + leading;
    (
        &text[..trimmed_len],
        SourceSpan::new(start, start + trimmed_len),
    )
}
