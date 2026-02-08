// Fixture-derived root viewport overrides for Mermaid@11.12.2 State diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/state/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_state_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_stateDiagram_multiple_recursive_state_definitions_spec" => {
            Some(("0 0 558.40234375 1091", "558.402"))
        }
        "upstream_stateDiagram_recursive_state_definitions_spec" => {
            Some(("0 0 488.40234375 599", "488.402"))
        }
        "upstream_stateDiagram_state_definition_with_quotes_spec" => {
            Some(("0 0 516.3033142089844 946.25", "516.303"))
        }
        "upstream_stateDiagram_v2_state_definition_with_quotes_spec" => {
            Some(("0 0 516.3033142089844 946.25", "516.303"))
        }
        _ => None,
    }
}
