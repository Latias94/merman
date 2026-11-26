//! Stateful ER lexer with source-relative token spans.

use crate::scan::{ScanStep, TokenScanner, consume_line_ending};
use merman_model::text::is_ecmascript_whitespace;
use merman_model::{OperationControl, SourceSpan};

/// One spanned token or recoverable lexical error.
pub type ErLexicalEvent = Result<(usize, ErToken, usize), ErLexicalError>;

#[derive(Debug, Clone)]
struct SpannedId {
    name: String,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct ErIdList {
    ids: Vec<SpannedId>,
}

impl ErIdList {
    pub(super) fn into_names(self) -> Vec<String> {
        self.ids.into_iter().map(|id| id.name).collect()
    }

    /// Byte span of all identifiers in the list.
    #[cfg(feature = "editor-facts")]
    pub fn span(&self) -> SourceSpan {
        let start = self.ids.first().map(|id| id.span.start).unwrap_or(0);
        let end = self.ids.last().map(|id| id.span.end).unwrap_or(start);
        SourceSpan::new(start, end)
    }

    /// Identifier spelling and its byte span, in source order.
    #[cfg(feature = "editor-facts")]
    pub fn into_spanned_ids(self) -> impl Iterator<Item = (String, SourceSpan)> {
        self.ids.into_iter().map(|id| (id.name, id.span))
    }
}

#[derive(Debug, Clone)]
pub struct ErDirection {
    pub(super) value: String,
    #[cfg(any(test, feature = "editor-facts"))]
    selection: SourceSpan,
}

#[cfg(feature = "editor-facts")]
impl ErDirection {
    /// Byte span of the authored direction value.
    pub const fn selection(&self) -> SourceSpan {
        self.selection
    }
}

#[derive(Debug, Clone)]
pub enum ErToken {
    ErDiagram,
    Newline,

    Name(String),
    Str(String),
    IdList(ErIdList),
    RestOfLine(String),

    AccTitle(String),
    AccDescr(String),
    AccDescrMultiline(String),

    BlockStart,
    BlockStop,
    SquareStart,
    SquareStop,
    StyleSeparator,
    Colon,
    Comma,
    Question,

    StyleKw,
    ClassDefKw,
    ClassKw,
    SubgraphKw,
    EndKw,
    Direction(ErDirection),

    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
    OnlyOne,
    MdParent,
    Identifying,
    NonIdentifying,

    AttrWord(String),
    AttrKey(String),
    Comment(String),
}

#[derive(Debug, Clone)]
pub struct ErLexicalError {
    pub message: String,
    pub span: SourceSpan,
}

impl ErLexicalError {
    fn new(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Default,
    Block,
    NeedIdListOnly,
    NeedIdListThenLineRest,
    NeedClassFirstIdList,
    NeedClassSecondIdList,
    LineRest,
}

pub(super) struct Lexer<'input> {
    input: &'input str,
    control: Option<&'input OperationControl>,
    stopped: bool,
    pos: usize,
    mode: Mode,
    direction_scan_end: usize,
    generic_scan_end: usize,
    #[cfg(test)]
    generic_lookahead_bytes: usize,
}

impl<'input> Lexer<'input> {
    pub(super) fn new(input: &'input str, control: Option<&'input OperationControl>) -> Self {
        Self {
            input,
            control,
            stopped: false,
            pos: 0,
            mode: Mode::Default,
            direction_scan_end: 0,
            generic_scan_end: 0,
            #[cfg(test)]
            generic_lookahead_bytes: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn skip_ws_default(&mut self) {
        if matches!(self.mode, Mode::NeedIdListThenLineRest | Mode::LineRest)
            && consume_line_ending(self.input, self.pos).is_none()
        {
            self.skip_style_whitespace();
            return;
        }
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' {
                self.pos += 1;
                continue;
            }
            break;
        }
    }

    fn skip_ws_block(&mut self) {
        while let Some(ch) = self.input[self.pos..].chars().next() {
            if !is_ecmascript_whitespace(ch) {
                break;
            }
            self.pos += ch.len_utf8();
        }
    }

    fn scan_checkpoint(&mut self, steps: usize) -> bool {
        if steps.is_multiple_of(128)
            && self
                .control
                .is_some_and(|control| control.checkpoint().is_err())
        {
            self.stopped = true;
            return false;
        }
        true
    }

    fn skip_style_whitespace(&mut self) {
        let mut steps = 0usize;
        while let Some(ch) = self.input[self.pos..].chars().next() {
            if !is_ecmascript_whitespace(ch) || !self.scan_checkpoint(steps) {
                break;
            }
            self.pos += ch.len_utf8();
            steps = steps.wrapping_add(1);
        }
    }

