//! Shared upstream SVG baseline policy.

fn normalized_fixture_stem(name_or_stem: &str) -> &str {
    name_or_stem
        .strip_suffix(".mmd")
        .or_else(|| name_or_stem.strip_suffix(".svg"))
        .unwrap_or(name_or_stem)
}

pub(crate) fn upstream_svg_baseline_skip_reason(
    diagram: &str,
    fixture_name_or_stem: &str,
) -> Option<&'static str> {
    let stem = normalized_fixture_stem(fixture_name_or_stem);

    if diagram == "sequence" && stem == "stress_end_keyword_016" {
        return Some("upstream Mermaid 11.15 rejects `(end)` as a participant id");
    }

    if diagram == "flowchart" {
        if stem == "upstream_flow_text_ellipse_vertex_parser_only_spec" {
            return Some(
                "upstream Mermaid 11.15 cannot render this parser-only ellipse vertex fixture",
            );
        }

        if matches!(
            stem,
            "upstream_html_demos_flowchart_flowchart_040_parser_only_katex"
                | "upstream_html_demos_flowchart_flowchart_042_parser_only_katex"
                | "upstream_html_demos_flowchart_flowchart_044_parser_only_katex"
                | "upstream_html_demos_flowchart_graph_039_parser_only_katex"
        ) {
            return Some(
                "upstream Mermaid 11.15 cannot regenerate this parser-only KaTeX HTML-demo fixture",
            );
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::upstream_svg_baseline_skip_reason;

    #[test]
    fn upstream_svg_baseline_skip_reason_accepts_fixture_names_and_stems() {
        assert_eq!(
            upstream_svg_baseline_skip_reason("sequence", "stress_end_keyword_016.mmd"),
            Some("upstream Mermaid 11.15 rejects `(end)` as a participant id")
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason(
                "flowchart",
                "upstream_html_demos_flowchart_flowchart_040_parser_only_katex.svg"
            ),
            Some(
                "upstream Mermaid 11.15 cannot regenerate this parser-only KaTeX HTML-demo fixture"
            )
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason("flowchart", "upstream_docs_flowchart_basic_001"),
            None
        );
    }
}
