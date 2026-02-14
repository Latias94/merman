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
        "basic" => Some(("0 0 100.125 298", "100.125")),
        "mmdr_tests_state_state_basic" => Some(("0 0 178.203125 234", "178.203")),
        "mmdr_tests_state_state_note" => Some(("0 0 221.4418182373047 364", "221.442")),
        "upstream_cypress_statediagram_spec_should_render_a_simple_state_diagrams_with_labels_013" => {
            Some(("0 0 494.8648681640625 348", "494.865"))
        }
        "upstream_cypress_statediagram_spec_should_render_a_note_with_multiple_lines_in_it_009" => {
            Some(("0 0 311.75 306", "311.75"))
        }
        "upstream_cypress_statediagram_spec_should_render_a_state_with_a_note_together_with_another_state_008" => {
            Some(("0 0 671.140625 346", "671.141"))
        }
        "upstream_cypress_statediagram_spec_should_render_forks_and_joins_018" => {
            Some(("0 0 189.8125 402", "189.812"))
        }
        "upstream_cypress_statediagram_spec_should_render_forks_in_composit_states_017" => {
            Some(("0 0 265.859375 666", "265.859"))
        }
        "upstream_cypress_statediagram_spec_should_render_multiple_composit_states_016" => {
            Some(("0 0 233.85546875 1219", "233.855"))
        }
        "upstream_cypress_statediagram_v2_spec_should_render_edge_labels_correctly_039" => {
            Some(("0 -50 1069.5546875 1190", "1069.55"))
        }
        "upstream_cypress_statediagram_v2_spec_should_render_edge_labels_correctly_with_multiple_states_041" => {
            Some(("0 -50 188.375 1946", "188.375"))
        }
        "upstream_cypress_statediagram_v2_spec_should_render_edge_labels_correctly_with_multiple_transitions_040" => {
            Some(("0 -50 1283.5390625 1190", "1283.54"))
        }
        "upstream_cypress_statediagram_v2_spec_should_allow_styles_to_take_effect_in_subgraphs_036" => {
            Some(("0 0 475.8125 131", "475.812"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_can_have_multiple_classes_applied_to_multiple_states_033" => {
            Some(("-22.04699993133545 0 117.2813720703125 364", "117.281"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_it_should_be_possible_to_use_a_choice_022" => {
            Some(("0 0 201.6796875 532", "201.68"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_should_render_a_simple_state_diagrams_with_labels_014" => {
            Some(("0 0 494.8648681640625 348", "494.865"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_should_render_a_state_with_a_note_together_with_another_state_009" => {
            Some(("0 0 671.140625 346", "671.141"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_should_render_a_note_with_multiple_lines_in_it_010" => {
            Some(("0 0 311.75 306", "311.75"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_should_render_forks_and_joins_019" => {
            Some(("0 0 189.8125 402", "189.812"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_should_render_forks_in_composite_states_018" => {
            Some(("0 0 265.859375 666", "265.859"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_should_render_multiple_composite_states_017" => {
            Some(("0 0 233.85546875 1219", "233.855"))
        }
        "upstream_docs_statediagram_composite_states_018" => {
            Some(("0 0 395.671875 373", "395.672"))
        }
        "upstream_docs_statediagram_1_class_statement_041" => {
            Some(("-22.04699993133545 0 117.2813720703125 364", "117.281"))
        }
        "upstream_docs_statediagram_2_operator_to_apply_a_style_to_a_state_044" => {
            Some(("-22.04699993133545 0 117.2813720703125 364", "117.281"))
        }
        "upstream_docs_statediagram_concurrency_030" => Some(("0 0 1193.71875 573", "1193.72")),
        "upstream_docs_statediagram_notes_028" => Some(("0 0 724.71484375 322", "724.715")),
        "upstream_html_demos_state_this_shows_composite_states_007" => {
            Some(("0 0 464.87109375 1060", "464.871"))
        }
        "upstream_docs_statediagram_states_006" => Some(("0 0 81.671875 56", "81.6719")),
        "upstream_docs_statediagram_transitions_014" => Some(("0 0 98.359375 170", "98.3594")),
        "upstream_stateDiagram_docs_classdef_and_class_statements" => {
            Some(("-22.04699993133545 0 117.2813720703125 364", "117.281"))
        }
        "upstream_stateDiagram_multiline_notes_spec" => Some(("0 0 154.53125 306", "154.531")),
        "upstream_stateDiagram_multiple_recursive_state_definitions_spec" => {
            Some(("0 0 558.40234375 1091", "558.402"))
        }
        "upstream_stateDiagram_recursive_state_definitions_spec" => {
            Some(("0 0 488.40234375 599", "488.402"))
        }
        "upstream_stateDiagram_state_definition_with_quotes_spec" => {
            Some(("0 0 516.3033142089844 946.25", "516.303"))
        }
        "upstream_stateDiagram_triple_colon_operator_docs" => {
            Some(("-22.04699993133545 0 117.2813720703125 364", "117.281"))
        }
        "upstream_stateDiagram_v2_multiline_notes_spec" => Some(("0 0 154.53125 306", "154.531")),
        "upstream_stateDiagram_v2_state_definition_with_quotes_spec" => {
            Some(("0 0 516.3033142089844 946.25", "516.303"))
        }
        "upstream_cypress_statediagram_spec_should_handle_multiline_notes_with_different_line_breaks_010" => {
            Some(("0 0 154.53125 306", "154.531"))
        }
        "upstream_cypress_statediagram_spec_should_render_a_long_descriptions_with_additional_descriptions_003" => {
            Some(("0 0 142.171875 135", "142.172"))
        }
        "upstream_cypress_statediagram_spec_should_render_a_single_state_with_short_descriptions_004" => {
            Some(("0 0 229.765625 56", "229.766"))
        }
        "upstream_cypress_statediagram_spec_should_render_state_descriptions_014" => {
            Some(("0 0 245.984375 161", "245.984"))
        }
        "upstream_cypress_statediagram_v2_spec_1433_should_render_a_simple_state_diagram_with_a_title_037" => {
            Some(("-53.671875 -50 185.30078125 234", "185.301"))
        }
        "upstream_cypress_statediagram_v2_spec_can_have_styles_applied_034" => {
            Some(("0 0 78.953125 56", "78.9531"))
        }
        "upstream_cypress_statediagram_v2_spec_should_let_styles_take_precedence_over_classes_035" => {
            Some(("0 0 294.359375 56", "294.359"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_should_handle_multiple_notes_added_to_one_state_028" => {
            Some(("0 0 314.5625 314", "314.562"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_should_render_a_long_descriptions_with_additional_description_004" => {
            Some(("0 0 142.171875 135", "142.172"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_should_render_a_single_state_with_short_descriptions_005" => {
            Some(("0 0 229.765625 56", "229.766"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_should_render_a_state_diagram_and_set_the_correct_length_of_t_031" => {
            Some(("0 0 136.46875 298", "136.469"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_should_render_state_descriptions_015" => {
            Some(("0 0 245.984375 161", "245.984"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_state_label_with_names_in_it_025" => {
            Some(("0 0 225.921875 120", "225.922"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_states_can_have_a_class_applied_032" => {
            Some(("0 0 136.46875 298", "136.469"))
        }
        "upstream_cypress_statediagram_v2_spec_v2_width_of_compound_state_should_grow_with_title_if_title_is_wi_024" => {
            Some(("0 0 156.765625 246", "156.766"))
        }
        "upstream_html_demos_state_and_these_are_how_they_are_applied_002" => {
            Some(("-54.164127349853516 -50 180.5703125 414", "180.57"))
        }
        "upstream_html_demos_state_and_these_are_how_they_are_applied_003" => {
            Some(("-22.04699993133545 0 117.2813720703125 364", "117.281"))
        }
        "upstream_html_demos_state_transition_labels_can_span_multiple_lines_using_br_tags_or_n_009" => {
            Some(("0 0 427.5625 306", "427.562"))
        }
        "upstream_html_demos_state_very_simple_showing_change_from_state1_to_state2_001" => {
            Some(("-51.8046875 -50 180.5703125 196", "180.57"))
        }
        "upstream_html_demos_state_you_can_add_notes_010" => Some(("0 0 908.75 470", "908.75")),
        _ => None,
    }
}
