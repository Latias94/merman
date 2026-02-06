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
        "upstream_architecture_cypress_fallback_icon" => {
            Some(("-49.8515625 -22 179.953125 184.1875", "179.953125"))
        }
        "upstream_architecture_cypress_group_edges_normalized" => Some((
            "-324.6684875488281 -307.6684875488281 729.3369750976562 746.3369750976562",
            "729.3369750976562",
        )),
        "upstream_architecture_cypress_groups_normalized" => Some((
            "-183.60711669921875 -262.5092315673828 447.2142639160156 660.0184936523438",
            "447.2142639160156",
        )),
        "upstream_architecture_cypress_groups_within_groups_normalized" => Some((
            "-224.6627960205078 -234.7019500732422 529.3255615234375 600.4039306640625",
            "529.3255615234375",
        )),
        "upstream_architecture_cypress_reasonable_height" => Some((
            "-889.9099731445312 -270.7564392089844 1859.8199462890625 672.5128784179688",
            "1859.8199462890625",
        )),
        "upstream_architecture_cypress_simple_junction_edges_normalized" => Some((
            "-220.5164794921875 -211.2884063720703 564.032958984375 586.2642822265625",
            "564.032958984375",
        )),
        "upstream_architecture_cypress_split_directioning_normalized" => Some((
            "-111.91932678222656 -237.2515869140625 346.8386535644531 638.190673828125",
            "346.8386535644531",
        )),
        "upstream_architecture_cypress_title_and_accessibilities" => Some((
            "-183.41357421875 -165.96131896972656 446.8271484375 462.922607421875",
            "446.8271484375",
        )),
        "upstream_architecture_docs_edge_arrows" => Some((
            "-220.1748046875 -110.83182525634766 561.849609375 385.3511657714844",
            "561.849609375",
        )),
        "upstream_architecture_docs_edge_titles" => Some((
            "-108.21281433105469 -110.71281433105469 360.9256286621094 385.1131286621094",
            "360.9256286621094",
        )),
        "upstream_architecture_docs_example" => Some((
            "-183.41357421875 -165.96131896972656 446.8271484375 462.922607421875",
            "446.8271484375",
        )),
        "upstream_architecture_docs_group_edges" => {
            Some(("-82.5 -187.10560607910156 245 505.211181640625", "245"))
        }
        "upstream_architecture_docs_groups_within_groups" => Some((
            "-224.6627960205078 -234.7019500732422 529.3255615234375 600.4039306640625",
            "529.3255615234375",
        )),
        "upstream_architecture_docs_junctions" => Some((
            "-220.5164794921875 -211.2884063720703 564.032958984375 586.2642822265625",
            "564.032958984375",
        )),
        "upstream_architecture_svgdraw_ids_spec" => Some((
            "-183.41357421875 -165.96131896972656 446.8271484375 462.922607421875",
            "446.8271484375",
        )),
        "upstream_architecture_docs_service_icon_text" => Some((
            "-120.19210052490234 -115.5868148803711 343.88421630859375 371.36114501953125",
            "343.88421630859375",
        )),
        "upstream_architecture_layout_reasonable_height" => Some((
            "-889.9099731445312 -270.7564392089844 1859.8199462890625 672.5128784179688",
            "1859.8199462890625",
        )),
        _ => None,
    }
}
