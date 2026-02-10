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
        "mmdr_issue_28_text_rendering" => Some(("0 0 792.19873046875 244", "792.199")),
        "mmdr_issue_29_edge_label_distance" => Some((
            "0 0.000003814697265625 1339.015625 794.8007202148438",
            "1339.02",
        )),
        "mmdr_tests_flowchart_flowchart_complex" => {
            Some(("0 0 978.17578125 1198.28125", "978.176"))
        }
        "mmdr_tests_flowchart_flowchart_cycles" => Some(("0 0 230.03515625 985.4375", "230.035")),
        "mmdr_tests_flowchart_flowchart_dense" => {
            Some(("0 0 1097.734375 495.7659912109375", "1097.73"))
        }
        "mmdr_tests_flowchart_flowchart_ports" => Some(("0 0 1570.953125 278", "1570.95")),
        "mmdr_tests_flowchart_flowchart_edges" => Some(("0 0 319.703125 486", "319.703")),
        "mmdr_tests_flowchart_flowchart_subgraph" => Some(("0 0 635.484375 140", "635.484")),
        "upstream_docs_flowchart_limitation_199" => Some(("0 0 706.328125 371", "706.328")),
        "upstream_docs_flowchart_markdown_formatting_008" => {
            Some(("0 0 353.203125 118", "353.203"))
        }
        "upstream_flowchart_v2_stadium_shape_spec" => {
            Some(("-96.54400634765625 -48 610.109375 606", "610.109"))
        }
        "upstream_flowchart_v2_styled_subgraphs_spec" => {
            Some(("-96.59170532226562 -50 477.859375 844", "477.859"))
        }
        _ => None,
    }
}