    fn starts_with_ci(&self, s: &str) -> bool {
        self.input[self.pos..]
            .get(..s.len())
            .is_some_and(|h| h.eq_ignore_ascii_case(s))
    }

    fn starts_with_word_ci(&self, s: &str) -> bool {
        self.starts_with_ci(s) && self.ends_word_at(s.len())
    }

    fn read_to_newline(&mut self) -> String {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if matches!(b, b'\r' | b'\n') {
                break;
            }
            self.pos += 1;
        }
        self.input[start..self.pos].to_string()
    }

    fn lex_comment(&mut self) -> bool {
        if self.input[self.pos..].starts_with("%%") {
            let _ = self.read_to_newline();
            return true;
        }
        false
    }

    fn lex_newline(&mut self) -> Option<(usize, ErToken, usize)> {
        if self.mode == Mode::Block {
            return None;
        }
        let start = self.pos;
        self.pos = consume_line_ending(self.input, self.pos)?;
        while let Some(end) = consume_line_ending(self.input, self.pos) {
            self.pos = end;
        }
        if matches!(self.mode, Mode::NeedIdListThenLineRest | Mode::LineRest) {
            self.mode = Mode::Default;
        }
        Some((start, ErToken::Newline, self.pos))
    }

    fn lex_acc_title(
        &mut self,
    ) -> Option<std::result::Result<(usize, ErToken, usize), ErLexicalError>> {
        let start = self.pos;
        if !self.starts_with_ci("accTitle") {
            return None;
        }
        let after = self.pos + "accTitle".len();
        let rest = &self.input[after..];
        let rest_trim = rest.trim_start_matches(is_ecmascript_whitespace);
        if !rest_trim.starts_with(':') {
            return None;
        }
        let consumed_ws = rest.len() - rest_trim.len();
        let body =
            self.input[after + consumed_ws + 1..].trim_start_matches(is_ecmascript_whitespace);
        self.pos = self.input.len() - body.len();
        let s = self.read_to_newline();
        Some(Ok((
            start,
            ErToken::AccTitle(s.trim_matches(is_ecmascript_whitespace).to_string()),
            self.pos,
        )))
    }

    fn lex_acc_descr(&mut self) -> Option<ScanStep<ErLexicalEvent>> {
        let start = self.pos;
        if !self.starts_with_ci("accDescr") {
            return None;
        }
        let after = self.pos + "accDescr".len();
        let rest = &self.input[after..];
        let rest_trim = rest.trim_start_matches(is_ecmascript_whitespace);
        if let Some(body) = rest_trim.strip_prefix('{') {
            let body = body.trim_start_matches(is_ecmascript_whitespace);
            self.pos = self.input.len() - body.len();
            let Some(end_rel) = self.input[self.pos..].find('}') else {
                self.pos = self.input.len();
                return Some(ScanStep::Emit(Err(ErLexicalError::new(
                    "Unterminated accDescr block; missing '}'",
                    SourceSpan::new(start, self.input.len()),
                ))));
            };
            let body = self.input[self.pos..self.pos + end_rel].to_string();
            self.pos = self.pos + end_rel + 1;
            if body.is_empty() {
                return Some(ScanStep::Skip);
            }
            return Some(ScanStep::Emit(Ok((
                start,
                ErToken::AccDescrMultiline(body.trim_matches(is_ecmascript_whitespace).to_string()),
                self.pos,
            ))));
        }
        let body = rest_trim
            .strip_prefix(':')?
            .trim_start_matches(is_ecmascript_whitespace);
        self.pos = self.input.len() - body.len();
        let s = self.read_to_newline();
        Some(ScanStep::Emit(Ok((
            start,
            ErToken::AccDescr(s.trim_matches(is_ecmascript_whitespace).to_string()),
            self.pos,
        ))))
    }

