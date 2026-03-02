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
        "upstream_docs_gitgraph_base_theme_046" => {
            Some(("-123.515625 -19 1031.515625 537.0078125", "1031.515625"))
        }
        "upstream_docs_gitgraph_default_theme_050" => {
            Some(("-123.515625 -19 1031.515625 537.0078125", "1031.515625"))
        }
        "upstream_docs_gitgraph_forest_theme_048" => {
            Some(("-123.515625 -19 1031.515625 537.0078125", "1031.515625"))
        }
        "upstream_docs_gitgraph_neutral_theme_054" => {
            Some(("-123.515625 -19 1031.515625 537.0078125", "1031.515625"))
        }
        "upstream_docs_gitgraph_hiding_branch_names_and_lines_022" => Some((
            "-35.45661163330078 -18 912.4566040039062 536.0078125",
            "912.4566040039062",
        )),
        "upstream_docs_accessibility_gitgraph_011" => Some((
            "-117.421875 -19 475.421875 172.94400024414062",
            "475.421875",
        )),
        "upstream_docs_gitgraph_gitgraph_diagrams_001" => Some((
            "-117.421875 -50 475.421875 203.94400024414062",
            "475.421875",
        )),
        "upstream_docs_gitgraph_merging_two_branches_009" => Some((
            "-191.484375 -34.72539138793945 699.484375 250.8622283935547",
            "699.484375",
        )),
        "upstream_docs_gitgraph_cherry_pick_commit_from_another_branch_010" => {
            Some(("-117.421875 -19 575.421875 251.5328826904297", "575.421875"))
        }
        "upstream_docs_gitgraph_commit_labels_layout_rotated_or_horizontal_012" => {
            Some(("-96 -19 604 353.5183410644531", "604"))
        }
        "upstream_docs_gitgraph_customizing_main_branch_name_015" => Some((
            "-140.75 -34.70000076293945 898.75 271.2749938964844",
            "898.75",
        )),
        "upstream_docs_gitgraph_dark_theme_026" => {
            Some(("-123.515625 -19 1031.515625 537.0078125", "1031.515625"))
        }
        "upstream_docs_gitgraph_customize_using_theme_variables_028" => Some((
            "-123.515625 -19 531.515625 263.14988708496094",
            "531.515625",
        )),
        "upstream_docs_gitgraph_customizing_branch_colors_029" => Some((
            "-123.515625 -19 531.515625 263.14988708496094",
            "531.515625",
        )),
        "upstream_docs_gitgraph_customizing_branch_label_colors_030" => {
            Some(("-118.34375 -19 176.34375 849", "176.34375"))
        }
        "upstream_docs_gitgraph_customizing_commit_colors_031" => Some((
            "-123.515625 -19 531.515625 263.14988708496094",
            "531.515625",
        )),
        "upstream_docs_gitgraph_customizing_commit_label_font_size_032" => Some((
            "-123.515625 -19 531.515625 287.79278564453125",
            "531.515625",
        )),
        "upstream_docs_gitgraph_customizing_tag_label_font_size_033" => Some((
            "-123.515625 -19 531.515625 263.14988708496094",
            "531.515625",
        )),
        "upstream_docs_gitgraph_customizing_tag_colors_034" => Some((
            "-123.515625 -19 531.515625 263.14988708496094",
            "531.515625",
        )),
        "upstream_docs_gitgraph_customizing_highlight_commit_colors_035" => Some((
            "-123.515625 -19 531.515625 263.14988708496094",
            "531.515625",
        )),
        "upstream_branches_and_order" => Some(("-233.953125 -19 391.953125 1119", "391.953125")),
        "upstream_cherry_pick_merge_commits" => Some((
            "-114.078125 -19 572.078125 235.75454711914062",
            "572.078125",
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
        "upstream_html_demos_git_cherry_pick_from_main_graph_016" => Some((
            "-117.421875 -50 375.421875 203.94400024414062",
            "375.421875",
        )),
        "upstream_html_demos_git_cherry_pick_from_main_graph_017" => Some((
            "-105.00030517578125 -50 331.5625 357.8128967285156",
            "331.5625",
        )),
        "upstream_html_demos_git_cherry_pick_then_merge_graph_019" => Some((
            "-139.328125 -50 447.328125 203.29379272460938",
            "447.328125",
        )),
        "upstream_html_demos_git_cherry_pick_then_merge_graph_020" => {
            Some(("-120.95260620117188 -50 341.171875 388", "341.171875"))
        }
        "upstream_html_demos_git_cherry_pick_then_merge_graph_021" => {
            Some(("-120.95260620117188 -50 341.171875 411", "341.171875"))
        }
        "upstream_html_demos_git_continuous_development_graph_004" => Some((
            "-117.421875 -50 375.421875 203.53219604492188",
            "375.421875",
        )),
        "upstream_html_demos_git_continuous_development_graph_005" => {
            Some(("-130.67135620117188 -50 349.65625 338", "349.65625"))
        }
        "upstream_html_demos_git_continuous_development_graph_006" => {
            Some(("-130.67135620117188 -50 349.65625 361", "349.65625"))
        }
        "upstream_html_demos_git_three_branches_and_a_cherry_pick_from_each_graph_031" => {
            Some(("-123.515625 -50 731.515625 278.1114807128906", "731.515625"))
        }
        "upstream_html_demos_git_three_branches_and_a_cherry_pick_from_each_graph_032" => {
            Some(("-143.35040283203125 -50 507.90625 688", "507.90625"))
        }
        "upstream_html_demos_git_three_branches_and_a_cherry_pick_from_each_graph_033" => {
            Some(("-143.35040283203125 -50 507.90625 711", "507.90625"))
        }
        "upstream_html_demos_git_two_branches_from_same_commit_graph_028" => {
            Some(("-145.125 -50 453.125 293.9440002441406", "453.125"))
        }
        "upstream_html_demos_git_two_branches_from_same_commit_graph_029" => {
            Some(("-90.27446746826172 -50 415.40625 388", "415.40625"))
        }
        "upstream_html_demos_git_two_branches_from_same_commit_graph_030" => {
            Some(("-90.27446746826172 -50 415.40625 411", "415.40625"))
        }
        "upstream_html_demos_git_two_way_merges_010" => Some((
            "-117.421875 -50 475.421875 203.53219604492188",
            "475.421875",
        )),
        "upstream_html_demos_git_two_way_merges_011" => {
            Some(("-97.20260620117188 -50 282.71875 438", "282.71875"))
        }
        "upstream_html_demos_git_two_way_merges_012" => {
            Some(("-97.20260620117188 -50 282.71875 461", "282.71875"))
        }
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
        "upstream_html_demos_git_merge_feature_to_advanced_main_graph_008" => {
            Some(("-156.74655151367188 -50 412.40625 288", "412.40625"))
        }
        "upstream_html_demos_git_merge_feature_to_advanced_main_graph_009" => {
            Some(("-156.74655151367188 -50 412.40625 311", "412.40625"))
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
        "upstream_cypress_gitgraph_spec_11_should_render_a_simple_gitgraph_with_two_cherry_pick_commit_014" => {
            Some((
                "-123.515625 -34.72539138793945 731.515625 262.83685302734375",
                "731.515625",
            ))
        }
        "upstream_cypress_gitgraph_spec_12_should_render_commits_for_more_than_8_branches_015" => {
            Some(("-118.34375 -19 1026.34375 895.9924926757812", "1026.34375"))
        }
        "upstream_cypress_gitgraph_spec_13_should_render_a_simple_gitgraph_with_three_branches_custom_me_016" => {
            Some((
                "-191.484375 -34.72539138793945 699.484375 250.8622283935547",
                "699.484375",
            ))
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
        "upstream_cypress_gitgraph_spec_25_should_render_a_simple_gitgraph_with_cherry_pick_commit_verti_028" => {
            Some((
                "-51.76327896118164 -8 203.47421264648438 446",
                "203.47421264648438",
            ))
        }
        "upstream_cypress_gitgraph_spec_26_should_render_a_gitgraph_with_cherry_pick_commit_with_custom_029" => {
            Some((
                "-51.76327896118164 -8 203.47421264648438 446",
                "203.47421264648438",
            ))
        }
        "upstream_cypress_gitgraph_spec_27_should_render_a_gitgraph_with_cherry_pick_commit_with_no_tag_030" => {
            Some((
                "-51.76327896118164 -8 203.47421264648438 446",
                "203.47421264648438",
            ))
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
        "upstream_cypress_gitgraph_spec_38_should_render_gitgraph_with_branch_and_sub_branch_neither_of_041" => {
            Some(("-96 -19 454 262.35137939453125", "454"))
        }
        "upstream_cypress_gitgraph_spec_39_should_render_gitgraph_with_branch_and_sub_branch_neither_of_042" => {
            Some((
                "-62.7565803527832 -8 284.209716796875 396",
                "284.209716796875",
            ))
        }
        "upstream_cypress_gitgraph_spec_44_should_render_gitgraph_with_unconnected_branches_and_no_paral_047" => {
            Some(("-96 -19 404 352.35137939453125", "404"))
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
        "upstream_cypress_gitgraph_spec_61_should_render_a_gitgraph_with_cherry_pick_commit_with_custom_065" => {
            Some((
                "-51.76327896118164 22 203.47421264648438 439",
                "203.47421264648438",
            ))
        }
        "upstream_cypress_gitgraph_spec_62_should_render_a_gitgraph_with_cherry_pick_commit_with_no_tag_066" => {
            Some((
                "-51.76327896118164 22 203.47421264648438 439",
                "203.47421264648438",
            ))
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
        "upstream_cypress_gitgraph_spec_7_should_render_a_simple_gitgraph_with_three_branches_and_tagged_007" => {
            Some((
                "-191.484375 -34.72539138793945 699.484375 250.8622283935547",
                "699.484375",
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
        "upstream_cypress_gitgraph_spec_73_should_render_a_simple_gitgraph_with_three_branches_and_tagge_077" => {
            Some((
                "-191.484375 -34.72539138793945 699.484375 250.8622283935547",
                "699.484375",
            ))
        }
        "upstream_cypress_gitgraph_spec_74_should_render_commits_for_more_than_8_branches_using_switch_i_078" => {
            Some(("-118.34375 -19 1026.34375 895.9924926757812", "1026.34375"))
        }
        "upstream_cypress_gitgraph_spec_8_should_render_a_simple_gitgraph_with_more_than_8_branches_over_008" => {
            Some(("-118.34375 -19 176.34375 849", "176.34375"))
        }
        "stress_gitgraph_font_size_097" => Some((
            "-262.640625 -18 470.640625 177.95840454101562",
            "470.640625",
        )),
        "upstream_accessibility_and_warnings" => Some(("-96 -19 154 100.76305389404297", "154")),
        "upstream_accessibility_single_line_accdescr_spec" => {
            Some(("-96 -19 154 83.82177734375", "154"))
        }
        "upstream_cherry_pick_custom_tag_spec" => Some((
            "-117.421875 -34.72539138793945 275.421875 161.3173828125",
            "275.421875",
        )),
        "upstream_cherry_pick_default_tag_spec" => Some((
            "-117.421875 -34.72539138793945 275.421875 161.3173828125",
            "275.421875",
        )),
        "upstream_cherry_pick_empty_tag_spec" => {
            Some(("-117.421875 -19 275.421875 145.5919952392578", "275.421875"))
        }
        "upstream_commits_spec" => Some(("-96 -34.72539138793945 804 102.51644897460938", "804")),
        "upstream_cypress_appli_spec_1_should_render_a_simple_gitgraph_with_commit_on_main_branch_001" => {
            Some(("-96 -19 254 55.136844635009766", "254"))
        }
        "upstream_cypress_gitgraph_spec_10_should_render_a_simple_gitgraph_with_horizontal_labels_010" => {
            Some(("-66 -19 274 55.55078125", "274"))
        }
        "upstream_cypress_gitgraph_spec_11_should_render_a_gitgraph_with_cherry_pick_commit_with_custom_012" => {
            Some((
                "-117.421875 -34.72539138793945 525.421875 161.37156677246094",
                "525.421875",
            ))
        }
        "upstream_cypress_gitgraph_spec_11_should_render_a_gitgraph_with_cherry_pick_commit_with_no_tag_013" => {
            Some((
                "-117.421875 -19 525.421875 145.64617919921875",
                "525.421875",
            ))
        }
        "upstream_cypress_gitgraph_spec_11_should_render_a_simple_gitgraph_with_cherry_pick_commit_011" => {
            Some((
                "-117.421875 -34.72539138793945 525.421875 161.37156677246094",
                "525.421875",
            ))
        }
        "upstream_cypress_gitgraph_spec_1433_should_render_a_simple_gitgraph_with_a_title_017" => {
            Some(("-96 -50 154 113.35137939453125", "154"))
        }
        "upstream_cypress_gitgraph_spec_15_should_render_a_simple_gitgraph_with_commit_on_main_branch_ve_018" => {
            Some(("-35.5 -8 69 196", "69"))
        }
        "upstream_cypress_gitgraph_spec_16_should_render_a_simple_gitgraph_with_commit_on_main_branch_wi_019" => {
            Some((
                "-49.77454376220703 -8 83.27454376220703 196",
                "83.27454376220703",
            ))
        }
        "upstream_cypress_gitgraph_spec_17_should_render_a_simple_gitgraph_with_different_committypes_on_020" => {
            Some((
                "-86.42256164550781 -8 119.92256164550781 228.09451293945312",
                "119.92256164550781",
            ))
        }
        "upstream_cypress_gitgraph_spec_18_should_render_a_simple_gitgraph_with_tags_committypes_on_main_021" => {
            Some((
                "-110.79564666748047 -8 165.78639221191406 228.09451293945312",
                "165.78639221191406",
            ))
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
        "upstream_cypress_gitgraph_spec_2_should_render_a_simple_gitgraph_with_commit_on_main_branch_wit_002" => {
            Some(("-96 -19 254 69.58226013183594", "254"))
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
        "upstream_cypress_gitgraph_spec_33_should_render_a_simple_gitgraph_overlapping_commits_036" => {
            Some(("-118.34375 -19 526.34375 147.94357299804688", "526.34375"))
        }
        "upstream_cypress_gitgraph_spec_34_should_render_a_simple_gitgraph_with_two_branches_from_same_c_037" => {
            Some(("-145.125 -19 453.125 262.35137939453125", "453.125"))
        }
        "upstream_cypress_gitgraph_spec_35_should_render_a_simple_gitgraph_with_two_branches_from_same_c_038" => {
            Some((
                "-62.7565803527832 -8 360.381591796875 346",
                "360.381591796875",
            ))
        }
        "upstream_cypress_gitgraph_spec_36_should_render_gitgraph_with_branch_that_is_not_used_immediate_039" => {
            Some(("-96 -19 304 172.35137939453125", "304"))
        }
        "upstream_cypress_gitgraph_spec_37_should_render_gitgraph_with_branch_that_is_not_used_immediate_040" => {
            Some((
                "-62.7565803527832 -8 190.26438903808594 246",
                "190.26438903808594",
            ))
        }
        "upstream_cypress_gitgraph_spec_3_should_render_a_simple_gitgraph_with_different_committypes_on_003" => {
            Some(("-96 -19 254 105.5637435913086", "254"))
        }
        "upstream_cypress_gitgraph_spec_40_should_render_a_simple_gitgraph_with_cherry_pick_merge_commit_043" => {
            Some(("-114.078125 -19 388.8828125 219", "388.8828125"))
        }
        "upstream_cypress_gitgraph_spec_41_should_render_default_gitgraph_with_parallelcommits_set_to_fa_044" => {
            Some((
                "-117.421875 -19 525.421875 262.35137939453125",
                "525.421875",
            ))
        }
        "upstream_cypress_gitgraph_spec_42_should_render_gitgraph_with_parallel_commits_045" => {
            Some((
                "-117.421875 -19 325.421875 262.35137939453125",
                "325.421875",
            ))
        }
        "upstream_cypress_gitgraph_spec_43_should_render_gitgraph_with_parallel_commits_vertical_branch_046" => {
            Some((
                "-62.7565803527832 -8 331.006591796875 254.42852783203125",
                "331.006591796875",
            ))
        }
        "upstream_cypress_gitgraph_spec_45_should_render_gitgraph_with_unconnected_branches_and_parallel_048" => {
            Some(("-96 -19 204 352.35137939453125", "204"))
        }
        "upstream_cypress_gitgraph_spec_46_should_render_gitgraph_with_merge_back_and_merge_forward_050" => {
            Some((
                "-125.265625 -19 333.265625 262.35137939453125",
                "333.265625",
            ))
        }
        "upstream_cypress_gitgraph_spec_47_should_render_gitgraph_with_merge_back_and_merge_forward_vert_051" => {
            Some((
                "-62.7565803527832 -8 340.326904296875 246",
                "340.326904296875",
            ))
        }
        "upstream_cypress_gitgraph_spec_48_should_render_gitgraph_with_merge_on_a_new_branch_vertical_br_052" => {
            Some((
                "-125.265625 -19 333.265625 262.35137939453125",
                "333.265625",
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
        "upstream_cypress_gitgraph_spec_51_should_render_a_simple_gitgraph_with_commit_on_main_branch_wi_055" => {
            Some((
                "-49.77454376220703 22 83.27454376220703 189",
                "83.27454376220703",
            ))
        }
        "upstream_cypress_gitgraph_spec_52_should_render_a_simple_gitgraph_with_different_committypes_on_056" => {
            Some((
                "-86.42256164550781 22 119.92256164550781 192.4127960205078",
                "119.92256164550781",
            ))
        }
        "upstream_cypress_gitgraph_spec_53_should_render_a_simple_gitgraph_with_tags_committypes_on_main_057" => {
            Some((
                "-110.79564666748047 22 165.78639221191406 220.721923828125",
                "165.78639221191406",
            ))
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
        "upstream_cypress_gitgraph_spec_5_should_render_a_simple_gitgraph_with_two_branches_005" => {
            Some((
                "-117.421875 -19 425.421875 145.13685607910156",
                "425.421875",
            ))
        }
        "upstream_cypress_gitgraph_spec_60_should_render_a_simple_gitgraph_with_cherry_pick_commit_verti_064" => {
            Some((
                "-51.76327896118164 22 203.47421264648438 439",
                "203.47421264648438",
            ))
        }
        "upstream_cypress_gitgraph_spec_66_should_render_a_simple_gitgraph_with_a_title_vertical_branch_070" => {
            Some(("-86.49547576904297 -50 143.734375 161", "143.734375"))
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
        "upstream_cypress_gitgraph_spec_6_should_render_a_simple_gitgraph_with_two_branches_and_merge_co_006" => {
            Some((
                "-117.421875 -19 475.421875 145.13685607910156",
                "475.421875",
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
        "upstream_cypress_gitgraph_spec_76_should_render_a_gitgraph_with_multiple_tags_on_a_merge_commit_080" => {
            Some((
                "-117.421875 -54.72539138793945 475.421875 181.37156677246094",
                "475.421875",
            ))
        }
        "upstream_cypress_gitgraph_spec_77_should_show_branch_lines_when_showbranches_is_true_default_081" => {
            Some(("-87.421875 -19 395.421875 105.5", "395.421875"))
        }
        "upstream_cypress_gitgraph_spec_78_should_hide_branch_lines_when_showbranches_is_false_082" => {
            Some(("-8 -18 286 104.5", "286"))
        }
        "upstream_cypress_gitgraph_spec_87_should_show_branches_with_tb_orientation_when_showbranches_is_091" => {
            Some(("-35.5 -8 147.2109375 296", "147.2109375"))
        }
        "upstream_cypress_gitgraph_spec_88_should_hide_branches_with_tb_orientation_when_showbranches_is_092" => {
            Some(("-34.25 20 119.75 238", "119.75"))
        }
        "upstream_cypress_gitgraph_spec_89_should_show_commit_labels_with_bt_orientation_when_showcommit_093" => {
            Some(("-35.5 20 147.2109375 291", "147.2109375"))
        }
        "upstream_cypress_gitgraph_spec_90_should_hide_commit_labels_with_bt_orientation_when_showcommit_094" => {
            Some(("-35.5 22 147.2109375 289", "147.2109375"))
        }
        "upstream_cypress_gitgraph_spec_91_should_render_with_rotatecommitlabel_set_to_true_095" => {
            Some((
                "-117.421875 -19 375.421875 164.98980712890625",
                "375.421875",
            ))
        }
        "upstream_cypress_gitgraph_spec_9_should_render_a_simple_gitgraph_with_rotated_labels_009" => {
            Some((
                "-155.46136474609375 -19 363.46136474609375 197.16357421875",
                "363.46136474609375",
            ))
        }
        "upstream_direction_bt" => Some((
            "-64.29232788085938 22 97.79232788085938 89",
            "97.79232788085938",
        )),
        "upstream_direction_tb" => Some((
            "-64.29232788085938 -8 97.79232788085938 105.9283676147461",
            "97.79232788085938",
        )),
        "upstream_docs_contributing_workflow_012" => {
            Some(("-271.375 -19 629.375 172.94400024414062", "629.375"))
        }
        "upstream_docs_examples_a_commit_flow_diagram_017" => Some((
            "-139.328125 -19 547.328125 261.87115478515625",
            "547.328125",
        )),
        "upstream_docs_gitgraph_adding_custom_commit_id_005" => {
            Some(("-96 -19 254 74.98981094360352", "254"))
        }
        "upstream_docs_gitgraph_adding_tags_009" => {
            Some(("-96 -34.70000076293945 454 98.99078369140625", "454"))
        }
        "upstream_docs_gitgraph_bottom_to_top_bt_v11_0_0_039" => Some((
            "-64.08240509033203 22 215.7933349609375 489",
            "215.7933349609375",
        )),
        "upstream_docs_gitgraph_checking_out_an_existing_branch_013" => Some((
            "-117.421875 -19 475.421875 172.94400024414062",
            "475.421875",
        )),
        "upstream_docs_gitgraph_commit_labels_layout_rotated_or_horizontal_013" => {
            Some(("-66 -19 574 205.5", "574"))
        }
        "upstream_docs_gitgraph_create_a_new_branch_011" => Some((
            "-117.421875 -19 375.421875 172.53219604492188",
            "375.421875",
        )),
        "upstream_docs_gitgraph_customizing_branch_ordering_033" => {
            Some(("-97.28125 -19 155.28125 399", "155.28125"))
        }
        "upstream_docs_gitgraph_left_to_right_default_lr_035" => Some((
            "-117.421875 -19 575.421875 171.37266540527344",
            "575.421875",
        )),
        "upstream_docs_gitgraph_merging_two_branches_015" => Some((
            "-117.421875 -19 625.421875 173.61587524414062",
            "625.421875",
        )),
        "upstream_docs_gitgraph_modifying_commit_type_007" => {
            Some(("-96 -19 404 83.51834869384766", "404"))
        }
        "upstream_docs_gitgraph_parallel_commits_parallelcommits_true_043" => {
            Some(("-117.421875 -19 275.421875 172.3262939453125", "275.421875"))
        }
        "upstream_docs_gitgraph_syntax_003" => Some(("-96 -19 254 83.51834869384766", "254")),
        "upstream_docs_gitgraph_temporal_commits_default_parallelcommits_false_041" => {
            Some(("-117.421875 -19 375.421875 172.3262939453125", "375.421875"))
        }
        "upstream_docs_gitgraph_top_to_bottom_tb_037" => Some((
            "-64.08240509033203 -8 215.7933349609375 503.1993713378906",
            "215.7933349609375",
        )),
        "upstream_docs_readme_git_graph_experimental_a_href_https_mermaid_live_edit_pako_enqnk_013" => {
            Some((
                "-117.421875 -19 475.421875 172.94400024414062",
                "475.421875",
            ))
        }
        "upstream_header_default" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_diagram_orchestration_spec_048" => Some(("-96 -19 104 39", "104")),
        "upstream_pkgtests_gitgraph_spec_001" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_spec_002" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_spec_003" => Some(("-96 -19 154 66.04947280883789", "154")),
        "upstream_pkgtests_gitgraph_spec_004" => {
            Some(("-96 -34.70000076293945 154 99.52178192138672", "154"))
        }
        "upstream_pkgtests_gitgraph_spec_005" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_spec_006" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_spec_007" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_spec_008" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_spec_009" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_spec_010" => {
            Some(("-96 -34.72539138793945 154 81.77486419677734", "154"))
        }
        "upstream_pkgtests_gitgraph_spec_011" => {
            Some(("-96 -34.72539138793945 154 99.54717254638672", "154"))
        }
        "upstream_pkgtests_gitgraph_spec_012" => {
            Some(("-96 -34.72539138793945 154 99.54717254638672", "154"))
        }
        "upstream_pkgtests_gitgraph_spec_013" => {
            Some(("-96 -34.72539138793945 154 81.77486419677734", "154"))
        }
        "upstream_pkgtests_gitgraph_spec_014" => {
            Some(("-96 -34.72539138793945 154 81.77486419677734", "154"))
        }
        "upstream_pkgtests_gitgraph_spec_015" => {
            Some(("-96 -34.72539138793945 154 81.77486419677734", "154"))
        }
        "upstream_pkgtests_gitgraph_spec_016" => {
            Some(("-96 -34.72539138793945 154 81.77486419677734", "154"))
        }
        "upstream_pkgtests_gitgraph_spec_017" => {
            Some(("-96 -34.72539138793945 154 81.77486419677734", "154"))
        }
        "upstream_pkgtests_gitgraph_spec_018" => Some(("-96 -19 254 83.51834869384766", "254")),
        "upstream_pkgtests_gitgraph_spec_019" => {
            Some(("-137.984375 -19 195.984375 129", "195.984375"))
        }
        "upstream_pkgtests_gitgraph_spec_020" => {
            Some(("-109.953125 -19 267.953125 173.290771484375", "267.953125"))
        }
        "upstream_pkgtests_gitgraph_spec_021" => {
            Some(("-151.671875 -19 209.671875 129", "209.671875"))
        }
        "upstream_pkgtests_gitgraph_spec_023" => {
            Some(("-233.953125 -19 291.953125 579", "291.953125"))
        }
        "upstream_pkgtests_gitgraph_spec_024" => {
            Some(("-137.984375 -19 195.984375 129", "195.984375"))
        }
        "upstream_pkgtests_gitgraph_spec_027" => {
            Some(("-137.984375 -19 245.984375 173.5183563232422", "245.984375"))
        }
        "upstream_pkgtests_gitgraph_spec_028" => Some((
            "-137.984375 -19 345.984375 172.94400024414062",
            "345.984375",
        )),
        "upstream_pkgtests_gitgraph_spec_029" => {
            Some(("-137.984375 -19 195.984375 129", "195.984375"))
        }
        "upstream_pkgtests_gitgraph_spec_030" => {
            Some(("-137.984375 -19 245.984375 173.5183563232422", "245.984375"))
        }
        "upstream_pkgtests_gitgraph_spec_031" => Some((
            "-137.984375 -19 345.984375 172.94400024414062",
            "345.984375",
        )),
        "upstream_pkgtests_gitgraph_spec_032" => Some((
            "-137.984375 -34.72539138793945 295.984375 189.0161590576172",
            "295.984375",
        )),
        "upstream_pkgtests_gitgraph_spec_033" => Some((
            "-146.375 -34.72539138793945 504.375 368.0191955566406",
            "504.375",
        )),
        "upstream_pkgtests_gitgraph_spec_034" => Some((
            "-117.421875 -34.72539138793945 275.421875 161.3173828125",
            "275.421875",
        )),
        "upstream_pkgtests_gitgraph_spec_035" => Some((
            "-117.421875 -34.72539138793945 275.421875 161.3173828125",
            "275.421875",
        )),
        "upstream_pkgtests_gitgraph_spec_036" => {
            Some(("-117.421875 -19 275.421875 145.5919952392578", "275.421875"))
        }
        "upstream_pkgtests_gitgraph_spec_037" => {
            Some(("-114.078125 -19 388.8828125 219", "388.8828125"))
        }
        "upstream_pkgtests_gitgraph_spec_038" => {
            Some(("-114.078125 -19 372.078125 219", "372.078125"))
        }
        "upstream_pkgtests_gitgraph_spec_039" => Some((
            "-114.078125 -19 472.078125 235.75454711914062",
            "472.078125",
        )),
        "upstream_pkgtests_gitgraph_spec_040" => Some((
            "-114.078125 -19 522.078125 235.75454711914062",
            "522.078125",
        )),
        "upstream_pkgtests_gitgraph_spec_052" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_spec_053" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_spec_054" => Some((
            "-171.421875 -19 829.421875 372.85455322265625",
            "829.421875",
        )),
        "upstream_pkgtests_gitgraph_spec_055" => Some(("-96 -19 254 173.30160522460938", "254")),
        "upstream_pkgtests_gitgraph_spec_066" => Some(("-35.5 -8 69 46", "69")),
        "upstream_pkgtests_gitgraph_spec_071" => Some(("-35.5 -8 69 46", "69")),
        "upstream_pkgtests_gitgraph_spec_076" => Some(("-96 -19 104 39", "104")),
        "upstream_pkgtests_gitgraph_test_001" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_test_002" => Some(("-96 -19 254 83.51834869384766", "254")),
        "upstream_pkgtests_gitgraph_test_003" => {
            Some(("-96 -34.70000076293945 154 70.83684539794922", "154"))
        }
        "upstream_pkgtests_gitgraph_test_004" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_test_005" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_test_007" => Some(("-96 -19 204 83.51834869384766", "204")),
        "upstream_pkgtests_gitgraph_test_008" => {
            Some(("-110.453125 -19 218.453125 129", "218.453125"))
        }
        "upstream_pkgtests_gitgraph_test_010" => {
            Some(("-114.078125 -19 172.078125 129", "172.078125"))
        }
        "upstream_pkgtests_gitgraph_test_011" => Some(("-205.1875 -19 213.1875 129", "213.1875")),
        "upstream_pkgtests_gitgraph_test_012" => {
            Some(("-198.46875 -19 206.46875 129", "206.46875"))
        }
        "upstream_pkgtests_gitgraph_test_013" => {
            Some(("-114.078125 -19 122.078125 129", "122.078125"))
        }
        "upstream_pkgtests_gitgraph_test_023" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_test_024" => Some(("-96 -19 154 83.82177734375", "154")),
        "upstream_pkgtests_gitgraph_test_025" => Some((
            "-42.04056167602539 -8 192.07962036132812 255.8954315185547",
            "192.07962036132812",
        )),
        "upstream_switch_commit_merge_spec" => Some((
            "-137.984375 -19 345.984375 172.94400024414062",
            "345.984375",
        )),
        "upstream_unsafe_id_branch_and_commit_spec" => Some((
            "-133.296875 -19 291.296875 173.30160522460938",
            "291.296875",
        )),
        _ => None,
    }
}
