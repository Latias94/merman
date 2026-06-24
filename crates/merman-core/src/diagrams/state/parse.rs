use crate::{
    EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, Error, ParseMetadata, Result,
    SourceSpan,
};
use serde_json::Value;

use super::db::StateDb;
use super::{Lexer, StateDiagramRenderModel, StateStmt, Stmt, Tok};

pub fn parse_state(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut doc = super::state_grammar::RootParser::new()
        .parse(Lexer::new(code))
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("{e:?}"),
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
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("{e:?}"),
        })?;

    let mut divider_cnt = 0usize;
    assign_divider_ids(&mut doc, &mut divider_cnt);

    let mut db = StateDb::new();
    db.set_root_doc(doc);
    db.to_model_for_render_typed(meta)
}

pub fn parse_state_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    match super::state_grammar::RootParser::new().parse(Lexer::new(code)) {
        Ok(doc) => {
            let mut facts = editor_facts_from_state_ast(&doc);
            collect_state_editor_facts_from_tokens(
                code,
                StateTokenFactMode::Supplemental,
                &mut facts,
            );
            facts
        }
        Err(_) => recover_state_editor_facts_from_tokens(code),
    }
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

fn editor_facts_from_state_ast(statements: &[Stmt]) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    collect_state_editor_facts_from_statements(statements, &mut facts);
    facts
}

fn recover_state_editor_facts_from_tokens(code: &str) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    facts.mark_recovered();

    collect_state_editor_facts_from_tokens(code, StateTokenFactMode::Recovered, &mut facts);

    facts
}

#[derive(Debug, Clone, Copy)]
enum StateTokenFactMode {
    Supplemental,
    Recovered,
}

