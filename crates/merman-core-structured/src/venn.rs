//! Venn tokenization, statement recovery, and semantic model construction.

use merman_model::families::venn::{
    VennDiagramModel, VennStyleEntryModel, VennSubsetModel, VennTextNodeModel,
};
use merman_model::text::is_ecmascript_whitespace;
use merman_model::{OperationControl, OperationControlResult, SourceSpan};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};

type StatementResult<T> = Result<T, Cow<'static, str>>;

/// The model accumulated before and after recoverable statement errors.
pub struct VennParseOutcome {
    pub model: VennDiagramModel,
    pub first_issue: Option<VennSourceIssue>,
}

#[derive(Debug)]
pub struct VennSourceIssue {
    pub kind: VennIssueKind,
    pub span: SourceSpan,
}

#[derive(Debug)]
pub enum VennIssueKind {
    InvalidHeader,
    MissingHeader,
    Statement(Cow<'static, str>),
}

impl VennIssueKind {
    pub fn message(&self) -> &str {
        match self {
            Self::InvalidHeader => "expected venn-beta header",
            Self::MissingHeader => "expected venn-beta",
            Self::Statement(message) => message,
        }
    }
}

#[cfg(feature = "editor-facts")]
#[derive(Clone, Copy)]
pub enum VennFieldKind {
    Header,
    Title,
    Set,
    SetLabel,
    Size,
    UnionSet,
    UnionLabel,
    TextSet,
    TextNode,
    TextLabel,
    StyleTarget,
    StyleValue,
}

#[cfg(feature = "editor-facts")]
pub enum VennObservation<'a> {
    Directive(&'static str),
    IdList(SourceSpan),
    Field {
        kind: VennFieldKind,
        text: &'a str,
        span: SourceSpan,
        selection: SourceSpan,
    },
    Recovery(&'a VennSourceIssue),
}

#[cfg(feature = "editor-facts")]
struct VennObserver<'a> {
    capture_fields: bool,
    observe: Option<&'a mut dyn FnMut(VennObservation<'_>)>,
}

#[cfg(feature = "editor-facts")]
impl<'a> VennObserver<'a> {
    fn rich(&mut self) -> Option<&mut (dyn FnMut(VennObservation<'_>) + 'a)> {
        if self.capture_fields {
            self.observe.as_deref_mut()
        } else {
            None
        }
    }

    fn recovery(&mut self, issue: &VennSourceIssue) {
        if let Some(observe) = self.observe.as_deref_mut() {
            observe(VennObservation::Recovery(issue));
        }
    }
}

#[derive(Default)]
struct VennSemanticState {
    model: VennDiagramModel,
    known_sets: HashSet<String>,
    current_sets: Option<Vec<String>>,
}

impl VennSemanticState {
    fn add_subset(&mut self, identifiers: Vec<String>, label: Option<String>, size: Option<f64>) {
        let mut sets = normalize_identifier_list(identifiers);
        let resolved_size = size.unwrap_or_else(|| 10.0 / (sets.len() as f64).powi(2));
        self.current_sets = Some(sets.clone());

        if sets.len() == 1 {
            self.known_sets.insert(sets[0].clone());
        }

        self.model.subsets.push(VennSubsetModel {
            sets: std::mem::take(&mut sets),
            size: resolved_size,
            label: label
                .map(|value| normalize_text(&value))
                .filter(|value| !value.is_empty()),
        });
    }

    fn validate_union_identifiers(&self, identifiers: &[String]) -> StatementResult<()> {
        let unknown = identifiers
            .iter()
            .map(|identifier| normalize_text(identifier))
            .filter(|identifier| !self.known_sets.contains(identifier))
            .collect::<Vec<_>>();

        if unknown.is_empty() {
            Ok(())
        } else {
            Err(format!("unknown set identifier: {}", unknown.join(", ")).into())
        }
    }

