//! Tree-sitter language support for Mermaid source.

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_mermaid() -> *const ();
}

/// The generated Tree-sitter Mermaid language function.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_mermaid) };

/// Generated public node types.
pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");

/// Canonical portable highlight query.
pub const HIGHLIGHTS_QUERY: &str = include_str!("../../queries/portable/highlights.scm");

/// Canonical portable injection query.
pub const INJECTIONS_QUERY: &str = include_str!("../../queries/portable/injections.scm");

/// Canonical portable locals query.
pub const LOCALS_QUERY: &str = include_str!("../../queries/portable/locals.scm");

/// Canonical portable tags query.
pub const TAGS_QUERY: &str = include_str!("../../queries/portable/tags.scm");

#[cfg(test)]
mod tests {
    use super::LANGUAGE;

    #[test]
    fn language_loads_and_parses_mermaid() {
        let language: tree_sitter::Language = LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language)
            .expect("generated Mermaid language must load");
        let tree = parser
            .parse("flowchart TD\nA --> B\n", None)
            .expect("parser must produce a tree");
        assert_eq!(tree.root_node().kind(), "source_file");
        assert!(!tree.root_node().has_error());
    }
}
