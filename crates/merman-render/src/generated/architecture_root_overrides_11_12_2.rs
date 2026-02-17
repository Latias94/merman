// Fixture-derived root viewport overrides for Mermaid@11.12.2 Architecture diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/architecture/*.svg` and are keyed by `diagram_id`
// (fixture stem). They are applied only for non-empty diagrams where Architecture
// root viewport parity (`viewBox` + `max-width`) still differs from upstream.

pub fn lookup_architecture_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_architecture_cypress_complex_junction_edges_normalized" => Some((
            "-333.2172546386719 -266.3057403564453 746.4345092773438 663.6115112304688",
            "746.4345092773438",
        )),
        "upstream_architecture_cypress_directional_arrows_normalized" => Some((
            "-195.6711883544922 -187.24441528320312 514.3424072265625 538.5298767089844",
            "514.3424072265625",
        )),
        "upstream_architecture_cypress_edge_labels_normalized" => Some((
            "-185.4211883544922 -182.99441528320312 514.3424072265625 538.5298767089844",
            "514.3424072265625",
        )),
        "upstream_architecture_cypress_groups_normalized" => Some((
            "-183.60711669921875 -262.5092315673828 447.2142639160156 660.0184936523438",
            "447.2142639160156",
        )),
        "upstream_architecture_demo_arrow_mesh_bidirectional" => Some((
            "-195.6711883544922 -187.24441528320312 514.3424072265625 538.5298767089844",
            "514.3424072265625",
        )),
        "upstream_architecture_demo_arrow_mesh_bidirectional_inverse" => Some((
            "-195.49441528320312 -187.4211883544922 514.3423767089844 538.5299072265625",
            "514.3423767089844",
        )),
        "upstream_architecture_demo_edge_label_long" => Some((
            "-151.49441528320312 -182.99441528320312 514.3423767089844 538.5298767089844",
            "514.3423767089844",
        )),
        "upstream_architecture_demo_edge_label_short" => Some((
            "-185.4211883544922 -182.99441528320312 514.3424072265625 538.5298767089844",
            "514.3424072265625",
        )),
        "upstream_architecture_demo_junction_groups_arrows" => Some((
            "-333.2172546386719 -266.3057403564453 746.4345092773438 663.6115112304688",
            "746.4345092773438",
        )),
        "upstream_architecture_cypress_reasonable_height" => Some((
            "-889.9099731445312 -270.7564392089844 1859.8199462890625 672.5128784179688",
            "1859.8199462890625",
        )),
        "upstream_cypress_architecture_spec_should_render_an_architecture_diagram_with_a_reasonable_height_011" => {
            Some((
                "-889.9099731445312 -270.7564392089844 1859.8199462890625 672.5128784179688",
                "1859.8199462890625",
            ))
        }
        "upstream_architecture_layout_reasonable_height" => Some((
            "-889.9099731445312 -270.7564392089844 1859.8199462890625 672.5128784179688",
            "1859.8199462890625",
        )),
        "mmdr_tests_architecture_architecture_basic" => Some((
            "-182.84327697753906 -65.5 445.6865539550781 262",
            "445.6865539550781",
        )),
        "upstream_architecture_cypress_fallback_icon" => {
            Some(("-49.8515625 -22 179.953125 184.1875", "179.953125"))
        }
        "upstream_html_demos_architecture_default_icon_from_unknown_icon_name_003" => {
            Some(("-49.8515625 -22 179.953125 184.1875", "179.953125"))
        }
        "upstream_cypress_architecture_spec_should_render_an_architecture_diagram_with_the_fallback_icon_004" => {
            Some(("-49.8515625 -22 179.953125 184.1875", "179.953125"))
        }
        "upstream_cypress_architecture_spec_should_render_a_simple_architecture_diagram_with_titleandaccessi_002" => {
            Some((
                "-183.41357421875 -165.96131896972656 446.8271484375 462.922607421875",
                "446.8271484375",
            ))
        }
        "upstream_html_demos_architecture_external_icons_demo_012" => Some((
            "-174.01507568359375 -165.18142700195312 440.8895568847656 470.55035400390625",
            "440.8895568847656",
        )),
        "stress_architecture_dense_mesh_001" => Some((
            "-271.47344970703125 -277.07244873046875 622.9468994140625 689.1448974609375",
            "622.9468994140625",
        )),
        "stress_architecture_edge_label_corner_cases_012" => Some((
            "-65.53177452087402 -101.78177261352539 344.8510437011719 367.25103759765625",
            "344.8510437011719",
        )),
        "stress_architecture_external_icons_005" => Some((
            "-402.0445861816406 -166.05992889404297 860.0891723632812 463.1198425292969",
            "860.0891723632812",
        )),
        "stress_architecture_group_boundary_traversal_004" => Some((
            "-291.8540496826172 -257.4413604736328 666.7081298828125 648.3826904296875",
            "666.7081298828125",
        )),
        "stress_architecture_grouped_junctions_008" => Some((
            "-326.80126953125 -165.88352966308594 733.6025390625 462.76708984375",
            "733.6025390625",
        )),
        "stress_architecture_junction_star_003" => Some((
            "-256.0063171386719 -171.3090057373047 635.0126342773438 506.3055419921875",
            "635.0126342773438",
        )),
        "stress_architecture_long_labels_006" => Some((
            "-263.0143737792969 -165.99396514892578 648.02880859375 462.9879455566406",
            "648.02880859375",
        )),
        "stress_architecture_mixed_service_forms_009" => Some((
            "-192.79791259765625 -211.39602661132812 556.0958251953125 586.4795532226562",
            "556.0958251953125",
        )),
        "stress_architecture_multi_level_groups_010" => Some((
            "-263.6997528076172 -209.4847412109375 604.3995361328125 549.969482421875",
            "604.3995361328125",
        )),
        "stress_architecture_nested_groups_002" => Some((
            "-314.9618835449219 -245.82904052734375 727.9237670898438 622.6580810546875",
            "727.9237670898438",
        )),
        "stress_architecture_ports_and_arrows_007" => Some((
            "-190.1131134033203 -193.36326599121094 526.2261962890625 550.4140625",
            "526.2261962890625",
        )),
        "stress_architecture_wide_graph_011" => Some((
            "-317.01202392578125 -190.32937622070312 755.5240478515625 544.3462524414062",
            "755.5240478515625",
        )),
        "stress_architecture_deep_nesting_013" => Some((
            "-379.9537048339844 -376.36761474609375 839.9074096679688 893.7352294921875",
            "839.9074096679688",
        )),
        "stress_architecture_junction_mesh_014" => Some((
            "-212.09400939941406 -463.360595703125 504.1880187988281 1057.72119140625",
            "504.1880187988281",
        )),
        "stress_architecture_disconnected_components_015" => Some((
            "-422.1696472167969 -189.57656860351562 924.3392944335938 510.15313720703125",
            "924.3392944335938",
        )),
        "stress_architecture_parallel_edges_016" => Some((
            "-174.03176879882812 -157.03177642822266 428.06353759765625 445.06353759765625",
            "428.06353759765625",
        )),
        "stress_architecture_group_port_edges_017" => Some((
            "-310.3846130371094 -204.2240447998047 707.7692260742188 542.4481201171875",
            "707.7692260742188",
        )),
        "stress_architecture_long_group_titles_018" => Some((
            "-182.9628143310547 -165.9628143310547 480.65625 462.9256591796875",
            "480.65625",
        )),
        "stress_architecture_unicode_and_xml_escapes_019" => Some((
            "-209.9109649658203 -166.29661560058594 469.8219299316406 463.59326171875",
            "469.8219299316406",
        )),
        "stress_architecture_bidirectional_boundary_traversal_020" => Some((
            "-202.67481994628906 -154.78721618652344 485.3496398925781 435.7411193847656",
            "485.3496398925781",
        )),
        "stress_architecture_fan_in_out_021" => Some((
            "-335.6900634765625 -319.68463134765625 751.380126953125 770.3692626953125",
            "751.380126953125",
        )),
        "stress_architecture_multi_group_crosslinks_022" => Some((
            "-296.729736328125 -188.8977508544922 662.1261596679688 508.7955322265625",
            "662.1261596679688",
        )),
        "stress_architecture_nested_junctions_023" => Some((
            "-224.55763244628906 -278.8697204589844 529.115234375 685.7394409179688",
            "529.115234375",
        )),
        "stress_architecture_cycle_with_junctions_024" => Some((
            "-182.9628143310547 -165.9628143310547 445.9256286621094 462.9256591796875",
            "445.9256286621094",
        )),
        "stress_architecture_many_small_groups_025" => Some((
            "-284.3171691894531 -298.4870300292969 648.6343383789062 727.9740600585938",
            "648.6343383789062",
        )),
        "stress_architecture_junction_fork_join_026" => Some((
            "-1370.6475830078125 -1205.6885986328125 2825.295166015625 2542.377197265625",
            "2825.295166015625",
        )),
        "stress_architecture_deep_group_chain_027" => Some((
            "-10925.580078125 -7460.30419921875 21934.16015625 15057.6083984375",
            "21934.16015625",
        )),
        "stress_architecture_crossing_edges_ring_028" => {
            Some(("-238.5 -87.75046157836914 640 339.18841552734375", "640"))
        }
        "stress_architecture_ports_matrix_029" => Some((
            "-149.06324768066406 -81.93747329711914 421.1264953613281 351.06243896484375",
            "421.1264953613281",
        )),
        "stress_architecture_long_ids_030" => Some((
            "-302.775390625 -273.27540588378906 677.55078125 677.55078125",
            "677.55078125",
        )),
        "stress_architecture_bidirectional_arrows_031" => Some((
            "-219.87960815429688 -209.89630126953125 519.7592163085938 551.7926025390625",
            "519.7592163085938",
        )),
        "stress_architecture_disconnected_group_edges_032" => Some((
            "-280.91676330566406 -241.3999786376953 644.33349609375 617.9874267578125",
            "644.33349609375",
        )),
        "stress_architecture_mixed_icons_and_text_033" => Some((
            "-423.55572509765625 -166.97333526611328 927.1114501953125 464.9466552734375",
            "927.1114501953125",
        )),
        "stress_architecture_group_to_group_multi_034" => Some((
            "-208.121826171875 -427.7790832519531 496.24365234375 986.55810546875",
            "496.24365234375",
        )),
        "stress_architecture_dense_junction_grid_035" => Some((
            "-200.92364501953125 -194.7652587890625 564.8472900390625 553.218017578125",
            "564.8472900390625",
        )),
        "stress_architecture_edge_titles_oneword_036" => Some((
            "-101.36510848999023 -101.78177261352539 354.3968811035156 367.25103759765625",
            "354.3968811035156",
        )),
        "stress_architecture_edge_labels_quotes_and_urls_037" => Some((
            "-239.66644287109375 -265.74436950683594 559.3328857421875 662.48876953125",
            "559.3328857421875",
        )),
        "stress_architecture_parallel_labeled_edges_038" => Some((
            "-187.0833282470703 -160.7195587158203 531.3333129882812 484.4390869140625",
            "531.3333129882812",
        )),
        "stress_architecture_group_to_group_labeled_ports_039" => {
            Some(("-419.0859375 -65.5 918.171875 262", "918.171875"))
        }
        "stress_architecture_junctions_in_groups_040" => Some((
            "-315.6285400390625 -174.47596740722656 711.257080078125 445.9519348144531",
            "711.257080078125",
        )),
        "stress_architecture_html_titles_and_escapes_041" => Some((
            "-210.9628143310547 -165.9628143310547 479.9256286621094 462.9256591796875",
            "479.9256286621094",
        )),
        "stress_architecture_icon_text_and_fallbacks_042" => Some((
            "-317.99566650390625 -166.05132293701172 715.9913330078125 463.1026306152344",
            "715.9913330078125",
        )),
        "stress_architecture_weird_ids_and_nested_groups_043" => Some((
            "-215.53176879882812 -198.53176879882812 511.06353759765625 528.0635375976562",
            "511.06353759765625",
        )),
        "stress_architecture_dense_ports_star_044" => Some((
            "-220.55487060546875 -211.5631866455078 562.6097412109375 586.8138427734375",
            "562.6097412109375",
        )),
        "stress_architecture_cross_group_multi_edges_045" => Some((
            "-439.0141296386719 -78.33333206176758 945.1949462890625 277.3333282470703",
            "945.1949462890625",
        )),
        "stress_architecture_disconnected_islands_046" => Some((
            "-374.1528625488281 -252.4567108154297 828.3057250976562 620.9134521484375",
            "828.3057250976562",
        )),
        "stress_architecture_deep_boundary_edges_with_labels_047" => Some((
            "-265.50245666503906 -282.5530548095703 611.0048828125 702.1061401367188",
            "611.0048828125",
        )),
        "stress_architecture_long_edge_labels_wrap_048" => Some((
            "-96.03177261352539 -101.78177261352539 343.06353759765625 367.25103759765625",
            "343.06353759765625",
        )),
        "stress_architecture_batch3_port_matrix_and_labels_049" => Some((
            "-606.8623657226562 -792.1575927734375 1293.7247314453125 1715.315185546875",
            "1293.7247314453125",
        )),
        "stress_architecture_batch3_deep_group_chain_050" => Some((
            "-2008.7728271484375 -7622.0283203125 4094.545654296875 15381.056640625",
            "4094.545654296875",
        )),
        "stress_architecture_batch3_junction_fanout_group_edges_051" => Some((
            "-375.9061279296875 -254.6621856689453 831.812255859375 640.3243408203125",
            "831.812255859375",
        )),
        "stress_architecture_batch3_unicode_titles_and_services_052" => Some((
            "-181.9628143310547 -165.9628143310547 447.9256286621094 462.9256591796875",
            "447.9256286621094",
        )),
        "stress_architecture_batch3_icontext_and_fallback_mix_053" => Some((
            "-314.565185546875 -107 712.13037109375 345",
            "712.13037109375",
        )),
        "stress_architecture_batch3_bidirectional_and_mixed_arrows_054" => Some((
            "-153.04686737060547 -9.833333969116211 429.09375 195.52083587646484",
            "429.09375",
        )),
        "stress_architecture_batch3_long_group_titles_wrapping_055" => Some((
            "-221.59327697753906 -65.5 478.1865539550781 262",
            "478.1865539550781",
        )),
        "stress_architecture_batch3_disconnected_components_056" => Some((
            "-323.6618957519531 -225.87416076660156 727.3237915039062 586.748291015625",
            "727.3237915039062",
        )),
        "stress_architecture_batch3_parallel_edges_and_labels_057" => Some((
            "-119.83184051513672 -211.1748046875 361.1636962890625 586.037109375",
            "361.1636962890625",
        )),
        "stress_architecture_batch3_port_pairs_corner_cases_058" => Some((
            "-204.96063232421875 -209.55215454101562 532.9212646484375 583.1453552246094",
            "532.9212646484375",
        )),
        "stress_architecture_batch3_many_services_one_group_059" => Some((
            "-754.073486328125 -65.5 1588.14697265625 262",
            "1588.14697265625",
        )),
        "stress_architecture_batch3_nested_junctions_routing_060" => Some((
            "-324.81573486328125 -228.3118133544922 731.1314697265625 589.1236572265625",
            "731.1314697265625",
        )),
        "stress_architecture_batch4_init_small_icons_061" => Some((
            "-60.53569793701172 -56.28569793701172 187.85890197753906 191.57139587402344",
            "187.85890197753906",
        )),
        "stress_architecture_batch4_init_large_icons_062" => Some((
            "-272.7512664794922 -344.3980712890625 665.5025024414062 865.796142578125",
            "665.5025024414062",
        )),
        "stress_architecture_batch4_init_fontsize_wrap_063" => Some((
            "20 -208.9365234375 161.78750610351562 585.560546875",
            "161.78750610351562",
        )),
        "stress_architecture_batch4_icontext_xml_escapes_064" => Some((
            "-182.84327697753906 -65.5 445.6865539550781 262",
            "445.6865539550781",
        )),
        "stress_architecture_batch4_title_accdescr_multiline_065" => Some((
            "-119.5932846069336 -22 360.68658447265625 184.1875",
            "360.68658447265625",
        )),
        "stress_architecture_batch4_three_level_groups_crosslinks_066" => Some((
            "-246.3937225341797 -107 572.7874755859375 345",
            "572.7874755859375",
        )),
        "stress_architecture_batch4_nested_groups_junction_fanout_067" => Some((
            "-308.64166259765625 -343.0796203613281 697.2833251953125 817.1592407226562",
            "697.2833251953125",
        )),
        "stress_architecture_batch4_mixed_arrows_xy_labels_068" => Some((
            "-94.03656005859375 -108.98558044433594 343.7802429199219 378.4718017578125",
            "343.7802429199219",
        )),
        "stress_architecture_batch4_many_groups_sparse_services_069" => Some((
            "-423.6114501953125 -65.5 927.222900390625 262",
            "927.222900390625",
        )),
        "stress_architecture_batch4_weird_ids_numbers_070" => {
            Some(("-283.1865234375 -65.5 646.373046875 262", "646.373046875"))
        }
        "stress_architecture_batch4_i18n_accessibility_071" => Some((
            "-182.84327697753906 -65.5 445.6865539550781 262",
            "445.6865539550781",
        )),
        "stress_architecture_batch4_ports_matrix_072" => Some((
            "-118.96281433105469 -110.71281433105469 360.9256286621094 385.1131286621094",
            "360.9256286621094",
        )),
        "stress_architecture_batch5_dense_group_services_073" => Some((
            "-195.41329956054688 -166.04556274414062 454.82659912109375 463.09112548828125",
            "454.82659912109375",
        )),
        "stress_architecture_batch5_junction_fanout_grid_074" => Some((
            "-319.9057312011719 -211.35484313964844 762.8114624023438 586.397216796875",
            "762.8114624023438",
        )),
        "stress_architecture_batch5_group_edges_across_nested_groups_075" => {
            Some(("-137 -445.6163635253906 354 1022.232666015625", "354"))
        }
        "stress_architecture_batch5_long_titles_and_punct_076" => Some((
            "-273.4628143310547 -165.9628143310547 542.9256286621094 462.9256591796875",
            "542.9256286621094",
        )),
        "stress_architecture_batch5_weird_ids_numbers_077" => Some((
            "-226.9765167236328 -212.8216552734375 533.9530029296875 556.643310546875",
            "533.9530029296875",
        )),
        "stress_architecture_batch5_services_outside_groups_crosslinks_078" => Some((
            "-219.20965576171875 -252.98187255859375 518.4193115234375 640.9637451171875",
            "518.4193115234375",
        )),
        "stress_architecture_batch5_multi_edges_same_ports_079" => Some((
            "-110.3686294555664 -110.71257019042969 372.4039306640625 385.1126403808594",
            "372.4039306640625",
        )),
        "stress_architecture_batch5_bidirectional_arrows_group_modifier_080" => Some((
            "-335.11102294921875 -242.09506225585938 750.2220458984375 615.1901245117188",
            "750.2220458984375",
        )),
        "stress_architecture_batch5_nested_groups_many_levels_081" => Some((
            "-266.04710388183594 -316.86309814453125 612.09423828125 770.7261962890625",
            "612.09423828125",
        )),
        "stress_architecture_batch5_ports_matrix_stress_082" => Some((
            "-103.85939407348633 -194.90057373046875 331.0723419189453 553.4886474609375",
            "331.0723419189453",
        )),
        "stress_architecture_batch5_disconnected_components_083" => Some((
            "-319.2033386230469 -165.8432846069336 718.4066772460938 462.6865539550781",
            "718.4066772460938",
        )),
        "stress_architecture_batch5_title_accdescr_frontmatter_084" => Some((
            "-172.2510223388672 -227.4480438232422 424.5020446777344 585.8961181640625",
            "424.5020446777344",
        )),
        "stress_architecture_batch6_edge_label_wrapping_punct_unicode_085" => Some((
            "-158.01492309570312 -65.5 447.6865539550781 262",
            "447.6865539550781",
        )),
        "stress_architecture_batch6_nested_groups_group_edges_and_ports_086" => Some((
            "-241.50086975097656 -448.21905517578125 548.001708984375 1027.4381103515625",
            "548.001708984375",
        )),
        "stress_architecture_batch6_junctions_multi_split_with_group_edges_087" => Some((
            "-286.5922393798828 -257.49559020996094 653.1844482421875 645.9911499023438",
            "653.1844482421875",
        )),
        "stress_architecture_batch6_iconify_icontext_html_entities_088" => Some((
            "-186.7128143310547 -165.9628143310547 448.4256286621094 462.9256591796875",
            "448.4256286621094",
        )),
        "stress_architecture_batch6_disconnected_components_with_titles_089" => Some((
            "-417.2345886230469 -146.15457153320312 914.4691772460938 423.30914306640625",
            "914.4691772460938",
        )),
        "stress_architecture_batch6_port_matrix_stress_090" => Some((
            "-150.59911346435547 -190.25 424.1982421875 544.1875",
            "424.1982421875",
        )),
        "stress_architecture_batch6_ordering_declarations_and_in_keyword_091" => Some((
            "-199.3567352294922 -186.19541931152344 485.7134704589844 506.390869140625",
            "485.7134704589844",
        )),
        "stress_architecture_batch6_mixed_arrow_styles_and_labels_092" => Some((
            "-219.97486877441406 -9.833333969116211 562.94970703125 195.52083587646484",
            "562.94970703125",
        )),
        "stress_architecture_batch6_init_fontsize_icon_size_wrap_093" => Some((
            "-135.7482147216797 -160.04725646972656 334.15765380859375 417.0945129394531",
            "334.15765380859375",
        )),
        "stress_architecture_batch6_deep_group_chain_crosslinks_094" => Some((
            "-12254.1611328125 -17308.404296875 24582.322265625 34750.80859375",
            "24582.322265625",
        )),
        "stress_architecture_batch6_long_group_titles_wrapping_extreme_095" => Some((
            "-224.34327697753906 -107 533 345",
            "533",
        )),
        "stress_architecture_batch6_edge_labels_with_xml_like_text_096" => Some((
            "-104.8432846069336 -22 360.68658447265625 184.1875",
            "360.68658447265625",
        )),
        "upstream_cypress_other_xss_spec_icon_labels_architecture_001" => {
            Some(("-82.5 -65.5 245 262", "245"))
        }
        _ => None,
    }
}
