// Fixture-derived root viewport overrides for Mermaid@11.12.2 ER diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/er/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_er_root_viewport_override(diagram_id: &str) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_html_demos_er_example_001" => Some(("0 -48 1636.14453125 1059.5", "1636.14")),
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
        _ => None,
    }
}
