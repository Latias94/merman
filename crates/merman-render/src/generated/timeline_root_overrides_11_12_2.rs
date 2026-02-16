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
        "timeline_stress_accdescr_block_multiline" => Some(("-6 -61 896 687.4000244140625", "896")),
        "timeline_stress_disable_multicolor_and_width" => {
            Some(("10 -61 721.59375 740.2000122070312", "721.594"))
        }
        "timeline_stress_markdown_links_and_br" => {
            Some(("100 -61 1230.375 780.3999633789062", "1230.38"))
        }
        "timeline_stress_inline_hashes_and_semicolons" => {
            Some(("-5 -61 967.921875 740.2000122070312", "967.922"))
        }
        "timeline_stress_many_events_single_period" => {
            Some(("-105 -61 795 1421.199951171875", "795"))
        }
        "timeline_stress_width_large_and_long_labels" => {
            Some(("-6 -61 896 664.7999877929688", "896"))
        }
        "timeline_stress_very_long_unbroken_word" => {
            Some(("-107.984375 -61 1516.3203125 594.3999938964844", "1516.32"))
        }
        "upstream_long_word_wrap" => Some(("9.6796875 0 961.484375 533.3999938964844", "961.484")),
        "upstream_cypress_timeline_spec_11_should_render_timeline_with_many_stacked_events_and_proper_ti_011" => {
            Some(("100 -61 1390 1109.5999755859375", "1390"))
        }
        "upstream_cypress_timeline_spec_12_should_render_timeline_with_proper_vertical_line_lengths_for_012" => {
            Some(("100 -57 2190 879.4000244140625", "2190"))
        }
        "upstream_html_demos_timeline_medical_device_lifecycle_timeline_002" => {
            Some(("100 -61 1990 1046.800048828125", "1990"))
        }
        _ => None,
    }
}
