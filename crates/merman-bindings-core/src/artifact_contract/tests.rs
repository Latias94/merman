use super::*;

const STATIC_SEMANTIC_CONTRACT: ValidatedArtifactContract =
    ArtifactContractSpec::new(TargetKey::Native)
        .with_operations(&[OperationKey::SemanticJson])
        .materialize();

const STATIC_EMPTY_CONTRACT: ValidatedArtifactContract =
    ArtifactContractSpec::new(TargetKey::Native).materialize();

const STATIC_WEB_EDITOR_CONTRACT: ValidatedArtifactContract =
    ArtifactContractSpec::new(TargetKey::Web)
        .with_operations(&[OperationKey::SemanticJson])
        .with_supplemental_capabilities(&[CapabilityKey::Editor])
        .with_transport_extensions(&[TransportCompiledExtensionKey::Editor])
        .materialize();

fn panics(action: impl FnOnce()) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)).is_err()
}

#[test]
fn semantic_snapshot_is_exact_and_does_not_inherit_compiled_features() {
    assert_eq!(STATIC_SEMANTIC_CONTRACT.target(), TargetKey::Native);
    assert_eq!(
        STATIC_SEMANTIC_CONTRACT
            .operation_keys()
            .collect::<Vec<_>>(),
        [OperationKey::SemanticJson]
    );
    assert_eq!(
        STATIC_SEMANTIC_CONTRACT
            .capability_keys()
            .collect::<Vec<_>>(),
        []
    );
    assert_eq!(
        STATIC_SEMANTIC_CONTRACT.output_keys().collect::<Vec<_>>(),
        []
    );
    assert_eq!(
        STATIC_SEMANTIC_CONTRACT.metadata_keys().collect::<Vec<_>>(),
        []
    );
    assert_eq!(
        STATIC_SEMANTIC_CONTRACT
            .option_group_keys()
            .collect::<Vec<_>>(),
        []
    );
    assert_eq!(
        STATIC_SEMANTIC_CONTRACT
            .constructor_service_keys()
            .collect::<Vec<_>>(),
        []
    );
    assert_eq!(
        STATIC_SEMANTIC_CONTRACT
            .text_measurement_provider_keys()
            .collect::<Vec<_>>(),
        []
    );
    assert_eq!(
        STATIC_SEMANTIC_CONTRACT.runtime_policy_exposure(),
        RuntimePolicyExposure::DeterministicOnly
    );
}

