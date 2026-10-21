#[cfg(feature = "editor-facts")]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::family::SemanticConstruction;
#[cfg(feature = "editor-facts")]
use crate::parse_diagnostic::{format_lalrpop_parse_error, lalrpop_recovery_span};
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{
    Error, MermaidConfig, OperationControl, OperationControlResult, SemanticMetadata,
    parse_diagnostic::lalrpop_parse_diagnostic,
};
#[cfg(test)]
use std::cell::Cell;
#[cfg(feature = "editor-facts")]
use std::collections::{HashMap, hash_map::Entry};

use super::SequenceDiagramModel;
use super::Tok;
use super::ast::{ActionArena, Actions};
use super::db::SequenceDb;
#[cfg(feature = "editor-facts")]
use super::db::{is_css_color_value, split_box_color_and_title};
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

struct SequenceSyntaxOutcome {
    actions: std::result::Result<Actions, SequenceGrammarError>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct SequenceSyntax {
    events: Vec<SequenceLexicalEvent>,
}

impl SequenceSyntax {
    fn lex(code: &str, control: &OperationControl) -> OperationControlResult<Self> {
        #[cfg(test)]
        SEQUENCE_SYNTAX_CONSTRUCTION_COUNT.set(SEQUENCE_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

        let events = crate::diagrams::scan::collect_lexical_events(&mut Lexer::new(code), control)?;
        Ok(Self { events })
    }

    fn into_actions(
        self,
        #[cfg(feature = "editor-facts")] code: &str,
        #[cfg(feature = "editor-facts")] facts_demand: EditorFactsDemand,
        control: &OperationControl,
    ) -> OperationControlResult<SequenceSyntaxOutcome> {
        let Self { events } = self;
        #[cfg(feature = "editor-facts")]
        let editor_facts = match facts_demand {
            EditorFactsDemand::Omit => None,
            EditorFactsDemand::Capture => Some(collect_sequence_editor_facts_from_events(
                &events, code, control,
            )?),
        };
        control.checkpoint()?;
        let mut emitted = 0usize;
        let controlled_events = events.into_iter().take_while(|_| {
            let active = !emitted.is_multiple_of(128) || control.checkpoint().is_ok();
            emitted = emitted.saturating_add(1);
            active
        });
        let mut arena = ActionArena::default();
        let actions = sequence_grammar::ActionsParser::new()
            .parse(&mut arena, control, controlled_events)
            .map(|list| arena.into_actions(list));
        control.checkpoint()?;
        Ok(SequenceSyntaxOutcome {
            actions,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        })
    }
}

pub(super) struct SequenceSemanticSource {
    pub(super) db: SequenceDb,
    #[cfg(feature = "editor-facts")]
    pub(super) editor_facts: Option<EditorSemanticFacts>,
}

pub(super) enum SequenceSemanticFailure {
    Grammar {
        error: SequenceGrammarError,
        #[cfg(feature = "editor-facts")]
        editor_facts: Option<EditorSemanticFacts>,
    },
    Db {
        message: String,
        #[cfg(feature = "editor-facts")]
        editor_facts: Option<EditorSemanticFacts>,
    },
}

impl SequenceSemanticFailure {
    pub(super) fn into_parse_error_for_type(
        self,
        diagram_type: &str,
        fallback_offset: usize,
    ) -> Error {
        match self {
            Self::Grammar {
                error,
                #[cfg(feature = "editor-facts")]
                    editor_facts: _,
            } => Error::diagram_parse_diagnostic(
                diagram_type.to_string(),
                lalrpop_parse_diagnostic(&error, fallback_offset),
            ),
            Self::Db {
                message,
                #[cfg(feature = "editor-facts")]
                    editor_facts: _editor_facts,
            } => Error::diagram_parse_fallback(diagram_type.to_string(), message),
        }
    }

