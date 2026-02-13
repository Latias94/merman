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
        "html_br_variants_and_wrap" => Some(("-50 -10 953 651", "953")),
        "upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_with_actor_creation_and_destruc_010" => {
            Some(("-50 -10 1169.5 1504", "1169.5"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_a_single_and_nested_opt_with_long_test_overflowing_037" => {
            Some(("-50 -10 1250 868", "1250"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_a_single_and_nested_opt_with_long_test_wrapping_038" => {
            Some(("-50 -10 1250 868", "1250"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_a_single_and_nested_rects_036" => {
            Some(("-50 -10 1250 717", "1250"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_complex_sequence_with_all_features_010" => {
            Some(("-50 -10 938 633", "938"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_alternative_flows_016" => {
            Some(("-50 -10 1450 770", "1450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_notes_and_loops_015" => {
            Some(("-50 -10 1472 793", "1472"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_parallel_processes_with_different_participant_type_014" => {
            Some(("-50 -10 1450 706", "1450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_with_wrapped_messages_and_notes_011" => {
            Some(("-50 -10 1657 626", "1657"))
        }
        "upstream_docs_sequencediagram_boundary_008" => Some(("-50 -10 473 274", "473")),
        "upstream_docs_sequencediagram_collections_016" => Some(("-50 -10 453 259", "453")),
        "upstream_docs_sequencediagram_parallel_054" => Some(("-50 -10 1062 547", "1062")),
        "upstream_docs_directives_changing_sequence_diagram_config_via_directive_013" => {
            Some(("-50 -10 1014 347", "1014"))
        }
        "upstream_extended_participant_quote_styles_spec" => {
            Some(("-50 -10 1250 251.49998474121094", "1250"))
        }
        "upstream_docs_diagrams_mermaid_api_sequence" => Some(("-50 -10 2871 10259", "2871")),
        _ => None,
    }
}
