use crate::{Error, MAX_DIAGRAM_NESTING_DEPTH, ParseMetadata, Result};
use serde_json::Value;

use super::db::StateDb;
use super::{Lexer, StateDiagramRenderModel, Stmt};

pub fn parse_state(code: &str, meta: &ParseMetadata) -> Result<Value> {
    validate_state_source_depth(code, meta)?;
    let mut doc = super::state_grammar::RootParser::new()
        .parse(Lexer::new(code))
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("{e:?}"),
        })?;
    validate_state_doc_depth(&doc, meta)?;

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
    validate_state_source_depth(code, meta)?;
    let mut doc = super::state_grammar::RootParser::new()
        .parse(Lexer::new(code))
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("{e:?}"),
        })?;
    validate_state_doc_depth(&doc, meta)?;

    let mut divider_cnt = 0usize;
    assign_divider_ids(&mut doc, &mut divider_cnt);

    let mut db = StateDb::new();
    db.set_root_doc(doc);
    db.to_model_for_render_typed(meta)
}

fn validate_state_source_depth(code: &str, meta: &ParseMetadata) -> Result<()> {
    let mut depth = 0usize;
    for line in code.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("%%") {
            continue;
        }
        for ch in trimmed.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    if depth > MAX_DIAGRAM_NESTING_DEPTH {
                        return Err(Error::DiagramParse {
                            diagram_type: meta.diagram_type.clone(),
                            message: format!(
                                "state diagram nesting depth exceeds maximum of {MAX_DIAGRAM_NESTING_DEPTH}"
                            ),
                        });
                    }
                }
                '}' if depth > 0 => {
                    depth -= 1;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn validate_state_doc_depth(stmts: &[Stmt], meta: &ParseMetadata) -> Result<()> {
    let mut stack: Vec<(&[Stmt], usize)> = vec![(stmts, 0)];
    while let Some((doc, depth)) = stack.pop() {
        if depth > MAX_DIAGRAM_NESTING_DEPTH {
            return Err(Error::DiagramParse {
                diagram_type: meta.diagram_type.clone(),
                message: format!(
                    "state diagram nesting depth exceeds maximum of {MAX_DIAGRAM_NESTING_DEPTH}"
                ),
            });
        }
        for stmt in doc {
            if let Stmt::State(st) = stmt {
                if let Some(inner) = st.doc.as_deref() {
                    stack.push((inner, depth + 1));
                }
            }
        }
    }
    Ok(())
}

fn assign_divider_ids(stmts: &mut [Stmt], cnt: &mut usize) {
    for s in stmts.iter_mut() {
        match s {
            Stmt::State(st) => {
                if st.ty == "divider" && st.id == "__divider__" {
                    *cnt += 1;
                    st.id = format!("divider-id-{cnt}");
                }
                if let Some(doc) = st.doc.as_mut() {
                    assign_divider_ids(doc, cnt);
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
