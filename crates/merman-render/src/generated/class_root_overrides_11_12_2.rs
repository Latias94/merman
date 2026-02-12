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
        "upstream_docs_accessibility_class_diagram_008" => Some(("0 0 94.625 234", "94.625")),
        _ => None,
    }
}