#[test]
fn semantic_contract_rejects_every_unadvertised_constructor_option_group() {
    for group in BindingOptionGroupKey::ALL {
        let options = format!(r#"{{"{}":{{}}}}"#, group.id());
        let error = STATIC_SEMANTIC_CONTRACT
            .create_engine(options.as_bytes())
            .err()
            .unwrap_or_else(|| panic!("semantic contract accepted `{}`", group.id()));

        assert_eq!(error.status(), crate::BindingStatus::OptionsJsonError);
        assert!(error.message().contains(group.id()));
        assert!(error.message().contains("not exposed by target `native`"));
    }
}

#[test]
fn semantic_contract_rejects_unadvertised_request_option_groups_after_feature_unification() {
    let engine = STATIC_SEMANTIC_CONTRACT.create_engine(b"").unwrap();
    for group in [
        BindingOptionGroupKey::Ascii,
        BindingOptionGroupKey::Environment,
        BindingOptionGroupKey::Layout,
        BindingOptionGroupKey::Lint,
        BindingOptionGroupKey::Presentation,
        BindingOptionGroupKey::Svg,
    ] {
        let options = format!(r#"{{"{}":{{}}}}"#, group.id());
        let error = engine
            .execute(
                crate::BindingOperationRequest::new(
                    OperationKey::SemanticJson.id(),
                    b"flowchart TD\nA --> B",
                )
                .with_options_json(options.as_bytes()),
            )
            .unwrap_err();

        assert_eq!(error.status(), crate::BindingStatus::OptionsJsonError);
        assert!(error.message().contains(group.id()));
        assert!(error.message().contains("not exposed by target `native`"));
    }

    let nested_lint = engine
        .execute(
            crate::BindingOperationRequest::new(
                OperationKey::SemanticJson.id(),
                b"flowchart TD\nA --> B",
            )
            .with_options_json(br#"{"analysis":{"lint":{}}}"#),
        )
        .unwrap_err();
    assert_eq!(nested_lint.status(), crate::BindingStatus::OptionsJsonError);
    assert!(nested_lint.message().contains("lint"));
    assert!(
        nested_lint
            .message()
            .contains("not exposed by target `native`")
    );
}

#[test]
fn full_default_snapshot_matches_the_feature_owned_declaration() {
    assert_eq!(
        DEFAULT_ARTIFACT_SNAPSHOT
            .operation_keys()
            .collect::<Vec<_>>(),
        DEFAULT_OPERATIONS
    );
    assert_eq!(
        DEFAULT_ARTIFACT_SNAPSHOT
            .payload_schema_keys()
            .collect::<Vec<_>>(),
        BindingPayloadSchemaKey::ALL
    );
    assert_eq!(
        DEFAULT_ARTIFACT_SNAPSHOT
            .constructor_service_keys()
            .collect::<Vec<_>>(),
        DEFAULT_CONSTRUCTOR_SERVICES
    );
    let expected_system_adapters = if DEFAULT_SYSTEM_ADAPTERS.len() == 3 {
        DEFAULT_SYSTEM_ADAPTERS
    } else {
        &[]
    };
    assert_eq!(
        DEFAULT_ARTIFACT_SNAPSHOT
            .system_adapter_keys()
            .collect::<Vec<_>>(),
        expected_system_adapters
    );
    assert_eq!(
        DEFAULT_ARTIFACT_SNAPSHOT.runtime_policy_exposure(),
        DEFAULT_RUNTIME_POLICY
    );

    let expected_metadata = MetadataKey::ALL
        .iter()
        .copied()
        .filter(|key| {
            key.spec()
                .required_capability()
                .is_none_or(|capability| DEFAULT_ARTIFACT_SNAPSHOT.exposes_capability(capability))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        DEFAULT_ARTIFACT_SNAPSHOT
            .metadata_keys()
            .collect::<Vec<_>>(),
        expected_metadata
    );

    for operation in DEFAULT_OPERATIONS {
        assert!(
            operation
                .spec()
                .capability
                .is_none_or(|capability| DEFAULT_ARTIFACT_SNAPSHOT.exposes_capability(capability))
        );
    }
}

#[test]
fn target_bits_follow_stable_descriptor_order() {
    for (index, target) in TargetKey::ALL.iter().copied().enumerate() {
        assert_eq!(target_bit(target), 1_u8 << index, "{}", target.id());
        if index > 0 {
            assert!(TargetKey::ALL[index - 1].id() < target.id());
        }
    }
}

#[test]
fn typed_transport_extensions_are_explicit_and_exact() {
    assert_eq!(STATIC_WEB_EDITOR_CONTRACT.target(), TargetKey::Web);
    assert_eq!(
        STATIC_WEB_EDITOR_CONTRACT
            .operation_keys()
            .collect::<Vec<_>>(),
        [OperationKey::SemanticJson]
    );
    assert_eq!(
        STATIC_WEB_EDITOR_CONTRACT
            .capability_keys()
            .collect::<Vec<_>>(),
        [CapabilityKey::Editor]
    );

    assert!(panics(|| {
        let _ = ArtifactContractSpec::new(TargetKey::Web)
            .with_operations(&[OperationKey::SemanticJson])
            .with_supplemental_capabilities(&[CapabilityKey::Editor])
            .materialize();
    }));
}

#[test]
fn duplicate_and_reconfigured_fields_fail_closed() {
    assert!(panics(|| {
        let _ = ArtifactContractSpec::new(TargetKey::Native)
            .with_operations(&[OperationKey::SemanticJson, OperationKey::SemanticJson])
            .materialize();
    }));
    assert!(panics(|| {
        let _ = ArtifactContractSpec::new(TargetKey::Native)
            .with_operations(&[OperationKey::SemanticJson])
            .with_operations(&[]);
    }));
    assert!(panics(|| {
        let _ = ArtifactContractSpec::new(TargetKey::Native)
            .with_metadata(&[])
            .with_all_available_metadata();
    }));
    assert!(panics(|| {
        let _ = ArtifactContractSpec::new(TargetKey::Web)
            .with_transport_extensions(&[
                TransportCompiledExtensionKey::Editor,
                TransportCompiledExtensionKey::Editor,
            ])
            .materialize();
    }));
}

#[test]
fn canonical_validator_rejects_invalid_target_and_compiled_requirements() {
    let jpeg_and_svg = CapabilityKey::Jpeg.compact_bit() | CapabilityKey::Svg.compact_bit();
    assert!(panics(|| {
        validate_operation_bits(
            OperationKey::Jpeg.compact_bit(),
            TargetKey::Web,
            jpeg_and_svg,
        );
    }));

    assert!(panics(|| {
        validate_operation_bits(
            OperationKey::Png.compact_bit(),
            TargetKey::Native,
            CapabilityKey::Png.compact_bit(),
        );
    }));
}

#[test]
fn canonical_validator_rejects_owned_or_uncompiled_supplemental_capabilities() {
    assert!(panics(|| {
        validate_supplemental_capability_bits(
            CapabilityKey::Svg.compact_bit(),
            TargetKey::Native,
            CapabilityKey::Svg.compact_bit(),
        );
    }));
    assert!(panics(|| {
        validate_supplemental_capability_bits(
            CapabilityKey::Editor.compact_bit(),
            TargetKey::Web,
            0,
        );
    }));
}

#[test]
fn canonical_validator_rejects_unavailable_metadata_and_services() {
    assert!(panics(|| {
        validate_metadata_bits(
            MetadataKey::LintRuleCatalog.compact_bit(),
            0,
            MetadataKey::LintRuleCatalog.compact_bit(),
        );
    }));
    assert!(panics(|| {
        validate_constructor_service_bits(
            ConstructorServiceKey::IconRegistry.compact_bit(),
            CapabilityKey::Svg.compact_bit(),
            false,
        );
    }));
}

#[test]
fn binding_options_runtime_policy_is_native_target_only() {
    assert!(panics(|| {
        let _ = ArtifactContractSpec::new(TargetKey::Web)
            .with_operations(&[OperationKey::SemanticJson])
            .with_runtime_policy_exposure(RuntimePolicyExposure::BindingOptions)
            .materialize();
    }));
}

#[test]
fn system_adapter_declarations_fail_closed() {
    let clock = CapabilityKey::SystemClock.compact_bit();
    let timing = CapabilityKey::SystemTiming.compact_bit();

    assert!(panics(|| {
        validate_system_adapter_bits(
            clock,
            TargetKey::Web,
            RuntimePolicyExposure::BindingOptions,
            clock,
        );
    }));
    assert!(panics(|| {
        validate_system_adapter_bits(
            clock,
            TargetKey::Native,
            RuntimePolicyExposure::DeterministicOnly,
            clock,
        );
    }));
    assert!(panics(|| {
        validate_system_adapter_bits(
            timing,
            TargetKey::Native,
            RuntimePolicyExposure::BindingOptions,
            timing,
        );
    }));
    assert!(panics(|| {
        validate_system_adapter_bits(
            clock,
            TargetKey::Native,
            RuntimePolicyExposure::BindingOptions,
            0,
        );
    }));
    assert!(panics(|| {
        let _ = ArtifactContractSpec::new(TargetKey::Native)
            .with_system_adapters(&[CapabilityKey::SystemClock, CapabilityKey::SystemClock])
            .materialize();
    }));
    assert!(panics(|| {
        let _ = ArtifactContractSpec::new(TargetKey::Native)
            .with_system_adapters(&[])
            .with_system_adapters(&[]);
    }));
}

#[test]
fn operation_admission_uses_the_validated_contract() {
    let semantic = crate::BindingOperationKind::from_id("semantic-json").unwrap();
    assert_eq!(
        STATIC_SEMANTIC_CONTRACT
            .admit_operation(semantic)
            .unwrap()
            .operation(),
        semantic
    );

    let analysis = crate::BindingOperationKind::from_id("analysis-json").unwrap();
    let error = STATIC_SEMANTIC_CONTRACT
        .admit_operation(analysis)
        .unwrap_err();
    assert_eq!(error.status(), crate::BindingStatus::UnsupportedOperation);
    assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
    assert_eq!(error.capability_id(), Some("analysis"));
}

#[test]
fn validation_still_precedes_operation_admission() {
    let engine = STATIC_SEMANTIC_CONTRACT.create_engine(b"").unwrap();

    for error in [
        STATIC_SEMANTIC_CONTRACT
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
}

#[test]
fn known_but_hidden_base_operation_is_not_a_missing_capability() {
    let error = STATIC_EMPTY_CONTRACT
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
    let error = STATIC_SEMANTIC_CONTRACT
        .create_engine(br#"{"runtime_policy":"native"}"#)
        .err()
        .expect("deterministic-only contracts must reject native policy");

    assert_eq!(error.status(), crate::BindingStatus::OptionsJsonError);
    assert!(error.message().contains("is not exposed"));
}

#[test]
fn native_policy_reports_the_first_missing_transport_adapter() {
    const CONTRACT: ValidatedArtifactContract = ArtifactContractSpec::new(TargetKey::Native)
        .with_operations(&[OperationKey::SemanticJson])
        .with_runtime_policy_exposure(RuntimePolicyExposure::BindingOptions)
        .materialize();

    let error = CONTRACT
        .create_engine(br#"{"runtime_policy":"native"}"#)
        .err()
        .expect("native policy requires the transport-owned adapter set");

    assert_eq!(error.status(), crate::BindingStatus::UnsupportedOperation);
    assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
    assert_eq!(error.capability_id(), Some("system-clock"));
}

#[cfg(feature = "system-clock")]
#[test]
fn native_policy_uses_the_exact_transport_selection_after_feature_unification() {
    const CONTRACT: ValidatedArtifactContract = ArtifactContractSpec::new(TargetKey::Native)
        .with_operations(&[OperationKey::SemanticJson])
        .with_system_adapters(&[CapabilityKey::SystemClock])
        .with_runtime_policy_exposure(RuntimePolicyExposure::BindingOptions)
        .materialize();

    let error = CONTRACT
        .create_engine(br#"{"runtime_policy":"native"}"#)
        .err()
        .expect("globally compiled adapters must not widen the transport contract");

    assert!(
        CONTRACT
            .runtime_capabilities()
            .system_adapter_ids
            .is_empty()
    );
    assert_eq!(error.status(), crate::BindingStatus::UnsupportedOperation);
    assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
    assert_eq!(error.capability_id(), Some("system-timezone"));
}

#[cfg(feature = "svg")]
#[test]
fn named_helpers_cannot_bypass_contract_admission() {
    let engine = STATIC_SEMANTIC_CONTRACT.create_engine(b"").unwrap();
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
    let error = STATIC_SEMANTIC_CONTRACT
        .create_engine_with_services(b"", services)
        .err()
        .expect("unadvertised constructor services must be rejected");

    assert_eq!(error.status(), crate::BindingStatus::InvalidArgument);
    assert!(error.message().contains("is not exposed"));
}

#[cfg(feature = "svg")]
#[test]
fn operations_and_services_derive_text_measurement_providers() {
    const VENDORED_ONLY: ValidatedArtifactContract = ArtifactContractSpec::new(TargetKey::Native)
        .with_operations(&[OperationKey::Svg])
        .materialize();
    const WITH_HOST: ValidatedArtifactContract = ArtifactContractSpec::new(TargetKey::Native)
        .with_operations(&[OperationKey::Svg])
        .with_constructor_services(&[ConstructorServiceKey::HostTextMeasurement])
        .materialize();
    const WITH_ICONS: ValidatedArtifactContract = ArtifactContractSpec::new(TargetKey::Native)
        .with_operations(&[OperationKey::Svg])
        .with_constructor_services(&[ConstructorServiceKey::IconRegistry])
        .materialize();

    assert_eq!(
        VENDORED_ONLY
            .text_measurement_provider_keys()
            .collect::<Vec<_>>(),
        [TextMeasurementProviderKey::Vendored]
    );
    assert_eq!(
        WITH_HOST
            .text_measurement_provider_keys()
            .collect::<Vec<_>>(),
        [
            TextMeasurementProviderKey::HostCallback,
            TextMeasurementProviderKey::Vendored,
        ]
    );
    assert_eq!(
        WITH_ICONS
            .text_measurement_provider_keys()
            .collect::<Vec<_>>(),
        [TextMeasurementProviderKey::Vendored]
    );
}

#[cfg(feature = "png")]
#[test]
fn compiled_prerequisites_enable_the_pipeline_without_advertising_its_output() {
    const PNG_CONTRACT: ValidatedArtifactContract = ArtifactContractSpec::new(TargetKey::Native)
        .with_operations(&[OperationKey::Png])
        .materialize();

    assert_eq!(
        PNG_CONTRACT.capability_keys().collect::<Vec<_>>(),
        [CapabilityKey::Png]
    );
    assert_eq!(
        PNG_CONTRACT.output_keys().collect::<Vec<_>>(),
        [OutputKey::Png]
    );
    assert_eq!(
        PNG_CONTRACT
            .text_measurement_provider_keys()
            .collect::<Vec<_>>(),
        [TextMeasurementProviderKey::Vendored]
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn descriptor_implications_are_closed_automatically() {
    const CONTRACT: ValidatedArtifactContract = ArtifactContractSpec::new(TargetKey::Native)
        .with_operations(&[OperationKey::Svg])
        .with_supplemental_capabilities(&[CapabilityKey::LayoutElk])
        .materialize();

    assert!(CONTRACT.exposes_capability(CapabilityKey::Svg));
    assert!(CONTRACT.exposes_capability(CapabilityKey::LayoutElk));
}
