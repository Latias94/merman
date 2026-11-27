//! Comment stripping for Mermaid diagram detection.

use merman_model::text::is_ecmascript_whitespace;
use merman_model::{OperationControl, OperationControlResult};
use std::borrow::Cow;

use crate::scan::controlled::{
    ControlledScanCheckpoints, find_ascii_pattern_controlled, find_newline_controlled,
    push_str_controlled,
};

/// Applies Mermaid 12.4's `stripAnyComments` to detector input.
///
/// A match replaces `%%` through the next LF, along with preceding ECMAScript whitespace,
/// with one LF. An unterminated suffix is preserved verbatim.
pub fn strip_detection_comments_controlled<'a>(
    text: &'a str,
    control: &OperationControl,
) -> OperationControlResult<Cow<'a, str>> {
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut consumed = 0;
    let mut output = None;

    while let Some(marker) = find_ascii_pattern_controlled(text, consumed, b"%%", &mut checkpoints)?
    {
        // The ASCII marker and LF searches return UTF-8 boundaries. Only LF ends a match.
        let Some(newline) = find_newline_controlled(text, marker + 2, &mut checkpoints)? else {
            break;
        };

        // The previous match's end bounds this reverse scan, including for adjacent comments.
        let mut start = marker;
        for (offset, ch) in text[consumed..marker].char_indices().rev() {
            checkpoints.scanned(ch.len_utf8())?;
            if !is_ecmascript_whitespace(ch) {
                break;
            }
            start = consumed + offset;
        }

        let out =
            output.get_or_insert_with(|| String::with_capacity(text.len() - (newline - start)));
        push_str_controlled(out, &text[consumed..start], &mut checkpoints)?;
        out.push('\n');
        consumed = newline + 1;
    }

    let result = if let Some(mut out) = output {
        push_str_controlled(&mut out, &text[consumed..], &mut checkpoints)?;
        Cow::Owned(out)
    } else {
        Cow::Borrowed(text)
    };
    checkpoints.finish()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::controlled::CONTROLLED_SCAN_CHECKPOINT_BYTES;

    fn strip(text: &str) -> Cow<'_, str> {
        strip_detection_comments_controlled(text, &OperationControl::new()).unwrap()
    }

    #[test]
    fn replaces_adjacent_and_inline_comments_with_one_lf_each() {
        for (input, expected) in [
            ("%%\n%% comment\nflowchart LR", "\n\nflowchart LR"),
            ("A\n\n \t%% first\n \n %% second\n B%%\nC", "A\n\n B\nC"),
            ("x%%% comment\n", "x\n"),
            ("%%{comment}\nA", "\nA"),
        ] {
            assert_eq!(strip(input).as_ref(), expected, "{input:?}");
        }
    }

    #[test]
    fn consumes_ecmascript_whitespace_without_splitting_utf8() {
        let input = " \n\t\u{feff}\u{00a0}\u{3000}%% comment\nλ\u{0085}\u{2028} %% 🙂\nΩ";
        assert_eq!(strip(input).as_ref(), "\nλ\u{0085}\nΩ");
    }

    #[test]
    fn uses_only_lf_to_end_a_comment() {
        for (input, expected) in [
            ("A%% first\rdiagram\r\nB", "A\nB"),
            ("A %% first\u{2028}x\u{2029}y\nB\r\nC", "A\nB\r\nC"),
            (
                "%% gone\n   \n%% stays\r\u{2028}\u{2029}",
                "\n   \n%% stays\r\u{2028}\u{2029}",
            ),
        ] {
            assert_eq!(strip(input).as_ref(), expected, "{input:?}");
        }
    }

    #[test]
    fn borrows_unchanged_text_including_unterminated_comments() {
        for input in [
            "",
            " \t\n\u{feff}A\r\n",
            "A % B\n",
            " \n\u{00a0}%%tail",
            "%%tail\rbody",
            "%%tail\u{2028}x\u{2029}y",
            "%%",
        ] {
            assert!(matches!(strip(input), Cow::Borrowed(text) if text == input));
        }
    }

    #[test]
    fn observes_cancellation_during_scan_rewind_copy_and_finish() {
        let chunk = CONTROLLED_SCAN_CHECKPOINT_BYTES;
        for (stage, input, successful) in [
            ("entry", String::new(), 0),
            ("marker search", " ".repeat(chunk + 1), 1),
            (
                "comment body",
                format!("%% done\n%%{}\n", "x".repeat(chunk)),
                1,
            ),
            ("whitespace rewind", format!("{}%%\n", " ".repeat(chunk)), 2),
            (
                "retained suffix copy",
                format!("%%\n{}", "x".repeat(chunk)),
                2,
            ),
            ("finish", "%%\n".to_owned(), 1),
        ] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(successful);
            assert!(
                strip_detection_comments_controlled(&input, &control).is_err(),
                "must observe cancellation during {stage}"
            );
        }
    }

    #[test]
    fn scans_adversarial_inputs_with_a_linear_checkpoint_budget() {
        for count in [4_000, 16_000] {
            for input in ["\n    ".repeat(count), format!("%%{}", "x%%".repeat(count))] {
                let control = OperationControl::new();
                // These no-match inputs need only a forward search, plus entry and finish.
                control
                    .cancel_after_checkpoints(2 + input.len() / CONTROLLED_SCAN_CHECKPOINT_BYTES);
                let result = strip_detection_comments_controlled(&input, &control).unwrap();
                assert!(matches!(result, Cow::Borrowed(text) if text == input.as_str()));
            }
        }
    }
}