    #[cfg(feature = "editor-facts")]
    pub(super) fn into_error_and_editor_facts_for_type(
        self,
        diagram_type: &str,
        fallback_offset: usize,
    ) -> (Error, EditorSemanticFacts) {
        match self {
            Self::Grammar {
                error,
                editor_facts,
            } => {
                let parse_error = Error::diagram_parse_diagnostic(
                    diagram_type.to_string(),
                    lalrpop_parse_diagnostic(&error, fallback_offset),
                );
                let span = match &error {
                    lalrpop_util::ParseError::User { error } => error.span,
                    _ => lalrpop_recovery_span(&error, fallback_offset),
                };
                let mut editor_facts =
                    editor_facts.expect("Sequence capture requests its failure journal");
                editor_facts.mark_recovered_from_parse_error(
                    format!(
                        "sequence parser recovered after parse error: {}",
                        format_lalrpop_parse_error(&error)
                    ),
                    Some(span),
                );
                (parse_error, editor_facts)
            }
            Self::Db {
                message,
                editor_facts,
            } => {
                let parse_error =
                    Error::diagram_parse_fallback(diagram_type.to_string(), message.clone());
                let mut editor_facts =
                    editor_facts.expect("Sequence capture requests its failure journal");
                editor_facts.mark_recovered_from_parse_error(
                    format!("sequence semantic construction failed: {message}"),
                    None,
                );
                (parse_error, editor_facts)
            }
        }
    }
}

pub(crate) fn parse_sequence_semantic_model_controlled(
    code: &str,
    meta: &SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<SemanticConstruction<SequenceDiagramModel>> {
    let construction = construct_sequence_semantic_source(
        code,
        sequence_wrap_enabled_from_config(&meta.semantic_config),
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(SequenceSemanticSource {
            db,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }) => {
            let mut model = db.into_model();
            model.sanitize_common_db_fields(&meta.semantic_config);
            SemanticConstruction {
                model: Ok(model),
                #[cfg(feature = "editor-facts")]
                editor_facts,
            }
        }
        #[cfg(feature = "editor-facts")]
        Err(failure) if editor_demand == EditorFactsDemand::Capture => {
            let (error, editor_facts) =
                failure.into_error_and_editor_facts_for_type(&meta.diagram_type, code.len());
            SemanticConstruction {
                model: Err(error),
                #[cfg(feature = "editor-facts")]
                editor_facts: Some(editor_facts),
            }
        }
        Err(failure) => SemanticConstruction {
            model: Err(failure.into_parse_error_for_type(&meta.diagram_type, code.len())),
            #[cfg(feature = "editor-facts")]
            editor_facts: None,
        },
    })
}

pub(super) fn construct_sequence_semantic_source(
    code: &str,
    wrap_enabled: Option<bool>,
    #[cfg(feature = "editor-facts")] facts_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<SequenceSemanticSource, Box<SequenceSemanticFailure>>>
{
    let syntax = SequenceSyntax::lex(code, control)?;
    let SequenceSyntaxOutcome {
        actions,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    } = syntax.into_actions(
        #[cfg(feature = "editor-facts")]
        code,
        #[cfg(feature = "editor-facts")]
        facts_demand,
        control,
    )?;
    let actions = match actions {
        Ok(actions) => actions,
        Err(error) => {
            return Ok(Err(Box::new(SequenceSemanticFailure::Grammar {
                error,
                #[cfg(feature = "editor-facts")]
                editor_facts,
            })));
        }
    };

    let db = match build_sequence_db(actions, wrap_enabled, control)? {
        Ok(db) => db,
        Err(message) => {
            return Ok(Err(Box::new(SequenceSemanticFailure::Db {
                message,
                #[cfg(feature = "editor-facts")]
                editor_facts,
            })));
        }
    };

    control.checkpoint()?;
    Ok(Ok(SequenceSemanticSource {
        db,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    }))
}

fn build_sequence_db(
    actions: Actions,
    wrap_enabled: Option<bool>,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<SequenceDb, String>> {
    let mut db = SequenceDb::new(wrap_enabled);
    for (index, action) in actions.into_iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        if let Err(error) = db.apply_controlled(action, control)? {
            return Ok(Err(error));
        }
    }
    Ok(Ok(db))
}

pub(super) fn sequence_wrap_enabled_from_config(config: &MermaidConfig) -> Option<bool> {
    config
        .as_value()
        .get("wrap")
        .and_then(|v| v.as_bool())
        .or_else(|| {
            config
                .as_value()
                .get("sequence")
                .and_then(|v| v.get("wrap"))
                .and_then(|v| v.as_bool())
        })
}

#[cfg(feature = "editor-facts")]
fn collect_sequence_editor_facts_from_events(
    events: &[SequenceLexicalEvent],
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<EditorSemanticFacts> {
    #[cfg(test)]
    crate::editor::test_work::record_collection();
    let mut facts = EditorSemanticFacts::new();
    let mut collector = SequenceEditorFactCollector::default();

    for (index, event) in events.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        match event {
            Ok((start, token, end)) => collector.accept(token, *start, *end, code, &mut facts),
            Err(_) => facts.mark_recovered(),
        }
    }

    control.checkpoint()?;
    Ok(facts)
}

#[derive(Debug, Default)]
#[cfg(feature = "editor-facts")]
struct SequenceEditorFactCollector {
    expected_actor: Option<ExpectedSequenceActor>,
    expected_text: Option<ExpectedSequenceText>,
    expected_rest_of_line: Option<ExpectedSequenceRestOfLine>,
    pending_message_source: Option<PendingSequenceActor>,
    participant_kinds: HashMap<String, EditorSemanticKind>,
}

#[derive(Debug)]
#[cfg(feature = "editor-facts")]
struct PendingSequenceActor {
    name: String,
    span: SourceSpan,
}

#[derive(Debug, Clone, Copy)]
#[cfg(feature = "editor-facts")]
enum ExpectedSequenceActor {
    Participant,
    Actor,
    MessageTarget,
    NoteActor,
    EnsuredInteractionTarget,
    ReferenceOnly,
}

#[derive(Debug, Clone, Copy)]
#[cfg(feature = "editor-facts")]
enum ExpectedSequenceText {
    Message,
    Note,
    Interaction,
    ParticipantLabel,
    FragmentLabel,
}

#[derive(Debug, Clone, Copy)]
#[cfg(feature = "editor-facts")]
enum ExpectedSequenceRestOfLine {
    BoxLabel,
    Text(ExpectedSequenceText),
}

#[cfg(feature = "editor-facts")]
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
            Tok::Create => self.expect_actor(ExpectedSequenceActor::Participant),
            Tok::Destroy => self.expect_actor(ExpectedSequenceActor::ReferenceOnly),
            Tok::Activate | Tok::Deactivate => {
                self.expect_actor(ExpectedSequenceActor::ReferenceOnly)
            }
            Tok::Links => {
                facts.push_directive_prefix("links");
                self.expect_actor(ExpectedSequenceActor::EnsuredInteractionTarget);
            }
            Tok::Link => {
                facts.push_directive_prefix("link");
                self.expect_actor(ExpectedSequenceActor::EnsuredInteractionTarget);
            }
            Tok::Properties => {
                facts.push_directive_prefix("properties");
                self.expect_actor(ExpectedSequenceActor::EnsuredInteractionTarget);
            }
            Tok::Details => {
                facts.push_directive_prefix("details");
                self.expect_actor(ExpectedSequenceActor::EnsuredInteractionTarget);
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
        let declared_kind = match expected {
            ExpectedSequenceActor::Actor => EditorSemanticKind::Variable,
            ExpectedSequenceActor::Participant
            | ExpectedSequenceActor::MessageTarget
            | ExpectedSequenceActor::NoteActor
            | ExpectedSequenceActor::EnsuredInteractionTarget
            | ExpectedSequenceActor::ReferenceOnly => EditorSemanticKind::Event,
        };
        let occurrence_detail = match expected {
            ExpectedSequenceActor::Actor => "sequence actor",
            ExpectedSequenceActor::Participant => "sequence participant",
            ExpectedSequenceActor::MessageTarget => "sequence participant reference",
            ExpectedSequenceActor::NoteActor => "sequence note participant",
            ExpectedSequenceActor::EnsuredInteractionTarget
            | ExpectedSequenceActor::ReferenceOnly => "sequence participant reference",
        };
        let span = SourceSpan::new(start, end);
        let is_explicit_declaration = matches!(
            expected,
            ExpectedSequenceActor::Participant | ExpectedSequenceActor::Actor
        );
        let ensures_participant = !matches!(expected, ExpectedSequenceActor::ReferenceOnly);
        let (is_definition, kind) = if is_explicit_declaration {
            let kind = *self
                .participant_kinds
                .entry(name.clone())
                .or_insert(declared_kind);
            (true, kind)
        } else if ensures_participant {
            match self.participant_kinds.entry(name.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(declared_kind);
                    (true, declared_kind)
                }
                Entry::Occupied(entry) => (false, *entry.get()),
            }
        } else {
            (
                false,
                self.participant_kinds
                    .get(&name)
                    .copied()
                    .unwrap_or(declared_kind),
            )
        };
        let detail = if is_definition && !is_explicit_declaration {
            "sequence implicit participant"
        } else {
            occurrence_detail
        };
        let symbol = if is_definition {
            EditorSemanticSymbol::new(name.clone(), Some(detail.to_string()), kind, span, span)
        } else {
            EditorSemanticSymbol::reference(
                name.clone(),
                Some(detail.to_string()),
                kind,
                span,
                span,
            )
        };
        facts.push_symbol(symbol);
        self.expected_text = match expected {
            ExpectedSequenceActor::MessageTarget => Some(ExpectedSequenceText::Message),
            ExpectedSequenceActor::NoteActor => Some(ExpectedSequenceText::Note),
            ExpectedSequenceActor::EnsuredInteractionTarget => {
                Some(ExpectedSequenceText::Interaction)
            }
            ExpectedSequenceActor::Participant
            | ExpectedSequenceActor::Actor
            | ExpectedSequenceActor::ReferenceOnly => None,
        };
        self.expected_actor = None;
    }

