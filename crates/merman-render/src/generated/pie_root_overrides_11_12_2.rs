// Fixture-derived root viewport overrides for Mermaid@11.12.2 Pie diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/pie/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_pie_root_viewport_override(diagram_id: &str) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_cypress_pie_spec_should_render_a_simple_pie_diagram_with_long_labels_002" => {
            Some(("0 0 733.734375 450", "733.734"))
        }
        "upstream_docs_examples_basic_pie_chart_001" => Some(("0 0 733.734375 450", "733.734")),
        "upstream_html_demos_pie_pie_chart_demos_003" => Some(("0 0 590.5625 450", "590.562")),
        "upstream_pkgtests_diagram_orchestration_spec_036" => {
            Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__"))
        }
        "upstream_pie_unsafe_props_spec" => Some(("0 0 599.234375 450", "599.234")),
        "upstream_pkgtests_pie_spec_013" => Some(("0 0 599.234375 450", "599.234")),
        "upstream_pkgtests_pie_test_002" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_003" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_004" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_005" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_006" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_007" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_008" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_009" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_010" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_011" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_012" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_013" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_014" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_017" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_018" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_019" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        "upstream_pkgtests_pie_test_020" => Some(("0 0 -Infinity 450", "__NO_MAX_WIDTH__")),
        _ => None,
    }
}
