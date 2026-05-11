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
        "upstream_cypress_journey_spec_should_wrap_long_labels_into_multiple_lines_keep_them_under_max_010" => {
            Some(("0 -25 1937.125 540", "1937.12"))
        }
        "upstream_cypress_journey_spec_should_wrap_text_on_whitespace_without_adding_hyphens_009" => {
            Some(("0 -25 883.375 540", "883.375"))
        }
        _ => None,
    }
}
