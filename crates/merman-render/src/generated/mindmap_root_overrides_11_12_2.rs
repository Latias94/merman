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
        "upstream_cypress_mindmap_tidy_tree_spec_example_001" => {
            Some(("5 5 311.59832763671875 106.109375", "311.598"))
        }
        "upstream_cypress_mindmap_tidy_tree_spec_2_tidy_tree_should_render_a_simple_mindmap_002" => {
            Some(("5 5 409.72393798828125 373.72052001953125", "409.724"))
        }
        "upstream_cypress_mindmap_tidy_tree_spec_3_tidy_tree_should_render_a_mindmap_with_different_shapes_003" => {
            Some(("5 5 1144.203369140625 700.1749877929688", "1144.2"))
        }
        "upstream_cypress_mindmap_tidy_tree_spec_4_tidy_tree_should_render_a_mindmap_with_children_004" => {
            Some(("5 5 687.355224609375 479.513671875", "687.355"))
        }
        "upstream_docs_tidy_tree_example_usage_001" => {
            Some(("5 5 409.72393798828125 373.72052001953125", "409.724"))
        }
        "upstream_docs_tidy_tree_example_usage_002" => {
            Some(("5 5 796.6170043945312 671.435546875", "796.617"))
        }
        "upstream_docs_intro_how_can_i_help_001" => {
            Some(("5 5 893.5901489257812 384.7295837402344", "893.59"))
        }
        "upstream_html_demos_mindmap_mindmap_diagram_demo_001" => {
            Some(("5 5 604.0132446289062 428.640869140625", "604.013"))
        }
        "upstream_html_demos_mindmap_mindmap_with_root_wrapping_text_and_a_shape_002" => {
            Some(("5 5 260 112", "260"))
        }
        "stress_deep_nesting_001" => Some(("5 5 765.265869140625 767.9276733398438", "765.266")),
        "stress_long_labels_br_icons_002" => {
            Some(("5 5 650.4656982421875 701.2064819335938", "650.466"))
        }
        "stress_shapes_mix_003" => Some((
            "7.105857849121094 5 681.14990234375 435.1829833984375",
            "681.15",
        )),
        "stress_unicode_punct_004" => Some(("5 5 525.7081298828125 541.6168212890625", "525.708")),
        "stress_many_siblings_005" => Some(("5 5 581.7508544921875 406.9787902832031", "581.751")),
        "stress_multiline_nodes_006" => Some(("5 5 408.6500244140625 547.2468872070312", "408.65")),
        "stress_icon_decorators_007" => Some(("5 5 327.7567138671875 524.3125", "327.757")),
        "stress_wrap_long_word_008" => Some(("5 5 1126.3408203125 324.1378173828125", "1126.34")),
        "stress_balanced_tree_009" => Some(("5 5 670.5387573242188 510.44244384765625", "670.539")),
        "stress_mixed_br_and_shapes_010" => {
            Some(("5 5 360.8953552246094 522.654541015625", "360.895"))
        }
        "stress_deep_wide_combo_011" => {
            Some(("5 5 785.1439819335938 678.3199462890625", "785.144"))
        }
        "stress_label_escaping_012" => Some(("5 5 623.0265502929688 363.4689025878906", "623.027")),
        "stress_mindmap_html_sanitization_013" => {
            Some(("5 5 232.828125 259.7193298339844", "232.828"))
        }
        "stress_mindmap_markdown_emphasis_icons_014" => {
            Some(("5 5 260.025146484375 331.54632568359375", "260.025"))
        }
        "stress_mindmap_many_siblings_decorators_015" => {
            Some(("5 5 373.0360412597656 290.3151550292969", "373.036"))
        }
        "stress_mindmap_long_words_wrapping_016" => {
            Some(("5 5 777.25 489.2870178222656", "777.25"))
        }
        "stress_mindmap_deep_mixed_shapes_017" => {
            Some(("5 5 166.3125 946.7999877929688", "166.312"))
        }
        "stress_mindmap_whitespace_comments_indent_018" => {
            Some(("5 5 435.1615905761719 589.0218505859375", "435.162"))
        }
        "stress_mindmap_delimiters_and_quotes_019" => {
            Some(("5 5 271.44976806640625 549.0762329101562", "271.45"))
        }
        "stress_mindmap_unicode_punct_020" => {
            Some(("5 5 641.8223266601562 240.2651824951172", "641.822"))
        }
        "stress_mindmap_multiline_markdown_021" => {
            Some(("5 5 740.1551513671875 131.41211700439453", "740.155"))
        }
        "stress_mindmap_proto_like_ids_022" => {
            Some(("5 5 142.109375 461.2441101074219", "142.109"))
        }
        "stress_mindmap_icon_class_order_023" => {
            Some(("5 5 358.48895263671875 72.1875", "358.489"))
        }
        "stress_mindmap_wide_tree_mixed_labels_024" => Some((
            "5 6.599998474121094 710.1619873046875 462.2999572753906",
            "710.162",
        )),
        _ => None,
    }
}