    fn lex_direction(&mut self) -> Option<(usize, ErToken, usize)> {
        let start = self.pos;
        if start < self.direction_scan_end {
            return None;
        }
        let rest = &self.input[start..];
        // Mermaid tries `.*direction\s+TB[^\n]*` (then BT, RL, LR) before names.
        // Match the whole rule before consuming anything: `direction` alone is an entity.
        let prefix_end = rest
            .find(['\n', '\r', '\u{2028}', '\u{2029}'])
            .unwrap_or(rest.len());
        for direction in ["TB", "BT", "RL", "LR"] {
            let value_end = rest[..prefix_end]
                .char_indices()
                .rev()
                .find_map(|(offset, _)| {
                    let candidate = &rest[offset..];
                    if !candidate
                        .get(.."direction".len())?
                        .eq_ignore_ascii_case("direction")
                    {
                        return None;
                    }
                    let suffix = &candidate["direction".len()..];
                    let value = suffix.trim_start_matches(is_ecmascript_whitespace);
                    if value.len() == suffix.len()
                        || !value.get(..2)?.eq_ignore_ascii_case(direction)
                    {
                        return None;
                    }
                    Some(rest.len() - value.len() + 2)
                });
            if let Some(value_end) = value_end {
                self.pos = start + value_end;
                // Unlike the leading `.`, the trailing `[^\n]*` includes carriage returns.
                let tail = &self.input[self.pos..];
                self.pos += tail.find('\n').unwrap_or(tail.len());
                return Some((
                    start,
                    ErToken::Direction(ErDirection {
                        value: direction.to_string(),
                        #[cfg(any(test, feature = "editor-facts"))]
                        selection: SourceSpan::new(start + value_end - 2, start + value_end),
                    }),
                    self.pos,
                ));
            }
        }
        // A failed whole-line lookup also proves every remaining token prefix cannot match.
        // Remember it so a line of entity names does not require quadratic rescanning.
        self.direction_scan_end = start + prefix_end;
        None
    }

    fn lex_keyword(&mut self) -> Option<(usize, ErToken, usize)> {
        let start = self.pos;
        if self.starts_with_word_ci("erDiagram") {
            self.pos += "erDiagram".len();
            return Some((start, ErToken::ErDiagram, self.pos));
        }
        if self.starts_with_word_ci("style") {
            self.pos += "style".len();
            self.mode = Mode::NeedIdListThenLineRest;
            return Some((start, ErToken::StyleKw, self.pos));
        }
        if self.starts_with_word_ci("classDef") {
            self.pos += "classDef".len();
            self.mode = Mode::NeedIdListThenLineRest;
            return Some((start, ErToken::ClassDefKw, self.pos));
        }
        if self.starts_with_word_ci("class") {
            self.pos += "class".len();
            self.mode = Mode::NeedClassFirstIdList;
            return Some((start, ErToken::ClassKw, self.pos));
        }
        if self.starts_with_word_ci("subgraph") {
            self.pos += "subgraph".len();
            return Some((start, ErToken::SubgraphKw, self.pos));
        }
        if self.starts_with_word_ci("end") {
            self.pos += "end".len();
            return Some((start, ErToken::EndKw, self.pos));
        }
        None
    }

    fn lex_id_list(&mut self) -> Option<(usize, ErToken, usize)> {
        if !matches!(
            self.mode,
            Mode::NeedIdListOnly
                | Mode::NeedIdListThenLineRest
                | Mode::NeedClassFirstIdList
                | Mode::NeedClassSecondIdList
        ) {
            return None;
        }
        let start = self.pos;
        self.skip_ws_default();
        let mut ids: Vec<SpannedId> = Vec::new();
        loop {
            let id_start = self.pos;
            let mut id_end = self.pos;
            for (rel, ch) in self.input[self.pos..].char_indices() {
                let ok = !ch.is_ascii()
                    || ch.is_ascii_alphanumeric()
                    || matches!(ch, '_' | '-' | '*' | '.');
                if !ok {
                    break;
                }
                id_end = self.pos + rel + ch.len_utf8();
            }
            if id_end == id_start {
                break;
            }
            ids.push(SpannedId {
                name: self.input[id_start..id_end].to_string(),
                #[cfg(feature = "editor-facts")]
                span: SourceSpan::new(id_start, id_end),
            });
            self.pos = id_end;

            self.skip_ws_default();
            if self.peek() != Some(b',') {
                break;
            }
            self.pos += 1;
            self.skip_ws_default();
        }
        if ids.is_empty() {
            return None;
        }
        self.mode = match self.mode {
            Mode::NeedIdListOnly => Mode::Default,
            Mode::NeedIdListThenLineRest => Mode::LineRest,
            Mode::NeedClassFirstIdList => Mode::NeedClassSecondIdList,
            Mode::NeedClassSecondIdList => Mode::Default,
            _ => Mode::Default,
        };
        Some((start, ErToken::IdList(ErIdList { ids }), self.pos))
    }

