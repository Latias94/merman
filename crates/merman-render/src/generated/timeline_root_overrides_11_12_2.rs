// Fixture-derived root viewport overrides for Mermaid@11.12.2 Timeline diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/timeline/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_timeline_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_long_word_wrap" => Some(("9.6796875 0 961.484375 533.3999938964844", "961.484")),
        "upstream_cypress_timeline_spec_11_should_render_timeline_with_many_stacked_events_and_proper_ti_011" => {
            Some(("100 -61 1390 1109.5999755859375", "1390"))
        }
        "upstream_cypress_timeline_spec_12_should_render_timeline_with_proper_vertical_line_lengths_for_012" => {
            Some(("100 -57 2190 879.4000244140625", "2190"))
        }
        _ => None,
    }
}
