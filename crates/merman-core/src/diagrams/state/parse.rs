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
        Ok(doc) => editor_facts_from_state_ast(&doc),
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

    let mut lexer = Lexer::new(code);
    while let Some(result) = lexer.next() {
        match result {
            Ok((start, token, end)) => {
                collect_state_editor_fact_from_token(token, start, end, &mut facts);
            }
            Err(_) => {}
        }
    }

    facts
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

fn collect_state_editor_fact_from_token(
    token: Tok,
    start: usize,
    end: usize,
    facts: &mut EditorSemanticFacts,
) {
    match token {
        Tok::Id(id) | Tok::CompositState(id) => {
            push_state_token_symbol(facts, id, start, end, "state");
        }
        Tok::StyledId((id, _class_id)) => {
            push_state_token_symbol(facts, id, start, end, "state");
        }
        Tok::Fork(id) => push_state_token_symbol(facts, id, start, end, "state fork"),
        Tok::Join(id) => push_state_token_symbol(facts, id, start, end, "state join"),
        Tok::Choice(id) => push_state_token_symbol(facts, id, start, end, "state choice"),
        Tok::ClassDef => facts.push_directive_prefix("classDef"),
        Tok::Class => facts.push_directive_prefix("class"),
        Tok::Style => facts.push_directive_prefix("style"),
        Tok::Click => facts.push_directive_prefix("click"),
        Tok::AccTitle(_) => facts.push_directive_prefix("accTitle"),
        Tok::AccDescr(_) | Tok::AccDescrMultiline(_) => facts.push_directive_prefix("accDescr"),
        Tok::Newline
        | Tok::Sd
        | Tok::EdgeState
        | Tok::Descr(_)
        | Tok::Arrow
        | Tok::StructStart
        | Tok::StructStop
        | Tok::As
        | Tok::Note
        | Tok::LeftOf
        | Tok::RightOf
        | Tok::NoteText(_)
        | Tok::StateDescr(_)
        | Tok::Concurrent
        | Tok::HideEmptyDescription
        | Tok::ScaleWidth(_)
        | Tok::ClassDefId(_)
        | Tok::ClassDefStyleOpts(_)
        | Tok::ClassEntityIds(_)
        | Tok::StyleClass(_)
        | Tok::StyleIds(_)
        | Tok::StyleDefStyleOpts(_)
        | Tok::Direction(_)
        | Tok::Href
        | Tok::StringLit(_) => {}
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
    facts.push_symbol(EditorSemanticSymbol::new(
        id,
        Some(detail.to_string()),
        EditorSemanticKind::Class,
        selection,
        selection,
    ));
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