    fn lex_rest_of_line(
        &mut self,
    ) -> Option<std::result::Result<(usize, ErToken, usize), ErLexicalError>> {
        if self.mode != Mode::LineRest {
            return None;
        }
        let start = self.pos;
        let mut styles = String::new();
        let mut separator = None;
        let mut has_component = false;
        let mut payload_end = start;
        let mut failure = None;
        let mut steps = 0usize;
        while self.pos < self.input.len() {
            if !self.scan_checkpoint(steps) {
                return None;
            }
            steps = steps.wrapping_add(1);
            if consume_line_ending(self.input, self.pos).is_some() {
                break;
            }
            let rest = &self.input[self.pos..];
            let ch = rest.chars().next()?;
            if is_ecmascript_whitespace(ch) {
                self.skip_style_whitespace();
                if self.stopped {
                    return None;
                }
                continue;
            }
            if separator.is_some() {
                failure = Some(SourceSpan::new(self.pos, self.pos + ch.len_utf8()));
                break;
            }
            if ch == ';' {
                separator = Some(self.pos);
                self.pos += 1;
                continue;
            }
            if ch == ',' {
                if !has_component {
                    failure = Some(SourceSpan::new(self.pos, self.pos + 1));
                    break;
                }
                styles.push(',');
                has_component = false;
                self.pos += 1;
                payload_end = self.pos;
                continue;
            }
            if matches!(ch, ':' | '#') {
                styles.push(ch);
                has_component = true;
                self.pos += 1;
                payload_end = self.pos;
                continue;
            }
            // Exclusive STYLE_TEXT keeps non-ASCII characters inside an already started
            // word. The earlier whitespace rule applies only at a token's start.
            let text_len = rest
                .chars()
                .take_while(|ch| {
                    !ch.is_ascii() || ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '*')
                })
                .map(char::len_utf8)
                .sum::<usize>();
            if text_len == 0 {
                failure = Some(SourceSpan::new(self.pos, self.pos + ch.len_utf8()));
                break;
            }
            styles.push_str(&rest[..text_len]);
            has_component = true;
            self.pos += text_len;
            payload_end = self.pos;
        }
        let end = separator.unwrap_or(self.pos);
        self.mode = Mode::Default;
        if separator.is_some_and(|offset| self.pos == self.input.len() && self.pos > offset + 1) {
            failure = Some(SourceSpan::new(self.pos, self.pos));
        }
        if let Some(span) = failure {
            // Recover once at the physical line boundary, retaining later editor facts.
            let _ = self.read_to_newline();
            return Some(Err(ErLexicalError::new("Invalid ER style payload", span)));
        }
        if !has_component {
            return Some(Err(ErLexicalError::new(
                "Expected ER style payload",
                SourceSpan::new(start, end),
            )));
        }
        Some(Ok((start, ErToken::RestOfLine(styles), payload_end)))
    }

    /// Whether the word of `len` bytes at the cursor ends at a word boundary, like Mermaid's `\b`
    /// after a word. Mermaid's regexes are not Unicode-aware, so only ASCII `[A-Za-z0-9_]`
    /// continues a word: `tokens` is a name, while `to-do` and `to注文` start with `to`.
    fn ends_word_at(&self, len: usize) -> bool {
        self.input
            .as_bytes()
            .get(self.pos + len)
            .is_none_or(|b| !(b.is_ascii_alphanumeric() || *b == b'_'))
    }

    fn lex_rel_tokens(&mut self) -> Option<(usize, ErToken, usize)> {
        let start = self.pos;
        let s = &self.input[self.pos..];

        // Word cardinalities only match at a word boundary (`/^(?:one or zero\b)/i`,
        // `/^(?:many\b)/i`, `/^(?:to\b)/i`, ...), so entity names such as `tokens`, `oneshot`
        // or `manyToMany` fall through to `lex_name_or_str`.
        let lower = s.to_ascii_lowercase();
        for (pat, tok) in [
            ("optionally to", ErToken::NonIdentifying),
            ("one or zero", ErToken::ZeroOrOne),
            ("zero or one", ErToken::ZeroOrOne),
            ("one or more", ErToken::OneOrMore),
            ("one or many", ErToken::OneOrMore),
            ("zero or more", ErToken::ZeroOrMore),
            ("zero or many", ErToken::ZeroOrMore),
            ("only one", ErToken::OnlyOne),
        ] {
            if lower.starts_with(pat) && self.ends_word_at(pat.len()) {
                self.pos += pat.len();
                return Some((start, tok, self.pos));
            }
        }

        if lower.starts_with("many(0)") {
            self.pos += "many(0)".len();
            return Some((start, ErToken::ZeroOrMore, self.pos));
        }
        if lower.starts_with("many(1)") {
            self.pos += "many(1)".len();
            return Some((start, ErToken::OneOrMore, self.pos));
        }
        if lower.starts_with("0+") {
            self.pos += "0+".len();
            return Some((start, ErToken::ZeroOrMore, self.pos));
        }
        if lower.starts_with("1+") {
            self.pos += "1+".len();
            return Some((start, ErToken::OneOrMore, self.pos));
        }
        if lower.starts_with("many") && self.ends_word_at("many".len()) {
            self.pos += "many".len();
            return Some((start, ErToken::ZeroOrMore, self.pos));
        }
        if lower.starts_with("one") && self.ends_word_at("one".len()) {
            self.pos += "one".len();
            return Some((start, ErToken::OnlyOne, self.pos));
        }
        if self.one_starts_numeric_cardinality() {
            self.pos += 1;
            return Some((start, ErToken::OnlyOne, self.pos));
        }
        if lower.starts_with("to") && self.ends_word_at("to".len()) {
            self.pos += "to".len();
            return Some((start, ErToken::Identifying, self.pos));
        }

        for (pat, tok) in [
            ("||", ErToken::OnlyOne),
            ("|o", ErToken::ZeroOrOne),
            ("o|", ErToken::ZeroOrOne),
            ("|{", ErToken::OneOrMore),
            ("o{", ErToken::ZeroOrMore),
            ("}|", ErToken::OneOrMore),
            ("}o", ErToken::ZeroOrMore),
        ] {
            // Only `|o` and `}o` end with a word character, so only they carry `\b` in Mermaid.
            let needs_boundary = pat.ends_with('o');
            if self.starts_with_ci(pat) && (!needs_boundary || self.ends_word_at(pat.len())) {
                self.pos += pat.len();
                return Some((start, tok, self.pos));
            }
        }

        if s.starts_with("..") || s.starts_with(".-") || s.starts_with("-.") {
            self.pos += 2;
            return Some((start, ErToken::NonIdentifying, self.pos));
        }
        if s.starts_with("--") {
            self.pos += 2;
            return Some((start, ErToken::Identifying, self.pos));
        }

        if self.starts_with_ci("u")
            && matches!(
                self.input.as_bytes().get(self.pos + 1),
                Some(b'.' | b'-' | b'|')
            )
        {
            self.pos += 1;
            return Some((start, ErToken::MdParent, self.pos));
        }

        None
    }

    fn one_starts_numeric_cardinality(&self) -> bool {
        let Some(after_one) = self.input[self.pos..].strip_prefix('1') else {
            return false;
        };

        if ["--", "..", ".-", "-."]
            .iter()
            .any(|operator| after_one.starts_with(operator))
        {
            return true;
        }

        let Some(first) = after_one.chars().next() else {
            return false;
        };
        if !is_ecmascript_whitespace(first) {
            return false;
        }

        after_one
            .trim_start_matches(is_ecmascript_whitespace)
            .chars()
            .next()
            .is_some_and(|next| {
                next.is_ascii_alphabetic()
                    || next.is_ascii_digit()
                    || matches!(next, '_' | '"' | '\'')
            })
    }

    fn lex_punct(&mut self) -> Option<(usize, ErToken, usize)> {
        let start = self.pos;
        match self.peek()? {
            b'{' => {
                self.pos += 1;
                self.mode = Mode::Block;
                Some((start, ErToken::BlockStart, self.pos))
            }
            b'}' => {
                if self.mode != Mode::Block {
                    return None;
                }
                self.pos += 1;
                self.mode = Mode::Default;
                Some((start, ErToken::BlockStop, self.pos))
            }
            b'[' => {
                self.pos += 1;
                Some((start, ErToken::SquareStart, self.pos))
            }
            b']' => {
                self.pos += 1;
                Some((start, ErToken::SquareStop, self.pos))
            }
            b':' => {
                if self.input[self.pos..].starts_with(":::") {
                    self.pos += 3;
                    self.mode = Mode::NeedIdListOnly;
                    return Some((start, ErToken::StyleSeparator, self.pos));
                }
                self.pos += 1;
                Some((start, ErToken::Colon, self.pos))
            }
            b',' => {
                self.pos += 1;
                Some((start, ErToken::Comma, self.pos))
            }
            _ => None,
        }
    }

    fn lex_block_token(
        &mut self,
    ) -> Option<std::result::Result<(usize, ErToken, usize), ErLexicalError>> {
        if self.mode != Mode::Block {
            return None;
        }
        let start = self.pos;
        self.skip_ws_block();
        if self.pos >= self.input.len() {
            self.mode = Mode::Default;
            return Some(Err(ErLexicalError::new(
                "EOF inside attribute block",
                SourceSpan::new(self.pos, self.pos),
            )));
        }
        if self.peek() == Some(b'}') {
            return None;
        }
        if self.peek() == Some(b',') {
            self.pos += 1;
            return Some(Ok((start, ErToken::Comma, self.pos)));
        }
        if self.peek() == Some(b'?') {
            self.pos += 1;
            return Some(Ok((start, ErToken::Question, self.pos)));
        }
        if self.peek() == Some(b'"') {
            self.pos += 1;
            let Some(rel_end) = self.input[self.pos..].find('"') else {
                return Some(Err(ErLexicalError::new(
                    "Unterminated comment string; missing '\"'",
                    SourceSpan::new(start, self.input.len()),
                )));
            };
            let s = self.input[self.pos..self.pos + rel_end].to_string();
            self.pos = self.pos + rel_end + 1;
            return Some(Ok((start, ErToken::Comment(s), self.pos)));
        }
        if self.peek() == Some(b'`') {
            self.pos += 1;
            let body_start = self.pos;
            let Some(rel_end) = self.input[self.pos..].find('`') else {
                return Some(Err(ErLexicalError::new(
                    "Unterminated attribute word; missing '`'",
                    SourceSpan::new(start, self.input.len()),
                )));
            };
            let body_end = self.pos + rel_end;
            let s = self.input[body_start..body_end].to_string();
            self.pos = body_end + 1;
            return Some(Ok((body_start, ErToken::AttrWord(s), body_end)));
        }
        if let Some(two) = self.input[self.pos..].get(..2)
            && ["PK", "FK", "UK"]
                .iter()
                .any(|key| two.eq_ignore_ascii_case(key))
        {
            let is_word_byte = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
            let prev_ok = self.pos == 0 || !is_word_byte(self.input.as_bytes()[self.pos - 1]);
            let next_ok = self
                .input
                .as_bytes()
                .get(self.pos + 2)
                .copied()
                .map(|b| !is_word_byte(b))
                .unwrap_or(true);
            if prev_ok && next_ok {
                let key = two.to_owned();
                self.pos += 2;
                return Some(Ok((start, ErToken::AttrKey(key), self.pos)));
            }
        }

        // Mermaid's generic rule permits spaces between its first and last tilde on the
        // same physical line. Only search beyond the first word when it contains a tilde;
        // a successful match consumes through the last one instead of rescanning suffixes.
        if self.pos >= self.generic_scan_end {
            let rest = &self.input[self.pos..];
            let prefix_end = rest.find(is_ecmascript_whitespace).unwrap_or(rest.len());
            #[cfg(test)]
            {
                self.generic_lookahead_bytes += prefix_end * 2;
            }
            if let Some(first_tilde) = rest[..prefix_end].find('~') {
                let line_end = rest
                    .find(['\r', '\n', '\u{2028}', '\u{2029}'])
                    .unwrap_or(rest.len());
                #[cfg(test)]
                {
                    self.generic_lookahead_bytes += line_end * 2;
                }
                if let Some(last_tilde) = rest[..line_end].rfind('~')
                    && last_tilde > first_tilde
                {
                    let suffix_start = last_tilde + 1;
                    let suffix_len = rest[suffix_start..]
                        .find(is_ecmascript_whitespace)
                        .unwrap_or(rest.len() - suffix_start);
                    let end = self.pos + suffix_start + suffix_len;
                    let word = self.input[self.pos..end].to_owned();
                    self.pos = end;
                    return Some(Ok((start, ErToken::AttrWord(word), end)));
                }
            }
            // Dropping a prefix before the first whitespace cannot make a failed generic
            // rule match. Keep that proof local to this immutable, forward-only cursor.
            self.generic_scan_end = self.pos + prefix_end;
        }

        // The selected case-insensitive [A-Za-z_\u00C0-\uFFFF] rule also admits the
        // micro sign through case folding, and astral characters through UTF-16 surrogates.
        let is_letter = |ch: char| ch.is_ascii_alphabetic() || ch == '\u{00b5}' || ch >= '\u{00c0}';
        let first = self.input[self.pos..].chars().next()?;
        if !is_letter(first) && !matches!(first, '*' | '_') {
            let start_word = self.pos;
            self.pos += self.input[self.pos..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
            return Some(Err(ErLexicalError::new(
                format!("Unexpected character inside attribute block at {start_word}"),
                SourceSpan::new(start_word, self.pos),
            )));
        }
        let start_word = self.pos;
        self.pos += first.len_utf8();
        self.pos += self.input[self.pos..]
            .chars()
            .take_while(|ch| {
                is_letter(*ch)
                    || ch.is_ascii_digit()
                    || matches!(ch, '*' | '-' | '_' | '.' | ',' | '[' | ']' | '(' | ')')
            })
            .map(char::len_utf8)
            .sum::<usize>();
        let raw = &self.input[start_word..self.pos];
        Some(Ok((start, ErToken::AttrWord(raw.to_string()), self.pos)))
    }

    fn lex_name_or_str(
        &mut self,
    ) -> Option<std::result::Result<(usize, ErToken, usize), ErLexicalError>> {
        if self.mode == Mode::Block {
            return None;
        }
        let start = self.pos;
        if self.peek()? == b'"' {
            self.pos += 1;
            let Some(rel_end) = self.input[self.pos..].find('"') else {
                return Some(Err(ErLexicalError::new(
                    "Unterminated string literal; missing '\"'",
                    SourceSpan::new(start, self.input.len()),
                )));
            };
            let s = self.input[self.pos..self.pos + rel_end].to_string();
            self.pos = self.pos + rel_end + 1;
            let is_entity_name = !s.is_empty()
                && !s.contains('%')
                && !s.contains('\\')
                && !s.contains('\r')
                && !s.contains('\n')
                && !s.contains('\u{0008}')
                && !s.contains('\u{000B}');
            if is_entity_name {
                return Some(Ok((start, ErToken::Name(s), self.pos)));
            }
            return Some(Ok((start, ErToken::Str(s), self.pos)));
        }

        let rest = &self.input[self.pos..];
        let mut number_len = rest.bytes().take_while(u8::is_ascii_digit).count();
        if number_len > 0 {
            if rest.as_bytes().get(number_len) == Some(&b'.') {
                let fractional_len = rest[number_len + 1..]
                    .bytes()
                    .take_while(u8::is_ascii_digit)
                    .count();
                if fractional_len > 0 {
                    number_len += 1 + fractional_len;
                }
            }
            self.pos += number_len;
            return Some(Ok((
                start,
                ErToken::Name(rest[..number_len].to_string()),
                self.pos,
            )));
        }

        let mut end = self.pos;
        for (rel, ch) in self.input[self.pos..].char_indices() {
            let ok =
                !ch.is_ascii() || ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '*' | '.');
            if !ok {
                break;
            }
            end = self.pos + rel + ch.len_utf8();
        }
        if end == self.pos {
            return None;
        }
        let s = self.input[self.pos..end].to_string();
        self.pos = end;
        Some(Ok((start, ErToken::Name(s), self.pos)))
    }
}

