use crate::sanitize::{sanitize_text, sanitize_text_or_array};
use crate::{Error, MermaidConfig, ParseMetadata, Result};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};

lalrpop_util::lalrpop_mod!(
    #[allow(clippy::filter_map_identity)]
    state_grammar,
    "/diagrams/state_grammar.rs"
);

#[derive(Debug, Clone)]
pub(crate) enum Tok {
    Newline,
    Sd,
    Id(String),
    StyledId((String, String)),
    EdgeState,
    Descr(String),
    Arrow,
    StructStart,
    StructStop,
    As,
    Note,
    LeftOf,
    RightOf,
    NoteText(String),
    StateDescr(String),
    CompositState(String),
    Fork(String),
    Join(String),
    Choice(String),
    Concurrent,
    HideEmptyDescription,
    ScaleWidth(usize),
    ClassDef,
    ClassDefId(String),
    ClassDefStyleOpts(String),
    Class,
    ClassEntityIds(String),
    StyleClass(String),
    Style,
    StyleIds(String),
    StyleDefStyleOpts(String),
    Direction(String),
    AccTitle(String),
    AccDescr(String),
    AccDescrMultiline(String),
    Click,
    Href,
    StringLit(String),
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub(crate) struct LexError {
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Default,
    Struct,
    State,
    StateId,
}

struct Lexer<'input> {
    input: &'input str,
    pos: usize,
    pending: VecDeque<(usize, Tok, usize)>,
    modes: Vec<Mode>,
    emitted_eof_newline: bool,
}

impl<'input> Lexer<'input> {
    fn new(input: &'input str) -> Self {
        Self {
            input,
            pos: 0,
            pending: VecDeque::new(),
            modes: vec![Mode::Default],
            emitted_eof_newline: false,
        }
    }

    fn normalize_note_block_text(raw: &'input str) -> String {
        // Mermaid's state diagram note blocks do not preserve leading indentation for each line.
        // The upstream SVG baselines reflect line-wise trimming.
        let lines: Vec<&str> = raw
            .lines()
            .map(|l| l.trim_end_matches('\r').trim())
            .collect();

        let mut start = 0usize;
        let mut end = lines.len();
        while start < end && lines[start].is_empty() {
            start += 1;
        }
        while end > start && lines[end - 1].is_empty() {
            end -= 1;
        }

        lines[start..end].join("\n")
    }

    fn mode(&self) -> Mode {
        *self.modes.last().unwrap_or(&Mode::Default)
    }

    fn push_mode(&mut self, m: Mode) {
        self.modes.push(m);
    }

    fn pop_mode(&mut self) {
        if self.modes.len() > 1 {
            self.modes.pop();
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn starts_with(&self, s: &str) -> bool {
        let hay = self.input.as_bytes();
        let pat = s.as_bytes();
        hay.get(self.pos..)
            .is_some_and(|tail| tail.starts_with(pat))
    }

    fn starts_with_ci(&self, s: &str) -> bool {
        let hay = self.input.as_bytes();
        let pat = s.as_bytes();
        hay.len() >= self.pos + pat.len()
            && hay[self.pos..self.pos + pat.len()].eq_ignore_ascii_case(pat)
    }

    fn starts_with_word_ci(&self, s: &str) -> bool {
        if !self.starts_with_ci(s) {
            return false;
        }
        let after = self.pos + s.len();
        if after >= self.input.len() {
            return true;
        }
        let b = self.input.as_bytes()[after];
        b.is_ascii_whitespace() || matches!(b, b'{' | b'}' | b'[' | b']' | b'"' | b':' | b';')
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == b'\r' {
                self.pos += 1;
                continue;
            }
            break;
        }
    }

    fn read_to_newline(&mut self) -> String {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if b == b'\n' {
                break;
            }
            self.pos += 1;
        }
        self.input[start..self.pos].to_string()
    }

    fn lex_newline(&mut self) -> Option<(usize, Tok, usize)> {
        if self.peek()? != b'\n' {
            return None;
        }
        let start = self.pos;
        while let Some(b'\n') = self.peek() {
            self.pos += 1;
        }
        if matches!(self.mode(), Mode::State | Mode::StateId) {
            self.pop_mode();
        }
        Some((start, Tok::Newline, self.pos))
    }

    fn skip_comment(&mut self) -> bool {
        if self.starts_with("%%") {
            let _ = self.read_to_newline();
            return true;
        }
        if self.peek() == Some(b'#') {
            let _ = self.read_to_newline();
            return true;
        }
        false
    }

    fn lex_string_lit(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        if self.peek()? != b'"' {
            return None;
        }
        let start = self.pos;
        self.pos += 1;
        let body_start = self.pos;
        while let Some(b) = self.peek() {
            if b == b'"' {
                break;
            }
            self.pos += 1;
        }
        if self.peek() != Some(b'"') {
            return Some(Err(LexError {
                message: "Unterminated string literal; missing '\"'".to_string(),
            }));
        }
        let body = self.input[body_start..self.pos].to_string();
        self.pos += 1;
        Some(Ok((start, Tok::StringLit(body), self.pos)))
    }

    fn lex_sd_header(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.starts_with_ci("stateDiagram-v2") {
            self.pos += "stateDiagram-v2".len();
            return Some((start, Tok::Sd, self.pos));
        }
        if self.starts_with_ci("stateDiagram") {
            self.pos += "stateDiagram".len();
            return Some((start, Tok::Sd, self.pos));
        }
        None
    }

    fn lex_direction(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if !self.starts_with_word_ci("direction") {
            return None;
        }
        self.pos += "direction".len();
        self.skip_ws();
        let dir_start = self.pos;
        while let Some(b) = self.peek() {
            if b.is_ascii_alphabetic() {
                self.pos += 1;
                continue;
            }
            break;
        }
        let dir = self.input[dir_start..self.pos].trim().to_string();
        if matches!(dir.as_str(), "TB" | "BT" | "RL" | "LR") {
            let _ = self.read_to_newline();
            return Some((start, Tok::Direction(dir), self.pos));
        }
        None
    }

    fn lex_accessibility(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if self.starts_with_word_ci("accTitle") {
            self.pos += "accTitle".len();
            self.skip_ws();
            if self.peek() != Some(b':') {
                return None;
            }
            self.pos += 1;
            self.skip_ws();
            let value = self.read_to_newline();
            return Some(Ok((
                start,
                Tok::AccTitle(value.trim().to_string()),
                self.pos,
            )));
        }

        if !self.starts_with_word_ci("accDescr") {
            return None;
        }
        self.pos += "accDescr".len();
        self.skip_ws();
        if self.peek() == Some(b':') {
            self.pos += 1;
            self.skip_ws();
            let value = self.read_to_newline();
            return Some(Ok((
                start,
                Tok::AccDescr(value.trim().to_string()),
                self.pos,
            )));
        }
        if self.peek() == Some(b'{') {
            self.pos += 1;
            let body_start = self.pos;
            let tail = &self.input.as_bytes()[self.pos..];
            let Some(end_rel) = tail.iter().position(|&b| b == b'}') else {
                return Some(Err(LexError {
                    message: "Unterminated accDescr block; missing '}'".to_string(),
                }));
            };
            let body = self.input[body_start..body_start + end_rel].to_string();
            self.pos = body_start + end_rel + 1;
            return Some(Ok((start, Tok::AccDescrMultiline(body), self.pos)));
        }
        None
    }

