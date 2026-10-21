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

pub(crate) const MERMAID_DOM_ID_PREFIX: &str = "classId-";

mod ast;
mod db;
mod lexer;
mod parse;

#[cfg(all(test, feature = "presentation"))]
mod tests;

pub use crate::models::class_diagram::ClassDiagramModel;
#[cfg(feature = "compat-json")]
pub(crate) use parse::parse_class;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) use parse::parse_class_json_and_editor_facts;
pub(crate) use parse::parse_class_semantic_model_controlled;
#[cfg(feature = "presentation")]
pub(crate) use parse::parse_class_typed_with_render_context_controlled;
#[cfg(test)]
pub(crate) use parse::{class_syntax_construction_count, reset_class_syntax_construction_count};

pub(crate) use ast::{Action, Relation, RelationData};
pub(crate) use lexer::{LexError, Tok};

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &crate::models::class_diagram::ClassDiagram,
    meta: &crate::ParseMetadata,
) -> crate::Result<serde_json::Value> {
    let mut value =
        serde_json::to_value(model).expect("Class typed model must remain JSON-serializable");
    if let Some(relations) = value
        .get_mut("relations")
        .and_then(serde_json::Value::as_array_mut)
    {
        for relation in relations {
            if let Some(relation) = relation.as_object_mut() {
                for field in ["relationTitle1", "relationTitle2"] {
                    if relation.get(field).is_some_and(serde_json::Value::is_null) {
                        relation.insert(
                            field.to_string(),
                            serde_json::Value::String("none".to_string()),
                        );
                    }
                }
            }
        }
    }
    let object = value
        .as_object_mut()
        .expect("Class typed model must serialize to a JSON object");
    object.remove("namespaceFacadeAliases");
    object.insert(
        "type".to_string(),
        serde_json::Value::String(meta.diagram_type.clone()),
    );
    Ok(value)
}
