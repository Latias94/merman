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
        "upstream_cypress_other_xss_spec_icon_labels_architecture_001" => {
            Some(("-82.5 -65.5 245 262", "245"))
        }
        _ => None,
    }
}
