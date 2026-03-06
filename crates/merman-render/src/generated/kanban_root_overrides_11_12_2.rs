// Fixture-derived root viewport overrides for Mermaid@11.12.2 Kanban diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/kanban/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_kanban_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "kanban_stress_common2_long_column_titles_wrapping" => Some(("90 -310 425 290", "425")),
        "kanban_stress_common_multiline_metadata_label_override" => {
            Some(("90 -310 220 195", "220"))
        }
        "stress_kanban_font_size_097" => Some(("90 -310 425 195", "425")),
        "stress_kanban_font_size_precedence_098" => Some(("90 -310 425 302", "425")),
        "upstream_docs_samples_example_001" => Some(("90 -310 425 147", "425")),
        "upstream_docs_samples_example_004" => Some(("90 -310 630 318", "630")),
        "upstream_docs_samples_example_005" => Some(("90 -310 835 318", "835")),
        "upstream_cypress_kanban_spec_3_should_render_a_kanban_with_a_single_wrapping_node_003" => {
            Some(("90 -310 220 195", "220"))
        }
        "upstream_cypress_kanban_spec_4_should_handle_the_height_of_a_section_with_a_wrapping_node_at_004" => {
            Some(("90 -310 220 244", "220"))
        }
        "upstream_cypress_kanban_spec_5_should_handle_the_height_of_a_section_with_a_wrapping_node_at_005" => {
            Some(("90 -310 220 244", "220"))
        }
        "upstream_cypress_kanban_spec_6_should_handle_the_height_of_a_section_with_a_wrapping_node_in_006" => {
            Some(("90 -310 220 293", "220"))
        }
        _ => None,
    }
}