    fn lex_stmt_line(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;

        if self.starts_with_ci("hide empty description") {
            self.pos += "hide empty description".len();
            let _ = self.read_to_newline();
            return Some(Ok((start, Tok::HideEmptyDescription, self.pos)));
        }

        if self.starts_with_word_ci("scale") {
            self.pos += "scale".len();
            self.skip_ws();

            let width_start = self.pos;
            while let Some(b) = self.peek() {
                if b.is_ascii_digit() {
                    self.pos += 1;
                    continue;
                }
                break;
            }
            if self.pos == width_start {
                return Some(Err(LexError {
                    message: "Expected a width number after 'scale'".to_string(),
                }));
            }
            let width: usize = match self.input[width_start..self.pos].parse() {
                Ok(v) => v,
                Err(_) => {
                    return Some(Err(LexError {
                        message: "Invalid width number after 'scale'".to_string(),
                    }));
                }
            };

            self.skip_ws();
            if !self.starts_with_word_ci("width") {
                return Some(Err(LexError {
                    message: "Expected 'width' after `scale <n>`".to_string(),
                }));
            }
            self.pos += "width".len();
            let _ = self.read_to_newline();
            return Some(Ok((start, Tok::ScaleWidth(width), self.pos)));
        }

        if self.starts_with_word_ci("click") {
            self.pos += "click".len();
            return Some(Ok((start, Tok::Click, self.pos)));
        }
        if self.starts_with_word_ci("href") {
            self.pos += "href".len();
            return Some(Ok((start, Tok::Href, self.pos)));
        }

        if self.starts_with_word_ci("note") {
            let kw_end = self.pos + "note".len();
            self.pos = kw_end;
            self.skip_ws();

            // Floating note: note "text" as id
            if self.peek() == Some(b'"') {
                let Some(Ok((_s, Tok::StringLit(text), _e))) = self.lex_string_lit() else {
                    return Some(Err(LexError {
                        message: "Unterminated note string; missing '\"'".to_string(),
                    }));
                };
                self.skip_ws();
                if !self.starts_with_word_ci("as") {
                    return Some(Err(LexError {
                        message: "Expected 'as' in floating note statement".to_string(),
                    }));
                }
                let as_start = self.pos;
                self.pos += "as".len();
                self.skip_ws();
                let id_start = self.pos;
                let id = self.read_to_newline().trim().to_string();

                self.pending.push_back((start, Tok::Note, kw_end));
                self.pending
                    .push_back((kw_end, Tok::NoteText(text), as_start));
                self.pending.push_back((as_start, Tok::As, id_start));
                self.pending.push_back((id_start, Tok::Id(id), self.pos));
                return self.pending.pop_front().map(Ok);
            }

            // Positioned note: note left of|right of ID : text
            let pos_start = self.pos;
            let pos_tok = if self.starts_with_ci("left of") {
                self.pos += "left of".len();
                Tok::LeftOf
            } else if self.starts_with_ci("right of") {
                self.pos += "right of".len();
                Tok::RightOf
            } else {
                return Some(Err(LexError {
                    message: "Expected 'left of' or 'right of' after 'note'".to_string(),
                }));
            };

            self.skip_ws();
            let id_start = self.pos;
            while let Some(b) = self.peek() {
                if b == b':' || b == b'\n' || b.is_ascii_whitespace() || b == b'-' {
                    break;
                }
                self.pos += 1;
            }
            let id = self.input[id_start..self.pos].trim().to_string();

            self.skip_ws();
            let text_start = self.pos;
            let text = if self.peek() == Some(b':') {
                self.pos += 1;

                self.read_to_newline().trim().to_string()
            } else {
                let Some(rest) = self.input.get(self.pos..) else {
                    return Some(Err(LexError {
                        message: "Internal lexer error: invalid UTF-8 boundary".to_string(),
                    }));
                };
                let rest_lower = rest.to_ascii_lowercase();
                let Some(idx) = rest_lower.find("end note") else {
                    return Some(Err(LexError {
                        message: "Unterminated note block; missing 'end note'".to_string(),
                    }));
                };
                let t = Self::normalize_note_block_text(&rest[..idx]);
                self.pos += idx + "end note".len();
                t
            };

            self.pending.push_back((start, Tok::Note, kw_end));
            self.pending.push_back((pos_start, pos_tok, id_start));
            self.pending.push_back((id_start, Tok::Id(id), text_start));
            self.pending
                .push_back((text_start, Tok::NoteText(text), self.pos));
            return self.pending.pop_front().map(Ok);
        }

        if self.starts_with_word_ci("classDef") {
            let kw_end = self.pos + "classDef".len();
            self.pos = kw_end;
            self.skip_ws();
            let id_start = self.pos;
            while let Some(b) = self.peek() {
                if b.is_ascii_alphanumeric() || b == b'_' {
                    self.pos += 1;
                    continue;
                }
                break;
            }
            let id_end = self.pos;
            let id = self.input[id_start..id_end].trim().to_string();
            self.skip_ws();
            let raw = self.read_to_newline().trim().to_string();

            self.pending.push_back((start, Tok::ClassDef, kw_end));
            self.pending
                .push_back((id_start, Tok::ClassDefId(id), id_end));
            self.pending
                .push_back((id_end, Tok::ClassDefStyleOpts(raw), self.pos));
            return self.pending.pop_front().map(Ok);
        }

        if self.starts_with_word_ci("class") {
            let kw_end = self.pos + "class".len();
            self.pos = kw_end;
            self.skip_ws();
            let ids_start = self.pos;
            let mut ids_end = self.pos;
            loop {
                let word_start = self.pos;
                while let Some(b) = self.peek() {
                    if b.is_ascii_alphanumeric() || b == b'_' {
                        self.pos += 1;
                        continue;
                    }
                    break;
                }
                if self.pos == word_start {
                    break;
                }
                ids_end = self.pos;
                let after_word = self.pos;
                self.skip_ws();
                if self.peek() == Some(b',') {
                    self.pos += 1;
                    self.skip_ws();
                    continue;
                }
                self.pos = after_word;
                break;
            }
            let ids = self.input[ids_start..ids_end].trim().to_string();
            self.skip_ws();
            let style = self.read_to_newline().trim().to_string();

            self.pending.push_back((start, Tok::Class, kw_end));
            self.pending
                .push_back((ids_start, Tok::ClassEntityIds(ids), ids_end));
            self.pending
                .push_back((ids_end, Tok::StyleClass(style), self.pos));
            return self.pending.pop_front().map(Ok);
        }

        if self.starts_with_word_ci("style") {
            let kw_end = self.pos + "style".len();
            self.pos = kw_end;
            self.skip_ws();
            let ids_start = self.pos;
            while let Some(b) = self.peek() {
                if b.is_ascii_alphanumeric() || b == b'_' || b == b',' {
                    self.pos += 1;
                    continue;
                }
                break;
            }
            let ids_end = self.pos;
            let ids = self.input[ids_start..ids_end].trim().to_string();
            self.skip_ws();
            let raw = self.read_to_newline().trim().to_string();

            self.pending.push_back((start, Tok::Style, kw_end));
            self.pending
                .push_back((ids_start, Tok::StyleIds(ids), ids_end));
            self.pending
                .push_back((ids_end, Tok::StyleDefStyleOpts(raw), self.pos));
            return self.pending.pop_front().map(Ok);
        }

        if self.starts_with_word_ci("state") {
            self.pos += "state".len();
            self.push_mode(Mode::State);
            return None;
        }

        None
    }

