#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, Result, SourceSpan};
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(all(test, feature = "compat-json"))]
use serde_json::json;
use std::collections::{BTreeMap, HashSet};

pub(crate) use merman_model::families::venn::{
    VennDiagramModel, VennStyleEntryModel, VennSubsetModel, VennTextNodeModel,
};

pub(crate) fn sanitize_common_db_fields(
    model: &mut VennDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

#[derive(Debug)]
struct VennSemanticState {
    model: VennDiagramModel,
    known_sets: HashSet<String>,
    current_sets: Option<Vec<String>>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
    #[cfg(feature = "editor-facts")]
    editor_demand: EditorFactsDemand,
}

impl VennSemanticState {
    fn new(#[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand) -> Self {
        Self {
            model: VennDiagramModel::default(),
            known_sets: HashSet::new(),
            current_sets: None,
            #[cfg(feature = "editor-facts")]
            editor_facts: match editor_demand {
                EditorFactsDemand::Omit => None,
                EditorFactsDemand::Recovery => Some(EditorSemanticFacts::new()),
                EditorFactsDemand::Capture => {
                    #[cfg(test)]
                    crate::editor::test_work::record_collection();
                    Some(EditorSemanticFacts::new())
                }
            },
            #[cfg(feature = "editor-facts")]
            editor_demand,
        }
    }

    #[cfg(feature = "editor-facts")]
    fn rich_editor_facts(&mut self) -> Option<&mut EditorSemanticFacts> {
        if self.editor_demand.captures_rich() {
            self.editor_facts.as_mut()
        } else {
            None
        }
    }

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

    fn validate_union_identifiers(&self, identifiers: &[String], diagram_type: &str) -> Result<()> {
        let unknown = identifiers
            .iter()
            .map(|identifier| normalize_text(identifier))
            .filter(|identifier| !self.known_sets.contains(identifier))
            .collect::<Vec<_>>();

        if unknown.is_empty() {
            Ok(())
        } else {
            Err(parse_error(
                diagram_type,
                format!("unknown set identifier: {}", unknown.join(", ")),
            ))
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

struct VennSemanticSource {
    model: VennDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct VennParseOutcome {
    source: VennSemanticSource,
    first_error: Option<Error>,
}

impl VennParseOutcome {
    #[cfg(any(test, feature = "presentation"))]
    fn into_strict_source(self) -> Result<VennSemanticSource> {
        match self.first_error {
            Some(error) => Err(error),
            None => Ok(self.source),
        }
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

    fn take_identifier_like(&mut self, diagram_type: &str) -> Result<VennFieldSpan> {
        if !matches!(
            self.peek().kind,
            VennTokenKind::Identifier | VennTokenKind::String
        ) {
            return Err(parse_error(diagram_type, "expected identifier"));
        }
        let token = self.take();
        Ok(self.field(token))
    }

    fn take_bare_identifier(&mut self, diagram_type: &str) -> Result<VennFieldSpan> {
        if self.peek().kind != VennTokenKind::Identifier {
            return Err(parse_error(diagram_type, "expected identifier"));
        }
        let token = self.take();
        Ok(self.field(token))
    }

    fn take_text_id(&mut self, diagram_type: &str) -> Result<(VennFieldSpan, TextIdKind)> {
        if self.peek().kind == VennTokenKind::Numeric {
            let token = self.take();
            return Ok((self.field(token), TextIdKind::Numeric));
        }
        let token = self.take_identifier_like(diagram_type)?;
        Ok((token, TextIdKind::IdentifierOrString))
    }

    fn take_optional_bracket_label(&mut self, diagram_type: &str) -> Result<Option<VennFieldSpan>> {
        let token = self.peek();
        if token.kind == VennTokenKind::BracketLabel {
            self.take();
            return Ok(Some(self.field(token)));
        }
        if self.input[token.span.start..].starts_with('[') {
            return Err(parse_error(diagram_type, "unterminated bracket label"));
        }
        Ok(None)
    }

    fn take_optional_size(&mut self, diagram_type: &str) -> Result<Option<VennFieldSpan>> {
        if !self.take_delimiter(':') {
            return Ok(None);
        }
        if self.peek().kind != VennTokenKind::Numeric {
            return Err(parse_error(diagram_type, "expected numeric"));
        }
        let token = self.take();
        Ok(Some(self.field(token)))
    }

    fn take_style_value(&mut self, diagram_type: &str) -> Result<VennFieldSpan> {
        if self.peek().kind == VennTokenKind::String {
            let token = self.take();
            return Ok(self.field(token));
        }
        if !self.peek().kind.is_style_value() {
            return Err(parse_error(diagram_type, "expected style value"));
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
            return Err(parse_error(diagram_type, "expected style value"));
        }
        Ok(value)
    }

    fn expect_end(&mut self, diagram_type: &str) -> Result<()> {
        let token = self.peek();
        if token.kind.ends_statement() {
            Ok(())
        } else {
            Err(parse_error(
                diagram_type,
                format!(
                    "unexpected trailing venn tokens: {}",
                    &self.input[token.span.start..token.span.end]
                ),
            ))
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

#[cfg(feature = "compat-json")]
pub(crate) fn parse_venn(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let source = parse_venn_semantic_source(code, meta)?;
    render_model_to_compat_json(&source.model, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_venn_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::CombinedSemanticParse> {
    let VennParseOutcome {
        source,
        first_error,
    } = construct_venn_parse_outcome_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )?;
    let construction = match first_error {
        Some(error) => Err(crate::family::CombinedSemanticFailure::new(
            error,
            source.editor_facts,
        )),
        None => Ok(source),
    };
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |source| {
            (
                render_model_to_compat_json(&source.model, meta),
                source.editor_facts.unwrap_or_default(),
            )
        },
        crate::family::CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_venn_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<VennDiagramModel> {
    Ok(parse_venn_semantic_source(code, meta)?.model)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_venn_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<VennDiagramModel>> {
    let outcome = construct_venn_parse_outcome_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    Ok(outcome.into_strict_source().map(|source| source.model))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &VennDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::venn::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_venn_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<VennSemanticSource> {
    construct_venn_parse_outcome_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &crate::OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .into_strict_source()
}

pub(crate) fn parse_venn_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<VennDiagramModel>> {
    let VennParseOutcome {
        source,
        first_error,
    } = construct_venn_parse_outcome_controlled(
        code,
        &metadata.diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    #[cfg(feature = "editor-facts")]
    let editor_facts = source.editor_facts;
    let model = match first_error {
        Some(error) => Err(error),
        None => {
            let mut model = source.model;
            sanitize_common_db_fields(&mut model, &metadata.semantic_config);
            Ok(model)
        }
    };
    Ok(crate::family::SemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

fn construct_venn_parse_outcome_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<VennParseOutcome> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("venn");

    let mut state = VennSemanticState::new(
        #[cfg(feature = "editor-facts")]
        editor_demand,
    );
    let mut header_decided = false;
    let mut first_error = None;
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
                    diagram_type,
                    &mut state,
                    &mut first_error,
                    parse_error(diagram_type, "expected venn-beta header"),
                    #[cfg(feature = "editor-facts")]
                    "expected venn-beta header",
                    span,
                );
                cursor.indent_mode = false;
                continue;
            }

            #[cfg(feature = "editor-facts")]
            let header_span = token.span;
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = state.rich_editor_facts() {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::Payload,
                    header_span,
                ));
            }
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = state.rich_editor_facts() {
                facts.push_symbol(EditorSemanticSymbol::payload(
                    "venn-beta".to_string(),
                    Some("venn header".to_string()),
                    EditorSemanticKind::String,
                    header_span,
                    header_span,
                ));
            }

            continue;
        }

        if let Err(error) = parse_venn_statement_facts(&mut cursor, &mut state, diagram_type) {
            let span = cursor.recover_statement(token.span.start);
            #[cfg(feature = "editor-facts")]
            let message = venn_error_message(&error);
            recover_venn_error(
                diagram_type,
                &mut state,
                &mut first_error,
                error,
                #[cfg(feature = "editor-facts")]
                format!("venn parser recovered after parse error: {message}"),
                span,
            );
            cursor.indent_mode = false;
        }
    }

    if !header_decided {
        recover_venn_error(
            diagram_type,
            &mut state,
            &mut first_error,
            parse_error(diagram_type, "expected venn-beta"),
            #[cfg(feature = "editor-facts")]
            "expected venn-beta",
            SourceSpan::new(0, 0),
        );
    }

    control.checkpoint()?;
    Ok(VennParseOutcome {
        source: VennSemanticSource {
            model: state.model,
            #[cfg(feature = "editor-facts")]
            editor_facts: state.editor_facts,
        },
        first_error,
    })
}

fn recover_venn_error(
    diagram_type: &str,
    state: &mut VennSemanticState,
    first_error: &mut Option<Error>,
    error: Error,
    #[cfg(feature = "editor-facts")] recovery_message: impl Into<String>,
    span: SourceSpan,
) {
    if first_error.is_none() {
        *first_error = Some(Error::diagram_parse_exact(
            diagram_type.to_string(),
            venn_error_message(&error),
            span,
        ));
    }
    #[cfg(feature = "editor-facts")]
    if let Some(facts) = state.editor_facts.as_mut() {
        facts.mark_recovered_from_parse_error(recovery_message, Some(span));
    }
    state.current_sets = None;
}

fn venn_error_message(error: &Error) -> String {
    match error {
        Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
        _ => error.to_string(),
    }
}

fn parse_venn_statement_facts(
    cursor: &mut VennCursor<'_>,
    state: &mut VennSemanticState,
    diagram_type: &str,
) -> Result<()> {
    let token = cursor.take();
    #[cfg(feature = "editor-facts")]
    if let Some(facts) = state.rich_editor_facts() {
        let prefix = match token.kind {
            VennTokenKind::Title => Some("title"),
            VennTokenKind::Set => Some("set"),
            VennTokenKind::Union => Some("union"),
            VennTokenKind::Text | VennTokenKind::IndentedText => Some("text"),
            VennTokenKind::Style => Some("style"),
            _ => None,
        };
        if let Some(prefix) = prefix {
            facts.push_directive_prefix(prefix);
        }
    }
    match token.kind {
        VennTokenKind::Title => {
            cursor
                .expect_end(diagram_type)
                .map_err(|_| parse_error(diagram_type, "invalid Venn title syntax"))?;
            let raw = &cursor.input[token.span.start + "title".len()..token.span.end];
            let trimmed = raw.trim();
            #[cfg(feature = "editor-facts")]
            {
                let start = token.span.start + "title".len() + raw.len() - raw.trim_start().len();
                let end = start + trimmed.len();
                let quoted =
                    trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2;
                push_venn_payload_fact(
                    state.rich_editor_facts(),
                    &VennFieldSpan {
                        text: normalize_text(trimmed),
                        span: SourceSpan::new(start, end),
                        selection: SourceSpan::new(
                            start + usize::from(quoted),
                            end - usize::from(quoted),
                        ),
                    },
                    "venn title",
                    EditorSemanticKind::String,
                );
            }
            state.model.title = Some(trimmed.to_string());
            Ok(())
        }
        VennTokenKind::Set => parse_venn_set_statement_facts(cursor, state, diagram_type),
        VennTokenKind::Union => parse_venn_union_statement_facts(cursor, state, diagram_type),
        VennTokenKind::Text | VennTokenKind::IndentedText => parse_venn_text_statement_facts(
            cursor,
            state,
            diagram_type,
            token.kind == VennTokenKind::IndentedText,
        ),
        VennTokenKind::Style => parse_venn_style_statement_facts(cursor, state, diagram_type),
        _ => Err(parse_error(
            diagram_type,
            format!(
                "unexpected venn statement: {}",
                &cursor.input[token.span.start..token.span.end]
            ),
        )),
    }
}

fn parse_venn_set_statement_facts(
    cursor: &mut VennCursor<'_>,
    state: &mut VennSemanticState,
    diagram_type: &str,
) -> Result<()> {
    let parsed = (|| {
        let identifier = cursor.take_identifier_like(diagram_type)?;
        if cursor.take_delimiter(',') {
            return Err(parse_error(diagram_type, "set requires single identifier"));
        }

        let label = cursor.take_optional_bracket_label(diagram_type)?;
        let size = cursor.take_optional_size(diagram_type)?;
        cursor.expect_end(diagram_type)?;
        let size_value = size
            .as_ref()
            .map(|field| field.text.parse::<f64>())
            .transpose()
            .map_err(|_| parse_error(diagram_type, "expected numeric"))?;
        Ok((identifier, label, size, size_value))
    })();
    let (identifier, label, _size, size_value) = parsed?;
    #[cfg(feature = "editor-facts")]
    let size = _size;

    #[cfg(feature = "editor-facts")]
    if let Some(facts) = state.rich_editor_facts() {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::NodeIdentifier,
            identifier.span,
        ));
    }
    #[cfg(feature = "editor-facts")]
    push_venn_entity_fact(
        state.rich_editor_facts(),
        &identifier,
        "venn set",
        EditorSemanticKind::Namespace,
    );
    #[cfg(feature = "editor-facts")]
    if let Some(label) = label.as_ref() {
        push_venn_payload_fact(
            state.rich_editor_facts(),
            label,
            "venn set label",
            EditorSemanticKind::String,
        );
    }
    #[cfg(feature = "editor-facts")]
    if let Some(size) = size.as_ref() {
        push_venn_payload_fact(
            state.rich_editor_facts(),
            size,
            "venn size",
            EditorSemanticKind::String,
        );
    }
    state.add_subset(
        vec![identifier.text],
        label.map(|field| field.text),
        size_value,
    );
    cursor.indent_mode = true;
    Ok(())
}

fn parse_venn_union_statement_facts(
    cursor: &mut VennCursor<'_>,
    state: &mut VennSemanticState,
    diagram_type: &str,
) -> Result<()> {
    let parsed = (|| {
        let identifiers = parse_venn_identifier_list(cursor, diagram_type)?;
        if identifiers.len() < 2 {
            return Err(parse_error(
                diagram_type,
                "union requires multiple identifiers",
            ));
        }
        let label = cursor.take_optional_bracket_label(diagram_type)?;
        let size = cursor.take_optional_size(diagram_type)?;
        cursor.expect_end(diagram_type)?;
        let size_value = size
            .as_ref()
            .map(|field| field.text.parse::<f64>())
            .transpose()
            .map_err(|_| parse_error(diagram_type, "expected numeric"))?;
        Ok((identifiers, label, size, size_value))
    })();
    let (identifiers, label, _size, size_value) = parsed?;
    #[cfg(feature = "editor-facts")]
    let size = _size;

    let identifier_values = identifiers
        .iter()
        .map(|identifier| identifier.text.clone())
        .collect::<Vec<_>>();
    state.validate_union_identifiers(&identifier_values, diagram_type)?;

    #[cfg(feature = "editor-facts")]
    let list_span = venn_list_span(&identifiers);
    #[cfg(feature = "editor-facts")]
    if let Some(facts) = state.rich_editor_facts() {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::IdList,
            list_span,
        ));
    }
    #[cfg(feature = "editor-facts")]
    for identifier in &identifiers {
        push_venn_reference_fact(
            state.rich_editor_facts(),
            identifier,
            "venn union set",
            EditorSemanticKind::Namespace,
        );
    }

    #[cfg(feature = "editor-facts")]
    if let Some(label) = label.as_ref() {
        push_venn_payload_fact(
            state.rich_editor_facts(),
            label,
            "venn union label",
            EditorSemanticKind::String,
        );
    }
    #[cfg(feature = "editor-facts")]
    if let Some(size) = size.as_ref() {
        push_venn_payload_fact(
            state.rich_editor_facts(),
            size,
            "venn size",
            EditorSemanticKind::String,
        );
    }

    state.add_subset(identifier_values, label.map(|field| field.text), size_value);
    cursor.indent_mode = true;
    Ok(())
}

fn parse_venn_text_statement_facts(
    cursor: &mut VennCursor<'_>,
    state: &mut VennSemanticState,
    diagram_type: &str,
    indented: bool,
) -> Result<()> {
    let parsed = (|| {
        let (explicit_sets, set_fields) = if indented {
            (
                state
                    .current_sets
                    .clone()
                    .ok_or_else(|| parse_error(diagram_type, "text requires set"))?,
                None,
            )
        } else {
            let sets = parse_venn_identifier_list(cursor, diagram_type)?;
            let values = sets.iter().map(|set| set.text.clone()).collect::<Vec<_>>();
            (values, Some(sets))
        };

        let (identifier, kind) = cursor.take_text_id(diagram_type)?;
        let label = cursor.take_optional_bracket_label(diagram_type)?;
        if kind == TextIdKind::Numeric && label.is_some() {
            return Err(parse_error(
                diagram_type,
                "unexpected label after numeric text id",
            ));
        }
        cursor.expect_end(diagram_type)?;
        if explicit_sets.is_empty() {
            return Err(parse_error(diagram_type, "text requires set"));
        }
        Ok((explicit_sets, set_fields, identifier, label))
    })();
    let (explicit_sets, _set_fields, identifier, label) = parsed?;

    #[cfg(feature = "editor-facts")]
    if let Some(sets) = _set_fields {
        let list_span = venn_list_span(&sets);
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = state.rich_editor_facts() {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::IdList,
                list_span,
            ));
        }
        for set in &sets {
            #[cfg(feature = "editor-facts")]
            push_venn_reference_fact(
                state.rich_editor_facts(),
                set,
                "venn text set",
                EditorSemanticKind::Namespace,
            );
        }
    }

    #[cfg(feature = "editor-facts")]
    if let Some(facts) = state.rich_editor_facts() {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::NodeIdentifier,
            identifier.span,
        ));
    }
    #[cfg(feature = "editor-facts")]
    push_venn_entity_fact(
        state.rich_editor_facts(),
        &identifier,
        "venn text node",
        EditorSemanticKind::Namespace,
    );
    #[cfg(feature = "editor-facts")]
    if let Some(label) = label.as_ref() {
        push_venn_payload_fact(
            state.rich_editor_facts(),
            label,
            "venn text label",
            EditorSemanticKind::String,
        );
    }

    state.add_text(
        explicit_sets,
        identifier.text,
        label.map(|field| field.text),
    );
    Ok(())
}

fn parse_venn_style_statement_facts(
    cursor: &mut VennCursor<'_>,
    state: &mut VennSemanticState,
    diagram_type: &str,
) -> Result<()> {
    let parsed = (|| {
        let targets = parse_venn_identifier_list(cursor, diagram_type)?;
        let mut styles = Vec::new();
        loop {
            let key = cursor.take_bare_identifier(diagram_type)?;
            if !cursor.take_delimiter(':') {
                return Err(parse_error(diagram_type, "expected ':' after style field"));
            }
            let value = cursor.take_style_value(diagram_type)?;
            styles.push((key, value));

            if cursor.take_delimiter(',') {
                continue;
            }
            break;
        }

        cursor.expect_end(diagram_type)?;
        Ok((targets, styles))
    })();
    let (targets, styles) = parsed?;

    #[cfg(feature = "editor-facts")]
    let list_span = venn_list_span(&targets);
    #[cfg(feature = "editor-facts")]
    if let Some(facts) = state.rich_editor_facts() {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::IdList,
            list_span,
        ));
    }
    #[cfg(feature = "editor-facts")]
    for target in &targets {
        push_venn_reference_fact(
            state.rich_editor_facts(),
            target,
            "venn style target",
            EditorSemanticKind::Namespace,
        );
    }
    #[cfg(feature = "editor-facts")]
    for (_, value) in &styles {
        push_venn_payload_fact(
            state.rich_editor_facts(),
            value,
            "venn style value",
            EditorSemanticKind::String,
        );
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

fn parse_venn_identifier_list(
    cursor: &mut VennCursor<'_>,
    diagram_type: &str,
) -> Result<Vec<VennFieldSpan>> {
    let first = cursor.take_identifier_like(diagram_type)?;
    let mut identifiers = vec![first];

    loop {
        if !cursor.take_delimiter(',') {
            break;
        }
        identifiers.push(cursor.take_identifier_like(diagram_type)?);
    }

    Ok(identifiers)
}

#[cfg(feature = "editor-facts")]
fn venn_list_span(items: &[VennFieldSpan]) -> SourceSpan {
    let start = items.first().map(|item| item.span.start).unwrap_or(0);
    let end = items.last().map(|item| item.span.end).unwrap_or(start);
    SourceSpan::new(start, end)
}

#[cfg(feature = "editor-facts")]
fn push_venn_entity_fact(
    facts: Option<&mut EditorSemanticFacts>,
    field: &VennFieldSpan,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    let Some(facts) = facts else {
        return;
    };
    if field.text.is_empty() {
        return;
    }
    facts.push_symbol(EditorSemanticSymbol::new(
        field.text.clone(),
        Some(detail.to_string()),
        kind,
        field.span,
        field.selection,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_venn_reference_fact(
    facts: Option<&mut EditorSemanticFacts>,
    field: &VennFieldSpan,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    let Some(facts) = facts else {
        return;
    };
    if field.text.is_empty() {
        return;
    }
    facts.push_symbol(EditorSemanticSymbol::reference(
        field.text.clone(),
        Some(detail.to_string()),
        kind,
        field.span,
        field.selection,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_venn_payload_fact(
    facts: Option<&mut EditorSemanticFacts>,
    field: &VennFieldSpan,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    let Some(facts) = facts else {
        return;
    };
    if field.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        field.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        field.text.clone(),
        Some(detail.to_string()),
        kind,
        field.span,
        field.selection,
    ));
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
    if !crate::diagrams::scan::is_ecmascript_whitespace(separator) {
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
            .take_while(|ch| crate::diagrams::scan::is_ecmascript_whitespace(*ch))
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
            .take_while(|ch| crate::diagrams::scan::is_ecmascript_whitespace(*ch))
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

fn parse_error(diagram_type: &str, message: impl Into<String>) -> Error {
    Error::diagram_parse_fallback(diagram_type.to_string(), message.into())
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::{EditorSemanticCompleteness, EditorSemanticRole};
    use crate::{
        MermaidConfig, MermaidEngine, ParseOptions, RenderSemanticModel, SourcePresentationMetadata,
    };

    fn meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "venn".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        }
    }

    fn parse(input: &str) -> VennDiagramModel {
        parse_venn_model_for_render(input, &meta()).unwrap()
    }

    #[test]
    fn venn_adjacent_sets_share_model_and_editor_semantics() {
        for (body, first_size) in [("set A:1set B", 1.0), ("set A set B", 10.0)] {
            let source = format!("venn-beta\n{body}\n");
            let model = parse(&source);
            assert_eq!(model.subsets.len(), 2);
            assert_eq!(model.subsets[0].sets, ["A"]);
            assert_eq!(model.subsets[0].size, first_size);
            assert_eq!(model.subsets[1].sets, ["B"]);
            assert_eq!(model.subsets[1].size, 10.0);
            let compat = parse_venn(&source, &meta()).unwrap();
            assert_eq!(compat["subsets"], json!(model.subsets));

            #[cfg(feature = "editor-facts")]
            {
                let (combined, facts) =
                    crate::family::test_support::into_result(parse_venn_json_and_editor_facts(
                        &source,
                        &meta(),
                        &crate::OperationControl::new(),
                    ))
                    .unwrap();
                assert_eq!(combined, compat);
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
                assert!(facts.diagnostics.is_empty());
                for name in ["A", "B"] {
                    let start = source.find(name).unwrap();
                    let symbol = facts
                        .symbols
                        .iter()
                        .find(|symbol| {
                            symbol.name == name && symbol.detail.as_deref() == Some("venn set")
                        })
                        .unwrap();
                    assert_eq!(symbol.span, SourceSpan::new(start, start + 1));
                    assert_eq!(symbol.selection, symbol.span);
                }
            }
        }
    }

    #[test]
    fn venn_adjacent_union_text_and_styles_follow_grammar() {
        let source = concat!(
            "venn-beta\n",
            "set A[set union text style]set B[Beta]:2",
            "union A,B[Both]:.5text A note[set union text style]",
            "style A fill:rgb(1, 2, 3), stroke:red blue ",
            "style B fill:\"set A union text style\"set C\n",
        );
        let model = parse(source);
        assert_eq!(model.subsets.len(), 4);
        assert_eq!(
            model.subsets[0].label.as_deref(),
            Some("set union text style")
        );
        assert_eq!(model.subsets[1].size, 2.0);
        assert_eq!(model.subsets[2].sets, ["A", "B"]);
        assert_eq!(model.subsets[2].label.as_deref(), Some("Both"));
        assert_eq!(model.subsets[2].size, 0.5);
        assert_eq!(model.subsets[3].sets, ["C"]);
        assert_eq!(model.text_nodes.len(), 1);
        assert_eq!(model.text_nodes[0].sets, ["A"]);
        assert_eq!(model.text_nodes[0].id, "note");
        assert_eq!(
            model.text_nodes[0].label.as_deref(),
            Some("set union text style")
        );
        assert_eq!(model.style_entries.len(), 2);
        assert_eq!(model.style_entries[0].styles["fill"], "rgb(1, 2, 3)");
        assert_eq!(model.style_entries[0].styles["stroke"], "red blue");
        assert_eq!(
            model.style_entries[1].styles["fill"],
            "set A union text style"
        );
    }

    #[test]
    fn venn_adjacent_statements_preserve_bol_indent_mode() {
        let model = parse(concat!(
            "venn-beta\nset A:1set B\n",
            "  text\"one\" text A two\n",
            "  %% comment\n\n",
            "  text 1set C\n",
            "  text three\n",
            "  style A fill:red\n",
            "  text A explicit\n",
        ));
        let texts = model
            .text_nodes
            .iter()
            .map(|node| {
                (
                    node.sets.iter().map(String::as_str).collect::<Vec<_>>(),
                    node.id.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            texts,
            vec![
                (vec!["B"], "one"),
                (vec!["A"], "two"),
                (vec!["B"], "1"),
                (vec!["C"], "three"),
                (vec!["A"], "explicit"),
            ]
        );
    }

    #[test]
    fn venn_adjacent_multiline_payloads_keep_keyword_text() {
        let source = concat!(
            "venn-beta\n",
            "set A[\"first\nset second\"]",
            "text A \"text\nstyle payload\"",
            "style A fill:\"union\nset payload\"set B\n",
        );
        let model = parse(source);
        assert_eq!(model.subsets.len(), 2);
        assert_eq!(model.subsets[0].label.as_deref(), Some("first\nset second"));
        assert_eq!(model.text_nodes[0].id, "text\nstyle payload");
        assert_eq!(model.style_entries[0].styles["fill"], "union\nset payload");
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_venn_json_and_editor_facts,
                source,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            for (raw, name, detail, inset) in [
                (
                    "[\"first\nset second\"]",
                    "first\nset second",
                    "venn set label",
                    2,
                ),
                (
                    "\"text\nstyle payload\"",
                    "text\nstyle payload",
                    "venn text node",
                    1,
                ),
                (
                    "\"union\nset payload\"",
                    "union\nset payload",
                    "venn style value",
                    1,
                ),
            ] {
                let start = source.find(raw).unwrap();
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
                    .unwrap();
                assert_eq!(symbol.span, SourceSpan::new(start, start + raw.len()));
                assert_eq!(
                    symbol.selection,
                    SourceSpan::new(start + inset, start + raw.len() - inset)
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn venn_adjacent_public_ranges_preserve_bom_unicode_and_line_endings() {
        let engine = MermaidEngine::new();
        for newline in ["\n", "\r\n"] {
            let source = format!(
                "\u{feff}%% préface{newline}venn-beta{newline}set \"集合\"[\"标签\"]:1set B{newline}union \"集合\",B[\"共同\"]:.5text \"集合\" \"注记\"[\"正文\"]style B fill:\"set 关键词\"set C{newline}"
            );
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let RenderSemanticModel::Venn(model) = parsed.model() else {
                panic!("expected Venn model");
            };
            assert_eq!(model.subsets.len(), 4);
            assert_eq!(model.subsets[0].sets, ["集合"]);
            assert_eq!(model.subsets[2].sets, ["B", "集合"]);
            assert_eq!(model.text_nodes[0].id, "注记");
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("venn", &source)
                .unwrap()
                .unwrap();
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.diagnostics.is_empty());
            for (raw, name, detail, inset) in [
                ("venn-beta", "venn-beta", "venn header", 0),
                ("\"集合\"", "集合", "venn set", 1),
                ("[\"标签\"]", "标签", "venn set label", 2),
                ("[\"共同\"]", "共同", "venn union label", 2),
                ("\"注记\"", "注记", "venn text node", 1),
                ("[\"正文\"]", "正文", "venn text label", 2),
                ("\"set 关键词\"", "set 关键词", "venn style value", 1),
            ] {
                let start = source.find(raw).unwrap();
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
                    .unwrap_or_else(|| panic!("missing {detail}: {name}"));
                assert_eq!(symbol.span, SourceSpan::new(start, start + raw.len()));
                assert_eq!(
                    symbol.selection,
                    SourceSpan::new(start + inset, start + raw.len() - inset)
                );
            }
            for name in ["B", "C"] {
                let start = source.find(&format!("set {name}")).unwrap() + 4;
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| {
                        symbol.name == name && symbol.detail.as_deref() == Some("venn set")
                    })
                    .unwrap();
                assert_eq!(symbol.selection, SourceSpan::new(start, start + 1));
            }
        }
    }

    #[test]
    fn venn_adjacent_recovery_preserves_completed_and_following_statements() {
        for newline in ["\n", "\r\n"] {
            for (body, invalid) in [
                ("set A:1union A,set C", "union A,"),
                ("set A:1set B invalid set C", "set B invalid"),
            ] {
                let source = format!("venn-beta{newline}{body}{newline}set D{newline}");
                let start = source.find(invalid).unwrap();
                let expected = SourceSpan::new(start, start + invalid.len());
                let Error::DiagramParse { diagnostic, .. } =
                    parse_venn(&source, &meta()).unwrap_err()
                else {
                    panic!("expected Venn parse error");
                };
                assert_eq!(diagnostic.span(), Some(expected));
                #[cfg(feature = "editor-facts")]
                {
                    let facts = crate::family::test_support::editor_facts(
                        parse_venn_json_and_editor_facts,
                        &source,
                        &meta(),
                    );
                    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                    assert!(
                        facts
                            .diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.span == Some(expected))
                    );
                    let sets = facts
                        .symbols
                        .iter()
                        .filter(|symbol| symbol.detail.as_deref() == Some("venn set"))
                        .collect::<Vec<_>>();
                    assert_eq!(
                        sets.iter()
                            .map(|symbol| symbol.name.as_str())
                            .collect::<Vec<_>>(),
                        ["A", "C", "D"]
                    );
                    for symbol in sets {
                        let offset = source.find(&format!("set {}", symbol.name)).unwrap() + 4;
                        assert_eq!(symbol.selection, SourceSpan::new(offset, offset + 1));
                    }
                }
            }
        }
    }

    #[test]
    fn venn_adjacent_tokens_do_not_relax_invalid_follow_rules() {
        for body in [
            "set A:1setB",
            "set A\ntext A 1[label]set B",
            "set A\nstyle A fill:\"a\" red set B",
            "set A\nstyle A fill:red \"a\" set B",
        ] {
            assert!(
                parse_venn(&format!("venn-beta\n{body}\n"), &meta()).is_err(),
                "{body}"
            );
        }
    }

    #[test]
    fn venn_adjacent_comments_preserve_utf16_prefix_rules_and_byte_ranges() {
        for prefix in ["é", "中", " "] {
            let source = format!("venn-beta\n{prefix}%% comment\nset A\n");
            assert_eq!(parse(&source).subsets[0].sets, ["A"]);
        }
        for newline in ["\n", "\r\n"] {
            let source = format!("venn-beta{newline}😀%% comment{newline}set A{newline}");
            let start = source.find('😀').unwrap();
            let expected = SourceSpan::new(start, start + '😀'.len_utf8());
            let Error::DiagramParse { diagnostic, .. } = parse_venn(&source, &meta()).unwrap_err()
            else {
                panic!("expected Venn lexical parse error");
            };
            assert_eq!(diagnostic.span(), Some(expected));
            #[cfg(feature = "editor-facts")]
            {
                let original = format!("\u{feff}{source}");
                let facts = MermaidEngine::new()
                    .parse_editor_semantic_facts_with_type_sync("venn", &original)
                    .unwrap()
                    .unwrap();
                let original_start = original.find('😀').unwrap();
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                assert!(facts.diagnostics.iter().any(|diagnostic| {
                    diagnostic.span
                        == Some(SourceSpan::new(
                            original_start,
                            original_start + '😀'.len_utf8(),
                        ))
                }));
                let entity = facts
                    .symbols
                    .iter()
                    .find(|symbol| {
                        symbol.name == "A" && symbol.detail.as_deref() == Some("venn set")
                    })
                    .unwrap();
                let entity_start = original.find("set A").unwrap() + 4;
                assert_eq!(
                    entity.selection,
                    SourceSpan::new(entity_start, entity_start + 1)
                );
            }
        }
    }

    #[test]
    fn parses_simple_sets_title_and_default_union_size() {
        let model = parse(
            r#"venn-beta
          title foo bar
          set A
          set B
          union A,B
      "#,
        );

        assert_eq!(model.title.as_deref(), Some("foo bar"));
        assert_eq!(
            model.subsets,
            vec![
                VennSubsetModel {
                    sets: vec!["A".to_string()],
                    size: 10.0,
                    label: None,
                },
                VennSubsetModel {
                    sets: vec!["B".to_string()],
                    size: 10.0,
                    label: None,
                },
                VennSubsetModel {
                    sets: vec!["A".to_string(), "B".to_string()],
                    size: 2.5,
                    label: None,
                },
            ]
        );
    }

    #[test]
    fn title_matches_the_upstream_lexer_boundary() {
        assert_eq!(
            parse("venn-beta\ntitle Valid title\n").title.as_deref(),
            Some("Valid title")
        );

        for source in [
            "venn-beta\ntitle: Invalid\n",
            "venn-beta\ntitle Invalid; suffix\n",
            "venn-beta\ntitle Invalid # suffix\n",
        ] {
            let error = parse_venn_model_for_render(source, &meta())
                .expect_err("the pinned Venn lexer rejects this title form");
            assert!(error.to_string().contains("title"), "{error}");
        }
    }

    #[test]
    fn parses_bracket_labels_and_size_suffixes() {
        let model = parse(
            r#"venn-beta
          title foo bar
          set A["Alpha"]:20
          set B[Beta]:12
          set C["Gamma"]:30
          union A,B["AB"]:5.3
          union C,A,B:1
      "#,
        );

        assert_eq!(model.subsets[0].label.as_deref(), Some("Alpha"));
        assert_eq!(model.subsets[0].size, 20.0);
        assert_eq!(model.subsets[1].label.as_deref(), Some("Beta"));
        assert_eq!(model.subsets[1].size, 12.0);
        assert_eq!(model.subsets[3].sets, ["A", "B"]);
        assert_eq!(model.subsets[3].label.as_deref(), Some("AB"));
        assert_eq!(model.subsets[3].size, 5.3);
        assert_eq!(model.subsets[4].sets, ["A", "B", "C"]);
    }

    #[test]
    fn parses_text_nodes_with_explicit_and_indented_forms() {
        let model = parse(
            r#"venn-beta
          set A["Frontend"]
            text A1["React"]
            text A2
          set B["Backend"]
            text B1
          union A,B["APIs"]
            text AB1["OpenAPI"]
      "#,
        );

        assert_eq!(
            model.text_nodes,
            vec![
                VennTextNodeModel {
                    sets: vec!["A".to_string()],
                    id: "A1".to_string(),
                    label: Some("React".to_string()),
                },
                VennTextNodeModel {
                    sets: vec!["A".to_string()],
                    id: "A2".to_string(),
                    label: None,
                },
                VennTextNodeModel {
                    sets: vec!["B".to_string()],
                    id: "B1".to_string(),
                    label: None,
                },
                VennTextNodeModel {
                    sets: vec!["A".to_string(), "B".to_string()],
                    id: "AB1".to_string(),
                    label: Some("OpenAPI".to_string()),
                },
            ]
        );
    }

    #[test]
    fn parses_explicit_text_statement_and_numeric_text_id() {
        let model = parse(
            r#"venn-beta
set A
set B
union A,B
text A alpha["Alpha note"]
text A,B 42
      "#,
        );

        assert_eq!(
            model.text_nodes,
            vec![
                VennTextNodeModel {
                    sets: vec!["A".to_string()],
                    id: "alpha".to_string(),
                    label: Some("Alpha note".to_string()),
                },
                VennTextNodeModel {
                    sets: vec!["A".to_string(), "B".to_string()],
                    id: "42".to_string(),
                    label: None,
                },
            ]
        );
    }

    #[test]
    fn parses_style_entries() {
        let model = parse(
            r#"venn-beta
          set A
          set B
          union A,B
          style A fill:#ff6b6b, color:#333
          style A,B fill:rgb(255, 0, 128)
          style B fill:rgba(255, 0, 128, 0.5)
      "#,
        );

        assert_eq!(model.style_entries[0].targets, ["A"]);
        assert_eq!(
            model.style_entries[0]
                .styles
                .get("fill")
                .map(String::as_str),
            Some("#ff6b6b")
        );
        assert_eq!(
            model.style_entries[0]
                .styles
                .get("color")
                .map(String::as_str),
            Some("#333")
        );
        assert_eq!(model.style_entries[1].targets, ["A", "B"]);
        assert_eq!(
            model.style_entries[1]
                .styles
                .get("fill")
                .map(String::as_str),
            Some("rgb(255, 0, 128)")
        );
        assert_eq!(
            model.style_entries[2]
                .styles
                .get("fill")
                .map(String::as_str),
            Some("rgba(255, 0, 128, 0.5)")
        );
    }

    #[test]
    fn rejects_invalid_set_and_union_shapes() {
        let err = parse_venn_model_for_render("venn-beta\nset A,B\n", &meta()).unwrap_err();
        assert!(err.to_string().contains("set requires single identifier"));

        let err = parse_venn_model_for_render("venn-beta\nunion A\n", &meta()).unwrap_err();
        assert!(
            err.to_string()
                .contains("union requires multiple identifiers")
        );

        let err = parse_venn_model_for_render("venn-beta\nset Foo\nunion Foo,Buz\n", &meta())
            .unwrap_err();
        assert!(err.to_string().contains("unknown set identifier"));
    }

    #[test]
    fn parses_quoted_identifiers_and_sorts_union_sets() {
        let model = parse(
            r#"venn-beta
        set "Foo Bar"
        set Buz
        union "Foo Bar",Buz
    "#,
        );

        assert_eq!(model.subsets[0].sets, ["Foo Bar"]);
        assert_eq!(model.subsets[2].sets, ["Buz", "Foo Bar"]);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn parse_venn_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
        let text = r##"venn-beta
title Product Surface
set A["Core"]:20
set B["Editor"]:14
union A,B["Shared"]:4
  text A1["Nested note"]
  text A1["Nested note"]
text A alpha["Alpha note"]
style A fill:#ff6b6b, color:#101010
style A,B fill:#00ffcc, color:#003333
"##;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("venn", text)
            .unwrap()
            .unwrap();

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        for prefix in ["title", "set", "union", "text", "style"] {
            assert!(
                facts.directive_prefixes.iter().any(|value| value == prefix),
                "missing directive prefix {prefix}"
            );
        }

        let symbol_at = |name: &str, detail: &str, start: usize| {
            facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.name == name
                        && symbol.detail.as_deref() == Some(detail)
                        && symbol.selection.start == start
                })
                .unwrap_or_else(|| panic!("missing symbol {name} with detail {detail} at {start}"))
        };

        let title_start = text.find("Product Surface").unwrap();
        let title = symbol_at("Product Surface", "venn title", title_start);
        assert_eq!(title.role, EditorSemanticRole::Payload);
        assert_eq!(title.kind, EditorSemanticKind::String);
        assert_eq!(title.selection.end, title_start + "Product Surface".len());

        let set_a_start = text.find("A[\"Core\"]").unwrap();
        let set_a = symbol_at("A", "venn set", set_a_start);
        assert_eq!(set_a.role, EditorSemanticRole::Entity);
        assert_eq!(set_a.kind, EditorSemanticKind::Namespace);
        assert_eq!(set_a.selection.end, set_a_start + "A".len());

        let set_label_start = text.find("Core").unwrap();
        let set_label = symbol_at("Core", "venn set label", set_label_start);
        assert_eq!(set_label.role, EditorSemanticRole::Payload);
        assert_eq!(set_label.kind, EditorSemanticKind::String);
        assert_eq!(set_label.selection.start, set_label_start);
        assert_eq!(set_label.selection.end, set_label_start + "Core".len());

        let union_a_start = text.find("union A,B").unwrap() + "union ".len();
        let union_a = symbol_at("A", "venn union set", union_a_start);
        assert_eq!(union_a.role, EditorSemanticRole::Reference);
        assert_eq!(union_a.kind, EditorSemanticKind::Namespace);

        let text_set_start = text.find("text A alpha").unwrap() + "text ".len();
        let text_set = symbol_at("A", "venn text set", text_set_start);
        assert_eq!(text_set.role, EditorSemanticRole::Reference);
        assert_eq!(text_set.kind, EditorSemanticKind::Namespace);

        let text_id_start = text.find("alpha").unwrap();
        let text_id = symbol_at("alpha", "venn text node", text_id_start);
        assert_eq!(text_id.role, EditorSemanticRole::Entity);
        assert_eq!(text_id.kind, EditorSemanticKind::Namespace);
        assert_eq!(text_id.selection.end, text_id_start + "alpha".len());

        let text_note_start = text.find("Alpha note").unwrap();
        let text_note = symbol_at("Alpha note", "venn text label", text_note_start);
        assert_eq!(text_note.role, EditorSemanticRole::Payload);

        let style_target_start = text.find("style A fill").unwrap() + "style ".len();
        let style_target = symbol_at("A", "venn style target", style_target_start);
        assert_eq!(style_target.role, EditorSemanticRole::Reference);
        assert_eq!(style_target.kind, EditorSemanticKind::Namespace);

        let fill_start = text.find("#ff6b6b").unwrap();
        let fill = symbol_at("#ff6b6b", "venn style value", fill_start);
        assert_eq!(fill.role, EditorSemanticRole::Payload);
        assert_eq!(fill.kind, EditorSemanticKind::String);
        assert_eq!(fill.selection.start, fill_start);

        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
                && expected.span == SourceSpan::new(set_a_start, set_a_start + "A".len())
        }));
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::IdList
                && expected.span.start <= union_a_start
                && expected.span.end >= union_a_start + "A".len()
        }));
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span.start <= title_start
                && expected.span.end >= title_start + "Product Surface".len()
        }));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn venn_reference_roles_do_not_promote_occurrences_to_set_definitions() {
        let text = concat!(
            "venn-beta\n",
            "style Future fill:blue\n",
            "set Future\n",
            "set A\n",
            "set B\n",
            "union A,B\n",
            "text A note[\"Note\"]\n",
            "style A fill:red\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_venn_json_and_editor_facts,
            text,
            &meta(),
        );

        let a_roles: Vec<_> = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "A")
            .map(|symbol| symbol.role)
            .collect();
        assert_eq!(
            a_roles,
            [
                EditorSemanticRole::Entity,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
            ]
        );
        assert!(
            facts.symbols.iter().any(|symbol| {
                symbol.name == "note" && symbol.role == EditorSemanticRole::Entity
            })
        );
        let future_roles: Vec<_> = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "Future")
            .map(|symbol| symbol.role)
            .collect();
        assert_eq!(
            future_roles,
            [EditorSemanticRole::Reference, EditorSemanticRole::Entity]
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn venn_combined_parse_constructs_once_and_preserves_projections() {
        let text = r##"venn-beta
title "Product overlap"
set "Frontend Team"["Frontend"]:.5
set Backend:12
union "Frontend Team",Backend["Shared"]:3
  text shared_note["API contracts"]
style "Frontend Team",Backend fill:rgba(255, 0, 128, 0.5), color:#101010
"##;
        let meta = meta();

        crate::diagrams::langium_common::reset_family_syntax_construction_count("venn");
        let (combined_json, combined_editor) = crate::family::test_support::into_result(
            parse_venn_json_and_editor_facts(text, &meta, &crate::OperationControl::new()),
        )
        .unwrap();
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("venn"),
            1,
            "one combined request must construct Venn syntax once"
        );

        assert_eq!(combined_json, parse_venn(text, &meta).unwrap());
        assert!(!combined_editor.symbols.is_empty());
    }

    #[test]
    fn venn_typed_and_json_projections_share_the_same_semantics() {
        let text = r#"venn-beta
title "Product overlap"
set A["Frontend"]:20
set B["Backend"]:12
union A,B["Shared"]:3
  text note["API contracts"]
style A,B fill:#ff6b6b, color:red
"#;

        let compat = parse_venn(text, &meta()).unwrap();
        let typed = parse_venn_model_for_render(text, &meta()).unwrap();

        assert_eq!(
            render_model_to_compat_json(&typed, &meta()).unwrap(),
            compat
        );
        assert_eq!(typed.title.as_deref(), Some("\"Product overlap\""));
        assert_eq!(compat["title"], json!(typed.title));
        assert_eq!(compat["accTitle"], json!(typed.acc_title));
        assert_eq!(compat["accDescr"], json!(typed.acc_descr));
        assert_eq!(compat["subsets"], json!(typed.subsets));
        assert_eq!(compat["textNodes"], json!(typed.text_nodes));
        assert_eq!(compat["styleEntries"], json!(typed.style_entries));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn venn_editor_projection_preserves_quoted_and_numeric_token_spans() {
        let text = "venn-beta\nset \"Frontend Team\"[\"Core\"]:.5\ntext \"Frontend Team\" 42\n";
        let facts = crate::family::test_support::editor_facts(
            parse_venn_json_and_editor_facts,
            text,
            &meta(),
        );

        let set_raw_start = text.find("\"Frontend Team\"").unwrap();
        let set = facts
            .symbols
            .iter()
            .find(|symbol| {
                symbol.name == "Frontend Team" && symbol.detail.as_deref() == Some("venn set")
            })
            .expect("quoted set fact");
        assert_eq!(
            set.span,
            SourceSpan::new(set_raw_start, set_raw_start + "\"Frontend Team\"".len())
        );
        assert_eq!(
            set.selection,
            SourceSpan::new(set_raw_start + 1, set_raw_start + "\"Frontend Team".len())
        );

        for (name, detail) in [(".5", "venn size"), ("42", "venn text node")] {
            let start = text.find(name).unwrap();
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
                .unwrap_or_else(|| panic!("missing {detail} fact"));
            assert_eq!(symbol.span, SourceSpan::new(start, start + name.len()));
            assert_eq!(symbol.selection, symbol.span);
        }
    }

    #[test]
    fn venn_malformed_statement_recovers_prior_facts_with_the_strict_error_span() {
        let text = "venn-beta\nset A[Alpha]\nunion A,\n";
        let statement_start = text.find("union A,").unwrap();
        let statement_span = SourceSpan::new(statement_start, statement_start + "union A,".len());

        let error = parse_venn(text, &meta()).expect_err("strict parse must reject the union");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured Venn parse error");
        };
        assert!(diagnostic.message().contains("expected identifier"));
        assert_eq!(diagnostic.span(), Some(statement_span));
        assert_eq!(
            diagnostic.span_kind(),
            crate::ParseDiagnosticSpanKind::Exact
        );

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_venn_json_and_editor_facts,
                text,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "A" && symbol.detail.as_deref() == Some("venn set")
            }));
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("expected identifier")
                    && diagnostic.span == Some(statement_span)
            }));
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn venn_recovery_preserves_later_semantic_facts_with_crlf_spans() {
        let text = concat!(
            "venn-beta\r\n",
            "  union A, %% missing identifier\r\n",
            "set Later[\"ok\"]:12\r\n",
            "title Done\r\n",
        );
        let invalid = "union A,";
        let invalid_start = text.find(invalid).unwrap();
        let facts = crate::family::test_support::editor_facts(
            parse_venn_json_and_editor_facts,
            text,
            &meta(),
        );

        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Later" && symbol.detail.as_deref() == Some("venn set")
        }));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span
                    == Some(SourceSpan::new(
                        invalid_start,
                        invalid_start + invalid.len(),
                    ))
        }));
    }

    #[test]
    fn venn_numeric_and_color_edges_follow_the_upstream_jison_grammar() {
        let model = parse("venn-beta\nset A:.5\nset B:+2\nunion A,B:-.25\n");
        assert_eq!(model.subsets[0].size, 0.5);
        assert_eq!(model.subsets[1].size, 2.0);
        assert_eq!(model.subsets[2].size, -0.25);

        let invalid_number = parse_venn("venn-beta\nset A:1.\n", &meta()).unwrap_err();
        assert!(
            invalid_number
                .to_string()
                .contains("unexpected trailing venn tokens")
        );

        let invalid_rgb =
            parse_venn("venn-beta\nset A\nstyle A fill:rgb(red, 0, 0)\n", &meta()).unwrap_err();
        assert!(invalid_rgb.to_string().contains("expected style value"));
    }

    #[test]
    fn render_model_entrypoint_returns_typed_venn_model() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "venn-beta\nset A\nset B\nunion A,B\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(parsed.metadata().diagram_type, "venn");
        let RenderSemanticModel::Venn(model) = parsed.model() else {
            panic!("expected Venn render model");
        };
        assert_eq!(model.subsets.len(), 3);
        assert_eq!(model.subsets[2].sets, ["A", "B"]);
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    fn venn_adjacent_recovery_bounds_failed_payload_lookahead() {
        for count in [256, 2048] {
            for (opener, suffix) in [("[[", ""), ("[[", "\""), ("[\"", ""), ("rgb(", "")] {
                let source = format!(
                    "venn-beta\nset A {}{suffix}\nset Later\n",
                    opener.repeat(count),
                );
                let mut cursor = VennCursor::new(&source);
                while cursor.peek().kind != VennTokenKind::End {
                    cursor.take();
                }
                assert!(
                    cursor.inspected_terminal_bytes <= source.len() * 2,
                    "payload lookahead rescanned a suffix: {opener:?}, {count}, {} bytes inspected",
                    cursor.inspected_terminal_bytes,
                );
                let outcome = construct_venn_parse_outcome_controlled(
                    &source,
                    "venn",
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Capture,
                    &crate::OperationControl::new(),
                )
                .unwrap();
                assert!(outcome.first_error.is_some());
                assert_eq!(outcome.source.model.subsets.last().unwrap().sets, ["Later"]);
            }
        }
    }

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "venn".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": "strict",
                "htmlLabels": true,
                "dompurifyConfig": { "FORBID_TAGS": ["b"] }
            })),
            title: None,
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn recovery_capture_preserves_venn_error_without_state_observer_facts() {
        let source = "venn-beta\ntitle Example\nset A\nunion A,\nset B\n";
        let capture = |demand| {
            parse_venn_semantic_model_controlled(
                source,
                &metadata(),
                demand,
                &crate::OperationControl::new(),
            )
            .unwrap()
        };
        let full = capture(EditorFactsDemand::Capture);
        let full_facts = full.editor_facts.unwrap();
        assert!(!full_facts.symbols.is_empty());
        crate::editor::test_work::reset();
        crate::diagrams::langium_common::reset_family_syntax_construction_count("venn");
        let recovery = capture(EditorFactsDemand::Recovery);
        assert_eq!(
            recovery.model.unwrap_err().to_string(),
            full.model.unwrap_err().to_string()
        );
        let facts = recovery.editor_facts.unwrap();
        assert_eq!(facts.diagnostics, full_facts.diagnostics);
        assert!(!facts.diagnostics.is_empty());
        assert!(facts.symbols.is_empty());
        assert!(facts.directive_prefixes.is_empty());
        assert!(facts.expected_syntax.is_empty());
        let work = crate::editor::test_work::read();
        assert_eq!(
            (work.symbols, work.directives, work.expected_syntax),
            (0, 0, 0)
        );
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("venn"),
            1
        );
    }

    #[test]
    fn semantic_subsets_text_and_styles_construct_once_without_editor_work() {
        let source = concat!(
            "venn-beta\n",
            "title <b>Overlap</b>\n",
            "set B[\"Backend\"]:12set A[\"Frontend\"]:20",
            "union B,A[\"Shared\"]:3\n",
            "  text note[\"API contracts\"]\n",
            "style B,A fill:#ff0000, color:blue\n",
        );
        crate::diagrams::langium_common::reset_family_syntax_construction_count("venn");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_venn_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(model.title.as_deref(), Some("Overlap"));
        assert_eq!(model.subsets.len(), 3);
        assert_eq!(model.subsets[2].sets, ["A", "B"]);
        assert_eq!(model.subsets[2].size, 3.0);
        assert_eq!(model.subsets[2].label.as_deref(), Some("Shared"));
        assert_eq!(model.text_nodes[0].id, "note");
        assert_eq!(model.text_nodes[0].sets, ["A", "B"]);
        assert_eq!(model.text_nodes[0].label.as_deref(), Some("API contracts"));
        assert_eq!(model.style_entries[0].targets, ["A", "B"]);
        assert_eq!(model.style_entries[0].styles["fill"], "#ff0000");
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("venn"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default()
        );
    }

    #[test]
    fn semantic_union_error_retains_the_exact_crlf_statement_span_without_editor_work() {
        let source = "venn-beta\r\nset A\r\n  union A, %% missing identifier\r\nset Later\r\n";
        let start = source.find("union A,").unwrap();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_venn_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a Venn parse error");
        };
        assert!(diagnostic.message().contains("expected identifier"));
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "union A,".len()))
        );
        assert_eq!(
            diagnostic.span_kind(),
            crate::ParseDiagnosticSpanKind::Exact
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default()
        );
    }

    #[test]
    fn semantic_entry_can_cancel_between_statements() {
        let source = "venn-beta\nset A set B set C set D\n";
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(4);
        assert!(matches!(
            parse_venn_semantic_model_controlled(
                source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn full_typed_and_json_requests_omit_editor_observers() {
        let meta = crate::SourcePresentationMetadata {
            diagram_type: "venn".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let source = "venn-beta\nset A\nset B\nunion A,B\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        parse_venn(source, &meta).unwrap();
        parse_venn_model_for_render_controlled(source, &meta, &crate::OperationControl::new())
            .unwrap()
            .unwrap();
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default(),
        );
    }
}
