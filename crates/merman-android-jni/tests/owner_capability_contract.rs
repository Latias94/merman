#![cfg(all(
    feature = "svg",
    not(feature = "layout-cytoscape"),
    not(feature = "layout-elk"),
    not(feature = "math")
))]

#[path = "../src/artifact_contract.rs"]
mod artifact_contract;

use artifact_contract::android_artifact_contract;
use merman_bindings_core::BindingStatus;

#[test]
fn ambient_render_dependencies_do_not_widen_the_android_owner_contract() {
    let artifact_contract = android_artifact_contract();
    let capabilities = artifact_contract.runtime_capabilities();
    assert!(capabilities.has_capability("svg"));

    let engine = artifact_contract
        .create_engine(b"")
        .expect("Android engine");
    for (capability_id, source) in [
        (
            "layout-cytoscape",
            b"architecture-beta\n  service api(server)[API]".as_slice(),
        ),
        (
            "layout-elk",
            b"---\nconfig:\n  layout: elk\n---\nflowchart TD\nA --> B".as_slice(),
        ),
        ("math", b"flowchart TD\nA[\"$$x^2$$\"] --> B".as_slice()),
    ] {
        assert!(!capabilities.has_capability(capability_id));

        let plan = String::from_utf8(engine.svg_plan_json(source).expect("SVG plan")).unwrap();
        assert!(
            plan.contains(&format!(
                "\"required_capability_ids\":[\"{capability_id}\"]"
            )),
            "{plan}"
        );
        assert!(
            plan.contains(&format!("\"missing_capability_ids\":[\"{capability_id}\"]")),
            "{plan}"
        );
        assert!(plan.contains("\"ready\":false"), "{plan}");

        let error = engine
            .render_svg(source)
            .expect_err("the Android owner contract must reject ambient render capabilities");
        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.capability_id(), Some(capability_id));
    }

    let error =
        match artifact_contract.create_engine(br#"{"environment":{"math_renderer":"ratex"}}"#) {
            Ok(_) => panic!("explicit ratex selection requires owner-selected math"),
            Err(error) => error,
        };
    assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
    assert_eq!(error.capability_id(), Some("math"));
}