    fn lex_state_mode_token(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        self.skip_ws();

        self.peek()?;

        if self.mode() == Mode::StateId {
            let body_start = self.pos;
            while let Some(b) = self.peek() {
                if b == b'\n' || b == b'{' {
                    break;
                }
                self.pos += 1;
            }
            let id = self.input[body_start..self.pos].trim().to_string();
            self.pop_mode(); // StateId
            self.pop_mode(); // State
            return Some(Ok((start, Tok::Id(id), self.pos)));
        }

        if self.peek() == Some(b'"') {
            self.pos += 1;
            let body_start = self.pos;
            while let Some(b) = self.peek() {
                if b == b'"' {
                    break;
                }
                self.pos += 1;
            }
            if self.peek() != Some(b'"') {
                return Some(Err(LexError {
                    message: "Unterminated state description string; missing '\"'".to_string(),
                }));
            }
            let body = self.input[body_start..self.pos].to_string();
            self.pos += 1;
            return Some(Ok((start, Tok::StateDescr(body), self.pos)));
        }

        if self.starts_with_word_ci("as") {
            self.pos += "as".len();
            self.push_mode(Mode::StateId);
            return Some(Ok((start, Tok::As, self.pos)));
        }

        // Fork/join/choice markers are recognized using the rest of the line.
        let Some(rel) = self.input.get(self.pos..) else {
            return Some(Err(LexError {
                message: "Internal lexer error: invalid UTF-8 boundary".to_string(),
            }));
        };
        let eol = rel.find('\n').unwrap_or(rel.len());
        let line = &rel[..eol];
        let trimmed = line.trim().to_string();
        let lower = trimmed.to_ascii_lowercase();
        for marker in ["<<fork>>", "[[fork]]"] {
            if lower.ends_with(marker) {
                let base = trimmed[..trimmed.len() - marker.len()].trim().to_string();
                self.pos += eol;
                self.pop_mode();
                return Some(Ok((start, Tok::Fork(base), self.pos)));
            }
        }
        for marker in ["<<join>>", "[[join]]"] {
            if lower.ends_with(marker) {
                let base = trimmed[..trimmed.len() - marker.len()].trim().to_string();
                self.pos += eol;
                self.pop_mode();
                return Some(Ok((start, Tok::Join(base), self.pos)));
            }
        }
        for marker in ["<<choice>>", "[[choice]]"] {
            if lower.ends_with(marker) {
                let base = trimmed[..trimmed.len() - marker.len()].trim().to_string();
                self.pos += eol;
                self.pop_mode();
                return Some(Ok((start, Tok::Choice(base), self.pos)));
            }
        }

        // Otherwise treat it as a composite state ID: read only the identifier token.
        let Some(id) = self.read_plain_id() else {
            return Some(Err(LexError {
                message: "Expected a state id".to_string(),
            }));
        };
        self.pop_mode();

        // Mermaid accepts `state <id>` with a `{ ... }` block that starts on the next line:
        //
        //   state Foo
        //   {
        //     ...
        //   }
        //
        // Treat the intervening whitespace/newlines as insignificant and advance to the `{` so the
        // parser sees `CompositState` followed immediately by a `Block`.
        let end = self.pos;
        let mut look = self.pos;
        while let Some(b) = self.input.as_bytes().get(look).copied() {
            if matches!(b, b' ' | b'\t' | b'\r') {
                look += 1;
                continue;
            }
            break;
        }
        if self.input.as_bytes().get(look) == Some(&b'\n') {
            let mut scan = look;
            while let Some(b) = self.input.as_bytes().get(scan).copied() {
                if matches!(b, b' ' | b'\t' | b'\r' | b'\n') {
                    scan += 1;
                    continue;
                }
                break;
            }
            if self.input.as_bytes().get(scan) == Some(&b'{') {
                self.pos = scan;
            }
        }

        Some(Ok((start, Tok::CompositState(id), end)))
    }

    fn lex_id(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let mut end = self.pos;
        while let Some(b) = self.input.as_bytes().get(end).copied() {
            if b == b':' || b == b'\n' || b.is_ascii_whitespace() || b == b'-' || b == b'{' {
                break;
            }
            end += 1;
        }
        if end == start {
            return None;
        }
        self.pos = end;
        Some((start, Tok::Id(self.input[start..end].to_string()), self.pos))
    }

    fn lex_descr(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.peek()? != b':' {
            return None;
        }
        self.pos += 1;
        let body_start = self.pos;
        while let Some(b) = self.peek() {
            if b == b'\n' || b == b';' || b == b':' {
                break;
            }
            self.pos += 1;
        }
        let body = self.input[body_start..self.pos].trim().to_string();
        Some((start, Tok::Descr(body), self.pos))
    }

    fn read_plain_id(&mut self) -> Option<String> {
        let start = self.pos;
        let mut end = self.pos;
        while let Some(b) = self.input.as_bytes().get(end).copied() {
            if b == b':' || b == b'\n' || b.is_ascii_whitespace() || b == b'-' || b == b'{' {
                break;
            }
            end += 1;
        }
        if end == start {
            return None;
        }
        self.pos = end;
        Some(self.input[start..end].to_string())
    }

    fn lex_styled_id(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;

        if self.starts_with("[*]:::") {
            self.pos += "[*]:::".len();
            let class_id = self.read_plain_id()?;
            return Some((
                start,
                Tok::StyledId(("[*]".to_string(), class_id)),
                self.pos,
            ));
        }

        // Look ahead: <id>:::<classId>
        let save = self.pos;
        let Some(id) = self.read_plain_id() else {
            self.pos = save;
            return None;
        };
        if !self.starts_with(":::") {
            self.pos = save;
            return None;
        }
        self.pos += 3;
        let Some(class_id) = self.read_plain_id() else {
            self.pos = save;
            return None;
        };
        Some((start, Tok::StyledId((id, class_id)), self.pos))
    }
}

