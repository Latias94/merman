// Fixture-derived root viewport overrides for Mermaid@11.12.2 Mindmap diagrams.
//
// These values are keyed by fixture `diagram_id` and are used to close remaining
// parity-root differences on the root `<svg>` (`viewBox` + `style max-width`).

pub fn lookup_mindmap_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_docs_unclear_indentation" => {
            Some(("5 5 242.63980102539062 210.3271942138672", "242.64"))
        }
        "upstream_shaped_root_without_id" => Some(("5 5 79.734375 74", "79.7344")),
        "upstream_whitespace_and_comments" => {
            Some(("5 5 317.027587890625 345.3640441894531", "317.028"))
        }
        _ => None,
    }
}
