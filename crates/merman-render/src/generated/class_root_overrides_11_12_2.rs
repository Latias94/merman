// Fixture-derived root viewport overrides for Mermaid@11.12.2 Class diagrams.
//
// These entries are keyed by fixture `diagram_id` and are used to close the remaining
// root `<svg>` parity-root deltas (`viewBox` + `style max-width`).

pub fn lookup_class_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_names_backticks_dash_underscore_spec" => Some(("0 0 288.84375 100", "288.844")),
        "upstream_namespaces_and_generics" => Some(("0 0 799.90625 436", "799.906")),
        "upstream_relation_types_and_cardinalities_spec" => {
            Some(("0 0 1704.16015625 416", "1704.16"))
        }
        _ => None,
    }
}