    fn add_text(&mut self, identifiers: Vec<String>, id: String, label: Option<String>) {
        self.model.text_nodes.push(VennTextNodeModel {
            sets: normalize_identifier_list(identifiers),
            id: normalize_text(&id),
            label: label
                .map(|value| normalize_text(&value))
                .filter(|value| !value.is_empty()),
        });
    }

    fn add_style(&mut self, targets: Vec<String>, styles: BTreeMap<String, String>) {
        self.model.style_entries.push(VennStyleEntryModel {
            targets: normalize_identifier_list(targets),
            styles,
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextIdKind {
    IdentifierOrString,
    Numeric,
}

#[derive(Debug, Clone)]
struct VennFieldSpan {
    text: String,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
    #[cfg(feature = "editor-facts")]
    selection: SourceSpan,
}

#[cfg(feature = "editor-facts")]
impl VennFieldSpan {
    fn observation(&self, kind: VennFieldKind) -> VennObservation<'_> {
        VennObservation::Field {
            kind,
            text: &self.text,
            span: self.span,
            selection: self.selection,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VennTokenKind {
    Header,
    Title,
    Set,
    Union,
    Text,
    IndentedText,
    Style,
    Identifier,
    String,
    BracketLabel,
    Numeric,
    Color,
    Comma,
    Colon,
    Newline,
    End,
    Invalid,
}

impl VennTokenKind {
    fn starts_statement(self) -> bool {
        matches!(
            self,
            Self::Title | Self::Set | Self::Union | Self::Text | Self::IndentedText | Self::Style
        )
    }

    fn ends_statement(self) -> bool {
        self.starts_statement() || matches!(self, Self::Newline | Self::End)
    }

    fn is_style_value(self) -> bool {
        matches!(self, Self::Identifier | Self::Numeric | Self::Color)
    }
}

#[derive(Debug, Clone, Copy)]
struct VennToken {
    kind: VennTokenKind,
    span: SourceSpan,
}

struct VennCursor<'a> {
    input: &'a str,
    pos: usize,
    consumed_end: usize,
    lookahead: Option<VennToken>,
    beginning_of_line: bool,
    indent_mode: bool,
    next_quote: usize,
    next_label_delimiter: usize,
    #[cfg(test)]
    inspected_terminal_bytes: usize,
}

impl<'a> VennCursor<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            consumed_end: 0,
            lookahead: None,
            beginning_of_line: false,
            indent_mode: false,
            next_quote: 0,
            next_label_delimiter: 0,
            #[cfg(test)]
            inspected_terminal_bytes: 0,
        }
    }

    fn peek(&mut self) -> VennToken {
        if let Some(token) = self.lookahead {
            return token;
        }
        let token = self.lex();
        self.lookahead = Some(token);
        token
    }

    fn take(&mut self) -> VennToken {
        let token = self.peek();
        self.lookahead = None;
        self.consumed_end = token.span.end;
        token
    }

    fn token(&mut self, kind: VennTokenKind, start: usize, len: usize) -> VennToken {
        self.pos = start + len;
        VennToken {
            kind,
            span: SourceSpan::new(start, self.pos),
        }
    }

    fn lex(&mut self) -> VennToken {
        // Match venn.jison's rule order and bol state. Payload tokens are consumed whole
        // before any following statement is considered; keywords use JavaScript's ASCII \b.
        loop {
            let start = self.pos;
            let input = self.input;
            let rest = &input[start..];
            let Some(first) = rest.chars().next() else {
                return self.token(VennTokenKind::End, start, 0);
            };
            let after_first = &rest[first.len_utf8()..];
            if rest.starts_with("%%") {
                self.pos += rest.find('\n').unwrap_or(rest.len());
                continue;
            }
            if first != '}' && first.len_utf16() == 1 && after_first.starts_with("%%") {
                self.pos += first.len_utf8() + after_first.find('\n').unwrap_or(after_first.len());
                continue;
            }
            if matches!(first, ' ' | '\t') {
                let len = rest
                    .bytes()
                    .take_while(|byte| matches!(byte, b' ' | b'\t'))
                    .count();
                self.pos += len;
                let after = &self.input[self.pos..];
                if self.beginning_of_line && self.indent_mode && keyword_prefix(after, "text") {
                    self.beginning_of_line = false;
                    return self.token(VennTokenKind::IndentedText, self.pos, "text".len());
                }
                continue;
            }
            if matches!(first, '\n' | '\r') {
                let len = rest
                    .bytes()
                    .take_while(|byte| matches!(byte, b'\n' | b'\r'))
                    .count();
                self.beginning_of_line = true;
                return self.token(VennTokenKind::Newline, start, len);
            }
            if self.beginning_of_line {
                self.indent_mode = false;
                self.beginning_of_line = false;
            }

            if let Some(len) = title_token_len(rest) {
                return self.token(VennTokenKind::Title, start, len);
            }
            for (keyword, kind) in [
                ("venn-beta", VennTokenKind::Header),
                ("set", VennTokenKind::Set),
                ("union", VennTokenKind::Union),
                ("text", VennTokenKind::Text),
                ("style", VennTokenKind::Style),
            ] {
                if keyword_prefix(rest, keyword) {
                    return self.token(kind, start, keyword.len());
                }
            }
            if let Some(len) = self.bracket_label_token_len(start) {
                return self.token(VennTokenKind::BracketLabel, start, len);
            }
            if let Some((value, _)) = parse_numeric_token(rest) {
                return self.token(VennTokenKind::Numeric, start, value.len());
            }
            if let Some((value, _)) =
                parse_hex_color_token(rest).or_else(|| parse_rgb_like_token(rest))
            {
                return self.token(VennTokenKind::Color, start, value.len());
            }
            if let Some((value, _)) = parse_bare_identifier_token(rest) {
                return self.token(VennTokenKind::Identifier, start, value.len());
            }
            if first == '"'
                && let Some(end) = self.payload_delimiter(start + 1, true)
            {
                return self.token(VennTokenKind::String, start, end + 1 - start);
            }
            let kind = match first {
                ',' => VennTokenKind::Comma,
                ':' => VennTokenKind::Colon,
                _ => VennTokenKind::Invalid,
            };
            return self.token(kind, start, first.len_utf8());
        }
    }

    fn payload_delimiter(&mut self, start: usize, quoted: bool) -> Option<usize> {
        let cached = if quoted {
            &mut self.next_quote
        } else {
            &mut self.next_label_delimiter
        };
        // Lexing advances monotonically. Cache both the next delimiter and a missing
        // delimiter (input.len()) so malformed nested openers cannot rescan a suffix.
        if *cached < start {
            let rest = &self.input[start..];
            let distance = if quoted {
                rest.find('"')
            } else {
                rest.find([']', '"'])
            };
            #[cfg(test)]
            {
                self.inspected_terminal_bytes +=
                    distance.map_or(rest.len(), |distance| distance + 1);
            }
            *cached = distance.map_or(self.input.len(), |distance| start + distance);
        }
        (*cached < self.input.len()).then_some(*cached)
    }

    fn bracket_label_token_len(&mut self, start: usize) -> Option<usize> {
        if self.input.as_bytes()[start] != b'[' {
            return None;
        }
        if self.input.as_bytes().get(start + 1) == Some(&b'"') {
            let end = self.payload_delimiter(start + 2, true)?;
            return (self.input.as_bytes().get(end + 1) == Some(&b']')).then_some(end + 2 - start);
        }
        let end = self.payload_delimiter(start + 1, false)?;
        (end > start + 1 && self.input.as_bytes()[end] == b']').then_some(end + 1 - start)
    }

    fn field(&self, token: VennToken) -> VennFieldSpan {
        let raw = &self.input[token.span.start..token.span.end];
        let (text, leading, trailing) = match token.kind {
            VennTokenKind::String => (&raw[1..raw.len() - 1], 1, 1),
            VennTokenKind::BracketLabel if raw.starts_with("[\"") => (&raw[2..raw.len() - 2], 2, 2),
            VennTokenKind::BracketLabel => {
                let inner = &raw[1..raw.len() - 1];
                (
                    inner.trim(),
                    1 + inner.len() - inner.trim_start().len(),
                    1 + inner.len() - inner.trim_end().len(),
                )
            }
            _ => (raw, 0, 0),
        };
        #[cfg(not(feature = "editor-facts"))]
        let _ = (leading, trailing);
        VennFieldSpan {
            text: text.to_string(),
            #[cfg(feature = "editor-facts")]
            span: token.span,
            #[cfg(feature = "editor-facts")]
            selection: SourceSpan::new(token.span.start + leading, token.span.end - trailing),
        }
    }

    fn take_identifier_like(&mut self) -> StatementResult<VennFieldSpan> {
        if !matches!(
            self.peek().kind,
            VennTokenKind::Identifier | VennTokenKind::String
        ) {
            return Err("expected identifier".into());
        }
        let token = self.take();
        Ok(self.field(token))
    }

    fn take_bare_identifier(&mut self) -> StatementResult<VennFieldSpan> {
        if self.peek().kind != VennTokenKind::Identifier {
            return Err("expected identifier".into());
        }
        let token = self.take();
        Ok(self.field(token))
    }

    fn take_text_id(&mut self) -> StatementResult<(VennFieldSpan, TextIdKind)> {
        if self.peek().kind == VennTokenKind::Numeric {
            let token = self.take();
            return Ok((self.field(token), TextIdKind::Numeric));
        }
        let token = self.take_identifier_like()?;
        Ok((token, TextIdKind::IdentifierOrString))
    }

    fn take_optional_bracket_label(&mut self) -> StatementResult<Option<VennFieldSpan>> {
        let token = self.peek();
        if token.kind == VennTokenKind::BracketLabel {
            self.take();
            return Ok(Some(self.field(token)));
        }
        if self.input[token.span.start..].starts_with('[') {
            return Err("unterminated bracket label".into());
        }
        Ok(None)
    }

    fn take_optional_size(&mut self) -> StatementResult<Option<VennFieldSpan>> {
        if !self.take_delimiter(':') {
            return Ok(None);
        }
        if self.peek().kind != VennTokenKind::Numeric {
            return Err("expected numeric".into());
        }
        let token = self.take();
        Ok(Some(self.field(token)))
    }

    fn take_style_value(&mut self) -> StatementResult<VennFieldSpan> {
        if self.peek().kind == VennTokenKind::String {
            let token = self.take();
            return Ok(self.field(token));
        }
        if !self.peek().kind.is_style_value() {
            return Err("expected style value".into());
        }
        let token = self.take();
        let mut value = self.field(token);
        while self.peek().kind.is_style_value() {
            let token = self.take();
            value.text.push(' ');
            value
                .text
                .push_str(&self.input[token.span.start..token.span.end]);
            #[cfg(feature = "editor-facts")]
            {
                value.span.end = token.span.end;
                value.selection.end = token.span.end;
            }
        }
        if self.peek().kind == VennTokenKind::Invalid {
            return Err("expected style value".into());
        }
        Ok(value)
    }

    fn expect_end(&mut self) -> StatementResult<()> {
        let token = self.peek();
        if token.kind.ends_statement() {
            Ok(())
        } else {
            Err(format!(
                "unexpected trailing venn tokens: {}",
                &self.input[token.span.start..token.span.end]
            )
            .into())
        }
    }

    fn take_delimiter(&mut self, delimiter: char) -> bool {
        let kind = match delimiter {
            ',' => VennTokenKind::Comma,
            ':' => VennTokenKind::Colon,
            _ => return false,
        };
        if self.peek().kind == kind {
            self.take();
            true
        } else {
            false
        }
    }

    fn recover_statement(&mut self, start: usize) -> SourceSpan {
        while !self.peek().kind.ends_statement() {
            self.take();
        }
        SourceSpan::new(start, self.consumed_end.max(start))
    }
}

/// Scans one preprocessed source after the caller's entry checkpoint.
pub fn parse_venn_source_controlled(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<VennParseOutcome> {
    parse_venn_source_impl(
        code,
        #[cfg(feature = "editor-facts")]
        false,
        #[cfg(feature = "editor-facts")]
        None,
        control,
    )
}

/// Scans one preprocessed source and reports editor observations as it parses.
///
/// Every span is relative to `code`. Recovery is always reported, and `capture_fields`
/// also enables directive and field observations.
#[cfg(feature = "editor-facts")]
pub fn parse_venn_source_with_observer_controlled(
    code: &str,
    capture_fields: bool,
    observe: &mut dyn FnMut(VennObservation<'_>),
    control: &OperationControl,
) -> OperationControlResult<VennParseOutcome> {
    parse_venn_source_impl(code, capture_fields, Some(observe), control)
}

fn parse_venn_source_impl(
    code: &str,
    #[cfg(feature = "editor-facts")] capture_fields: bool,
    #[cfg(feature = "editor-facts")] observe: Option<&mut dyn FnMut(VennObservation<'_>)>,
    control: &OperationControl,
) -> OperationControlResult<VennParseOutcome> {
    let mut state = VennSemanticState::default();
    #[cfg(feature = "editor-facts")]
    let mut observer = VennObserver {
        capture_fields,
        observe,
    };
    let mut header_decided = false;
    let mut first_issue = None;
    let mut cursor = VennCursor::new(code);

    loop {
        control.checkpoint()?;
        let token = cursor.peek();
        if token.kind == VennTokenKind::End {
            break;
        }
        if token.kind == VennTokenKind::Newline {
            cursor.take();
            continue;
        }

        if !header_decided {
            header_decided = true;
            cursor.take();
            if token.kind != VennTokenKind::Header {
                let span = cursor.recover_statement(token.span.start);
                recover_venn_error(
                    &mut state,
                    &mut first_issue,
                    VennIssueKind::InvalidHeader,
                    span,
                    #[cfg(feature = "editor-facts")]
                    &mut observer,
                );
                cursor.indent_mode = false;
                continue;
            }

            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observer.rich() {
                observe(VennObservation::Field {
                    kind: VennFieldKind::Header,
                    text: "venn-beta",
                    span: token.span,
                    selection: token.span,
                });
            }

            continue;
        }

        if let Err(message) = parse_venn_statement(
            &mut cursor,
            &mut state,
            #[cfg(feature = "editor-facts")]
            &mut observer,
        ) {
            let span = cursor.recover_statement(token.span.start);
            recover_venn_error(
                &mut state,
                &mut first_issue,
                VennIssueKind::Statement(message),
                span,
                #[cfg(feature = "editor-facts")]
                &mut observer,
            );
            cursor.indent_mode = false;
        }
    }

    if !header_decided {
        recover_venn_error(
            &mut state,
            &mut first_issue,
            VennIssueKind::MissingHeader,
            SourceSpan::new(0, 0),
            #[cfg(feature = "editor-facts")]
            &mut observer,
        );
    }

    control.checkpoint()?;
    Ok(VennParseOutcome {
        model: state.model,
        first_issue,
    })
}

fn recover_venn_error(
    state: &mut VennSemanticState,
    first_issue: &mut Option<VennSourceIssue>,
    kind: VennIssueKind,
    span: SourceSpan,
    #[cfg(feature = "editor-facts")] observer: &mut VennObserver<'_>,
) {
    let issue = VennSourceIssue { kind, span };
    let issue = if first_issue.is_none() {
        &*first_issue.insert(issue)
    } else {
        &issue
    };
    #[cfg(feature = "editor-facts")]
    observer.recovery(issue);
    #[cfg(not(feature = "editor-facts"))]
    let _ = issue;
    state.current_sets = None;
}

fn parse_venn_statement(
    cursor: &mut VennCursor<'_>,
    state: &mut VennSemanticState,
    #[cfg(feature = "editor-facts")] observer: &mut VennObserver<'_>,
) -> StatementResult<()> {
    let token = cursor.take();
    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observer.rich() {
        let prefix = match token.kind {
            VennTokenKind::Title => Some("title"),
            VennTokenKind::Set => Some("set"),
            VennTokenKind::Union => Some("union"),
            VennTokenKind::Text | VennTokenKind::IndentedText => Some("text"),
            VennTokenKind::Style => Some("style"),
            _ => None,
        };
        if let Some(prefix) = prefix {
            observe(VennObservation::Directive(prefix));
        }
    }
    match token.kind {
        VennTokenKind::Title => {
            cursor
                .expect_end()
                .map_err(|_| Cow::Borrowed("invalid Venn title syntax"))?;
            let raw = &cursor.input[token.span.start + "title".len()..token.span.end];
            let trimmed = raw.trim();
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observer.rich() {
                let start = token.span.start + "title".len() + raw.len() - raw.trim_start().len();
                let end = start + trimmed.len();
                let quoted =
                    trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2;
                let text = normalize_text(trimmed);
                observe(VennObservation::Field {
                    kind: VennFieldKind::Title,
                    text: &text,
                    span: SourceSpan::new(start, end),
                    selection: SourceSpan::new(
                        start + usize::from(quoted),
                        end - usize::from(quoted),
                    ),
                });
            }
            state.model.title = Some(trimmed.to_string());
            Ok(())
        }
        VennTokenKind::Set => parse_venn_set_statement(
            cursor,
            state,
            #[cfg(feature = "editor-facts")]
            observer,
        ),
        VennTokenKind::Union => parse_venn_union_statement(
            cursor,
            state,
            #[cfg(feature = "editor-facts")]
            observer,
        ),
        VennTokenKind::Text | VennTokenKind::IndentedText => parse_venn_text_statement(
            cursor,
            state,
            token.kind == VennTokenKind::IndentedText,
            #[cfg(feature = "editor-facts")]
            observer,
        ),
        VennTokenKind::Style => parse_venn_style_statement(
            cursor,
            state,
            #[cfg(feature = "editor-facts")]
            observer,
        ),
        _ => Err(format!(
            "unexpected venn statement: {}",
            &cursor.input[token.span.start..token.span.end]
        )
        .into()),
    }
}

fn parse_venn_set_statement(
    cursor: &mut VennCursor<'_>,
    state: &mut VennSemanticState,
    #[cfg(feature = "editor-facts")] observer: &mut VennObserver<'_>,
) -> StatementResult<()> {
    let identifier = cursor.take_identifier_like()?;
    if cursor.take_delimiter(',') {
        return Err("set requires single identifier".into());
    }

    let label = cursor.take_optional_bracket_label()?;
    let size = cursor.take_optional_size()?;
    cursor.expect_end()?;
    let size_value = size
        .as_ref()
        .map(|field| field.text.parse::<f64>())
        .transpose()
        .map_err(|_| Cow::Borrowed("expected numeric"))?;

    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observer.rich() {
        observe(identifier.observation(VennFieldKind::Set));
        if let Some(label) = label.as_ref() {
            observe(label.observation(VennFieldKind::SetLabel));
        }
        if let Some(size) = size.as_ref() {
            observe(size.observation(VennFieldKind::Size));
        }
    }
    state.add_subset(
        vec![identifier.text],
        label.map(|field| field.text),
        size_value,
    );
    cursor.indent_mode = true;
    Ok(())
}

fn parse_venn_union_statement(
    cursor: &mut VennCursor<'_>,
    state: &mut VennSemanticState,
    #[cfg(feature = "editor-facts")] observer: &mut VennObserver<'_>,
) -> StatementResult<()> {
    let identifiers = parse_venn_identifier_list(cursor)?;
    if identifiers.len() < 2 {
        return Err("union requires multiple identifiers".into());
    }
    let label = cursor.take_optional_bracket_label()?;
    let size = cursor.take_optional_size()?;
    cursor.expect_end()?;
    let size_value = size
        .as_ref()
        .map(|field| field.text.parse::<f64>())
        .transpose()
        .map_err(|_| Cow::Borrowed("expected numeric"))?;

    let identifier_values = identifiers
        .iter()
        .map(|identifier| identifier.text.clone())
        .collect::<Vec<_>>();
    state.validate_union_identifiers(&identifier_values)?;

    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observer.rich() {
        observe(VennObservation::IdList(venn_list_span(&identifiers)));
        for identifier in &identifiers {
            observe(identifier.observation(VennFieldKind::UnionSet));
        }
        if let Some(label) = label.as_ref() {
            observe(label.observation(VennFieldKind::UnionLabel));
        }
        if let Some(size) = size.as_ref() {
            observe(size.observation(VennFieldKind::Size));
        }
    }

    state.add_subset(identifier_values, label.map(|field| field.text), size_value);
    cursor.indent_mode = true;
    Ok(())
}

fn parse_venn_text_statement(
    cursor: &mut VennCursor<'_>,
    state: &mut VennSemanticState,
    indented: bool,
    #[cfg(feature = "editor-facts")] observer: &mut VennObserver<'_>,
) -> StatementResult<()> {
    let (explicit_sets, _set_fields) = if indented {
        (
            state
                .current_sets
                .clone()
                .ok_or(Cow::Borrowed("text requires set"))?,
            None,
        )
    } else {
        let sets = parse_venn_identifier_list(cursor)?;
        let values = sets.iter().map(|set| set.text.clone()).collect::<Vec<_>>();
        (values, Some(sets))
    };

    let (identifier, kind) = cursor.take_text_id()?;
    let label = cursor.take_optional_bracket_label()?;
    if kind == TextIdKind::Numeric && label.is_some() {
        return Err("unexpected label after numeric text id".into());
    }
    cursor.expect_end()?;
    if explicit_sets.is_empty() {
        return Err("text requires set".into());
    }

    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observer.rich() {
        if let Some(sets) = _set_fields {
            observe(VennObservation::IdList(venn_list_span(&sets)));
            for set in &sets {
                observe(set.observation(VennFieldKind::TextSet));
            }
        }
        observe(identifier.observation(VennFieldKind::TextNode));
        if let Some(label) = label.as_ref() {
            observe(label.observation(VennFieldKind::TextLabel));
        }
    }

    state.add_text(
        explicit_sets,
        identifier.text,
        label.map(|field| field.text),
    );
    Ok(())
}

fn parse_venn_style_statement(
    cursor: &mut VennCursor<'_>,
    state: &mut VennSemanticState,
    #[cfg(feature = "editor-facts")] observer: &mut VennObserver<'_>,
) -> StatementResult<()> {
    let targets = parse_venn_identifier_list(cursor)?;
    let mut styles = Vec::new();
    loop {
        let key = cursor.take_bare_identifier()?;
        if !cursor.take_delimiter(':') {
            return Err("expected ':' after style field".into());
        }
        let value = cursor.take_style_value()?;
        styles.push((key, value));

        if cursor.take_delimiter(',') {
            continue;
        }
        break;
    }

    cursor.expect_end()?;

    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observer.rich() {
        observe(VennObservation::IdList(venn_list_span(&targets)));
        for target in &targets {
            observe(target.observation(VennFieldKind::StyleTarget));
        }
        for (_, value) in &styles {
            observe(value.observation(VennFieldKind::StyleValue));
        }
    }

    state.add_style(
        targets.into_iter().map(|target| target.text).collect(),
        styles
            .into_iter()
            .map(|(key, value)| (key.text, value.text))
            .collect(),
    );
    Ok(())
}

fn parse_venn_identifier_list(cursor: &mut VennCursor<'_>) -> StatementResult<Vec<VennFieldSpan>> {
    let first = cursor.take_identifier_like()?;
    let mut identifiers = vec![first];

    loop {
        if !cursor.take_delimiter(',') {
            break;
        }
        identifiers.push(cursor.take_identifier_like()?);
    }

    Ok(identifiers)
}

#[cfg(feature = "editor-facts")]
fn venn_list_span(items: &[VennFieldSpan]) -> SourceSpan {
    let start = items.first().map(|item| item.span.start).unwrap_or(0);
    let end = items.last().map(|item| item.span.end).unwrap_or(start);
    SourceSpan::new(start, end)
}

fn keyword_prefix(input: &str, keyword: &str) -> bool {
    input
        .get(..keyword.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(keyword))
        && input
            .as_bytes()
            .get(keyword.len())
            .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
}

fn title_token_len(input: &str) -> Option<usize> {
    if !input.get(..5)?.eq_ignore_ascii_case("title") {
        return None;
    }
    let rest = &input[5..];
    let separator = rest.chars().next()?;
    if !is_ecmascript_whitespace(separator) {
        return None;
    }
    let payload = &rest[separator.len_utf8()..];
    let end = payload.find(['#', '\n', ';']).unwrap_or(payload.len());
    (end > 0).then_some(5 + separator.len_utf8() + end)
}

fn parse_bare_identifier_token(input: &str) -> Option<(&str, &str)> {
    let bytes = input.as_bytes();
    let first = *bytes.first()?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }

    let mut end = 1usize;
    while end < bytes.len() {
        let byte = bytes[end];
        if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-' {
            end += 1;
        } else {
            break;
        }
    }
    Some((&input[..end], &input[end..]))
}

fn parse_numeric_token(input: &str) -> Option<(&str, &str)> {
    let bytes = input.as_bytes();
    let mut end = usize::from(
        bytes
            .first()
            .is_some_and(|byte| matches!(byte, b'+' | b'-')),
    );
    let integer_start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let has_integer = end > integer_start;

    let has_fraction =
        bytes.get(end) == Some(&b'.') && bytes.get(end + 1).is_some_and(u8::is_ascii_digit);
    if has_fraction {
        end += 1;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
    }

    if !has_integer && !has_fraction {
        return None;
    }

    Some((&input[..end], &input[end..]))
}

fn parse_rgb_like_token(input: &str) -> Option<(&str, &str)> {
    let (prefix_len, component_count) = if input
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("rgba("))
    {
        ("rgba(".len(), 4)
    } else if input
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("rgb("))
    {
        ("rgb(".len(), 3)
    } else {
        return None;
    };
    let mut pos = prefix_len;
    for component in 0..component_count {
        pos += input[pos..]
            .chars()
            .take_while(|ch| is_ecmascript_whitespace(*ch))
            .map(char::len_utf8)
            .sum::<usize>();
        let number_start = pos;
        pos += input[pos..]
            .bytes()
            .take_while(|byte| byte.is_ascii_digit() || *byte == b'.')
            .count();
        if number_start == pos {
            return None;
        }
        pos += input[pos..]
            .chars()
            .take_while(|ch| is_ecmascript_whitespace(*ch))
            .map(char::len_utf8)
            .sum::<usize>();
        let delimiter = if component + 1 == component_count {
            b')'
        } else {
            b','
        };
        if input.as_bytes().get(pos) != Some(&delimiter) {
            return None;
        }
        pos += 1;
    }
    Some((&input[..pos], &input[pos..]))
}

fn parse_hex_color_token(input: &str) -> Option<(&str, &str)> {
    let rest = input.strip_prefix('#')?;
    let len = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_hexdigit())
        .take(8)
        .count();
    if (3..=8).contains(&len) {
        Some((&input[..len + 1], &input[len + 1..]))
    } else {
        None
    }
}

fn normalize_identifier_list(identifiers: Vec<String>) -> Vec<String> {
    let mut out = identifiers
        .into_iter()
        .map(|identifier| normalize_text(&identifier))
        .collect::<Vec<_>>();
    out.sort();
    out
}

fn normalize_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests;
