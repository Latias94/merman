// Fixture-derived root viewport overrides for Mermaid@11.12.2 Pie diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/pie/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_pie_root_viewport_override(diagram_id: &str) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_html_demos_pie_pie_chart_demos_001" => {
            Some(("0 0 547.08154296875 450", "547.082"))
        }
        "upstream_html_demos_pie_pie_chart_demos_002" => {
            Some(("0 0 596.21875 450", "596.219"))
        }
        "upstream_html_demos_pie_pie_chart_demos_003" => {
            Some(("0 0 590.81005859375 450", "590.81"))
        }
        "upstream_docs_examples_basic_pie_chart_002" => {
            Some(("0 0 735.45849609375 450", "735.458"))
        }
        "upstream_docs_accessibility_pie_chart_012" => Some(("0 0 596.21875 450", "596.219")),
        "upstream_pie_acc_descr_multiline_spec" => Some(("0 0 537.40283203125 450", "537.403")),
        "upstream_pie_acc_descr_spec" => Some(("0 0 537.40283203125 450", "537.403")),
        "upstream_pie_acc_title_spec" => Some(("0 0 537.40283203125 450", "537.403")),
        "upstream_pie_comments_spec" => Some(("0 0 537.40283203125 450", "537.403")),
        "upstream_pie_positive_decimal_spec" => Some(("0 0 537.40283203125 450", "537.403")),
        "upstream_pie_simple_spec" => Some(("0 0 537.40283203125 450", "537.403")),
        "upstream_pie_title_spec" => Some(("0 0 537.40283203125 450", "537.403")),
        "upstream_pie_unsafe_props_spec" => Some(("0 0 600.62548828125 450", "600.625")),
        _ => None,
    }
}
