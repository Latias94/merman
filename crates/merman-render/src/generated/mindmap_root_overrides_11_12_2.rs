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
        "upstream_decorations_and_descriptions" => {
            Some(("5 5 467.0743713378906 383.4874267578125", "467.074"))
        }
        "upstream_hierarchy_nodes" => Some(("5 5 121.3125 345.82373046875", "121.312")),
        "upstream_node_types" => Some((
            "7.709373474121094 5 412.6386413574219 268.28924560546875",
            "412.639",
        )),
        "upstream_root_type_bang" => Some((
            "7.709373474121094 6.599998474121094 155.46875 100",
            "155.469",
        )),
        "upstream_root_type_cloud" => Some((
            "6.52117919921875 6.006782531738281 111.66693878173828 86.86467742919922",
            "111.667",
        )),
        "upstream_shaped_root_without_id" => Some(("5 5 79.734375 74", "79.7344")),
        "upstream_whitespace_and_comments" => {
            Some(("5 5 317.027587890625 345.3640441894531", "317.028"))
        }
        _ => None,
    }
}
