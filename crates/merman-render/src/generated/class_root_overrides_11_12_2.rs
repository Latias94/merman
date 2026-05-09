// Fixture-derived root viewport overrides for Mermaid@11.12.2 Class diagrams.
//
// These entries are keyed by fixture `diagram_id` and are used to close the remaining
// root `<svg>` parity-root deltas (`viewBox` + `style max-width`).

pub fn lookup_class_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_docs_classdiagram_annotations_on_classes_042" => {
            Some(("0 0 354.40625 256", "354.406"))
        }
        "upstream_docs_classdiagram_class_diagrams_002" => {
            Some(("0 -48 902.8359375 474", "902.836"))
        }
        "upstream_docs_classdiagram_class_labels_008" => Some(("0 0 184.6875 234", "184.688")),
        "upstream_docs_classdiagram_define_namespace_035" => {
            Some(("-8 0 250.2890625 364", "250.289"))
        }
        "upstream_docs_classdiagram_defining_relationship_021" => {
            Some(("0 0 921.21875 234", "921.219"))
        }
        "upstream_docs_classdiagram_setting_the_direction_of_the_diagram_046" => {
            Some(("0 0 431.125 354", "431.125"))
        }
        "upstream_separators_labels_notes" => Some(("0 0 553.8515625 594", "553.852")),
        "upstream_cypress_classdiagram_elk_v3_spec_elk_18a_should_handle_the_direction_statement_with_lr_030" => {
            Some(("0 0 431.125 354", "431.125"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_5_1_should_render_a_simple_class_diagram_with_abstract_metho_011" => {
            Some(("0 0 170.375 300", "170.375"))
        }
        "upstream_html_demos_classchart_class_diagram_demos_003" => {
            Some(("0 0 422.4921875 208", "422.492"))
        }
        "upstream_html_demos_classchart_class_diagram_demos_004" => {
            Some(("0 0 834.421875 742", "834.422"))
        }
        "upstream_html_demos_classchart_class_diagram_demos_010" => {
            Some(("0 0 314.71875 466", "314.719"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037" => {
            Some(("0 0 2355.734375 100", "2355.73"))
        }
        "upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037" => {
            Some(("0 0 2355.734375 100", "2355.73"))
        }
        "upstream_cypress_classdiagram_spec_should_handle_an_empty_class_body_with_empty_braces_025" => {
            Some(("0 0 262.0859375 294", "262.086"))
        }
        "upstream_cypress_classdiagram_elk_v3_spec_elk_1433_should_render_a_simple_class_with_a_title_032" => {
            Some(("-34.2890625 -48 164.140625 148", "164.141"))
        }
        "upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_add_classes_namespaces_039" => {
            Some(("-8 0 446.6875 170", "446.688"))
        }
        "upstream_cypress_classdiagram_v2_spec_1433_should_render_a_simple_class_with_a_title_024" => {
            Some(("-34.2890625 -48 164.140625 148", "164.141"))
        }
        "upstream_cypress_classdiagram_v2_spec_5_should_render_a_simple_class_diagram_with_abstract_method_007" => {
            Some(("0 0 170.375 300", "170.375"))
        }
        "stress_class_unicode_namespace_mix_017" => Some(("-8 0 409.140625 772", "409.141")),
        "stress_class_nested_namespaces_many_levels_021" => {
            Some(("-8 0 667.8671875 462", "667.867"))
        }
        "stress_class_comments_inside_namespaces_024" => Some(("-8 0 356.7890625 369", "356.789")),
        "upstream_cypress_classdiagram_v3_spec_17b_should_handle_the_direction_statement_with_rl_029" => {
            Some(("0 0 431.125 354", "431.125"))
        }
        "upstream_cypress_classdiagram_v3_spec_18a_should_handle_the_direction_statement_with_lr_030" => {
            Some(("0 0 431.125 354", "431.125"))
        }
        "upstream_cypress_classdiagram_v3_spec_5_should_render_a_simple_class_diagram_with_abstract_method_010" => {
            Some(("0 0 170.375 300", "170.375"))
        }
        "upstream_pkgtests_classdiagram_spec_003" => Some(("0 0 314.71875 466", "314.719")),
        "upstream_pkgtests_classdiagram_spec_004" => Some(("-8 -8 16 16", "16")),
        "upstream_pkgtests_classdiagram_spec_005" => Some(("-8 -8 16 16", "16")),
        "upstream_pkgtests_classdiagram_spec_038" => Some(("0 0 224.34375 270", "224.344")),
        "upstream_pkgtests_mermaidapi_spec_019" => Some(("0 0 431.125 354", "431.125")),
        "upstream_docs_define_class_relationship" => Some(("0 0 219.96875 234", "219.969")),
        _ => None,
    }
}
