//! Text wrapping helpers used across diagrams.
//!
//! This module intentionally mirrors Mermaid behavior (including quirks) for parity.

use super::{
    DeterministicTextMeasurer, TextMeasurer, TextStyle, split_html_br_lines,
    trim_end_html_collapsible_ascii_whitespace,
};
use unicode_segmentation::UnicodeSegmentation;

#[cfg(test)]
thread_local! {
    // Explicit candidate content writes; excludes allocator moves and measurement callbacks.
    static CANDIDATE_WRITTEN_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn record_candidate_write(bytes: usize) {
    CANDIDATE_WRITTEN_BYTES.with(|written| written.set(written.get() + bytes));
}

pub fn ceil_to_1_64_px(v: f64) -> f64 {
    if !(v.is_finite() && v >= 0.0) {
        return 0.0;
    }
    // Avoid "ceil to next 1/64" due to tiny FP drift (e.g. `...0000000002` over the exact
    // lattice). Upstream Mermaid fixtures frequently land exactly on the 1/64px grid.
    let x = v * 64.0;
    let r = x.round();
    if (x - r).abs() < 1e-4 {
        return r / 64.0;
    }
    ((x) - 1e-5).ceil() / 64.0
}

pub fn round_to_1_64_px(v: f64) -> f64 {
    if !(v.is_finite() && v >= 0.0) {
        return 0.0;
    }
    let x = v * 64.0;
    let r = (x + 0.5).floor();
    r / 64.0
}

pub fn round_to_1_64_px_ties_to_even(v: f64) -> f64 {
    if !(v.is_finite() && v >= 0.0) {
        return 0.0;
    }
    let x = v * 64.0;
    let f = x.floor();
    let frac = x - f;
    let i = if frac < 0.5 {
        f
    } else if frac > 0.5 {
        f + 1.0
    } else {
        let fi = f as i64;
        if fi % 2 == 0 { f } else { f + 1.0 }
    };
    let out = i / 64.0;
    if out == -0.0 { 0.0 } else { out }
}

#[cfg(test)]
#[derive(Debug, Default)]
pub(super) struct GraphemeSplitStats {
    pub(super) grapheme_visits: usize,
    pub(super) partitioned_bytes: usize,
}

pub(super) struct GraphemeLineVisit<'a> {
    pub(super) tail: &'a str,
    #[cfg(test)]
    pub(super) stats: GraphemeSplitStats,
}

/// Visits every grapheme once, measuring complete candidates within the current line.
/// Returning false from `emit_line` stops at the first completed line without probing its tail.
/// The width callback receives true when its accumulation must start a new line.
pub(super) fn visit_grapheme_lines_to_width<'a, E>(
    token: &'a str,
    max_width_px: f64,
    mut measure_candidate: impl FnMut(&str, &str, bool) -> Result<f64, E>,
    mut emit_line: impl FnMut(&'a str) -> bool,
) -> Result<GraphemeLineVisit<'a>, E> {
    let mut line_start = 0usize;
    let mut width_px = 0.0;
    let mut has_positive_advance = false;
    #[cfg(test)]
    let mut stats = GraphemeSplitStats::default();

    for (index, grapheme) in token.grapheme_indices(true) {
        #[cfg(test)]
        {
            stats.grapheme_visits = stats.grapheme_visits.saturating_add(1);
        }
        let candidate_end = index + grapheme.len();
        let candidate_width_px = measure_candidate(
            &token[line_start..candidate_end],
            grapheme,
            index == line_start,
        )?;
        if has_positive_advance
            && candidate_width_px > width_px
            && candidate_width_px > max_width_px
        {
            let completed = &token[line_start..index];
            #[cfg(test)]
            {
                stats.partitioned_bytes = stats.partitioned_bytes.saturating_add(completed.len());
            }
            if !emit_line(completed) {
                return Ok(GraphemeLineVisit {
                    tail: &token[index..],
                    #[cfg(test)]
                    stats,
                });
            }
            line_start = index;
            width_px = measure_candidate(grapheme, grapheme, true)?;
            has_positive_advance = width_px > 0.0;
        } else {
            width_px = candidate_width_px;
            has_positive_advance |= candidate_width_px > 0.0;
        }
    }
    let tail = &token[line_start..];
    #[cfg(test)]
    {
        stats.partitioned_bytes = stats.partitioned_bytes.saturating_add(tail.len());
    }
    Ok(GraphemeLineVisit {
        tail,
        #[cfg(test)]
        stats,
    })
}

