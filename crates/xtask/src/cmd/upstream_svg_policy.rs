//! Shared upstream SVG baseline policy.

fn normalized_fixture_stem(name_or_stem: &str) -> &str {
    name_or_stem
        .strip_suffix(".mmd")
        .or_else(|| name_or_stem.strip_suffix(".svg"))
        .unwrap_or(name_or_stem)
}

pub(crate) fn flowchart_elk_svg_parity_admitted(name_or_stem: &str) -> bool {
    let _ = normalized_fixture_stem(name_or_stem);
    false
}

pub(crate) fn flowchart_elk_svg_probe_candidate(name_or_stem: &str) -> bool {
    matches!(
        normalized_fixture_stem(name_or_stem),
        "upstream_html_demos_flowchart_elk_flowchart_elk_001"
    )
}

pub(crate) fn flowchart_elk_svg_parity_skip_reason(name_or_stem: &str) -> Option<&'static str> {
    if flowchart_elk_svg_parity_admitted(name_or_stem) {
        None
    } else {
        Some(
            "Flowchart ELK fixture is not admitted to SVG parity yet; add it to the dedicated ELK layout lane after a targeted probe passes",
        )
    }
}

pub(crate) fn upstream_svg_baseline_skip_reason(
    diagram: &str,
    fixture_name_or_stem: &str,
) -> Option<&'static str> {
    let stem = normalized_fixture_stem(fixture_name_or_stem);

    if diagram == "sequence" && stem == "stress_end_keyword_016" {
        return Some("upstream Mermaid 11.15 rejects `(end)` as a participant id");
    }

    if diagram == "flowchart" && stem == "upstream_flow_text_ellipse_vertex_parser_only_spec" {
        return Some(
            "upstream Mermaid 11.15 cannot render this parser-only ellipse vertex fixture",
        );
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{
        flowchart_elk_svg_parity_admitted, flowchart_elk_svg_parity_skip_reason,
        flowchart_elk_svg_probe_candidate, upstream_svg_baseline_skip_reason,
    };

    #[test]
    fn flowchart_elk_svg_probe_candidates_accept_names_and_stems() {
        assert!(flowchart_elk_svg_probe_candidate(
            "upstream_html_demos_flowchart_elk_flowchart_elk_001"
        ));
        assert!(flowchart_elk_svg_probe_candidate(
            "upstream_html_demos_flowchart_elk_flowchart_elk_001.mmd"
        ));
        assert!(flowchart_elk_svg_probe_candidate(
            "upstream_html_demos_flowchart_elk_flowchart_elk_001.svg"
        ));
        assert!(!flowchart_elk_svg_parity_admitted(
            "upstream_html_demos_flowchart_elk_flowchart_elk_001"
        ));
        assert_eq!(
            flowchart_elk_svg_parity_skip_reason(
                "upstream_html_demos_flowchart_elk_flowchart_elk_001"
            ),
            Some(
                "Flowchart ELK fixture is not admitted to SVG parity yet; add it to the dedicated ELK layout lane after a targeted probe passes"
            )
        );
        assert_eq!(
            flowchart_elk_svg_parity_skip_reason(
                "upstream_cypress_flowchart_elk_spec_1_elk_should_render_a_simple_flowchart_001"
            ),
            Some(
                "Flowchart ELK fixture is not admitted to SVG parity yet; add it to the dedicated ELK layout lane after a targeted probe passes"
            )
        );
    }

    #[test]
    fn upstream_svg_baseline_skip_reason_accepts_fixture_names_and_stems() {
        assert_eq!(
            upstream_svg_baseline_skip_reason("sequence", "stress_end_keyword_016.mmd"),
            Some("upstream Mermaid 11.15 rejects `(end)` as a participant id")
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason(
                "flowchart",
                "upstream_flow_text_ellipse_vertex_parser_only_spec.svg"
            ),
            Some("upstream Mermaid 11.15 cannot render this parser-only ellipse vertex fixture")
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason(
                "flowchart",
                "upstream_html_demos_flowchart_flowchart_040_katex.svg"
            ),
            None
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason("flowchart", "upstream_docs_flowchart_basic_001"),
            None
        );
    }
}
