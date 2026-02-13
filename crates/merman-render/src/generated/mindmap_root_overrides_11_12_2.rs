// Fixture-derived root viewport overrides for Mermaid@11.12.2 Mindmap diagrams.
//
// These values are keyed by fixture `diagram_id` and are used to close remaining
// parity-root differences on the root `<svg>` (`viewBox` + `style max-width`).

pub fn lookup_mindmap_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_cypress_mindmap_spec_a_root_with_a_shape_002" => {
            Some(("5 5 89.734375 64", "89.7344"))
        }
        "upstream_cypress_mindmap_spec_a_root_with_an_icon_005" => {
            Some(("5 5 89.734375 64", "89.7344"))
        }
        "upstream_cypress_mindmap_spec_a_root_with_wrapping_text_and_a_shape_003" => {
            Some(("5 5 260 112", "260"))
        }
        "upstream_cypress_mindmap_spec_a_root_with_wrapping_text_and_long_words_that_exceed_width_004" => {
            Some(("5 5 458.5 136", "458.5"))
        }
        "upstream_cypress_mindmap_spec_adding_children_015" => {
            Some(("5 5 394.80145263671875 54", "394.801"))
        }
        "upstream_cypress_mindmap_spec_adding_grand_children_016" => {
            Some(("5 5 305.62548828125 210.70932006835938", "305.625"))
        }
        "upstream_cypress_mindmap_spec_blang_and_cloud_shape_006" => Some((
            "6.561412811279297 6.599998474121094 503.568115234375 100",
            "503.568",
        )),
        "upstream_cypress_mindmap_spec_blang_and_cloud_shape_with_icons_007" => Some((
            "6.561412811279297 6.599998474121094 503.568115234375 100",
            "503.568",
        )),
        "upstream_cypress_mindmap_spec_braches_008" => {
            Some(("5 5 611.6260375976562 360.7017517089844", "611.626"))
        }
        "upstream_cypress_mindmap_spec_braches_with_shapes_and_labels_009" => {
            Some(("5 5 615.91748046875 440.98748779296875", "615.917"))
        }
        "upstream_cypress_mindmap_spec_circle_shape_013" => Some(("5 5 111.3125 74", "111.312")),
        "upstream_cypress_mindmap_spec_default_shape_014" => Some(("5 5 121.3125 54", "121.312")),
        "upstream_cypress_mindmap_spec_example_001" => Some(("5 5 89.734375 54", "89.7344")),
        "upstream_cypress_mindmap_spec_formatted_label_with_linebreak_and_a_wrapping_label_and_emojis_017" => {
            Some(("5 5 553.4945068359375 112", "553.495"))
        }
        "upstream_cypress_mindmap_spec_has_a_label_with_char_sequence_graph_018" => {
            Some(("5 5 357.99908447265625 369.02362060546875", "357.999"))
        }
        "upstream_cypress_mindmap_spec_rounded_rect_shape_012" => {
            Some(("5 5 101.3125 101.3125", "101.312"))
        }
        "upstream_cypress_mindmap_spec_square_shape_011" => Some(("5 5 121.3125 64", "121.312")),
        "upstream_cypress_mindmap_spec_text_should_wrap_with_icon_010" => {
            Some(("5 5 373.2288513183594 146", "373.229"))
        }
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
