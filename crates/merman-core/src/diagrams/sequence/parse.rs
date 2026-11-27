use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::family::SemanticConstruction;
use crate::parse_diagnostic::{unexpected_eof_message, unexpected_token_message};
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{
    Error, MermaidConfig, OperationControl, OperationControlResult, ParseDiagnostic,
    ParseDiagnosticSpanKind, SemanticMetadata,
};
#[cfg(feature = "editor-facts")]
use merman_core_sequence::sequence::{
    SequenceLexicalEvent, SequenceToken as Tok, box_color_and_title,
};
use merman_core_sequence::sequence::{
    SequenceModelError, SequenceSyntax, SequenceSyntaxDiagnostic,
};
#[cfg(test)]
use std::cell::Cell;
#[cfg(feature = "editor-facts")]
use std::collections::{HashMap, hash_map::Entry};

use super::model::SequenceDiagramModel;

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

fn lex_sequence_syntax(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<SequenceSyntax> {
    #[cfg(test)]
    SEQUENCE_SYNTAX_CONSTRUCTION_COUNT.set(SEQUENCE_SYNTAX_CONSTRUCTION_COUNT.get() + 1);
    SequenceSyntax::lex(code, control)
}

fn sequence_parse_diagnostic(diagnostic: SequenceSyntaxDiagnostic) -> ParseDiagnostic {
    use ParseDiagnosticSpanKind::{Exact, InsertionPoint};
    match diagnostic {
        SequenceSyntaxDiagnostic::Lexical(error) => {
            ParseDiagnostic::new(error.message).with_span(error.span, Exact)
        }
        SequenceSyntaxDiagnostic::UnexpectedToken {
            token,
            span,
            expected,
        } => ParseDiagnostic::new(unexpected_token_message(&token, expected.names()))
            .with_span(span, Exact),
        SequenceSyntaxDiagnostic::UnexpectedEof { offset, expected } => {
            ParseDiagnostic::new(unexpected_eof_message(expected.names()))
                .with_span(SourceSpan::new(offset, offset), InsertionPoint)
        }
    }
}

pub(super) struct SequenceSemanticSource {
    pub(super) model: SequenceDiagramModel,
    #[cfg(feature = "editor-facts")]
    pub(super) editor_facts: Option<EditorSemanticFacts>,
}

pub(super) enum SequenceSemanticFailure {
    Grammar {
        diagnostic: ParseDiagnostic,
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
    pub(super) fn into_parse_error_for_type(self, diagram_type: &str) -> Error {
        match self {
            Self::Grammar {
                diagnostic,
                #[cfg(feature = "editor-facts")]
                    editor_facts: _,
            } => Error::diagram_parse_diagnostic(diagram_type.to_string(), diagnostic),
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
    ) -> (Error, EditorSemanticFacts) {
        match self {
            Self::Grammar {
                diagnostic,
                editor_facts,
            } => {
                let mut editor_facts =
                    editor_facts.expect("Sequence capture requests its failure journal");
                editor_facts.mark_recovered_from_parse_error(
                    format!(
                        "sequence parser recovered after parse error: {}",
                        diagnostic.message(),
                    ),
                    diagnostic.span(),
                );
                let parse_error =
                    Error::diagram_parse_diagnostic(diagram_type.to_string(), diagnostic);
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
            mut model,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }) => {
            super::sanitize_common_db_fields(&mut model, &meta.semantic_config);
            SemanticConstruction {
                model: Ok(model),
                #[cfg(feature = "editor-facts")]
                editor_facts,
            }
        }
        #[cfg(feature = "editor-facts")]
        Err(failure) if editor_demand.needs_recovery() => {
            let (error, editor_facts) =
                failure.into_error_and_editor_facts_for_type(&meta.diagram_type);
            SemanticConstruction {
                model: Err(error),
                #[cfg(feature = "editor-facts")]
                editor_facts: Some(editor_facts),
            }
        }
        Err(failure) => SemanticConstruction {
            model: Err(failure.into_parse_error_for_type(&meta.diagram_type)),
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
    let syntax = lex_sequence_syntax(code, control)?;
    #[cfg(feature = "editor-facts")]
    let editor_facts = match facts_demand {
        EditorFactsDemand::Omit => None,
        EditorFactsDemand::Recovery => Some(EditorSemanticFacts::new()),
        EditorFactsDemand::Capture => Some(collect_sequence_editor_facts_from_events(
            syntax.events(),
            code,
            control,
        )?),
    };
    let model = match syntax.into_model(wrap_enabled, control)? {
        Ok(model) => model,
        Err(SequenceModelError::Grammar(diagnostic)) => {
            return Ok(Err(Box::new(SequenceSemanticFailure::Grammar {
                diagnostic: sequence_parse_diagnostic(diagnostic),
                #[cfg(feature = "editor-facts")]
                editor_facts,
            })));
        }
        Err(SequenceModelError::Db(message)) => {
            return Ok(Err(Box::new(SequenceSemanticFailure::Db {
                message,
                #[cfg(feature = "editor-facts")]
                editor_facts,
            })));
        }
    };

    Ok(Ok(SequenceSemanticSource {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    }))
}

pub(super) fn sequence_wrap_enabled_from_config(config: &MermaidConfig) -> Option<bool> {
    config
        .get_bool("wrap")
        .or_else(|| config.get_bool("sequence.wrap"))
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
    let (_, title) = box_color_and_title(raw);
    let selection = sequence_payload_selection(title, start, end, code)?;
    Some((title.to_string(), selection))
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
    use crate::SourcePresentationMetadata;

    #[cfg(feature = "compat-json")]
    fn meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "sequence".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn sequence_wrap_prefers_root_booleans_over_the_layered_family_value() {
        let base = MermaidConfig::from_value(serde_json::json!({"sequence": {"wrap": false}}));
        for (root, expected) in [
            (None, true),
            (Some(serde_json::json!(false)), false),
            (Some(serde_json::Value::Null), true),
            (Some(serde_json::json!("false")), true),
        ] {
            let mut config = base.clone().with_shared_sections();
            config.set_value("sequence.wrap", serde_json::json!(true));
            if let Some(root) = root {
                config.set_value("wrap", root);
            }
            assert_eq!(sequence_wrap_enabled_from_config(&config), Some(expected));
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

    #[test]
    fn sequence_syntax_diagnostics_keep_exact_and_insertion_spans() {
        use ParseDiagnosticSpanKind::{Exact, InsertionPoint};

        let lexical = "sequenceDiagram\nparticipant 顾客@{ type: boundary";
        let unexpected = "sequenceDiagram\nparticipant 顾客\nend\n";
        let incomplete = "sequenceDiagram\nloop pending\n    ";
        let eof_offset = incomplete.trim_end_matches(' ').len();

        for (source, message, span, kind) in [
            (
                lexical,
                "Unterminated config object; missing '}'",
                SourceSpan::new(lexical.find("@{").unwrap(), lexical.len()),
                Exact,
            ),
            (
                unexpected,
                "unexpected end; expected newline, \"participant\"",
                SourceSpan::new(unexpected.rfind("end").unwrap(), unexpected.len() - 1),
                Exact,
            ),
            (
                incomplete,
                "unexpected end of input; expected newline, \"participant\"",
                SourceSpan::new(eof_offset, eof_offset),
                InsertionPoint,
            ),
        ] {
            let Err(failure) = construct_sequence_semantic_source(
                source,
                None,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap() else {
                panic!("expected a Sequence grammar error for {source:?}");
            };
            let SequenceSemanticFailure::Grammar { diagnostic, .. } = *failure else {
                panic!("expected a grammar error before database replay for {source:?}");
            };
            assert!(diagnostic.message().starts_with(message), "{diagnostic:?}");
            assert_eq!(diagnostic.span(), Some(span), "{source:?}");
            assert_eq!(diagnostic.span_kind(), kind, "{source:?}");
        }
    }

    #[test]
    fn cancelled_token_delivery_returns_terminal_before_grammar_eof() {
        let source = format!("sequenceDiagram\n{}", "A->>B: message\n".repeat(256));
        let control = OperationControl::new();
        let syntax = SequenceSyntax::lex(&source, &control).unwrap();
        #[cfg(feature = "editor-facts")]
        let fact_checkpoints = syntax.events().len().div_ceil(128);
        #[cfg(not(feature = "editor-facts"))]
        let fact_checkpoints = 0;
        control.cancel_after_checkpoints(fact_checkpoints + 2);
        #[cfg(feature = "editor-facts")]
        collect_sequence_editor_facts_from_events(syntax.events(), &source, &control)
            .expect("editor collection finishes before token delivery");
        let error = syntax
            .into_actions(&control)
            .expect_err("cancelled delivery returns the outer terminal");
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
        let syntax = lex_sequence_syntax(&source, &control).unwrap();
        control.cancel_after_checkpoints(1);

        let error = collect_sequence_editor_facts_from_events(syntax.events(), &source, &control)
            .expect_err("editor collection returns cancellation through the outer result");

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
