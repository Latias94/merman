use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
    editor::{format_lalrpop_parse_error, lalrpop_parse_diagnostic, lalrpop_recovery_span},
};
use serde_json::Value;

use super::db::StateDb;
use super::{Lexer, StateDiagramRenderModel, Stmt, Tok};

pub fn parse_state(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut doc = super::state_grammar::RootParser::new()
        .parse(Lexer::new(code))
        .map_err(|e| {
            Error::diagram_parse_diagnostic(
                meta.diagram_type.clone(),
                lalrpop_parse_diagnostic(&e, code.len()),
            )
        })?;

    let mut divider_cnt = 0usize;
    assign_divider_ids(&mut doc, &mut divider_cnt);

    let mut db = StateDb::new();
    db.set_root_doc(doc);
    db.to_model(meta)
}

pub fn parse_state_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<StateDiagramRenderModel> {
    let mut doc = super::state_grammar::RootParser::new()
        .parse(Lexer::new(code))
        .map_err(|e| {
            Error::diagram_parse_diagnostic(
                meta.diagram_type.clone(),
                lalrpop_parse_diagnostic(&e, code.len()),
            )
        })?;

    let mut divider_cnt = 0usize;
    assign_divider_ids(&mut doc, &mut divider_cnt);

    let mut db = StateDb::new();
    db.set_root_doc(doc);
    db.to_model_for_render_typed(meta)
}

pub fn parse_state_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    let parse_result = super::state_grammar::RootParser::new().parse(Lexer::new(code));
    let mut facts = state_editor_facts_from_events(collect_state_editor_events(code));
    if let Err(error) = parse_result {
        let span = lalrpop_recovery_span(&error, code.len());
        facts.mark_recovered_from_parse_error(
            format!(
                "state parser recovered after parse error: {}",
                format_lalrpop_parse_error(&error)
            ),
            Some(span),
        );
    }
    facts
}

fn assign_divider_ids(stmts: &mut [Stmt], cnt: &mut usize) {
    let mut stack = vec![stmts.iter_mut()];
    while let Some(iter) = stack.last_mut() {
        let Some(stmt) = iter.next() else {
            stack.pop();
            continue;
        };

        match stmt {
            Stmt::State(st) => {
                if st.ty == "divider" && st.id == "__divider__" {
                    *cnt += 1;
                    st.id = format!("divider-id-{cnt}");
                }
                if let Some(doc) = st.doc.as_mut() {
                    stack.push(doc.iter_mut());
                }
            }
            Stmt::Relation(relation) => {
                if relation.state1.ty == "divider" && relation.state1.id == "__divider__" {
                    *cnt += 1;
                    relation.state1.id = format!("divider-id-{cnt}");
                }
                if relation.state2.ty == "divider" && relation.state2.id == "__divider__" {
                    *cnt += 1;
                    relation.state2.id = format!("divider-id-{cnt}");
                }
            }
            _ => {}
        }
    }
}

fn state_editor_facts_from_events(events: Vec<StateEditorEvent>) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    for event in events {
        event.emit(&mut facts);
    }
    facts
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
enum StateEditorEvent {
    DirectivePrefix(&'static str),
    ExpectedSyntax {
        kind: EditorExpectedSyntaxKind,
        span: SourceSpan,
    },
    Entity {
        name: String,
        selection: SourceSpan,
        detail: &'static str,
        kind: EditorSemanticKind,
    },
    Outline {
        name: String,
        selection: SourceSpan,
        detail: &'static str,
        kind: EditorSemanticKind,
    },
    Payload {
        value: String,
        span: SourceSpan,
        selection: SourceSpan,
        detail: &'static str,
        kind: EditorSemanticKind,
    },
}

impl StateEditorEvent {
    fn emit(self, facts: &mut EditorSemanticFacts) {
        match self {
            Self::DirectivePrefix(prefix) => facts.push_directive_prefix(prefix),
            Self::ExpectedSyntax { kind, span } => {
                facts.push_expected_syntax(EditorExpectedSyntax::new(kind, span))
            }
            Self::Entity {
                name,
                selection,
                detail,
                kind,
            } => facts.push_symbol(EditorSemanticSymbol::new(
                name,
                Some(detail.to_string()),
                kind,
                selection,
                selection,
            )),
            Self::Outline {
                name,
                selection,
                detail,
                kind,
            } => facts.push_symbol(EditorSemanticSymbol::outline(
                name,
                Some(detail.to_string()),
                kind,
                selection,
                selection,
            )),
            Self::Payload {
                value,
                span,
                selection,
                detail,
                kind,
            } => facts.push_symbol(EditorSemanticSymbol::payload(
                value,
                Some(detail.to_string()),
                kind,
                span,
                selection,
            )),
        }
    }
}

#[derive(Debug)]
struct StatePendingEntity {
    name: String,
    selection: SourceSpan,
    detail: &'static str,
    kind: EditorSemanticKind,
}

fn collect_state_editor_events(code: &str) -> Vec<StateEditorEvent> {
    let mut lexer = Lexer::new(code);
    let mut collector = StateTokenFactCollector {
        code,
        context: StateTokenContext::Default,
        pending_entity: None,
        note_text_pending: false,
        note_position_seen: false,
        note_alias_pending: false,
        relation_label_pending: false,
        relation_target_seen: false,
    };
    let mut events = Vec::new();
    let mut last_position = lexer.position();
    while let Some(result) = lexer.next() {
        let current_position = lexer.position();
        match result {
            Ok((start, token, end)) => {
                collector.collect_token(token, start, end, &mut events);
            }
            Err(_) => {
                if current_position == last_position {
                    break;
                }
            }
        }
        last_position = current_position;
    }
    collector.flush_pending_entity(&mut events);
    events
}

struct StateTokenFactCollector<'a> {
    code: &'a str,
    context: StateTokenContext,
    pending_entity: Option<StatePendingEntity>,
    note_text_pending: bool,
    note_position_seen: bool,
    note_alias_pending: bool,
    relation_label_pending: bool,
    relation_target_seen: bool,
}

