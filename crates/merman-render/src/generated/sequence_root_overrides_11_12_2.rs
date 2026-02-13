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
        "participant_types" => Some(("-50 -10 1250 260", "1250")),
        "html_br_variants_and_wrap" => Some(("-50 -10 953 651", "953")),
        "upstream_cypress_sequencediagram_spec_should_render_bidirectional_arrows_003" => {
            Some(("-50 -10 513 435", "513"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_loops_with_a_slight_margin_007" => {
            Some(("-50 -10 1145 314", "1145"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_with_actor_creation_and_destruc_010" => {
            Some(("-50 -10 1169.5 1504", "1169.5"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_different_note_fonts_when_configured_011" => {
            Some(("-187 -10 587 308", "587"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_notes_aligned_to_the_left_when_configured_014" => {
            Some(("-150 -10 550 308", "550"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_multi_line_notes_aligned_to_the_left_when_configur_015" => {
            Some(("-150 -10 550 346", "550"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_notes_aligned_to_the_right_when_configured_016" => {
            Some(("-150 -10 550 308", "550"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_multi_line_notes_aligned_to_the_right_when_configu_017" => {
            Some(("-150 -10 550 346", "550"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_actor_descriptions_020" => {
            Some(("-50 -10 1145 259", "1145"))
        }
        "upstream_cypress_sequencediagram_spec_should_be_possible_to_use_actor_symbols_instead_of_boxes_023" => {
            Some(("-50 -10 450 259", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_have_actor_top_and_actor_bottom_classes_on_top_and_bottom_024" => {
            Some(("-50 -10 450 259", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_left_of_actor_025" => {
            Some(("-845 -10 1245 308", "1245"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026" => {
            Some(("-166 -10 566 422", "566"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_right_of_actor_027" => {
            Some(("-50 -10 1145 308", "1145"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_right_of_actor_028" => {
            Some(("-50 -10 450 441", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_over_actor_029" => {
            Some(("-397.5 -10 1070 308", "1070"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_over_actor_030" => {
            Some(("-50 -10 450 441", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_messages_from_an_actor_to_the_left_to_one_to_032" => {
            Some(("-50 -10 1145 259", "1145"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_messages_wrapped_inline_from_an_actor_to_the_033" => {
            Some(("-50 -10 450 327", "450"))
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
        "upstream_cypress_sequencediagram_spec_should_render_rect_around_and_inside_alts_040" => {
            Some(("-50 -10 681 597", "681"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_rect_around_and_inside_loops_039" => {
            Some(("-50 -10 871 695", "871"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_rect_around_and_inside_criticals_042" => {
            Some(("-50 -10 681 597", "681"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_rect_around_and_inside_breaks_043" => {
            Some(("-50 -10 681 478", "681"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_autonumber_when_configured_with_such_044" => {
            Some(("-50 -10 685 555", "685"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_autonumber_when_autonumber_keyword_is_used_045" => {
            Some(("-50 -10 685 555", "685"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_dark_theme_from_init_directive_and_configure_font_047" => {
            Some(("-50 -10 485 347", "485"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_with_wrapping_enabled_048" => {
            Some(("-50 -10 450 449", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_override_config_with_directive_settings_050" => {
            Some(("-235 -10 635 327", "635"))
        }
        "upstream_cypress_sequencediagram_spec_should_handle_bidirectional_arrows_with_autonumber_053" => {
            Some(("-50 -10 517 259", "517"))
        }
        "upstream_cypress_sequencediagram_spec_should_handle_different_line_breaks_004" => {
            Some(("-50 -10 1006.5 687", "1006.5"))
        }
        "upstream_cypress_sequencediagram_spec_should_handle_line_breaks_and_wrap_annotations_006" => {
            Some(("-50 -10 822 771", "822"))
        }
        "upstream_cypress_sequencediagram_spec_should_wrap_directive_long_actor_descriptions_022" => {
            Some(("-50 -10 450 401", "450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_complex_sequence_with_all_features_010" => {
            Some(("-50 -10 938 633", "938"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_long_notes_over_collections_022" => {
            Some(("-397.5 -10 1070 308", "1070"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_over_queue_023" => {
            Some(("-50 -10 450 441", "450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_right_of_database_021" => {
            Some(("-50 -10 450 441", "450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019" => {
            Some(("-166 -10 566 433", "566"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_notes_over_actor_and_boundary_024" => {
            Some(("-50 -10 450 284", "450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_messages_from_control_to_entity_026" => {
            Some(("-50 -10 450 338", "450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_long_messages_from_database_to_collections_025" => {
            Some(("-50 -10 1145 259", "1145"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_long_messages_from_queue_to_boundary_027" => {
            Some(("-50 -10 1145 274", "1145"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_long_notes_right_of_entity_020" => {
            Some(("-50 -10 1145 308", "1145"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_long_notes_left_of_boundary_018" => {
            Some(("-845 -10 1245 323", "1245"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_participant_creation_and_destruction_with_differen_012" => {
            Some(("-50 -10 1041 580", "1041"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_alternative_flows_016" => {
            Some(("-50 -10 1450 770", "1450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_notes_and_loops_015" => {
            Some(("-50 -10 1472 793", "1472"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_handle_complex_interactions_between_different_participant_013" => {
            Some(("-50 -10 1480 1030", "1480"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_parallel_processes_with_different_participant_type_014" => {
            Some(("-50 -10 1450 706", "1450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_with_different_font_settings_009" => {
            Some(("-50 -10 1480 1030", "1480"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_with_wrapped_messages_and_notes_011" => {
            Some(("-50 -10 1657 626", "1657"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_a_sequence_diagram_when_usemaxwidth_is_false_030" => {
            Some(("-50 -10 792 957", "792"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_wrapping_text_017" => {
            Some(("-50 -10 1650 660", "1650"))
        }
        "upstream_docs_sequencediagram_boundary_008" => Some(("-50 -10 473 274", "473")),
        "upstream_docs_sequencediagram_collections_016" => Some(("-50 -10 453 259", "453")),
        "upstream_docs_sequencediagram_control_010" => Some(("-50 -10 450 270", "450")),
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
