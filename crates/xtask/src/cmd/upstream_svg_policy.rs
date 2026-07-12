//! Shared upstream SVG baseline policy.

use crate::XtaskError;

const FLOWCHART_ELK_SOURCE_BACKED_PROBE_STEMS: &[&str] = &[
    "upstream_html_demos_ashish2_example_009",
    "upstream_html_demos_flow_elk_example_001",
    "upstream_html_demos_flowchart_elk_flowchart_elk_001",
    "upstream_html_demos_knsv2_example_011",
    "upstream_cypress_flowchart_elk_spec_1_elk_should_render_a_simple_flowchart_001",
    "upstream_cypress_flowchart_elk_spec_1_elk_should_render_a_simple_flowchart_002",
    "upstream_cypress_flowchart_elk_spec_2_elk_should_render_a_simple_flowchart_with_diagrampadding_set_t_003",
    "upstream_cypress_flowchart_elk_spec_3_elk_a_link_with_correct_arrowhead_to_a_subgraph_004",
    "upstream_cypress_flowchart_elk_spec_4_elk_length_of_edges_005",
    "upstream_cypress_flowchart_elk_spec_5_elk_should_render_escaped_without_html_labels_006",
    "upstream_cypress_flowchart_elk_spec_6_elk_should_render_non_escaped_with_html_labels_007",
    "upstream_cypress_flowchart_elk_spec_v2_elk_16_render_stadium_shape_008",
    "upstream_cypress_flowchart_elk_spec_50_elk_handle_nested_subgraphs_in_reverse_order_009",
    "upstream_cypress_flowchart_elk_spec_51_elk_handle_nested_subgraphs_in_reverse_order_010",
    "upstream_cypress_flowchart_elk_spec_52_elk_handle_nested_subgraphs_in_several_levels_011",
    "upstream_cypress_flowchart_elk_spec_53_elk_handle_nested_subgraphs_with_edges_in_and_out_012",
    "upstream_cypress_flowchart_elk_spec_54_elk_handle_nested_subgraphs_with_outgoing_links_013",
    "upstream_cypress_flowchart_elk_spec_55_elk_handle_nested_subgraphs_with_outgoing_links_2_014",
    "upstream_cypress_flowchart_elk_spec_56_elk_handle_nested_subgraphs_with_outgoing_links_3_015",
    "upstream_cypress_flowchart_elk_spec_57_elk_handle_nested_subgraphs_with_outgoing_links_2_017",
    "upstream_cypress_flowchart_elk_spec_57_elk_handle_nested_subgraphs_with_outgoing_links_4_016",
    "upstream_cypress_flowchart_elk_spec_57_x_handle_nested_subgraphs_with_outgoing_links_5_018",
    "upstream_cypress_flowchart_elk_spec_58_elk_handle_styling_with_style_expressions_019",
    "upstream_cypress_flowchart_elk_spec_59_elk_handle_styling_of_subgraphs_and_links_020",
    "upstream_cypress_flowchart_elk_spec_60_elk_handle_styling_for_all_node_shapes_v2_021",
    "upstream_cypress_flowchart_elk_spec_61_elk_fontawesome_icons_in_edge_labels_022",
    "upstream_cypress_flowchart_elk_spec_62_elk_should_render_styled_subgraphs_023",
    "upstream_cypress_flowchart_elk_spec_63_elk_title_on_subgraphs_should_be_themeable_024",
    "upstream_cypress_flowchart_elk_spec_65_elk_text_color_from_classes_025",
    "upstream_cypress_flowchart_elk_spec_66_elk_more_nested_subgraph_cases_tb_026",
    "upstream_cypress_flowchart_elk_spec_67_elk_more_nested_subgraph_cases_rl_027",
    "upstream_cypress_flowchart_elk_spec_68_elk_more_nested_subgraph_cases_bt_028",
    "upstream_cypress_flowchart_elk_spec_69_elk_more_nested_subgraph_cases_lr_029",
    "upstream_cypress_flowchart_elk_spec_70_elk_handle_nested_subgraph_cases_tb_link_out_and_link_between_030",
    "upstream_cypress_flowchart_elk_spec_71_elk_handle_nested_subgraph_cases_rl_link_out_and_link_between_031",
    "upstream_cypress_flowchart_elk_spec_72_elk_handle_nested_subgraph_cases_bt_link_out_and_link_between_032",
    "upstream_cypress_flowchart_elk_spec_74_elk_handle_nested_subgraph_cases_rl_link_out_and_link_between_033",
    "upstream_cypress_flowchart_elk_spec_74_elk_handle_labels_for_multiple_edges_from_and_to_the_same_cou_034",
    "upstream_cypress_flowchart_elk_spec_76_elk_handle_unicode_encoded_character_with_html_labels_true_035",
    "upstream_cypress_flowchart_elk_spec_2050_elk_handling_of_different_rendering_direction_in_subgraphs_036",
    "upstream_cypress_flowchart_elk_spec_1433_elk_should_render_a_titled_flowchart_with_titletopmargin_se_039",
    "upstream_cypress_flowchart_elk_spec_with_styling_and_classes_040",
    "upstream_cypress_flowchart_elk_spec_with_formatting_in_a_node_041",
    "upstream_cypress_flowchart_elk_spec_new_line_in_node_and_formatted_edge_label_042",
    "upstream_cypress_flowchart_elk_spec_sub_graphs_and_markdown_strings_043",
    "upstream_cypress_flowchart_elk_spec_with_styling_and_classes_044",
    "upstream_cypress_flowchart_elk_spec_with_formatting_in_a_node_045",
    "upstream_cypress_flowchart_elk_spec_new_line_in_node_and_formatted_edge_label_046",
    "upstream_cypress_flowchart_elk_spec_wrapping_long_text_with_a_new_line_047",
    "upstream_cypress_flowchart_elk_spec_sub_graphs_and_markdown_strings_048",
    "upstream_cypress_flowchart_elk_spec_2388_elk_handling_default_in_the_node_name_037",
    "upstream_cypress_flowchart_elk_spec_2824_elk_clipping_of_edges_038",
    "upstream_cypress_flowchart_elk_spec_6080_should_handle_diamond_shape_intersections_050",
    "upstream_cypress_flowchart_elk_spec_6088_1_should_handle_diamond_shape_intersections_051",
    "upstream_cypress_flowchart_elk_spec_6088_2_should_handle_diamond_shape_intersections_052",
    "upstream_cypress_flowchart_elk_spec_6088_3_should_handle_diamond_shape_intersections_053",
    "upstream_cypress_flowchart_elk_spec_6088_4_should_handle_diamond_shape_intersections_054",
    "upstream_cypress_flowchart_elk_spec_6088_5_should_handle_diamond_shape_intersections_055",
    "upstream_cypress_flowchart_elk_spec_6088_6_should_handle_diamond_shape_intersections_056",
    "upstream_cypress_flowchart_elk_spec_6647_elk_should_keep_node_order_when_using_elk_layout_unless_it_057",
    "upstream_cypress_flowchart_elk_spec_7213_should_render_elk_edges_with_right_angles_not_curves_058",
    "upstream_cypress_flowchart_elk_spec_7_elk_should_render_a_flowchart_when_usemaxwidth_is_true_default_059",
    "upstream_cypress_flowchart_elk_spec_8_elk_should_render_a_flowchart_when_usemaxwidth_is_false_060",
    "upstream_cypress_flowchart_elk_spec_elk_should_include_classes_on_the_edges_061",
    "upstream_cypress_flowchart_elk_spec_should_render_a_flowchart_with_title_062",
    "upstream_cypress_flowchart_elk_spec_sub_graphs_049",
    "upstream_cypress_flowchart_elk_spec_render_with_stylized_arrows_063",
    "upstream_cypress_flowchart_v2_spec_should_render_self_loops_elk_064",
];

