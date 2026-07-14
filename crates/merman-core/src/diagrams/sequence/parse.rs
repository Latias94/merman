use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
    editor::{format_lalrpop_parse_error, lalrpop_parse_diagnostic, lalrpop_recovery_span},
};
use serde_json::Value;
#[cfg(test)]
use std::cell::Cell;

use super::SequenceDiagramRenderModel;
use super::Tok;
use super::db::{SequenceDb, is_css_color_value, split_box_color_and_title};
use super::lexer::Lexer;
use super::sequence_grammar;

#[cfg(test)]
thread_local! {
    static SEQUENCE_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_sequence_syntax_construction_count() {
    SEQUENCE_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn sequence_syntax_construction_count() -> usize {
    SEQUENCE_SYNTAX_CONSTRUCTION_COUNT.get()
}

type SequenceLexicalEvent = std::result::Result<(usize, Tok, usize), super::LexError>;
type SequenceGrammarError = lalrpop_util::ParseError<usize, Tok, super::LexError>;

struct SequenceSyntax {
    events: Vec<SequenceLexicalEvent>,
}

impl SequenceSyntax {
    fn lex(code: &str) -> Self {
        #[cfg(test)]
        SEQUENCE_SYNTAX_CONSTRUCTION_COUNT.set(SEQUENCE_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

        let mut events = Vec::new();
        let mut lexer = Lexer::new(code);
        let mut last_position = lexer.position();
        while let Some(event) = lexer.next() {
            let current_position = lexer.position();
            let must_stop = event.is_err() && current_position == last_position;
            events.push(event);
            if must_stop {
                break;
            }
            last_position = current_position;
        }

        Self { events }
    }

    fn editor_facts(&self, code: &str) -> EditorSemanticFacts {
        collect_sequence_editor_facts_from_events(&self.events, code)
    }

    fn into_actions(self) -> std::result::Result<Vec<super::Action>, SequenceGrammarError> {
        sequence_grammar::ActionsParser::new().parse(self.events)
    }
}

struct SequenceSemanticSource {
    db: SequenceDb,
    editor_facts: EditorSemanticFacts,
}

enum SequenceSemanticFailure {
    Grammar {
        error: SequenceGrammarError,
        editor_facts: EditorSemanticFacts,
    },
    Db {
        message: String,
        editor_facts: EditorSemanticFacts,
    },
}

impl SequenceSemanticFailure {
    fn into_parse_error(self, meta: &ParseMetadata, fallback_offset: usize) -> Error {
        match self {
            Self::Grammar { error, .. } => Error::diagram_parse_diagnostic(
                meta.diagram_type.clone(),
                lalrpop_parse_diagnostic(&error, fallback_offset),
            ),
            Self::Db { message, .. } => {
                Error::diagram_parse_fallback(meta.diagram_type.clone(), message)
            }
        }
    }

    fn into_editor_facts(self, fallback_offset: usize) -> EditorSemanticFacts {
        match self {
            Self::Grammar {
                error,
                mut editor_facts,
            } => {
                let span = match &error {
                    lalrpop_util::ParseError::User { error } => error.span,
                    _ => lalrpop_recovery_span(&error, fallback_offset),
                };
                editor_facts.mark_recovered_from_parse_error(
                    format!(
                        "sequence parser recovered after parse error: {}",
                        format_lalrpop_parse_error(&error)
                    ),
                    Some(span),
                );
                editor_facts
            }
            Self::Db {
                message,
                mut editor_facts,
            } => {
                editor_facts.mark_recovered_from_parse_error(
                    format!("sequence semantic construction failed: {message}"),
                    None,
                );
                editor_facts
            }
        }
    }
}

pub fn parse_sequence(code: &str, meta: &ParseMetadata) -> Result<Value> {
    Ok(parse_sequence_semantic_source(code, meta)?
        .db
        .into_model(meta))
}

pub fn parse_sequence_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<SequenceDiagramRenderModel> {
    Ok(parse_sequence_semantic_source(code, meta)?
        .db
        .into_render_model())
}

pub(crate) fn parse_sequence_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let SequenceSemanticSource { db, editor_facts } = parse_sequence_semantic_source(code, meta)?;
    Ok((db.into_model(meta), editor_facts))
}

pub fn parse_sequence_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    match construct_sequence_semantic_source(code, sequence_wrap_enabled(meta)) {
        Ok(source) => source.editor_facts,
        Err(failure) => (*failure).into_editor_facts(code.len()),
    }
}

fn parse_sequence_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> Result<SequenceSemanticSource> {
    construct_sequence_semantic_source(code, sequence_wrap_enabled(meta))
        .map_err(|failure| (*failure).into_parse_error(meta, code.len()))
}

fn construct_sequence_semantic_source(
    code: &str,
    wrap_enabled: Option<bool>,
) -> std::result::Result<SequenceSemanticSource, Box<SequenceSemanticFailure>> {
    let syntax = SequenceSyntax::lex(code);
    let editor_facts = syntax.editor_facts(code);
    let actions = match syntax.into_actions() {
        Ok(actions) => actions,
        Err(error) => {
            return Err(Box::new(SequenceSemanticFailure::Grammar {
                error,
                editor_facts,
            }));
        }
    };

    let mut db = SequenceDb::new(wrap_enabled);
    for action in actions {
        if let Err(message) = db.apply(action) {
            return Err(Box::new(SequenceSemanticFailure::Db {
                message,
                editor_facts,
            }));
        }
    }

    Ok(SequenceSemanticSource { db, editor_facts })
}

fn sequence_wrap_enabled(meta: &ParseMetadata) -> Option<bool> {
    meta.effective_config
        .as_value()
        .get("wrap")
        .and_then(|v| v.as_bool())
        .or_else(|| {
            meta.effective_config
                .as_value()
                .get("sequence")
                .and_then(|v| v.get("wrap"))
                .and_then(|v| v.as_bool())
        })
}

fn collect_sequence_editor_facts_from_events(
    events: &[SequenceLexicalEvent],
    code: &str,
) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut collector = SequenceEditorFactCollector::default();

    for event in events {
        match event {
            Ok((start, token, end)) => collector.accept(token, *start, *end, code, &mut facts),
            Err(_) => facts.mark_recovered(),
        }
    }

    facts
}

