use crate::options::SourceMode;

pub(crate) fn diagram_html(source: &str, svg: &str, source_mode: SourceMode) -> String {
    let source_details = match source_mode {
        SourceMode::Hide => String::new(),
        SourceMode::Details => format!(
            "\n<details class=\"merman-rustdoc-source\"><summary>Mermaid source</summary>\n<pre><code class=\"language-mermaid\">{}</code></pre>\n</details>",
            escape_html(source)
        ),
    };
    format!(
        "\n<div class=\"merman-rustdoc-diagram\" data-merman-rustdoc=\"true\">\n{svg}\n</div>{source_details}\n"
    )
}

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
            "<svg></svg>",
            SourceMode::Details,
        );

        assert!(html.contains(r#"class="merman-rustdoc-source""#));
        assert!(html.contains("A[&lt;Start &amp; Go&gt;]"));
    }
}
