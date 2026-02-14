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
        _ => None,
    }
}