impl StateTokenFactMode {
    fn includes_plain_state_symbols(self) -> bool {
        matches!(self, Self::Recovered)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateTokenContext {
    Default,
    ClickTarget,
    ClickAfterTarget,
    ClickAfterHref,
    ClickTooltip,
}

fn collect_state_editor_facts_from_tokens(
    code: &str,
    mode: StateTokenFactMode,
    facts: &mut EditorSemanticFacts,
) {
    let mut lexer = Lexer::new(code);
    let mut collector = StateTokenFactCollector {
        code,
        mode,
        context: StateTokenContext::Default,
        note_text_pending: false,
        relation_label_pending: false,
        relation_target_seen: false,
    };
    while let Some(result) = lexer.next() {
        match result {
            Ok((start, token, end)) => {
                collector.collect_token(token, start, end, facts);
            }
            Err(_) => {}
        }
    }
}

struct StateTokenFactCollector<'a> {
    code: &'a str,
    mode: StateTokenFactMode,
    context: StateTokenContext,
    note_text_pending: bool,
    relation_label_pending: bool,
    relation_target_seen: bool,
}

fn collect_state_editor_facts_from_statements(
    statements: &[Stmt],
    facts: &mut EditorSemanticFacts,
) {
    for statement in statements {
        match statement {
            Stmt::State(state) => {
                push_state_stmt_symbol(facts, state, state_detail(state));
                if let Some(doc) = state.doc.as_deref() {
                    collect_state_editor_facts_from_statements(doc, facts);
                }
            }
            Stmt::Relation(relation) => {
                push_state_stmt_symbol(facts, &relation.state1, "state reference");
                push_state_stmt_symbol(facts, &relation.state2, "state reference");
            }
            Stmt::ClassDef { .. } => facts.push_directive_prefix("classDef"),
            Stmt::ApplyClass { .. } => facts.push_directive_prefix("class"),
            Stmt::Style { .. } => facts.push_directive_prefix("style"),
            Stmt::AccTitle(_) => facts.push_directive_prefix("accTitle"),
            Stmt::AccDescr(_) => facts.push_directive_prefix("accDescr"),
            Stmt::Click(_) => facts.push_directive_prefix("click"),
            Stmt::Noop | Stmt::Direction(_) => {}
        }
    }
}

impl StateTokenFactCollector<'_> {
    fn collect_token(
        &mut self,
        token: Tok,
        start: usize,
        end: usize,
        facts: &mut EditorSemanticFacts,
    ) {
        match token {
            Tok::Newline | Tok::StructStart | Tok::StructStop => {
                self.context = StateTokenContext::Default;
                self.note_text_pending = false;
                self.relation_label_pending = false;
                self.relation_target_seen = false;
            }
            Tok::Arrow => {
                self.relation_label_pending = true;
                self.relation_target_seen = false;
            }
            Tok::Click => {
                facts.push_directive_prefix("click");
                self.context = StateTokenContext::ClickTarget;
            }
            Tok::Href if self.context == StateTokenContext::ClickAfterTarget => {
                self.context = StateTokenContext::ClickAfterHref;
            }
            Tok::Id(id) if self.context == StateTokenContext::ClickTarget => {
                push_state_token_symbol(facts, id, start, end, "state click target");
                self.context = StateTokenContext::ClickAfterTarget;
            }
            Tok::StyledId((id, class_id)) if self.context == StateTokenContext::ClickTarget => {
                push_state_token_symbol(facts, id, start, end, "state click target");
                push_state_payload_symbol_from_token(
                    facts,
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
                if self.relation_label_pending {
                    self.relation_target_seen = true;
                }
            }
            Tok::Note => {
                self.note_text_pending = true;
            }
            Tok::NoteText(text) if self.note_text_pending => {
                push_state_payload_symbol_from_token(
                    facts,
                    self.code,
                    text.as_str(),
                    start,
                    end,
                    "state note",
                    EditorSemanticKind::String,
                );
                self.note_text_pending = false;
            }
            Tok::StateDescr(text) => {
                push_state_string_payload_symbol(
                    facts,
                    text.as_str(),
                    start,
                    end,
                    "state display label",
                    EditorSemanticKind::String,
                );
            }
            Tok::Descr(text) => {
                let detail = if self.relation_label_pending && self.relation_target_seen {
                    "state relation label"
                } else {
                    "state description"
                };
                push_state_payload_symbol_from_token(
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
                push_state_string_payload_symbol(
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
                push_state_string_payload_symbol(
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
                if self.relation_label_pending {
                    self.relation_target_seen = true;
                }
                if self.mode.includes_plain_state_symbols() {
                    push_state_token_symbol(facts, id, start, end, "state");
                }
            }
            Tok::StyledId((id, class_id)) => {
                if self.relation_label_pending {
                    self.relation_target_seen = true;
                }
                if self.mode.includes_plain_state_symbols() {
                    push_state_token_symbol(facts, id, start, end, "state");
                }
                push_state_payload_symbol_from_token(
                    facts,
                    self.code,
                    class_id.as_str(),
                    start,
                    end,
                    "state inline class",
                    EditorSemanticKind::Property,
                );
            }
            Tok::Fork(id) => {
                if self.relation_label_pending {
                    self.relation_target_seen = true;
                }
                if self.mode.includes_plain_state_symbols() {
                    push_state_token_symbol(facts, id, start, end, "state fork");
                }
            }
            Tok::Join(id) => {
                if self.relation_label_pending {
                    self.relation_target_seen = true;
                }
                if self.mode.includes_plain_state_symbols() {
                    push_state_token_symbol(facts, id, start, end, "state join");
                }
            }
            Tok::Choice(id) => {
                if self.relation_label_pending {
                    self.relation_target_seen = true;
                }
                if self.mode.includes_plain_state_symbols() {
                    push_state_token_symbol(facts, id, start, end, "state choice");
                }
            }
            Tok::ClassDef => facts.push_directive_prefix("classDef"),
            Tok::ClassDefId(id) => push_state_outline_symbol(
                facts,
                id,
                start,
                end,
                "state class definition",
                EditorSemanticKind::Property,
            ),
            Tok::ClassDefStyleOpts(raw) => push_state_payload_symbol_from_token(
                facts,
                self.code,
                raw.as_str(),
                start,
                end,
                "state class definition style",
                EditorSemanticKind::String,
            ),
            Tok::Class => facts.push_directive_prefix("class"),
            Tok::ClassEntityIds(ids) => push_state_id_list_symbols(
                facts,
                self.code,
                ids.as_str(),
                start,
                end,
                "state class target",
            ),
            Tok::StyleClass(class_id) => push_state_payload_symbol_from_token(
                facts,
                self.code,
                class_id.as_str(),
                start,
                end,
                "state class reference",
                EditorSemanticKind::Property,
            ),
            Tok::Style => facts.push_directive_prefix("style"),
            Tok::StyleIds(ids) => push_state_id_list_symbols(
                facts,
                self.code,
                ids.as_str(),
                start,
                end,
                "state style target",
            ),
            Tok::StyleDefStyleOpts(raw) => push_state_payload_symbol_from_token(
                facts,
                self.code,
                raw.as_str(),
                start,
                end,
                "state style",
                EditorSemanticKind::String,
            ),
            Tok::AccTitle(value) => {
                facts.push_directive_prefix("accTitle");
                push_state_payload_symbol_from_token(
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
                facts.push_directive_prefix("accDescr");
                push_state_payload_symbol_from_token(
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
            | Tok::LeftOf
            | Tok::RightOf
            | Tok::NoteText(_)
            | Tok::Concurrent
            | Tok::HideEmptyDescription
            | Tok::ScaleWidth(_)
            | Tok::Direction(_)
            | Tok::Href
            | Tok::StringLit(_) => {}
        }
    }
}

fn push_state_stmt_symbol(facts: &mut EditorSemanticFacts, state: &StateStmt, detail: &str) {
    if !is_editor_visible_state_id(state) {
        return;
    }

    let Some(span) = state.id_span else {
        return;
    };

    facts.push_symbol(EditorSemanticSymbol::new(
        state.id.clone(),
        Some(detail.to_string()),
        EditorSemanticKind::Class,
        span,
        span,
    ));
}

fn push_state_token_symbol(
    facts: &mut EditorSemanticFacts,
    id: String,
    start: usize,
    end: usize,
    detail: &'static str,
) {
    if !is_editor_visible_state_name(&id) {
        return;
    }

    let selection_end = start + id.len();
    let selection = SourceSpan::new(start, selection_end.min(end));
    push_state_symbol_with_selection(facts, id, selection, detail);
}

fn push_state_symbol_with_selection(
    facts: &mut EditorSemanticFacts,
    id: String,
    selection: SourceSpan,
    detail: &'static str,
) {
    if !is_editor_visible_state_name(&id) {
        return;
    }

    facts.push_symbol(EditorSemanticSymbol::new(
        id,
        Some(detail.to_string()),
        EditorSemanticKind::Class,
        selection,
        selection,
    ));
}

fn push_state_outline_symbol(
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
    facts.push_symbol(EditorSemanticSymbol::outline(
        name,
        Some(detail.to_string()),
        kind,
        selection,
        selection,
    ));
}

fn push_state_id_list_symbols(
    facts: &mut EditorSemanticFacts,
    code: &str,
    fallback_ids: &str,
    start: usize,
    end: usize,
    detail: &'static str,
) {
    let Some(slice) = code.get(start..end) else {
        for id in fallback_ids
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            push_state_symbol_with_selection(
                facts,
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
            push_state_symbol_with_selection(
                facts,
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

fn push_state_payload_symbol_from_token(
    facts: &mut EditorSemanticFacts,
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
    facts.push_symbol(EditorSemanticSymbol::payload(
        value,
        Some(detail.to_string()),
        kind,
        span,
        selection,
    ));
}

fn push_state_string_payload_symbol(
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
    facts.push_symbol(EditorSemanticSymbol::payload(
        value,
        Some(detail.to_string()),
        kind,
        span,
        selection,
    ));
}

fn token_value_selection(code: &str, start: usize, end: usize, value: &str) -> Option<SourceSpan> {
    let slice = code.get(start..end)?;
    let relative_start = slice.find(value)?;
    Some(SourceSpan::new(
        start + relative_start,
        start + relative_start + value.len(),
    ))
}

fn is_editor_visible_state_id(state: &StateStmt) -> bool {
    state.ty != "divider" && is_editor_visible_state_name(&state.id)
}

fn is_editor_visible_state_name(id: &str) -> bool {
    !id.trim().is_empty() && id.trim() != "[*]"
}

fn state_detail(state: &StateStmt) -> &'static str {
    match state.ty.as_str() {
        "fork" => "state fork",
        "join" => "state join",
        "choice" => "state choice",
        _ => "state",
    }
}
