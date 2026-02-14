// Fixture-derived root viewport overrides for Mermaid@11.12.2 GitGraph diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/gitgraph/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_gitgraph_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_docs_gitgraph_base_theme_046" => Some((
            "-124.078125 -19 1032.078125 536.0304565429688",
            "1032.078125",
        )),
        "upstream_docs_gitgraph_default_theme_050" => Some((
            "-124.078125 -19 1032.078125 536.0304565429688",
            "1032.078125",
        )),
        "upstream_docs_gitgraph_forest_theme_048" => Some((
            "-124.078125 -19 1032.078125 536.0304565429688",
            "1032.078125",
        )),
        "upstream_docs_gitgraph_neutral_theme_054" => Some((
            "-124.078125 -19 1032.078125 536.0304565429688",
            "1032.078125",
        )),
        "upstream_docs_gitgraph_hiding_branch_names_and_lines_022" => Some((
            "-38.39225769042969 -18 915.3922729492188 535.0304565429688",
            "915.3922729492188",
        )),
        "upstream_docs_accessibility_gitgraph_011" => Some((
            "-117.5078125 -19 475.5078125 174.04257202148438",
            "475.5078125",
        )),
        "upstream_docs_gitgraph_merging_two_branches_009" => Some((
            "-192.7578125 -34.70000076293945 700.7578125 251.35699462890625",
            "700.7578125",
        )),
        "upstream_docs_gitgraph_cherry_pick_commit_from_another_branch_010" => Some((
            "-117.5078125 -19 575.5078125 252.66668701171875",
            "575.5078125",
        )),
        "upstream_docs_gitgraph_commit_labels_layout_rotated_or_horizontal_012" => {
            Some(("-96.25 -19 604.25 354.1170654296875", "604.25"))
        }
        "upstream_docs_gitgraph_customizing_main_branch_name_015" => Some((
            "-141.75 -34.70000076293945 899.75 271.9604187011719",
            "899.75",
        )),
        "upstream_docs_gitgraph_dark_theme_026" => Some((
            "-124.078125 -19 1032.078125 536.0304565429688",
            "1032.078125",
        )),
        "upstream_docs_gitgraph_customize_using_theme_variables_028" => {
            Some(("-124.078125 -19 532.078125 262.2937927246094", "532.078125"))
        }
        "upstream_docs_gitgraph_customizing_branch_colors_029" => {
            Some(("-124.078125 -19 532.078125 262.2937927246094", "532.078125"))
        }
        "upstream_docs_gitgraph_customizing_branch_label_colors_030" => {
            Some(("-118.9453125 -19 176.9453125 849", "176.9453125"))
        }
        "upstream_docs_gitgraph_customizing_commit_colors_031" => {
            Some(("-124.078125 -19 532.078125 262.2937927246094", "532.078125"))
        }
        "upstream_docs_gitgraph_customizing_commit_label_font_size_032" => {
            Some(("-124.078125 -19 532.078125 286.4273376464844", "532.078125"))
        }
        "upstream_docs_gitgraph_customizing_tag_label_font_size_033" => {
            Some(("-124.078125 -19 532.078125 262.2937927246094", "532.078125"))
        }
        "upstream_docs_gitgraph_customizing_tag_colors_034" => {
            Some(("-124.078125 -19 532.078125 262.2937927246094", "532.078125"))
        }
        "upstream_docs_gitgraph_customizing_highlight_commit_colors_035" => {
            Some(("-124.078125 -19 532.078125 262.2937927246094", "532.078125"))
        }
        "upstream_branches_and_order" => Some(("-234.203125 -19 392.203125 1119", "392.203125")),
        "upstream_cherry_pick_merge_commits" => Some((
            "-114.3515625 -19 572.3515625 236.35057067871094",
            "572.3515625",
        )),
        "upstream_html_demos_git_cherry_pick_from_branch_graph_013" => Some((
            "-139.578125 -50 406.3798828125 204.81568908691406",
            "406.3798828125",
        )),
        "upstream_html_demos_git_cherry_pick_from_branch_graph_014" => Some((
            "-124.47250366210938 -50 347.265625 378.5620422363281",
            "347.265625",
        )),
        "upstream_html_demos_git_cherry_pick_from_branch_graph_015" => {
            Some(("-124.47250366210938 -50 347.265625 361", "347.265625"))
        }
        "upstream_html_demos_git_cherry_pick_from_main_graph_016" => Some((
            "-117.5078125 -50 375.5078125 205.11708068847656",
            "375.5078125",
        )),
        "upstream_html_demos_git_cherry_pick_from_main_graph_017" => Some((
            "-104.87841796875 -50 331.5625 358.5738525390625",
            "331.5625",
        )),
        "upstream_html_demos_git_cherry_pick_then_merge_graph_019" => Some((
            "-139.578125 -50 447.578125 204.81568908691406",
            "447.578125",
        )),
        "upstream_html_demos_git_cherry_pick_then_merge_graph_020" => {
            Some(("-121.42562866210938 -50 341.171875 388", "341.171875"))
        }
        "upstream_html_demos_git_cherry_pick_then_merge_graph_021" => {
            Some(("-121.42562866210938 -50 341.171875 411", "341.171875"))
        }
        "upstream_html_demos_git_continuous_development_graph_004" => Some((
            "-117.5078125 -50 375.5078125 205.11708068847656",
            "375.5078125",
        )),
        "upstream_html_demos_git_continuous_development_graph_005" => {
            Some(("-131.08526611328125 -50 349.65625 338", "349.65625"))
        }
        "upstream_html_demos_git_continuous_development_graph_006" => {
            Some(("-131.08526611328125 -50 349.65625 361", "349.65625"))
        }
        "upstream_html_demos_git_three_branches_and_a_cherry_pick_from_each_graph_031" => {
            Some(("-124.078125 -50 732.078125 278.6350402832031", "732.078125"))
        }
        "upstream_html_demos_git_three_branches_and_a_cherry_pick_from_each_graph_032" => {
            Some(("-144.70376586914062 -50 508.90625 688", "508.90625"))
        }
        "upstream_html_demos_git_three_branches_and_a_cherry_pick_from_each_graph_033" => {
            Some(("-144.70376586914062 -50 508.90625 711", "508.90625"))
        }
        "upstream_html_demos_git_two_branches_from_same_commit_graph_028" => {
            Some(("-145.125 -50 453.125 295.5024719238281", "453.125"))
        }
        "upstream_html_demos_git_two_branches_from_same_commit_graph_029" => {
            Some(("-92.12482452392578 -50 416.40625 388", "416.40625"))
        }
        "upstream_html_demos_git_two_branches_from_same_commit_graph_030" => {
            Some(("-92.12482452392578 -50 416.40625 411", "416.40625"))
        }
        "upstream_html_demos_git_two_way_merges_010" => Some((
            "-117.5078125 -50 475.5078125 205.11709594726562",
            "475.5078125",
        )),
        "upstream_html_demos_git_two_way_merges_011" => {
            Some(("-98.67864990234375 -50 283.71875 438", "283.71875"))
        }
        "upstream_html_demos_git_two_way_merges_012" => {
            Some(("-98.67864990234375 -50 283.71875 461", "283.71875"))
        }
        "upstream_html_demos_git_simple_branch_and_merge_graph_001" => {
            Some(("-165.0625 -50 348.546875 205.11708068847656", "348.546875"))
        }
        "upstream_html_demos_git_simple_branch_and_merge_graph_002" => {
            Some(("-134.18487548828125 -50 366.890625 238", "366.890625"))
        }
        "upstream_html_demos_git_simple_branch_and_merge_graph_003" => {
            Some(("-134.18487548828125 -50 366.890625 261", "366.890625"))
        }
        "upstream_html_demos_git_merge_feature_to_advanced_main_graph_007" => {
            Some(("-163.8203125 -50 395.0625 205.11708068847656", "395.0625"))
        }
        "upstream_html_demos_git_merge_feature_to_advanced_main_graph_008" => {
            Some(("-157.94268798828125 -50 413.40625 288", "413.40625"))
        }
        "upstream_html_demos_git_merge_feature_to_advanced_main_graph_009" => {
            Some(("-157.94268798828125 -50 413.40625 311", "413.40625"))
        }
        "upstream_html_demos_git_cherry_pick_from_main_graph_018" => {
            Some(("-104.87841796875 -50 331.5625 361", "331.5625"))
        }
        "upstream_html_demos_git_merge_from_main_onto_undeveloped_branch_graph_022" => Some((
            "-195.36328125 -50 480.21875 205.11708068847656",
            "480.21875",
        )),
        "upstream_html_demos_git_merge_from_main_onto_undeveloped_branch_graph_023" => {
            Some(("-206.03836059570312 -50 498.5625 288", "498.5625"))
        }
        "upstream_html_demos_git_merge_from_main_onto_undeveloped_branch_graph_024" => {
            Some(("-206.03836059570312 -50 498.5625 311", "498.5625"))
        }
        "upstream_html_demos_git_merge_from_main_onto_developed_branch_graph_025" => {
            Some(("-160.53515625 -50 460.5625 205.11708068847656", "460.5625"))
        }
        "upstream_html_demos_git_merge_from_main_onto_developed_branch_graph_026" => {
            Some(("-196.20245361328125 -50 478.890625 338", "478.890625"))
        }
        "upstream_html_demos_git_merge_from_main_onto_developed_branch_graph_027" => {
            Some(("-196.20245361328125 -50 478.890625 361", "478.890625"))
        }
        _ => None,
    }
}
