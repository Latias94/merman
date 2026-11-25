mod ast;
pub(crate) use ast::StateDocumentScratch;
pub(crate) use model::sanitize_common_db_fields;
#[cfg(feature = "presentation")]
pub(crate) use render_model::sanitize_common_db_fields as sanitize_render_common_db_fields;
mod db;
#[cfg(test)]
#[path = "tests.rs"]
mod lifecycle_tests;
mod model;
mod parse;
#[cfg(feature = "presentation")]
mod render_model;
#[cfg(test)]
mod terminal;

pub(super) use merman_model::families::state::CSS_EDGE_NOTE_EDGE;

pub(crate) use model::{
    StateDiagramModel, StateEdge, StateLink, StateLinks, StateNode, StateNote, StateRecord,
    StateRelation, StateStyleClass,
};

#[cfg(feature = "presentation")]
pub(crate) use render_model::StateDiagramRenderModel;
#[cfg(feature = "compat-json")]
pub(crate) use render_model::render_model_to_compat_json;
#[cfg(all(test, feature = "compat-json"))]
use render_model::render_model_to_compat_json_controlled;

#[cfg(feature = "compat-json")]
pub(crate) use parse::parse_state;
#[cfg(feature = "presentation")]
pub(crate) use parse::parse_state_model_for_render_controlled;
pub(crate) use parse::parse_state_semantic_model_controlled;

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) use parse::parse_state_json_and_editor_facts;
#[cfg(all(test, feature = "compat-json", feature = "editor-facts"))]
pub(crate) use parse::{reset_state_syntax_construction_count, state_syntax_construction_count};

pub(crate) use ast::{StateDocumentId, StateStatement, StateStatementNote, StateStatementState};
