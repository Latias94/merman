mod composed;
mod document;
mod encode;
mod framing;
mod label;
mod layout;
mod normalization;
mod width;
mod wrapped;

pub(crate) use composed::ComposedTextPlan;
#[cfg(test)]
use document::encode_text_lines;
pub(crate) use document::{
    BudgetedTextDocument, BudgetedTextLine, charge_text_layout, visit_safe_line_graphemes,
};
pub(crate) use encode::visit_html_escaped_text;
pub(crate) use framing::{
    push_document_field, push_document_list, push_line_field, push_line_list,
    push_optional_document_field, push_wrapped_field, push_wrapped_list,
    visit_quoted_terminal_text,
};
pub(crate) use label::{
    LabelBreakPolicy, NormalizedLabelPlan, try_build_normalized_label_lines,
    try_measure_normalized_label_lines, try_plan_normalized_label_lines_with_policy,
};
#[cfg(test)]
pub(crate) use label::{
    try_build_normalized_label_lines_with_probe, try_plan_normalized_label_lines,
};
pub(crate) use layout::{
    NormalizedTrimmedTextPlan, try_clone_layout_text, try_concat_layout_text,
    try_plan_normalized_trimmed_text, try_repeat_layout_char,
};
pub use normalization::{normalize_terminal_diagnostic, normalize_terminal_text};
pub(crate) use width::{
    SafeLine, SafeText, terminal_char_display_width, terminal_line_display_width,
};
pub(crate) use wrapped::BudgetedWrappedText;

#[cfg(test)]
use crate::color::AsciiColorMode;
#[cfg(test)]
use crate::error::AsciiError;
#[cfg(test)]
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
#[cfg(test)]
use crate::resource::ResourceContext;
#[cfg(test)]
use std::borrow::Cow;
#[cfg(test)]
use unicode_segmentation::UnicodeSegmentation;