/// Splits at a grapheme boundary using complete candidates and their newly appended graphemes.
#[cfg(test)]
fn split_graphemes_to_width<E>(
    token: &str,
    max_width_px: f64,
    mut measure_candidate: impl FnMut(&str, &str) -> Result<f64, E>,
) -> Result<(&str, &str), E> {
    let mut first_line = None;
    let visit = visit_grapheme_lines_to_width(
        token,
        max_width_px,
        |candidate, grapheme, _| measure_candidate(candidate, grapheme),
        |line| {
            first_line = Some(line);
            false
        },
    )?;
    Ok(match first_line {
        Some(line) => (line, visit.tail),
        None => (visit.tail, ""),
    })
}

/// Wraps SVG-like text into lines using the provided [`TextMeasurer`] for width decisions.
///
/// This mirrors Mermaid's `wrapLabel(...)` behavior at a high level (greedy word wrapping), but
/// delegates width measurements to the active measurer so the operation-selected font backend and
/// SVG measurement phase determine wrapping breakpoints.
pub fn wrap_text_lines_measurer(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width_px: Option<f64>,
) -> Vec<String> {
    fn wrap_line(
        line: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        max_width_px: f64,
    ) -> Vec<String> {
        use std::collections::VecDeque;

        if !max_width_px.is_finite() || max_width_px <= 0.0 {
            return vec![line.to_string()];
        }

        let mut tokens = VecDeque::from(DeterministicTextMeasurer::split_line_to_words(line));
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut trimmed_end = 0;

        while let Some(tok) = tokens.pop_front() {
            if cur.is_empty() && tok == " " {
                continue;
            }

            let previous_len = cur.len();
            // Inspect only the appended token, so retained spaces are not rescanned per probe.
            let token_end = trim_end_html_collapsible_ascii_whitespace(&tok).len();
            let candidate_end = if token_end == 0 {
                trimmed_end
            } else {
                previous_len + token_end
            };
            cur.push_str(&tok);
            #[cfg(test)]
            record_candidate_write(tok.len());
            if measurer.measure(&cur[..candidate_end], style).width <= max_width_px {
                trimmed_end = candidate_end;
                continue;
            }
            cur.truncate(previous_len);

            if trimmed_end != 0 {
                out.push(cur[..trimmed_end].to_string());
                cur.clear();
                trimmed_end = 0;
                tokens.push_front(tok);
                continue;
            }

            if tok == " " {
                continue;
            }

            let visit = match visit_grapheme_lines_to_width(
                &tok,
                max_width_px,
                |candidate, _, _| {
                    Ok::<_, std::convert::Infallible>(measurer.measure(candidate, style).width)
                },
                |line| {
                    out.push(line.to_string());
                    true
                },
            ) {
                Ok(visit) => visit,
                Err(error) => match error {},
            };
            if !visit.tail.is_empty() {
                if measurer.measure(visit.tail, style).width > max_width_px {
                    out.push(visit.tail.to_string());
                } else {
                    cur.clear();
                    cur.push_str(visit.tail);
                    trimmed_end = trim_end_html_collapsible_ascii_whitespace(visit.tail).len();
                    #[cfg(test)]
                    record_candidate_write(visit.tail.len());
                }
            }
        }

        if trimmed_end != 0 {
            out.push(cur[..trimmed_end].to_string());
        }

        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
    }

    let mut out: Vec<String> = Vec::new();
    for line in split_html_br_lines(text) {
        if let Some(w) = max_width_px {
            out.extend(wrap_line(line, measurer, style, w));
        } else {
            out.push(line.to_string());
        }
    }
    if out.is_empty() {
        vec!["".to_string()]
    } else {
        out
    }
}

