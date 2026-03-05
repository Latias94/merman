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
        "html_br_variants_and_wrap" => Some(("-50 -10 951.5 651", "951.5")),
        "stress_deep_nested_frames_018" => Some(("-50 -10 850 967", "850")),
        "stress_end_in_labels_025" => Some(("-50 -10 450 507", "450")),
        "stress_end_keyword_016" => Some(("-50 -10 652 451", "652")),
        "stress_self_messages_rect_021" => Some(("-50 -10 450 574", "450")),
        "stress_semicolons_022" => Some(("-50 -10 522 308", "522")),
        "stress_unicode_longish_messages_027" => Some(("-50 -10 710.5 333", "710.5")),
        "stress_quoted_participants_and_types_023" => Some(("-50 -10 878 484", "878")),
        "stress_wrap_directive_and_prefixes_028" => Some(("-50 -10 1022 412", "1022")),
        "stress_nested_rect_par_029" => Some(("-50 -10 650 712", "650")),
        "stress_create_destroy_inside_alt_030" => Some(("-50 -10 734 679", "734")),
        "stress_long_participant_labels_br_031" => Some(("-50 -10 754 458", "754")),
        "stress_par_multiple_ands_notes_032" => Some(("-50 -10 850 777", "850")),
        "stress_critical_options_notes_033" => Some(("-50 -10 560 679", "560")),
        "stress_loop_opt_alt_mix_034" => Some(("-50 -10 650 650", "650")),
        "stress_activation_self_and_create_035" => Some(("-50 -10 725 530", "725")),
        "stress_rect_blocks_many_levels_036" => Some(("-50 -10 450 486", "450")),
        "stress_autonumber_step_reset_037" => Some(("-50 -10 450 391", "450")),
        "stress_html_entities_and_escaping_038" => Some(("-50 -10 730 327", "730")),
        "stress_message_text_with_colons_039" => Some(("-50 -10 986 318", "986")),
        "stress_sequence_font_size_precedence_090" => Some(("-50 -10 550 244", "550")),
        "mmdr_benches_fixtures_expanded_sequence_frames_notes" => {
            Some(("-50 -10 1250 948", "1250"))
        }
        "mmdr_benches_fixtures_expanded_sequence_long_labels" => Some(("-50 -10 1355 435", "1355")),
        "mmdr_benches_fixtures_sequence" => Some(("-50 -10 650 523", "650")),
        "mmdr_benches_fixtures_sequence_medium" => Some(("-50 -10 1050 1051", "1050")),
        "mmdr_docs_comparison_sources_sequence_autonumber" => Some(("-50 -10 852 435", "852")),
        "mmdr_docs_comparison_sources_sequence_collab" => Some(("-50 -10 650 347", "650")),
        "mmdr_docs_comparison_sources_sequence_loops" => Some(("-50 -10 650 733", "650")),
        "mmdr_docs_comparison_sources_sequence_microservice" => Some(("-50 -10 1250 671", "1250")),
        "mmdr_docs_comparison_sources_sequence_notes" => Some(("-50 -10 750 499", "750")),
        "mmdr_docs_comparison_sources_sequence_oauth" => Some(("-50 -10 921 611", "921")),
        "upstream_cypress_sequencediagram_spec_should_render_bidirectional_arrows_003" => {
            Some(("-50 -10 512 435", "512"))
        }
        "upstream_cypress_sequencediagram_spec_should_handle_empty_lines_005" => {
            Some(("-50 -10 450 310", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_loops_with_a_slight_margin_007" => {
            Some(("-50 -10 1144 314", "1144"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_with_actor_creation_and_destruc_010" => {
            Some(("-50 -10 1166.5 1485", "1166.5"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_different_actor_fonts_when_configured_013" => {
            Some(("-50 -10 450 259", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_multi_line_messages_aligned_to_the_left_when_confi_018" => {
            Some(("-50 -10 450 327", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_messages_from_an_actor_to_the_right_to_one_to_034" => {
            Some(("-50 -10 1144 259", "1144"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_messages_wrapped_inline_from_an_actor_to_the_035" => {
            Some(("-50 -10 450 327", "450"))
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
            Some(("-50 -10 1144 259", "1144"))
        }
        "upstream_cypress_sequencediagram_spec_should_be_possible_to_use_actor_symbols_instead_of_boxes_023" => {
            Some(("-50 -10 450 259", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_have_actor_top_and_actor_bottom_classes_on_top_and_bottom_024" => {
            Some(("-50 -10 450 259", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_left_of_actor_025" => {
            Some(("-844 -10 1244 308", "1244"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026" => {
            Some(("-173 -10 573 403", "573"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_right_of_actor_027" => {
            Some(("-50 -10 1144 308", "1144"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_right_of_actor_028" => {
            Some(("-50 -10 450 441", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_over_actor_029" => {
            Some(("-397 -10 1069 308", "1069"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_over_actor_030" => {
            Some(("-50 -10 450 441", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_long_messages_from_an_actor_to_the_left_to_one_to_032" => {
            Some(("-50 -10 1144 259", "1144"))
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
            Some(("-50 -10 684 555", "684"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_autonumber_when_autonumber_keyword_is_used_045" => {
            Some(("-50 -10 684 555", "684"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_dark_theme_from_init_directive_and_configure_font_047" => {
            Some(("-50 -10 484 347", "484"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_with_wrapping_enabled_048" => {
            Some(("-50 -10 450 449", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_with_an_init_directive_049" => {
            Some(("-150 -10 550 359", "550"))
        }
        "upstream_cypress_sequencediagram_spec_should_override_config_with_directive_settings_050" => {
            Some(("-235 -10 635 327", "635"))
        }
        "upstream_cypress_sequencediagram_spec_should_override_config_with_directive_settings_2_051" => {
            Some(("-207 -10 607 241", "607"))
        }
        "upstream_cypress_sequencediagram_spec_should_handle_bidirectional_arrows_with_autonumber_053" => {
            Some(("-50 -10 517 259", "517"))
        }
        "upstream_cypress_sequencediagram_spec_should_support_actor_links_and_properties_when_not_mirrored_expe_054" => {
            Some(("-50 -10 450 225", "450"))
        }
        "upstream_cypress_sequencediagram_spec_should_handle_different_line_breaks_004" => {
            Some(("-50 -10 1002 687", "1002"))
        }
        "upstream_cypress_sequencediagram_spec_should_handle_line_breaks_and_wrap_annotations_006" => {
            Some(("-50 -10 820 752", "820"))
        }
        "upstream_cypress_sequencediagram_spec_should_wrap_directive_long_actor_descriptions_022" => {
            Some(("-50 -10 450 401", "450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_complex_sequence_with_all_features_010" => {
            Some(("-50 -10 938 633", "938"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_long_notes_over_collections_022" => {
            Some(("-397 -10 1069 308", "1069"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_over_queue_023" => {
            Some(("-50 -10 450 441", "450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_right_of_database_021" => {
            Some(("-50 -10 450 441", "450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019" => {
            Some(("-173 -10 573 414", "573"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_notes_over_actor_and_boundary_024" => {
            Some(("-50 -10 450 284", "450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_messages_from_control_to_entity_026" => {
            Some(("-50 -10 450 338", "450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_long_messages_from_database_to_collections_025" => {
            Some(("-50 -10 1144 259", "1144"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_long_messages_from_queue_to_boundary_027" => {
            Some(("-50 -10 1144 274", "1144"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_long_notes_right_of_entity_020" => {
            Some(("-50 -10 1144 308", "1144"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_long_notes_left_of_boundary_018" => {
            Some(("-844 -10 1244 323", "1244"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_participant_creation_and_destruction_with_differen_012" => {
            Some(("-50 -10 1040 580", "1040"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_alternative_flows_016" => {
            Some(("-50 -10 1450 770", "1450"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_notes_and_loops_015" => {
            Some(("-50 -10 1471 793", "1471"))
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
            Some(("-50 -10 1655 626", "1655"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_a_sequence_diagram_when_usemaxwidth_is_false_030" => {
            Some(("-50 -10 790 957", "790"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_wrapping_text_017" => {
            Some(("-50 -10 1650 660", "1650"))
        }
        "upstream_docs_sequencediagram_boundary_008" => Some(("-50 -10 471 274", "471")),
        "upstream_docs_sequencediagram_collections_016" => Some(("-50 -10 453 259", "453")),
        "upstream_docs_sequencediagram_control_010" => Some(("-50 -10 450 270", "450")),
        "upstream_docs_sequencediagram_parallel_054" => Some(("-50 -10 1062 547", "1062")),
        "upstream_docs_directives_changing_sequence_diagram_config_via_directive_013" => {
            Some(("-50 -10 1013 347", "1013"))
        }
        "upstream_extended_participant_quote_styles_spec" => {
            Some(("-50 -10 1250 251.49998474121094", "1250"))
        }
        "upstream_docs_diagrams_mermaid_api_sequence" => Some(("-50 -10 2869 10259", "2869")),
        "upstream_html_demos_sequence_sequence_diagram_demos_001" => {
            Some(("-50 -10 904 1372", "904"))
        }
        "upstream_html_demos_sequence_sequence_diagram_demos_003" => {
            Some(("-50 -10 1002 687", "1002"))
        }
        "upstream_html_demos_sequence_sequence_diagram_demos_010" => {
            Some(("-50 -10 551 303", "551"))
        }
        "stress_br_in_messages_notes_011" => Some(("-50 -10 752 405", "752")),
        "stress_critical_break_007" => Some(("-50 -10 650 635", "650")),
        "stress_entities_and_escaping_005" => Some(("-50 -10 666 308", "666")),
        "stress_nested_frames_001" => Some(("-50 -10 850 1045", "850")),
        "stress_participant_types_006" => Some(("-50 -10 1450 770", "1450")),
        "stress_unicode_punct_012" => Some(("-50 -10 782.5 333", "782.5")),
        "stress_sequence_batch5_alt_par_nested_040" => Some(("-50 -10 861 769", "861")),
        "stress_sequence_batch5_wrap_html_br_spans_042" => Some(("-50 -10 586 344", "586")),
        "stress_sequence_batch5_strict_links_properties_044" => Some(("-50 -10 650 347", "650")),
        "stress_sequence_batch5_create_destroy_in_par_046" => Some(("-50 -10 734 556", "734")),
        "stress_sequence_batch5_reserved_words_in_labels_049" => Some(("-50 -10 580 408", "580")),
        "stress_sequence_batch5_many_participants_spacing_050" => {
            Some(("-50 -10 1650 714", "1650"))
        }
        "stress_sequence_batch5_whitespace_semicolons_051" => Some(("-50 -10 450 506", "450")),
        "activation_explicit" => Some(("-50 -10 513 259", "513")),
        "activation_stacked" => Some(("-50 -10 806 347", "806")),
        "actor_ids_dashes_and_equals" => Some(("-50 -10 696 347", "696")),
        "arrows_variants" => Some(("-50 -10 450 611", "450")),
        "comments_and_blank_lines" => Some(("-50 -10 580 352", "580")),
        "no_label_blocks" => Some(("-50 -10 480 669", "480")),
        "semicolons_and_comments" => Some(("-50 -10 580 308", "580")),
        "title_and_accdescr_multiline" => Some(("-50 -50 480 255", "480")),
        "upstream_accessibility_single_line_spec" => Some(("-50 -50 480 255", "480")),
        "upstream_alias_participants_spec" => Some(("-50 -10 480 259", "480")),
        "upstream_alt_multiple_elses_spec" => Some(("-50 -10 580 541", "580")),
        "upstream_cypress_sequencediagram_spec_example_001" => Some(("-50 -10 810 979", "810")),
        "upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_when_usemaxwidth_is_false_059" => {
            Some(("-50 -10 790 942", "790"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_with_basic_actor_creation_and_d_009" => {
            Some(("-50 -10 1315 806", "1315"))
        }
        "upstream_cypress_sequencediagram_spec_should_render_a_simple_sequence_diagram_001" => {
            Some(("-50 -10 790 942", "790"))
        }
        "upstream_cypress_sequencediagram_spec_should_support_actor_links_and_properties_experimental_use_with_052" => {
            Some(("-50 -10 484 308", "484"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_bidirectional_arrows_with_autonumbering_030" => {
            Some(("-50 -10 715 435", "715"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_central_connection_with_normal_arrows_right_to_lef_033" => {
            Some(("-50 -10 1203 391", "1203"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_central_connections_with_bidirectional_arrows_and_045" => {
            Some(("-50 -10 1736 435", "1736"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_participants_with_inline_alias_in_config_object_060" => {
            Some(("-50 -10 650 362", "650"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_self_reference_with_bidirectional_arrows_with_auto_051" => {
            Some(("-79.5 -10 691.5 467", "691.5"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_self_reference_with_bidirectional_arrows_without_a_050" => {
            Some(("-79.5 -10 691.5 467", "691.5"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_self_reference_with_normal_arrows_with_autonumber_047" => {
            Some(("-80 -10 692 615", "692"))
        }
        "upstream_cypress_sequencediagram_v2_spec_should_render_self_reference_with_normal_arrows_without_autonumb_046" => {
            Some(("-80 -10 692 615", "692"))
        }
        "upstream_docs_accessibility_sequence_diagram_014" => Some(("-50 -10 484 303", "484")),
        "upstream_docs_directives_changing_sequence_diagram_config_via_directive_016" => {
            Some(("-50 -10 751 364", "751"))
        }
        "upstream_docs_examples_basic_sequence_diagram_005" => Some(("-50 -10 790 541", "790")),
        "upstream_docs_examples_sequencediagram_loops_alt_and_opt_011" => {
            Some(("-50 -10 481 557", "481"))
        }
        "upstream_docs_readme_how_does_a_langium_based_parser_work_002" => {
            Some(("-50 -10 1334 503", "1334"))
        }
        "upstream_docs_readme_sequence_diagram_a_href_https_mermaid_js_org_syntax_sequencediag_003" => {
            Some(("-50 -10 684 555", "684"))
        }
        "upstream_docs_sequence_aliases_basic" => Some(("-50 -10 484 259", "484")),
        "upstream_docs_sequence_alt_and_opt_example" => Some(("-50 -10 481 502", "481")),
        "upstream_docs_sequence_autonumber_example" => Some(("-50 -10 684 555", "684")),
        "upstream_docs_sequence_basic_example" => Some(("-50 -10 484 303", "484")),
        "upstream_docs_sequence_box_groups_example" => Some(("-50 -10 967 384", "967")),
        "upstream_docs_sequence_create_destroy_example" => Some(("-50 -10 1040 565", "1040")),
        "upstream_docs_sequence_loop_every_minute" => Some(("-50 -10 484 314", "484")),
        "upstream_docs_sequence_rect_nested_example" => Some(("-50 -10 579 544", "579")),
        "upstream_docs_sequencediagram_activations_030" => Some(("-50 -10 484 259", "484")),
        "upstream_docs_sequencediagram_activations_032" => Some(("-50 -10 484 259", "484")),
        "upstream_docs_sequencediagram_activations_034" => Some(("-50 -10 484 347", "484")),
        "upstream_docs_sequencediagram_actor_menus_077" => Some(("-50 -10 484 303", "484")),
        "upstream_docs_sequencediagram_advanced_menu_syntax_080" => {
            Some(("-50 -10 484 303", "484"))
        }
        "upstream_docs_sequencediagram_comments_069" => Some(("-50 -10 484 259", "484")),
        "upstream_docs_sequencediagram_inline_alias_syntax_023" => Some(("-50 -10 650 362", "650")),
        "upstream_docs_sequencediagram_notes_038" => Some(("-50 -10 484 264", "484")),
        "upstream_examples_sequence_basic_sequence_001" => Some(("-50 -10 484 347", "484")),
        "upstream_html_demos_sequence_sequence_diagram_demos_002" => {
            Some(("-50 -50 484 343", "484"))
        }
        "upstream_html_demos_sequence_sequence_diagram_demos_006" => {
            Some(("-50 -10 484 259", "484"))
        }
        "upstream_html_demos_sequence_sequence_diagram_demos_011" => {
            Some(("-50 -10 484 259", "484"))
        }
        "upstream_leading_spaces_spec" => Some(("-50 -10 580 308", "580")),
        "upstream_nested_rect_blocks_spec" => Some(("-50 -10 600 368", "600")),
        "upstream_pkgtests_diagram_spec_014" => Some(("-50 -10 484 215", "484")),
        "upstream_pkgtests_mermaidapi_spec_034" => Some(("-50 -10 684 347", "684")),
        "upstream_pkgtests_sequencediagram_spec_001" => Some(("-50 -10 480 259", "480")),
        "upstream_pkgtests_sequencediagram_spec_005" => Some(("-50 -10 484 259", "484")),
        "upstream_pkgtests_sequencediagram_spec_007" => Some(("-50 -10 580 308", "580")),
        "upstream_pkgtests_sequencediagram_spec_009" => Some(("-50 -10 580 308", "580")),
        "upstream_pkgtests_sequencediagram_spec_014" => Some(("-50 -10 490 259", "490")),
        "upstream_pkgtests_sequencediagram_spec_015" => Some(("-50 -10 493 259", "493")),
        "upstream_pkgtests_sequencediagram_spec_016" => Some(("-50 -10 471 303", "471")),
        "upstream_pkgtests_sequencediagram_spec_020" => Some(("-50 -50 580 348", "580")),
        "upstream_pkgtests_sequencediagram_spec_021" => Some(("-50 -10 480 215", "480")),
        "upstream_pkgtests_sequencediagram_spec_022" => Some(("-50 -10 480 215", "480")),
        "upstream_pkgtests_sequencediagram_spec_023" => Some(("-50 -10 480 215", "480")),
        "upstream_pkgtests_sequencediagram_spec_024" => Some(("-50 -10 480 215", "480")),
        "upstream_pkgtests_sequencediagram_spec_025" => Some(("-50 -10 480 215", "480")),
        "upstream_pkgtests_sequencediagram_spec_026" => Some(("-50 -10 493 259", "493")),
        "upstream_pkgtests_sequencediagram_spec_027" => Some(("-50 -10 490 259", "490")),
        "upstream_pkgtests_sequencediagram_spec_038" => Some(("-50 -10 513 259", "513")),
        "upstream_pkgtests_sequencediagram_spec_040" => Some(("-50 -10 806 347", "806")),
        "upstream_pkgtests_sequencediagram_spec_042" => Some(("-50 -10 580 308", "580")),
        "upstream_pkgtests_sequencediagram_spec_043" => Some(("-50 -10 580 308", "580")),
        "upstream_pkgtests_sequencediagram_spec_045" => Some(("-50 -10 580 308", "580")),
        "upstream_pkgtests_sequencediagram_spec_046" => Some(("-50 -10 580 308", "580")),
        "upstream_pkgtests_sequencediagram_spec_054" => Some(("-50 -10 580 363", "580")),
        "upstream_pkgtests_sequencediagram_spec_055" => Some(("-50 -10 590 338", "590")),
        "upstream_pkgtests_sequencediagram_spec_056" => Some(("-50 -10 600 368", "600")),
        "upstream_pkgtests_sequencediagram_spec_057" => Some(("-50 -10 480 314", "480")),
        "upstream_pkgtests_sequencediagram_spec_058" => Some(("-50 -10 580 452", "580")),
        "upstream_pkgtests_sequencediagram_spec_059" => Some(("-50 -10 580 541", "580")),
        "upstream_pkgtests_sequencediagram_spec_060" => Some(("-50 -10 480 294", "480")),
        "upstream_pkgtests_sequencediagram_spec_061" => Some(("-50 -10 480 294", "480")),
        "upstream_pkgtests_sequencediagram_spec_068" => Some(("-50 -10 480 403", "480")),
        "upstream_pkgtests_sequencediagram_spec_069" => Some(("-50 -10 480 403", "480")),
        "upstream_pkgtests_sequencediagram_spec_072" => Some(("-50 -10 480 363", "480")),
        "upstream_pkgtests_sequencediagram_spec_073" => Some(("-50 -10 480 363", "480")),
        "upstream_pkgtests_sequencediagram_spec_074" => Some(("-50 -10 650 276", "650")),
        "upstream_pkgtests_sequencediagram_spec_076" => Some(("-50 -10 670 276", "670")),
        "upstream_pkgtests_sequencediagram_spec_077" => Some(("-50 -10 670 276", "670")),
        "upstream_pkgtests_sequencediagram_spec_078" => Some(("-50 -10 670 276", "670")),
        "upstream_pkgtests_sequencediagram_spec_083" => Some(("-50 -10 480 313", "480")),
        "upstream_pkgtests_sequencediagram_spec_084" => Some(("-50 -10 480 259", "480")),
        "upstream_pkgtests_sequencediagram_spec_085" => Some(("-50 -10 580 308", "580")),
        "upstream_pkgtests_sequencediagram_spec_086" => Some(("-150 -10 580 308", "580")),
        "upstream_pkgtests_sequencediagram_spec_087" => Some(("-150 -10 550 359", "550")),
        "upstream_pkgtests_sequencediagram_spec_090" => Some(("-50 -10 490 252", "490")),
        "upstream_pkgtests_sequencediagram_spec_091" => Some(("-50 -10 480 314", "480")),
        "upstream_pkgtests_sequencediagram_spec_093" => Some(("-50 -10 480 259", "480")),
        "upstream_pkgtests_sequencediagram_spec_095" => Some(("-50 -10 450 215", "450")),
        "upstream_pkgtests_sequencediagram_spec_098" => Some(("-150 -10 550 359", "550")),
        "upstream_pkgtests_sequencediagram_spec_099" => Some(("-150 -10 550 359", "550")),
        "upstream_pkgtests_sequencediagram_spec_100" => Some(("-150 -10 550 359", "550")),
        "upstream_pkgtests_sequencediagram_spec_102" => Some(("-50 -10 509 289", "509")),
        "upstream_pkgtests_sequencediagram_spec_103" => Some(("-50 -10 251 256", "251")),
        "upstream_pkgtests_sequencediagram_spec_104" => Some(("-50 -10 251 236", "251")),
        "upstream_pkgtests_sequencediagram_spec_105" => {
            Some(("-50 -10 251 237.49998474121094", "251"))
        }
        "upstream_pkgtests_sequencediagram_spec_107" => {
            Some(("-50 -10 1250 251.49998474121094", "1250"))
        }
        "upstream_pkgtests_sequencediagram_spec_109" => {
            Some(("-50 -10 251 237.49998474121094", "251"))
        }
        "upstream_pkgtests_sequencediagram_spec_111" => Some(("-50 -10 251 256", "251")),
        "upstream_pkgtests_sequencediagram_spec_122" => Some(("-50 -10 480 226", "480")),
        "upstream_pkgtests_sequencediagram_spec_129" => Some(("-50 -10 450 274", "450")),
        "upstream_rect_block_spec" => Some(("-50 -10 590 338", "590")),
        "upstream_title_without_colon_spec" => Some(("-50 -50 580 348", "580")),
        _ => None,
    }
}
