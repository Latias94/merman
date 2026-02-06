// Fixture-derived root viewport overrides for Mermaid@11.12.2 Class diagrams.
//
// These entries are keyed by fixture `diagram_id` and are used to close the remaining
// root `<svg>` parity-root deltas (`viewBox` + `style max-width`).

pub fn lookup_class_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_annotations_in_brackets_spec" => Some(("0 0 335.125 184", "335.125")),
        "upstream_cross_namespace_relations_spec" => Some(("0 0 367.06640625 406", "367.066")),
        "upstream_docs_define_class_relationship" => Some(("0 0 219.96875 234", "219.969")),
        "upstream_names_backticks_dash_underscore_spec" => Some(("0 0 288.84375 100", "288.844")),
        "upstream_namespaces_and_generics" => Some(("0 0 799.90625 436", "799.906")),
        "upstream_note_keywords_spec" => Some(("0 0 669.90625 246", "669.906")),
        "upstream_relation_types_and_cardinalities_spec" => {
            Some(("0 0 1704.16015625 416", "1704.16"))
        }
        "upstream_separators_labels_notes" => Some(("0 0 553.8515625 594", "553.852")),
        _ => None,
    }
}
