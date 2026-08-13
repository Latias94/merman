#![cfg(not(any(feature = "svg", feature = "ascii")))]

use merman::{OperationControl, Renderer};

#[test]
fn semantic_artifact_exposes_compatibility_json_without_output_features() {
    let artifact = Renderer::new()
        .prepare_semantic("info", OperationControl::new())
        .expect("semantic preparation succeeds")
        .expect("Info should be detected");

    let compatibility = artifact.compatibility_json().expect("compatibility JSON");
    assert_eq!(compatibility["type"], "info");
}
