#[cfg(feature = "editor-facts")]
mod editor;
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

#[cfg(feature = "editor-facts")]
pub(crate) use editor::editor_rename_edits;
#[cfg(feature = "editor-facts")]
pub(crate) use merman_core_graph::class::editor_name_requires_backticks;

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &merman_model::families::class::ClassDiagram,
    meta: &crate::SourcePresentationMetadata,
) -> crate::Result<serde_json::Value> {
    merman_model::families::class::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}
