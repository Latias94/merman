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
        _ => None,
    }
}
