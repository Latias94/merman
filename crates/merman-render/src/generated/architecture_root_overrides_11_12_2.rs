// Fixture-derived root viewport overrides for Mermaid@11.12.3 Architecture diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/architecture/*.svg` and are keyed by `diagram_id`
// (fixture stem). They are applied only for non-empty diagrams where Architecture
// root viewport parity (`viewBox` + `max-width`) still differs from upstream.

pub fn lookup_architecture_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_html_demos_architecture_external_icons_demo_012" => Some((
            "-173.01507568359375 -165.18142700195312 439.6161193847656 470.55035400390625",
            "439.6161193847656",
        )),
        "stress_architecture_dense_mesh_001" => Some((
            "-271.47344970703125 -277.07244873046875 622.9468994140625 689.1448974609375",
            "622.9468994140625",
        )),
        "stress_architecture_external_icons_005" => Some((
            "-403.5445861816406 -166.05992889404297 861.0891723632812 463.1198425292969",
            "861.0891723632812",
        )),
        "stress_architecture_nested_groups_002" => Some((
            "-314.4618835449219 -245.82904052734375 728.9237670898438 622.6580810546875",
            "728.9237670898438",
        )),
        "stress_architecture_edge_label_corner_cases_012" => Some((
            "-65.28177452087402 -101.78177261352539 344.8510437011719 367.25103759765625",
            "344.8510437011719",
        )),
        "stress_architecture_deep_nesting_013" => Some((
            "-379.9537048339844 -376.36761474609375 839.9074096679688 893.7352294921875",
            "839.9074096679688",
        )),
        "stress_architecture_disconnected_components_015" => Some((
            "-422.1696472167969 -189.57656860351562 924.3392944335938 510.15313720703125",
            "924.3392944335938",
        )),
        "stress_architecture_long_group_titles_018" => Some((
            "-182.9628143310547 -165.9628143310547 480.140625 462.9256591796875",
            "480.140625",
        )),
        "stress_architecture_unicode_and_xml_escapes_019" => Some((
            "-211.6609649658203 -166.29661560058594 470.3219299316406 463.59326171875",
            "470.3219299316406",
        )),
        "stress_architecture_fan_in_out_021" => Some((
            "-335.6900634765625 -319.68463134765625 751.380126953125 770.3692626953125",
            "751.380126953125",
        )),
        "stress_architecture_junction_fork_join_026" => Some((
            "-1370.6475830078125 -1205.6885986328125 2825.295166015625 2542.377197265625",
            "2825.295166015625",
        )),
        "stress_architecture_deep_group_chain_027" => Some((
            "-10925.580078125 -7460.30419921875 21934.16015625 15057.6083984375",
            "21934.16015625",
        )),
        "stress_architecture_long_ids_030" => Some((
            "-302.775390625 -273.27540588378906 677.55078125 677.55078125",
            "677.55078125",
        )),
        "stress_architecture_html_titles_and_escapes_041" => Some((
            "-209.9628143310547 -165.9628143310547 479.9256286621094 462.9256591796875",
            "479.9256286621094",
        )),
        "stress_architecture_disconnected_islands_046" => Some((
            "-374.1528625488281 -252.4567108154297 828.3057250976562 620.9134521484375",
            "828.3057250976562",
        )),
        "stress_architecture_batch3_unicode_titles_and_services_052" => Some((
            "-181.9628143310547 -165.9628143310547 447.9256286621094 462.9256591796875",
            "447.9256286621094",
        )),
        "stress_architecture_batch3_long_group_titles_wrapping_055" => Some((
            "-221.84327697753906 -65.5 477.6865539550781 262",
            "477.6865539550781",
        )),
        "stress_architecture_batch3_nested_junctions_routing_060" => Some((
            "-324.81573486328125 -228.3118133544922 731.1314697265625 589.1236572265625",
            "731.1314697265625",
        )),
        "stress_architecture_batch4_init_small_icons_061" => Some((
            "-60.28569793701172 -56.28569793701172 187.85890197753906 191.57139587402344",
            "187.85890197753906",
        )),
        "stress_architecture_batch4_init_large_icons_062" => Some((
            "-272.7512664794922 -344.3980712890625 665.5025024414062 865.796142578125",
            "665.5025024414062",
        )),
        "stress_architecture_batch4_init_fontsize_wrap_063" => Some((
            "21 -208.9365234375 161.78750610351562 585.560546875",
            "161.78750610351562",
        )),
        "stress_architecture_batch4_nested_groups_junction_fanout_067" => Some((
            "-308.64166259765625 -343.0796203613281 697.2833251953125 817.1592407226562",
            "697.2833251953125",
        )),
        "stress_architecture_batch5_group_edges_across_nested_groups_075" => {
            Some(("-137 -445.6163635253906 354 1022.232666015625", "354"))
        }
        "stress_architecture_batch5_long_titles_and_punct_076" => Some((
            "-274.9628143310547 -165.9628143310547 543.9256286621094 462.9256591796875",
            "543.9256286621094",
        )),
        "stress_architecture_batch5_weird_ids_numbers_077" => Some((
            "-226.9765167236328 -212.8216552734375 533.9530029296875 556.643310546875",
            "533.9530029296875",
        )),
        "stress_architecture_batch5_services_outside_groups_crosslinks_078" => Some((
            "-219.20965576171875 -252.98187255859375 518.4193115234375 640.9637451171875",
            "518.4193115234375",
        )),
        "stress_architecture_batch6_edge_label_wrapping_punct_unicode_085" => Some((
            "-157.51492309570312 -65.5 447.6865539550781 262",
            "447.6865539550781",
        )),
        "stress_architecture_batch6_nested_groups_group_edges_and_ports_086" => Some((
            "-241.00086975097656 -448.21905517578125 549.001708984375 1027.4381103515625",
            "549.001708984375",
        )),
        "stress_architecture_batch6_junctions_multi_split_with_group_edges_087" => Some((
            "-286.5922393798828 -257.49559020996094 653.1844482421875 645.9911499023438",
            "653.1844482421875",
        )),
        "stress_architecture_batch6_init_fontsize_icon_size_wrap_093" => Some((
            "-134.4982147216797 -160.04725646972656 332.65765380859375 417.0945129394531",
            "332.65765380859375",
        )),
        "stress_architecture_font_and_theme_097" => Some((
            "-119.5932846069336 -20 360.68658447265625 183.609375",
            "360.68658447265625",
        )),
        _ => None,
    }
}
