use crate::family::EditorFactsDemand;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, OperationControl, OperationControlResult, ParseDiagnostic,
    ParseDiagnosticSpanKind, ParseMetadata, Result, SourceSpan,
    editor::{
        format_lalrpop_parse_error, has_ascii_separator, lalrpop_parse_diagnostic,
        lalrpop_recovery_span, line_content_end, source_value_span, trailing_ascii_whitespace_slot,
    },
};
use serde_json::Value;
#[cfg(test)]
use std::cell::Cell;
use std::collections::HashSet;

use super::db::StateDb;
use super::{
    Lexer, StateDiagramModel, StateDiagramRenderModel, StateDocument, StateStatement, Tok,
};

#[cfg(test)]
thread_local! {
    static STATE_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_state_syntax_construction_count() {
    STATE_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn state_syntax_construction_count() -> usize {
    STATE_SYNTAX_CONSTRUCTION_COUNT.get()
}

type StateLexicalEvent = std::result::Result<(usize, Tok, usize), super::LexError>;
type StateGrammarError = lalrpop_util::ParseError<usize, Tok, super::LexError>;

struct StateSyntax {
    events: Vec<StateLexicalEvent>,
}

impl StateSyntax {
    fn lex(code: &str, control: &OperationControl) -> OperationControlResult<Self> {
        #[cfg(test)]
        STATE_SYNTAX_CONSTRUCTION_COUNT.set(STATE_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

        let events = crate::diagrams::scan::collect_lexical_events(&mut Lexer::new(code), control)?;
        Ok(Self { events })
    }

    fn into_editor_facts_and_document(
        self,
        code: &str,
        facts_demand: EditorFactsDemand,
        control: &OperationControl,
    ) -> OperationControlResult<(
        EditorSemanticFacts,
        std::result::Result<StateDocument, StateGrammarError>,
    )> {
        let editor_facts = match facts_demand {
            EditorFactsDemand::Omit => EditorSemanticFacts::new(),
            EditorFactsDemand::Capture => {
                collect_state_editor_facts_from_events(&self.events, code, control)?
            }
        };
        control.checkpoint()?;
        let mut emitted = 0usize;
        let controlled_events = self.events.into_iter().take_while(|_| {
            let active = !emitted.is_multiple_of(128) || control.checkpoint().is_ok();
            emitted = emitted.saturating_add(1);
            active
        });
        let mut document = StateDocument::default();
        let parsed =
            super::state_grammar::RootParser::new().parse(&mut document, controlled_events);
        let document = parsed.map(|root| {
            document.root = Some(root);
            document
        });
        control.checkpoint()?;
        Ok((editor_facts, document))
    }
}

struct StateSemanticSource {
    db: StateDb,
    editor_facts: EditorSemanticFacts,
}

struct StateSemanticFailure {
    error: Box<StateGrammarError>,
    editor_facts: Box<EditorSemanticFacts>,
}

impl StateSemanticFailure {
    fn into_parse_error(self, meta: &ParseMetadata, fallback_offset: usize) -> Error {
        self.into_parse_error_for_type(&meta.diagram_type, fallback_offset)
    }

    fn into_parse_error_for_type(self, diagram_type: &str, fallback_offset: usize) -> Error {
        Error::diagram_parse_diagnostic(
            diagram_type.to_string(),
            state_parse_diagnostic(&self.error, fallback_offset),
        )
    }

    fn into_error_and_editor_facts(
        self,
        meta: &ParseMetadata,
        fallback_offset: usize,
    ) -> (Error, EditorSemanticFacts) {
        self.into_error_and_editor_facts_for_type(&meta.diagram_type, fallback_offset)
    }

    fn into_error_and_editor_facts_for_type(
        mut self,
        diagram_type: &str,
        fallback_offset: usize,
    ) -> (Error, EditorSemanticFacts) {
        let error = Error::diagram_parse_diagnostic(
            diagram_type.to_string(),
            state_parse_diagnostic(&self.error, fallback_offset),
        );
        let span = match self.error.as_ref() {
            lalrpop_util::ParseError::User { error } => error
                .span
                .unwrap_or_else(|| SourceSpan::new(fallback_offset, fallback_offset)),
            _ => lalrpop_recovery_span(&self.error, fallback_offset),
        };
        self.editor_facts.mark_recovered_from_parse_error(
            format!(
                "state parser recovered after parse error: {}",
                format_lalrpop_parse_error(self.error.as_ref())
            ),
            Some(span),
        );
        (error, *self.editor_facts)
    }
}

fn state_parse_diagnostic(error: &StateGrammarError, fallback_offset: usize) -> ParseDiagnostic {
    let diagnostic = lalrpop_parse_diagnostic(error, fallback_offset);
    match error {
        lalrpop_util::ParseError::UnrecognizedToken {
            token: (start, Tok::Newline, end),
            ..
        } if start == end && *end == fallback_offset => diagnostic.with_span(
            SourceSpan::new(*start, *end),
            ParseDiagnosticSpanKind::InsertionPoint,
        ),
        _ => diagnostic,
    }
}

pub(crate) fn parse_state(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_state_semantic_source(code, meta)?.db.into_model(meta)
}

pub(crate) fn parse_state_model_for_render_controlled(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<StateDiagramRenderModel>> {
    let construction = construct_state_semantic_source(code, EditorFactsDemand::Omit, control)?;
    match construction {
        Ok(source) => source.db.into_model_controlled(meta, control),
        Err(failure) => Ok(Err(failure.into_parse_error(meta, code.len()))),
    }
}

pub(crate) fn parse_state_semantic_model_controlled(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<Result<StateDiagramModel>> {
    let construction = construct_state_semantic_source(code, EditorFactsDemand::Omit, control)?;
    match construction {
        Ok(source) => {
            match source
                .db
                .into_semantic_model_controlled(diagram_type, config, control)?
            {
                Ok(mut model) => {
                    model.sanitize_common_db_fields(config);
                    control.checkpoint()?;
                    Ok(Ok(model))
                }
                Err(error) => Ok(Err(error)),
            }
        }
        Err(failure) => Ok(Err(
            failure.into_parse_error_for_type(diagram_type, code.len())
        )),
    }
}

pub(crate) fn parse_state_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_state_semantic_source(code, EditorFactsDemand::Capture, control)?;
    let construction = match construction {
        Ok(StateSemanticSource { db, editor_facts }) => {
            let model = match db.into_model_controlled(meta, control)? {
                Ok(model) => super::render_model::render_model_to_compat_json_controlled(
                    &model, meta, control,
                )?,
                Err(error) => Err(error),
            };
            Ok((model, editor_facts))
        }
        Err(failure) => Err(failure),
    };
    control.checkpoint()?;
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |(model, editor_facts)| {
            (
                model.map(crate::ManagedSemanticJson::into_unmanaged_value),
                editor_facts,
            )
        },
        |failure| failure.into_error_and_editor_facts(meta, code.len()),
    );
    Ok(parsed)
}

fn parse_state_semantic_source(code: &str, meta: &ParseMetadata) -> Result<StateSemanticSource> {
    construct_state_semantic_source(code, EditorFactsDemand::Omit, &OperationControl::new())
        .expect("a private parse control cannot be cancelled")
        .map_err(|failure| failure.into_parse_error(meta, code.len()))
}

fn construct_state_semantic_source(
    code: &str,
    facts_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<StateSemanticSource, StateSemanticFailure>> {
    let syntax = StateSyntax::lex(code, control)?;
    let (editor_facts, document) =
        syntax.into_editor_facts_and_document(code, facts_demand, control)?;
    let mut doc = match document {
        Ok(doc) => doc,
        Err(error) => {
            return Ok(Err(StateSemanticFailure {
                error: Box::new(error),
                editor_facts: Box::new(editor_facts),
            }));
        }
    };

    let mut divider_cnt = 0usize;
    assign_divider_ids(&mut doc, &mut divider_cnt, control)?;

    let mut db = StateDb::new();
    db.set_root_doc(doc, control)?;
    control.checkpoint()?;
    Ok(Ok(StateSemanticSource { db, editor_facts }))
}

pub(super) fn assign_divider_ids(
    document: &mut StateDocument,
    cnt: &mut usize,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let Some(root) = document.root else {
        return Ok(());
    };
    let mut stack = vec![(root, 0usize)];
    let mut inspected = 0usize;
    while let Some((doc, index)) = stack.last_mut() {
        if inspected.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let Some(stmt) = document.documents[doc.0].get_mut(*index) else {
            stack.pop();
            continue;
        };
        *index += 1;
        let mut child = None;
        match stmt {
            StateStatement::State(st) => {
                if st.ty == "divider" && st.id == "__divider__" {
                    *cnt += 1;
                    st.id = format!("divider-id-{cnt}");
                }
                child = st.doc;
            }
            StateStatement::Relation(relation) => {
                for st in [&mut relation.state1, &mut relation.state2] {
                    if st.ty == "divider" && st.id == "__divider__" {
                        *cnt += 1;
                        st.id = format!("divider-id-{cnt}");
                    }
                }
            }
            _ => {}
        }
        if let Some(child) = child {
            stack.push((child, 0));
        }
        inspected = inspected.saturating_add(1);
    }
    control.checkpoint()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateTokenContext {
    Default,
    ClickTarget,
    ClickAfterTarget,
    ClickAfterHref,
    ClickTooltip,
}

#[derive(Debug)]
struct StatePendingEntity {
    name: String,
    selection: SourceSpan,
    detail: &'static str,
    kind: EditorSemanticKind,
}

fn collect_state_editor_facts_from_events(
    lexical_events: &[StateLexicalEvent],
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<EditorSemanticFacts> {
    #[cfg(test)]
    crate::editor::test_work::record_collection();
    let mut collector = StateTokenFactCollector {
        code,
        context: StateTokenContext::Default,
        pending_entity: None,
        pending_directive_slot: None,
        directive_start: None,
        interaction_target_end: None,
        note_text_pending: false,
        note_position_seen: false,
        note_alias_pending: false,
        relation_label_pending: false,
        relation_target_seen: false,
        declared_entities: HashSet::new(),
    };
    let mut facts = EditorSemanticFacts::new();
    for (index, event) in lexical_events.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        match event {
            Ok((start, token, end)) => {
                collector.collect_token(token.clone(), *start, *end, &mut facts, control)?;
            }
            Err(error) => {
                if let Some(expected) = error.expected_syntax.as_ref() {
                    facts.push_expected_syntax(EditorExpectedSyntax::new(
                        expected.kind,
                        expected.span,
                    ));
                }
            }
        }
    }
    collector.finish_statement(line_content_end(code, code.len()), &mut facts);
    control.checkpoint()?;
    Ok(facts)
}

struct StateTokenFactCollector<'a> {
    code: &'a str,
    context: StateTokenContext,
    pending_entity: Option<StatePendingEntity>,
    pending_directive_slot: Option<PendingDirectiveSlot>,
    directive_start: Option<usize>,
    interaction_target_end: Option<usize>,
    note_text_pending: bool,
    note_position_seen: bool,
    note_alias_pending: bool,
    relation_label_pending: bool,
    relation_target_seen: bool,
    declared_entities: HashSet<String>,
}

#[derive(Debug, Clone, Copy)]
struct PendingDirectiveSlot {
    kind: EditorExpectedSyntaxKind,
    anchor_end: usize,
}

impl StateTokenFactCollector<'_> {
    fn collect_token(
        &mut self,
        token: Tok,
        start: usize,
        end: usize,
        facts: &mut EditorSemanticFacts,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        match token {
            Tok::Newline => self.finish_statement(line_content_end(self.code, start), facts),
            Tok::StructStart | Tok::StructStop => self.reset_statement(facts),
            Tok::Arrow => {
                self.flush_pending_entity_for_relation(facts);
                self.relation_label_pending = true;
                self.relation_target_seen = false;
            }
            Tok::Click => {
                self.flush_pending_entity(facts);
                facts.push_directive_prefix("click");
                self.directive_start = Some(start);
                self.expect_after_separator(EditorExpectedSyntaxKind::NodeIdentifier, end);
                self.context = StateTokenContext::ClickTarget;
            }
            Tok::Href if self.context == StateTokenContext::ClickAfterTarget => {
                push_state_expected_syntax(
                    facts,
                    EditorExpectedSyntaxKind::InteractionAction,
                    SourceSpan::new(start, end),
                );
                self.context = StateTokenContext::ClickAfterHref;
                self.interaction_target_end = None;
            }
            Tok::Id(id) if self.context == StateTokenContext::ClickTarget => {
                self.pending_directive_slot = None;
                self.interaction_target_end = Some(end);
                push_state_reference_fact(facts, id, start, end, "state click target");
                self.context = StateTokenContext::ClickAfterTarget;
            }
            Tok::StyledId((id, class_id)) if self.context == StateTokenContext::ClickTarget => {
                self.pending_directive_slot = None;
                self.interaction_target_end = Some(end);
                push_state_reference_fact(facts, id, start, end, "state click target");
                push_state_payload_fact_from_token_as(
                    facts,
                    self.code,
                    class_id.as_str(),
                    SourceSpan::new(start, end),
                    "state inline class",
                    EditorSemanticKind::Property,
                    EditorExpectedSyntaxKind::ClassName,
                );
                self.context = StateTokenContext::ClickAfterTarget;
            }
            Tok::EdgeState if self.context == StateTokenContext::ClickTarget => {
                self.pending_directive_slot = None;
                self.interaction_target_end = Some(end);
                self.context = StateTokenContext::ClickAfterTarget;
            }
            Tok::Id(_) if self.context == StateTokenContext::ClickAfterTarget => {
                push_state_expected_syntax(
                    facts,
                    EditorExpectedSyntaxKind::InteractionAction,
                    SourceSpan::new(start, end),
                );
                self.interaction_target_end = None;
                self.context = StateTokenContext::Default;
            }
            Tok::EdgeState => {
                self.flush_pending_entity(facts);
                if self.relation_label_pending {
                    self.relation_target_seen = true;
                }
            }
            Tok::Note => {
                self.flush_pending_entity(facts);
                self.note_text_pending = true;
                self.note_position_seen = false;
                self.note_alias_pending = false;
            }
            Tok::LeftOf | Tok::RightOf => {
                self.note_position_seen = true;
            }
            Tok::NoteText(text) if self.note_text_pending => {
                self.flush_pending_entity(facts);
                push_state_payload_fact_from_token(
                    facts,
                    self.code,
                    text.as_str(),
                    start,
                    end,
                    "state note",
                    EditorSemanticKind::String,
                );
                self.note_alias_pending = !self.note_position_seen;
                self.note_text_pending = false;
            }
            Tok::StateDescr(text) => {
                self.flush_pending_entity(facts);
                push_state_string_payload_fact(
                    facts,
                    text.as_str(),
                    start,
                    end,
                    "state display label",
                    EditorSemanticKind::String,
                );
            }
            Tok::Descr(text) => {
                self.flush_pending_entity(facts);
                let detail = if self.relation_label_pending && self.relation_target_seen {
                    "state relation label"
                } else {
                    "state description"
                };
                push_state_payload_fact_from_token(
                    facts,
                    self.code,
                    text.as_str(),
                    start,
                    end,
                    detail,
                    EditorSemanticKind::String,
                );
                self.relation_label_pending = false;
                self.relation_target_seen = false;
            }
            Tok::StringLit(text)
                if matches!(
                    self.context,
                    StateTokenContext::ClickAfterTarget | StateTokenContext::ClickAfterHref
                ) =>
            {
                self.flush_pending_entity(facts);
                self.interaction_target_end = None;
                push_state_string_payload_fact(
                    facts,
                    text.as_str(),
                    start,
                    end,
                    "state click url",
                    EditorSemanticKind::String,
                );
                self.context = if self.context == StateTokenContext::ClickAfterTarget {
                    StateTokenContext::ClickTooltip
                } else {
                    StateTokenContext::Default
                };
            }
            Tok::StringLit(text) if self.context == StateTokenContext::ClickTooltip => {
                push_state_string_payload_fact(
                    facts,
                    text.as_str(),
                    start,
                    end,
                    "state click tooltip",
                    EditorSemanticKind::String,
                );
                self.context = StateTokenContext::Default;
            }
            Tok::Id(id) | Tok::CompositState(id) => {
                self.handle_state_entity(id, start, end, "state", facts);
            }
            Tok::StyledId((id, class_id)) => {
                self.handle_state_entity(id, start, end, "state", facts);
                push_state_payload_fact_from_token_as(
                    facts,
                    self.code,
                    class_id.as_str(),
                    SourceSpan::new(start, end),
                    "state inline class",
                    EditorSemanticKind::Property,
                    EditorExpectedSyntaxKind::ClassName,
                );
            }
            Tok::Fork(id) => {
                self.handle_state_entity(id, start, end, "state fork", facts);
            }
            Tok::Join(id) => {
                self.handle_state_entity(id, start, end, "state join", facts);
            }
            Tok::Choice(id) => {
                self.handle_state_entity(id, start, end, "state choice", facts);
            }
            Tok::ClassDef => {
                self.flush_pending_entity(facts);
                facts.push_directive_prefix("classDef");
                self.directive_start = Some(start);
                self.expect_after_separator(EditorExpectedSyntaxKind::ClassName, end);
            }
            Tok::ClassDefId(id) => {
                if !id.trim().is_empty() {
                    push_state_expected_syntax(
                        facts,
                        EditorExpectedSyntaxKind::ClassName,
                        SourceSpan::new(start, end),
                    );
                    self.expect_after_separator(EditorExpectedSyntaxKind::StyleValue, end);
                }
                push_state_class_definition_fact(
                    facts,
                    id,
                    start,
                    end,
                    "state class definition",
                    EditorSemanticKind::Property,
                );
            }
            Tok::ClassDefStyleOpts(raw) => {
                if let Some(span) = trailing_ascii_whitespace_slot(self.code, start, end) {
                    push_state_expected_syntax(facts, EditorExpectedSyntaxKind::StyleValue, span);
                }
                self.pending_directive_slot = None;
                push_state_payload_fact_from_token(
                    facts,
                    self.code,
                    raw.as_str(),
                    start,
                    end,
                    "state class definition style",
                    EditorSemanticKind::String,
                );
            }
            Tok::Class => {
                self.flush_pending_entity(facts);
                facts.push_directive_prefix("class");
                self.directive_start = Some(start);
                self.expect_after_separator(EditorExpectedSyntaxKind::IdList, end);
            }
            Tok::ClassEntityIds(ids) => {
                let has_targets = !ids.trim().is_empty();
                push_state_reference_id_list_facts(
                    facts,
                    self.code,
                    ids.as_str(),
                    SourceSpan::new(start, end),
                    "state class target",
                    EditorExpectedSyntaxKind::IdList,
                    control,
                )?;
                if has_targets {
                    self.expect_after_separator(EditorExpectedSyntaxKind::ClassName, end);
                }
            }
            Tok::StyleClass(class_id) => {
                if !class_id.trim().is_empty() {
                    self.pending_directive_slot = None;
                }
                push_state_payload_fact_from_token_as(
                    facts,
                    self.code,
                    class_id.as_str(),
                    SourceSpan::new(start, end),
                    "state class reference",
                    EditorSemanticKind::Property,
                    EditorExpectedSyntaxKind::ClassName,
                );
            }
            Tok::Style => {
                self.flush_pending_entity(facts);
                facts.push_directive_prefix("style");
                self.directive_start = Some(start);
                self.expect_after_separator(EditorExpectedSyntaxKind::IdList, end);
            }
            Tok::StyleIds(ids) => {
                let has_targets = !ids.trim().is_empty();
                push_state_reference_id_list_facts(
                    facts,
                    self.code,
                    ids.as_str(),
                    SourceSpan::new(start, end),
                    "state style target",
                    EditorExpectedSyntaxKind::IdList,
                    control,
                )?;
                if has_targets {
                    self.expect_after_separator(EditorExpectedSyntaxKind::StyleValue, end);
                }
            }
            Tok::StyleDefStyleOpts(raw) => {
                if let Some(span) = trailing_ascii_whitespace_slot(self.code, start, end) {
                    push_state_expected_syntax(facts, EditorExpectedSyntaxKind::StyleValue, span);
                }
                self.pending_directive_slot = None;
                push_state_payload_fact_from_token(
                    facts,
                    self.code,
                    raw.as_str(),
                    start,
                    end,
                    "state style",
                    EditorSemanticKind::String,
                );
            }
            Tok::AccTitle(value) => {
                self.flush_pending_entity(facts);
                facts.push_directive_prefix("accTitle");
                push_state_payload_fact_from_token(
                    facts,
                    self.code,
                    value.as_str(),
                    start,
                    end,
                    "state accessibility title",
                    EditorSemanticKind::String,
                );
            }
            Tok::AccDescr(value) | Tok::AccDescrMultiline(value) => {
                self.flush_pending_entity(facts);
                facts.push_directive_prefix("accDescr");
                push_state_payload_fact_from_token(
                    facts,
                    self.code,
                    value.as_str(),
                    start,
                    end,
                    "state accessibility description",
                    EditorSemanticKind::String,
                );
            }
            Tok::Sd
            | Tok::As
            | Tok::NoteText(_)
            | Tok::Concurrent
            | Tok::HideEmptyDescription
            | Tok::ScaleWidth(_)
            | Tok::Href
            | Tok::StringLit(_) => {}
            Tok::Direction(direction) => {
                push_state_expected_syntax(
                    facts,
                    EditorExpectedSyntaxKind::CardinalDirectionValue,
                    direction.selection,
                );
            }
        }
        Ok(())
    }

    fn handle_state_entity(
        &mut self,
        id: String,
        start: usize,
        end: usize,
        default_detail: &'static str,
        facts: &mut EditorSemanticFacts,
    ) {
        if self.note_alias_pending {
            self.note_alias_pending = false;
            push_state_expected_syntax(
                facts,
                EditorExpectedSyntaxKind::Payload,
                SourceSpan::new(start, end),
            );
            return;
        }

        if self.note_text_pending && self.note_position_seen {
            self.push_state_relation_occurrence(id, start, end, "state note target", facts);
            return;
        }

        if self.relation_label_pending {
            self.flush_pending_entity(facts);
            self.push_state_relation_occurrence(id, start, end, "state reference", facts);
            self.relation_target_seen = true;
            return;
        }

        self.queue_state_entity(id, start, end, default_detail, facts);
    }

    fn queue_state_entity(
        &mut self,
        id: String,
        start: usize,
        end: usize,
        detail: &'static str,
        facts: &mut EditorSemanticFacts,
    ) {
        self.flush_pending_entity(facts);
        let Some(entity) = state_pending_entity(id, start, end, detail) else {
            return;
        };
        push_state_expected_syntax(
            facts,
            EditorExpectedSyntaxKind::NodeIdentifier,
            entity.selection,
        );
        self.pending_entity = Some(entity);
    }

    fn flush_pending_entity(&mut self, facts: &mut EditorSemanticFacts) {
        if let Some(entity) = self.pending_entity.take() {
            self.declared_entities.insert(entity.name.clone());
            facts.push_symbol(EditorSemanticSymbol::new(
                entity.name,
                Some(entity.detail.to_string()),
                entity.kind,
                entity.selection,
                entity.selection,
            ));
        }
    }

    fn flush_pending_entity_for_relation(&mut self, facts: &mut EditorSemanticFacts) {
        if let Some(entity) = self.pending_entity.take() {
            self.push_state_relation_occurrence_with_selection(
                entity.name,
                entity.selection,
                "state reference",
                facts,
            );
        }
    }

    fn push_state_relation_occurrence(
        &mut self,
        id: String,
        start: usize,
        end: usize,
        detail: &'static str,
        facts: &mut EditorSemanticFacts,
    ) {
        let Some(entity) = state_pending_entity(id, start, end, detail) else {
            return;
        };
        push_state_expected_syntax(
            facts,
            EditorExpectedSyntaxKind::NodeIdentifier,
            entity.selection,
        );
        self.push_state_relation_occurrence_with_selection(
            entity.name,
            entity.selection,
            entity.detail,
            facts,
        );
    }

    fn push_state_relation_occurrence_with_selection(
        &mut self,
        name: String,
        selection: SourceSpan,
        detail: &'static str,
        facts: &mut EditorSemanticFacts,
    ) {
        let kind = EditorSemanticKind::Class;
        if self.declared_entities.insert(name.clone()) {
            facts.push_symbol(EditorSemanticSymbol::new(
                name,
                Some(detail.to_string()),
                kind,
                selection,
                selection,
            ));
        } else {
            facts.push_symbol(EditorSemanticSymbol::reference(
                name,
                Some(detail.to_string()),
                kind,
                selection,
                selection,
            ));
        }
    }

    fn finish_statement(&mut self, line_end: usize, facts: &mut EditorSemanticFacts) {
        self.flush_pending_entity(facts);
        if let Some(start) = self.directive_start {
            push_state_expected_syntax(
                facts,
                EditorExpectedSyntaxKind::Directive,
                SourceSpan::new(start, line_end),
            );
        }
        if let Some(slot) = self.pending_directive_slot
            && has_ascii_separator(self.code, slot.anchor_end, line_end)
        {
            push_state_expected_syntax(facts, slot.kind, SourceSpan::new(line_end, line_end));
        }
        if let Some(target_end) = self.interaction_target_end
            && has_ascii_separator(self.code, target_end, line_end)
        {
            push_state_expected_syntax(
                facts,
                EditorExpectedSyntaxKind::InteractionAction,
                SourceSpan::new(line_end, line_end),
            );
        }
        self.reset_statement_state();
    }

    fn reset_statement(&mut self, facts: &mut EditorSemanticFacts) {
        self.flush_pending_entity(facts);
        self.reset_statement_state();
    }

    fn reset_statement_state(&mut self) {
        self.context = StateTokenContext::Default;
        self.pending_directive_slot = None;
        self.directive_start = None;
        self.interaction_target_end = None;
        self.note_text_pending = false;
        self.note_position_seen = false;
        self.note_alias_pending = false;
        self.relation_label_pending = false;
        self.relation_target_seen = false;
    }

    fn expect_after_separator(&mut self, kind: EditorExpectedSyntaxKind, anchor_end: usize) {
        self.pending_directive_slot = Some(PendingDirectiveSlot { kind, anchor_end });
    }
}

fn state_pending_entity(
    id: String,
    start: usize,
    end: usize,
    detail: &'static str,
) -> Option<StatePendingEntity> {
    if !is_editor_visible_state_name(&id) {
        return None;
    }

    let selection_end = start + id.len();
    let selection = SourceSpan::new(start, selection_end.min(end));
    Some(StatePendingEntity {
        name: id,
        selection,
        detail,
        kind: EditorSemanticKind::Class,
    })
}

fn push_state_reference_fact(
    facts: &mut EditorSemanticFacts,
    id: String,
    start: usize,
    end: usize,
    detail: &'static str,
) {
    let Some(entity) = state_pending_entity(id, start, end, detail) else {
        return;
    };
    push_state_expected_syntax(
        facts,
        EditorExpectedSyntaxKind::NodeIdentifier,
        entity.selection,
    );
    facts.push_symbol(EditorSemanticSymbol::reference(
        entity.name,
        Some(entity.detail.to_string()),
        entity.kind,
        entity.selection,
        entity.selection,
    ));
}

fn push_state_reference_fact_with_selection(
    facts: &mut EditorSemanticFacts,
    id: String,
    selection: SourceSpan,
    detail: &'static str,
) {
    if !is_editor_visible_state_name(&id) {
        return;
    }

    push_state_expected_syntax(facts, EditorExpectedSyntaxKind::NodeIdentifier, selection);
    facts.push_symbol(EditorSemanticSymbol::reference(
        id,
        Some(detail.to_string()),
        EditorSemanticKind::Class,
        selection,
        selection,
    ));
}

fn push_state_class_definition_fact(
    facts: &mut EditorSemanticFacts,
    name: String,
    start: usize,
    end: usize,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    if name.trim().is_empty() {
        return;
    }

    let selection = SourceSpan::new(start, end);
    facts.push_symbol(EditorSemanticSymbol::class_definition(
        name,
        Some(detail.to_string()),
        kind,
        selection,
        selection,
    ));
}

fn push_state_expected_syntax(
    facts: &mut EditorSemanticFacts,
    kind: EditorExpectedSyntaxKind,
    span: SourceSpan,
) {
    if span.start > span.end {
        return;
    }

    facts.push_expected_syntax(EditorExpectedSyntax::new(kind, span));
}

fn push_state_reference_id_list_facts(
    facts: &mut EditorSemanticFacts,
    code: &str,
    fallback_ids: &str,
    span: SourceSpan,
    detail: &'static str,
    expected_kind: EditorExpectedSyntaxKind,
    control: &OperationControl,
) -> OperationControlResult<()> {
    push_state_expected_syntax(facts, expected_kind, span);

    let Some(slice) = code.get(span.start..span.end) else {
        for (index, raw_id) in fallback_ids.split(',').enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            let id = raw_id.trim();
            if !id.is_empty() {
                push_state_reference_fact_with_selection(facts, id.to_string(), span, detail);
            }
        }
        return control.checkpoint();
    };

    let mut cursor = 0usize;
    let mut parts = 0usize;
    while cursor <= slice.len() {
        if parts.is_multiple_of(128) {
            control.checkpoint()?;
        }
        parts += 1;
        let next_comma = slice[cursor..]
            .find(',')
            .map(|offset| cursor + offset)
            .unwrap_or(slice.len());
        let part = &slice[cursor..next_comma];
        let leading = part.len().saturating_sub(part.trim_start().len());
        let trailing = part.trim_end().len();
        if leading < trailing {
            let id_start = cursor + leading;
            let id_end = cursor + trailing;
            push_state_reference_fact_with_selection(
                facts,
                slice[id_start..id_end].to_string(),
                SourceSpan::new(span.start + id_start, span.start + id_end),
                detail,
            );
        }

        if next_comma == slice.len() {
            break;
        }
        cursor = next_comma + 1;
    }
    control.checkpoint()
}

fn push_state_payload_fact_from_token(
    facts: &mut EditorSemanticFacts,
    code: &str,
    value: &str,
    start: usize,
    end: usize,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    push_state_payload_fact_from_token_as(
        facts,
        code,
        value,
        SourceSpan::new(start, end),
        detail,
        kind,
        EditorExpectedSyntaxKind::Payload,
    );
}

fn push_state_payload_fact_from_token_as(
    facts: &mut EditorSemanticFacts,
    code: &str,
    value: &str,
    span: SourceSpan,
    detail: &'static str,
    kind: EditorSemanticKind,
    expected_kind: EditorExpectedSyntaxKind,
) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }

    let selection = source_value_span(code, span, value).unwrap_or(span);
    push_state_expected_syntax(facts, expected_kind, selection);
    facts.push_symbol(EditorSemanticSymbol::payload(
        value.to_string(),
        Some(detail.to_string()),
        kind,
        span,
        selection,
    ));
}

fn push_state_string_payload_fact(
    facts: &mut EditorSemanticFacts,
    value: &str,
    start: usize,
    end: usize,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    if value.is_empty() {
        return;
    }

    let span = SourceSpan::new(start, end);
    let selection = if end >= start + value.len() + 2 {
        SourceSpan::new(start + 1, start + 1 + value.len())
    } else {
        span
    };
    push_state_expected_syntax(facts, EditorExpectedSyntaxKind::Payload, selection);
    facts.push_symbol(EditorSemanticSymbol::payload(
        value.to_string(),
        Some(detail.to_string()),
        kind,
        span,
        selection,
    ));
}

fn is_editor_visible_state_name(id: &str) -> bool {
    !id.trim().is_empty() && id.trim() != "[*]"
}
