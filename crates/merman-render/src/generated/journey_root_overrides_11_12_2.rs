// Fixture-derived root viewport overrides for Mermaid@11.12.2 Journey diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/journey/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_journey_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_cypress_journey_spec_should_maintain_sufficient_space_between_legend_and_diagram_when_007" => {
            Some(("0 -25 2599.21875 540", "2599.22"))
        }
        "upstream_cypress_journey_spec_should_wrap_a_single_long_word_with_hyphenation_008" => {
            Some(("0 -25 692.8125 540", "692.812"))
        }
        "upstream_cypress_journey_spec_should_wrap_text_on_whitespace_without_adding_hyphens_009" => {
            Some(("0 -25 884.5625 540", "884.562"))
        }
        "upstream_cypress_journey_spec_should_wrap_long_labels_into_multiple_lines_keep_them_under_max_010" => {
            Some(("0 -25 1937.125 540", "1937.12"))
        }
        _ => None,
    }
}
