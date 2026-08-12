use crate::Result;
use crate::color::AsciiColorMode;
use crate::resource::AsciiResourceLimitId;
use crate::resource::{AsciiResourcePolicy, CheckedOutput};

pub(super) fn encode_budgeted_lines_with_expected(
    lines: Vec<String>,
    color_mode: AsciiColorMode,
    policy: AsciiResourcePolicy,
    expected_len: usize,
    before_materialize: impl FnOnce(),
) -> Result<String> {
    let encoded_len = encoded_lines_len(&lines, color_mode, policy)?;
    policy.check(AsciiResourceLimitId::MaxOutputBytes, encoded_len)?;
    if expected_len != encoded_len {
        return Err(crate::error::AsciiError::UnsupportedFeature {
            diagram_type: "structured_text",
            feature: "encoded output byte accounting",
        });
    }
    before_materialize();

    let mut output = CheckedOutput::new(policy);
    for (index, line) in lines.into_iter().enumerate() {
        if index > 0 {
            output.push_char('\n')?;
        }
        if color_mode == AsciiColorMode::Html {
            push_html_escaped_text(&mut output, &line)?;
        } else {
            output.push_str(&line)?;
        }
    }
    Ok(output.finish())
}

fn encoded_lines_len(
    lines: &[String],
    color_mode: AsciiColorMode,
    policy: AsciiResourcePolicy,
) -> Result<usize> {
    let separators = lines.len().saturating_sub(1);
    lines.iter().try_fold(separators, |encoded_len, line| {
        encoded_len
            .checked_add(encoded_text_len(line, color_mode, policy)?)
            .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxOutputBytes))
    })
}

pub(super) fn encoded_text_len(
    value: &str,
    color_mode: AsciiColorMode,
    policy: AsciiResourcePolicy,
) -> Result<usize> {
    if color_mode != AsciiColorMode::Html {
        return Ok(value.len());
    }

    value.chars().try_fold(0usize, |encoded_len, ch| {
        let char_len = html_escape(ch).map_or_else(|| ch.len_utf8(), str::len);
        encoded_len
            .checked_add(char_len)
            .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxOutputBytes))
    })
}

pub(crate) fn push_html_escaped_text(output: &mut CheckedOutput, value: &str) -> Result<()> {
    // HTML escaping classifies syntax scalars without participating in terminal layout.
    for ch in value.chars() {
        if let Some(escaped) = html_escape(ch) {
            output.push_str(escaped)?;
        } else {
            output.push_char(ch)?;
        }
    }
    Ok(())
}

fn html_escape(ch: char) -> Option<&'static str> {
    match ch {
        '&' => Some("&amp;"),
        '<' => Some("&lt;"),
        '>' => Some("&gt;"),
        '"' => Some("&quot;"),
        '\'' => Some("&#39;"),
        _ => None,
    }
}
