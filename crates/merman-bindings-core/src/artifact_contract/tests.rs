use super::*;

fn semantic_exposure() -> TransportExposure {
    TransportExposure::for_target(TargetKey::Native)
        .with_operations([OperationKey::SemanticJson])
        .unwrap()
}

fn semantic_contract() -> ValidatedArtifactContract {
    CompiledBindingSurface::current()
        .validate(semantic_exposure())
        .unwrap()
}

#[test]
fn explicit_selections_reject_duplicates() {
    let error = semantic_exposure()
        .with_operations([OperationKey::SemanticJson])
        .unwrap_err();
    assert!(error.message().contains("declared more than once"));
}

#[test]
fn transport_cannot_expose_an_uncompiled_operation() {
    let exposure = TransportExposure::for_target(TargetKey::Native)
        .with_operations([OperationKey::Png])
        .unwrap();
    let result = CompiledBindingSurface::current().validate(exposure);
    assert_eq!(result.is_ok(), cfg!(all(feature = "png", feature = "svg")));
}

#[test]
fn output_projection_uses_the_generated_operation_relationship() {
    let compiled = CompiledBindingSurface::current();
    let operations = OperationKey::ALL
        .iter()
        .copied()
        .filter(|operation| compiled.operations.contains(operation))
        .filter(|operation| operation.spec().targets.contains(&TargetKey::Native))
        .collect::<Vec<_>>();
    let expected = operations
        .iter()
        .filter_map(|operation| operation.spec().output)
        .collect::<BTreeSet<_>>();
    let exposure = TransportExposure::for_target(TargetKey::Native)
        .with_operations(operations)
        .unwrap();

    let contract = compiled.validate(exposure).unwrap();
    assert_eq!(contract.output_keys().collect::<BTreeSet<_>>(), expected);
}

#[cfg(feature = "png")]
#[test]
fn operation_requirements_validate_the_pipeline_without_advertising_its_output() {
    let exposure = TransportExposure::for_target(TargetKey::Native)
        .with_operations([OperationKey::Png])
        .unwrap();
    let contract = CompiledBindingSurface::current()
        .validate(exposure)
        .unwrap();

    assert_eq!(
        contract.capability_keys().collect::<BTreeSet<_>>(),
        BTreeSet::from([CapabilityKey::Png])
    );
    assert_eq!(
        contract.output_keys().collect::<BTreeSet<_>>(),
        BTreeSet::from([OutputKey::Png])
    );
    assert_eq!(
        contract
            .text_measurement_provider_keys()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([TextMeasurementProviderKey::Vendored])
    );
}

#[cfg(feature = "png")]
#[test]
fn operation_requirements_reject_a_missing_compiled_pipeline() {
    let mut compiled = CompiledBindingSurface::current();
    compiled.capabilities.remove(&CapabilityKey::Svg);
    let exposure = TransportExposure::for_target(TargetKey::Native)
        .with_operations([OperationKey::Png])
        .unwrap();

    let error = compiled.validate(exposure).unwrap_err();
    assert!(error.message().contains("operation requirement `svg`"));
    assert!(error.message().contains("png"));
}

#[test]
fn operation_admission_uses_the_validated_contract() {
    let contract = semantic_contract();
    let semantic = crate::BindingOperationKind::from_id("semantic-json").unwrap();
    assert_eq!(
        contract.admit_operation(semantic).unwrap().operation(),
        semantic
    );
    let analysis = crate::BindingOperationKind::from_id("analysis-json").unwrap();
    let error = contract.admit_operation(analysis).unwrap_err();
    assert_eq!(semantic.operation_id(), "semantic-json");
    assert_eq!(error.status(), crate::BindingStatus::UnsupportedOperation);
}

