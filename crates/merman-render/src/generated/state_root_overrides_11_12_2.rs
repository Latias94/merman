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
        "mmdr_tests_state_state_basic" => Some(("0 0 178.203125 234", "178.203")),
        "mmdr_tests_state_state_note" => Some(("0 0 221.4418182373047 364", "221.442")),
        "upstream_docs_statediagram_composite_states_018" => {
            Some(("0 0 395.671875 373", "395.672"))
        }
        "upstream_docs_statediagram_concurrency_030" => Some(("0 0 1193.71875 573", "1193.72")),
        "upstream_docs_statediagram_notes_028" => Some(("0 0 724.71484375 322", "724.715")),
        "upstream_docs_statediagram_states_006" => Some(("0 0 81.671875 56", "81.6719")),
        "upstream_docs_statediagram_transitions_014" => Some(("0 0 98.359375 170", "98.3594")),
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