#[cfg(test)]
const MAX_DIAGNOSTIC_GRAPHEMES: usize = normalization::diagnostic_limits().0;
#[cfg(test)]
const MAX_DIAGNOSTIC_INPUT_BYTES: usize = normalization::diagnostic_limits().1;
#[cfg(test)]
const MAX_DIAGNOSTIC_BYTES: usize = normalization::diagnostic_limits().2;
#[cfg(test)]
const DIAGNOSTIC_ELLIPSIS: &str = normalization::diagnostic_limits().3;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;
    use std::cell::Cell;

    fn options_with_limit(id: AsciiResourceLimitId, limit: usize) -> AsciiRenderOptions {
        let resources = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(id, limit)
            .expect("positive resource limit");
        AsciiRenderOptions::ascii().with_resource_policy(resources)
    }

    fn assert_limit_error(
        error: AsciiError,
        expected_id: AsciiResourceLimitId,
        expected_actual: usize,
        expected_max: usize,
    ) {
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == expected_id
                    && details.actual == expected_actual
                    && details.max == expected_max
        ));
    }

    #[test]
    fn printable_text_stays_borrowed() {
        let input = "Cafe\u{301} 👩‍💻 🇺🇸 中文";
        let normalized = normalize_terminal_text(input);

        assert!(matches!(normalized, Cow::Borrowed(_)));
        assert_eq!(normalized, input);
    }

    #[test]
    fn crlf_is_structural_but_tab_and_lone_carriage_return_are_visible() {
        assert_eq!(
            normalize_terminal_text("one\r\ntwo\tthree\rfour"),
            "one\ntwo\\u{9}three\\u{D}four"
        );
    }

    #[test]
    fn c0_c1_escape_del_and_bidi_controls_are_exhaustively_visible() {
        let bidi = [
            '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}',
            '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
        ];
        let mut input = String::new();
        input.extend(['\0', '\u{1b}', '\u{7f}', '\u{80}', '\u{9f}']);
        input.extend(bidi);

        let normalized = normalize_terminal_text(&input);

        for control in input.chars() {
            assert!(
                !normalized.contains(control),
                "raw control {control:?} leaked"
            );
            assert!(
                normalized.contains(&format!("\\u{{{:X}}}", u32::from(control))),
                "missing visible escape for {control:?}: {normalized}"
            );
        }
    }

    #[test]
    fn standalone_zero_width_graphemes_are_visible() {
        assert_eq!(
            normalize_terminal_text("\u{301}\u{200d}\u{fe0f}"),
            "\\u{301}\\u{200D}\\u{FE0F}"
        );
    }

    #[test]
    fn legal_joiners_and_variation_selectors_survive_inside_visible_graphemes() {
        for input in ["👩‍💻", "✈️", "a\u{200c}b", "👍🏽"] {
            assert_eq!(normalize_terminal_text(input), input);
        }
    }

    #[test]
    fn normalization_is_idempotent() {
        let once = normalize_terminal_text("a\u{1b}\u{202e}\u{301}\r\nb").into_owned();
        let twice = normalize_terminal_text(&once);

        assert_eq!(twice, once);
    }

    #[test]
    fn budgeted_document_normalization_matches_safe_text_boundary() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        for input in [
            "one\r\ntwo\tthree\rfour",
            "a\u{1b}\u{202e}\u{301}\r\nb",
            "👩‍💻 ✈️ a\u{200c}b 👍🏽",
            "\u{301}\u{200d}\u{fe0f}",
        ] {
            let rendered = encode_text_lines(vec![input.to_string()], &options)
                .expect("unbounded budgeted normalization should render");
            let expected = normalize_terminal_text(input);

            assert_eq!(rendered, expected.as_ref());
        }
    }

    #[test]
    fn diagnostics_truncate_only_at_grapheme_boundaries() {
        let input = "👩‍💻".repeat(MAX_DIAGNOSTIC_GRAPHEMES + 1);
        let normalized = normalize_terminal_diagnostic(&input);

        assert_eq!(
            normalized.graphemes(true).count(),
            MAX_DIAGNOSTIC_GRAPHEMES + 3
        );
        assert!(normalized.ends_with("..."));
    }

    #[test]
    fn diagnostics_bound_control_escape_expansion_by_bytes() {
        let input = "\u{1b}".repeat(MAX_DIAGNOSTIC_INPUT_BYTES * 2);
        let normalized = normalize_terminal_diagnostic(&input);

        assert!(normalized.len() <= MAX_DIAGNOSTIC_BYTES);
        assert!(normalized.ends_with(DIAGNOSTIC_ELLIPSIS));
        assert!(!normalized.contains('\u{1b}'));
    }

    #[test]
    fn diagnostics_reject_one_oversized_grapheme_without_splitting_it() {
        let mut input = String::from("a");
        input.extend(std::iter::repeat_n('\u{301}', MAX_DIAGNOSTIC_INPUT_BYTES));

        let normalized = normalize_terminal_diagnostic(&input);

        assert_eq!(normalized, DIAGNOSTIC_ELLIPSIS);
        assert!(normalized.len() <= MAX_DIAGNOSTIC_BYTES);
    }

    #[test]
    fn html_output_budget_counts_actual_escaped_bytes_at_exact_boundary() {
        let escaped = "&lt;&amp;";
        let exact = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, escaped.len())
            .with_color_mode(AsciiColorMode::Html);
        let rendered = encode_text_lines(vec!["<&".to_string()], &exact)
            .expect("exact escaped byte budget should render");

        assert_eq!(rendered, escaped);

        let below = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, escaped.len() - 1)
            .with_color_mode(AsciiColorMode::Html);
        let error = encode_text_lines(vec!["<&".to_string()], &below)
            .expect_err("one byte below escaped output should fail");
        assert_limit_error(
            error,
            AsciiResourceLimitId::MaxOutputBytes,
            escaped.len(),
            escaped.len() - 1,
        );
    }

    #[test]
    fn plain_output_budget_counts_utf8_bytes_at_exact_boundary() {
        let text = "中";
        let exact = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, text.len());
        let rendered = encode_text_lines(vec![text.to_string()], &exact)
            .expect("exact UTF-8 byte budget should render");

        assert_eq!(rendered, text);

        let below = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, text.len() - 1);
        let error = encode_text_lines(vec![text.to_string()], &below)
            .expect_err("one byte below the UTF-8 output should fail");
        assert_limit_error(
            error,
            AsciiResourceLimitId::MaxOutputBytes,
            text.len(),
            text.len() - 1,
        );
    }

    #[test]
    fn document_output_admission_precedes_materialization_for_all_encodings() {
        let cases = [
            (AsciiColorMode::Plain, "<&>\"'中\ntail"),
            (AsciiColorMode::Ansi16, "<&>\"'中\ntail"),
            (AsciiColorMode::Ansi256, "<&>\"'中\ntail"),
            (AsciiColorMode::TrueColor, "<&>\"'中\ntail"),
            (AsciiColorMode::Html, "&lt;&amp;&gt;&quot;&#39;中\ntail"),
        ];

        for (color_mode, expected) in cases {
            let exact = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len())
                .with_color_mode(color_mode);
            let exact_materialized = Cell::new(false);
            let mut document = BudgetedTextDocument::new(&exact);
            document
                .push_line("<&>\"'中")
                .expect("the first row should enter the document");
            document
                .push_line("tail")
                .expect("the second row should enter the document");
            let rendered = document
                .finish_with_probe(&exact, &exact_materialized)
                .expect("the exact encoded output should be admitted");

            assert!(exact_materialized.get(), "mode={color_mode:?}");
            assert_eq!(rendered, expected, "mode={color_mode:?}");

            let below =
                options_with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len() - 1)
                    .with_color_mode(color_mode);
            let below_retained = std::rc::Rc::new(Cell::new(0));
            let mut document = BudgetedTextDocument::new(&below);
            document.set_retain_probe(std::rc::Rc::clone(&below_retained));
            document
                .push_line("<&>\"'中")
                .expect("the first row still fits below the complete document boundary");
            let retained_before_second_row = below_retained.get();
            let error = document
                .push_line("tail")
                .expect_err("N-1 must reject before retaining the second row");

            assert_eq!(
                below_retained.get(),
                retained_before_second_row,
                "mode={color_mode:?}",
            );
            assert_limit_error(
                error,
                AsciiResourceLimitId::MaxOutputBytes,
                expected.len(),
                expected.len() - 1,
            );
        }
    }

    #[test]
    fn line_fragment_output_admission_precedes_any_retained_append() {
        let cases = [
            (AsciiColorMode::Plain, "abc", 3),
            (AsciiColorMode::Plain, "\u{1b}", "\\u{1B}".len()),
            (AsciiColorMode::Html, "<", "&lt;".len()),
        ];

        for (color_mode, raw, encoded_len) in cases {
            let options = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, encoded_len - 1)
                .with_color_mode(color_mode);
            let retained = std::rc::Rc::new(Cell::new(0));
            let mut document = BudgetedTextDocument::new(&options);
            document.set_retain_probe(std::rc::Rc::clone(&retained));

            let error = document
                .push_line(raw)
                .expect_err("N-1 must reject the complete normalized fragment before append");

            assert_eq!(retained.get(), 0, "mode={color_mode:?}, raw={raw:?}");
            assert_limit_error(
                error,
                AsciiResourceLimitId::MaxOutputBytes,
                encoded_len,
                encoded_len - 1,
            );
        }
    }

    #[test]
    fn wrapped_output_admission_covers_prefix_word_space_and_continuation_prefix() {
        let prefix_options = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, 1);
        let prefix_retained = std::rc::Rc::new(Cell::new(0));
        let prefix_producer_visited = Cell::new(false);
        let mut document = BudgetedTextDocument::new(&prefix_options);
        document.set_retain_probe(std::rc::Rc::clone(&prefix_retained));
        let error = document
            .push_wrapped_prefixed_line_with("- ", "  ", 80, |line| {
                prefix_producer_visited.set(true);
                line.push_str("a")
            })
            .expect_err("N-1 prefix bytes must reject before starting the producer");
        assert!(!prefix_producer_visited.get());
        assert_eq!(prefix_retained.get(), 0);
        assert_limit_error(error, AsciiResourceLimitId::MaxOutputBytes, 2, 1);

        let exact_word_options = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, 5);
        let mut document = BudgetedTextDocument::new(&exact_word_options);
        document
            .push_wrapped_prefixed_line_with("- ", "  ", 80, |line| line.push_str("abc"))
            .expect("the exact prefix and body output bytes should fit");
        assert_eq!(
            document
                .finish(&exact_word_options)
                .expect("exact output should encode"),
            "- abc"
        );

        let word_options = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, 4);
        let word_retained = std::rc::Rc::new(Cell::new(0));
        let mut document = BudgetedTextDocument::new(&word_options);
        document.set_retain_probe(std::rc::Rc::clone(&word_retained));
        let error = document
            .push_wrapped_prefixed_line_with("- ", "  ", 80, |line| line.push_str("abc"))
            .expect_err("N-1 word bytes must reject before retaining the wrapped body");
        assert_eq!(word_retained.get(), 0);
        assert_limit_error(error, AsciiResourceLimitId::MaxOutputBytes, 5, 4);

        let space_options = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, 2);
        let space_retained = std::rc::Rc::new(Cell::new(0));
        let mut document = BudgetedTextDocument::new(&space_options);
        document.set_retain_probe(std::rc::Rc::clone(&space_retained));
        let error = document
            .push_wrapped_prefixed_line_with("", "", 80, |line| line.push_str("a b"))
            .expect_err("N-1 synthesized space must reject before entering the row");
        assert_eq!(space_retained.get(), 0);
        assert_limit_error(error, AsciiResourceLimitId::MaxOutputBytes, 3, 2);

        let exact_continuation_options =
            options_with_limit(AsciiResourceLimitId::MaxOutputBytes, 5);
        let mut document = BudgetedTextDocument::new(&exact_continuation_options);
        document
            .push_wrapped_prefixed_line_with("", ">>", 1, |line| line.push_str("ab"))
            .expect("the exact continuation prefix output bytes should fit");
        assert_eq!(
            document
                .finish(&exact_continuation_options)
                .expect("exact continuation output should encode"),
            "a\n>>b"
        );

        let continuation_options = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, 4);
        let continuation_retained = std::rc::Rc::new(Cell::new(0));
        let mut document = BudgetedTextDocument::new(&continuation_options);
        document.set_retain_probe(std::rc::Rc::clone(&continuation_retained));
        let error = document
            .push_wrapped_prefixed_line_with("", ">>", 1, |line| line.push_str("ab"))
            .expect_err("N-1 continuation prefix must reject before prefix insertion");
        assert_eq!(continuation_retained.get(), 0);
        assert_limit_error(error, AsciiResourceLimitId::MaxOutputBytes, 5, 4);
    }

    #[test]
    fn wrapped_prefix_budgets_are_admitted_before_body_retention() {
        let first_prefix_options = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 1);
        let first_prefix_retained = std::rc::Rc::new(Cell::new(0));
        let first_prefix_producer_visited = Cell::new(false);
        let mut document = BudgetedTextDocument::new(&first_prefix_options);
        document.set_retain_probe(std::rc::Rc::clone(&first_prefix_retained));
        let error = document
            .push_wrapped_prefixed_line_with("- ", "  ", 80, |line| {
                first_prefix_producer_visited.set(true);
                line.push_str("a")
            })
            .expect_err("an oversized first prefix must reject before starting the producer");
        assert!(!first_prefix_producer_visited.get());
        assert_eq!(first_prefix_retained.get(), 0);
        assert_limit_error(error, AsciiResourceLimitId::MaxDocumentCells, 2, 1);

        let continuation_options = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 3);
        let continuation_retained = std::rc::Rc::new(Cell::new(0));
        let mut document = BudgetedTextDocument::new(&continuation_options);
        document.set_retain_probe(std::rc::Rc::clone(&continuation_retained));
        let error = document
            .push_wrapped_prefixed_line_with("", ">>", 1, |line| line.push_str("ab"))
            .expect_err("a continuation prefix must reject before retaining its first body cell");
        assert_eq!(continuation_retained.get(), 0);
        assert_limit_error(error, AsciiResourceLimitId::MaxDocumentCells, 4, 3);

        let grapheme = "\"\u{301}";
        assert_eq!(grapheme.graphemes(true).count(), 1);
        let exact_grapheme_options =
            options_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, grapheme.len());
        let mut document = BudgetedTextDocument::new(&exact_grapheme_options);
        document
            .push_wrapped_prefixed_line_with(grapheme, "", 80, |line| line.push_str("a"))
            .expect("the exact first-prefix grapheme budget should fit");
        assert_eq!(
            document
                .finish(&exact_grapheme_options)
                .expect("exact first prefix should encode"),
            format!("{grapheme}a")
        );

        let grapheme_options =
            options_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, grapheme.len() - 1);
        let grapheme_retained = std::rc::Rc::new(Cell::new(0));
        let grapheme_producer_visited = Cell::new(false);
        let mut document = BudgetedTextDocument::new(&grapheme_options);
        document.set_retain_probe(std::rc::Rc::clone(&grapheme_retained));
        let error = document
            .push_wrapped_prefixed_line_with(grapheme, "", 80, |line| {
                grapheme_producer_visited.set(true);
                line.push_str("a")
            })
            .expect_err("an oversized prefix grapheme must reject before starting the producer");
        assert!(!grapheme_producer_visited.get());
        assert_eq!(grapheme_retained.get(), 0);
        assert_limit_error(
            error,
            AsciiResourceLimitId::MaxGraphemeBytes,
            grapheme.len(),
            grapheme.len() - 1,
        );

        let continuation = "x\u{301}";
        assert_eq!(continuation.graphemes(true).count(), 1);
        let exact_continuation_options =
            options_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, continuation.len());
        let mut document = BudgetedTextDocument::new(&exact_continuation_options);
        document
            .push_wrapped_prefixed_line_with("", continuation, 1, |line| line.push_str("ab"))
            .expect("the exact continuation-prefix grapheme budget should fit");
        assert_eq!(
            document
                .finish(&exact_continuation_options)
                .expect("exact continuation prefix should encode"),
            format!("a\n{continuation}b")
        );

        let continuation_options = options_with_limit(
            AsciiResourceLimitId::MaxGraphemeBytes,
            continuation.len() - 1,
        );
        let continuation_retained = std::rc::Rc::new(Cell::new(0));
        let mut document = BudgetedTextDocument::new(&continuation_options);
        document.set_retain_probe(std::rc::Rc::clone(&continuation_retained));
        let error = document
            .push_wrapped_prefixed_line_with("", continuation, 1, |line| line.push_str("ab"))
            .expect_err("an oversized used continuation grapheme must reject in planning");
        assert_eq!(continuation_retained.get(), 0);
        assert_limit_error(
            error,
            AsciiResourceLimitId::MaxGraphemeBytes,
            continuation.len(),
            continuation.len() - 1,
        );
    }

    #[test]
    fn unused_continuation_prefix_does_not_enter_content_budgets() {
        let oversized = "x\u{301}";
        assert_eq!(oversized.graphemes(true).count(), 1);
        let options = AsciiRenderOptions::ascii().with_resource_policy(
            AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 1)
                .expect("positive grapheme limit")
                .with_limit(AsciiResourceLimitId::MaxDocumentCells, 1)
                .expect("positive document limit")
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, 1)
                .expect("positive output limit"),
        );
        let mut document = BudgetedTextDocument::new(&options);
        document
            .push_wrapped_prefixed_line_with("", oversized, 80, |line| line.push_str("a"))
            .expect("an unused continuation prefix must not consume content limits");
        assert_eq!(
            document.finish(&options).expect("document should encode"),
            "a"
        );
    }

    #[test]
    fn wrapped_planning_failure_leaves_shared_ledgers_unchanged() {
        let options = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 1);
        let mut document = BudgetedTextDocument::new(&options);
        document.push_line("x").expect("prior row should fit");
        let work_before = document.resources_mut().layout_work_used();
        let cells_before = document.resources_mut().document_cells_used();

        document
            .push_wrapped_prefixed_line_with("", "", 80, |line| line.push_str("y"))
            .expect_err("the second cell must be rejected during planning");

        assert_eq!(document.resources_mut().layout_work_used(), work_before);
        assert_eq!(document.resources_mut().document_cells_used(), cells_before);
        assert_eq!(
            document.finish(&options).expect("prior row should remain"),
            "x"
        );
    }

    #[test]
    fn wrapped_success_replays_the_producer_after_admission() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let visits = Cell::new(0usize);
        let mut document = BudgetedTextDocument::new(&options);
        document
            .push_wrapped_prefixed_line_with("- ", "  ", 80, |line| {
                visits.set(visits.get() + 1);
                line.push_str("body")
            })
            .expect("an admitted producer should replay deterministically");
        assert_eq!(visits.get(), 2);
        assert_eq!(
            document.finish(&options).expect("document should encode"),
            "- body"
        );
    }

    #[test]
    fn wrapped_replay_mismatch_discards_local_rows_and_shared_charges() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let visits = Cell::new(0usize);
        let mut document = BudgetedTextDocument::new(&options);
        document
            .push_line("prior")
            .expect("prior row should render");
        let work_before = document.resources_mut().layout_work_used();
        let cells_before = document.resources_mut().document_cells_used();

        let error = document
            .push_wrapped_prefixed_line_with("", "", 80, |line| {
                let visit = visits.get();
                visits.set(visit + 1);
                if visit == 0 {
                    line.push_str("planned")
                } else {
                    line.push_str("changed")
                }
            })
            .expect_err("same-sized but different replay text must be rejected");

        assert_eq!(visits.get(), 2);
        assert!(matches!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "structured_text",
                feature: "wrapped text replay",
            }
        ));
        assert_eq!(document.resources_mut().layout_work_used(), work_before);
        assert_eq!(document.resources_mut().document_cells_used(), cells_before);
        assert_eq!(
            document.finish(&options).expect("prior row should remain"),
            "prior"
        );
    }

    #[test]
    fn wrapped_replay_error_discards_local_rows_and_shared_charges() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let visits = Cell::new(0usize);
        let mut document = BudgetedTextDocument::new(&options);
        document
            .push_line("prior")
            .expect("prior row should render");
        let work_before = document.resources_mut().layout_work_used();
        let cells_before = document.resources_mut().document_cells_used();
        let replay_error = AsciiError::UnsupportedFeature {
            diagram_type: "test",
            feature: "wrapped replay failure",
        };

        let error = document
            .push_wrapped_prefixed_line_with("", "", 80, |line| {
                let visit = visits.get();
                visits.set(visit + 1);
                line.push_str("body")?;
                if visit == 1 {
                    return Err(replay_error.clone());
                }
                Ok(())
            })
            .expect_err("a second-pass producer error must abort the transaction");

        assert_eq!(visits.get(), 2);
        assert_eq!(error, replay_error);
        assert_eq!(document.resources_mut().layout_work_used(), work_before);
        assert_eq!(document.resources_mut().document_cells_used(), cells_before);
        assert_eq!(
            document.finish(&options).expect("prior row should remain"),
            "prior"
        );
    }

    #[test]
    fn wrapped_layout_work_is_admitted_at_exact_and_limit_minus_one() {
        let unbounded = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut document = BudgetedTextDocument::new(&unbounded);
        document
            .push_wrapped_prefixed_line_with("", "", 80, |line| line.push_str("a"))
            .expect("the work probe should render");
        let total_work = document.resources_mut().layout_work_used();

        let exact = options_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, total_work);
        let mut document = BudgetedTextDocument::new(&exact);
        document
            .push_wrapped_prefixed_line_with("", "", 80, |line| line.push_str("a"))
            .expect("the exact planning and replay work should fit");
        assert_eq!(document.resources_mut().layout_work_used(), total_work);

        let below = options_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, total_work - 1);
        let retained = std::rc::Rc::new(Cell::new(0));
        let mut document = BudgetedTextDocument::new(&below);
        document.set_retain_probe(std::rc::Rc::clone(&retained));
        let error = document
            .push_wrapped_prefixed_line_with("", "", 80, |line| line.push_str("a"))
            .expect_err("one unit below the complete two-pass work must reject during planning");

        assert_eq!(retained.get(), 0);
        assert_eq!(document.resources_mut().layout_work_used(), 0);
        assert_eq!(document.resources_mut().document_cells_used(), 0);
        assert_limit_error(
            error,
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            total_work,
            total_work - 1,
        );
    }

    #[test]
    fn wrapped_prefix_width_scan_checks_layout_work_incrementally() {
        let options = options_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 2);
        let producer_visited = Cell::new(false);
        let mut document = BudgetedTextDocument::new(&options);

        let error = document
            .push_wrapped_prefixed_line_with("abc", "", 80, |line| {
                producer_visited.set(true);
                line.push_str("body")
            })
            .expect_err("the third prefix grapheme must exceed the scan budget immediately");

        assert!(!producer_visited.get());
        assert_eq!(document.resources_mut().layout_work_used(), 0);
        assert_eq!(document.resources_mut().document_cells_used(), 0);
        assert_limit_error(error, AsciiResourceLimitId::MaxLayoutWorkUnits, 3, 2);
    }

    #[test]
    fn wrapped_empty_and_explicit_line_breaks_preserve_existing_rows() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut document = BudgetedTextDocument::new(&options);
        document
            .push_wrapped_prefixed_line_with("empty: ", "       ", 80, |_line| Ok(()))
            .expect("an empty producer should emit its first prefix");
        document
            .push_wrapped_prefixed_line_with("- ", "  ", 80, |line| line.push_str("a\nb"))
            .expect("an explicit line break should select the continuation prefix");

        assert_eq!(
            document.finish(&options).expect("document should encode"),
            "empty: \n- a\n  b"
        );
    }

    #[test]
    fn quoted_output_admission_precedes_rejected_fragment_materialization() {
        let line_options = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, 2);
        let line_retained = std::rc::Rc::new(Cell::new(0));
        let mut document = BudgetedTextDocument::new(&line_options);
        document.set_retain_probe(std::rc::Rc::clone(&line_retained));
        let error = document
            .push_line_with(|line| line.push_quoted_text("a"))
            .expect_err("N-1 closing quote must reject before append");
        assert_eq!(line_retained.get(), 2);
        assert_limit_error(error, AsciiResourceLimitId::MaxOutputBytes, 3, 2);

        let wrapped_options = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, 3);
        let wrapped_retained = std::rc::Rc::new(Cell::new(0));
        let mut document = BudgetedTextDocument::new(&wrapped_options);
        document.set_retain_probe(std::rc::Rc::clone(&wrapped_retained));
        let error = document
            .push_wrapped_prefixed_line_with("", "", 80, |line| line.push_quoted_text("\""))
            .expect_err("N-1 wrapped closing quote must reject before normalization");
        assert_eq!(wrapped_retained.get(), 0);
        assert_limit_error(error, AsciiResourceLimitId::MaxOutputBytes, 4, 3);
    }

    #[test]
    fn structured_rows_debit_layout_work_at_exact_boundary() {
        let lines = vec!["one".to_string(), "two".to_string()];
        let exact = options_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 8);
        let rendered = encode_text_lines(lines.clone(), &exact)
            .expect("two row scans should fit their exact layout work budget");

        assert_eq!(rendered, "one\ntwo");

        let below = options_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 7);
        let error = encode_text_lines(lines, &below)
            .expect_err("one work unit below the row scans should fail");
        assert_limit_error(error, AsciiResourceLimitId::MaxLayoutWorkUnits, 8, 7);
    }

    #[test]
    fn control_escape_expansion_is_budgeted_before_document_append() {
        let escaped = "\\u{1B}";
        let exact = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, escaped.len());
        let rendered = encode_text_lines(vec!["\u{1b}".to_string()], &exact)
            .expect("one visible escape should fit its exact document budget");

        assert_eq!(rendered, escaped);

        let mut document = BudgetedTextDocument::new(&exact);
        let error = document
            .push_line("\u{1b}x")
            .expect_err("one extra visible cell should fail before it is appended");
        assert_limit_error(
            error,
            AsciiResourceLimitId::MaxDocumentCells,
            escaped.len() + 1,
            escaped.len(),
        );
    }

    #[test]
    fn streaming_line_writer_stops_before_later_fragment_after_document_limit() {
        let options = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 1);
        let visited = Cell::new(0usize);
        let mut document = BudgetedTextDocument::new(&options);

        let error = document
            .push_line_with(|line| {
                visited.set(visited.get() + 1);
                line.push_str("ab")?;
                visited.set(visited.get() + 1);
                line.push_str("never reached")
            })
            .expect_err("the second fragment must be rejected before it is visited");

        assert_eq!(visited.get(), 1);
        assert_limit_error(error, AsciiResourceLimitId::MaxDocumentCells, 2, 1);
    }

    #[test]
    fn streaming_wrapped_writer_stops_before_later_fragment_after_document_limit() {
        let options = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 1);
        let visited = Cell::new(0usize);
        let mut document = BudgetedTextDocument::new(&options);

        let error = document
            .push_wrapped_prefixed_line_with("", "", 80, |line| {
                visited.set(visited.get() + 1);
                line.push_str("ab")?;
                visited.set(visited.get() + 1);
                line.push_str("never reached")
            })
            .expect_err("the over-limit fragment must stop the producer immediately");

        assert_eq!(visited.get(), 1);
        assert_limit_error(error, AsciiResourceLimitId::MaxDocumentCells, 2, 1);
    }

    #[test]
    fn streaming_wrapped_writer_preserves_word_and_paragraph_layout() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut document = BudgetedTextDocument::new(&options);
        document
            .push_wrapped_prefixed_line_with("- ", "  ", 10, |line| {
                line.push_str("  alpha   ")?;
                line.push_str("beta\ngamma  ")
            })
            .expect("streamed fragments should preserve wrapping semantics");

        assert_eq!(
            document.finish(&options).expect("document should encode"),
            "- alpha\n  beta\n  gamma"
        );
    }

    #[test]
    fn quoted_structured_fields_preserve_whitespace_and_delimiter_ownership() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut document = BudgetedTextDocument::new(&options);
        document
            .push_line_with(|line| {
                line.push_str("- ")?;
                line.push_quoted_text(" leading \"value\" \\ ")
            })
            .expect("quoted fields should stream without materializing an escaped copy");
        document
            .push_line_with(|line| line.push_quoted_text("line\nbreak"))
            .expect("quoted line fields should encode structural newlines visibly");

        assert_eq!(
            document.finish(&options).expect("document should encode"),
            concat!(r#"- " leading \"value\" \\ ""#, "\n", r#""line\nbreak""#,)
        );
    }

    #[test]
    fn wrapped_quoted_fields_preserve_whitespace_and_escape_structural_text() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut document = BudgetedTextDocument::new(&options);
        document
            .push_wrapped_prefixed_line_with("- ", "  ", 80, |line| {
                line.push_quoted_text(" leading \"value\" \\ \nnext")
            })
            .expect("wrapped quoted fields should retain authored separators safely");

        assert_eq!(
            document.finish(&options).expect("document should encode"),
            r#"- " leading \"value\" \\ \nnext""#,
        );
    }

    #[test]
    fn wrapped_quoted_fields_honor_exact_document_cell_limits() {
        let exact = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 7);
        let mut document = BudgetedTextDocument::new(&exact);
        document
            .push_wrapped_prefixed_line_with("- ", "  ", 80, |line| line.push_quoted_text("a b"))
            .expect("two prefix cells plus five quoted-value cells should fit exactly");
        assert_eq!(
            document.finish(&exact).expect("document should encode"),
            "- \"a b\""
        );

        let limited = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 6);
        let mut document = BudgetedTextDocument::new(&limited);
        let error = document
            .push_wrapped_prefixed_line_with("- ", "  ", 80, |line| line.push_quoted_text("a b"))
            .expect_err("limit-minus-one must reject the quoted row");
        assert_limit_error(error, AsciiResourceLimitId::MaxDocumentCells, 7, 6);
    }

    #[test]
    fn words_after_a_long_word_resume_normal_row_packing() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut document = BudgetedTextDocument::new(&options);
        document
            .push_wrapped_prefixed_line_with("- ", "  ", 5, |line| {
                line.push_str("abcd ")?;
                line.push_str("e f")
            })
            .expect("normal words after a split word should share the next row");

        assert_eq!(
            document.finish(&options).expect("document should encode"),
            "- abc\n  d\n  e f"
        );
    }

    #[test]
    fn optional_text_escape_expansion_hits_layout_limit_at_exact_plus_one() {
        let exact = options_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 14);
        let mut document = BudgetedTextDocument::new(&exact);
        document
            .push_optional_line(Some("\u{1b}"))
            .expect("raw scan plus streamed visible escape should fit");
        assert_eq!(
            document.finish(&exact).expect("document should encode"),
            "\\u{1B}"
        );

        let mut document = BudgetedTextDocument::new(&exact);
        let error = document
            .push_optional_line(Some("\u{1b}x"))
            .expect_err("one additional streamed grapheme should exceed the exact limit");
        assert_limit_error(error, AsciiResourceLimitId::MaxLayoutWorkUnits, 15, 14);
    }

    #[test]
    fn optional_text_trims_all_whitespace_without_an_invalid_drain_range() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut document = BudgetedTextDocument::new(&options);

        document
            .push_optional_line(Some("  \n  "))
            .expect("whitespace-only optional text should be ignored");
        assert_eq!(
            document.finish(&options).expect("document should encode"),
            ""
        );
    }

    #[test]
    fn optional_text_trims_structural_whitespace_without_materializing_it() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut document = BudgetedTextDocument::new(&options);
        document
            .push_optional_prefixed_line("title: ", Some(" \r\n\tfoo\t \n"))
            .expect("trimmed optional text should stream into the document");

        assert_eq!(
            document.finish(&options).expect("document should encode"),
            "title: \\u{9}foo\\u{9}"
        );
    }

    #[test]
    fn cjk_document_budget_uses_profile_display_width_at_exact_boundary() {
        let exact = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 2)
            .with_terminal_width_profile(TerminalWidthProfile::Cjk);
        let rendered = encode_text_lines(vec!["·".to_string()], &exact)
            .expect("ambiguous glyph should fit its exact CJK width");

        assert_eq!(rendered, "·");

        let below = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 1)
            .with_terminal_width_profile(TerminalWidthProfile::Cjk);
        let error = encode_text_lines(vec!["·".to_string()], &below)
            .expect_err("one cell below the CJK width should fail");
        assert_limit_error(error, AsciiResourceLimitId::MaxDocumentCells, 2, 1);
    }

    #[test]
    fn wrapped_rows_enter_document_through_the_same_budgeted_boundary() {
        let exact = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 8);
        let mut document = BudgetedTextDocument::new(&exact);
        document
            .push_wrapped_prefixed_line("- ", "  ", "abcd", 4)
            .expect("two four-cell rows should fit the exact document budget");
        assert_eq!(
            document
                .finish(&exact)
                .expect("budgeted rows should encode"),
            "- ab\n  cd"
        );

        let below = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 7);
        let mut document = BudgetedTextDocument::new(&below);
        let error = document
            .push_wrapped_prefixed_line("- ", "  ", "abcd", 4)
            .expect_err("one cell below the wrapped document should fail before row insertion");
        assert_limit_error(error, AsciiResourceLimitId::MaxDocumentCells, 8, 7);
    }

    #[test]
    fn wrapped_quoted_visible_escapes_remain_atomic_at_a_wrap_boundary() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut document = BudgetedTextDocument::new(&options);
        document
            .push_wrapped_prefixed_line_with("", "", 12, |line| {
                push_wrapped_field(line, "", "value", "before\u{202e}after")
            })
            .expect("quoted visible escapes should remain terminal-safe when wrapped");

        let rendered = document
            .finish(&options)
            .expect("wrapped field should encode");
        assert!(rendered.contains(r"\u{202E}"), "{rendered:?}");
        assert!(!rendered.contains("\\u{2\n02E}"), "{rendered:?}");
    }

    #[test]
    fn quoted_framing_checks_raw_grapheme_before_scalar_escape() {
        let grapheme = "\"\u{301}";
        assert_eq!(grapheme.graphemes(true).count(), 1);

        let exact = options_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, grapheme.len());
        let mut document = BudgetedTextDocument::new(&exact);
        document
            .push_line_with(|line| line.push_quoted_text(grapheme))
            .expect("quoted grapheme should fit its exact raw-byte budget");

        let below = options_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, grapheme.len() - 1);
        let mut document = BudgetedTextDocument::new(&below);
        let error = document
            .push_line_with(|line| line.push_quoted_text(grapheme))
            .expect_err("quoting must not split an oversized source grapheme");
        assert_limit_error(
            error,
            AsciiResourceLimitId::MaxGraphemeBytes,
            grapheme.len(),
            grapheme.len() - 1,
        );
    }

    #[test]
    fn oversized_structural_prefix_does_not_force_vertical_label_text() {
        let options = AsciiRenderOptions::ascii();
        let mut document = BudgetedTextDocument::new(&options);
        document
            .push_wrapped_prefixed_line("          ", "          ", "Leaf", 8)
            .expect("an oversized hierarchy prefix should leave a useful content width");

        assert_eq!(
            document.finish(&options).expect("document should encode"),
            "          Leaf"
        );
    }

    #[test]
    fn grapheme_budget_checks_whole_cluster_at_exact_boundary() {
        let grapheme = "👩‍💻";
        let exact = options_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, grapheme.len());
        let rendered = encode_text_lines(vec![grapheme.to_string()], &exact)
            .expect("whole grapheme should fit its exact byte budget");

        assert_eq!(rendered, grapheme);

        let below = options_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, grapheme.len() - 1);
        let error = encode_text_lines(vec![grapheme.to_string()], &below)
            .expect_err("one byte below the grapheme size should fail");
        assert_limit_error(
            error,
            AsciiResourceLimitId::MaxGraphemeBytes,
            grapheme.len(),
            grapheme.len() - 1,
        );
    }

    #[test]
    fn raw_zero_width_cluster_is_checked_before_visible_escape_expansion() {
        let grapheme = "\u{301}\u{301}";
        assert_eq!(grapheme.graphemes(true).count(), 1);

        let exact = options_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, grapheme.len());
        let rendered = encode_text_lines(vec![grapheme.to_string()], &exact)
            .expect("raw cluster equal to the limit should normalize safely");
        assert_eq!(rendered, "\\u{301}\\u{301}");

        let below = options_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, grapheme.len() - 1);
        let error = encode_text_lines(vec![grapheme.to_string()], &below)
            .expect_err("oversized raw cluster must fail before escape expansion is retained");
        assert_limit_error(
            error,
            AsciiResourceLimitId::MaxGraphemeBytes,
            grapheme.len(),
            grapheme.len() - 1,
        );
    }

    #[test]
    fn label_builder_rejects_control_expansion_before_materialization() {
        let escaped = "\\u{1B}";
        let below = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, escaped.len() - 1);
        let mut resources = ResourceContext::new(below.resources);
        let materialized = Cell::new(false);

        let error = try_build_normalized_label_lines_with_probe(
            "\u{1b}",
            TerminalWidthProfile::Unicode,
            false,
            None,
            &mut resources,
            &materialized,
        )
        .expect_err("control expansion must be rejected before retaining a String or Vec");

        assert!(!materialized.get());
        assert_eq!(resources.layout_work_used(), 0);
        assert_eq!(resources.document_cells_used(), 0);
        assert_limit_error(
            error,
            AsciiResourceLimitId::MaxDocumentCells,
            escaped.len(),
            escaped.len() - 1,
        );
    }

    #[test]
    fn label_builder_checks_output_lower_bound_before_materialization() {
        let escaped = "\\u{1B}";
        let below = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, escaped.len() - 1);
        let mut resources = ResourceContext::new(below.resources);
        let materialized = Cell::new(false);

        let error = try_build_normalized_label_lines_with_probe(
            "\u{1b}",
            TerminalWidthProfile::Unicode,
            false,
            None,
            &mut resources,
            &materialized,
        )
        .expect_err("normalized output bytes must be rejected before label materialization");

        assert!(!materialized.get());
        assert_eq!(resources.layout_work_used(), 0);
        assert_eq!(resources.document_cells_used(), 0);
        assert_limit_error(
            error,
            AsciiResourceLimitId::MaxOutputBytes,
            escaped.len(),
            escaped.len() - 1,
        );
    }

    #[test]
    fn label_builder_uses_exact_cjk_and_zwj_document_cells() {
        let raw = "·<br>👩‍💻";
        let exact = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 4)
            .with_terminal_width_profile(TerminalWidthProfile::Cjk);
        let resources = ResourceContext::new(exact.resources);
        let label = try_build_normalized_label_lines(
            raw,
            TerminalWidthProfile::Cjk,
            false,
            None,
            &resources,
        )
        .expect("exact CJK and ZWJ cells should be admitted")
        .expect("non-trimmed label should be retained");
        let (lines, width) = label.into_parts();

        assert_eq!(lines, ["·", "👩‍💻"]);
        assert_eq!(width, 2);

        let below = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 3)
            .with_terminal_width_profile(TerminalWidthProfile::Cjk);
        let resources = ResourceContext::new(below.resources);
        let error = try_build_normalized_label_lines(
            raw,
            TerminalWidthProfile::Cjk,
            false,
            None,
            &resources,
        )
        .expect_err("one cell below the exact CJK and ZWJ total should fail");
        assert_limit_error(error, AsciiResourceLimitId::MaxDocumentCells, 4, 3);
    }

    #[test]
    fn authored_break_budget_counts_retained_rows_not_structural_separators() {
        let raw = "First<br />Line";
        let exact = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, 9);
        let resources = ResourceContext::new(exact.resources);
        let label = try_build_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            None,
            &resources,
        )
        .expect("the retained row bytes should fit exactly")
        .expect("the label should remain visible");
        assert_eq!(
            label.into_parts(),
            (vec!["First".to_string(), "Line".to_string()], 5)
        );

        let below = options_with_limit(AsciiResourceLimitId::MaxOutputBytes, 8);
        let resources = ResourceContext::new(below.resources);
        let error = try_build_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            None,
            &resources,
        )
        .expect_err("one byte below the retained rows must fail before materialization");
        assert_limit_error(error, AsciiResourceLimitId::MaxOutputBytes, 9, 8);
    }

    #[test]
    fn escaped_mermaid_newline_keeps_the_literal_backslash_sequence() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(options.resources);
        let label = try_build_normalized_label_lines(
            r"literal\\nvalue\ntail",
            TerminalWidthProfile::Unicode,
            false,
            None,
            &resources,
        )
        .expect("escaped and authored line breaks should normalize")
        .expect("the label should remain visible");

        assert_eq!(
            label.into_parts(),
            (vec![r"literal\\nvalue".to_string(), "tail".to_string()], 15)
        );
    }

    #[test]
    fn authored_empty_rows_are_grid_admitted_before_row_allocation() {
        let raw = "<br><br><br>";
        let exact = options_with_limit(AsciiResourceLimitId::MaxGridCells, 4);
        let resources = ResourceContext::new(exact.resources);
        let label = try_build_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            None,
            &resources,
        )
        .expect("four empty rows should fit their minimum allocation extent")
        .expect("authored breaks should retain the label");
        assert_eq!(label.into_parts(), (vec![String::new(); 4], 0));

        let below = options_with_limit(AsciiResourceLimitId::MaxGridCells, 3);
        let mut resources = ResourceContext::new(below.resources);
        let materialized = Cell::new(false);
        let error = try_build_normalized_label_lines_with_probe(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            None,
            &mut resources,
            &materialized,
        )
        .expect_err("one grid cell below the minimum row extent must fail first");

        assert!(!materialized.get());
        assert_eq!(resources.layout_work_used(), 0);
        assert_eq!(resources.document_cells_used(), 0);
        assert_limit_error(error, AsciiResourceLimitId::MaxGridCells, 4, 3);
    }

    #[test]
    fn relation_label_trim_keeps_visible_controls_and_authored_breaks() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(options.resources);
        let label = try_build_normalized_label_lines(
            " \t\\nvalue\n ",
            TerminalWidthProfile::Unicode,
            true,
            None,
            &resources,
        )
        .expect("trimmed relation label should normalize")
        .expect("visible controls and authored breaks keep the label non-empty");
        let (lines, width) = label.into_parts();

        assert_eq!(lines, ["\\u{9}", "value"]);
        assert_eq!(width, 5);
    }

    #[test]
    fn label_plan_widths_match_materialized_wrapped_rows() {
        let cases = [
            ("alpha beta gamma", false, 8, TerminalWidthProfile::Unicode),
            ("alpha\n\nbeta", false, 8, TerminalWidthProfile::Unicode),
            (
                "alpha<br>beta\\ngamma",
                false,
                5,
                TerminalWidthProfile::Unicode,
            ),
            (
                "extraordinary word",
                false,
                4,
                TerminalWidthProfile::Unicode,
            ),
            ("中 文 👩‍💻 ·", false, 4, TerminalWidthProfile::Cjk),
            (" ́word", false, 6, TerminalWidthProfile::Unicode),
            ("\u{1b} value ", true, 7, TerminalWidthProfile::Unicode),
        ];

        for (raw, trim, wrap_width, profile) in cases {
            let options = AsciiRenderOptions::ascii()
                .with_resource_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_terminal_width_profile(profile);
            let resources = ResourceContext::new(options.resources);
            let plan =
                try_plan_normalized_label_lines(raw, profile, trim, Some(wrap_width), &resources)
                    .expect("label plan should be measurable")
                    .expect("test labels should remain visible");
            let mut planned_widths = Vec::new();
            plan.try_visit_line_widths(raw, &resources, |width| {
                planned_widths.push(width);
                Ok(())
            })
            .expect("planned row widths should be visitable");

            let materialized = plan.materialize(raw, &resources).unwrap_or_else(|error| {
                let normalized = normalize_terminal_text(raw);
                let direct = crate::text::wrap_display_lines_with_profile(
                    normalized.as_ref(),
                    wrap_width,
                    profile,
                );
                panic!(
                    "the admitted plan should materialize exactly for {raw:?}: {error}; planned={planned_widths:?}; direct={direct:?}"
                )
            });
            let (lines, width) = materialized.into_parts();
            let actual_widths = lines
                .iter()
                .map(|line| terminal_line_display_width(line, profile))
                .collect::<Vec<_>>();

            assert_eq!(planned_widths, actual_widths, "raw={raw:?}");
            assert_eq!(plan.metrics().line_count, lines.len(), "raw={raw:?}");
            assert_eq!(plan.metrics().max_width, width, "raw={raw:?}");
        }
    }

    #[test]
    fn wrapped_label_budget_uses_final_rows_after_whitespace_collapse() {
        let raw = "alpha          beta";
        let exact = AsciiRenderOptions::ascii()
            .with_resource_limit(AsciiResourceLimitId::MaxDocumentCells, 10)
            .unwrap();
        let resources = ResourceContext::new(exact.resources);
        let label = try_build_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            Some(32),
            &resources,
        )
        .expect("the final collapsed row should fit exactly")
        .expect("the label should remain visible");
        assert_eq!(label.into_parts(), (vec!["alpha beta".to_string()], 10));

        let below = AsciiRenderOptions::ascii()
            .with_resource_limit(AsciiResourceLimitId::MaxDocumentCells, 9)
            .unwrap();
        let resources = ResourceContext::new(below.resources);
        let error = try_build_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            Some(32),
            &resources,
        )
        .expect_err("one cell below the final row must fail before materialization");
        assert_limit_error(error, AsciiResourceLimitId::MaxDocumentCells, 10, 9);

        let exact = AsciiRenderOptions::ascii()
            .with_resource_limit(AsciiResourceLimitId::MaxOutputBytes, 10)
            .unwrap();
        let resources = ResourceContext::new(exact.resources);
        try_build_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            Some(32),
            &resources,
        )
        .expect("the final collapsed bytes should fit exactly")
        .expect("the label should remain visible");

        let below = AsciiRenderOptions::ascii()
            .with_resource_limit(AsciiResourceLimitId::MaxOutputBytes, 9)
            .unwrap();
        let resources = ResourceContext::new(below.resources);
        let error = try_build_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            Some(32),
            &resources,
        )
        .expect_err("one byte below the final collapsed output must fail");
        assert_limit_error(error, AsciiResourceLimitId::MaxOutputBytes, 10, 9);
    }

    #[test]
    fn wrapped_label_keeps_visible_escape_atoms_intact() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(options.resources);
        let lines = try_plan_normalized_label_lines(
            "\u{301}word",
            TerminalWidthProfile::Unicode,
            false,
            Some(6),
            &resources,
        )
        .expect("the escaped label should be measurable")
        .expect("the escaped label should remain visible")
        .materialize("\u{301}word", &resources)
        .expect("the escaped label should materialize atomically")
        .into_parts()
        .0;

        assert_eq!(lines, ["\\u{301}", "word"]);
    }

    #[test]
    fn wrapped_label_preserves_combining_marks_with_their_whitespace_base() {
        let raw = " \u{301}word";
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(options.resources);
        let lines = try_plan_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            Some(6),
            &resources,
        )
        .expect("the complete grapheme should be measurable")
        .expect("the complete grapheme should remain visible")
        .materialize(raw, &resources)
        .expect("wrapping must not detach the combining mark from its space base")
        .into_parts()
        .0;

        assert_eq!(lines, [raw]);
    }

    #[test]
    fn label_plan_accounts_each_scan_in_render_wide_work() {
        let raw = "alpha          beta gamma";
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(options.resources);
        let plan = try_plan_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            Some(10),
            &resources,
        )
        .expect("the work probe should be measurable")
        .expect("the work probe should remain visible");
        plan.try_visit_line_widths(raw, &resources, |_width| Ok(()))
            .expect("the retained-row scan should be charged");
        let before_materialization = resources.layout_work_used();
        let materialized = Cell::new(false);
        plan.materialize_with_probe(raw, &resources, &materialized)
            .expect("the materialization scan should be charged");
        assert!(materialized.get());
        let total_work = resources.layout_work_used();
        assert!(total_work > before_materialization);

        let exact = options_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, total_work);
        let exact_resources = ResourceContext::new(exact.resources);
        let exact_plan = try_plan_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            Some(10),
            &exact_resources,
        )
        .expect("the exact work plan should succeed")
        .expect("the exact work label should remain visible");
        exact_plan
            .try_visit_line_widths(raw, &exact_resources, |_width| Ok(()))
            .expect("the exact retained-row scan should succeed");
        exact_plan
            .materialize(raw, &exact_resources)
            .expect("the exact materialization scan should succeed");
        assert_eq!(exact_resources.layout_work_used(), total_work);

        let below = options_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, total_work - 1);
        let below_resources = ResourceContext::new(below.resources);
        let below_plan = try_plan_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            Some(10),
            &below_resources,
        )
        .expect("the below-limit plan should still fit before materialization")
        .expect("the below-limit label should remain visible");
        below_plan
            .try_visit_line_widths(raw, &below_resources, |_width| Ok(()))
            .expect("the below-limit retained-row scan should still fit");
        let materialized = Cell::new(false);
        let error = below_plan
            .materialize_with_probe(raw, &below_resources, &materialized)
            .expect_err("one work unit below the full scan cost must fail first");

        assert!(!materialized.get());
        assert_limit_error(
            error,
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            total_work,
            total_work - 1,
        );
    }

    #[test]
    fn label_plan_grid_admission_precedes_materialization() {
        let raw = "alpha beta";
        let below = options_with_limit(AsciiResourceLimitId::MaxGridCells, 9);
        let below_resources = ResourceContext::new(below.resources);
        let plan = try_plan_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            Some(5),
            &below_resources,
        )
        .expect("text planning should not consume a grid")
        .expect("the label should be visible");
        assert_eq!(plan.metrics().line_count, 2);
        assert_eq!(plan.metrics().max_width, 5);

        let materialized = Cell::new(false);
        let error = (|| {
            below_resources.grid_extent(plan.metrics().max_width, plan.metrics().line_count)?;
            plan.materialize_with_probe(raw, &below_resources, &materialized)
        })()
        .expect_err("a 5x2 label grid must reject a nine-cell limit");

        assert!(!materialized.get());
        assert_limit_error(error, AsciiResourceLimitId::MaxGridCells, 10, 9);

        let exact = options_with_limit(AsciiResourceLimitId::MaxGridCells, 10);
        let exact_resources = ResourceContext::new(exact.resources);
        let exact_plan = try_plan_normalized_label_lines(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            Some(5),
            &exact_resources,
        )
        .expect("exact text planning should succeed")
        .expect("the label should be visible");
        exact_resources
            .grid_extent(
                exact_plan.metrics().max_width,
                exact_plan.metrics().line_count,
            )
            .expect("the exact label grid should be admitted");
        let exact_materialized = Cell::new(false);
        exact_plan
            .materialize_with_probe(raw, &exact_resources, &exact_materialized)
            .expect("materialization should follow successful admission");
        assert!(exact_materialized.get());
    }

    #[test]
    fn label_break_policies_preserve_sequence_message_semantics() {
        let options = AsciiRenderOptions::ascii()
            .with_resource_profile(ResourceProfile::UnboundedForTrustedInput);
        let raw = "alpha\n\nbeta<br>gamma\\ndelta";

        let resources = ResourceContext::new(options.resources);
        let wrapped = try_plan_normalized_label_lines_with_policy(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            Some(80),
            LabelBreakPolicy::StructuralParagraphs,
            &resources,
        )
        .expect("structural message rows should be measurable")
        .expect("non-empty message rows should be retained")
        .materialize(raw, &resources)
        .expect("structural message rows should materialize")
        .into_parts()
        .0;
        assert_eq!(wrapped, ["alpha", "beta<br>gamma\\ndelta"]);

        let resources = ResourceContext::new(options.resources);
        let visible = try_plan_normalized_label_lines_with_policy(
            raw,
            TerminalWidthProfile::Unicode,
            false,
            None,
            LabelBreakPolicy::VisibleLine,
            &resources,
        )
        .expect("unwrapped message text should be measurable")
        .expect("non-empty message text should be retained")
        .materialize(raw, &resources)
        .expect("unwrapped message text should materialize")
        .into_parts()
        .0;
        assert_eq!(visible, ["alpha\\u{A}\\u{A}beta<br>gamma\\ndelta"]);
    }

    #[test]
    fn non_html_color_modes_keep_structured_text_plain() {
        for color_mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
        ] {
            let options = AsciiRenderOptions::ascii()
                .with_resource_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_color_mode(color_mode);

            assert_eq!(
                encode_text_lines(vec!["<safe>".to_string()], &options)
                    .expect("unbounded structured text should render"),
                "<safe>"
            );
        }
    }
}
