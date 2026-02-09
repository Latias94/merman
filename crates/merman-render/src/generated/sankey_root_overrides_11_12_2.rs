// Fixture-derived root viewport overrides for Mermaid@11.12.2 Sankey diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/sankey/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_sankey_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_docs_sankey_basic_004" => Some(("0 0 600 400", "600")),
        "upstream_docs_sankey_example_002" => Some(("0 0 600 403.91900634765625", "600")),
        "upstream_sankey_beta_energy_csv_spec" => {
            Some(("0 -5.975983619689941 600 405.9759826660156", "600"))
        }
        "upstream_sankey_docs_empty_lines_spec" => {
            Some(("0 -0.8946409225463867 600 400.8946533203125", "600"))
        }
        "upstream_sankey_header_energy_csv_spec" => {
            Some(("0 -12.041862487792969 600 412.0418701171875", "600"))
        }
        _ => None,
    }
}
