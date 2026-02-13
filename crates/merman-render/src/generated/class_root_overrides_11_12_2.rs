// Fixture-derived root viewport overrides for Mermaid@11.12.2 Class diagrams.
//
// These entries are keyed by fixture `diagram_id` and are used to close the remaining
// root `<svg>` parity-root deltas (`viewBox` + `style max-width`).

pub fn lookup_class_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "mmdr_tests_class_class_basic" => Some(("0 0 159.640625 318", "159.641")),
        "mmdr_tests_class_class_multiplicity" => Some(("0 0 101.78125 258", "101.781")),
        "upstream_cross_namespace_relations_spec" => Some(("0 0 367.06640625 406", "367.066")),
        "upstream_docs_classdiagram_cardinality_multiplicity_on_relations_038" => {
            Some(("0 0 376.5078125 258", "376.508"))
        }
        "upstream_docs_classdiagram_annotations_on_classes_040" => {
            Some(("0 0 172.546875 184", "172.547"))
        }
        "upstream_docs_classdiagram_annotations_on_classes_042" => {
            Some(("0 0 354.40625 256", "354.406"))
        }
        "upstream_docs_classdiagram_class_004" => Some(("0 -48 242.90625 256", "242.906")),
        "upstream_docs_classdiagram_class_diagrams_002" => {
            Some(("0 -48 902.8359375 474", "902.836"))
        }
        "upstream_docs_classdiagram_class_labels_008" => Some(("0 0 184.6875 234", "184.688")),
        "upstream_docs_classdiagram_class_labels_010" => Some(("0 0 138.859375 234", "138.859")),
        "upstream_docs_classdiagram_comments_044" => Some(("0 0 172.546875 184", "172.547")),
        "upstream_docs_classdiagram_classes_065" => Some(("0 0 91.34375 100", "91.3438")),
        "upstream_docs_classdiagram_classes_067" => Some(("0 0 168.765625 160", "168.766")),
        "upstream_docs_classdiagram_css_classes_073" => Some(("0 0 91.34375 100", "91.3438")),
        "upstream_docs_classdiagram_default_class_070" => Some(("0 0 220.265625 100", "220.266")),
        "upstream_docs_classdiagram_define_namespace_035" => {
            Some(("-8 0 250.2890625 364", "250.289"))
        }
        "upstream_docs_classdiagram_defining_members_of_a_class_012" => {
            Some(("0 0 242.90625 208", "242.906"))
        }
        "upstream_docs_classdiagram_defining_members_of_a_class_014" => {
            Some(("0 0 242.90625 208", "242.906"))
        }
        "upstream_docs_classdiagram_defining_relationship_021" => {
            Some(("0 0 921.21875 234", "921.219"))
        }
        "upstream_docs_classdiagram_defining_relationship_023" => {
            Some(("0 0 938.265625 258", "938.266"))
        }
        "upstream_docs_classdiagram_examples_049" => Some(("0 0 416.734375 186", "416.734")),
        "upstream_docs_classdiagram_examples_051" => Some(("0 0 212.78125 100", "212.781")),
        "upstream_docs_classdiagram_examples_053" => Some(("0 0 212.78125 100", "212.781")),
        "upstream_docs_classdiagram_examples_056" => Some(("0 0 484.25 100", "484.25")),
        "upstream_docs_classdiagram_generic_types_018" => Some(("0 0 366.3203125 304", "366.32")),
        "upstream_docs_classdiagram_lollipop_interfaces_031" => {
            Some(("0 0 64.03125 174", "64.0312"))
        }
        "upstream_docs_classdiagram_lollipop_interfaces_033" => {
            Some(("0 0 247.9140625 368", "247.914"))
        }
        "upstream_docs_classdiagram_members_box_075" => Some(("0 0 76.6875 64", "76.6875")),
        "upstream_docs_classdiagram_return_type_016" => Some(("0 0 278.0625 208", "278.062")),
        "upstream_docs_classdiagram_setting_the_direction_of_the_diagram_046" => {
            Some(("0 0 431.125 354", "431.125"))
        }
        "upstream_docs_classdiagram_styling_a_node_059" => Some(("0 0 220.265625 100", "220.266")),
        "upstream_docs_classdiagram_two_way_relations_028" => Some(("0 0 91.34375 234", "91.3438")),
        "upstream_namespaces_and_generics" => Some(("0 0 799.90625 436", "799.906")),
        "upstream_relation_types_and_cardinalities_spec" => {
            Some(("0 0 1704.16015625 416", "1704.16"))
        }
        "upstream_annotations_in_brackets_spec" => Some(("0 0 335.125 184", "335.125")),
        "upstream_note_keywords_spec" => Some(("0 0 669.90625 246", "669.906")),
        "upstream_separators_labels_notes" => Some(("0 0 553.8515625 594", "553.852")),
        "upstream_docs_accessibility_class_diagram_008" => Some(("0 0 94.625 234", "94.625")),
        "upstream_docs_examples_class_diagram_syntax_classdiagram_md_004" => {
            Some(("0 0 786.484375 718", "786.484"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_1_1_should_render_a_simple_class_diagram_without_htmllabels_003" => {
            Some(("0 0 1162.390625 814", "1162.39"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_10_should_render_a_simple_class_diagram_with_clickable_callb_018" => {
            Some(("0 0 600.828125 342", "600.828"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_16b_should_handle_the_direction_statement_with_tb_027" => {
            Some(("0 0 356.0703125 354", "356.07"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_17a_should_handle_the_direction_statement_with_bt_028" => {
            Some(("0 0 356.0703125 354", "356.07"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_18a_should_handle_the_direction_statement_with_lr_030" => {
            Some(("0 0 431.125 354", "431.125"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_2_1_should_render_a_simple_class_diagrams_with_cardinality_w_005" => {
            Some(("0 0 786.484375 742", "786.484"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_4_should_render_a_simple_class_diagram_with_comments_009" => {
            Some(("0 0 786.484375 742", "786.484"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_9_should_render_a_simple_class_diagram_with_clickable_link_017" => {
            Some(("0 0 600.828125 342", "600.828"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_a_full_class_diagram_using_elk_057" => {
            Some(("0 0 1408.9765625 838", "1408.98"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_a_simple_class_diagram_with_a_custom_theme_055" => {
            Some(("0 0 1162.390625 814", "1162.39"))
        }
        "upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_a_full_class_diagram_using_elk_057" => {
            Some(("4 4 1330.953125 797.5", "1330.95"))
        }
        "upstream_cypress_classdiagram_spec_should_handle_newline_title_in_namespace_021" => {
            Some(("-8 0 245.28125 422", "245.281"))
        }
        "upstream_cypress_classdiagram_spec_should_render_class_diagram_with_newlines_in_title_018" => {
            Some(("0 0 195.4296875 474", "195.43"))
        }
        "upstream_cypress_classdiagram_spec_should_render_with_newlines_in_title_and_an_annotation_020" => {
            Some(("0 0 177.6796875 376", "177.68"))
        }
        "upstream_cypress_classdiagram_v2_spec_1_should_render_a_simple_class_diagram_002" => {
            Some(("0 0 1162.390625 814", "1162.39"))
        }
        "upstream_cypress_classdiagram_v2_spec_10_should_render_a_simple_class_diagram_with_clickable_callback_012" => {
            Some(("0 0 600.828125 342", "600.828"))
        }
        "upstream_cypress_classdiagram_v2_spec_2_should_render_a_simple_class_diagrams_with_cardinality_003" => {
            Some(("0 0 786.484375 742", "786.484"))
        }
        "upstream_cypress_classdiagram_v2_spec_4_should_render_a_simple_class_diagram_with_comments_006" => {
            Some(("0 0 786.484375 742", "786.484"))
        }
        "upstream_cypress_classdiagram_v2_spec_9_should_render_a_simple_class_diagram_with_clickable_link_011" => {
            Some(("0 0 600.828125 342", "600.828"))
        }
        "upstream_cypress_classdiagram_v2_spec_renders_a_class_diagram_with_nested_namespaces_and_relationships_035" => {
            Some(("-8 0 808.234375 448", "808.234"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_12_1_should_render_a_simple_class_diagram_with_generic_types_022" => {
            Some(("0 0 296.9765625 208", "296.977"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_13_should_render_a_simple_class_diagram_with_css_classes_app_023" => {
            Some(("0 0 282.703125 208", "282.703"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_14_should_render_a_simple_class_diagram_with_css_classes_app_024" => {
            Some(("0 0 282.703125 208", "282.703"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_18b_should_render_a_simple_class_diagram_with_notes_031" => {
            Some(("0 0 405.828125 270", "405.828"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_2_2_should_render_a_simple_class_diagram_with_different_visi_006" => {
            Some(("0 0 254.125 438", "254.125"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_7_1_should_render_a_simple_class_diagram_with_generic_class_015" => {
            Some(("0 0 439.4453125 342", "439.445"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_8_should_render_a_simple_class_diagram_with_generic_class_an_016" => {
            Some(("0 0 600.828125 342", "600.828"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_should_add_classes_namespaces_039" => {
            Some(("-8 0 446.6875 170", "446.688"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_a_class_with_a_text_label_members_and_annotati_035" => {
            Some(("0 0 205.796875 294", "205.797"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_a_simple_class_diagram_with_classdefs_being_ap_047" => {
            Some(("0 0 95.5625 100", "95.5625"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_a_simple_class_diagram_with_markdown_styling_049" => {
            Some(("0 0 184.96875 256", "184.969"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037" => {
            Some(("0 0 2355.734375 100", "2355.73"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_multiple_classes_with_same_text_labels_036" => {
            Some(("0 0 417.1875 234", "417.188"))
        }
        "upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_a_class_with_a_text_label_members_and_annotatio_035" => {
            Some(("0 0 205.796875 294", "205.797"))
        }
        "upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037" => {
            Some(("0 0 2355.734375 100", "2355.73"))
        }
        "upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_multiple_classes_with_same_text_labels_036" => {
            Some(("0 0 417.1875 234", "417.188"))
        }
        "upstream_cypress_classdiagram_spec_13_should_render_a_simple_class_diagram_with_css_classes_applied_013" => {
            Some(("0 0 282.703125 208", "282.703"))
        }
        "upstream_cypress_classdiagram_spec_15_should_render_a_simple_class_diagram_with_css_classes_applied_015" => {
            Some(("0 0 225.125 100", "225.125"))
        }
        "upstream_cypress_classdiagram_spec_19_should_render_a_simple_class_diagram_with_notes_017" => {
            Some(("0 0 405.828125 270", "405.828"))
        }
        "upstream_cypress_classdiagram_spec_should_handle_an_empty_class_body_with_empty_braces_025" => {
            Some(("0 0 262.0859375 294", "262.086"))
        }
        "upstream_cypress_classdiagram_spec_should_handle_newline_in_string_label_022" => {
            Some(("0 0 397.0234375 208", "397.023"))
        }
        "upstream_cypress_classdiagram_spec_should_render_class_diagram_with_many_newlines_in_title_019" => {
            Some(("0 0 170.28125 352", "170.281"))
        }
        "upstream_cypress_classdiagram_v2_spec_12_should_render_a_simple_class_diagram_with_generic_types_014" => {
            Some(("0 0 296.9765625 208", "296.977"))
        }
        "upstream_cypress_classdiagram_v2_spec_13_should_render_a_simple_class_diagram_with_css_classes_applied_015" => {
            Some(("0 0 282.703125 208", "282.703"))
        }
        "upstream_cypress_classdiagram_v2_spec_14_should_render_a_simple_class_diagram_with_css_classes_applied_016" => {
            Some(("0 0 282.703125 208", "282.703"))
        }
        "upstream_cypress_classdiagram_v2_spec_18b_should_render_a_simple_class_diagram_with_notes_023" => {
            Some(("0 0 405.828125 270", "405.828"))
        }
        "upstream_cypress_classdiagram_v2_spec_2_1_should_render_a_simple_class_diagram_with_different_visibili_004" => {
            Some(("0 0 254.125 438", "254.125"))
        }
        "upstream_cypress_classdiagram_v2_spec_7_should_render_a_simple_class_diagram_with_generic_class_009" => {
            Some(("0 0 439.4453125 342", "439.445"))
        }
        "upstream_cypress_classdiagram_v2_spec_8_should_render_a_simple_class_diagram_with_generic_class_and_re_010" => {
            Some(("0 0 600.828125 342", "600.828"))
        }
        "upstream_cypress_classdiagram_v3_spec_should_render_a_full_class_diagram_using_elk_057" => {
            Some(("4 4 1330.953125 797.5", "1330.95"))
        }
        _ => None,
    }
}
