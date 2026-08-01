include_checked_in_lalrpop_parser!(
    #[allow(clippy::empty_line_after_outer_attr)]
    class_grammar,
    "class_grammar.rs"
);

pub(crate) const LINE_SOLID: i32 = 0;
pub(crate) const LINE_DOTTED: i32 = 1;

pub(crate) const REL_AGGREGATION: i32 = 0;
pub(crate) const REL_EXTENSION: i32 = 1;
pub(crate) const REL_COMPOSITION: i32 = 2;
pub(crate) const REL_DEPENDENCY: i32 = 3;
pub(crate) const REL_LOLLIPOP: i32 = 4;
pub(crate) const REL_NONE: i32 = -1;

pub(super) const MERMAID_DOM_ID_PREFIX: &str = "classId-";

mod ast;
mod db;
mod lexer;
mod parse;

#[cfg(test)]
mod tests;

pub(crate) use parse::parse_class_json_and_editor_facts;
#[cfg(test)]
pub(crate) use parse::{class_syntax_construction_count, reset_class_syntax_construction_count};
pub(crate) use parse::{parse_class, parse_class_typed};

pub(crate) use ast::{Action, Relation, RelationData};
pub(crate) use lexer::{LexError, Tok};

pub(crate) fn render_model_to_compat_json(
    model: &crate::models::class_diagram::ClassDiagram,
    meta: &crate::ParseMetadata,
) -> crate::Result<serde_json::Value> {
    let mut value =
        serde_json::to_value(model).expect("Class typed model must remain JSON-serializable");
    value
        .as_object_mut()
        .expect("Class typed model must serialize to a JSON object")
        .insert(
            "type".to_string(),
            serde_json::Value::String(meta.diagram_type.clone()),
        );
    Ok(value)
}
