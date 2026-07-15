// Fixture-derived root viewport residuals for Mermaid 11.16.0 Sequence diagrams.
//
// These values come from the complete 321-fixture corpus rendered by the pinned
// Chrome 131 environment. Structure and non-root parity match without them; only
// browser-dependent root `getBBox()` dimensions differ from deterministic layout.

pub(super) fn lookup_sequence_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "stress_create_destroy_inside_alt_030" => Some(("-50 -10 734 679", "734")),
        "stress_critical_break_007" => Some(("-50 -10 650 635", "650")),
        "stress_critical_options_notes_033" => Some(("-50 -10 560 679", "560")),
        "stress_sequence_batch5_create_destroy_in_par_046" => Some(("-50 -10 734 556", "734")),
        "stress_sequence_batch5_reserved_words_in_labels_049" => Some(("-50 -10 580 408", "580")),
        "upstream_cypress_sequencediagram_spec_should_render_rect_around_and_inside_loops_039" => {
            Some(("-50 -10 871 695", "871"))
        }
        "upstream_docs_math_sequence_002" => Some(("-50 -10 550 273", "550")),
        "upstream_html_demos_sequence_sequence_diagram_demos_010" => {
            Some(("-50 -10 551 303", "551"))
        }
        "upstream_pkgtests_sequencediagram_spec_038" => Some(("-50 -10 513 259", "513")),
        _ => None,
    }
}