    fn push_pending_message_source(&mut self, facts: &mut EditorSemanticFacts) {
        if let Some(actor) = self.pending_message_source.take() {
            let (is_definition, kind) = match self.participant_kinds.entry(actor.name.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(EditorSemanticKind::Event);
                    (true, EditorSemanticKind::Event)
                }
                Entry::Occupied(entry) => (false, *entry.get()),
            };
            let symbol = if is_definition {
                EditorSemanticSymbol::new(
                    actor.name,
                    Some("sequence implicit participant".to_string()),
                    kind,
                    actor.span,
                    actor.span,
                )
            } else {
                EditorSemanticSymbol::reference(
                    actor.name,
                    Some("sequence participant reference".to_string()),
                    kind,
                    actor.span,
                    actor.span,
                )
            };
            facts.push_symbol(symbol);
        }
    }
}

#[cfg(feature = "editor-facts")]
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

#[cfg(feature = "editor-facts")]
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

#[cfg(feature = "editor-facts")]
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

#[cfg(feature = "editor-facts")]
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

#[cfg(feature = "editor-facts")]
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

#[cfg(test)]
mod tests {
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    use super::super::compat::parse_sequence_json_and_editor_facts;
    use super::*;
    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    use crate::EditorSemanticRole;
    use crate::MermaidConfig;
    #[cfg(feature = "compat-json")]
    use crate::ParseMetadata;
    use std::fmt::Write as _;

