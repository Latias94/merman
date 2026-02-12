// Fixture-derived root viewport overrides for Mermaid@11.12.2 Mindmap diagrams.
//
// These values are keyed by fixture `diagram_id` and are used to close remaining
// parity-root differences on the root `<svg>` (`viewBox` + `style max-width`).

pub fn lookup_mindmap_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_decorations_and_descriptions" => {
            Some(("5 5 467.0743713378906 383.4874267578125", "467.074"))
        }
        "upstream_docs_mindmap_classes_023" => {
            Some(("5 5 217.6907958984375 243.04266357421875", "217.691"))
        }
        "upstream_docs_mindmap_circle_011" => Some(("5 5 129.078125 129.078125", "129.078")),
        "upstream_docs_mindmap_cloud_015" => Some((
            "7.269050598144531 6.441379547119141 143.2079849243164 101.05145263671875",
            "143.208",
        )),
        "upstream_docs_mindmap_default_019" => Some(("5 5 222.265625 54", "222.266")),
        "upstream_docs_mindmap_hexagon_017" => Some(("5 5 204.6432342529297 64", "204.643")),
        "upstream_docs_mindmap_icons_021" => Some(("5 5 287.67645263671875 74", "287.676")),
        "upstream_docs_example_icons_br" => {
            Some(("5 5 756.3554077148438 720.9426879882812", "756.355"))
        }
        "upstream_docs_mindmap_bang_013" => Some((
            "8.327735900878906 6.599998474121094 186.38671875 100",
            "186.387",
        )),
        "upstream_docs_mindmap_markdown_strings_028" => {
            Some(("5 5 789.57177734375 132.8335189819336", "789.572"))
        }
        "upstream_docs_mindmap_rounded_square_009" => Some(("5 5 210.15625 74", "210.156")),
        "upstream_docs_mindmap_square_007" => Some(("5 5 156.5 64", "156.5")),
        "upstream_whitespace_and_comments" => {
            Some(("5 5 317.027587890625 345.3640441894531", "317.028"))
        }
        "mmdr_tests_mindmap_basic" => Some(("5 5 530.9208984375 72.1875", "530.921")),
        "upstream_docs_tidy_tree_example_usage_001" => {
            Some(("5 5 409.72393798828125 373.72052001953125", "409.724"))
        }
        "upstream_docs_tidy_tree_example_usage_002" => {
            Some(("5 5 796.6170043945312 671.435546875", "796.617"))
        }
        "upstream_docs_intro_how_can_i_help_001" => {
            Some(("5 5 893.5901489257812 384.7295837402344", "893.59"))
        }
        _ => None,
    }
}
