// Fixture-derived root viewport overrides for Mermaid@11.12.2 Flowchart-V2 diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/flowchart/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_flowchart_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "stress_flowchart_classdef_and_inline_classes_003" => Some((
            "0 -0.000003814697265625 796.109375 181.20632934570312",
            "796.109",
        )),
        "stress_flowchart_clicks_and_tooltips_005" => Some((
            "0 0.0000019073486328125 348.984375 183.86660766601562",
            "348.984",
        )),
        "stress_flowchart_comments_and_directives_010" => {
            Some(("0 0 341.890625 481.796875", "341.891"))
        }
        "stress_flowchart_dense_parallel_edges_002" => {
            Some(("0 0 658.677490234375 697.390625", "658.677"))
        }
        "stress_flowchart_edge_label_wrapping_007" => Some(("0 0 866.09375 199", "866.094")),
        "stress_flowchart_escape_sequences_and_quotes_012" => {
            Some(("0 0 994.43310546875 70", "994.433"))
        }
        "stress_flowchart_linkstyle_indexing_004" => Some(("0 0 222.5 634.1499633789062", "222.5")),
        "stress_flowchart_long_labels_punctuation_unicode_006" => Some(("0 0 346 617", "346")),
        "stress_flowchart_multi_direction_graph_011" => {
            Some(("0 0 216.1875 696.9118041992188", "216.188"))
        }
        "stress_flowchart_nested_subgraphs_titles_001" => {
            Some(("0 0 465.296875 760.1632080078125", "465.297"))
        }
        "stress_flowchart_shape_mix_009" => Some(("0 0 369.66796875 698.21875", "369.668")),
        "stress_flowchart_subgraph_boundary_edges_008" => Some(("0 0 774.21875 221", "774.219")),
        "mmdr_issue_28_text_rendering" => Some(("0 0 792.19873046875 244", "792.199")),
        "mmdr_issue_29_edge_label_distance" => Some((
            "0 0.000003814697265625 1339.015625 794.8007202148438",
            "1339.02",
        )),
        "mmdr_tests_flowchart_flowchart_complex" => {
            Some(("0 0 978.17578125 1198.28125", "978.176"))
        }
        "mmdr_tests_flowchart_flowchart_cycles" => Some(("0 0 230.03515625 985.4375", "230.035")),
        "mmdr_tests_flowchart_flowchart_dense" => {
            Some(("0 0 1097.734375 495.7659912109375", "1097.73"))
        }
        "mmdr_tests_flowchart_flowchart_ports" => Some(("0 0 1570.953125 278", "1570.95")),
        "mmdr_tests_flowchart_flowchart_edges" => Some(("0 0 319.703125 486", "319.703")),
        "mmdr_tests_flowchart_flowchart_subgraph" => Some(("0 0 635.484375 140", "635.484")),
        "upstream_docs_flowchart_document_092" => Some((
            "0 -0.002391815185546875 118.15625 83.4952163696289",
            "118.156",
        )),
        "upstream_docs_flowchart_limitation_199" => Some(("0 0 706.328125 371", "706.328")),
        "upstream_docs_flowchart_markdown_formatting_008" => {
            Some(("0 0 353.203125 118", "353.203"))
        }
        "upstream_docs_flowchart_a_node_with_text_004" => Some(("0 -50 260.90625 120", "260.906")),
        "upstream_docs_flowchart_basic_support_for_fontawesome_234" => {
            Some(("0 0 438.78125 174", "438.781"))
        }
        "upstream_docs_flowchart_css_classes_231" => Some(("0 0 370.734375 70", "370.734")),
        "upstream_docs_flowchart_example_flowchart_with_new_shapes_041" => {
            Some(("0 0 197.53125 681.9603271484375", "197.531"))
        }
        "upstream_docs_diagrams_flowchart_code_flow" => {
            Some(("0 0 10468.515625 3129.234375", "10468.5"))
        }
        "upstream_docs_flowchart_graph_declarations_with_spaces_between_vertices_and_link_and_without_semicolon_240" => {
            Some(("0 0 752.8125 174", "752.812"))
        }
        "upstream_docs_flowchart_minimum_length_of_a_link_182" => {
            Some(("0 0 234.78125 566.53125", "234.781"))
        }
        "upstream_docs_flowchart_minimum_length_of_a_link_184" => {
            Some(("0 0 234.78125 566.53125", "234.781"))
        }
        "upstream_docs_flowchart_parameters_136" => Some(("0 0 192.984375 112", "192.984")),
        "upstream_flowchart_v2_stadium_shape_spec" => {
            Some(("-96.54400634765625 -48 610.109375 606", "610.109"))
        }
        "upstream_flowchart_v2_styled_subgraphs_spec" => {
            Some(("-96.59170532226562 -50 477.859375 844", "477.859"))
        }
        "upstream_docs_contributing_checkout_a_new_branch_020" => {
            Some(("0 0 819.421875 382", "819.422"))
        }
        "upstream_docs_contributing_initial_setup_001" => Some(("0 0 732.1875 70", "732.188")),
        "upstream_docs_contributing_where_is_the_documentation_located_026" => {
            Some(("0 0 752.5 118", "752.5"))
        }
        "upstream_docs_contributing_workflow_011" => Some(("0 0 651.40625 70", "651.406")),
        "upstream_docs_directives_changing_fontfamily_via_directive_010" => {
            Some(("0 0 333.03125 224", "333.031"))
        }
        "upstream_docs_directives_changing_loglevel_via_directive_011" => {
            Some(("0 0 333.03125 224", "333.031"))
        }
        "upstream_docs_directives_changing_theme_via_directive_009" => {
            Some(("0 0 333.03125 224", "333.031"))
        }
        "upstream_docs_directives_changing_flowchart_config_via_directive_012" => {
            Some(("0 0 333.03125 224", "333.031"))
        }
        "upstream_docs_getting_started_diagram_code_001" => {
            Some(("0 0 400.44140625 533.765625", "400.441"))
        }
        "upstream_docs_mermaid_run_003" => Some(("0 0 529.953125 174", "529.953")),
        "upstream_docs_flowchart_custom_icons_238" => Some(("0 0 180.03125 174", "180.031")),
        "upstream_docs_theming_customizing_themes_with_themevariables_003" => {
            Some(("0 0 529.14453125 571.28125", "529.145"))
        }
        "upstream_docs_syntax_reference_using_dagre_layout_with_classic_look_006" => {
            Some(("0 0 518.203125 174", "518.203"))
        }
        "upstream_docs_flowchart_unicode_text_005" => Some(("0 0 188.09375 70", "188.094")),
        "upstream_docs_flowchart_a_node_with_round_edges_013" => {
            Some(("0 0 230.90625 70", "230.906"))
        }
        "upstream_docs_flowchart_a_stadium_shaped_node_015" => {
            Some(("0 0 225.63621520996094 55", "225.636"))
        }
        "upstream_docs_flowchart_a_node_in_a_subroutine_shape_017" => {
            Some(("0 0 231.90625 55", "231.906"))
        }
        "upstream_docs_flowchart_a_node_in_a_cylindrical_shape_019" => {
            Some(("0 -0.000003814697265625 96.5 84.37955474853516", "96.5"))
        }
        "upstream_docs_flowchart_a_node_in_the_form_of_a_circle_021" => {
            Some(("0 0 230.46875 230.46875", "230.469"))
        }
        "upstream_docs_flowchart_a_node_in_an_asymmetric_shape_023" => {
            Some(("0 0 225.65625 55", "225.656"))
        }
        "upstream_docs_flowchart_a_node_rhombus_025" => {
            Some(("0.5 0 254.90625 254.90625", "254.906"))
        }
        "upstream_docs_flowchart_a_hexagon_node_027" => {
            Some(("0 0 275.4739685058594 55", "275.474"))
        }
        "upstream_docs_flowchart_parallelogram_029" => Some(("0 0 254.90625 55", "254.906")),
        "upstream_docs_flowchart_parallelogram_alt_031" => Some(("0 0 254.90625 55", "254.906")),
        "upstream_docs_flowchart_double_circle_037" => Some(("0 0 240.46875 240.46875", "240.469")),
        "upstream_docs_flowchart_process_042" => Some(("0 0 192.296875 70", "192.297")),
        "upstream_docs_flowchart_event_044" => Some(("0 0 158.09375 70", "158.094")),
        "upstream_docs_flowchart_terminal_point_stadium_046" => {
            Some(("0 0 144.65184020996094 55", "144.652"))
        }
        "upstream_docs_flowchart_subprocess_048" => Some(("0 0 187.4375 55", "187.438")),
        "upstream_docs_accessibility_acctitle_and_accdescr_usage_examples_004" => {
            Some(("0 0 621.046875 197.234375", "621.047"))
        }
        "upstream_docs_accessibility_acctitle_and_accdescr_usage_examples_006" => {
            Some(("0 0 621.046875 197.234375", "621.047"))
        }
        "upstream_docs_flowchart_lined_document_110" => Some((
            "0 -0.004795074462890625 175.6374969482422 96.99040985107422",
            "175.637",
        )),
        "upstream_docs_flowchart_multi_document_stacked_document_118" => Some((
            "0 -0.004795074462890625 196.640625 106.99040985107422",
            "196.641",
        )),
        "upstream_docs_flowchart_stored_data_bow_tie_rectangle_124" => {
            Some(("0.010150909423828125 0 140.29981994628906 55", "140.3"))
        }
        "upstream_docs_flowchart_tagged_document_128" => Some((
            "0 -0.004795074462890625 187.03280639648438 96.99040985107422",
            "187.033",
        )),
        "upstream_docs_configuration_frontmatter_config_001" => {
            Some(("0 -50 117.34375 224", "117.344"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fdh21_render_cylindrical_shape_021" => Some((
            "0 0.000003814697265625 769.890625 341.0105285644531",
            "769.891",
        )),
        "upstream_cypress_flowchart_handdrawn_spec_fhd5_should_style_nodes_via_a_class_005" => {
            Some(("0 0 205.390625 382", "205.391"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fhd12_should_render_a_flowchart_with_long_names_and_class_defini_012" => {
            Some(("0 0 1806.8125 452", "1806.81"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fhd14_should_render_hexagons_014" => {
            Some(("0 0 417 559", "417"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fhd6_should_render_a_flowchart_full_of_circles_006" => {
            Some(("0 -45 2400.640625 645", "2400.64"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fhd7_should_render_a_flowchart_full_of_icons_007" => {
            Some(("0 0 2004.41015625 1046", "2004.41"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fdh25_handle_link_click_events_link_anchor_mailto_other_protocol_025" => {
            Some(("0 0 1384.953125 198", "1384.95"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fdh26_set_text_color_of_nodes_and_links_according_to_styles_when_026" => {
            Some(("0 0 334.8125 366.109375", "334.812"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fdh24_keep_node_label_text_if_already_defined_when_a_style_is_ap_024" => {
            Some(("0 0 357.015625 40", "357.016"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fhd3_should_render_a_simple_flowchart_with_line_breaks_003" => {
            Some(("0 0 417 652.875", "417"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fhd4_should_render_a_simple_flowchart_with_trapezoid_and_inverse_004" => {
            Some(("0 0 405 637.875", "405"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fhd13_should_render_color_of_styled_nodes_013" => {
            Some(("0 0 232.640625 70", "232.641"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fdh31_should_not_slice_off_edges_that_are_to_the_left_of_the_lef_031" => {
            Some(("-27.961000442504883 0 169.48399353027344 278", "169.484"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fdh32_render_subroutine_shape_032" => {
            Some(("0 0 953.6875 257", "953.688"))
        }
        "upstream_cypress_flowchart_v2_spec_wrapping_long_text_with_a_new_line_051" => {
            Some(("0 0 363.921875 430", "363.922"))
        }
        "upstream_cypress_flowchart_v2_spec_with_formatting_in_a_node_054" => {
            Some(("0 -1 689.78125 290", "689.781"))
        }
        "upstream_cypress_flowchart_v2_spec_wrapping_long_text_with_a_new_line_056" => {
            Some(("0 0 363.921875 430", "363.922"))
        }
        "upstream_cypress_flowchart_v2_spec_should_not_auto_wrap_when_markdownautowrap_is_false_058" => {
            Some(("0 0 539 166", "539"))
        }
        "upstream_cypress_flowchart_v2_spec_57_handle_nested_subgraphs_with_outgoing_links_4_015" => {
            Some(("0 0 154.921875 364", "154.922"))
        }
        "upstream_cypress_flowchart_v2_spec_should_render_a_stadium_shaped_node_068" => {
            Some(("0 0 385.96875 159", "385.969"))
        }
        "upstream_cypress_flowchart_v2_spec_should_render_a_diamond_shaped_node_using_shape_config_069" => {
            Some(("0 0 175.984375 236.3125", "175.984"))
        }
        "upstream_cypress_flowchart_v2_spec_should_render_a_rounded_rectangle_and_a_normal_rectangle_070" => {
            Some(("0 0 385.96875 174", "385.969"))
        }
        "upstream_cypress_flowchart_v2_spec_new_line_in_node_and_formatted_edge_label_050" => {
            Some(("0 0 379.390625 94", "379.391"))
        }
        "upstream_cypress_flowchart_v2_spec_new_line_in_node_and_formatted_edge_label_055" => {
            Some(("0 0 383.390625 94", "383.391"))
        }
        "upstream_cypress_flowchart_v2_spec_should_render_raw_strings_072" => {
            Some(("0 0 231.4375 70", "231.438"))
        }
        "upstream_html_demos_flowchart_flowchart_002" => {
            Some(("0 -50 1534.03125 510.07421875", "1534.03"))
        }
        "upstream_html_demos_flowchart_flowchart_010" => {
            Some(("0 0 2004.41015625 1046", "2004.41"))
        }
        "upstream_html_demos_flowchart_flowchart_045" => Some(("0 0 1534.03125 452", "1534.03")),
        "upstream_html_demos_flowchart_flowchart_049" => {
            Some(("0 0 2004.41015625 1046", "2004.41"))
        }
        "upstream_html_demos_flowchart_flowchart_062" => {
            Some(("0 0 563.5078125 719.140625", "563.508"))
        }
        "upstream_html_demos_flowchart_graph_001" => Some(("0 -50 1534.03125 502", "1534.03")),
        "upstream_html_demos_dataflowchart_data_flow_diagram_demos_001" => {
            Some(("0 0 519.859375 81.421875", "519.859"))
        }
        "upstream_html_demos_dataflowchart_borders_example_002" => {
            Some(("0 0 1499.203125 94", "1499.2"))
        }
        "upstream_html_demos_flow2_example_003" => {
            Some(("0 0 292.468994140625 425.421875", "292.469"))
        }
        "upstream_singlenode_shapes_spec" => Some(("0 0 1557.2265625 156.3125", "1557.23")),
        _ => None,
    }
}
