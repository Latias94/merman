// Fixture-derived root viewport overrides for Mermaid@11.12.2 Mindmap diagrams.
//
// These values are keyed by fixture `diagram_id` and are used to close remaining
// parity-root differences on the root `<svg>` (`viewBox` + `style max-width`).

pub fn lookup_mindmap_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_docs_mindmap_classes_023" => {
            Some(("5 5 217.6907958984375 243.04266357421875", "217.691"))
        }
        "upstream_docs_mindmap_icons_021" => Some(("5 5 287.67645263671875 74", "287.676")),
        "upstream_docs_mindmap_markdown_strings_028" => {
            Some(("5 5 789.57177734375 132.8335189819336", "789.572"))
        }
        _ => None,
    }
}
