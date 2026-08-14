use tree_sitter_mermaid::{
    LANGUAGE_ABI, LANGUAGE_SYMBOL, NODE_SCHEMA_VERSION, PACKAGE_VERSION, QUERY_SCHEMA_VERSION,
    TREE_SITTER_RUST_RUNTIME_VERSION,
};

#[test]
fn package_identity_starts_without_claiming_generated_language_support() {
    assert_eq!(PACKAGE_VERSION, "0.1.0");
    assert_eq!(LANGUAGE_SYMBOL, "mermaid");
    assert_eq!(LANGUAGE_ABI, 14);
    assert_eq!(NODE_SCHEMA_VERSION, 1);
    assert_eq!(QUERY_SCHEMA_VERSION, 1);
    assert_eq!(TREE_SITTER_RUST_RUNTIME_VERSION, "0.26.12");
}
