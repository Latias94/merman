use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
    editor::{format_lalrpop_parse_error, lalrpop_recovery_span},
};
use serde_json::Value;

use super::SequenceDiagramRenderModel;
use super::Tok;
use super::db::{SequenceDb, fast_parse_sequence_signals_only_db};
use super::lexer::Lexer;
use super::sequence_grammar;

pub fn parse_sequence(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let db = parse_sequence_db(code, meta)?;
    Ok(db.into_model(meta))
}

pub fn parse_sequence_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<SequenceDiagramRenderModel> {
    let db = parse_sequence_db(code, meta)?;
    Ok(db.into_render_model())
}

pub fn parse_sequence_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    let parse_result = sequence_grammar::ActionsParser::new().parse(Lexer::new(code));
    let mut facts = collect_sequence_editor_facts_from_tokens(code);
    if let Err(error) = parse_result {
        let span = lalrpop_recovery_span(&error, code.len());
        facts.mark_recovered_with_diagnostic(
            format!(
                "sequence parser recovered after parse error: {}",
                format_lalrpop_parse_error(&error)
            ),
            Some(span),
        );
    }

    facts
}

fn parse_sequence_db(code: &str, meta: &ParseMetadata) -> Result<SequenceDb> {
    let wrap_enabled = meta
        .effective_config
        .as_value()
        .get("wrap")
        .and_then(|v| v.as_bool())
        .or_else(|| {
            meta.effective_config
                .as_value()
                .get("sequence")
                .and_then(|v| v.get("wrap"))
                .and_then(|v| v.as_bool())
        });

    if let Some(db) = fast_parse_sequence_signals_only_db(code, wrap_enabled) {
        return Ok(db);
    }

    let actions = sequence_grammar::ActionsParser::new()
        .parse(Lexer::new(code))
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format_lalrpop_parse_error(&e),
        })?;

    let mut db = SequenceDb::new(wrap_enabled);
    for a in actions {
        db.apply(a).map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: e,
        })?;
    }

    Ok(db)
}

fn collect_sequence_editor_facts_from_tokens(code: &str) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut collector = SequenceEditorFactCollector::default();

    let mut lexer = Lexer::new(code);
    while let Some(result) = lexer.next() {
        match result {
            Ok((start, token, end)) => collector.accept(token, start, end, code, &mut facts),
            Err(_) => {
                facts.mark_recovered();
            }
        }
    }

    facts
}

#[derive(Debug, Default)]
struct SequenceEditorFactCollector {
    expected_actor: Option<ExpectedSequenceActor>,
    expected_text: Option<ExpectedSequenceText>,
    pending_message_source: Option<PendingSequenceActor>,
    after_box_keyword: bool,
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
}

impl SequenceEditorFactCollector {
    fn accept(
        &mut self,
        token: Tok,
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
                self.after_box_keyword = true;
            }
            Tok::SignalType(_) => {
                self.push_pending_message_source(facts);
                self.expect_actor(ExpectedSequenceActor::MessageTarget);
            }
            Tok::Actor(name) => {
                if let Some(expected) = self.expected_actor {
                    self.push_actor(name, expected, start, end, facts);
                } else {
                    self.pending_message_source = Some(PendingSequenceActor {
                        name,
                        span: SourceSpan::new(start, end),
                    });
                }
            }
            Tok::RestOfLine(text) => {
                if self.after_box_keyword {
                    push_sequence_box_symbol(text, start, end, code, facts);
                    self.after_box_keyword = false;
                }
            }
            Tok::Text(text) => {
                if let Some(expected) = self.expected_text.take() {
                    push_sequence_text_payload(text, expected, start, end, code, facts);
                }
            }
            Tok::Title(text) | Tok::CompatTitle(text) => {
                facts.push_directive_prefix("title");
                push_sequence_named_payload(text, "sequence title", start, end, code, facts);
            }
            Tok::AccTitle(text) => {
                facts.push_directive_prefix("accTitle");
                push_sequence_named_payload(
                    text,
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
                    text,
                    "sequence accessibility description",
                    start,
                    end,
                    code,
                    facts,
                );
            }
            Tok::End
            | Tok::Loop
            | Tok::Rect
            | Tok::Opt
            | Tok::Alt
            | Tok::Else
            | Tok::Par
            | Tok::ParOver
            | Tok::And
            | Tok::Critical
            | Tok::Option
            | Tok::Break
            | Tok::As
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
        self.pending_message_source = None;
        self.after_box_keyword = false;
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
    let Some((name, selection)) = sequence_box_name_and_selection(&text, start, end, code) else {
        return;
    };
    facts.push_symbol(EditorSemanticSymbol::new(
        name,
        Some("sequence box".to_string()),
        EditorSemanticKind::Package,
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

    let (color, title_candidate) = split_sequence_box_color_and_title(trimmed);
    let title_start = if looks_like_css_color(color) {
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

fn looks_like_css_color(value: &str) -> bool {
    let value = value.trim();
    value.starts_with('#')
        || value.starts_with("rgb(")
        || value.starts_with("rgba(")
        || value.starts_with("hsl(")
        || value.starts_with("hsla(")
        || matches!(
            value,
            "transparent" | "red" | "green" | "blue" | "white" | "black" | "grey" | "gray"
        )
}

fn split_sequence_box_color_and_title(input: &str) -> (&str, &str) {
    let lower = input.to_ascii_lowercase();
    for prefix in ["rgba", "rgb", "hsla", "hsl"] {
        if lower.starts_with(prefix)
            && let Some(end) = input.find(')')
        {
            return (input[..=end].trim(), &input[end + 1..]);
        }
    }

    let mut end = 0usize;
    for (idx, ch) in input.char_indices() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            end = idx + ch.len_utf8();
            continue;
        }
        break;
    }
    (&input[..end], &input[end..])
}