impl Iterator for Lexer<'_> {
    type Item = std::result::Result<(usize, Tok, usize), LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.pending.pop_front() {
            return Some(Ok(item));
        }

        if self.pos >= self.input.len() {
            if self.emitted_eof_newline {
                return None;
            }
            self.emitted_eof_newline = true;
            return Some(Ok((self.pos, Tok::Newline, self.pos)));
        }
        self.skip_ws();
        if let Some(nl) = self.lex_newline() {
            return Some(Ok(nl));
        }
        if self.skip_comment() {
            return self.next();
        }

        if self.mode() == Mode::StateId {
            if let Some(tok) = self.lex_state_mode_token() {
                return Some(tok);
            }
        }

        if let Some(sd) = self.lex_sd_header() {
            return Some(Ok(sd));
        }

        if let Some(dir) = self.lex_direction() {
            return Some(Ok(dir));
        }

        if let Some(acc) = self.lex_accessibility() {
            return Some(acc);
        }

        if !matches!(self.mode(), Mode::State | Mode::StateId) {
            if let Some(tok) = self.lex_stmt_line() {
                return Some(tok);
            }
        }

        if self.mode() == Mode::State {
            if let Some(tok) = self.lex_state_mode_token() {
                return Some(tok);
            }
        }

        let start = self.pos;
        if self.starts_with("-->") {
            self.pos += 3;
            return Some(Ok((start, Tok::Arrow, self.pos)));
        }

        if self.mode() == Mode::Struct && self.starts_with("--") {
            self.pos += 2;
            return Some(Ok((start, Tok::Concurrent, self.pos)));
        }

        if self.starts_with("[*]") && !self.starts_with("[*]:::") {
            self.pos += 3;
            return Some(Ok((start, Tok::EdgeState, self.pos)));
        }

        if self.peek() == Some(b'{') {
            self.pos += 1;
            if self.mode() == Mode::State {
                self.pop_mode();
            }
            self.push_mode(Mode::Struct);
            return Some(Ok((start, Tok::StructStart, self.pos)));
        }

        if self.peek() == Some(b'}') {
            self.pos += 1;
            if self.mode() == Mode::Struct {
                self.pop_mode();
            }
            return Some(Ok((start, Tok::StructStop, self.pos)));
        }

        if let Some(tok) = self.lex_string_lit() {
            return Some(tok);
        }

        if let Some(tok) = self.lex_styled_id() {
            return Some(Ok(tok));
        }

        if let Some(tok) = self.lex_descr() {
            return Some(Ok(tok));
        }

        if let Some(tok) = self.lex_id() {
            return Some(Ok(tok));
        }

        let bad = self
            .input
            .get(self.pos..)
            .and_then(|s| s.chars().next())
            .unwrap_or('?');
        self.pos += bad.len_utf8().max(1);
        Some(Err(LexError {
            message: format!("Unexpected character '{bad}'"),
        }))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Note {
    pub position: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ClickStmt {
    pub id: String,
    pub url: String,
    pub tooltip: String,
}

#[derive(Debug, Clone)]
pub(crate) struct StateStmt {
    pub id: String,
    pub ty: String,
    pub description: Option<String>,
    pub descriptions: Vec<String>,
    pub doc: Option<Vec<Stmt>>,
    pub note: Option<Note>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
    pub start: Option<bool>,
}

impl StateStmt {
    pub(crate) fn new(id: String) -> Self {
        Self {
            id,
            ty: "default".to_string(),
            description: None,
            descriptions: Vec::new(),
            doc: None,
            note: None,
            classes: Vec::new(),
            styles: Vec::new(),
            text_styles: Vec::new(),
            start: None,
        }
    }

    pub(crate) fn new_typed(id: String, ty: &str) -> Self {
        Self {
            ty: ty.to_string(),
            ..Self::new(id)
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub(crate) enum Stmt {
    Noop,
    State(StateStmt),
    Relation {
        state1: StateStmt,
        state2: StateStmt,
        description: Option<String>,
    },
    ClassDef {
        id: String,
        classes: String,
    },
    ApplyClass {
        ids: String,
        class_name: String,
    },
    Style {
        ids: String,
        styles: String,
    },
    Direction(String),
    AccTitle(String),
    AccDescr(String),
    Click(ClickStmt),
}

pub fn parse_state(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut doc = state_grammar::RootParser::new()
        .parse(Lexer::new(code))
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("{e:?}"),
        })?;

    let mut divider_cnt = 0usize;
    assign_divider_ids(&mut doc, &mut divider_cnt);

    let mut db = StateDb::new();
    db.set_root_doc(doc);
    db.to_model(meta)
}

#[derive(Debug, Clone, Default)]
struct StyleClass {
    id: String,
    styles: Vec<String>,
    text_styles: Vec<String>,
}

#[derive(Debug, Clone)]
struct RelationEdge {
    id1: String,
    id2: String,
    relation_title: Option<String>,
}

#[derive(Debug, Clone)]
struct StateRecord {
    id: String,
    ty: String,
    descriptions: Vec<String>,
    doc: Option<Vec<Stmt>>,
    note: Option<Note>,
    classes: Vec<String>,
    styles: Vec<String>,
    text_styles: Vec<String>,
    start: Option<bool>,
}

impl StateRecord {
    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "type": self.ty,
            "descriptions": self.descriptions,
            "doc": self.doc.as_ref().map(|d| d.iter().map(stmt_to_json).collect::<Vec<_>>()),
            "note": self.note.as_ref().map(|n| json!({"position": n.position, "text": n.text})),
            "classes": self.classes,
            "styles": self.styles,
            "textStyles": self.text_styles,
            "start": self.start,
        })
    }
}

#[derive(Debug, Default)]
struct StateDb {
    root_doc: Vec<Stmt>,
    states: HashMap<String, StateRecord>,
    state_order: Vec<String>,
    relations: Vec<RelationEdge>,
    style_classes: IndexMap<String, StyleClass>,
    direction: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    generated_id_cnt: usize,
    links: HashMap<String, Link>,
}

impl StateDb {
    fn new() -> Self {
        Self::default()
    }

    fn generate_id(&mut self) -> String {
        self.generated_id_cnt += 1;
        let cnt = self.generated_id_cnt as u64;

        // Mermaid `@11.12.2` uses `Math.random().toString(36).substr(2, 12)` plus a monotonically
        // increasing counter. We keep the same `id-<base36>-<n>` shape, but generate the middle
        // segment deterministically to keep snapshots stable.
        let mut x = cnt ^ 0x9e37_79b9_7f4a_7c15u64;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9u64);
        x ^= x >> 32;
        x = x.wrapping_mul(0x94d0_49bb_1331_11ebu64);
        x ^= x >> 32;

        fn to_base36(mut v: u64) -> String {
            const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
            if v == 0 {
                return "0".to_string();
            }
            let mut buf = [0u8; 32];
            let mut i = buf.len();
            while v > 0 {
                let rem = (v % 36) as usize;
                v /= 36;
                i -= 1;
                buf[i] = DIGITS[rem];
            }
            String::from_utf8_lossy(&buf[i..]).to_string()
        }

        let mut mid = to_base36(x);
        if mid.len() < 12 {
            let pad = "0".repeat(12 - mid.len());
            mid = format!("{pad}{mid}");
        } else if mid.len() > 12 {
            mid = mid[mid.len() - 12..].to_string();
        }

