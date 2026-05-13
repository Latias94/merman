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
        "upstream_docs_gitgraph_customizing_commit_label_font_size_032" => Some((
            "-123.515625 -19 531.515625 287.79278564453125",
            "531.515625",
        )),
        "upstream_html_demos_git_cherry_pick_from_branch_graph_013" => {
            Some(("-139.328125 -50 405.859375 203.3262939453125", "405.859375"))
        }
        "upstream_html_demos_git_cherry_pick_from_branch_graph_014" => Some((
            "-124.17623901367188 -50 347.265625 378.19744873046875",
            "347.265625",
        )),
        "upstream_html_demos_git_cherry_pick_from_branch_graph_015" => {
            Some(("-124.17623901367188 -50 347.265625 361", "347.265625"))
        }
        "upstream_html_demos_git_cherry_pick_from_main_graph_017" => Some((
            "-105.00030517578125 -50 331.5625 357.8128967285156",
            "331.5625",
        )),
        "upstream_merges_spec" => Some((
            "-146.375 -34.72539138793945 654.375 341.15484619140625",
            "654.375",
        )),
        "upstream_html_demos_git_simple_branch_and_merge_graph_001" => {
            Some(("-164.9375 -50 348.546875 204.290771484375", "348.546875"))
        }
        "upstream_html_demos_git_simple_branch_and_merge_graph_002" => {
            Some(("-134.10476684570312 -50 366.890625 238", "366.890625"))
        }
        "upstream_html_demos_git_simple_branch_and_merge_graph_003" => {
            Some(("-134.10476684570312 -50 366.890625 261", "366.890625"))
        }
        "upstream_html_demos_git_merge_feature_to_advanced_main_graph_007" => {
            Some(("-162.6953125 -50 394.0625 203.94400024414062", "394.0625"))
        }
        "upstream_html_demos_git_cherry_pick_from_main_graph_018" => {
            Some(("-105.00030517578125 -50 331.5625 361", "331.5625"))
        }
        "upstream_html_demos_git_merge_from_main_onto_undeveloped_branch_graph_022" => {
            Some(("-194.3203125 -50 479.21875 203.94400024414062", "479.21875"))
        }
        "upstream_html_demos_git_merge_from_main_onto_undeveloped_branch_graph_023" => {
            Some(("-204.80123901367188 -50 497.5625 288", "497.5625"))
        }
        "upstream_html_demos_git_merge_from_main_onto_undeveloped_branch_graph_024" => {
            Some(("-204.80123901367188 -50 497.5625 311", "497.5625"))
        }
        "upstream_html_demos_git_merge_from_main_onto_developed_branch_graph_025" => {
            Some(("-159.4921875 -50 459.5625 203.53219604492188", "459.5625"))
        }
        "upstream_html_demos_git_merge_from_main_onto_developed_branch_graph_026" => {
            Some(("-194.78854370117188 -50 477.890625 338", "477.890625"))
        }
        "upstream_html_demos_git_merge_from_main_onto_developed_branch_graph_027" => {
            Some(("-194.78854370117188 -50 477.890625 361", "477.890625"))
        }
        "upstream_cypress_gitgraph_spec_21_should_render_a_simple_gitgraph_with_three_branches_and_tagge_024" => {
            Some((
                "-49.88502883911133 -8 374.3381652832031 546",
                "374.3381652832031",
            ))
        }
        "upstream_cypress_gitgraph_spec_22_should_render_a_simple_gitgraph_with_more_than_8_branches_ove_025" => {
            Some(("-35.5 -8 1137.046875 96", "1137.046875"))
        }
        "upstream_cypress_gitgraph_spec_28_should_render_a_simple_gitgraph_with_two_cherry_pick_commit_v_031" => {
            Some((
                "-51.76327896118164 -8 324.7320251464844 646",
                "324.7320251464844",
            ))
        }
        "upstream_cypress_gitgraph_spec_29_should_render_commits_for_more_than_8_branches_vertical_branc_032" => {
            Some((
                "-62.7565803527832 -8 1164.303466796875 958.140869140625",
                "1164.303466796875",
            ))
        }
        "upstream_cypress_gitgraph_spec_30_should_render_a_simple_gitgraph_with_three_branches_custom_me_033" => {
            Some((
                "-60.336952209472656 -8 384.7900695800781 546",
                "384.7900695800781",
            ))
        }
        "upstream_cypress_gitgraph_spec_39_should_render_gitgraph_with_branch_and_sub_branch_neither_of_042" => {
            Some((
                "-62.7565803527832 -8 284.209716796875 396",
                "284.209716796875",
            ))
        }
        "upstream_cypress_gitgraph_spec_46_should_render_gitgraph_with_unconnected_branches_and_parallel_049" => {
            Some((
                "-62.7565803527832 -8 401.811279296875 354.42852783203125",
                "401.811279296875",
            ))
        }
        "upstream_cypress_gitgraph_spec_56_should_render_a_simple_gitgraph_with_three_branches_and_tagge_060" => {
            Some((
                "-49.88502883911133 22 374.3381652832031 539",
                "374.3381652832031",
            ))
        }
        "upstream_cypress_gitgraph_spec_57_should_render_a_simple_gitgraph_with_more_than_8_branches_ove_061" => {
            Some(("-35.5 22 1137.046875 89", "1137.046875"))
        }
        "upstream_cypress_gitgraph_spec_63_should_render_a_simple_gitgraph_with_two_cherry_pick_commit_v_067" => {
            Some((
                "-51.76327896118164 22 324.7320251464844 639",
                "324.7320251464844",
            ))
        }
        "upstream_cypress_gitgraph_spec_64_should_render_commits_for_more_than_8_branches_vertical_branc_068" => {
            Some((
                "-62.7565803527832 22 1164.303466796875 939",
                "1164.303466796875",
            ))
        }
        "upstream_cypress_gitgraph_spec_65_should_render_a_simple_gitgraph_with_three_branches_custom_me_069" => {
            Some((
                "-60.336952209472656 22 384.7900695800781 539",
                "384.7900695800781",
            ))
        }
        "upstream_cypress_gitgraph_spec_70_should_render_gitgraph_with_branch_and_sub_branch_neither_of_074" => {
            Some((
                "-62.7565803527832 22 284.209716796875 389",
                "284.209716796875",
            ))
        }
        "upstream_cypress_gitgraph_spec_72_should_render_gitgraph_with_unconnected_branches_and_parallel_076" => {
            Some((
                "-62.7565803527832 22 401.811279296875 339",
                "401.811279296875",
            ))
        }
        "stress_gitgraph_font_size_097" => Some((
            "-262.640625 -18 470.640625 177.95840454101562",
            "470.640625",
        )),
        "upstream_cypress_gitgraph_spec_15_should_render_a_simple_gitgraph_with_commit_on_main_branch_ve_018" => {
            Some(("-35.5 -8 69 196", "69"))
        }
        "upstream_cypress_gitgraph_spec_19_should_render_a_simple_gitgraph_with_two_branches_vertical_br_022" => {
            Some(("-35.5 -8 187.2109375 346", "187.2109375"))
        }
        "upstream_cypress_gitgraph_spec_20_should_render_a_simple_gitgraph_with_two_branches_and_merge_c_023" => {
            Some(("-35.5 -8 187.2109375 396", "187.2109375"))
        }
        "upstream_cypress_gitgraph_spec_23_should_render_a_simple_gitgraph_with_rotated_labels_vertical_026" => {
            Some((
                "-179.8490447998047 -8 213.3490447998047 368.98809814453125",
                "213.3490447998047",
            ))
        }
        "upstream_cypress_gitgraph_spec_24_should_render_a_simple_gitgraph_with_horizontal_labels_vertic_027" => {
            Some(("-62.875 -8 96.375 246", "96.375"))
        }
        "upstream_cypress_gitgraph_spec_31_should_render_a_simple_gitgraph_with_a_title_vertical_branch_034" => {
            Some((
                "-86.49547576904297 -50 143.734375 146.42852783203125",
                "143.734375",
            ))
        }
        "upstream_cypress_gitgraph_spec_32_should_render_a_simple_gitgraph_overlapping_commits_vertical_035" => {
            Some((
                "-37.90840721130371 -8 190.0802764892578 446",
                "190.0802764892578",
            ))
        }
        "upstream_cypress_gitgraph_spec_35_should_render_a_simple_gitgraph_with_two_branches_from_same_c_038" => {
            Some((
                "-62.7565803527832 -8 360.381591796875 346",
                "360.381591796875",
            ))
        }
        "upstream_cypress_gitgraph_spec_37_should_render_gitgraph_with_branch_that_is_not_used_immediate_040" => {
            Some((
                "-62.7565803527832 -8 190.26438903808594 246",
                "190.26438903808594",
            ))
        }
        "upstream_cypress_gitgraph_spec_43_should_render_gitgraph_with_parallel_commits_vertical_branch_046" => {
            Some((
                "-62.7565803527832 -8 331.006591796875 254.42852783203125",
                "331.006591796875",
            ))
        }
        "upstream_cypress_gitgraph_spec_47_should_render_gitgraph_with_merge_back_and_merge_forward_vert_051" => {
            Some((
                "-62.7565803527832 -8 340.326904296875 246",
                "340.326904296875",
            ))
        }
        "upstream_cypress_gitgraph_spec_49_should_render_gitgraph_with_merge_on_a_new_branch_vertical_br_053" => {
            Some((
                "-62.7565803527832 -8 340.326904296875 246",
                "340.326904296875",
            ))
        }
        "upstream_cypress_gitgraph_spec_4_should_render_a_simple_gitgraph_with_tags_committypes_on_main_004" => {
            Some(("-96 -34.70000076293945 254 145.16966247558594", "254"))
        }
        "upstream_cypress_gitgraph_spec_50_should_render_a_simple_gitgraph_with_commit_on_main_branch_ve_054" => {
            Some(("-35.5 22 69 189", "69"))
        }
        "upstream_cypress_gitgraph_spec_54_should_render_a_simple_gitgraph_with_two_branches_vertical_br_058" => {
            Some(("-35.5 22 187.2109375 339", "187.2109375"))
        }
        "upstream_cypress_gitgraph_spec_55_should_render_a_simple_gitgraph_with_two_branches_and_merge_c_059" => {
            Some(("-35.5 22 187.2109375 389", "187.2109375"))
        }
        "upstream_cypress_gitgraph_spec_58_should_render_a_simple_gitgraph_with_rotated_labels_vertical_062" => {
            Some((
                "-179.8490447998047 22 213.3490447998047 338.87762451171875",
                "213.3490447998047",
            ))
        }
        "upstream_cypress_gitgraph_spec_59_should_render_a_simple_gitgraph_with_horizontal_labels_vertic_063" => {
            Some(("-62.875 20 96.375 241", "96.375"))
        }
        "upstream_cypress_gitgraph_spec_67_should_render_a_simple_gitgraph_overlapping_commits_vertical_071" => {
            Some((
                "-37.90840721130371 22 190.0802764892578 439",
                "190.0802764892578",
            ))
        }
        "upstream_cypress_gitgraph_spec_68_should_render_a_simple_gitgraph_with_two_branches_from_same_c_072" => {
            Some((
                "-62.7565803527832 22 360.381591796875 339",
                "360.381591796875",
            ))
        }
        "upstream_cypress_gitgraph_spec_69_should_render_gitgraph_with_branch_that_is_not_used_immediate_073" => {
            Some((
                "-62.7565803527832 22 190.26438903808594 239",
                "190.26438903808594",
            ))
        }
        "upstream_cypress_gitgraph_spec_71_should_render_gitgraph_with_parallel_commits_vertical_branch_075" => {
            Some((
                "-62.7565803527832 2 331.006591796875 239",
                "331.006591796875",
            ))
        }
        "upstream_cypress_gitgraph_spec_75_should_render_a_gitgraph_with_multiple_tags_on_a_merge_commit_079" => {
            Some((
                "-67.54059982299805 22 219.2515411376953 389",
                "219.2515411376953",
            ))
        }
        "upstream_cypress_gitgraph_spec_87_should_show_branches_with_tb_orientation_when_showbranches_is_091" => {
            Some(("-35.5 -8 147.2109375 296", "147.2109375"))
        }
        "upstream_cypress_gitgraph_spec_89_should_show_commit_labels_with_bt_orientation_when_showcommit_093" => {
            Some(("-35.5 20 147.2109375 291", "147.2109375"))
        }
        "upstream_cypress_gitgraph_spec_90_should_hide_commit_labels_with_bt_orientation_when_showcommit_094" => {
            Some(("-35.5 22 147.2109375 289", "147.2109375"))
        }
        "upstream_direction_tb" => Some((
            "-64.29232788085938 -8 97.79232788085938 105.9283676147461",
            "97.79232788085938",
        )),
        "upstream_direction_bt" => Some((
            "-64.29232788085938 22 97.79232788085938 89",
            "97.79232788085938",
        )),
        "upstream_docs_gitgraph_bottom_to_top_bt_v11_0_0_039" => Some((
            "-64.08240509033203 22 215.7933349609375 489",
            "215.7933349609375",
        )),
        "upstream_docs_gitgraph_top_to_bottom_tb_037" => Some((
            "-64.08240509033203 -8 215.7933349609375 503.1993713378906",
            "215.7933349609375",
        )),
        "upstream_pkgtests_gitgraph_spec_066" => Some(("-35.5 -8 69 46", "69")),
        "upstream_pkgtests_gitgraph_spec_071" => Some(("-35.5 -8 69 46", "69")),
        "upstream_pkgtests_gitgraph_test_025" => Some((
            "-42.04056167602539 -8 192.07962036132812 255.8954315185547",
            "192.07962036132812",
        )),
        _ => None,
    }
}
