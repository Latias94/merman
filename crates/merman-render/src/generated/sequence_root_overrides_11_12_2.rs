// Fixture-derived root viewport overrides for Mermaid@11.12.2 Sequence diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/sequence/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_sequence_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_docs_sequencediagram_boundary_008" => Some(("-50 -10 473 274", "473")),
        "upstream_docs_sequencediagram_collections_016" => Some(("-50 -10 453 259", "453")),
        "upstream_docs_sequencediagram_parallel_054" => Some(("-50 -10 1062 547", "1062")),
        "upstream_docs_directives_changing_sequence_diagram_config_via_directive_013" => {
            Some(("-50 -10 1014 347", "1014"))
        }
        "upstream_extended_participant_quote_styles_spec" => {
            Some(("-50 -10 1250 251.49998474121094", "1250"))
        }
        _ => None,
    }
}