fn try_wrap_label_like_mermaid_lines<E>(
    label: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width_px: f64,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<Vec<String>, E> {
    checkpoint()?;
    if label.is_empty() {
        return Ok(Vec::new());
    }
    if !max_width_px.is_finite() || max_width_px <= 0.0 {
        return Ok(vec![label.to_string()]);
    }

    // Mermaid short-circuits wrapping if the label already contains `<br>` breaks.
    let explicit_lines = super::split_mermaid_common_br_lines(label);
    if explicit_lines.len() > 1 {
        let mut lines = Vec::with_capacity(explicit_lines.len());
        for line in explicit_lines {
            checkpoint()?;
            lines.push(line.to_string());
        }
        checkpoint()?;
        return Ok(lines);
    }
    fn w_px<E>(
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        s: &str,
        checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
    ) -> std::result::Result<f64, E> {
        // Upstream uses `calculateTextWidth(...)` which rounds the SVG bbox width.
        checkpoint()?;
        let width = measurer
            .measure_svg_simple_text_bbox_width_for_wrap_px(s, style)
            .round();
        checkpoint()?;
        Ok(width)
    }

    fn break_string_like_mermaid<E>(
        word: &str,
        max_width_px: f64,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
    ) -> std::result::Result<(Vec<String>, String), E> {
        let chars: Vec<char> = word.chars().collect();
        let mut lines: Vec<String> = Vec::new();
        let mut current = String::new();
        for (idx, ch) in chars.iter().enumerate() {
            checkpoint()?;
            current.push(*ch);
            #[cfg(test)]
            record_candidate_write(ch.len_utf8());
            let line_w = w_px(measurer, style, &current, checkpoint)?;
            if line_w >= max_width_px {
                let is_last = idx + 1 == chars.len();
                if !is_last {
                    current.push('-');
                    #[cfg(test)]
                    record_candidate_write(1);
                }
                lines.push(std::mem::take(&mut current));
            }
        }
        Ok((lines, current))
    }

    // Mermaid splits on ASCII spaces and drops empty chunks (collapsing multiple spaces).
    let words: Vec<&str> = label.split(' ').filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        return Ok(vec![label.to_string()]);
    }

    let mut completed: Vec<String> = Vec::new();
    let mut next_line = String::new();
    for (idx, word) in words.iter().enumerate() {
        checkpoint()?;
        let word_len = w_px(measurer, style, &format!("{word} "), checkpoint)?;
        let next_len = w_px(measurer, style, &next_line, checkpoint)?;
        if word_len > max_width_px {
            let (hyphenated, remaining) =
                break_string_like_mermaid(word, max_width_px, measurer, style, checkpoint)?;
            completed.push(next_line.clone());
            completed.extend(hyphenated);
            next_line = remaining;
        } else if next_len + word_len >= max_width_px {
            completed.push(next_line.clone());
            next_line = (*word).to_string();
        } else if next_line.is_empty() {
            next_line = (*word).to_string();
        } else {
            next_line.push(' ');
            next_line.push_str(word);
        }

        let is_last = idx + 1 == words.len();
        if is_last {
            completed.push(next_line.clone());
        }
    }

    checkpoint()?;
    Ok(completed.into_iter().filter(|l| !l.is_empty()).collect())
}

/// Wraps a label using Mermaid's `wrapLabel(...)` logic, producing wrapped *lines*.
///
/// This is used by Sequence diagrams (Mermaid@11.x) when `wrap: true` is enabled and when actor
/// descriptions are marked `wrap: true` by the DB layer.
pub fn wrap_label_like_mermaid_lines(
    label: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width_px: f64,
) -> Vec<String> {
    let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
    match try_wrap_label_like_mermaid_lines(label, measurer, style, max_width_px, &mut checkpoint) {
        Ok(lines) => lines,
        Err(error) => match error {},
    }
}

pub(crate) fn wrap_label_like_mermaid_lines_with_checkpoint(
    label: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width_px: f64,
    mut checkpoint: impl FnMut() -> crate::Result<()>,
) -> crate::Result<Vec<String>> {
    try_wrap_label_like_mermaid_lines(label, measurer, style, max_width_px, &mut checkpoint)
}

#[cfg(test)]
mod tests {
    use super::{
        split_graphemes_to_width, visit_grapheme_lines_to_width, wrap_text_lines_measurer,
    };
    use crate::text::{
        DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode,
        try_measure_wrapped_text,
    };
    use std::cell::{Cell, RefCell};

    struct WidthProbe<F>(F);

