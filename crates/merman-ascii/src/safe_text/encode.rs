use crate::Result;
use crate::color::AsciiColorMode;
use crate::resource::{AsciiResourcePolicy, CheckedOutput};

pub(super) fn encode_budgeted_lines(
    lines: Vec<String>,
    color_mode: AsciiColorMode,
    policy: AsciiResourcePolicy,
) -> Result<String> {
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

pub(crate) fn push_html_escaped_text(output: &mut CheckedOutput, value: &str) -> Result<()> {
    // HTML escaping classifies syntax scalars without participating in terminal layout.
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;")?,
            '<' => output.push_str("&lt;")?,
            '>' => output.push_str("&gt;")?,
            '"' => output.push_str("&quot;")?,
            '\'' => output.push_str("&#39;")?,
            _ => output.push_char(ch)?,
        }
    }
    Ok(())
}
