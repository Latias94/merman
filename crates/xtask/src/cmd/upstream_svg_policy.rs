//! Shared upstream SVG baseline policy.

const FLOWCHART_ELK_SOURCE_BACKED_PROBE_STEMS: &[&str] = &[
    "upstream_html_demos_flowchart_elk_flowchart_elk_001",
    "upstream_cypress_flowchart_elk_spec_3_elk_a_link_with_correct_arrowhead_to_a_subgraph_004",
    "upstream_cypress_flowchart_elk_spec_4_elk_length_of_edges_005",
    "upstream_cypress_flowchart_elk_spec_50_elk_handle_nested_subgraphs_in_reverse_order_009",
    "upstream_cypress_flowchart_elk_spec_52_elk_handle_nested_subgraphs_in_several_levels_011",
    "upstream_cypress_flowchart_elk_spec_53_elk_handle_nested_subgraphs_with_edges_in_and_out_012",
    "upstream_cypress_flowchart_elk_spec_54_elk_handle_nested_subgraphs_with_outgoing_links_013",
    "upstream_cypress_flowchart_elk_spec_55_elk_handle_nested_subgraphs_with_outgoing_links_2_014",
    "upstream_cypress_flowchart_elk_spec_56_elk_handle_nested_subgraphs_with_outgoing_links_3_015",
    "upstream_cypress_flowchart_elk_spec_57_elk_handle_nested_subgraphs_with_outgoing_links_2_017",
    "upstream_cypress_flowchart_elk_spec_57_elk_handle_nested_subgraphs_with_outgoing_links_4_016",
    "upstream_cypress_flowchart_elk_spec_57_x_handle_nested_subgraphs_with_outgoing_links_5_018",
    "upstream_cypress_flowchart_elk_spec_66_elk_more_nested_subgraph_cases_tb_026",
    "upstream_cypress_flowchart_elk_spec_67_elk_more_nested_subgraph_cases_rl_027",
    "upstream_cypress_flowchart_elk_spec_68_elk_more_nested_subgraph_cases_bt_028",
    "upstream_cypress_flowchart_elk_spec_69_elk_more_nested_subgraph_cases_lr_029",
    "upstream_cypress_flowchart_elk_spec_70_elk_handle_nested_subgraph_cases_tb_link_out_and_link_between_030",
    "upstream_cypress_flowchart_elk_spec_71_elk_handle_nested_subgraph_cases_rl_link_out_and_link_between_031",
    "upstream_cypress_flowchart_elk_spec_72_elk_handle_nested_subgraph_cases_bt_link_out_and_link_between_032",
    "upstream_cypress_flowchart_elk_spec_74_elk_handle_labels_for_multiple_edges_from_and_to_the_same_cou_034",
    "upstream_cypress_flowchart_elk_spec_render_with_stylized_arrows_063",
];

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

pub(crate) fn flowchart_elk_svg_source_backed_probe_stems() -> &'static [&'static str] {
    FLOWCHART_ELK_SOURCE_BACKED_PROBE_STEMS
}

pub(crate) fn flowchart_elk_svg_source_backed_probe_admitted(name_or_stem: &str) -> bool {
    FLOWCHART_ELK_SOURCE_BACKED_PROBE_STEMS.contains(&normalized_fixture_stem(name_or_stem))
}

pub(crate) fn flowchart_elk_svg_compare_admitted(
    name_or_stem: &str,
    include_elk_probes: bool,
    backend: merman_render::FlowchartElkBackend,
) -> bool {
    flowchart_elk_svg_parity_admitted(name_or_stem)
        || (include_elk_probes
            && backend == merman_render::FlowchartElkBackend::SourcePorted
            && flowchart_elk_svg_source_backed_probe_admitted(name_or_stem))
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
        flowchart_elk_svg_compare_admitted, flowchart_elk_svg_parity_admitted,
        flowchart_elk_svg_parity_skip_reason, flowchart_elk_svg_source_backed_probe_admitted,
        upstream_svg_baseline_skip_reason,
    };

    #[test]
    fn flowchart_elk_svg_source_backed_probes_accept_names_and_stems() {
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_html_demos_flowchart_elk_flowchart_elk_001"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_html_demos_flowchart_elk_flowchart_elk_001.mmd"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_html_demos_flowchart_elk_flowchart_elk_001.svg"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_3_elk_a_link_with_correct_arrowhead_to_a_subgraph_004"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_4_elk_length_of_edges_005"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_50_elk_handle_nested_subgraphs_in_reverse_order_009"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_52_elk_handle_nested_subgraphs_in_several_levels_011"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_53_elk_handle_nested_subgraphs_with_edges_in_and_out_012"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_54_elk_handle_nested_subgraphs_with_outgoing_links_013"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_55_elk_handle_nested_subgraphs_with_outgoing_links_2_014"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_56_elk_handle_nested_subgraphs_with_outgoing_links_3_015"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_57_elk_handle_nested_subgraphs_with_outgoing_links_2_017"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_57_elk_handle_nested_subgraphs_with_outgoing_links_4_016"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_57_x_handle_nested_subgraphs_with_outgoing_links_5_018"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_66_elk_more_nested_subgraph_cases_tb_026"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_67_elk_more_nested_subgraph_cases_rl_027"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_68_elk_more_nested_subgraph_cases_bt_028"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_69_elk_more_nested_subgraph_cases_lr_029"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_70_elk_handle_nested_subgraph_cases_tb_link_out_and_link_between_030"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_71_elk_handle_nested_subgraph_cases_rl_link_out_and_link_between_031"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_72_elk_handle_nested_subgraph_cases_bt_link_out_and_link_between_032"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_74_elk_handle_labels_for_multiple_edges_from_and_to_the_same_cou_034"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_render_with_stylized_arrows_063"
        ));
        assert!(!flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_1_elk_should_render_a_simple_flowchart_001"
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
    fn flowchart_elk_svg_probe_admission_requires_source_ported_backend() {
        let stem = "upstream_html_demos_flowchart_elk_flowchart_elk_001";

        assert!(!flowchart_elk_svg_compare_admitted(
            stem,
            false,
            merman_render::FlowchartElkBackend::SourcePorted
        ));
        assert!(!flowchart_elk_svg_compare_admitted(
            stem,
            true,
            merman_render::FlowchartElkBackend::Compat
        ));
        assert!(flowchart_elk_svg_compare_admitted(
            stem,
            true,
            merman_render::FlowchartElkBackend::SourcePorted
        ));
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
