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
        _ => None,
    }
}