#[derive(Debug, Default)]
struct SequenceEditorFactCollector {
    expected_actor: Option<ExpectedSequenceActor>,
    expected_text: Option<ExpectedSequenceText>,
    expected_rest_of_line: Option<ExpectedSequenceRestOfLine>,
    pending_message_source: Option<PendingSequenceActor>,
}

#[derive(Debug)]
struct PendingSequenceActor {
    name: String,
    span: SourceSpan,
}

#[derive(Debug, Clone, Copy)]
enum ExpectedSequenceActor {
    Participant,
    Actor,
    MessageTarget,
    NoteActor,
    InteractionTarget,
}

#[derive(Debug, Clone, Copy)]
enum ExpectedSequenceText {
    Message,
    Note,
    Interaction,
    ParticipantLabel,
    FragmentLabel,
}

#[derive(Debug, Clone, Copy)]
enum ExpectedSequenceRestOfLine {
    BoxLabel,
    Text(ExpectedSequenceText),
}

impl SequenceEditorFactCollector {
    fn accept(
        &mut self,
        token: &Tok,
        start: usize,
        end: usize,
        code: &str,
        facts: &mut EditorSemanticFacts,
    ) {
        match token {
            Tok::SequenceDiagram | Tok::Newline => self.reset_line_state(),
            Tok::Participant => self.expect_actor(ExpectedSequenceActor::Participant),
            Tok::ActorKw => self.expect_actor(ExpectedSequenceActor::Actor),
            Tok::Create | Tok::Destroy => self.expect_actor(ExpectedSequenceActor::Participant),
            Tok::Activate | Tok::Deactivate => {
                self.expect_actor(ExpectedSequenceActor::InteractionTarget)
            }
            Tok::Links => {
                facts.push_directive_prefix("links");
                self.expect_actor(ExpectedSequenceActor::InteractionTarget);
            }
            Tok::Link => {
                facts.push_directive_prefix("link");
                self.expect_actor(ExpectedSequenceActor::InteractionTarget);
            }
            Tok::Properties => {
                facts.push_directive_prefix("properties");
                self.expect_actor(ExpectedSequenceActor::InteractionTarget);
            }
            Tok::Details => {
                facts.push_directive_prefix("details");
                self.expect_actor(ExpectedSequenceActor::InteractionTarget);
            }
            Tok::Note => {
                self.pending_message_source = None;
            }
            Tok::LeftOf | Tok::RightOf | Tok::Over | Tok::Comma => {
                self.expect_actor(ExpectedSequenceActor::NoteActor);
            }
            Tok::Box => {
                self.expected_rest_of_line = Some(ExpectedSequenceRestOfLine::BoxLabel);
            }
            Tok::As => {
                self.expected_rest_of_line = Some(ExpectedSequenceRestOfLine::Text(
                    ExpectedSequenceText::ParticipantLabel,
                ));
            }
            Tok::Loop
            | Tok::Rect
            | Tok::Opt
            | Tok::Alt
            | Tok::Else
            | Tok::Par
            | Tok::ParOver
            | Tok::And
            | Tok::Critical
            | Tok::Option
            | Tok::Break => {
                self.expected_rest_of_line = Some(ExpectedSequenceRestOfLine::Text(
                    ExpectedSequenceText::FragmentLabel,
                ));
            }
            Tok::SignalType(_) => {
                self.push_pending_message_source(facts);
                self.expect_actor(ExpectedSequenceActor::MessageTarget);
            }
            Tok::Actor(name) => {
                if let Some(expected) = self.expected_actor {
                    self.push_actor(name.clone(), expected, start, end, facts);
                } else {
                    self.pending_message_source = Some(PendingSequenceActor {
                        name: name.clone(),
                        span: SourceSpan::new(start, end),
                    });
                }
            }
            Tok::RestOfLine(text) => match self.expected_rest_of_line.take() {
                Some(ExpectedSequenceRestOfLine::BoxLabel) => {
                    push_sequence_box_symbol(text.clone(), start, end, code, facts);
                }
                Some(ExpectedSequenceRestOfLine::Text(expected)) => {
                    push_sequence_text_payload(text.clone(), expected, start, end, code, facts);
                }
                None => {}
            },
            Tok::Text(text) => {
                if let Some(expected) = self.expected_text.take() {
                    push_sequence_text_payload(text.clone(), expected, start, end, code, facts);
                }
            }
            Tok::Title(text) | Tok::CompatTitle(text) => {
                facts.push_directive_prefix("title");
                push_sequence_named_payload(
                    text.clone(),
                    "sequence title",
                    start,
                    end,
                    code,
                    facts,
                );
            }
            Tok::AccTitle(text) => {
                facts.push_directive_prefix("accTitle");
                push_sequence_named_payload(
                    text.clone(),
                    "sequence accessibility title",
                    start,
                    end,
                    code,
                    facts,
                );
            }
            Tok::AccDescr(text) | Tok::AccDescrMultiline(text) => {
                facts.push_directive_prefix("accDescr");
                push_sequence_named_payload(
                    text.clone(),
                    "sequence accessibility description",
                    start,
                    end,
                    code,
                    facts,
                );
            }
            Tok::End
            | Tok::Autonumber
            | Tok::Off
            | Tok::Plus
            | Tok::Minus
            | Tok::Central
            | Tok::Num(_)
            | Tok::Config(_) => {}
        }
    }