impl TokenScanner for Lexer<'_> {
    type Event = ErLexicalEvent;

    fn scan_step(&mut self) -> ScanStep<Self::Event> {
        if self.stopped {
            return ScanStep::End;
        }
        // INITIAL accessibility rules precede directions, which in turn
        // precede whitespace. Exclusive attribute/style modes bypass them.
        if matches!(
            self.mode,
            Mode::Default
                | Mode::NeedIdListOnly
                | Mode::NeedClassFirstIdList
                | Mode::NeedClassSecondIdList
        ) {
            if let Some(tok) = self.lex_acc_title() {
                return ScanStep::Emit(tok);
            }
            if let Some(step) = self.lex_acc_descr() {
                return step;
            }
            if let Some(tok) = self.lex_direction() {
                return ScanStep::Emit(Ok(tok));
            }
        }
        let before_whitespace = self.pos;
        match self.mode {
            Mode::Block => self.skip_ws_block(),
            _ => self.skip_ws_default(),
        }
        if self.stopped {
            return ScanStep::End;
        }
        if self.pos != before_whitespace {
            return ScanStep::Skip;
        }

        if self.pos >= self.input.len() {
            if self.mode == Mode::Block {
                self.mode = Mode::Default;
                return ScanStep::Emit(Err(ErLexicalError::new(
                    "EOF inside attribute block",
                    SourceSpan::new(self.pos, self.pos),
                )));
            }
            return ScanStep::End;
        }

        if self.lex_comment() {
            return ScanStep::Skip;
        }

        // Empty exclusive backtick bodies emit no ATTRIBUTE_WORD token in Mermaid.
        if self.mode == Mode::Block && self.input[self.pos..].starts_with("``") {
            self.pos += 2;
            return ScanStep::Skip;
        }

        if let Some(tok) = self.lex_block_token() {
            return ScanStep::Emit(tok);
        }

        if let Some(tok) = self.lex_rest_of_line() {
            return ScanStep::Emit(tok);
        }
        if self.stopped {
            return ScanStep::End;
        }

        if let Some(tok) = self.lex_newline() {
            return ScanStep::Emit(Ok(tok));
        }

        if self.mode == Mode::NeedIdListThenLineRest {
            // Mermaid's exclusive style mode treats keyword-spelled IDs as text.
            if let Some(tok) = self.lex_id_list() {
                return ScanStep::Emit(Ok(tok));
            }
        } else {
            if let Some(tok) = self.lex_keyword() {
                return ScanStep::Emit(Ok(tok));
            }
            if let Some(tok) = self.lex_id_list() {
                return ScanStep::Emit(Ok(tok));
            }
        }

        if let Some(tok) = self.lex_punct() {
            return ScanStep::Emit(Ok(tok));
        }

        if let Some(tok) = self.lex_rel_tokens() {
            return ScanStep::Emit(Ok(tok));
        }

        if let Some(tok) = self.lex_name_or_str() {
            return ScanStep::Emit(tok);
        }

        let start = self.pos;
        let Some(ch) = self.input[self.pos..].chars().next() else {
            return ScanStep::End;
        };
        self.pos += ch.len_utf8();
        ScanStep::Emit(Err(ErLexicalError::new(
            format!("Unexpected character at {start}"),
            SourceSpan::new(start, self.pos),
        )))
    }
}

