include_checked_in_lalrpop_parser!(
    #[allow(clippy::empty_line_after_outer_attr)]
    class_grammar,
    "class_grammar.rs"
);

pub(crate) use merman_model::families::class::{
    LINE_DOTTED, LINE_SOLID, MERMAID_DOM_ID_PREFIX, REL_AGGREGATION, REL_COMPOSITION,
    REL_DEPENDENCY, REL_EXTENSION, REL_LOLLIPOP, REL_NONE,
};

mod ast;
mod db;
mod lexer;
mod parse;
pub(crate) mod source_data;

#[cfg(all(test, feature = "presentation"))]
mod tests;

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
    model: &merman_model::families::class::ClassDiagram,
    meta: &crate::SourcePresentationMetadata,
) -> crate::Result<serde_json::Value> {
    merman_model::families::class::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}
