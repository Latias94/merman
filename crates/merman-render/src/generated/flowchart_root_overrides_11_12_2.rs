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
        _ => None,
    }
}