    fn reset_line_state(&mut self) {
        self.expected_actor = None;
        self.expected_text = None;
        self.expected_rest_of_line = None;
        self.pending_message_source = None;
    }

    fn expect_actor(&mut self, expected: ExpectedSequenceActor) {
        self.expected_actor = Some(expected);
    }

    fn push_actor(
        &mut self,
        name: String,
        expected: ExpectedSequenceActor,
        start: usize,
        end: usize,
        facts: &mut EditorSemanticFacts,
    ) {
        let kind = match expected {
            ExpectedSequenceActor::Actor => EditorSemanticKind::Variable,
            ExpectedSequenceActor::Participant
            | ExpectedSequenceActor::MessageTarget
            | ExpectedSequenceActor::NoteActor
            | ExpectedSequenceActor::InteractionTarget => EditorSemanticKind::Event,
        };
        let detail = match expected {
            ExpectedSequenceActor::Actor => "sequence actor",
            ExpectedSequenceActor::Participant => "sequence participant",
            ExpectedSequenceActor::MessageTarget => "sequence participant reference",
            ExpectedSequenceActor::NoteActor => "sequence note participant",
            ExpectedSequenceActor::InteractionTarget => "sequence participant reference",
        };
        let span = SourceSpan::new(start, end);
        facts.push_symbol(EditorSemanticSymbol::new(
            name.clone(),
            Some(detail.to_string()),
            kind,
            span,
            span,
        ));
        self.expected_text = match expected {
            ExpectedSequenceActor::MessageTarget => Some(ExpectedSequenceText::Message),
            ExpectedSequenceActor::NoteActor => Some(ExpectedSequenceText::Note),
            ExpectedSequenceActor::InteractionTarget => Some(ExpectedSequenceText::Interaction),
            ExpectedSequenceActor::Participant | ExpectedSequenceActor::Actor => None,
        };
        self.expected_actor = None;
    }