    #[cfg(feature = "compat-json")]
    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "sequence".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn canonical_sequence_capture_distinguishes_grammar_and_database_failures() {
        for (source, valid) in [
            ("sequenceDiagram\nparticipant A\nA->>B: hello\n", true),
            ("sequenceDiagram\nparticipant A\nA->>\n", false),
            ("sequenceDiagram\nparticipant A\ndeactivate A\n", false),
        ] {
            let metadata = SemanticMetadata {
                diagram_type: "sequence".to_string(),
                config: MermaidConfig::empty_object(),
                semantic_config: MermaidConfig::empty_object(),
                title: None,
            };
            reset_sequence_syntax_construction_count();
            #[cfg(feature = "editor-facts")]
            crate::editor::test_work::reset();
            let parsed = parse_sequence_semantic_model_controlled(
                source,
                &metadata,
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            assert_eq!(parsed.model.is_ok(), valid);
            let facts = parsed.editor_facts.expect("same-pass Sequence facts");
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "A"));
            assert_eq!(sequence_syntax_construction_count(), 1);
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read().collections, 1);
            assert_eq!(
                facts.completeness,
                if valid {
                    crate::EditorSemanticCompleteness::Complete
                } else {
                    crate::EditorSemanticCompleteness::Recovered
                }
            );
            assert_eq!(facts.diagnostics.is_empty(), valid);
            if !valid {
                let expected = if source.contains("deactivate") {
                    "sequence semantic construction failed:"
                } else {
                    "sequence parser recovered after parse error:"
                };
                assert!(facts.diagnostics[0].message.starts_with(expected));
            }
        }
    }

    #[test]
    fn canonical_semantic_model_retains_lifecycle_ownership_without_editor_work() {
        let source = concat!(
            "sequenceDiagram\n",
            "participant A\n",
            "participant C\n",
            "create participant B\n",
            "destroy A\n",
            "loop pending\n",
            "Note over C: pending\n",
            "end\n",
            "autonumber\n",
            "activate C\n",
            "deactivate C\n",
            "C->>B: create\n",
            "A--xC: destroy\n",
        );
        let metadata = SemanticMetadata {
            diagram_type: "sequence".to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        };
        reset_sequence_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();

        let model: SequenceDiagramModel = parse_sequence_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .expect("semantic construction is not cancelled")
        .model
        .expect("canonical Sequence semantics");

        assert_eq!(sequence_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        assert_eq!(model.actor_order, ["A", "C", "B"]);
        assert_eq!(model.created_actors.get("B"), Some(&0));
        assert_eq!(model.destroyed_actors.get("A"), Some(&0));
        assert_eq!(model.created_actor_message_index("B"), Some(6));
        assert_eq!(model.destroyed_actor_message_index("A"), Some(7));
    }

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn editor_facts(source: &str) -> EditorSemanticFacts {
        crate::family::test_support::editor_facts(
            parse_sequence_json_and_editor_facts,
            source,
            &meta(),
        )
    }

    fn carrier_actions(source: &str, expected: (usize, usize)) -> Actions {
        let control = OperationControl::new();
        let mut arena = ActionArena::default();
        let root = sequence_grammar::ActionsParser::new()
            .parse(&mut arena, &control, Lexer::new(source))
            .expect("Sequence grammar accepts carrier workload");
        assert_eq!(arena.carrier_counts(), expected);
        println!(
            "source_bytes={} action_records={} action_links={}",
            source.len(),
            expected.0,
            expected.1,
        );
        arena.into_actions(root)
    }

    #[test]
    fn grammar_carriers_attach_each_event_once_at_increasing_width_and_depth() {
        for size in [8, 128, 1_024] {
            let flat = format!("sequenceDiagram\n{}", "A->>B: message\n".repeat(size));
            assert_eq!(
                carrier_actions(&flat, (size * 3, size * 3 - 1)).count(),
                size * 3
            );

            let mut nested = String::from("sequenceDiagram\n");
            for index in 0..size {
                writeln!(nested, "loop L{index}").unwrap();
            }
            nested.push_str("A->>B: message\n");
            nested.push_str(&"end\n".repeat(size));
            assert_eq!(
                carrier_actions(&nested, (size * 2 + 3, size * 2 + 2)).count(),
                size * 2 + 3
            );

            let mut boxed = String::from("sequenceDiagram\nbox Team\n");
            for index in 0..size {
                writeln!(boxed, "participant P{index}").unwrap();
            }
            boxed.push_str("end\n");
            assert_eq!(
                carrier_actions(&boxed, (size + 2, size + 1)).count(),
                size + 2
            );

            let mut branched = String::from("sequenceDiagram\nalt Start\nA->>B: first\n");
            for index in 0..size {
                writeln!(branched, "else E{index}\nA->>B: branch").unwrap();
            }
            branched.push_str("end\n");
            assert_eq!(
                carrier_actions(&branched, (size * 4 + 5, size * 4 + 4)).count(),
                size * 4 + 5
            );
        }
    }

    #[test]
    fn grammar_carrier_reductions_observe_cancellation() {
        let source = format!("sequenceDiagram\n{}", "A->>B: message\n".repeat(1_024));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let mut arena = ActionArena::default();
        let error = sequence_grammar::ActionsParser::new()
            .parse(&mut arena, &control, Lexer::new(&source))
            .err()
            .expect("construction checkpoint interrupts reductions");
        assert!(matches!(error, lalrpop_util::ParseError::User { .. }));
        assert!(arena.carrier_counts().0 < 1_024 * 3);
        assert_eq!(
            control.checkpoint().unwrap_err().reason,
            crate::CancelReason::Requested
        );
    }

    #[test]
    fn cancelled_token_delivery_returns_terminal_before_grammar_eof() {
        let source = format!("sequenceDiagram\n{}", "A->>B: message\n".repeat(256));
        let control = OperationControl::new();
        let syntax = SequenceSyntax::lex(&source, &control).unwrap();
        #[cfg(feature = "editor-facts")]
        let fact_checkpoints = syntax.events.len().div_ceil(128);
        #[cfg(not(feature = "editor-facts"))]
        let fact_checkpoints = 0;
        control.cancel_after_checkpoints(fact_checkpoints + 2);
        let error = syntax
            .into_actions(
                #[cfg(feature = "editor-facts")]
                &source,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Capture,
                &control,
            )
            .err()
            .expect("cancelled delivery returns the outer terminal");
        assert_eq!(error.reason, crate::CancelReason::Requested);
    }

    #[cfg(feature = "compat-json")]
    #[test]
    #[cfg(feature = "editor-facts")]
    fn cancelled_editor_collection_stops_with_bounded_work_and_recovers() {
        let source = format!("sequenceDiagram\n{}", "A->>B: message\n".repeat(256));
        let control = OperationControl::new();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_sequence_syntax_construction_count();
        let syntax = SequenceSyntax::lex(&source, &control).unwrap();
        control.cancel_after_checkpoints(1);

        let error = syntax
            .into_actions(
                #[cfg(feature = "editor-facts")]
                &source,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Capture,
                &control,
            )
            .err()
            .expect("editor collection returns cancellation through the outer result");

        assert_eq!(error.reason, crate::CancelReason::Requested);
        assert_eq!(sequence_syntax_construction_count(), 1);
        let partial = crate::editor::test_work::read();
        assert_eq!(partial.collections, 1);
        assert!(partial.symbols > 0 && partial.symbols <= 128 * 3);
        assert!(partial.expected_syntax > 0 && partial.expected_syntax <= 128);
        assert_eq!(control.checkpoint(), Err(error));
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), partial);

        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_sequence_syntax_construction_count();
        let facts = editor_facts(&source);
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Complete
        );
        assert!(facts.symbols.len() > partial.symbols);
        assert_eq!(sequence_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read().collections, 1);
    }

    #[test]
    fn cancelled_replay_releases_remaining_flat_events() {
        let source = format!("sequenceDiagram\n{}", "A->>B: message\n".repeat(256));
        let actions = carrier_actions(&source, (256 * 3, 256 * 3 - 1));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(16);
        let Err(error) = build_sequence_db(actions, None, &control) else {
            panic!("replay returns cancellation through the outer result");
        };
        assert_eq!(error.reason, crate::CancelReason::Requested);
    }

    #[cfg(feature = "compat-json")]
    #[test]
    #[cfg(feature = "editor-facts")]
    fn participant_definitions_references_and_payloads_have_distinct_roles() {
        let source = concat!(
            "sequenceDiagram\n",
            "actor Alice as User\n",
            "participant Bob\n",
            "Alice->>Bob: Request\n",
            "Note over Alice,Bob: Review\n",
            "activate Bob\n",
            "deactivate Bob\n",
            "destroy Bob\n",
        );
        let facts = editor_facts(source);

        let alice: Vec<_> = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "Alice")
            .collect();
        let bob: Vec<_> = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "Bob")
            .collect();
        assert_eq!(alice[0].role, EditorSemanticRole::Entity);
        assert_eq!(alice[0].kind, EditorSemanticKind::Variable);
        assert!(
            alice[1..]
                .iter()
                .all(|symbol| symbol.role == EditorSemanticRole::Reference)
        );
        assert!(
            alice[1..]
                .iter()
                .all(|symbol| symbol.kind == EditorSemanticKind::Variable)
        );
        assert_eq!(bob[0].role, EditorSemanticRole::Entity);
        assert!(
            bob[1..]
                .iter()
                .all(|symbol| symbol.role == EditorSemanticRole::Reference)
        );
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Request" && symbol.role == EditorSemanticRole::Payload
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Review" && symbol.role == EditorSemanticRole::Payload
        }));
    }

    #[cfg(feature = "compat-json")]
    #[test]
    #[cfg(feature = "editor-facts")]
    fn unicode_implicit_participant_is_defined_once_then_referenced() {
        let source = concat!(
            "sequenceDiagram\n",
            "顾客->>服务: 创建\n",
            "服务-->>顾客: 完成\n",
        );
        let facts = editor_facts(source);

        for name in ["顾客", "服务"] {
            let occurrences: Vec<_> = facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == name)
                .collect();
            assert_eq!(occurrences.len(), 2);
            assert_eq!(occurrences[0].role, EditorSemanticRole::Entity);
            assert_eq!(occurrences[1].role, EditorSemanticRole::Reference);
            assert_eq!(occurrences[0].kind, occurrences[1].kind);
            assert_eq!(
                &source[occurrences[0].selection.start..occurrences[0].selection.end],
                name
            );
            assert_eq!(
                &source[occurrences[1].selection.start..occurrences[1].selection.end],
                name
            );
        }
    }

    #[cfg(feature = "compat-json")]
    #[test]
    #[cfg(feature = "editor-facts")]
    fn reference_only_statements_do_not_create_participant_entities() {
        let source = concat!(
            "sequenceDiagram\n",
            "activate Missing\n",
            "deactivate Missing\n",
            "destroy Missing\n",
        );
        let facts = editor_facts(source);
        let missing: Vec<_> = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "Missing")
            .collect();

        assert_eq!(missing.len(), 3);
        assert!(
            missing
                .iter()
                .all(|symbol| symbol.role == EditorSemanticRole::Reference)
        );
    }
}