fn normalized_fixture_stem(name_or_stem: &str) -> &str {
    name_or_stem
        .strip_suffix(".mmd")
        .or_else(|| name_or_stem.strip_suffix(".svg"))
        .unwrap_or(name_or_stem)
}

pub(crate) fn flowchart_elk_svg_parity_admitted(name_or_stem: &str) -> bool {
    flowchart_elk_svg_source_backed_probe_admitted(name_or_stem)
}

pub(crate) fn flowchart_elk_svg_source_backed_probe_stems() -> &'static [&'static str] {
    FLOWCHART_ELK_SOURCE_BACKED_PROBE_STEMS
}

pub(crate) fn default_flowchart_elk_backend() -> merman_render::FlowchartElkBackend {
    merman_render::FlowchartElkBackend::SourcePorted
}

pub(crate) fn parse_flowchart_elk_backend(
    raw: Option<&str>,
) -> Result<merman_render::FlowchartElkBackend, XtaskError> {
    match raw.map(str::trim) {
        Some("compat") => Ok(merman_render::FlowchartElkBackend::Compat),
        Some("source-ported" | "source_ported" | "source") => {
            Ok(merman_render::FlowchartElkBackend::SourcePorted)
        }
        _ => Err(XtaskError::Usage),
    }
}

pub(crate) fn flowchart_elk_backend_name(
    backend: merman_render::FlowchartElkBackend,
) -> &'static str {
    match backend {
        merman_render::FlowchartElkBackend::Compat => "compat",
        merman_render::FlowchartElkBackend::SourcePorted => "source-ported",
    }
}