    fn push_pending_message_source(&mut self, facts: &mut EditorSemanticFacts) {
        if let Some(actor) = self.pending_message_source.take() {
            facts.push_symbol(EditorSemanticSymbol::new(
                actor.name,
                Some("sequence participant reference".to_string()),
                EditorSemanticKind::Event,
                actor.span,
                actor.span,
            ));
        }
    }
}

fn push_sequence_text_payload(
    text: String,
    expected: ExpectedSequenceText,
    start: usize,
    end: usize,
    code: &str,
    facts: &mut EditorSemanticFacts,
) {
    let detail = match expected {
        ExpectedSequenceText::Message => "sequence message",
        ExpectedSequenceText::Note => "sequence note",
        ExpectedSequenceText::Interaction => "sequence interaction payload",
        ExpectedSequenceText::ParticipantLabel => "sequence participant label",
        ExpectedSequenceText::FragmentLabel => "sequence fragment label",
    };
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        SourceSpan::new(start, end),
    ));
    push_sequence_named_payload(text, detail, start, end, code, facts);
}

fn push_sequence_named_payload(
    text: String,
    detail: &str,
    start: usize,
    end: usize,
    code: &str,
    facts: &mut EditorSemanticFacts,
) {
    let Some(selection) = sequence_payload_selection(&text, start, end, code) else {
        return;
    };
    facts.push_symbol(EditorSemanticSymbol::payload(
        text,
        Some(detail.to_string()),
        EditorSemanticKind::String,
        SourceSpan::new(start, end),
        selection,
    ));
}

fn sequence_payload_selection(
    text: &str,
    start: usize,
    end: usize,
    code: &str,
) -> Option<SourceSpan> {
    if text.is_empty() {
        return None;
    }
    let raw = code.get(start..end)?;
    let local_start = raw.rfind(text)?;
    Some(SourceSpan::new(
        start + local_start,
        start + local_start + text.len(),
    ))
}

fn push_sequence_box_symbol(
    text: String,
    start: usize,
    end: usize,
    code: &str,
    facts: &mut EditorSemanticFacts,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        SourceSpan::new(start, end),
    ));
    let Some((name, selection)) = sequence_box_name_and_selection(&text, start, end, code) else {
        return;
    };
    facts.push_symbol(EditorSemanticSymbol::payload(
        name,
        Some("sequence box".to_string()),
        EditorSemanticKind::String,
        SourceSpan::new(start, end),
        selection,
    ));
}

fn sequence_box_name_and_selection(
    text: &str,
    start: usize,
    end: usize,
    code: &str,
) -> Option<(String, SourceSpan)> {
    let raw = code.get(start..end).unwrap_or(text);
    let leading = raw.len().saturating_sub(raw.trim_start().len());
    let trailing = raw.trim_end().len();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (color, title_candidate) = split_box_color_and_title(trimmed);
    let title_start = if is_css_color_value(color) {
        trimmed.len().saturating_sub(title_candidate.len())
    } else {
        0
    };
    let title = trimmed[title_start..].trim();
    if title.is_empty() {
        return None;
    }

    let local_start = leading + title_start + trimmed[title_start..].len()
        - trimmed[title_start..].trim_start().len();
    let local_end = start + trailing;
    Some((
        title.to_string(),
        SourceSpan::new(start + local_start, local_end),
    ))
}
