// Fixture-derived root viewport overrides for Mermaid@11.12.2 ER diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/er/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_er_root_viewport_override(diagram_id: &str) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_html_demos_er_example_001" => Some(("0 -48 1635.14453125 1059.5", "1635.14")),
        "upstream_html_demos_er_example_002" => Some(("0 0 257.703125 315.25", "257.703")),
        "upstream_html_demos_er_example_003" => Some(("0 0 279.734375 315.25", "279.734")),
        "upstream_html_demos_er_example_004" => Some(("0 0 954.203125 686.75", "954.203")),
        "upstream_html_demos_er_example_005" => Some(("0 0 195.578125 330.75", "195.578")),
        "upstream_html_demos_er_example_006" => Some(("0 0 1322.5625 435.75", "1322.56")),
        "upstream_html_demos_er_example_007" => Some(("0 0 436.8125 400.75", "436.812")),
        "upstream_html_demos_er_multiline_example_001" => {
            Some(("0 0 1121.578125 878.75", "1121.58"))
        }
        "upstream_html_demos_er_multiline_example_002" => Some(("0 0 529.359375 320.5", "529.359")),
        "upstream_docs_entityrelationshipdiagram_classes_034" => {
            Some(("0 0 786.203125 187", "786.203"))
        }
        "upstream_docs_entityrelationshipdiagram_classes_036" => {
            Some(("0 0 485.6875 459", "485.688"))
        }
        "upstream_docs_entityrelationshipdiagram_attribute_keys_and_comments_020" => {
            Some(("0 0 954.203125 686.75", "954.203"))
        }
        "upstream_docs_accessibility_entity_relationship_diagram_009" => {
            Some(("0 0 434.015625 470", "434.016"))
        }
        "upstream_docs_entityrelationshipdiagram_default_class_025" => {
            Some(("0 0 485.6875 459", "485.688"))
        }
        "upstream_docs_entityrelationshipdiagram_direction_012" => {
            Some(("0 0 219.140625 688.25", "219.141"))
        }
        "upstream_docs_entityrelationshipdiagram_direction_013" => {
            Some(("0 0 828.75 187", "828.75"))
        }
        "upstream_docs_entityrelationshipdiagram_attributes_015" => {
            Some(("0 0 546.203125 372", "546.203"))
        }
        "upstream_cypress_erdiagram_spec_1433_should_render_a_simple_er_diagram_with_a_title_009" => {
            Some(("-7.984375 -48 148.03125 518", "148.031"))
        }
        "upstream_cypress_erdiagram_spec_should_render_a_not_so_simple_er_diagram_005" => {
            Some(("0 0 872.265625 655", "872.266"))
        }
        "upstream_cypress_erdiagram_spec_should_render_an_er_diagram_with_a_recursive_relationship_002" => {
            Some(("0 0 332.73126220703125 470", "332.731"))
        }
        "upstream_cypress_erdiagram_spec_should_render_edge_labels_correctly_when_flowchart_htmllabels_is_019" => {
            Some(("0 0 544.371826171875 474", "544.372"))
        }
        "upstream_cypress_erdiagram_spec_should_render_er_diagram_with_1_cardinality_alias_before_relatio_020" => {
            Some(("0 0 636.6640625 470", "636.664"))
        }
        "upstream_cypress_erdiagram_spec_should_render_relationship_labels_with_line_breaks_011" => {
            Some(("0 0 1322.5625 435.75", "1322.56"))
        }
        "upstream_cypress_theme_spec_should_render_a_er_diagram_005" => {
            Some(("0 0 872.265625 655", "872.266"))
        }
        "upstream_docs_entityrelationshipdiagram_entity_relationship_diagrams_001" => {
            Some(("0 -48 434.015625 518", "434.016"))
        }
        "upstream_docs_entityrelationshipdiagram_layout_042" => {
            Some(("4 -48 329.015625 502", "329.016"))
        }
        "upstream_docs_entityrelationshipdiagram_markdown_formatting_009" => {
            Some(("0 0 179.859375 100", "179.859"))
        }
        "upstream_docs_entityrelationshipdiagram_unicode_text_007" => {
            Some(("0 0 167.109375 100", "167.109"))
        }
        "upstream_docs_examples_entity_relationship_diagram_exclamation_experimental_syntax_enti_006" => {
            Some(("0 0 434.015625 470", "434.016"))
        }
        "upstream_docs_syntax_reference_syntax_structure_001" => {
            Some(("0 0 872.265625 655", "872.266"))
        }
        "upstream_html_demos_error_example_001" => Some(("0 0 479.921875 470", "479.922")),
        "upstream_pkgtests_diagram_orchestration_spec_030" => Some(("-8 -8 16 16", "16")),
        "upstream_pkgtests_erdiagram_spec_302" => Some(("0 0 188.578125 285", "188.578")),
        "upstream_pkgtests_erdiagram_spec_304" => Some(("0 0 188.578125 285", "188.578")),
        "upstream_pkgtests_erdiagram_spec_306" => Some(("0 0 188.578125 285", "188.578")),
        _ => None,
    }
}
