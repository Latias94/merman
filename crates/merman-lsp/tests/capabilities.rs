use std::{fs, path::PathBuf};

#[test]
fn capability_matrix_document_marks_first_class_families_mature() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/lsp/CAPABILITIES.md");
    let contents =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));

    for expected in [
        "| Flowchart | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Sequence | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| State | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Class | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| ER | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Mindmap | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Gantt | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Architecture | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| GitGraph | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Kanban | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Radar | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Treemap | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Block | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| C4 | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| ZenUML | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Ishikawa | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Journey | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Info | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Timeline | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Pie | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Packet | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Sankey | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Tree View | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Event Modeling | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Quadrant Chart | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Requirement | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Venn | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| XY Chart | Yes | Yes | Yes | Yes | Yes | Yes |",
    ] {
        assert!(
            contents.contains(expected),
            "capability matrix is missing mature row: {expected}"
        );
    }
}

#[test]
fn capability_matrix_document_marks_partial_families_outside_first_class_contract() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/lsp/CAPABILITIES.md");
    let contents =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));

    assert!(
        contents.contains("## Coverage Boundary"),
        "capability matrix is missing the coverage boundary section"
    );

    for expected in ["| Error | Internal only |"] {
        assert!(
            contents.contains(expected),
            "capability matrix is missing partial-family row: {expected}"
        );
    }
}
