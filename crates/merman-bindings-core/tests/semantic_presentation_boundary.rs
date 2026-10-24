#![cfg(feature = "diagram-flowchart")]

use merman_bindings_core::{BindingEngine, BindingOperationRequest, BindingStatus, execute_once};

const SOURCE: &[u8] = b"flowchart TD\nA --> B";
const INVALID_VISUAL_THEME: &[u8] = concat!(
    "%%{init: {\"themeVariables\": {\"primaryColor\": \"not-a-color\"}}}%%\n",
    "flowchart TD\nA --> B",
)
.as_bytes();

#[test]
fn binding_options_remain_eagerly_validated_before_source_work() {
    let cases: &[(&[u8], BindingStatus, &str)] = &[
        (b"{", BindingStatus::OptionsJsonError, "options_json"),
        (
            br#"{"site_config":false}"#,
            BindingStatus::InvalidArgument,
            "site_config",
        ),
        (
            br#"{"fixed_today":"not-a-date"}"#,
            BindingStatus::InvalidArgument,
            "fixed_today",
        ),
        #[cfg(feature = "svg")]
        (
            br#"{"theme":{"preset":"unknown-preset"}}"#,
            BindingStatus::InvalidArgument,
            "theme.preset",
        ),
        #[cfg(feature = "svg")]
        (
            br#"{"theme":{"spec":{"canvas":{"base":"white; color: red"}}}}"#,
            BindingStatus::InvalidArgument,
            "theme.spec.paint",
        ),
        #[cfg(feature = "svg")]
        (
            br#"{"svg":{"viewbox_padding":-1}}"#,
            BindingStatus::InvalidArgument,
            "svg.viewbox_padding",
        ),
    ];
    let engine = BindingEngine::new(b"").unwrap();
    for &(options, status, field) in cases {
        let constructor_error = BindingEngine::new(options).err().expect(field);
        assert_eq!(constructor_error.status(), status, "{field}");
        assert!(
            constructor_error.message().contains(field),
            "{constructor_error:?}",
        );
        for operation in [
            "semantic-json",
            #[cfg(feature = "analysis")]
            "analysis-json",
            #[cfg(feature = "ascii")]
            "ascii",
        ] {
            let request =
                BindingOperationRequest::new(operation, b"\xff").with_options_json(options);
            assert_eq!(
                execute_once(request.clone()).unwrap_err(),
                constructor_error
            );
            assert_eq!(engine.execute(request).unwrap_err(), constructor_error);
        }
    }
}

#[test]
fn full_binding_operations_preserve_source_theme_errors() {
    let engine = BindingEngine::new(b"").unwrap();
    for operation in [
        "semantic-json",
        #[cfg(feature = "svg")]
        "svg",
        #[cfg(feature = "svg")]
        "svg-plan-json",
        #[cfg(feature = "svg")]
        "layout-json",
        #[cfg(feature = "svg")]
        "edge-geometry-json",
    ] {
        let request = BindingOperationRequest::new(operation, INVALID_VISUAL_THEME);
        let error = execute_once(request.clone()).unwrap_err();
        assert!(
            error.message().contains("not-a-color"),
            "{operation}: {error:?}",
        );
        assert_eq!(engine.execute(request).unwrap_err(), error, "{operation}");
        assert!(
            !engine
                .execute_data(BindingOperationRequest::new(operation, SOURCE))
                .unwrap()
                .is_empty(),
        );
    }
    #[cfg(feature = "svg")]
    {
        let error = engine
            .prepare_source(
                INVALID_VISUAL_THEME,
                merman_bindings_core::OperationControl::new(),
            )
            .err()
            .expect("prepared render sources retain full theme validation");
        assert!(error.message().contains("not-a-color"), "{error:?}");
    }
}