impl StateTokenFactCollector<'_> {
    fn collect_token(
        &mut self,
        token: Tok,
        start: usize,
        end: usize,
        events: &mut Vec<StateEditorEvent>,
    ) {
        match token {
            Tok::Newline | Tok::StructStart | Tok::StructStop => {
                self.finish_statement(events);
            }
            Tok::Arrow => {
                self.flush_pending_entity_as("state reference", events);
                self.relation_label_pending = true;
                self.relation_target_seen = false;
            }
            Tok::Click => {
                self.flush_pending_entity(events);
                events.push(StateEditorEvent::DirectivePrefix("click"));
                self.context = StateTokenContext::ClickTarget;
            }
            Tok::Href if self.context == StateTokenContext::ClickAfterTarget => {
                self.context = StateTokenContext::ClickAfterHref;
            }
            Tok::Id(id) if self.context == StateTokenContext::ClickTarget => {
                push_state_entity_event(events, id, start, end, "state click target");
                self.context = StateTokenContext::ClickAfterTarget;
            }
            Tok::StyledId((id, class_id)) if self.context == StateTokenContext::ClickTarget => {
                push_state_entity_event(events, id, start, end, "state click target");
                push_state_payload_event_from_token(
                    events,
                    self.code,
                    class_id.as_str(),
                    start,
                    end,
                    "state inline class",
                    EditorSemanticKind::Property,
                );
                self.context = StateTokenContext::ClickAfterTarget;
            }
            Tok::EdgeState if self.context == StateTokenContext::ClickTarget => {
                self.context = StateTokenContext::ClickAfterTarget;
            }
            Tok::EdgeState => {
                self.flush_pending_entity(events);
                if self.relation_label_pending {
                    self.relation_target_seen = true;
                }
            }
            Tok::Note => {
                self.flush_pending_entity(events);
                self.note_text_pending = true;
                self.note_position_seen = false;
                self.note_alias_pending = false;
            }
            Tok::LeftOf | Tok::RightOf => {
                self.note_position_seen = true;
            }
            Tok::NoteText(text) if self.note_text_pending => {
                self.flush_pending_entity(events);
                push_state_payload_event_from_token(
                    events,
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
                self.flush_pending_entity(events);
                push_state_string_payload_event(
                    events,
                    text.as_str(),
                    start,
                    end,
                    "state display label",
                    EditorSemanticKind::String,
                );
            }
            Tok::Descr(text) => {
                self.flush_pending_entity(events);
                let detail = if self.relation_label_pending && self.relation_target_seen {
                    "state relation label"
                } else {
                    "state description"
                };
                push_state_payload_event_from_token(
                    events,
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
                self.flush_pending_entity(events);
                push_state_string_payload_event(
                    events,
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
                push_state_string_payload_event(
                    events,
                    text.as_str(),
                    start,
                    end,
                    "state click tooltip",
                    EditorSemanticKind::String,
                );
                self.context = StateTokenContext::Default;
            }
            Tok::Id(id) | Tok::CompositState(id) => {
                self.handle_state_entity(id, start, end, "state", events);
            }
            Tok::StyledId((id, class_id)) => {
                self.handle_state_entity(id, start, end, "state", events);
                push_state_payload_event_from_token(
                    events,
                    self.code,
                    class_id.as_str(),
                    start,
                    end,
                    "state inline class",
                    EditorSemanticKind::Property,
                );
            }
            Tok::Fork(id) => {
                self.handle_state_entity(id, start, end, "state fork", events);
            }
            Tok::Join(id) => {
                self.handle_state_entity(id, start, end, "state join", events);
            }
            Tok::Choice(id) => {
                self.handle_state_entity(id, start, end, "state choice", events);
            }
            Tok::ClassDef => {
                self.flush_pending_entity(events);
                events.push(StateEditorEvent::DirectivePrefix("classDef"));
            }
            Tok::ClassDefId(id) => {
                push_state_expected_syntax(
                    events,
                    EditorExpectedSyntaxKind::Payload,
                    SourceSpan::new(start, end),
                );
                push_state_outline_event(
                    events,
                    id,
                    start,
                    end,
                    "state class definition",
                    EditorSemanticKind::Property,
                );
            }
            Tok::ClassDefStyleOpts(raw) => push_state_payload_event_from_token(
                events,
                self.code,
                raw.as_str(),
                start,
                end,
                "state class definition style",
                EditorSemanticKind::String,
            ),
            Tok::Class => {
                self.flush_pending_entity(events);
                events.push(StateEditorEvent::DirectivePrefix("class"));
            }
            Tok::ClassEntityIds(ids) => push_state_id_list_events(
                events,
                self.code,
                ids.as_str(),
                start,
                end,
                "state class target",
            ),
            Tok::StyleClass(class_id) => push_state_payload_event_from_token(
                events,
                self.code,
                class_id.as_str(),
                start,
                end,
                "state class reference",
                EditorSemanticKind::Property,
            ),
            Tok::Style => {
                self.flush_pending_entity(events);
                events.push(StateEditorEvent::DirectivePrefix("style"));
            }
            Tok::StyleIds(ids) => push_state_id_list_events(
                events,
                self.code,
                ids.as_str(),
                start,
                end,
                "state style target",
            ),
            Tok::StyleDefStyleOpts(raw) => push_state_payload_event_from_token(
                events,
                self.code,
                raw.as_str(),
                start,
                end,
                "state style",
                EditorSemanticKind::String,
            ),
            Tok::AccTitle(value) => {
                self.flush_pending_entity(events);
                events.push(StateEditorEvent::DirectivePrefix("accTitle"));
                push_state_payload_event_from_token(
                    events,
                    self.code,
                    value.as_str(),
                    start,
                    end,
                    "state accessibility title",
                    EditorSemanticKind::String,
                );
            }
            Tok::AccDescr(value) | Tok::AccDescrMultiline(value) => {
                self.flush_pending_entity(events);
                events.push(StateEditorEvent::DirectivePrefix("accDescr"));
                push_state_payload_event_from_token(
                    events,
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
            | Tok::Direction(_)
            | Tok::Href
            | Tok::StringLit(_) => {}
        }
    }

    fn handle_state_entity(
        &mut self,
        id: String,
        start: usize,
        end: usize,
        default_detail: &'static str,
        events: &mut Vec<StateEditorEvent>,
    ) {
        if self.note_alias_pending {
            self.note_alias_pending = false;
            push_state_expected_syntax(
                events,
                EditorExpectedSyntaxKind::Payload,
                SourceSpan::new(start, end),
            );
            return;
        }

        if self.relation_label_pending {
            self.flush_pending_entity(events);
            push_state_entity_event(events, id, start, end, "state reference");
            self.relation_target_seen = true;
            return;
        }

        self.queue_state_entity(id, start, end, default_detail, events);
    }

    fn queue_state_entity(
        &mut self,
        id: String,
        start: usize,
        end: usize,
        detail: &'static str,
        events: &mut Vec<StateEditorEvent>,
    ) {
        self.flush_pending_entity(events);
        let Some(entity) = state_pending_entity(id, start, end, detail) else {
            return;
        };
        push_state_expected_syntax(
            events,
            EditorExpectedSyntaxKind::NodeIdentifier,
            entity.selection,
        );
        self.pending_entity = Some(entity);
    }

    fn flush_pending_entity(&mut self, events: &mut Vec<StateEditorEvent>) {
        if let Some(entity) = self.pending_entity.take() {
            events.push(StateEditorEvent::Entity {
                name: entity.name,
                selection: entity.selection,
                detail: entity.detail,
                kind: entity.kind,
            });
        }
    }

    fn flush_pending_entity_as(
        &mut self,
        detail: &'static str,
        events: &mut Vec<StateEditorEvent>,
    ) {
        if let Some(entity) = self.pending_entity.take() {
            events.push(StateEditorEvent::Entity {
                name: entity.name,
                selection: entity.selection,
                detail,
                kind: entity.kind,
            });
        }
    }

    fn finish_statement(&mut self, events: &mut Vec<StateEditorEvent>) {
        self.flush_pending_entity(events);
        self.context = StateTokenContext::Default;
        self.note_text_pending = false;
        self.note_position_seen = false;
        self.note_alias_pending = false;
        self.relation_label_pending = false;
        self.relation_target_seen = false;
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

fn push_state_entity_event(
    events: &mut Vec<StateEditorEvent>,
    id: String,
    start: usize,
    end: usize,
    detail: &'static str,
) {
    let Some(entity) = state_pending_entity(id, start, end, detail) else {
        return;
    };
    push_state_expected_syntax(
        events,
        EditorExpectedSyntaxKind::NodeIdentifier,
        entity.selection,
    );
    events.push(StateEditorEvent::Entity {
        name: entity.name,
        selection: entity.selection,
        detail: entity.detail,
        kind: entity.kind,
    });
}

fn push_state_entity_event_with_selection(
    events: &mut Vec<StateEditorEvent>,
    id: String,
    selection: SourceSpan,
    detail: &'static str,
) {
    if !is_editor_visible_state_name(&id) {
        return;
    }

    push_state_expected_syntax(events, EditorExpectedSyntaxKind::NodeIdentifier, selection);
    events.push(StateEditorEvent::Entity {
        name: id,
        selection,
        detail,
        kind: EditorSemanticKind::Class,
    });
}

fn push_state_outline_event(
    events: &mut Vec<StateEditorEvent>,
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
    events.push(StateEditorEvent::Outline {
        name,
        selection,
        detail,
        kind,
    });
}

fn push_state_expected_syntax(
    events: &mut Vec<StateEditorEvent>,
    kind: EditorExpectedSyntaxKind,
    span: SourceSpan,
) {
    if span.start >= span.end {
        return;
    }

    events.push(StateEditorEvent::ExpectedSyntax { kind, span });
}

fn push_state_id_list_events(
    events: &mut Vec<StateEditorEvent>,
    code: &str,
    fallback_ids: &str,
    start: usize,
    end: usize,
    detail: &'static str,
) {
    push_state_expected_syntax(
        events,
        EditorExpectedSyntaxKind::IdList,
        SourceSpan::new(start, end),
    );

    let Some(slice) = code.get(start..end) else {
        for id in fallback_ids
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            push_state_entity_event_with_selection(
                events,
                id.to_string(),
                SourceSpan::new(start, end),
                detail,
            );
        }
        return;
    };

    let mut cursor = 0usize;
    while cursor <= slice.len() {
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
            push_state_entity_event_with_selection(
                events,
                slice[id_start..id_end].to_string(),
                SourceSpan::new(start + id_start, start + id_end),
                detail,
            );
        }

        if next_comma == slice.len() {
            break;
        }
        cursor = next_comma + 1;
    }
}

fn push_state_payload_event_from_token(
    events: &mut Vec<StateEditorEvent>,
    code: &str,
    value: &str,
    start: usize,
    end: usize,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }

    let span = SourceSpan::new(start, end);
    let selection = token_value_selection(code, start, end, value).unwrap_or(span);
    push_state_expected_syntax(events, EditorExpectedSyntaxKind::Payload, selection);
    events.push(StateEditorEvent::Payload {
        value: value.to_string(),
        span,
        selection,
        detail,
        kind,
    });
}

fn push_state_string_payload_event(
    events: &mut Vec<StateEditorEvent>,
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
    push_state_expected_syntax(events, EditorExpectedSyntaxKind::Payload, selection);
    events.push(StateEditorEvent::Payload {
        value: value.to_string(),
        span,
        selection,
        detail,
        kind,
    });
}

fn token_value_selection(code: &str, start: usize, end: usize, value: &str) -> Option<SourceSpan> {
    let slice = code.get(start..end)?;
    let relative_start = slice.find(value)?;
    Some(SourceSpan::new(
        start + relative_start,
        start + relative_start + value.len(),
    ))
}

fn is_editor_visible_state_name(id: &str) -> bool {
    !id.trim().is_empty() && id.trim() != "[*]"
}
