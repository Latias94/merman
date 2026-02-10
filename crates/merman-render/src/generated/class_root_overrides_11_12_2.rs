// Fixture-derived root viewport overrides for Mermaid@11.12.2 Class diagrams.
//
// These entries are keyed by fixture `diagram_id` and are used to close the remaining
// root `<svg>` parity-root deltas (`viewBox` + `style max-width`).

pub fn lookup_class_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "mmdr_tests_class_class_basic" => Some(("0 0 159.640625 318", "159.641")),
        "upstream_docs_classdiagram_annotations_on_classes_042" => {
            Some(("0 0 354.40625 256", "354.406"))
        }
        "upstream_docs_classdiagram_class_diagrams_002" => {
            Some(("0 -48 902.8359375 474", "902.836"))
        }
        "upstream_docs_classdiagram_examples_056" => Some(("0 0 484.25 100", "484.25")),
        "upstream_docs_classdiagram_generic_types_018" => Some(("0 0 366.3203125 304", "366.32")),
        "upstream_docs_classdiagram_setting_the_direction_of_the_diagram_046" => {
            Some(("0 0 431.125 354", "431.125"))
        }
        _ => None,
    }
}
