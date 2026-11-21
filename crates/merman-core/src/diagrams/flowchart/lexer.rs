#[cfg(feature = "editor-facts")]
use super::ast::{FlowchartClickEditorEvidence, FlowchartDirectiveEditorEvidence};
use super::text::{LabelScanner, LabelStringState};
use super::{
    ArrowToken, DirectionStatementToken, LabeledText, LexError, LinkToken, NodeLabelToken,
    SubgraphHeader, TitleKind, Tok, destruct_end_link, destruct_labeled_end_link,
    destruct_start_link, is_ecmascript_trim_char, lex, parse_label_text,
};
#[cfg(feature = "editor-facts")]
use super::{ClickAction, ClickStmt};
use crate::diagrams::jison_unicode::is_mermaid_unicode_text;
#[cfg(feature = "editor-facts")]
use crate::{EditorExpectedSyntax, EditorExpectedSyntaxKind};
use crate::{OperationControl, SourceSpan};
use std::collections::VecDeque;

#[cfg(feature = "editor-facts")]
fn directive_argument_spans(
    rest: &str,
    rest_start: usize,
) -> (Option<SourceSpan>, Option<SourceSpan>) {
    let leading = rest
        .char_indices()
        .take_while(|(_, ch)| is_ecmascript_trim_char(*ch))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0);
    let body = &rest[leading..];
    if body.is_empty() {
        return (None, None);
    }

    let first_len = body
        .char_indices()
        .find(|(_, ch)| is_ecmascript_trim_char(*ch))
        .map_or(body.len(), |(index, _)| index);
    let first_start = rest_start + leading;
    let first_end = first_start + first_len;
    let first = SourceSpan::new(first_start, first_end);
    let remainder = &body[first_len..];
    if remainder.is_empty() {
        return (Some(first), None);
    }

    let remainder_leading = remainder
        .char_indices()
        .take_while(|(_, ch)| is_ecmascript_trim_char(*ch))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0);
    let value_start = first_end + remainder_leading;
    let value_end = rest_start + rest.len();
    (Some(first), Some(SourceSpan::new(value_start, value_end)))
}

#[cfg(feature = "editor-facts")]
fn directive_error<I>(
    mut error: LexError,
    prefix: &'static str,
    statement_span: SourceSpan,
    expected_syntax: I,
) -> LexError
where
    I: IntoIterator<Item = (EditorExpectedSyntaxKind, SourceSpan)>,
{
    for (kind, span) in expected_syntax {
        error = error.expecting(kind, span);
    }
    error.in_directive(prefix, statement_span)
}

#[cfg(feature = "editor-facts")]
fn active_following_span(
    rest: &str,
    rest_start: usize,
    following: Option<SourceSpan>,
) -> Option<SourceSpan> {
    let following = following?;
    let local_start = following.start.checked_sub(rest_start)?;
    let raw = rest.get(local_start..)?;
    let trailing = raw
        .char_indices()
        .rev()
        .take_while(|(_, ch)| is_ecmascript_trim_char(*ch))
        .map(|(index, _)| raw.len() - index)
        .last()
        .unwrap_or(0);
    if raw.is_empty() || trailing > 0 {
        return Some(SourceSpan::new(following.end, following.end));
    }
    Some(following)
}

#[cfg(feature = "editor-facts")]
fn trailing_value_slot(
    rest: &str,
    rest_start: usize,
    following: Option<SourceSpan>,
) -> Option<SourceSpan> {
    let active = active_following_span(rest, rest_start, following)?;
    (active.start == active.end).then_some(active)
}

#[cfg(feature = "editor-facts")]
fn directive_editor_evidence(
    statement_span: SourceSpan,
    first: Option<(EditorExpectedSyntaxKind, SourceSpan)>,
    following: Option<(EditorExpectedSyntaxKind, SourceSpan)>,
) -> FlowchartDirectiveEditorEvidence {
    FlowchartDirectiveEditorEvidence::new(
        EditorExpectedSyntax::new(EditorExpectedSyntaxKind::Directive, statement_span),
        first.map(|(kind, span)| EditorExpectedSyntax::new(kind, span)),
        following.map(|(kind, span)| EditorExpectedSyntax::new(kind, span)),
    )
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

fn non_ascii_id_char_len(input: &str, pos: usize) -> Option<usize> {
    if !input.is_char_boundary(pos) {
        return None;
    }
    let ch = input[pos..].chars().next()?;
    (!ch.is_ascii() && is_mermaid_unicode_text(ch)).then(|| ch.len_utf8())
}

#[derive(Clone, Copy)]
enum LinkFamily {
    Normal,
    Thick,
    Dotted,
    Invisible,
}

impl LinkFamily {
    fn from_start(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [b'-', b'-', ..] => Some(Self::Normal),
            [b'=', b'=', ..] => Some(Self::Thick),
            [b'-', b'.', ..] => Some(Self::Dotted),
            _ => None,
        }
    }
}

// One or more dots followed by a hyphen, after any leading marker or hyphen.
fn dotted_link_prefix_len(bytes: &[u8]) -> Option<usize> {
    let dots = bytes.iter().take_while(|&&byte| byte == b'.').count();
    (dots > 0 && bytes.get(dots) == Some(&b'-')).then_some(dots + 1)
}

// LINK and START_LINK take precedence when the lexer starts a new idStringToken.
fn link_prefix_len(bytes: &[u8]) -> Option<usize> {
    let marker = usize::from(matches!(bytes.first(), Some(b'x' | b'o' | b'<')));
    let body = &bytes[marker..];
    let len = if body.starts_with(b".") {
        dotted_link_prefix_len(body)?
    } else {
        LinkFamily::from_start(body)?;
        2
    };
    Some(marker + len)
}

#[cfg(feature = "editor-facts")]
pub(crate) fn node_rename_text(name: &str, preceding: &str, following: &str) -> String {
    let mut text = String::with_capacity(name.len() + 2);
    // An open LINK can consume a marker or an extra dash from the target.
    let adds_marker = name.starts_with(['o', 'x'])
        && matches!(
            preceding.as_bytes(),
            [.., b'-' | b'.', b'-'] | [.., b'=', b'=']
        );
    if adds_marker
        || (name.starts_with('-') && preceding.ends_with("--"))
        || (name.contains('#') && fills_hex_style_value(preceding, following))
    {
        text.push(' ');
    }
    text.push_str(name);
    if following.starts_with(';')
        && name.rsplit_once('#').is_some_and(|(_, word)| {
            !word.is_empty() && word.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
    {
        // Mermaid preprocesses #word; as a character entity before lexing the statement.
        text.push(' ');
    } else if let Some(len) = link_prefix_len(following.as_bytes()) {
        let probe = [name, &following[..len]].concat();
        let mut lexer = Lexer::new(&probe);
        lexer.advance_id();
        if lexer.pos != name.len() {
            text.push(' ');
        }
    }
    text
}

#[cfg(feature = "editor-facts")]
fn fills_hex_style_value(preceding: &str, following: &str) -> bool {
    let line_ending = ['\n', '\r', '\u{2028}', '\u{2029}'];
    let after = following.split(line_ending).next().unwrap_or_default();
    if !after.contains(';') {
        return false;
    }
    let before = preceding.rsplit(line_ending).next().unwrap_or_default();
    let value = before
        .rsplit(is_ecmascript_trim_char)
        .next()
        .unwrap_or_default();
    let Some(colon) = value.rfind(':') else {
        return false;
    };
    // Mermaid's `style.*:\S*#.*;` and `classDef` equivalent also see labels on this line.
    let head = &before[..before.len() - value.len() + colon];
    head.contains("style") || head.contains("classDef")
}

fn is_node_string_ascii_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!\"#$%&'*+.`?\\_/".contains(&byte)
}

fn single_quoted_value_end(suffix: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, byte) in suffix.bytes().enumerate() {
        match (escaped, byte) {
            (_, b'\n' | b'\r') => return None,
            (false, b'\'') => return Some(index),
            (false, b'\\') => escaped = true,
            _ => escaped = false,
        }
    }
    None
}

