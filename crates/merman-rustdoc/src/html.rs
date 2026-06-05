use crate::options::SourceMode;
use crate::render::RenderedDiagram;

pub(crate) fn diagram_html(
    source: &str,
    diagram: &RenderedDiagram,
    source_mode: SourceMode,
) -> String {
    let source_details = match source_mode {
        SourceMode::Hide => String::new(),
        SourceMode::Details => format!(
            "\n<details class=\"merman-rustdoc-source\"><summary>Mermaid source</summary>\n<pre><code class=\"language-mermaid\">{}</code></pre>\n</details>",
            escape_html(source)
        ),
    };
    let diagram_body = match diagram {
        RenderedDiagram::Single(svg) => svg.to_string(),
        RenderedDiagram::RustdocTheme { light, dark } => format!(
            "{RUSTDOC_THEME_CSS}\n<div class=\"merman-rustdoc-theme merman-rustdoc-theme-light\" data-merman-rustdoc-theme=\"light\">\n{light}\n</div>\n<div class=\"merman-rustdoc-theme merman-rustdoc-theme-dark\" data-merman-rustdoc-theme=\"dark\">\n{dark}\n</div>"
        ),
    };
    format!(
        "\n<div class=\"merman-rustdoc-diagram\" data-merman-rustdoc=\"true\">\n{diagram_body}\n</div>{source_details}\n"
    )
}

const RUSTDOC_THEME_CSS: &str = r#"<style>
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

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_details_escape_mermaid_source() {
        let html = diagram_html(
            "flowchart TD\nA[<Start & Go>] --> B[Done]",
            &RenderedDiagram::Single("<svg></svg>".to_string()),
            SourceMode::Details,
        );

        assert!(html.contains(r#"class="merman-rustdoc-source""#));
        assert!(html.contains("A[&lt;Start &amp; Go&gt;]"));
    }

    #[test]
    fn rustdoc_theme_diagram_includes_light_dark_switching_markup() {
        let html = diagram_html(
            "flowchart TD\nA --> B",
            &RenderedDiagram::RustdocTheme {
                light: r#"<svg id="light"></svg>"#.to_string(),
                dark: r#"<svg id="dark"></svg>"#.to_string(),
            },
            SourceMode::Hide,
        );

        assert!(html.contains(r#"data-merman-rustdoc-theme="light""#));
        assert!(html.contains(r#"data-merman-rustdoc-theme="dark""#));
        assert!(html.contains(r#":root[data-theme="dark"]"#));
        assert!(html.contains(r#":root[data-theme="ayu"]"#));
    }
}