        format!("id-{mid}-{}", self.generated_id_cnt)
    }

    fn set_root_doc(&mut self, mut doc: Vec<Stmt>) {
        self.translate_doc("root", &mut doc);
        self.root_doc = doc;
        self.extract();
    }

    fn translate_state_ref(&self, parent_id: &str, s: &mut StateStmt, first: bool) {
        if s.id.trim() == "[*]" {
            s.id = format!("{}_{}", parent_id, if first { "start" } else { "end" });
            s.start = Some(first);
        } else {
            s.id = s.id.trim().to_string();
        }
    }

    fn translate_state_concurrency_split(&mut self, doc: &mut Vec<Stmt>) {
        let old = std::mem::take(doc);
        let mut out: Vec<Stmt> = Vec::new();
        let mut current: Vec<Stmt> = Vec::new();
        let mut saw_divider = false;

        for stmt in old {
            match &stmt {
                Stmt::State(s) if s.ty == "divider" => {
                    saw_divider = true;
                    let mut divider = s.clone();
                    divider.doc = Some(std::mem::take(&mut current));
                    out.push(Stmt::State(divider));
                }
                _ => current.push(stmt),
            }
        }

        if saw_divider && !current.is_empty() {
            let mut divider = StateStmt::new_typed(self.generate_id(), "divider");
            divider.doc = Some(std::mem::take(&mut current));
            out.push(Stmt::State(divider));
        }

        if saw_divider {
            *doc = out;
        } else {
            *doc = current;
        }
    }

    fn translate_doc(&mut self, parent_id: &str, doc: &mut [Stmt]) {
        for stmt in doc.iter_mut() {
            match stmt {
                Stmt::Relation {
                    state1,
                    state2,
                    description: _,
                } => {
                    self.translate_state_ref(parent_id, state1, true);
                    self.translate_state_ref(parent_id, state2, false);
                }
                Stmt::State(s) => {
                    self.translate_state_ref(parent_id, s, true);
                    if let Some(inner) = s.doc.as_mut() {
                        self.translate_state_concurrency_split(inner);
                        self.translate_doc(&s.id, inner);
                    }
                }
                _ => {}
            }
        }
    }

    fn extract(&mut self) {
        self.states.clear();
        self.state_order.clear();
        self.relations.clear();
        self.style_classes.clear();
        self.direction = None;
        self.acc_title = None;
        self.acc_descr = None;
        self.generated_id_cnt = 0;
        self.links.clear();

        let stmts = self.root_doc.clone();
        for stmt in stmts {
            match stmt {
                Stmt::State(s) => {
                    self.add_state(
                        &s.id,
                        &s.ty,
                        s.doc.clone(),
                        s.description.as_deref(),
                        &s.descriptions,
                        s.note.clone(),
                        &s.classes,
                        &s.styles,
                        &s.text_styles,
                        s.start,
                    );
                }
                Stmt::Relation {
                    state1,
                    state2,
                    description,
                } => self.add_relation(&state1, &state2, description.as_deref()),
                Stmt::ClassDef { id, classes } => self.add_style_class(&id, &classes),
                Stmt::ApplyClass { ids, class_name } => self.set_css_class(&ids, &class_name),
                Stmt::Style { ids, styles } => self.handle_style_def(&ids, &styles),
                Stmt::Direction(dir) => self.direction = Some(dir),
                Stmt::AccTitle(t) => self.acc_title = Some(t),
                Stmt::AccDescr(d) => self.acc_descr = Some(normalize_multiline_ws(&d)),
                Stmt::Click(c) => self.add_link(&c.id, &c.url, &c.tooltip),
                Stmt::Noop => {}
            }
        }
    }

    fn add_link(&mut self, state_id: &str, url: &str, tooltip: &str) {
        self.links.insert(
            state_id.to_string(),
            Link {
                url: url.to_string(),
                tooltip: tooltip.to_string(),
            },
        );
    }

    fn ensure_state(&mut self, id: &str) -> &mut StateRecord {
        let id = id.trim();
        if !self.states.contains_key(id) {
            self.state_order.push(id.to_string());
            self.states.insert(
                id.to_string(),
                StateRecord {
                    id: id.to_string(),
                    ty: "default".to_string(),
                    descriptions: Vec::new(),
                    doc: None,
                    note: None,
                    classes: Vec::new(),
                    styles: Vec::new(),
                    text_styles: Vec::new(),
                    start: None,
                },
            );
        }
        self.states.get_mut(id).unwrap()
    }

    fn add_description(&mut self, id: &str, descr: &str) {
        let clean = descr.trim().trim_start_matches(':').trim().to_string();
        if clean.is_empty() {
            return;
        }
        self.ensure_state(id).descriptions.push(clean);
    }

    #[allow(clippy::too_many_arguments)]
    fn add_state(
        &mut self,
        id: &str,
        ty: &str,
        doc: Option<Vec<Stmt>>,
        description: Option<&str>,
        descriptions: &[String],
        note: Option<Note>,
        classes: &[String],
        styles: &[String],
        text_styles: &[String],
        start: Option<bool>,
    ) {
        let st = self.ensure_state(id);
        if st.doc.is_none() && doc.is_some() {
            st.doc = doc;
        }
        if st.ty == "default" && ty != "default" {
            st.ty = ty.to_string();
        }
        if st.start.is_none() {
            st.start = start;
        }
        if note.is_some() {
            st.note = note;
        }

        if let Some(d) = description {
            self.add_description(id, d);
        }
        for d in descriptions {
            self.add_description(id, d);
        }
        for c in classes {
            self.set_css_class(id, c);
        }
        for s in styles {
            self.set_style(id, s);
        }
        for ts in text_styles {
            self.set_text_style(id, ts);
        }
    }

    fn add_relation(&mut self, s1: &StateStmt, s2: &StateStmt, title: Option<&str>) {
        let id1 = s1.id.trim();
        let id2 = s2.id.trim();

        self.add_state(
            id1,
            &s1.ty,
            s1.doc.clone(),
            s1.description.as_deref(),
            &s1.descriptions,
            s1.note.clone(),
            &s1.classes,
            &s1.styles,
            &s1.text_styles,
            s1.start,
        );
        self.add_state(
            id2,
            &s2.ty,
            s2.doc.clone(),
            s2.description.as_deref(),
            &s2.descriptions,
            s2.note.clone(),
            &s2.classes,
            &s2.styles,
            &s2.text_styles,
            s2.start,
        );

        let relation_title = title
            .map(|t| t.trim().to_string())
            .filter(|s| !s.is_empty());

        // Mermaid `@11.12.2` self-loops are special-cased during rendering/layout (fixed
        // `*-cyclic-special-*` ids). Multiple self-loop statements on the same node effectively
        // overwrite; keep the latest label/title.
        if id1 == id2 {
            if let Some(existing) = self
                .relations
                .iter_mut()
                .find(|r| r.id1 == id1 && r.id2 == id2)
            {
                existing.relation_title = relation_title;
                return;
            }
        }

        self.relations.push(RelationEdge {
            id1: id1.to_string(),
            id2: id2.to_string(),
            relation_title,
        });
    }

    fn add_style_class(&mut self, id: &str, style_attributes: &str) {
        let entry = self
            .style_classes
            .entry(id.trim().to_string())
            .or_insert_with(|| StyleClass {
                id: id.trim().to_string(),
                styles: Vec::new(),
                text_styles: Vec::new(),
            });

        for attrib in style_attributes.split(',') {
            let fixed = attrib
                .split_once(';')
                .map(|(a, _)| a)
                .unwrap_or(attrib)
                .trim()
                .to_string();
            if fixed.is_empty() {
                continue;
            }
            if attrib.contains("color") {
                let t1 = fixed.replace("fill", "bgFill");
                let t2 = t1.replace("color", "fill");
                entry.text_styles.push(t2);
            }
            entry.styles.push(fixed);
        }
    }

    fn set_css_class(&mut self, item_ids: &str, css_class_name: &str) {
        for id in item_ids.split(',') {
            let trimmed = id.trim();
            if trimmed.is_empty() {
                continue;
            }
            self.ensure_state(trimmed)
                .classes
                .push(css_class_name.trim().to_string());
        }
    }

    fn set_style(&mut self, item_id: &str, style_text: &str) {
        self.ensure_state(item_id)
            .styles
            .push(style_text.trim().to_string());
    }

    fn set_text_style(&mut self, item_id: &str, style_text: &str) {
        self.ensure_state(item_id)
            .text_styles
            .push(style_text.trim().to_string());
    }

    fn handle_style_def(&mut self, ids: &str, styles: &str) {
        let styles_vec: Vec<String> = styles
            .split(',')
            .map(|s| s.replace(';', "").trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        for id in ids.split(',') {
            let trimmed = id.trim();
            if trimmed.is_empty() {
                continue;
            }
            self.ensure_state(trimmed).styles = styles_vec.clone();
        }
    }

    fn to_model(&self, meta: &ParseMetadata) -> Result<Value> {
        let states_json: serde_json::Map<String, Value> = self
            .state_order
            .iter()
            .filter_map(|id| self.states.get(id))
            .map(|s| (s.id.clone(), s.to_json()))
            .collect();

        let relations_json: Vec<Value> = self
            .relations
            .iter()
            .map(|r| {
                json!({
                    "id1": r.id1,
                    "id2": r.id2,
                    "relationTitle": r.relation_title,
                })
            })
            .collect();

        let style_classes_json: serde_json::Map<String, Value> = self
            .style_classes
            .iter()
            .map(|(k, sc)| {
                (
                    k.clone(),
                    json!({
                        "id": sc.id,
                        "styles": sc.styles,
                        "textStyles": sc.text_styles,
                    }),
                )
            })
            .collect();

        let look = meta
            .effective_config
            .as_value()
            .as_object()
            .and_then(|o| o.get("look"))
            .cloned()
            .unwrap_or(Value::Null);

        let (nodes_json, edges_json) = build_layout_data(
            &self.root_doc,
            &self.states,
            &self.style_classes,
            &meta.effective_config,
            &look,
        )
        .map_err(|message| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message,
        })?;

        let links_json: serde_json::Map<String, Value> = self
            .links
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    json!({
                        "url": v.url,
                        "tooltip": v.tooltip,
                    }),
                )
            })
            .collect();

        Ok(json!({
            "type": meta.diagram_type,
            // Mermaid's `StateDB.getData()` returns a layout-ready `{ nodes, edges, other, config, direction }`.
            // We keep additional keys (`states`, `relations`, `styleClasses`, `links`) to help downstream
            // integrations and parity debugging.
            "nodes": nodes_json,
            "edges": edges_json,
            "other": {},
            "config": meta.effective_config.as_value(),
            "direction": self.direction.clone().unwrap_or_else(|| "TB".to_string()),
            "accTitle": self.acc_title,
            "accDescr": self.acc_descr,
            "states": Value::Object(states_json),
            "relations": relations_json,
            "styleClasses": Value::Object(style_classes_json),
            "links": Value::Object(links_json),
        }))
    }
}