fn node_string_len(bytes: &[u8]) -> usize {
    // Jison 0.4.18 compiles flow.jison's `=(?!=)` alternative to `(?!)`, so a single
    // equals sign is outside NODE_STRING. Thick links have their own earlier rules.
    for (pos, &byte) in bytes.iter().enumerate() {
        if is_node_string_ascii_byte(byte) {
            continue;
        }
        if byte == b'-'
            && bytes
                .get(pos + 1)
                .is_some_and(|next| !matches!(next, b'>' | b'-' | b'.'))
        {
            continue;
        }
        return pos;
    }
    bytes.len()
}

pub(super) struct DirectionMatch {
    pub(super) direction: &'static str,
    pub(super) offset: usize,
    #[cfg(any(test, feature = "editor-facts"))]
    unambiguous: bool,
}

pub(super) fn match_direction(input: &str, prefix: &str) -> Option<DirectionMatch> {
    // Jison selects the first matching rule. Its greedy prefix selects the last occurrence
    // of that rule's direction, which also owns the editor selection.
    let candidates = prefix
        .rmatch_indices("direction")
        .filter_map(|(offset, keyword)| {
            let suffix = &input[offset + keyword.len()..];
            let value = suffix.trim_start_matches(is_ecmascript_trim_char);
            if suffix.len() == value.len() {
                return None;
            }
            ["TB", "BT", "RL", "LR", "TD"]
                .into_iter()
                .enumerate()
                .find(|(_, dir)| value.starts_with(dir))
                .map(|(priority, dir)| (priority, dir, input.len() - value.len()))
        });
    #[cfg(any(test, feature = "editor-facts"))]
    let mut count = 0_usize;
    #[cfg(any(test, feature = "editor-facts"))]
    let candidates = candidates.inspect(|_| count += 1);
    let (_, direction, offset) = candidates.min_by_key(|(priority, _, _)| *priority)?;
    Some(DirectionMatch {
        direction,
        offset,
        #[cfg(any(test, feature = "editor-facts"))]
        unambiguous: count == 1,
    })
}

