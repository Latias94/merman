lalrpop_util::lalrpop_mod!(
    #[allow(clippy::empty_line_after_outer_attr)]
    class_grammar,
    "/diagrams/class_grammar.rs"
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
pub use parse::{parse_class, parse_class_editor_facts, parse_class_typed};

pub(crate) use ast::{Action, Relation, RelationData};
pub(crate) use lexer::{LexError, Tok};