#[derive(Debug, Clone)]
struct Link {
    url: String,
    tooltip: String,
}

const DEFAULT_NESTED_DOC_DIR: &str = "TB";

const G_EDGE_STYLE: &str = "fill:none";
const G_EDGE_ARROWHEADSTYLE: &str = "fill: #333";
const G_EDGE_LABELPOS: &str = "c";
const G_EDGE_LABELTYPE: &str = "text";
const G_EDGE_THICKNESS: &str = "normal";

const DOMID_STATE: &str = "state";
const DOMID_TYPE_SPACER: &str = "----";

const NOTE: &str = "note";
const PARENT: &str = "parent";
const NOTE_ID: &str = "----note";
const PARENT_ID: &str = "----parent";

const SHAPE_STATE: &str = "rect";
const SHAPE_STATE_WITH_DESC: &str = "rectWithTitle";
const SHAPE_START: &str = "stateStart";
const SHAPE_END: &str = "stateEnd";
const SHAPE_DIVIDER: &str = "divider";
const SHAPE_GROUP: &str = "roundedWithTitle";
const SHAPE_NOTE: &str = "note";
const SHAPE_NOTEGROUP: &str = "noteGroup";

const CSS_EDGE: &str = "transition";
const CSS_EDGE_NOTE_EDGE: &str = "transition note-edge";
const CSS_DIAGRAM_STATE: &str = "statediagram-state";
const CSS_DIAGRAM_NOTE: &str = "statediagram-note";
const CSS_DIAGRAM_CLUSTER: &str = "statediagram-cluster";
const CSS_DIAGRAM_CLUSTER_ALT: &str = "statediagram-cluster-alt";

fn state_dom_id(item_id: &str, counter: usize, ty: Option<&str>) -> String {
    let type_str = ty
        .filter(|t| !t.is_empty())
        .map(|t| format!("{DOMID_TYPE_SPACER}{t}"))
        .unwrap_or_default();
    format!("{DOMID_STATE}-{item_id}{type_str}-{counter}")
}

#[derive(Debug, Clone)]
struct NodeScratch {
    id: String,
    shape: String,
    label: Value,
    css_classes: String,
    css_styles: Vec<String>,
    node_type: Option<String>,
    dir: Option<String>,
    is_group: bool,
    parent_id: Option<String>,
}

fn get_dir_for_doc(doc: &[Stmt], default_dir: &str) -> String {
    let mut dir = default_dir.to_string();
    for stmt in doc {
        if let Stmt::Direction(d) = stmt {
            dir = d.clone();
        }
    }
    dir
}

fn compiled_styles(css_classes: &str, classes: &IndexMap<String, StyleClass>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for class_name in css_classes.split_whitespace() {
        if let Some(c) = classes.get(class_name) {
            out.extend(c.styles.iter().cloned());
        }
    }
    out
}