pub(crate) fn flowchart_elk_svg_source_backed_probe_admitted(name_or_stem: &str) -> bool {
    FLOWCHART_ELK_SOURCE_BACKED_PROBE_STEMS.contains(&normalized_fixture_stem(name_or_stem))
}

pub(crate) fn flowchart_elk_svg_compare_admitted(
    name_or_stem: &str,
    include_elk_probes: bool,
    backend: merman_render::FlowchartElkBackend,
) -> bool {
    let admitted = flowchart_elk_svg_parity_admitted(name_or_stem)
        || (include_elk_probes && flowchart_elk_svg_source_backed_probe_admitted(name_or_stem));
    admitted && backend == merman_render::FlowchartElkBackend::SourcePorted
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

pub(crate) fn flowchart_elk_svg_compare_skip_reason(
    name_or_stem: &str,
    include_elk_probes: bool,
    backend: merman_render::FlowchartElkBackend,
) -> Option<&'static str> {
    if flowchart_elk_svg_compare_admitted(name_or_stem, include_elk_probes, backend) {
        return None;
    }

    let admitted_under_source_backed = flowchart_elk_svg_parity_admitted(name_or_stem)
        || (include_elk_probes && flowchart_elk_svg_source_backed_probe_admitted(name_or_stem));
    if admitted_under_source_backed && backend != merman_render::FlowchartElkBackend::SourcePorted {
        return Some(
            "Flowchart ELK SVG parity admission requires the source-backed ELK backend; use `--flowchart-elk-backend source-ported` or omit the compat override",
        );
    }

    flowchart_elk_svg_parity_skip_reason(name_or_stem)
}

pub(crate) fn upstream_svg_baseline_skip_reason(
    diagram: &str,
    fixture_name_or_stem: &str,
) -> Option<&'static str> {
    let stem = normalized_fixture_stem(fixture_name_or_stem);

    if diagram == "sequence" && stem == "stress_end_keyword_016" {
        return Some("pinned Mermaid 11.16 rejects `(end)` as a participant id");
    }

    if diagram == "flowchart" && stem == "upstream_flow_text_ellipse_vertex_parser_only_spec" {
        return Some("pinned Mermaid 11.16 cannot render this parser-only ellipse vertex fixture");
    }

    if diagram == "flowchart" && stem.starts_with("local_flowchart_elk_hardening_") {
        return Some(
            "local Flowchart ELK hardening fixture is covered by semantic/layout snapshots and has no upstream SVG baseline",
        );
    }

    if diagram == "state" && stem == "upstream_state_parser_spec" {
        return Some("pinned Mermaid 11.16 crashes on this parser-only state fixture");
    }

    if diagram == "class" && stem == "upstream_text_label_variants_spec" {
        return Some("pinned Mermaid 11.16 fails on the whitespace-only class label fixture");
    }

    if diagram == "gantt"
        && matches!(
            stem,
            "click_loose"
                | "click_strict"
                | "dateformat_hash_comment_truncates"
                | "excludes_hash_comment_truncates"
                | "today_marker_and_axis"
        )
    {
        return Some(
            "fixture is retained for parser parity but is not a stable pinned Mermaid 11.16 SVG baseline",
        );
    }

    if diagram == "c4"
        && matches!(
            stem,
            "nesting_updates"
                | "upstream_boundary_spec"
                | "upstream_c4container_header_and_direction_spec"
                | "upstream_container_spec"
                | "upstream_person_ext_spec"
                | "upstream_person_spec"
                | "upstream_system_spec"
                | "upstream_update_element_style_all_fields_spec"
        )
    {
        return Some("pinned Mermaid 11.16 C4 renderer rejects this parser fixture at render time");
    }

    None
}