#[cfg(test)]
impl Iterator for Lexer<'_> {
    type Item = ErLexicalEvent;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::collect_lexical_events;

    #[test]
    fn er_direction_lexer_preserves_ecmascript_line_terminators() {
        // Test raw lexer rules before MermaidEngine preprocessing normalizes line endings.
        for separator in ['\r', '\u{2028}', '\u{2029}'] {
            let source = format!("direction RL{separator}direction TB");
            let (start, token, end) = Lexer::new(&source, None).next().unwrap().unwrap();
            let ErToken::Direction(direction) = token else {
                panic!("expected direction");
            };
            assert_eq!(direction.value, "RL");
            assert_eq!(direction.selection, SourceSpan::new(10, 12));
            assert_eq!((start, end), (0, source.len()));
        }
        let tokens = Lexer::new("A B C\ndirection BT", None)
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            matches!(&tokens.last().unwrap().1, ErToken::Direction(direction) if direction.value == "BT")
        );
    }

    #[test]
    fn er_numeric_cardinality_lookahead_uses_ecmascript_whitespace() {
        for (source, cardinality) in [
            ("1\u{00a0}B", true),
            ("1\u{feff}B", true),
            ("1\u{0085}B", false),
        ] {
            let (start, token, end) = Lexer::new(source, None).next().unwrap().unwrap();
            assert_eq!((start, end), (0, 1));
            if cardinality {
                assert!(matches!(token, ErToken::OnlyOne), "{source:?}");
            } else {
                assert!(
                    matches!(token, ErToken::Name(name) if name == "1"),
                    "{source:?}"
                );
            }
        }
    }

    #[test]
    fn er_attribute_generic_lookahead_bounds_failed_suffix_scans() {
        for count in [64, 1024] {
            for invalid in ["!", "\u{0085}"] {
                for tilde in ["", "~"] {
                    let source = format!(
                        "erDiagram\nA {{{}{tilde}}}\nLater {{int id}}\n",
                        invalid.repeat(count),
                    );
                    let mut lexer = Lexer::new(&source, None);
                    let events =
                        collect_lexical_events(&mut lexer, &OperationControl::new()).unwrap();
                    assert!(
                        lexer.generic_lookahead_bytes <= 5 * source.len(),
                        "{count} {invalid:?} {tilde}"
                    );
                    let error = events
                        .iter()
                        .find_map(|event| event.as_ref().err())
                        .unwrap();
                    let start = source.find(invalid).unwrap();
                    assert_eq!(error.span, SourceSpan::new(start, start + invalid.len()));
                    assert!(events.iter().any(|event| {
                        matches!(event, Ok((_, ErToken::Name(name), _)) if name == "Later")
                    }));
                }
            }
        }
    }

    #[test]
    fn er_empty_backticks_observe_cancellation_before_finishing_skipped_input() {
        let source = format!("erDiagram\nA {{{}}}\n", "``".repeat(1024));
        let mut lexer = Lexer::new(&source, None);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        assert!(collect_lexical_events(&mut lexer, &control).is_err());
        assert!(lexer.pos < source.len() / 2);
    }

    #[test]
    fn er_multiline_style_observes_cancellation_before_finishing_aggregate() {
        for body in ["x \n".repeat(1024), " \n".repeat(1024)] {
            let source = format!("erDiagram\nA\nstyle A {body}x\n");
            let control = OperationControl::new();
            control.cancel_after_checkpoints(6);
            let mut lexer = Lexer::new(&source, Some(&control));
            assert!(collect_lexical_events(&mut lexer, &control).is_err());
            assert!(lexer.pos < source.len() / 2);
        }
    }

    #[test]
    fn er_parent_marker_uses_only_the_following_character() {
        for source in ["u-table", "U.table", "u|", "\"A\"u--"] {
            let mut lexer = Lexer::new(source, None);
            if source.starts_with('"') {
                assert!(matches!(lexer.next().unwrap().unwrap().1, ErToken::Name(_)));
            }
            assert!(
                matches!(lexer.next().unwrap().unwrap().1, ErToken::MdParent),
                "{source}"
            );
        }
        for source in ["u", "user", "u_table"] {
            assert!(
                matches!(
                    Lexer::new(source, None).next().unwrap().unwrap().1,
                    ErToken::Name(_)
                ),
                "{source}"
            );
        }
    }
}