fn upsert_node(nodes: &mut Vec<Value>, index: &mut HashMap<String, usize>, node: Value) {
    let Some(id) = node.get("id").and_then(|v| v.as_str()) else {
        return;
    };
    match index.entry(id.to_string()) {
        Entry::Occupied(o) => {
            if let Some(dst) = nodes.get_mut(*o.get()) {
                if let (Some(dst_obj), Some(src_obj)) = (dst.as_object_mut(), node.as_object()) {
                    for (k, v) in src_obj {
                        dst_obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        Entry::Vacant(v) => {
            v.insert(nodes.len());
            nodes.push(node);
        }
    }
}

fn build_layout_data(
    root_doc: &[Stmt],
    states: &HashMap<String, StateRecord>,
    classes: &IndexMap<String, StyleClass>,
    config: &MermaidConfig,
    look: &Value,
) -> std::result::Result<(Vec<Value>, Vec<Value>), String> {
    let mut nodes: Vec<Value> = Vec::new();
    let mut edges: Vec<Value> = Vec::new();
    let mut node_index: HashMap<String, usize> = HashMap::new();

    let mut node_db: HashMap<String, NodeScratch> = HashMap::new();
    let mut graph_item_count: usize = 0;

    #[allow(clippy::too_many_arguments)]
    fn setup_doc(
        parent: Option<&StateStmt>,
        doc: &[Stmt],
        states: &HashMap<String, StateRecord>,
        classes: &IndexMap<String, StyleClass>,
        config: &MermaidConfig,
        look: &Value,
        nodes: &mut Vec<Value>,
        node_index: &mut HashMap<String, usize>,
        edges: &mut Vec<Value>,
        node_db: &mut HashMap<String, NodeScratch>,
        graph_item_count: &mut usize,
        alt_flag: bool,
    ) -> std::result::Result<(), String> {
        for item in doc {
            match item {
                Stmt::State(s) => data_fetcher(
                    parent,
                    s,
                    states,
                    classes,
                    config,
                    look,
                    nodes,
                    node_index,
                    edges,
                    node_db,
                    graph_item_count,
                    alt_flag,
                )?,
                Stmt::Relation {
                    state1,
                    state2,
                    description,
                } => {
                    data_fetcher(
                        parent,
                        state1,
                        states,
                        classes,
                        config,
                        look,
                        nodes,
                        node_index,
                        edges,
                        node_db,
                        graph_item_count,
                        alt_flag,
                    )?;
                    data_fetcher(
                        parent,
                        state2,
                        states,
                        classes,
                        config,
                        look,
                        nodes,
                        node_index,
                        edges,
                        node_db,
                        graph_item_count,
                        alt_flag,
                    )?;

                    let edge_label_raw = description.clone().unwrap_or_default();
                    let edge_label = sanitize_text(&edge_label_raw, config);
                    edges.push(json!({
                        "id": format!("edge{graph_item_count}"),
                        "start": state1.id,
                        "end": state2.id,
                        "arrowhead": "normal",
                        "arrowTypeEnd": "arrow_barb",
                        "style": G_EDGE_STYLE,
                        "labelStyle": "",
                        "label": edge_label,
                        "arrowheadStyle": G_EDGE_ARROWHEADSTYLE,
                        "labelpos": G_EDGE_LABELPOS,
                        "labelType": G_EDGE_LABELTYPE,
                        "thickness": G_EDGE_THICKNESS,
                        "classes": CSS_EDGE,
                        "look": look,
                    }));
                    *graph_item_count += 1;
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn data_fetcher(
        parent: Option<&StateStmt>,
        parsed_item: &StateStmt,
        states: &HashMap<String, StateRecord>,
        classes: &IndexMap<String, StyleClass>,
        config: &MermaidConfig,
        look: &Value,
        nodes: &mut Vec<Value>,
        node_index: &mut HashMap<String, usize>,
        edges: &mut Vec<Value>,
        node_db: &mut HashMap<String, NodeScratch>,
        graph_item_count: &mut usize,
        alt_flag: bool,
    ) -> std::result::Result<(), String> {
        let item_id = parsed_item.id.clone();
        if item_id == "root" || item_id.is_empty() {
            return Ok(());
        }

        let db_state = states.get(&item_id);
        let class_str = db_state.map(|s| s.classes.join(" ")).unwrap_or_default();
        let styles = db_state.map(|s| s.styles.clone()).unwrap_or_default();

        let entry = node_db.entry(item_id.clone()).or_insert_with(|| {
            let mut css_classes = String::new();
            if !class_str.trim().is_empty() {
                css_classes.push_str(class_str.trim());
                css_classes.push(' ');
            }
            css_classes.push_str(CSS_DIAGRAM_STATE);

            let mut shape = SHAPE_STATE.to_string();
            if parsed_item.start == Some(true) {
                shape = SHAPE_START.to_string();
            } else if parsed_item.start == Some(false) {
                shape = SHAPE_END.to_string();
            }
            if parsed_item.ty != "default" {
                shape = parsed_item.ty.clone();
            }

            NodeScratch {
                id: item_id.clone(),
                shape,
                label: json!(sanitize_text(&item_id, config)),
                css_classes,
                css_styles: styles.clone(),
                node_type: None,
                dir: None,
                is_group: false,
                parent_id: None,
            }
        });

        // Apply description statements like Mermaid's `dataFetcher.ts`.
        //
        // Note: Mermaid supports a compact form that combines a quoted label and a trailing `: ...`
        // description on the same line:
        //
        //   state "Some long name" as S1: The description
        //
        // The lexer captures `S1: The description` as a single `Id` token, which the grammar splits
        // into `id="S1"` + `descriptions=["The description"]`. Apply both the primary `description`
        // and any extra `descriptions` in order so the resulting `label` array becomes:
        //   ["Some long name", "The description"]
        // which later converts to (label, description[]) via `StateDB.extract()`.
        let mut descrs: Vec<&str> = Vec::new();
        if let Some(d) = parsed_item.description.as_deref() {
            if !d.trim().is_empty() {
                descrs.push(d);
            }
        }
        for d in &parsed_item.descriptions {
            if !d.trim().is_empty() {
                descrs.push(d);
            }
        }

        if !descrs.is_empty() {
            let base_label = sanitize_text(&item_id, config);
            for descr in descrs {
                match &mut entry.label {
                    Value::Array(arr) => {
                        entry.shape = SHAPE_STATE_WITH_DESC.to_string();
                        arr.push(Value::String(descr.to_string()));
                    }
                    Value::String(s) => {
                        if !s.is_empty() {
                            entry.shape = SHAPE_STATE_WITH_DESC.to_string();
                            if *s == base_label {
                                entry.label = Value::Array(vec![Value::String(descr.to_string())]);
                            } else {
                                entry.label = Value::Array(vec![
                                    Value::String(s.clone()),
                                    Value::String(descr.to_string()),
                                ]);
                            }
                        } else {
                            entry.shape = SHAPE_STATE.to_string();
                            entry.label = Value::String(descr.to_string());
                        }
                    }
                    _ => {
                        entry.shape = SHAPE_STATE.to_string();
                        entry.label = Value::String(descr.to_string());
                    }
                }
            }

            entry.label = sanitize_text_or_array(&entry.label, config);

            // If there's only 1 description entry, just use a regular state shape.
            if entry.shape == SHAPE_STATE_WITH_DESC {
                if let Some(arr) = entry.label.as_array() {
                    if arr.len() == 1 {
                        entry.shape = if entry.node_type.as_deref() == Some("group") {
                            SHAPE_GROUP.to_string()
                        } else {
                            SHAPE_STATE.to_string()
                        };
                    }
                }
            }
        }

        // Group handling (composite states)
        if entry.node_type.is_none() {
            if let Some(doc) = parsed_item.doc.as_ref() {
                entry.node_type = Some("group".to_string());
                entry.is_group = true;
                let dir = get_dir_for_doc(doc, DEFAULT_NESTED_DOC_DIR);
                entry.dir = Some(dir);
                entry.shape = if parsed_item.ty == "divider" {
                    SHAPE_DIVIDER.to_string()
                } else {
                    SHAPE_GROUP.to_string()
                };

                let mut css = entry.css_classes.clone();
                css.push(' ');
                css.push_str(CSS_DIAGRAM_CLUSTER);
                if alt_flag {
                    css.push(' ');
                    css.push_str(CSS_DIAGRAM_CLUSTER_ALT);
                }
                entry.css_classes = css;
            }
        }

        if let Some(p) = parent {
            if p.id != "root" {
                entry.parent_id = Some(p.id.clone());
            }
        }

        let mut node_data = json!({
            "labelStyle": "",
            "shape": entry.shape,
            "label": entry.label,
            "cssClasses": entry.css_classes,
            "cssCompiledStyles": [],
            "cssStyles": entry.css_styles,
            "id": entry.id,
            "dir": entry.dir,
            "domId": state_dom_id(&item_id, *graph_item_count, None),
            "type": entry.node_type,
            "isGroup": entry.is_group,
            "padding": 8,
            "rx": 10,
            "ry": 10,
            "look": look,
            "parentId": entry.parent_id,
            "centerLabel": true,
        });

        if node_data["shape"].as_str() == Some(SHAPE_DIVIDER) {
            node_data["label"] = json!("");
        }

        // Notes create a note node + note group + note edge
        if let Some(mut n) = parsed_item.note.clone() {
            let flowchart_padding = config
                .as_value()
                .as_object()
                .and_then(|o| o.get("flowchart"))
                .and_then(|v| v.as_object())
                .and_then(|o| o.get("padding"))
                .cloned()
                .unwrap_or(Value::Null);

            n.text = sanitize_text(&n.text, config);

            let note_id = format!("{item_id}{NOTE_ID}-{graph_item_count}");
            let parent_node_id = format!("{item_id}{PARENT_ID}");
            let note_dom_id = state_dom_id(&item_id, *graph_item_count, Some(NOTE));
            let group_dom_id = state_dom_id(&item_id, *graph_item_count, Some(PARENT));

            let group_data = json!({
                "labelStyle": "",
                "shape": SHAPE_NOTEGROUP,
                "label": n.text,
                "cssClasses": entry.css_classes,
                "cssStyles": [],
                "cssCompiledStyles": [],
                "id": parent_node_id,
                "domId": group_dom_id,
                "type": "group",
                "isGroup": true,
                "padding": 16,
                "look": look,
                "position": n.position,
            });

            let note_data = json!({
                "labelStyle": "",
                "shape": SHAPE_NOTE,
                "label": n.text,
                "cssClasses": CSS_DIAGRAM_NOTE,
                "cssStyles": [],
                "cssCompiledStyles": [],
                "id": note_id,
                "domId": note_dom_id,
                "type": entry.node_type,
                "isGroup": entry.is_group,
                "padding": flowchart_padding,
                "look": look,
                "position": n.position,
                "parentId": format!("{item_id}{PARENT_ID}"),
            });

            upsert_node(nodes, node_index, group_data);
            upsert_node(nodes, node_index, note_data.clone());
            upsert_node(nodes, node_index, node_data.clone());

            // style compilation after insertion
            for id in [parent_node_id.as_str(), note_id.as_str(), item_id.as_str()] {
                if let Some(idx) = node_index.get(id).copied() {
                    let css = nodes[idx]["cssClasses"].as_str().unwrap_or("");
                    let compiled = compiled_styles(css, classes);
                    if let Some(obj) = nodes[idx].as_object_mut() {
                        obj.insert("cssCompiledStyles".to_string(), json!(compiled));
                    }
                }
            }

            let (mut from, mut to) = (item_id.clone(), note_id);
            if n.position.as_deref() == Some("left of") {
                std::mem::swap(&mut from, &mut to);
            }

            edges.push(json!({
                "id": format!("{from}-{to}"),
                "start": from,
                "end": to,
                "arrowhead": "none",
                "arrowTypeEnd": "",
                "style": G_EDGE_STYLE,
                "labelStyle": "",
                "classes": CSS_EDGE_NOTE_EDGE,
                "arrowheadStyle": G_EDGE_ARROWHEADSTYLE,
                "labelpos": G_EDGE_LABELPOS,
                "labelType": G_EDGE_LABELTYPE,
                "thickness": G_EDGE_THICKNESS,
                "look": look,
            }));
            *graph_item_count += 1;
        } else {
            let css = node_data["cssClasses"].as_str().unwrap_or("");
            let compiled = compiled_styles(css, classes);
            if let Some(obj) = node_data.as_object_mut() {
                obj.insert("cssCompiledStyles".to_string(), json!(compiled));
            }
            upsert_node(nodes, node_index, node_data);
        }

        if let Some(doc) = parsed_item.doc.as_ref() {
            setup_doc(
                Some(parsed_item),
                doc,
                states,
                classes,
                config,
                look,
                nodes,
                node_index,
                edges,
                node_db,
                graph_item_count,
                !alt_flag,
            )?;
        }

        Ok(())
    }

    setup_doc(
        None,
        root_doc,
        states,
        classes,
        config,
        look,
        &mut nodes,
        &mut node_index,
        &mut edges,
        &mut node_db,
        &mut graph_item_count,
        false,
    )?;

    // Post-process label arrays into (label, description) like Mermaid's StateDB.extract().
    for node in nodes.iter_mut() {
        let Some(obj) = node.as_object_mut() else {
            continue;
        };
        let Some(label_val) = obj.get("label").cloned() else {
            continue;
        };
        let Some(arr) = label_val.as_array() else {
            continue;
        };
        if arr.is_empty() {
            continue;
        }
        let label0 = arr[0].clone();
        let rest: Vec<Value> = arr.iter().skip(1).cloned().collect();

        if obj.get("isGroup").and_then(|v| v.as_bool()) == Some(true) && !rest.is_empty() {
            return Err("Group nodes can only have label".to_string());
        }
        obj.insert("label".to_string(), label0);
        obj.insert("description".to_string(), Value::Array(rest));
    }

    Ok((nodes, edges))
}

fn stmt_to_json(stmt: &Stmt) -> Value {
    match stmt {
        Stmt::Noop => json!(null),
        Stmt::State(s) => json!({
            "stmt": "state",
            "id": s.id,
            "type": s.ty,
            "description": s.description,
            "doc": s.doc.as_ref().map(|d| d.iter().map(stmt_to_json).collect::<Vec<_>>()),
            "classes": s.classes,
        }),
        Stmt::Relation {
            state1,
            state2,
            description,
        } => json!({
            "stmt": "relation",
            "state1": { "id": state1.id, "type": state1.ty, "classes": state1.classes },
            "state2": { "id": state2.id, "type": state2.ty, "classes": state2.classes },
            "description": description,
        }),
        Stmt::ClassDef { id, classes } => {
            json!({ "stmt": "classDef", "id": id, "classes": classes })
        }
        Stmt::ApplyClass { ids, class_name } => {
            json!({ "stmt": "applyClass", "id": ids, "styleClass": class_name })
        }
        Stmt::Style { ids, styles } => json!({ "stmt": "style", "id": ids, "styleClass": styles }),
        Stmt::Direction(v) => json!({ "stmt": "dir", "value": v }),
        Stmt::AccTitle(t) => json!(t),
        Stmt::AccDescr(d) => json!(d),
        Stmt::Click(c) => {
            json!({ "stmt": "click", "id": c.id, "url": c.url, "tooltip": c.tooltip })
        }
    }
}

fn normalize_multiline_ws(input: &str) -> String {
    let trimmed = input.trim();
    let mut out = String::with_capacity(trimmed.len());
    let mut chars = trimmed.chars().peekable();
    while let Some(ch) = chars.next() {
        out.push(ch);
        if ch == '\n' {
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
        }
    }
    out
}

fn assign_divider_ids(stmts: &mut [Stmt], cnt: &mut usize) {
    for s in stmts.iter_mut() {
        match s {
            Stmt::State(st) => {
                if st.ty == "divider" && st.id == "__divider__" {
                    *cnt += 1;
                    st.id = format!("divider-id-{cnt}");
                }
                if let Some(doc) = st.doc.as_mut() {
                    assign_divider_ids(doc, cnt);
                }
            }
            Stmt::Relation { state1, state2, .. } => {
                if state1.ty == "divider" && state1.id == "__divider__" {
                    *cnt += 1;
                    state1.id = format!("divider-id-{cnt}");
                }
                if state2.ty == "divider" && state2.id == "__divider__" {
                    *cnt += 1;
                    state2.id = format!("divider-id-{cnt}");
                }
            }
            _ => {}
        }
    }
}
