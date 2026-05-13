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
        "stress_flowchart_comments_and_directives_010" => {
            Some(("0 0 341.890625 481.796875", "341.891"))
        }
        "stress_flowchart_dense_parallel_edges_002" => {
            Some(("0 0 658.677490234375 697.390625", "658.677"))
        }
        "stress_flowchart_escape_sequences_and_quotes_012" => {
            Some(("0 0 994.43310546875 70", "994.433"))
        }
        "stress_flowchart_shape_mix_009" => Some(("0 0 369.66796875 698.21875", "369.668")),
        "stress_flowchart_subgraph_title_margin_extremes_015" => {
            Some(("0 25 806.421875 796", "806.422"))
        }
        "stress_flowchart_text_style_overrides_076" => Some(("0 0 521.75 88", "521.75")),
        "stress_flowchart_subgraph_title_margins_extreme_nested_030" => {
            Some(("0 -50 487.671875 283", "487.672"))
        }
        "stress_flowchart_unicode_punct_in_ids_labels_035" => {
            Some(("0 0 824.703125 70", "824.703"))
        }
        "stress_flowchart_subgraph_title_long_with_punct_038" => {
            Some(("0 0 567.546875 140", "567.547"))
        }
        "stress_flowchart_subgraph_deep_nesting_title_padding_044" => {
            Some(("0 0 628.5234375 703", "628.523"))
        }
        "stress_flowchart_icons_basic_051" => Some(("0 0 438.78125 278", "438.781")),
        "stress_flowchart_icons_in_edge_labels_053" => Some(("0 0 130.6875 326", "130.688")),
        "stress_flowchart_icons_multiline_br_054" => Some(("0 0 145.5 374", "145.5")),
        "stress_flowchart_icons_unicode_and_wrap_056" => Some(("0 0 701.515625 94", "701.516")),
        "stress_flowchart_icons_click_security_strict_057" => {
            Some(("0 0 139.890625 174", "139.891"))
        }
        "stress_flowchart_icons_classdef_and_style_058" => Some(("0 0 351.578125 174", "351.578")),
        "stress_flowchart_icons_subgraph_mixed_061" => Some(("0 0 353.75 274", "353.75")),
        "stress_flowchart_icons_edge_to_cluster_062" => Some(("0 0 422.078125 244", "422.078")),
        "upstream_docs_flowchart_basic_support_for_fontawesome_234" => {
            Some(("0 0 438.78125 174", "438.781"))
        }
        "upstream_docs_diagrams_flowchart_code_flow" => {
            Some(("0 0 10468.515625 3129.234375", "10468.5"))
        }
        "upstream_docs_flowchart_special_characters_that_break_syntax_185" => {
            Some(("0 0 272.65625 70", "272.656"))
        }
        "upstream_flowchart_v2_stadium_shape_spec" => {
            Some(("-96.54400634765625 -50 610.109375 608", "610.109"))
        }
        "upstream_docs_mermaid_run_003" => Some(("0 0 529.953125 174", "529.953")),
        "upstream_docs_flowchart_unicode_text_005" => Some(("0 0 187.109375 70", "187.109")),
        "upstream_cypress_flowchart_handdrawn_spec_fdh21_render_cylindrical_shape_021" => Some((
            "0 0.000003814697265625 769.890625 341.0105285644531",
            "769.891",
        )),
        "upstream_cypress_flowchart_handdrawn_spec_fhd5_should_style_nodes_via_a_class_005" => {
            Some(("0 0 205.390625 382", "205.391"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fhd12_should_render_a_flowchart_with_long_names_and_class_defini_012" => {
            Some(("0 0 1926.8125 452", "1926.81"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fhd6_should_render_a_flowchart_full_of_circles_006" => {
            Some(("0 -45 2400.640625 645", "2400.64"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fhd7_should_render_a_flowchart_full_of_icons_007" => {
            Some(("0 0 2004.41015625 1046", "2004.41"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fdh32_render_subroutine_shape_032" => {
            Some(("0 0 953.6875 257", "953.688"))
        }
        "upstream_cypress_flowchart_v2_spec_wrapping_long_text_with_a_new_line_051" => {
            Some(("0 0 363.921875 430", "363.922"))
        }
        "upstream_cypress_flowchart_v2_spec_wrapping_long_text_with_a_new_line_056" => {
            Some(("0 0 363.921875 430", "363.922"))
        }
        "upstream_cypress_flowchart_v2_spec_57_handle_nested_subgraphs_with_outgoing_links_4_015" => {
            Some(("0 0 154.921875 364", "154.922"))
        }
        "upstream_html_demos_flowchart_flowchart_010" => {
            Some(("0 0 2004.41015625 1046", "2004.41"))
        }
        "upstream_html_demos_flowchart_flowchart_049" => {
            Some(("0 0 2004.41015625 1046", "2004.41"))
        }
        "upstream_cypress_flowchart_handdrawn_spec_fdh49_should_add_edge_animation_049" => {
            Some(("0 0 309.125 322.390625", "309.125"))
        }
        "upstream_cypress_flowchart_icon_spec_example_002" => Some(("0 0 92.046875 70", "92.0469")),
        "upstream_cypress_flowchart_icon_spec_should_render_aws_icons_with_labels_and_rect_elements_005" => {
            Some(("0 0 104.6875 368", "104.688"))
        }
        "upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset34_034" => {
            Some(("0 0 609.140625 80", "609.141"))
        }
        "upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset37_037" => {
            Some(("0.0284881591796875 0 296.1708984375 76", "296.171"))
        }
        "upstream_cypress_flowchart_spec_17_render_multiline_texts_017" => {
            Some(("0 0 304.046875 734", "304.047"))
        }
        "upstream_cypress_flowchart_spec_20_multiple_nodes_and_chaining_in_one_statement_020" => {
            Some(("0 0 234.015625 300", "234.016"))
        }
        "upstream_cypress_flowchart_spec_23_render_a_simple_flowchart_with_rankspacing_set_to_100_023" => {
            Some(("0 0 465.625 695.21875", "465.625"))
        }
        "upstream_cypress_flowchart_spec_24_keep_node_label_text_if_already_defined_when_a_style_is_appli_024" => {
            Some(("0 0 410.828125 38", "410.828"))
        }
        "upstream_cypress_flowchart_spec_25_handle_link_click_events_link_anchor_mailto_other_protocol_sc_025" => {
            Some(("0 0 1525.3125 246", "1525.31"))
        }
        "upstream_cypress_flowchart_spec_27_set_text_color_of_nodes_and_links_according_to_styles_when_ht_027" => {
            Some(("0 0 376.296875 373.40625", "376.297"))
        }
        "upstream_cypress_flowchart_spec_3_should_render_a_simple_flowchart_with_line_breaks_003" => {
            Some(("0 0 440.03125 737.25", "440.031"))
        }
        "upstream_cypress_flowchart_spec_4_should_render_a_simple_flowchart_with_trapezoid_and_inverse_tr_004" => {
            Some(("0 0 428.03125 722.25", "428.031"))
        }
        "upstream_cypress_flowchart_spec_6_should_render_a_flowchart_full_of_circles_006" => {
            Some(("0 -45 2638.375 645", "2638.38"))
        }
        "upstream_cypress_flowchart_spec_7_should_render_a_flowchart_full_of_icons_007" => {
            Some(("0 0 2241.375 1142", "2241.38"))
        }
        "upstream_cypress_flowchart_spec_8_should_render_labels_with_numbers_at_the_start_008" => {
            Some(("0 0 177.625 140", "177.625"))
        }
        "upstream_cypress_flowchart_v2_spec_1433_should_render_a_titled_flowchart_with_titletopmargin_set_to_040" => {
            Some(("-33.6171875 -35 152.671875 209", "152.672"))
        }
        "upstream_cypress_flowchart_v2_spec_3258_should_render_subgraphs_with_main_graph_nodespacing_and_ran_046" => {
            Some(("-66.3203125 -50 406.09375 196", "406.094"))
        }
        "upstream_cypress_flowchart_v2_spec_57_handle_nested_subgraphs_with_outgoing_links_4_016" => {
            Some(("0 0 154.921875 364", "154.922"))
        }
        "upstream_cypress_flowchart_v2_spec_wrapping_long_text_with_a_new_line_052" => {
            Some(("0 0 363.921875 430", "363.922"))
        }
        "upstream_cypress_newshapes_spec_newshapessets_newshapesset3_lr_allpairs_067" => Some((
            "-0.0100250244140625 0 1274.9244384765625 300.7861328125",
            "1274.92",
        )),
        "upstream_cypress_newshapes_spec_newshapessets_newshapesset5_lr_md_html_false_086" => {
            Some((
                "0 -0.009033203125 373.23016357421875 924.65283203125",
                "373.23",
            ))
        }
        "upstream_cypress_newshapes_spec_newshapessets_newshapesset5_lr_md_html_true_085" => {
            Some((
                "0 -0.00905609130859375 396.140625 989.5728149414062",
                "396.141",
            ))
        }
        "upstream_cypress_newshapes_spec_newshapessets_newshapesset5_tb_md_html_true_037" => {
            Some(("0 0 1714.1500244140625 272.97283935546875", "1714.15"))
        }
        "upstream_cypress_newshapes_spec_newshapessets_newshapesset6_lr_md_html_true_093" => {
            Some(("0 0 379.890625 274.8000183105469", "379.891"))
        }
        "upstream_cypress_newshapes_spec_newshapessets_newshapesset6_tb_md_html_true_045" => {
            Some((
                "0.00000762939453125 0 535.1500244140625 224.39999389648438",
                "535.15",
            ))
        }
        "upstream_cypress_oldshapes_spec_shapessets_shapesset5_tb_md_html_false_038" => {
            Some(("0 0 1377.199462890625 199.20001220703125", "1377.2"))
        }
        "upstream_flow_vertice_chaining_amp_to_single_spec" => {
            Some(("-53.9921875 -50 312.484375 224", "312.484"))
        }
        "upstream_flowchart_v2_arrows_double_dotted_text_spec" => {
            Some(("-149.5 -50 384.4375 248", "384.438"))
        }
        "upstream_flowchart_v2_arrows_double_point_spec" => {
            Some(("-103.765625 -50 292.96875 224", "292.969"))
        }
        "upstream_flowchart_v2_arrows_double_point_text_spec" => {
            Some(("-143.4921875 -50 372.421875 248", "372.422"))
        }
        "upstream_flowchart_v2_arrows_double_thick_spec" => {
            Some(("-102.9140625 -50 291.265625 224", "291.266"))
        }
        "upstream_flowchart_v2_arrows_graph_direction_caret_spec" => {
            Some(("-127.8046875 -50 341.046875 224", "341.047"))
        }
        "upstream_flowchart_v2_arrows_graph_direction_gt_spec" => {
            Some(("-68.2734375 -50 341.046875 120", "341.047"))
        }
        "upstream_flowchart_v2_arrows_graph_direction_lt_spec" => {
            Some(("-79.4375 -50 363.375 120", "363.375"))
        }
        "upstream_flowchart_v2_lines_edge_id_curve_without_overriding_default_spec" => {
            Some(("-123.890625 -50 452.421875 224", "452.422"))
        }
        "upstream_flowchart_v2_lines_linkstyle_multi_numbered_interpolate_spec" => {
            Some(("-110.6015625 -50 425.84375 224", "425.844"))
        }
        "upstream_flowchart_v2_lines_linkstyle_numbered_interpolate_spec" => {
            Some(("-86.1171875 -50 376.875 224", "376.875"))
        }
        "upstream_flowchart_v2_lines_linkstyle_numbered_interpolate_with_style_spec" => {
            Some(("-128.5 -50 461.640625 224", "461.641"))
        }
        "upstream_flowchart_v2_lines_stroke_dotted_spec" => {
            Some(("-64.5703125 -50 214.578125 224", "214.578"))
        }
        "upstream_flowchart_v2_lines_stroke_thick_spec" => {
            Some(("-57.71875 -50 200.875 224", "200.875"))
        }
        "upstream_flowchart_v2_subgraph_nodeSpacing_rankSpacing_main_graph_spec" => {
            Some(("-66.3203125 -50 406.09375 196", "406.094"))
        }
        "upstream_flowchart_v2_subgraph_numeric_id_spec" => {
            Some(("-96.1875 -50 408.453125 298", "408.453"))
        }
        "upstream_flowchart_v2_titled_flowchart_titleTopMargin_10_spec" => {
            Some(("-33.6171875 -35 152.671875 209", "152.672"))
        }
        "upstream_html_demos_flowchart_flowchart_004" => Some(("0 0 417 646", "417")),
        "upstream_html_demos_flowchart_flowchart_008" => Some(("0 -45 2400.640625 645", "2400.64")),
        "upstream_html_demos_flowchart_flowchart_016" => Some(("0 0 622.921875 70", "622.922")),
        "upstream_html_demos_flowchart_flowchart_022" => Some(("0 0 953.6875 257", "953.688")),
        "upstream_html_demos_flowchart_flowchart_024" => Some((
            "0 0.000003814697265625 769.890625 341.0105285644531",
            "769.891",
        )),
        "upstream_html_demos_flowchart_flowchart_046" => Some(("0 0 417 646", "417")),
        "upstream_html_demos_flowchart_flowchart_048" => Some(("0 -45 2400.640625 645", "2400.64")),
        "upstream_html_demos_flowchart_flowchart_052" => Some(("0 0 622.921875 70", "622.922")),
        "upstream_html_demos_flowchart_flowchart_055" => Some(("0 0 953.6875 257", "953.688")),
        "upstream_html_demos_flowchart_flowchart_056" => Some((
            "0 0.000003814697265625 769.890625 341.0105285644531",
            "769.891",
        )),
        "upstream_html_demos_flowchart_flowchart_063" => {
            Some(("-98.8515625 -50 406.09375 196", "406.094"))
        }
        "upstream_html_demos_flowchart_graph_003" => Some(("0 -50 417 696", "417")),
        "upstream_html_demos_flowchart_graph_021" => Some(("0 0 953.6875 257", "953.688")),
        "upstream_html_demos_flowchart_graph_023" => Some((
            "0 0.000003814697265625 769.890625 341.0105285644531",
            "769.891",
        )),
        "upstream_pkgtests_flow_singlenode_spec_010" => Some(("0 0 438.859375 70", "438.859")),
        _ => None,
    }
}