    impl<F: Fn(&str) -> f64> TextMeasurer for WidthProbe<F> {
        fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: (self.0)(text),
                height: 1.0,
                line_count: 1,
            }
        }
    }

    #[test]
    fn emitted_candidate_construction_writes_scale_with_input_bytes() {
        let measurer = WidthProbe(|_: &str| 0.0);
        for count in [64, 256, 1024] {
            for text in [
                format!("{}z", "a ".repeat(count)),
                format!("a{}b", " ".repeat(count)),
            ] {
                super::CANDIDATE_WRITTEN_BYTES.with(|written| written.set(0));
                let lines =
                    wrap_text_lines_measurer(&text, &measurer, &TextStyle::default(), Some(1.0));
                let written = super::CANDIDATE_WRITTEN_BYTES.with(Cell::get);
                assert_eq!(lines, [text.as_str()]);
                assert!(
                    written <= 4 * text.len(),
                    "input_bytes={}, candidate_written_bytes={written}",
                    text.len(),
                );
            }
        }
    }

    #[test]
    fn sequence_candidate_construction_writes_scale_with_input_bytes() {
        let measurer = WidthProbe(|text: &str| text.len() as f64);
        for count in [64, 256, 1024] {
            let text = "a".repeat(count);
            super::CANDIDATE_WRITTEN_BYTES.with(|written| written.set(0));
            let lines = super::wrap_label_like_mermaid_lines(
                &text,
                &measurer,
                &TextStyle::default(),
                (count / 2) as f64,
            );
            let written = super::CANDIDATE_WRITTEN_BYTES.with(Cell::get);
            assert_eq!(
                lines,
                [format!("{}-", "a".repeat(count / 2)), "a".repeat(count / 2)]
            );
            assert!(
                written <= 4 * text.len(),
                "input_bytes={}, candidate_written_bytes={written}",
                text.len(),
            );
        }
    }

    #[test]
    fn emitted_candidates_preserve_complete_requests_trimming_and_retry_order() {
        let probes = RefCell::new(Vec::new());
        let measurer = WidthProbe(|text: &str| {
            probes.borrow_mut().push(text.to_string());
            match text {
                "a" => 3.0,
                "a  b" | "c" => 1.0,
                _ => 4.0,
            }
        });
        let lines = wrap_text_lines_measurer("a  b c", &measurer, &TextStyle::default(), Some(3.0));
        assert_eq!(lines, ["a  b", "c"]);
        assert_eq!(
            *probes.borrow(),
            ["a", "a", "a", "a  b", "a  b", "a  b c", "c"],
        );

        probes.borrow_mut().clear();
        let measurer = WidthProbe(|text: &str| {
            probes.borrow_mut().push(text.to_string());
            0.0
        });
        let text = "\t a\u{00a0}\t \u{000b}b\u{000c} c";
        assert_eq!(
            wrap_text_lines_measurer(text, &measurer, &TextStyle::default(), Some(1.0)),
            [text],
        );
        assert_eq!(
            *probes.borrow(),
            [
                "",
                "",
                "\t a\u{00a0}",
                "\t a\u{00a0}",
                "\t a\u{00a0}\t \u{000b}b",
                "\t a\u{00a0}\t \u{000b}b",
                text,
            ],
        );
    }

    #[test]
    fn sequence_candidates_preserve_callback_and_checkpoint_error_order() {
        let expected = [
            "check",
            "check",
            "check",
            "measure:abcd ",
            "check",
            "check",
            "measure:",
            "check",
            "check",
            "check",
            "measure:a",
            "check",
            "check",
            "check",
            "measure:ab",
            "check",
            "check",
            "check",
            "measure:abc",
            "check",
            "check",
            "check",
            "measure:d",
            "check",
            "check",
            "check",
            "measure:x ",
            "check",
            "check",
            "measure:d",
            "check",
            "check",
        ];
        let checkpoints = expected.iter().filter(|event| **event == "check").count();
        for fail_at in 0..=checkpoints {
            let events = RefCell::new(Vec::new());
            let checks = Cell::new(0);
            let measurer = WidthProbe(|text: &str| {
                events.borrow_mut().push(format!("measure:{text}"));
                match text {
                    "abcd " => 5.0,
                    "a" | "x " => 2.0,
                    "ab" | "d" => 1.0,
                    "abc" => 3.0,
                    "" => 0.0,
                    _ => panic!("unexpected candidate {text:?}"),
                }
            });
            let result = super::try_wrap_label_like_mermaid_lines(
                "abcd x",
                &measurer,
                &TextStyle::default(),
                3.0,
                &mut || {
                    events.borrow_mut().push("check".to_string());
                    checks.set(checks.get() + 1);
                    if checks.get() == fail_at {
                        Err("canceled")
                    } else {
                        Ok(())
                    }
                },
            );
            if fail_at == 0 {
                assert_eq!(result.unwrap(), ["abc-", "d", "x"]);
                assert_eq!(*events.borrow(), expected);
            } else {
                assert_eq!(result.unwrap_err(), "canceled");
                let end = expected
                    .iter()
                    .enumerate()
                    .filter(|(_, event)| **event == "check")
                    .nth(fail_at - 1)
                    .unwrap()
                    .0
                    + 1;
                assert_eq!(*events.borrow(), expected[..end], "fail_at={fail_at}");
            }
        }
    }

    #[test]
    fn grapheme_splitting_keeps_non_increasing_overwide_candidates() {
        let mut widths = [0.0, 8.0, 8.0, 7.0, 9.0].into_iter();
        let parts = split_graphemes_to_width("\u{0301}Wabc", 1.0, |_, _| {
            Ok::<_, ()>(widths.next().unwrap())
        })
        .unwrap();
        assert_eq!(parts, ("\u{0301}Wab", "c"));
    }

    #[test]
    fn grapheme_splitting_stops_on_measurement_errors() {
        let mut probes = Vec::new();
        let result =
            split_graphemes_to_width("\u{0301}👩‍🔬WZ", 1.0, |candidate, grapheme| {
                probes.push((candidate.to_string(), grapheme.to_string()));
                match grapheme {
                    "\u{0301}" => Ok(0.0),
                    "👩‍🔬" => Ok(8.0),
                    _ => Err("canceled"),
                }
            });
        assert_eq!(result, Err("canceled"));
        assert_eq!(
            probes,
            [
                ("\u{0301}".to_string(), "\u{0301}".to_string()),
                ("\u{0301}👩‍🔬".to_string(), "👩‍🔬".to_string()),
                ("\u{0301}👩‍🔬W".to_string(), "W".to_string()),
            ],
        );
    }

    #[test]
    fn streamed_grapheme_lines_stop_after_a_later_measurement_error() {
        let mut completed = Vec::new();
        let mut probes = Vec::new();
        let result = visit_grapheme_lines_to_width(
            "abcd",
            1.0,
            |candidate, grapheme, _| {
                probes.push(candidate.to_string());
                if grapheme == "c" {
                    Err("canceled")
                } else {
                    Ok(candidate.len() as f64)
                }
            },
            |line| {
                completed.push(line.to_string());
                true
            },
        );
        assert!(matches!(result, Err("canceled")));
        assert_eq!(completed, ["a"]);
        assert_eq!(probes, ["a", "ab", "b", "bc"]);
    }

    #[test]
    fn emitted_long_tokens_match_shared_grapheme_wrapping() {
        let measurer = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };
        for text in [
            "W\u{0301}W",
            "\u{0301}WW",
            "👩‍🔬👨‍🔬",
            "👍🏽👍🏻",
            "🇨🇳🇺🇸",
            "1️⃣2️⃣",
            "a\u{200b}b",
        ] {
            for limit in [1.0, 8.0, 16.0, 25.0] {
                let expected = try_measure_wrapped_text(
                    text,
                    &style,
                    Some(limit),
                    WrapMode::SvgLike,
                    19.0,
                    |candidate, style| Ok::<_, ()>(measurer.measure(candidate, style).width),
                )
                .unwrap();
                let emitted = wrap_text_lines_measurer(text, &measurer, &style, Some(limit));
                assert_eq!(emitted, expected.lines, "text={text:?}, limit={limit}");
                assert_eq!(emitted.concat(), text);
            }
        }
    }

    #[test]
    fn emitted_wrapping_keeps_leading_zero_width_text_with_overwide_grapheme() {
        let measurer = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };
        assert_eq!(
            wrap_text_lines_measurer("\u{0301}WW", &measurer, &style, Some(1.0)),
            ["\u{0301}W", "W"],
        );
        assert_eq!(
            wrap_text_lines_measurer("👩‍🔬👨‍🔬", &measurer, &style, Some(1.0)),
            ["👩‍🔬", "👨‍🔬"],
        );
    }
}