pub(super) struct Lexer<'input> {
    pub(super) input: &'input str,
    pub(super) pos: usize,
    pub(super) pending: VecDeque<std::result::Result<(usize, Tok, usize), LexError>>,
    pub(super) after_vertex: bool,
    pub(super) allow_header_direction: bool,
    pub(super) recover_partial_node_labels: bool,
    // The direction rules look through the rest of a statement. Cache misses across its tokens.
    direction_checked_until: usize,
    direction_statement_ends: VecDeque<usize>,
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(super) accessibility: super::accessibility::FlowchartAccessibility,
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(super) control: Option<OperationControl>,
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(super) at_statement_start: bool,
    label_scanner: LabelScanner,
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
            after_vertex: false,
            allow_header_direction: false,
            recover_partial_node_labels: false,
            direction_checked_until: 0,
            direction_statement_ends: VecDeque::new(),
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            accessibility: super::accessibility::FlowchartAccessibility::default(),
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            control: None,
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            at_statement_start: true,
            label_scanner: LabelScanner::default(),
        }
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(super) fn with_control(mut self, control: &OperationControl) -> Self {
        self.label_scanner.set_control(control);
        self.control = Some(control.clone());
        #[cfg(test)]
        super::accessibility::record_flowchart_accessibility_scan();
        self
    }

    #[cfg(all(test, any(feature = "diagram-flowchart", feature = "diagram-swimlane")))]
    pub(super) fn label_scan_work(&self) -> usize {
        self.label_scanner.work
    }

    fn find_label_end(
        &mut self,
        start: usize,
        delim: &'static str,
    ) -> Result<Option<usize>, LexError> {
        self.label_scanner
            .find_unquoted_delim(self.input, start, delim)
            .map_err(|_| {
                self.pos = self.input.len();
                LexError::new("Flowchart label scan cancelled")
            })
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    #[cfg(feature = "editor-facts")]
    pub(super) fn recovering(input: &'input str) -> Self {
        let mut lexer = Self {
            recover_partial_node_labels: true,
            ..Self::new(input)
        };
        lexer.accessibility.capture_statements = true;
        lexer
    }

    pub(super) fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    pub(super) fn starts_with_kw(&self, kw: &str) -> bool {
        // Jison compiles literal keywords with JavaScript's ASCII word boundary.
        self.input[self.pos..]
            .strip_prefix(kw)
            .is_some_and(|suffix| {
                !suffix
                    .as_bytes()
                    .first()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            })
    }

    fn starts_with_href_keyword(&self) -> bool {
        // HREF uses `href[\s]`, including at a token boundary inside an idString. Mermaid
        // appends a newline to the input before invoking the generated parser.
        self.input[self.pos..]
            .strip_prefix("href")
            .is_some_and(|suffix| suffix.chars().next().is_none_or(is_ecmascript_trim_char))
    }

    fn reserved_keyword(&self) -> Option<&'static str> {
        let candidates: &[&str] = match self.peek()? {
            b'c' => &["classDef", "class"],
            b'e' => &["end"],
            b'f' => &["flowchart"],
            b'g' => &["graph"],
            b'h' => return self.starts_with_href_keyword().then_some("href"),
            b'i' => &["interpolate"],
            b'l' => &["linkStyle"],
            b's' => &["style", "subgraph", "swimlane-beta"],
            b'_' => &["_self", "_blank", "_parent", "_top"],
            _ => return None,
        };
        candidates
            .iter()
            .copied()
            .find(|kw| self.starts_with_kw(kw))
    }

    pub(super) fn lex_reserved_token_error(&mut self) -> Option<LexError> {
        let rest = &self.input[self.pos..];
        // Jison compiles these INITIAL keyword rules with an ASCII word boundary before
        // NODE_STRING. Presentation statements are scanned as a whole below, but their keyword
        // still requires a SPACE token before args.
        // HREF, INTERPOLATE, and LINK_TARGET have no standalone use as keywords. Their valid
        // positions are consumed inside the click/linkStyle scanners instead.
        let keyword = self.reserved_keyword()?;
        if matches!(keyword, "end" | "flowchart" | "graph" | "swimlane-beta") {
            return None;
        }
        if matches!(
            keyword,
            "style" | "linkStyle" | "classDef" | "class" | "subgraph"
        ) {
            let suffix = &rest[keyword.len()..];
            if suffix.is_empty()
                || suffix.chars().next().is_some_and(is_ecmascript_trim_char)
                || (keyword == "subgraph" && suffix.starts_with(';'))
            {
                return None;
            }
        }

        Some(self.unexpected_keyword(keyword))
    }

    fn unexpected_keyword(&mut self, keyword: &str) -> LexError {
        let start = self.pos;
        self.pos += keyword.len();
        LexError::with_span(
            format!("Unexpected flowchart keyword {keyword}"),
            SourceSpan::new(start, self.pos),
        )
    }

    pub(super) fn skip_ws(&mut self) {
        while let Some(ch) = self.input[self.pos..].chars().next() {
            if !is_ecmascript_trim_char(ch) || ch == '\n' {
                break;
            }
            self.pos += ch.len_utf8();
        }
    }

    pub(super) fn lex_sep(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        match self.peek()? {
            b'\n' => {
                let bytes = self.input.as_bytes();
                let mut look = self.pos + 1;
                while let Some(ch) = self.input[look..].chars().next() {
                    if ch == '\n' || !is_ecmascript_trim_char(ch) {
                        break;
                    }
                    look += ch.len_utf8();
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
            b';' if !self.allow_header_direction => {
                self.pos += 1;
                Some((start, Tok::Sep, self.pos))
            }
            _ => None,
        }
    }

    pub(super) fn lex_direction(&mut self) -> Option<(usize, Tok, usize)> {
        if !self.allow_header_direction {
            return None;
        }
        let start = self.pos;
        let rest = &self.input[self.pos..];
        for d in ["TB", "TD", "BT", "LR", "RL", "BR"] {
            if rest.starts_with(d) {
                let after = self.pos + d.len();
                if !self.header_direction_has_separator(after) {
                    return None;
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
                if !self.header_direction_has_separator(after) {
                    return None;
                }
                self.pos = after;
                self.allow_header_direction = false;
                return Some((start, Tok::Direction(d.to_string()), self.pos));
            }
        }

        None
    }

    fn header_direction_has_separator(&self, after: usize) -> bool {
        let rest = &self.input[after..];
        let after_space = rest.trim_start_matches(|ch| ch != '\n' && is_ecmascript_trim_char(ch));
        // flow.jison FirstStmtSeparator permits a direct semicolon or whitespace plus newline.
        // Diagram.fromText appends a newline, so the public EOF spelling is equivalent.
        rest.starts_with(';') || after_space.is_empty() || after_space.starts_with('\n')
    }

    fn direction_statement_end(&mut self, start: usize, require_keyword: bool) -> usize {
        while self
            .direction_statement_ends
            .front()
            .is_some_and(|&end| end < start)
        {
            self.direction_statement_ends.pop_front();
        }
        if let Some(&end) = self.direction_statement_ends.front() {
            return end;
        }
        let line = self.input[start..].split('\n').next().unwrap_or_default();
        let line_end = start + line.len();
        if line.contains(';') && (!require_keyword || line.contains("direction")) {
            // Only a separator token ends a compact statement. Semicolons inside node labels,
            // edge labels and shape data belong to the token that consumed them.
            let mut tokens = Lexer::new(line);
            tokens.direction_checked_until = usize::MAX;
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            if let Some(control) = &self.control {
                tokens.label_scanner.set_control(control);
                tokens.control = Some(control.clone());
            }
            self.direction_statement_ends
                .extend(tokens.flatten().filter_map(|(pos, token, _)| {
                    (matches!(token, Tok::Sep) && line.as_bytes().get(pos) == Some(&b';'))
                        .then_some(start + pos)
                }));
        }
        self.direction_statement_ends.push_back(line_end);
        self.direction_statement_ends[0]
    }

    pub(super) fn lex_whole_line_direction(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if self.allow_header_direction || start < self.direction_checked_until {
            return None;
        }
        // JavaScript `.*` stops at a line terminator. Merman also admits semicolon-separated
        // statements. Only the whitespace between `direction` and the value can cross lines.
        let boundary = self.direction_statement_end(start, true);
        let prefix = self.input[start..boundary]
            .split(['\r', '\u{2028}', '\u{2029}'])
            .next()
            .unwrap_or_default();
        let rest = &self.input[start..];
        let Some(matched) = match_direction(rest, prefix) else {
            self.direction_checked_until = start + prefix.len();
            return None;
        };
        let direction_start = start + matched.offset;
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        if let Some(token) = self.lex_accessibility_statement() {
            return Some(token);
        }
        // flow.jison allows DEFAULT as an identifier token. An adjacent `@{` then opens
        // the shape-data state at the next cursor, ahead of the direction rules.
        if rest.starts_with('"')
            || rest.starts_with("@{")
            || rest.starts_with("default@{")
            || ["click", "call", "href"].iter().any(|kw| {
                rest.strip_prefix(kw)
                    .and_then(|tail| tail.chars().next())
                    .is_some_and(is_ecmascript_trim_char)
            })
        {
            return None;
        }

        let end = direction_start + matched.direction.len();
        let statement_end = self.direction_statement_end(direction_start, false);
        let selection = SourceSpan::new(direction_start, end);
        // At the current cursor, these INITIAL keywords precede the direction rules. An
        // unindented `class direction LR` emits CLASS then direction_lr where a target is needed.
        // With leading spaces, the direction rule instead consumes the whole statement.
        if let Some(keyword) = self
            .reserved_keyword()
            .or_else(|| self.starts_with_kw("default").then_some("default"))
        {
            if matches!(keyword, "graph" | "flowchart" | "end" | "swimlane-beta") {
                return None;
            }
            self.pos = statement_end;
            return Some(Err(LexError::with_span(
                format!("unexpected direction in Flowchart {keyword} statement"),
                selection,
            )));
        }
        self.pos = statement_end;
        Some(Ok((
            start,
            Tok::DirectionStmt(DirectionStatementToken {
                direction: matched.direction.to_string(),
                #[cfg(any(test, feature = "editor-facts"))]
                // Replacing a competing value can change which line the token consumes.
                completion_span: matched.unambiguous.then_some(selection),
                #[cfg(any(test, feature = "editor-facts"))]
                recovery_error: None,
            }),
            statement_end,
        )))
    }

    #[cfg(feature = "editor-facts")]
    pub(super) fn lex_partial_direction_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        if !self.recover_partial_node_labels {
            return None;
        }
        let start = self.pos;
        let rest = self.input[start..].strip_prefix("direction")?;
        let value = rest.trim_start_matches(|ch| ch != '\n' && is_ecmascript_trim_char(ch));
        if value.len() == rest.len() {
            return None;
        }
        let direction_start = self.input.len() - value.len();
        let direction_len = value
            .find(|ch: char| is_ecmascript_trim_char(ch) || ch == ';')
            .unwrap_or(value.len());
        let candidate = &value[..direction_len];
        // Two adjacent vertex IDs cannot form a statement. A spaced `&` groups vertices,
        // while labels, named links, and other continuations have their own token kinds.
        if candidate == "&" || !super::is_valid_editor_node_id(candidate) {
            return None;
        }

        let direction_end = direction_start + direction_len;
        let statement_end = self.input[direction_end..]
            .find(['\n', ';'])
            .map_or(self.input.len(), |offset| direction_end + offset);
        let selection = SourceSpan::new(direction_start, direction_end);

        let error = LexError::with_span("invalid flowchart direction", selection).expecting(
            crate::EditorExpectedSyntaxKind::FlowchartDirectionValue,
            selection,
        );

        self.pos = statement_end;

        Some(Ok((
            start,
            Tok::DirectionStmt(DirectionStatementToken {
                direction: String::new(),
                completion_span: Some(selection),
                recovery_error: Some(error),
            }),
            statement_end,
        )))
    }

    fn advance_to_stmt_end(&mut self, literal_word_apostrophes: bool) {
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
                if literal_word_apostrophes {
                    // Bare directive values accept NODE_STRING apostrophes. Keep support for
                    // delimited single-quoted values on the current line.
                    let in_bare_word =
                        self.input[start..self.pos]
                            .chars()
                            .next_back()
                            .is_some_and(|ch| {
                                if ch.is_ascii() {
                                    ch == '-' || is_node_string_ascii_byte(ch as u8)
                                } else {
                                    is_mermaid_unicode_text(ch)
                                }
                            });
                    if !in_bare_word
                        && let Some(end) = single_quoted_value_end(&self.input[self.pos + 1..])
                    {
                        self.pos += end + 1;
                    }
                } else {
                    in_single_quote = true;
                }
                self.pos += 1;
                continue;
            }

            if b == b'\n' || b == b';' {
                break;
            }
            self.pos += 1;
        }
    }

    fn capture_directive_arguments(
        &mut self,
        leading_ids: usize,
    ) -> Result<(usize, &'input str, usize), LexError> {
        let start = self.pos;
        // Directive identifiers follow NODE_STRING lexing, where apostrophes are literal.
        // Scan the remaining payload separately so quoted tooltip/style delimiters still work.
        for _ in 0..leading_ids {
            let id_start = self.pos;
            self.advance_id();
            if self.starts_with_href_keyword() {
                let error = self.unexpected_keyword("href");
                self.advance_to_stmt_end(true);
                return Err(error);
            }
            if self.pos == id_start {
                break;
            }
            let id_end = self.pos;
            self.skip_ws();
            if self.pos == id_end {
                break;
            }
        }
        Ok(self.capture_directive_payload(start))
    }

    fn capture_directive_payload(&mut self, start: usize) -> (usize, &'input str, usize) {
        self.advance_to_stmt_end(true);
        (start, &self.input[start..self.pos], self.pos)
    }

    fn stmt_end_from(&mut self, start: usize) -> usize {
        self.pos = start;
        self.advance_to_stmt_end(false);
        self.pos
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
        #[cfg(feature = "editor-facts")]
        let expected = super::shape_value_expected_span(self.input, start, self.pos)
            .unwrap_or(SourceSpan::new(self.pos, self.pos));
        Some(Err({
            let error = LexError::with_span("Unterminated shape data (missing `}`)", span);
            #[cfg(feature = "editor-facts")]
            let error = error.expecting(crate::EditorExpectedSyntaxKind::ShapeValue, expected);
            error
        }))
    }

    pub(super) fn lex_edge_id(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if start >= self.input.len() {
            return None;
        }

        let mut cursor = start;
        let mut at = None;
        while cursor < self.input.len() {
            let ch = self.input[cursor..]
                .chars()
                .next()
                .expect("edge id scan position must contain a character");
            if is_ecmascript_trim_char(ch)
                || matches!(ch, '"' | '[' | ']' | '(' | ')' | '{' | '}' | '|')
            {
                break;
            }
            if ch == '@' {
                let next = self.input[cursor + 1..].chars().next();
                if cursor > start && matches!(next, Some(next) if next != '{' && next != '"') {
                    at = Some(cursor);
                }
            }
            cursor += ch.len_utf8();
        }

        let at = at?;
        self.pos = at + 1;
        Some((
            start,
            Tok::EdgeId(self.input[start..at].to_string()),
            self.pos,
        ))
    }

    pub(super) fn lex_style_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("style") {
            return None;
        }
        self.pos += "style".len();
        self.skip_ws();
        let (rest_start, rest, end) = match self.capture_directive_arguments(1) {
            Ok(args) => args,
            Err(error) => return Some(Err(error)),
        };
        #[cfg(feature = "editor-facts")]
        let statement_span = SourceSpan::new(start, end);
        #[cfg(feature = "editor-facts")]
        let (target, style) = directive_argument_spans(rest, rest_start);
        #[cfg(feature = "editor-facts")]
        let style_slot = trailing_value_slot(rest, rest_start, style);
        match lex::parse_style_stmt(rest) {
            Ok(mut stmt) => {
                lex::attach_style_stmt_spans(&mut stmt, rest, rest_start);
                #[cfg(feature = "editor-facts")]
                {
                    stmt.editor_evidence = directive_editor_evidence(
                        statement_span,
                        target.map(|span| (EditorExpectedSyntaxKind::NodeIdentifier, span)),
                        style_slot.map(|span| (EditorExpectedSyntaxKind::StyleValue, span)),
                    );
                }
                Some(Ok((start, Tok::StyleStmt(stmt), end)))
            }
            Err(error) => Some(Err({
                #[cfg(feature = "editor-facts")]
                {
                    directive_error(
                        error,
                        "style",
                        statement_span,
                        target
                            .map(|span| (EditorExpectedSyntaxKind::NodeIdentifier, span))
                            .into_iter()
                            .chain((target.is_none()).then_some((
                                EditorExpectedSyntaxKind::NodeIdentifier,
                                SourceSpan::new(end, end),
                            ))),
                    )
                }
                #[cfg(not(feature = "editor-facts"))]
                {
                    error
                }
            })),
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
        self.skip_ws();
        let (_rest_start, rest, end) = match self.capture_directive_arguments(1) {
            Ok(args) => args,
            Err(error) => return Some(Err(error)),
        };
        #[cfg(feature = "editor-facts")]
        let rest_start = _rest_start;
        #[cfg(feature = "editor-facts")]
        let statement_span = SourceSpan::new(start, end);
        #[cfg(feature = "editor-facts")]
        let (class_name, style) = directive_argument_spans(rest, rest_start);
        #[cfg(feature = "editor-facts")]
        let style_slot = trailing_value_slot(rest, rest_start, style);
        match lex::parse_classdef_stmt(rest) {
            Ok(stmt) => {
                #[cfg(feature = "editor-facts")]
                let mut stmt = stmt;
                #[cfg(feature = "editor-facts")]
                {
                    lex::attach_classdef_stmt_spans(&mut stmt, rest, rest_start);
                    stmt.editor_evidence = directive_editor_evidence(
                        statement_span,
                        class_name.map(|span| (EditorExpectedSyntaxKind::ClassName, span)),
                        style_slot.map(|span| (EditorExpectedSyntaxKind::StyleValue, span)),
                    );
                }
                Some(Ok((start, Tok::ClassDefStmt(stmt), end)))
            }
            Err(error) => Some(Err({
                #[cfg(feature = "editor-facts")]
                {
                    directive_error(
                        error,
                        "classDef",
                        statement_span,
                        class_name.map(|span| (EditorExpectedSyntaxKind::ClassName, span)),
                    )
                }
                #[cfg(not(feature = "editor-facts"))]
                {
                    error
                }
            })),
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
        self.skip_ws();
        let (_rest_start, rest, end) = match self.capture_directive_arguments(2) {
            Ok(args) => args,
            Err(error) => return Some(Err(error)),
        };
        #[cfg(feature = "editor-facts")]
        let rest_start = _rest_start;
        #[cfg(feature = "editor-facts")]
        let statement_span = SourceSpan::new(start, end);
        #[cfg(feature = "editor-facts")]
        let (targets, class_name) = directive_argument_spans(rest, rest_start);
        #[cfg(feature = "editor-facts")]
        let class_name = active_following_span(rest, rest_start, class_name);
        match lex::parse_class_assign_stmt(rest) {
            Ok(stmt) => {
                #[cfg(feature = "editor-facts")]
                let mut stmt = stmt;
                #[cfg(feature = "editor-facts")]
                {
                    lex::attach_class_assign_stmt_spans(&mut stmt, rest, rest_start);
                    stmt.editor_evidence = directive_editor_evidence(
                        statement_span,
                        targets.map(|span| (EditorExpectedSyntaxKind::IdList, span)),
                        class_name.map(|span| (EditorExpectedSyntaxKind::ClassName, span)),
                    );
                }
                Some(Ok((start, Tok::ClassAssignStmt(stmt), end)))
            }
            Err(error) => {
                #[cfg(feature = "editor-facts")]
                let target = targets.unwrap_or_else(|| SourceSpan::new(end, end));
                Some(Err({
                    #[cfg(feature = "editor-facts")]
                    {
                        directive_error(
                            error,
                            "class",
                            statement_span,
                            [(EditorExpectedSyntaxKind::IdList, target)]
                                .into_iter()
                                .chain(
                                    class_name
                                        .map(|span| (EditorExpectedSyntaxKind::ClassName, span)),
                                ),
                        )
                    }
                    #[cfg(not(feature = "editor-facts"))]
                    {
                        error
                    }
                }))
            }
        }
    }

    pub(super) fn lex_click_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.input[self.pos..]
            .strip_prefix("click")
            .and_then(|rest| rest.chars().next())
            .is_some_and(is_ecmascript_trim_char)
        {
            return None;
        }
        self.pos += "click".len();
        self.skip_ws();
        // The click state takes a bare target word, including names ending in `href`.
        let target_start = self.pos;
        let suffix = &self.input[target_start..];
        self.pos += suffix
            .find(|ch| is_ecmascript_trim_char(ch) || ch == ';')
            .unwrap_or(suffix.len());
        self.advance_to_stmt_end(true);
        let end = self.pos;
        let body = SourceSpan::new(target_start, end);
        #[cfg(feature = "editor-facts")]
        let statement_span = SourceSpan::new(start, end);
        #[cfg(feature = "editor-facts")]
        let (target, _) = directive_argument_spans(&self.input[body.start..end], body.start);
        match lex::parse_click_stmt(self.input, body) {
            Ok(stmt) => {
                #[cfg(feature = "editor-facts")]
                let mut stmt = stmt;
                #[cfg(feature = "editor-facts")]
                {
                    stmt.editor_evidence = directive_editor_evidence(
                        statement_span,
                        target.map(|span| (EditorExpectedSyntaxKind::NodeIdentifier, span)),
                        None,
                    );
                }
                Some(Ok((start, Tok::ClickStmt(stmt), end)))
            }
            Err(error) => {
                #[cfg(feature = "editor-facts")]
                let target = target.unwrap_or_else(|| SourceSpan::new(end, end));
                let error = {
                    #[cfg(feature = "editor-facts")]
                    {
                        directive_error(
                            error,
                            "click",
                            statement_span,
                            [(EditorExpectedSyntaxKind::NodeIdentifier, target)],
                        )
                    }
                    #[cfg(not(feature = "editor-facts"))]
                    {
                        error
                    }
                };
                #[cfg(feature = "editor-facts")]
                if self.recover_partial_node_labels && target.start < target.end {
                    let (ids, id_spans) = lex::split_click_targets(
                        &self.input[target.start..target.end],
                        target.start,
                    )
                    .map(|(id, span)| (id.to_string(), span))
                    .unzip();
                    return Some(Ok((
                        start,
                        Tok::ClickStmt(ClickStmt {
                            ids,
                            id_spans,
                            tooltip: None,
                            action: ClickAction::Callback,
                            #[cfg(feature = "editor-facts")]
                            editor_evidence: directive_editor_evidence(
                                statement_span,
                                Some((EditorExpectedSyntaxKind::NodeIdentifier, target)),
                                None,
                            ),
                            #[cfg(feature = "editor-facts")]
                            interaction_evidence: FlowchartClickEditorEvidence::default(),
                            #[cfg(any(test, feature = "editor-facts"))]
                            recovery_error: Some(error),
                        }),
                        end,
                    )));
                }
                Some(Err(error))
            }
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
        self.skip_ws();
        let (rest_start, rest, end) = self.capture_directive_payload(self.pos);
        match lex::parse_link_style_stmt(rest, rest_start) {
            Ok(stmt) => Some(Ok((start, Tok::LinkStyleStmt(stmt), end))),
            Err(e) => Some(Err(e)),
        }
    }

    pub(super) fn lex_subgraph_header_after_keyword(
        &mut self,
        keyword_start: usize,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        // Mermaid's current flowchart parser treats all horizontal whitespace after the
        // `subgraph` keyword as a separator. Extra spaces therefore do not become part of the
        // header token and must not force an otherwise named subgraph onto an auto-generated id.
        //
        // Example:
        // - `subgraph main`   -> id stays `main`
        // - `subgraph  main`  -> id also stays `main`
        let rest = &self.input[self.pos..];
        if rest.starts_with('\n') || rest.starts_with("\r\n") || rest.starts_with(';') {
            return None;
        }
        while let Some(ch) = self.input[self.pos..].chars().next()
            && ch != '\n'
            && ch != '\r'
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

        let mut string_state = LabelStringState::default();
        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];
            if let Some(consumed) = string_state.consume_delimiter(&self.input[self.pos..]) {
                self.pos += consumed;
                continue;
            }
            if string_state.is_plain() && matches!(b, b'\n' | b'\r' | b';' | b'[') {
                break;
            }
            self.pos += self.input[self.pos..].chars().next()?.len_utf8();
        }

        let raw_id_end = self.pos;
        let authored_id = &self.input[start..raw_id_end];
        let (id_payload, id_kind) = if authored_id.starts_with('"') {
            match super::text::parse_quoted_label(authored_id) {
                Ok(parsed) => parsed,
                Err(error) => {
                    return Some(Err(LexError::with_span(
                        error.message,
                        SourceSpan::new(start, raw_id_end),
                    )));
                }
            }
        } else {
            if authored_id.contains('"') {
                return Some(Err(LexError::with_span(
                    "Invalid subgraph header: quoted strings cannot be mixed with unquoted text",
                    SourceSpan::new(start, raw_id_end),
                )));
            }
            (authored_id, TitleKind::Text)
        };
        let id_selection = super::text::borrowed_span(authored_id, id_payload);
        let raw_id = id_payload.to_string();
        let mut raw_title = raw_id.clone();
        let mut title_kind = match id_kind {
            TitleKind::String => TitleKind::Text,
            kind => kind,
        };
        let mut id_equals_title = true;

        if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'[' {
            id_equals_title = false;
            self.pos += 1;
            let title_start = self.pos;
            string_state = LabelStringState::default();
            while self.pos < self.input.len() {
                let b = self.input.as_bytes()[self.pos];
                if let Some(consumed) = string_state.consume_delimiter(&self.input[self.pos..]) {
                    self.pos += consumed;
                    continue;
                }
                if string_state.is_plain() && matches!(b, b']' | b'\n' | b'\r') {
                    break;
                }
                self.pos += self.input[self.pos..].chars().next()?.len_utf8();
            }
            let authored_title = &self.input[title_start..self.pos];
            let (payload, kind) = match lex::parse_node_label_payload(authored_title) {
                Ok(parsed) => parsed,
                Err(error) => {
                    return Some(Err(LexError::with_span(
                        error.message,
                        SourceSpan::new(title_start, self.pos),
                    )));
                }
            };
            raw_title = payload.to_string();
            title_kind = kind;
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b']' {
                self.pos += 1;
            }
        }

        Some(Ok((
            start,
            Tok::SubgraphHeader(SubgraphHeader {
                raw_id,
                header_span: Some(SourceSpan::new(keyword_start, self.pos)),
                raw_id_span: Some(SourceSpan::new(
                    start + id_selection.start,
                    start + id_selection.end,
                )),
                raw_title,
                title_kind,
                id_equals_title,
            }),
            self.pos,
        )))
    }

    pub(super) fn lex_amp(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.peek()? != b'&' || !self.after_vertex {
            return None;
        }
        // flow.jison groups nodes with `node spaceList AMP spaceList styledVertex`.
        // In an identifier AMP itself is a legal idStringToken.
        let is_space = |ch| !matches!(ch, '\r' | '\n') && is_ecmascript_trim_char(ch);
        if !self.input[..start]
            .chars()
            .next_back()
            .is_some_and(is_space)
            || !self.input[start + 1..].chars().next().is_some_and(is_space)
        {
            return None;
        }
        self.pos += 1;
        Some((start, Tok::Amp, self.pos))
    }

    fn advance_id(&mut self) {
        let bytes = self.input.as_bytes();
        while let Some(&first) = bytes.get(self.pos) {
            let rest = &bytes[self.pos..];
            if link_prefix_len(rest).is_some() || self.reserved_keyword().is_some() {
                break;
            }

            // Before each idStringToken, Jison gives quote states and single-character
            // tokens priority. A quote inside a running NODE_STRING is literal.
            let len = match first {
                b'"' => break,
                b'0'..=b'9' => rest.iter().take_while(|byte| byte.is_ascii_digit()).count(),
                b',' | b'&' | b'#' | b'*' => 1,
                b':' if !rest.starts_with(b":::") => 1,
                b'v' if self.starts_with_kw("v") => 1,
                b'd' if self.starts_with_kw("default") => "default".len(),
                // MINUS is an idStringToken when the NODE_STRING hyphen lookahead fails.
                b'-' => node_string_len(rest).max(1),
                byte if !byte.is_ascii() => {
                    non_ascii_id_char_len(self.input, self.pos).unwrap_or(0)
                }
                _ => node_string_len(rest),
            };
            if len == 0 {
                break;
            }
            self.pos += len;
        }
    }

    pub(super) fn lex_id(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        self.advance_id();
        if self.pos == start {
            return None;
        }

        let id = self.input[start..self.pos].to_string();
        Some((start, Tok::Id(id), self.pos))
    }

    pub(super) fn lex_arrow_and_label(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        let input = self.input;
        let bytes = input.as_bytes();

        struct LinkEndMatch {
            label_end: usize,
            match_end: usize,
            operator: String,
            operator_span: SourceSpan,
        }

        struct StartLinkMatch {
            family: LinkFamily,
            operator: String,
            #[cfg(feature = "editor-facts")]
            operator_span: SourceSpan,
            match_end: usize,
        }

        let match_link_end = |pos: usize, family: LinkFamily| -> Option<LinkEndMatch> {
            let len = bytes.len();
            let label_end = pos;
            let operator_start = skip_ecmascript_whitespace(input, pos);
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
                    cur += dotted_link_prefix_len(&bytes[cur..])?;
                    if cur < len && matches!(bytes[cur], b'x' | b'o' | b'>') {
                        cur += 1;
                    }
                }
            }

            let operator_end = cur;
            let match_end = skip_ecmascript_whitespace(input, operator_end);
            Some(LinkEndMatch {
                label_end,
                match_end,
                operator: input[operator_start..operator_end].to_string(),
                operator_span: SourceSpan::new(operator_start, operator_end),
            })
        };

        let compute_link =
            |end: String, start: Option<String>| -> std::result::Result<LinkToken, LexError> {
                let end_semantics = if start.is_some() {
                    destruct_labeled_end_link(&end)
                } else {
                    destruct_end_link(&end)
                };
                let mut start_marker = end_semantics.start_marker;

                if let Some(start_str) = start.as_deref() {
                    let start_semantics = destruct_start_link(start_str);
                    if start_semantics.stroke_kind != end_semantics.stroke_kind {
                        return Err(LexError::new(
                            "Invalid link: stroke mismatch between start and end".to_string(),
                        ));
                    }

                    start_marker = start_semantics.marker;
                }

                Ok(LinkToken {
                    end,
                    start_marker,
                    end_marker: end_semantics.end_marker,
                    stroke_kind: end_semantics.stroke_kind,
                    visibility: end_semantics.visibility,
                    length: end_semantics.length,
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
                    recovery_error: None,
                };

                // Optional pipe label: `A--x|label|B` or `A --> |label| B`.
                let pipe_pos = self.pos;
                if pipe_pos < self.input.len() && bytes[pipe_pos] == b'|' {
                    self.pos = pipe_pos + 1;
                    let label_start = self.pos;
                    let label_end = match self.find_label_end(label_start, "|") {
                        Ok(end) => end,
                        Err(error) => return Some(Err(error)),
                    };
                    if let Some(label_end) = label_end {
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
                        let label = labeled_text_with_spans(parsed, token_span, raw_span);
                        self.pending
                            .push_back(Ok((pipe_pos, Tok::EdgeLabel(label), self.pos)));
                    } else {
                        self.pos = self.input.len();
                        let error = {
                            let error = LexError::with_span(
                                "Unterminated flowchart pipe edge label",
                                SourceSpan::new(pipe_pos, self.pos),
                            );
                            #[cfg(feature = "editor-facts")]
                            let error = error.expecting(
                                crate::EditorExpectedSyntaxKind::Payload,
                                SourceSpan::new(self.pos, self.pos),
                            );
                            error
                        };
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
            let operator_start = skip_ecmascript_whitespace(input, pos);
            let marker = usize::from(matches!(
                bytes.get(operator_start),
                Some(b'x' | b'o' | b'<')
            ));
            let family = LinkFamily::from_start(&bytes[operator_start + marker..])?;
            let end = operator_start + marker + 2;
            Some(StartLinkMatch {
                family,
                operator: input[operator_start..end].to_string(),
                #[cfg(feature = "editor-facts")]
                operator_span: SourceSpan::new(operator_start, end),
                match_end: skip_ecmascript_whitespace(input, end),
            })
        };

        let Some(start_match) = parse_start_link(self.pos) else {
            let operator_start = skip_ecmascript_whitespace(self.input, self.pos);
            if self.input[operator_start..].starts_with("->") {
                let label_start = operator_start + 2;
                // At a vertex position, MINUS can be the ID of an odd node: `->label]`.
                // Reuse the odd-label scanner so quoted brackets and cancellation agree.
                if !self.after_vertex || input[start..operator_start].contains('\n') {
                    match self.find_label_end(label_start, "]") {
                        Ok(Some(close)) => {
                            let label = &input[label_start..close];
                            // An unquoted `[` starts a square node, including on a later line.
                            if !label.is_empty() && (label.starts_with('"') || !label.contains('['))
                            {
                                return None;
                            }
                        }
                        Ok(None) => {}
                        Err(error) => return Some(Err(error)),
                    }
                }
                self.pos = label_start;
                let selection = SourceSpan::new(operator_start, self.pos);
                return Some(Err({
                    let error =
                        LexError::with_span("incomplete flowchart edge operator", selection);
                    #[cfg(feature = "editor-facts")]
                    let error = error.expecting(
                        crate::EditorExpectedSyntaxKind::FlowchartOperator,
                        selection,
                    );
                    error
                }));
            }
            return None;
        };
        let family = start_match.family;
        let after_start = start_match.match_end;
        let edge_text_start = after_start;

        let arrow_token =
            |link: LinkToken, _end_span: SourceSpan, recovery_error: Option<LexError>| ArrowToken {
                link,
                recovery_error,
            };

        let mut scan = edge_text_start;
        let mut state = LabelStringState::default();
        while scan < self.input.len() {
            let rest = &self.input[scan..];
            if let Some(consumed) = state.consume_delimiter(rest) {
                scan += consumed;
                continue;
            }
            if state.is_plain() {
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
                            let error = {
                                let error = LexError::with_span(error.message, raw_span);
                                #[cfg(feature = "editor-facts")]
                                let error = error
                                    .expecting(crate::EditorExpectedSyntaxKind::Payload, raw_span);
                                error
                            };
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
                    let link =
                        match compute_link(link_match.operator, Some(start_match.operator.clone()))
                        {
                            Ok(v) => v,
                            Err(e) => return Some(Err(e)),
                        };
                    let arrow = arrow_token(link, link_match.operator_span, None);

                    let label = labeled_text_with_spans(
                        parsed,
                        SourceSpan::new(edge_text_start, match_start),
                        raw_span,
                    );
                    self.pending
                        .push_back(Ok((edge_text_start, Tok::EdgeLabel(label), match_end)));
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
            let ch = self.input[scan..]
                .chars()
                .next()
                .expect("edge label scan position must contain a character");
            scan += ch.len_utf8();
        }

        self.pos = self.input.len();
        Some(Err({
            let error = LexError::with_span(
                "Unterminated edge label (missing link terminator)",
                SourceSpan::new(edge_text_start, self.pos),
            );
            #[cfg(feature = "editor-facts")]
            let error = error.expecting(
                crate::EditorExpectedSyntaxKind::FlowchartOperator,
                SourceSpan::new(start_match.operator_span.start, after_start),
            );
            error
        }))
    }

    pub(super) fn lex_node_label(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        let input = self.input;
        let rest = &input[self.pos..];

        if rest.starts_with("[\\") {
            let open = "[\\";
            let content_start = self.pos + open.len();
            let end_slash = match self.find_label_end(content_start, "/]") {
                Ok(end) => end,
                Err(error) => return Some(Err(error)),
            };
            let end_backslash = match self.find_label_end(content_start, "\\]") {
                Ok(end) => end,
                Err(error) => return Some(Err(error)),
            };

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
                                #[cfg(feature = "editor-facts")]
                                trigger_span: Some(SourceSpan::new(start, content_start)),
                                #[cfg(any(test, feature = "editor-facts"))]
                                error: LexError::with_span(
                                    "Unterminated node label (missing `/]` or `\\]`)",
                                    SourceSpan::new(start, token_end),
                                ),
                            },
                        );
                        self.pos = token_end;
                        return Some(Ok((start, token, self.pos)));
                    }
                    let token_end = self.stmt_end_from(content_start);
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
                #[cfg(feature = "editor-facts")]
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
            let end_slash = match self.find_label_end(content_start, "/]") {
                Ok(end) => end,
                Err(error) => return Some(Err(error)),
            };
            let end_backslash = match self.find_label_end(content_start, "\\]") {
                Ok(end) => end,
                Err(error) => return Some(Err(error)),
            };

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
                                #[cfg(feature = "editor-facts")]
                                trigger_span: Some(SourceSpan::new(start, content_start)),
                                #[cfg(any(test, feature = "editor-facts"))]
                                error: LexError::with_span(
                                    "Unterminated node label (missing `/]` or `\\]`)",
                                    SourceSpan::new(start, token_end),
                                ),
                            },
                        );
                        self.pos = token_end;
                        return Some(Ok((start, token, self.pos)));
                    }
                    let token_end = self.stmt_end_from(content_start);
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
                #[cfg(feature = "editor-facts")]
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
            let end = match self.find_label_end(content_start, close) {
                Ok(end) => end,
                Err(error) => return Some(Err(error)),
            };
            let token = if let Some(end_start) = end {
                let token_end = end_start + close.len();
                let token = match build_node_label_token(
                    self.input,
                    shape,
                    SourceSpan::new(start, token_end),
                    SourceSpan::new(content_start, end_start),
                    #[cfg(feature = "editor-facts")]
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
                    let token_end = self.stmt_end_from(content_start);
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
                        #[cfg(feature = "editor-facts")]
                        trigger_span: Some(SourceSpan::new(start, content_start)),
                        #[cfg(any(test, feature = "editor-facts"))]
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
            let end = match self.find_label_end(content_start, "]") {
                Ok(end) => end,
                Err(error) => return Some(Err(error)),
            };
            let token = if let Some(end_start) = end {
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
                    #[cfg(feature = "editor-facts")]
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
                    let token_end = self.stmt_end_from(content_start);
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
                        #[cfg(feature = "editor-facts")]
                        trigger_span: Some(SourceSpan::new(start, content_start)),
                        #[cfg(any(test, feature = "editor-facts"))]
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
            let end = match self.find_label_end(content_start, "}") {
                Ok(end) => end,
                Err(error) => return Some(Err(error)),
            };
            let token = if let Some(end_start) = end {
                let token_end = end_start + 1;
                let token = match build_node_label_token(
                    self.input,
                    "diamond",
                    SourceSpan::new(start, token_end),
                    SourceSpan::new(content_start, end_start),
                    #[cfg(feature = "editor-facts")]
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
                    let token_end = self.stmt_end_from(content_start);
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
                        #[cfg(feature = "editor-facts")]
                        trigger_span: Some(SourceSpan::new(start, content_start)),
                        #[cfg(any(test, feature = "editor-facts"))]
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
            let end = match self.find_label_end(content_start, ")") {
                Ok(end) => end,
                Err(error) => return Some(Err(error)),
            };
            let token = if let Some(end_start) = end {
                let token_end = end_start + 1;
                let token = match build_node_label_token(
                    self.input,
                    "round",
                    SourceSpan::new(start, token_end),
                    SourceSpan::new(content_start, end_start),
                    #[cfg(feature = "editor-facts")]
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
                    let token_end = self.stmt_end_from(content_start);
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
                        #[cfg(feature = "editor-facts")]
                        trigger_span: Some(SourceSpan::new(start, content_start)),
                        #[cfg(any(test, feature = "editor-facts"))]
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
    #[cfg(feature = "editor-facts")] trigger_span: Option<SourceSpan>,
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
        #[cfg(feature = "editor-facts")]
        trigger_span,
    )
}

fn build_node_label_token_from_raw(
    _input: &str,
    shape: &str,
    token_span: SourceSpan,
    _content_span: SourceSpan,
    raw: &str,
    raw_span: SourceSpan,
    #[cfg(feature = "editor-facts")] trigger_span: Option<SourceSpan>,
) -> std::result::Result<Tok, LexError> {
    let text = lex::parse_node_label_text(raw).map_err(|mut error| {
        error.span = error.span.or(Some(raw_span));
        error
    })?;
    Ok(Tok::NodeLabel(NodeLabelToken {
        shape: shape.to_string(),
        text: labeled_text_with_spans(text, token_span, raw_span),
        #[cfg(feature = "editor-facts")]
        trigger_span,
        #[cfg(any(test, feature = "editor-facts"))]
        recovery_error: None,
    }))
}

struct PartialNodeLabelRecovery {
    #[cfg(feature = "editor-facts")]
    trigger_span: Option<SourceSpan>,
    #[cfg(any(test, feature = "editor-facts"))]
    error: LexError,
}

fn build_partial_node_label_token_from_raw(
    _input: &str,
    shape: &str,
    token_span: SourceSpan,
    _content_span: SourceSpan,
    raw: &str,
    raw_span: SourceSpan,
    _recovery: PartialNodeLabelRecovery,
) -> Tok {
    let text = lex::parse_node_label_text(raw).unwrap_or_else(|_| {
        let (text, kind) = parse_label_text(raw);
        LabeledText {
            text,
            kind,
            span: None,
            selection: None,
        }
    });
    Tok::NodeLabel(NodeLabelToken {
        shape: shape.to_string(),
        text: labeled_text_with_spans(text, token_span, raw_span),
        #[cfg(feature = "editor-facts")]
        trigger_span: _recovery.trigger_span,
        #[cfg(any(test, feature = "editor-facts"))]
        recovery_error: Some(_recovery.error),
    })
}

fn labeled_text_with_spans(
    mut text: LabeledText,
    token_span: SourceSpan,
    content_span: SourceSpan,
) -> LabeledText {
    text.span = Some(token_span);
    text.selection = Some(text.selection.map_or(content_span, |relative| {
        SourceSpan::new(
            content_span.start + relative.start,
            content_span.start + relative.end,
        )
    }));
    text
}

#[cfg(all(test, feature = "diagram-flowchart"))]
mod tests {
    use super::*;
    use crate::{SemanticDiagramModel, SemanticParser};

    fn assert_linear_label_scan_work(lexer: &Lexer<'_>, source: &str) {
        assert!(
            lexer.label_scan_work() <= 4 * source.len(),
            "ordinary label scans must be linear: {} steps for {} bytes",
            lexer.label_scan_work(),
            source.len(),
        );
    }

    fn assert_repeated_slanted_label_scan_work(open: &str, close: &str, shape: &str, suffix: &str) {
        let parser = SemanticParser::new();
        let header = "flowchart TD\n";
        let statement = format!("A{open}x{close}\n");
        for repetitions in [64, 128, 256] {
            let source = format!("{header}{}{suffix}", statement.repeat(repetitions));
            let mut lexer = Lexer::new(&source);
            let tokens = lexer
                .by_ref()
                .collect::<Result<Vec<_>, _>>()
                .expect("repeated slanted labels must tokenize");
            let labels = tokens
                .iter()
                .filter_map(|(start, token, end)| match token {
                    Tok::NodeLabel(label) => Some((*start, label, *end)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(labels.len(), repetitions + usize::from(!suffix.is_empty()));
            for (index, (start, label, end)) in labels[..repetitions].iter().enumerate() {
                let expected_start = header.len() + index * statement.len() + 1;
                let expected_end = header.len() + (index + 1) * statement.len() - 1;
                assert_eq!((*start, *end), (expected_start, expected_end));
                assert_eq!(label.shape, shape);
                assert_eq!(label.text.text, "x");
                assert_eq!(label.text.kind, TitleKind::Text);
                assert_eq!(label.text.span, Some(SourceSpan::new(*start, *end)));
                assert_eq!(
                    label.text.selection,
                    Some(SourceSpan::new(
                        *start + open.len(),
                        *start + open.len() + 1
                    )),
                );
                assert!(label.recovery_error.is_none());
            }
            let parsed = parser
                .parse_diagram_sync(&source)
                .expect("repeated definitions of the same node must parse");
            let SemanticDiagramModel::Flowchart(model) = parsed.model else {
                panic!("expected Flowchart model");
            };
            assert_eq!(model.nodes.len(), 1 + usize::from(!suffix.is_empty()));
            assert_eq!(model.nodes[0].id, "A");
            assert_eq!(model.nodes[0].label.as_deref(), Some("x"));
            assert_linear_label_scan_work(&lexer, &source);
        }
    }

    #[test]
    fn ordinary_label_scans_are_linear_with_slash_closers() {
        for (open, shape) in [("[/", "lean_right"), ("[\\", "inv_trapezoid")] {
            assert_repeated_slanted_label_scan_work(open, "/]", shape, "");
        }
    }

    #[test]
    fn ordinary_label_scans_are_linear_with_backslash_closers() {
        for (open, shape) in [("[\\", "lean_left"), ("[/", "trapezoid")] {
            assert_repeated_slanted_label_scan_work(open, "\\]", shape, "");
        }
    }

    #[test]
    fn ordinary_label_scans_are_linear_with_distant_alternate_closers() {
        for (open, close, shape, suffix) in [
            ("[/", "/]", "lean_right", "B[\\y\\]\n"),
            ("[\\", "\\]", "lean_left", "B[/y/]\n"),
        ] {
            assert_repeated_slanted_label_scan_work(open, close, shape, suffix);
        }
    }

    #[test]
    fn ordinary_label_scans_preserve_multiline_utf8_and_quoted_closers() {
        let header = "flowchart TD\r\n";
        for (raw, payload, kind, payload_offset) in [
            (
                "first 雪😀\r\nlast",
                "first 雪😀\r\nlast",
                TitleKind::Text,
                0,
            ),
            (
                "\"first 雪😀 /] \\]\r\nlast\"",
                "first 雪😀 /] \\]\r\nlast",
                TitleKind::String,
                1,
            ),
            (
                "\"`first 雪😀 /] \\]\r\nlast`\"",
                "first 雪😀 /] \\]\r\nlast",
                TitleKind::Markdown,
                2,
            ),
        ] {
            for (open, close, shape, suffix) in [
                ("[/", "/]", "lean_right", "B[\\tail\\]\r\n"),
                ("[\\", "/]", "inv_trapezoid", "B[\\tail\\]\r\n"),
                ("[\\", "\\]", "lean_left", "B[/tail/]\r\n"),
                ("[/", "\\]", "trapezoid", "B[/tail/]\r\n"),
            ] {
                let statement = format!("A{open}{raw}{close}\r\n");
                let source = format!("{header}{}{suffix}", statement.repeat(64));
                let mut lexer = Lexer::new(&source);
                let tokens = lexer
                    .by_ref()
                    .collect::<Result<Vec<_>, _>>()
                    .expect("multiline labels must tokenize");
                let labels = tokens
                    .iter()
                    .filter_map(|(start, token, end)| match token {
                        Tok::NodeLabel(label) => Some((*start, label, *end)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(labels.len(), 65);
                for (index, (start, label, end)) in labels[..64].iter().enumerate() {
                    assert_eq!(*start, header.len() + index * statement.len() + 1);
                    assert_eq!(*end, header.len() + (index + 1) * statement.len() - 2);
                    assert_eq!(label.shape, shape);
                    assert_eq!(label.text.text, payload);
                    assert_eq!(label.text.kind, kind);
                    assert_eq!(label.text.span, Some(SourceSpan::new(*start, *end)));
                    let selection_start = *start + open.len() + payload_offset;
                    let selection =
                        SourceSpan::new(selection_start, selection_start + payload.len());
                    assert_eq!(label.text.selection, Some(selection));
                    assert_eq!(&source[selection.start..selection.end], payload);
                    assert!(label.recovery_error.is_none());
                }
                let parsed = SemanticParser::new()
                    .parse_diagram_sync(&source)
                    .expect("quoted closers must stay inside the multiline payload");
                let SemanticDiagramModel::Flowchart(model) = parsed.model else {
                    panic!("expected Flowchart model");
                };
                assert_eq!(model.nodes.len(), 2);
                assert_eq!(model.nodes[0].label, Some(payload.replace("\r\n", "\n")));
                assert_linear_label_scan_work(&lexer, &source);
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn ordinary_label_scans_observe_cancellation_before_materializing_plain_payloads() {
        for (open, close) in [("[/", "/]"), ("[\\", "\\]"), ("[", "]")] {
            for payload in ["x".repeat(32768), "雪😀x".repeat(8192)] {
                let source = format!("flowchart TD\nA{open}{payload}{close}\n");
                for recover in [false, true] {
                    let control = OperationControl::new().for_phase(crate::OperationPhase::Parse);
                    control.cancel_after_checkpoints(0);
                    let mut lexer = if recover {
                        Lexer::recovering(&source)
                    } else {
                        Lexer::new(&source)
                    }
                    .with_control(&control);
                    let tokens = lexer.by_ref().collect::<Vec<_>>();
                    assert!(tokens.iter().any(|item| item.is_err()));
                    assert!(
                        tokens
                            .iter()
                            .all(|item| !matches!(item, Ok((_, Tok::NodeLabel(_), _))))
                    );
                    assert!(control.is_cancelled());
                    assert_eq!(lexer.label_scan_work(), 4096);
                    assert!(lexer.next().is_none());
                }
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    fn assert_unterminated_node_label_scan_work(recover: bool) {
        let header = "flowchart TD\n";
        for (open, shape) in [
            ("[/", "lean_right"),
            ("[\\", "inv_trapezoid"),
            ("[", "square"),
            ("(", "round"),
            ("{", "diamond"),
            ("[[", "subroutine"),
            ("{{", "hexagon"),
            ("((", "circle"),
            ("(((", "doublecircle"),
            ("(-", "ellipse"),
            ("([", "stadium"),
            ("[(", "cylinder"),
            (">", "odd"),
        ] {
            let statement = format!("A{open}x\n");
            for repetitions in [64, 128, 256] {
                let source = format!("{header}{}B --> C\n", statement.repeat(repetitions));
                let mut lexer = if recover {
                    Lexer::recovering(&source)
                } else {
                    Lexer::new(&source)
                };
                let mut errors = Vec::new();
                let mut labels = Vec::new();
                let mut ids = Vec::new();
                for item in lexer.by_ref() {
                    match item {
                        Ok((_, Tok::Id(id), _)) => ids.push(id),
                        Ok((start, Tok::NodeLabel(label), end)) => {
                            assert!(recover);
                            assert_eq!(label.shape, shape);
                            assert_eq!(label.text.text, "x");
                            assert_eq!(label.text.kind, TitleKind::Text);
                            assert_eq!(label.text.span, Some(SourceSpan::new(start, end)));
                            assert_eq!(
                                label.text.selection,
                                Some(SourceSpan::new(start + open.len(), end)),
                            );
                            errors.push(label.recovery_error.expect("incomplete label error"));
                            labels.push((start, end));
                        }
                        Err(error) => {
                            assert!(!recover, "recovery must emit a partial node label: {error}");
                            errors.push(error);
                        }
                        _ => {}
                    }
                }
                assert_eq!(errors.len(), repetitions);
                assert_eq!(labels.len(), if recover { repetitions } else { 0 });
                assert_eq!(ids.len(), repetitions + 2);
                assert!(ids[..repetitions].iter().all(|id| id == "A"));
                assert_eq!(&ids[repetitions..], &["B", "C"]);
                for (index, error) in errors.iter().enumerate() {
                    let start = header.len() + index * statement.len() + 1;
                    let end = header.len() + (index + 1) * statement.len() - 1;
                    assert!(error.message.starts_with("Unterminated node label"));
                    assert_eq!(error.span, Some(SourceSpan::new(start, end)));
                    if recover {
                        assert_eq!(labels[index], (start, end));
                    }
                }
                assert!(SemanticParser::new().parse_diagram_sync(&source).is_err());
                assert_linear_label_scan_work(&lexer, &source);
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn ordinary_label_scans_are_linear_after_strict_unterminated_labels() {
        assert_unterminated_node_label_scan_work(false);
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn ordinary_label_scans_are_linear_after_recovered_unterminated_labels() {
        assert_unterminated_node_label_scan_work(true);
    }
}
