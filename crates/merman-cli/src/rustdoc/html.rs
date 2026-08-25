use super::config::SourceDisplay;
use std::fmt;

const RUSTDOC_THEME_CSS: &str = r#"<style>
.merman-rustdoc-diagram,
.merman-rustdoc-theme {
  contain: layout paint;
  isolation: isolate;
  max-width: 100%;
  overflow: auto;
  position: relative;
}
.merman-rustdoc-theme > svg {
  display: block;
  max-width: 100%;
}
.merman-rustdoc-theme-dark {
  display: none;
}
:root[data-theme="dark"] .merman-rustdoc-theme-light,
:root[data-theme="ayu"] .merman-rustdoc-theme-light {
  display: none;
}
:root[data-theme="dark"] .merman-rustdoc-theme-dark,
:root[data-theme="ayu"] .merman-rustdoc-theme-dark {
  display: block;
}
</style>"#;

pub(super) fn diagram_html_len(
    wrapper_id: &str,
    source: &str,
    light: &str,
    dark: &str,
    source_display: SourceDisplay,
) -> Option<usize> {
    let mut counter = ByteCounter(Some(0));
    write_diagram_html_to(
        &mut counter,
        wrapper_id,
        source,
        light,
        dark,
        source_display,
    )
    .expect("the byte counter cannot return a formatting error");
    counter.0
}

pub(super) fn write_diagram_html(
    output: &mut String,
    wrapper_id: &str,
    source: &str,
    light: &str,
    dark: &str,
    source_display: SourceDisplay,
) -> fmt::Result {
    write_diagram_html_to(output, wrapper_id, source, light, dark, source_display)
}

fn write_diagram_html_to(
    output: &mut impl fmt::Write,
    wrapper_id: &str,
    source: &str,
    light: &str,
    dark: &str,
    source_display: SourceDisplay,
) -> fmt::Result {
    write!(
        output,
        "{RUSTDOC_THEME_CSS}\n<div id=\"{wrapper_id}\" class=\"merman-rustdoc-diagram\" data-merman-rustdoc=\"true\">\n<div class=\"merman-rustdoc-theme merman-rustdoc-theme-light\" data-merman-rustdoc-theme=\"light\">\n{light}\n</div>\n<div class=\"merman-rustdoc-theme merman-rustdoc-theme-dark\" data-merman-rustdoc-theme=\"dark\">\n{dark}\n</div>\n</div>"
    )?;
    if source_display == SourceDisplay::Details {
        output.write_str(
            "\n<details class=\"merman-rustdoc-source\"><summary>Mermaid source</summary>\n<pre><code class=\"language-mermaid\">",
        )?;
        write_escaped_html(output, source)?;
        output.write_str("</code></pre>\n</details>")?;
    }
    Ok(())
}

fn write_escaped_html(output: &mut impl fmt::Write, input: &str) -> fmt::Result {
    for character in input.chars() {
        match character {
            '&' => output.write_str("&amp;")?,
            '<' => output.write_str("&lt;")?,
            '>' => output.write_str("&gt;")?,
            '"' => output.write_str("&quot;")?,
            '\'' => output.write_str("&#39;")?,
            _ => output.write_char(character)?,
        }
    }
    Ok(())
}

struct ByteCounter(Option<usize>);

impl fmt::Write for ByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0 = self.0.and_then(|length| length.checked_add(value.len()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_is_static_dual_theme_html_and_escapes_source_details() {
        let mut html = String::new();
        write_diagram_html(
            &mut html,
            "diagram-wrapper",
            "flowchart TD\nA[<Start & Go>] --> B",
            "<svg id=\"light\"/>",
            "<svg id=\"dark\"/>",
            SourceDisplay::Details,
        )
        .unwrap();

        assert!(html.contains(r#"id="diagram-wrapper""#));
        assert!(html.contains(r#"data-merman-rustdoc-theme="light""#));
        assert!(html.contains(r#"data-merman-rustdoc-theme="dark""#));
        assert!(html.contains(r#":root[data-theme="ayu"]"#));
        assert!(html.contains("contain: layout paint"));
        assert!(html.contains("isolation: isolate"));
        assert!(html.contains("overflow: auto"));
        assert!(html.contains("A[&lt;Start &amp; Go&gt;]"));
        assert!(!html.contains("<script"));
        assert_eq!(
            diagram_html_len(
                "diagram-wrapper",
                "flowchart TD\nA[<Start & Go>] --> B",
                "<svg id=\"light\"/>",
                "<svg id=\"dark\"/>",
                SourceDisplay::Details,
            ),
            Some(html.len())
        );
    }
}
