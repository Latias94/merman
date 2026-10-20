#![cfg(all(feature = "svg", feature = "diagram-flowchart"))]

use merman_bindings_core::{BindingEngine, BindingEngineServices, BindingStatus, OperationControl};

const SOURCE: &[u8] = b"flowchart LR\nA((Circle)) -->|label| B[End]";

#[test]
fn geometry_output_budget_applies_to_source_and_retained_consumers() {
    let expected = merman_bindings_core::edge_geometry_json(SOURCE, b"").unwrap();
    let engine = BindingEngine::new(b"").unwrap();
    let prepared = engine
        .prepare_source(SOURCE, OperationControl::new())
        .unwrap();
    let services = BindingEngineServices::new();
    for length in [expected.len(), expected.len() - 1] {
        let options = serde_json::to_vec(
            &serde_json::json!({"resources":{"limits":{"max_geometry_json_bytes":length}}}),
        )
        .unwrap();
        let source = merman_bindings_core::edge_geometry_json(SOURCE, &options);
        let retained = prepared.edge_geometry_json(&options, OperationControl::new(), &services);
        if length == expected.len() {
            assert_eq!(source.unwrap(), expected);
            assert_eq!(retained.unwrap(), expected);
        } else {
            for error in [source.unwrap_err(), retained.unwrap_err()] {
                assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
                let details = error.resource_details().unwrap();
                assert_eq!(details.limit_id, "max_geometry_json_bytes");
                assert_eq!(details.phase, "geometry_output");
            }
        }
    }
}

#[test]
fn geometry_rejects_irrelevant_svg_and_backend_resource_overrides() {
    let engine = BindingEngine::new(b"").unwrap();
    let prepared = engine
        .prepare_source(SOURCE, OperationControl::new())
        .unwrap();
    let services = BindingEngineServices::new();
    for limit in [
        "max_svg_bytes",
        "max_svg_elements",
        "svg_backend_tree_nodes",
        "svg_backend_tree_depth",
    ] {
        let options =
            serde_json::to_vec(&serde_json::json!({"resources":{"limits":{limit:1024}}})).unwrap();
        for error in [
            merman_bindings_core::edge_geometry_json(SOURCE, &options).unwrap_err(),
            prepared
                .edge_geometry_json(&options, OperationControl::new(), &services)
                .unwrap_err(),
        ] {
            assert_eq!(
                error.status(),
                BindingStatus::InvalidArgument,
                "{limit}: {error:?}"
            );
            assert!(error.message().contains(limit));
        }
    }
    let options = br#"{"resources":{"limits":{"max_geometry_json_bytes":1024}}}"#;
    assert_eq!(
        merman_bindings_core::render_svg(SOURCE, options)
            .unwrap_err()
            .status(),
        BindingStatus::InvalidArgument
    );
}