pub(crate) fn upstream_svg_compare_skip_reason(
    diagram: &str,
    fixture_name_or_stem: &str,
) -> Option<&'static str> {
    let stem = normalized_fixture_stem(fixture_name_or_stem);

    if let Some(reason) = upstream_svg_baseline_skip_reason(diagram, stem) {
        return Some(reason);
    }

    if diagram == "class" && stem == "upstream_parser_class_spec" {
        return Some(
            "pinned Mermaid 11.16 renders prototype-key class ids with NaN transforms and missing nodes; compare-class-svgs and compare-svg-xml already exclude this fixture",
        );
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{
        default_flowchart_elk_backend, flowchart_elk_backend_name,
        flowchart_elk_svg_compare_admitted, flowchart_elk_svg_compare_skip_reason,
        flowchart_elk_svg_parity_admitted, flowchart_elk_svg_parity_skip_reason,
        flowchart_elk_svg_source_backed_probe_admitted, parse_flowchart_elk_backend,
        upstream_svg_baseline_skip_reason, upstream_svg_compare_skip_reason,
    };

    #[test]
    fn flowchart_elk_svg_source_backed_probes_accept_names_and_stems() {
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_html_demos_ashish2_example_009"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_html_demos_flow_elk_example_001"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_html_demos_flowchart_elk_flowchart_elk_001"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_html_demos_knsv2_example_011"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_html_demos_flowchart_elk_flowchart_elk_001.mmd"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_html_demos_flowchart_elk_flowchart_elk_001.svg"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_1_elk_should_render_a_simple_flowchart_001"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_1_elk_should_render_a_simple_flowchart_002"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_2_elk_should_render_a_simple_flowchart_with_diagrampadding_set_t_003"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_3_elk_a_link_with_correct_arrowhead_to_a_subgraph_004"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_4_elk_length_of_edges_005"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_5_elk_should_render_escaped_without_html_labels_006"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_6_elk_should_render_non_escaped_with_html_labels_007"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_v2_elk_16_render_stadium_shape_008"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_50_elk_handle_nested_subgraphs_in_reverse_order_009"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_51_elk_handle_nested_subgraphs_in_reverse_order_010"
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
            "upstream_cypress_flowchart_elk_spec_58_elk_handle_styling_with_style_expressions_019"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_59_elk_handle_styling_of_subgraphs_and_links_020"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_60_elk_handle_styling_for_all_node_shapes_v2_021"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_61_elk_fontawesome_icons_in_edge_labels_022"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_62_elk_should_render_styled_subgraphs_023"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_63_elk_title_on_subgraphs_should_be_themeable_024"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_65_elk_text_color_from_classes_025"
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
            "upstream_cypress_flowchart_elk_spec_74_elk_handle_nested_subgraph_cases_rl_link_out_and_link_between_033"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_74_elk_handle_labels_for_multiple_edges_from_and_to_the_same_cou_034"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_76_elk_handle_unicode_encoded_character_with_html_labels_true_035"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_1433_elk_should_render_a_titled_flowchart_with_titletopmargin_se_039"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_with_styling_and_classes_040"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_with_formatting_in_a_node_041"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_new_line_in_node_and_formatted_edge_label_042"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_sub_graphs_and_markdown_strings_043"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_with_styling_and_classes_044"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_with_formatting_in_a_node_045"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_new_line_in_node_and_formatted_edge_label_046"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_wrapping_long_text_with_a_new_line_047"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_sub_graphs_and_markdown_strings_048"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_2388_elk_handling_default_in_the_node_name_037"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_2824_elk_clipping_of_edges_038"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_6080_should_handle_diamond_shape_intersections_050"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_6088_1_should_handle_diamond_shape_intersections_051"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_6088_2_should_handle_diamond_shape_intersections_052"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_6088_3_should_handle_diamond_shape_intersections_053"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_6088_4_should_handle_diamond_shape_intersections_054"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_6088_5_should_handle_diamond_shape_intersections_055"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_6088_6_should_handle_diamond_shape_intersections_056"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_6647_elk_should_keep_node_order_when_using_elk_layout_unless_it_057"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_7213_should_render_elk_edges_with_right_angles_not_curves_058"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_7_elk_should_render_a_flowchart_when_usemaxwidth_is_true_default_059"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_8_elk_should_render_a_flowchart_when_usemaxwidth_is_false_060"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_elk_should_include_classes_on_the_edges_061"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_should_render_a_flowchart_with_title_062"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_sub_graphs_049"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_elk_spec_render_with_stylized_arrows_063"
        ));
        assert!(flowchart_elk_svg_source_backed_probe_admitted(
            "upstream_cypress_flowchart_v2_spec_should_render_self_loops_elk_064"
        ));
        assert!(flowchart_elk_svg_parity_admitted(
            "upstream_html_demos_flowchart_elk_flowchart_elk_001"
        ));
        assert_eq!(
            flowchart_elk_svg_parity_skip_reason(
                "upstream_html_demos_flowchart_elk_flowchart_elk_001"
            ),
            None
        );
        assert_eq!(
            flowchart_elk_svg_parity_skip_reason(
                "upstream_cypress_flowchart_elk_spec_1_elk_should_render_a_simple_flowchart_001"
            ),
            None
        );
    }

    #[test]
    fn flowchart_elk_svg_compare_admission_requires_source_ported_backend() {
        let stem = "upstream_html_demos_flowchart_elk_flowchart_elk_001";

        assert!(flowchart_elk_svg_compare_admitted(
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
        assert_eq!(
            flowchart_elk_svg_compare_skip_reason(
                stem,
                false,
                merman_render::FlowchartElkBackend::Compat
            ),
            Some(
                "Flowchart ELK SVG parity admission requires the source-backed ELK backend; use `--flowchart-elk-backend source-ported` or omit the compat override"
            )
        );
    }

    #[test]
    fn flowchart_elk_backend_helpers_default_to_source_ported() {
        assert_eq!(
            default_flowchart_elk_backend(),
            merman_render::FlowchartElkBackend::SourcePorted
        );
        assert_eq!(
            parse_flowchart_elk_backend(Some("source_ported")).unwrap(),
            merman_render::FlowchartElkBackend::SourcePorted
        );
        assert_eq!(
            parse_flowchart_elk_backend(Some("source")).unwrap(),
            merman_render::FlowchartElkBackend::SourcePorted
        );
        assert_eq!(
            parse_flowchart_elk_backend(Some("compat")).unwrap(),
            merman_render::FlowchartElkBackend::Compat
        );
        assert!(parse_flowchart_elk_backend(Some("unknown")).is_err());
        assert_eq!(
            flowchart_elk_backend_name(default_flowchart_elk_backend()),
            "source-ported"
        );
    }

    #[test]
    fn upstream_svg_baseline_skip_reason_accepts_fixture_names_and_stems() {
        assert_eq!(
            upstream_svg_baseline_skip_reason("sequence", "stress_end_keyword_016.mmd"),
            Some("pinned Mermaid 11.16 rejects `(end)` as a participant id")
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason(
                "flowchart",
                "upstream_flow_text_ellipse_vertex_parser_only_spec.svg"
            ),
            Some("pinned Mermaid 11.16 cannot render this parser-only ellipse vertex fixture")
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason(
                "flowchart",
                "upstream_html_demos_flowchart_flowchart_040_katex.svg"
            ),
            None
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason(
                "flowchart",
                "local_flowchart_elk_hardening_compound_self_loops_003.mmd"
            ),
            Some(
                "local Flowchart ELK hardening fixture is covered by semantic/layout snapshots and has no upstream SVG baseline"
            )
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason("state", "upstream_state_parser_spec.mmd"),
            Some("pinned Mermaid 11.16 crashes on this parser-only state fixture")
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason("class", "upstream_text_label_variants_spec.mmd"),
            Some("pinned Mermaid 11.16 fails on the whitespace-only class label fixture")
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason("gantt", "click_loose.mmd"),
            Some(
                "fixture is retained for parser parity but is not a stable pinned Mermaid 11.16 SVG baseline"
            )
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason("c4", "upstream_person_spec.mmd"),
            Some("pinned Mermaid 11.16 C4 renderer rejects this parser fixture at render time")
        );
        assert_eq!(
            upstream_svg_baseline_skip_reason("flowchart", "upstream_docs_flowchart_basic_001"),
            None
        );
    }

    #[test]
    fn upstream_svg_compare_skip_reason_covers_compare_only_class_artifacts() {
        assert_eq!(
            upstream_svg_compare_skip_reason("state", "upstream_state_parser_spec"),
            Some("pinned Mermaid 11.16 crashes on this parser-only state fixture")
        );
        assert_eq!(
            upstream_svg_compare_skip_reason("class", "upstream_text_label_variants_spec"),
            Some("pinned Mermaid 11.16 fails on the whitespace-only class label fixture")
        );
        let reason = upstream_svg_compare_skip_reason("class", "upstream_parser_class_spec")
            .expect("prototype-key class ids should be skipped from compare");
        assert!(reason.contains("prototype-key class ids"));
        assert_eq!(
            upstream_svg_compare_skip_reason("class", "upstream_namespaces_and_generics"),
            None
        );
    }
}
