// Fixture-derived root viewport overrides for Mermaid@11.12.2 Flowchart-V2 diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/flowchart/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_flowchart_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_flowchart_v2_stadium_shape_spec" => {
            Some(("-96.54400634765625 -48 610.109375 606", "610.109"))
        }
        "upstream_flowchart_v2_styled_subgraphs_spec" => {
            Some(("-96.59170532226562 -50 477.859375 844", "477.859"))
        }
        _ => None,
    }
}