#[test]
fn contract_bound_execution_preserves_validation_before_admission() {
    let contract = semantic_contract();
    let engine = contract.create_engine(b"").unwrap();

    for error in [
        contract
            .execute_once(
                crate::BindingOperationRequest::new("analysis-json", b"flowchart TD\nA --> B")
                    .with_options_json(b"{"),
            )
            .unwrap_err(),
        engine
            .execute(
                crate::BindingOperationRequest::new("analysis-json", b"flowchart TD\nA --> B")
                    .with_options_json(b"{"),
            )
            .unwrap_err(),
    ] {
        assert_eq!(error.status(), crate::BindingStatus::OptionsJsonError);
    }

    for error in [
        contract
            .execute_once(crate::BindingOperationRequest::new(
                "analysis-json",
                b"flowchart TD\nA --> B",
            ))
            .unwrap_err(),
        engine
            .execute(crate::BindingOperationRequest::new(
                "analysis-json",
                b"flowchart TD\nA --> B",
            ))
            .unwrap_err(),
    ] {
        assert_eq!(error.status(), crate::BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
        assert_eq!(error.capability_id(), Some("analysis"));
    }
}

#[test]
fn known_hidden_base_operation_is_not_a_missing_capability() {
    let contract = CompiledBindingSurface::current()
        .validate(TransportExposure::for_target(TargetKey::Native))
        .unwrap();
    let error = contract
        .execute_once(crate::BindingOperationRequest::new(
            "semantic-json",
            b"flowchart TD\nA --> B",
        ))
        .unwrap_err();

    assert_eq!(error.status(), crate::BindingStatus::UnsupportedOperation);
    assert_eq!(error.kind(), crate::BindingErrorKind::Generic);
    assert_eq!(error.capability_id(), None);
    assert!(error.message().contains("is not exposed"));
}

#[test]
fn deterministic_only_contract_rejects_native_runtime_policy() {
    let error = semantic_contract()
        .create_engine(br#"{"runtime_policy":"native"}"#)
        .err()
        .expect("deterministic-only contracts must reject native policy");

    assert_eq!(error.status(), crate::BindingStatus::OptionsJsonError);
    assert!(error.message().contains("is not exposed"));
}

#[test]
fn binding_options_runtime_policy_is_native_target_only() {
    let exposure = TransportExposure::for_target(TargetKey::Web)
        .with_operations([OperationKey::SemanticJson])
        .unwrap()
        .with_runtime_policy_exposure(RuntimePolicyExposure::BindingOptions);
    let error = CompiledBindingSurface::current()
        .validate(exposure)
        .unwrap_err();

    assert!(error.message().contains("not valid for target `web`"));
}

#[cfg(feature = "svg")]
#[test]
fn named_helpers_cannot_bypass_contract_admission() {
    let engine = semantic_contract().create_engine(b"").unwrap();
    assert!(engine.parse_json(b"flowchart TD\nA --> B").is_ok());

    let error = engine.render_svg(b"flowchart TD\nA --> B").unwrap_err();
    assert_eq!(error.status(), crate::BindingStatus::UnsupportedOperation);
    assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
    assert_eq!(error.capability_id(), Some("svg"));
}

#[cfg(feature = "svg")]
#[test]
fn unadvertised_constructor_service_is_rejected() {
    struct NoopHostTextMeasurer;

    impl crate::HostTextMeasurer for NoopHostTextMeasurer {
        fn measure(
            &self,
            _request: crate::HostTextMeasurementRequest<'_>,
        ) -> crate::HostMeasurementResult {
            Ok(None)
        }
    }

    let services =
        crate::BindingEngineServices::new().with_host_text_measurer(Arc::new(NoopHostTextMeasurer));
    let error = semantic_contract()
        .create_engine_with_services(b"", services)
        .err()
        .expect("unadvertised constructor services must be rejected");

    assert_eq!(error.status(), crate::BindingStatus::InvalidArgument);
    assert!(error.message().contains("is not exposed"));
}

#[cfg(feature = "svg")]
#[test]
fn operations_and_services_derive_text_measurement_providers() {
    let vendored_only = CompiledBindingSurface::current()
        .validate(
            TransportExposure::for_target(TargetKey::Native)
                .with_operations([OperationKey::Svg])
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        vendored_only
            .text_measurement_provider_keys()
            .collect::<Vec<_>>(),
        [TextMeasurementProviderKey::Vendored]
    );

    let with_host = CompiledBindingSurface::current()
        .validate(
            TransportExposure::for_target(TargetKey::Native)
                .with_operations([OperationKey::Svg])
                .unwrap()
                .with_constructor_services([ConstructorServiceKey::HostTextMeasurement])
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        with_host
            .text_measurement_provider_keys()
            .collect::<Vec<_>>(),
        [
            TextMeasurementProviderKey::HostCallback,
            TextMeasurementProviderKey::Vendored,
        ]
    );

    let with_icons = CompiledBindingSurface::current()
        .validate(
            TransportExposure::for_target(TargetKey::Native)
                .with_operations([OperationKey::Svg])
                .unwrap()
                .with_constructor_services([ConstructorServiceKey::IconRegistry])
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        with_icons
            .text_measurement_provider_keys()
            .collect::<Vec<_>>(),
        [TextMeasurementProviderKey::Vendored]
    );
}

#[cfg(feature = "svg")]
#[test]
fn descriptor_implications_are_closed_automatically() {
    let exposure = TransportExposure::for_target(TargetKey::Native)
        .with_operations([OperationKey::Svg])
        .unwrap()
        .with_supplemental_capabilities([CapabilityKey::LayoutElk])
        .unwrap();
    let result = CompiledBindingSurface::current().validate(exposure);
    assert_eq!(result.is_ok(), merman::svg::layout_elk_available());
    if let Ok(contract) = result {
        assert!(
            contract
                .capability_keys()
                .any(|key| key == CapabilityKey::Svg)
        );
    }
}
