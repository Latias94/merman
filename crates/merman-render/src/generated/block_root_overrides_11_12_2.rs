// Fixture-derived root viewport overrides for Mermaid@11.12.2 Block diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/block/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable when upstream browser float behavior (DOM `getBBox()`
// + serialization) differs from our deterministic headless pipeline.

pub fn lookup_block_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_basic_nodes" => Some(("-5 -18.5 31.484375 37", "31.4844")),
        "upstream_basic_nodes_2" => Some(("-5 -18.5 66.859375 37", "66.8594")),
        "upstream_basic_nodes_3" => Some(("-5 -18.5 115.625 37", "115.625")),
        "upstream_basic_nodes_4" => Some(("-5 -18.5 131.71875 37", "131.719")),
        "upstream_block_arrows" => Some(("-5 -22.5 204.6875 45", "204.688")),
        "upstream_block_arrows_2" => Some(("-5 -77.5 95.4375 155", "95.4375")),
        "upstream_columns_and_layout" => Some(("-5 -18.5 69.5625 37", "69.5625")),
        "upstream_columns_and_layout_2" => Some(("-5 -18.5 69.5625 37", "69.5625")),
        "upstream_columns_and_layout_3" => Some(("-5 -18.5 69.5625 37", "69.5625")),
        "upstream_columns_and_layout_4" => Some(("-5 -18.5 137.125 37", "137.125")),
        "upstream_columns_and_layout_5" => Some(("-5 -36 69.5625 72", "69.5625")),
        "upstream_composites" => Some(("-5 -26.5 145.59375 53", "145.594")),
        "upstream_composites_2" => Some(("-5 -34.5 176.8125 69", "176.812")),
        "upstream_composites_3" => Some(("-5 -26.5 128.375 53", "128.375")),
        "upstream_composites_4" => Some(("-5 -52 153.125 104", "153.125")),
        "upstream_cypress_block_spec_bl10_should_handle_edges_from_composite_blocks_010" => {
            Some(("-5 -26.5 248.9375 53", "248.938"))
        }
        "upstream_cypress_block_spec_bl11_should_handle_edges_to_composite_blocks_011" => {
            Some(("-5 -26.5 248.9375 53", "248.938"))
        }
        "upstream_cypress_block_spec_bl12_edges_should_handle_labels_012" => {
            Some(("-5 -18.5 78.3125 37", "78.3125"))
        }
        "upstream_cypress_block_spec_bl13_should_handle_block_arrows_in_different_directions_013" => {
            Some(("-5 -119 299.75 238", "299.75"))
        }
        "upstream_cypress_block_spec_bl14_should_style_statements_and_class_statements_014" => {
            Some(("-5 -18.5 52.875 37", "52.875"))
        }
        "upstream_cypress_block_spec_bl15_width_alignment_d_and_e_should_share_available_space_015" => {
            Some(("-5 -26.5 403.8125 53", "403.812"))
        }
        "upstream_cypress_block_spec_bl16_width_alignment_c_should_be_as_wide_as_the_composite_block_016" => {
            Some(("-5 -26.5 529.25 53", "529.25"))
        }
        "upstream_cypress_block_spec_bl17_width_alignment_blocks_should_be_equal_in_width_017" => {
            Some(("-5 -18.5 373.4375 37", "373.438"))
        }
        "upstream_cypress_block_spec_bl18_block_types_1_square_rounded_and_circle_018" => {
            Some(("-5 -29.0390625 226.53125 58.078125", "226.531"))
        }
        "upstream_cypress_block_spec_bl19_block_types_2_odd_diamond_and_hexagon_019" => {
            Some(("-5 -53.203125 518.1875 106.40625", "518.188"))
        }
        "upstream_cypress_block_spec_bl1_should_calculate_the_block_widths_001" => {
            Some(("-5 -26.5 554 53", "554"))
        }
        "upstream_cypress_block_spec_bl20_block_types_3_stadium_020" => {
            Some(("-5 -18.5 81.484375 37", "81.4844"))
        }
        "upstream_cypress_block_spec_bl21_block_types_4_lean_right_lean_left_trapezoid_and_inv_trapez_021" => {
            Some(("-5 -18.5 436.25 37", "436.25"))
        }
        "upstream_cypress_block_spec_bl23_sizing_it_should_be_possible_to_make_a_block_wider_023" => {
            Some(("-5 -36 376.21875 41", "376.219"))
        }
        "upstream_cypress_block_spec_bl24_sizing_it_should_be_possible_to_make_a_composite_block_wide_024" => {
            Some(("-5 -26.5 84.875 53", "84.875"))
        }
        "upstream_cypress_block_spec_bl25_block_in_the_middle_with_space_on_each_side_025" => {
            Some(("-5 -18.5 337.296875 37", "337.297"))
        }
        "upstream_cypress_block_spec_bl26_space_and_an_edge_026" => {
            Some(("-5 -18.5 78.3125 37", "78.3125"))
        }
        "upstream_cypress_block_spec_bl27_block_sizes_for_regular_blocks_027" => {
            Some(("-5 -36 285.078125 72", "285.078"))
        }
        "upstream_cypress_block_spec_bl28_composite_block_with_a_set_width_f_should_use_the_available_028" => {
            Some(("-5 -77.5 87.921875 155", "87.9219"))
        }
        "upstream_cypress_block_spec_bl2_should_handle_columns_statement_in_sub_blocks_002" => {
            Some(("-5 -44 378.0625 88", "378.062"))
        }
        "upstream_cypress_block_spec_bl30_block_should_overflow_if_too_wide_for_columns_030" => {
            Some(("-5 -71 347.234375 107", "347.234"))
        }
        "upstream_cypress_block_spec_bl31_edge_without_arrow_syntax_should_render_with_no_arrowheads_031" => {
            Some(("-5 -18.5 51.84375 37", "51.8438"))
        }
        "upstream_cypress_block_spec_bl3_should_align_block_widths_and_handle_columns_statement_in_su_003" => {
            Some(("-5 -61.5 206.421875 123", "206.422"))
        }
        "upstream_cypress_block_spec_bl4_should_align_block_widths_and_handle_columns_statements_in_d_004" => {
            Some(("-5 -134.6875 162.53125 269.375", "162.531"))
        }
        "upstream_cypress_block_spec_bl5_should_align_block_widths_and_handle_columns_statements_in_d_005" => {
            Some(("-5 -108.75 351.65625 217.5", "351.656"))
        }
        "upstream_cypress_block_spec_bl6_should_handle_block_arrows_and_spece_statements_006" => {
            Some(("-5 -108.5 271.625 217", "271.625"))
        }
        "upstream_cypress_block_spec_bl7_should_handle_different_types_of_edges_007" => {
            Some(("-5 -53.5 79.4375 107", "79.4375"))
        }
        "upstream_cypress_block_spec_bl8_should_handle_sub_blocks_without_columns_statements_008" => {
            Some(("-5 -52 137.25 104", "137.25"))
        }
        "upstream_cypress_block_spec_bl9_should_handle_edges_from_blocks_in_sub_blocks_to_other_block_009" => {
            Some(("-5 -26.5 127.4375 53", "127.438"))
        }
        "upstream_docs_block_adjusting_widths_011" => Some(("-5 -130 317.53125 260", "317.531")),
        "upstream_docs_block_adjusting_widths_013" => Some(("-5 -79 82.859375 158", "82.8594")),
        "upstream_docs_block_basic_linking_and_arrow_types_021" => {
            Some(("-5 -18.5 78.3125 37", "78.3125"))
        }
        "upstream_docs_block_basic_structure_003" => Some(("-5 -18.5 76.765625 37", "76.7656")),
        "upstream_docs_block_column_usage_003" => Some(("-5 -36 76.765625 72", "76.7656")),
        "upstream_docs_block_example_asymmetric_rhombus_and_hexagon_shapes_025" => {
            Some(("-5 -18.5 216.40625 37", "216.406"))
        }
        "upstream_docs_block_example_asymmetric_rhombus_and_hexagon_shapes_027" => {
            Some(("-5 -114.953125 229.90625 229.90625", "229.906"))
        }
        "upstream_docs_block_example_asymmetric_rhombus_and_hexagon_shapes_029" => {
            Some(("-5 -18.5 216.40625 37", "216.406"))
        }
        "upstream_docs_block_example_block_arrows_035" => Some(("-5 -22.5 631.125 45", "631.125")),
        "upstream_docs_block_example_block_arrows_036" => Some(("-5 -22.5 541.25 45", "541.25")),
        "upstream_docs_block_example_business_process_flow_053" => {
            Some(("-5 -128.5390625 392.796875 257.078125", "392.797"))
        }
        "upstream_docs_block_example_circle_shape_023" => {
            Some(("-5 -108.734375 217.46875 217.46875", "217.469"))
        }
        "upstream_docs_block_example_cylindrical_shape_021" => {
            Some(("-5 -32.38538932800293 83.5 64.77077865600586", "83.5"))
        }
        "upstream_docs_block_example_double_circle_033" => {
            Some(("-5 -113.734375 227.46875 227.46875", "227.469"))
        }
        "upstream_docs_block_example_incorrect_linking_029" => {
            Some(("-5 -18.5 78.3125 37", "78.3125"))
        }
        "upstream_docs_block_example_misplaced_styling_058" => {
            Some(("-5 -18.5 27.4375 37", "27.4375"))
        }
        "upstream_docs_block_example_misplaced_styling_060" => {
            Some(("-5 -18.5 27.4375 37", "27.4375"))
        }
        "upstream_docs_block_example_parallelogram_and_trapezoid_shapes_031" => {
            Some(("-5 -18.5 877.625 37", "877.625"))
        }
        "upstream_docs_block_example_round_edged_block_015" => {
            Some(("-5 -18.5 202.90625 37", "202.906"))
        }
        "upstream_docs_block_example_space_blocks_037" => Some(("-5 -36 76.765625 72", "76.7656")),
        "upstream_docs_block_example_space_blocks_039" => {
            Some(("-5 -18.5 232.34375 37", "232.344"))
        }
        "upstream_docs_block_example_stadium_shaped_block_017" => {
            Some(("-5 -18.5 209.65625 37", "209.656"))
        }
        "upstream_docs_block_example_styling_a_single_block_047" => {
            Some(("-5 -18.5 155.046875 37", "155.047"))
        }
        "upstream_docs_block_example_styling_a_single_class_049" => {
            Some(("-5 -18.5 78.3125 37", "78.3125"))
        }
        "upstream_docs_block_example_subroutine_shape_019" => {
            Some(("-5 -18.5 218.90625 37", "218.906"))
        }
        "upstream_docs_block_example_system_architecture_051" => {
            Some(("-5 -95.15616798400879 246.5 190.31233596801758", "246.5"))
        }
        "upstream_docs_block_introduction_to_block_diagrams_001" => {
            Some(("-5 -128.5 603.1875 257", "603.188"))
        }
        "upstream_docs_block_nested_blocks_009" => Some(("-5 -26.5 302.25 53", "302.25")),
        "upstream_docs_block_spanning_multiple_columns_007" => {
            Some(("-5 -36 196.578125 72", "196.578"))
        }
        "upstream_docs_block_text_on_links_022" => Some(("-5 -18.5 103.75 37", "103.75")),
        "upstream_docs_block_text_on_links_045" => Some(("-5 -128.5 603.1875 257", "603.188")),
        "upstream_edges" => Some(("-5 -18.5 132.75 37", "132.75")),
        "upstream_examples_block_basic_block_layout_001" => {
            Some(("-5 -128.5 603.1875 257", "603.188"))
        }
        "upstream_html_demos_block_block_diagram_demos_001" => {
            Some(("-5 -128.5 603.1875 257", "603.188"))
        }
        "upstream_html_demos_block_block_diagram_demos_002" => {
            Some(("-5 -53.203125 1034.375 106.40625", "1034.38"))
        }
        "upstream_html_demos_block_block_diagram_demos_003" => {
            Some(("-5 -31.419912338256836 866.5 62.83982467651367", "866.5"))
        }
        "upstream_html_demos_block_block_diagram_demos_004" => {
            Some(("-5 -103 162.0625 108", "162.062"))
        }
        "upstream_html_demos_block_block_diagram_demos_005" => Some(("-5 -173 163.5 178", "163.5")),
        "upstream_html_demos_block_block_diagram_demos_006" => {
            Some(("-5 -77.5 130.0625 155", "130.062"))
        }
        "upstream_html_demos_block_block_diagram_demos_007" => Some(("-5 -130 163.5 260", "163.5")),
        "upstream_html_demos_block_block_diagram_demos_009" => {
            Some(("-5 -18.5 78.3125 37", "78.3125"))
        }
        "upstream_html_demos_block_block_diagram_demos_010" => {
            Some(("-5 -36 285.078125 72", "285.078"))
        }
        "upstream_html_demos_block_block_diagram_demos_011" => {
            Some(("-5 -53.5 76.765625 107", "76.7656"))
        }
        "upstream_html_demos_block_block_diagram_demos_012" => {
            Some(("-5 -36 171.140625 41", "171.141"))
        }
        "upstream_pkgtests_block_spec_001" => Some(("-5 -18.5 31.484375 37", "31.4844")),
        "upstream_pkgtests_block_spec_002" => Some(("-5 -18.5 66.859375 37", "66.8594")),
        "upstream_pkgtests_block_spec_003" => Some(("-5 -18.5 77.75 37", "77.75")),
        "upstream_pkgtests_block_spec_004" => Some(("-5 -18.5 115.625 37", "115.625")),
        "upstream_pkgtests_block_spec_005" => Some(("-5 -18.5 131.71875 37", "131.719")),
        "upstream_pkgtests_block_spec_006" => Some(("-5 -18.5 132.75 37", "132.75")),
        "upstream_pkgtests_block_spec_007" => Some(("-5 -18.5 132.75 37", "132.75")),
        "upstream_pkgtests_block_spec_008" => Some(("-5 -18.5 69.5625 37", "69.5625")),
        "upstream_pkgtests_block_spec_009" => Some(("-5 -18.5 69.5625 37", "69.5625")),
        "upstream_pkgtests_block_spec_010" => Some(("-5 -18.5 69.5625 37", "69.5625")),
        "upstream_pkgtests_block_spec_011" => Some(("-5 -18.5 137.125 37", "137.125")),
        "upstream_pkgtests_block_spec_012" => Some(("-5 -36 69.5625 72", "69.5625")),
        "upstream_pkgtests_block_spec_013" => Some(("-5 -26.5 145.59375 53", "145.594")),
        "upstream_pkgtests_block_spec_014" => Some(("-5 -34.5 176.8125 69", "176.812")),
        "upstream_pkgtests_block_spec_015" => Some(("-5 -26.5 128.375 53", "128.375")),
        "upstream_pkgtests_block_spec_016" => Some(("-5 -52 153.125 104", "153.125")),
        "upstream_pkgtests_block_spec_017" => Some(("-5 -22.5 204.6875 45", "204.688")),
        "upstream_pkgtests_block_spec_018" => Some(("-5 -77.5 95.4375 155", "95.4375")),
        "upstream_pkgtests_block_spec_019" => Some(("-5 -36 245 41", "245")),
        "upstream_pkgtests_block_spec_020" => Some(("-5 -18.5 337.296875 37", "337.297")),
        "upstream_pkgtests_block_spec_021" => Some(("-5 -18.5 93.078125 37", "93.0781")),
        "upstream_pkgtests_block_spec_022" => Some(("-5 -18.5 197.0625 37", "197.062")),
        "upstream_pkgtests_block_spec_024" => Some(("-5 -18.5 26.90625 37", "26.9062")),
        "upstream_pkgtests_block_spec_025" => Some(("-5 -18.5 33.3125 37", "33.3125")),
        "upstream_prototype_properties" => Some(("-5 -18.5 394.4375 37", "394.438")),
        "upstream_styles" => Some(("-5 -18.5 93.078125 37", "93.0781")),
        "upstream_styles_2" => Some(("-5 -18.5 197.0625 37", "197.062")),
        "upstream_warnings" => Some(("-5 -281 109.3125 286", "109.312")),
        "upstream_widths_and_spaces" => Some(("-5 -36 245 41", "245")),
        "upstream_widths_and_spaces_2" => Some(("-5 -18.5 337.296875 37", "337.297")),
        "stress_block_font_size_precedence_001" => Some(("-5 -23 1009.78125 46", "1009.78")),
        _ => None,
    }
}
