use crate::capability::{
    CapabilityKey, OperationKey, OutputKey, TargetKey, TransportCompiledExtensionKey,
    compiled_capability_keys, compiled_operation_keys,
};
use crate::metadata_registry::{MetadataKey, metadata_spec};
use crate::option_contract::{BindingOptionGroupKey, compiled_option_group_keys};
use crate::payload_contract::BindingPayloadSchemaKey;
use crate::service_contract::{
    ConstructorServiceKey, RuntimePolicyExposure, TextMeasurementProviderKey,
};
use crate::{BindingEngineServices, BindingError};
use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

mod surface;
mod validation;

use surface::CompiledSelection;
pub use surface::TransportExposure;
use validation::{
    close_capability_implications, invalid_artifact_contract, is_supplemental_capability,
    resolve_metadata, resolve_system_adapters, validate_compiled_capabilities,
    validate_compiled_operation_requirements, validate_services,
};

/// Descriptor-derived facts compiled into `merman-bindings-core` plus typed transport extensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledBindingSurface {
    capabilities: BTreeSet<CapabilityKey>,
    operations: BTreeSet<OperationKey>,
    metadata: BTreeSet<MetadataKey>,
    option_groups: BTreeSet<BindingOptionGroupKey>,
    text_measurement_providers: BTreeSet<TextMeasurementProviderKey>,
    constructor_services: BTreeSet<ConstructorServiceKey>,
}

impl CompiledBindingSurface {
    /// Captures the exact binding-core facts for the current Cargo feature selection.
    #[must_use]
    pub fn current() -> Self {
        let capabilities = compiled_capability_keys().clone();

        let operations = compiled_operation_keys();

        let metadata = MetadataKey::ALL
            .iter()
            .copied()
            .filter(|key| metadata_spec(*key).is_available(&capabilities))
            .collect();

        let option_groups = compiled_option_group_keys();

        let text_measurement_providers = TextMeasurementProviderKey::ALL
            .iter()
            .copied()
            .filter(|provider| provider.spec().is_compiled(&capabilities))
            .collect();

        let constructor_services = ConstructorServiceKey::ALL
            .iter()
            .copied()
            .filter(|service| service.spec().is_compiled(&capabilities))
            .collect();

        Self {
            capabilities,
            operations,
            metadata,
            option_groups,
            text_measurement_providers,
            constructor_services,
        }
    }

    /// Adds a capability compiled by the transport crate rather than binding-core itself.
    pub fn with_transport_extension(
        mut self,
        extension: TransportCompiledExtensionKey,
    ) -> Result<Self, BindingError> {
        let capability = extension.capability();
        if !self.capabilities.insert(capability) {
            return Err(invalid_artifact_contract(format!(
                "compiled capability `{}` was declared more than once",
                capability.id()
            )));
        }
        Ok(self)
    }

    pub fn validate(
        &self,
        exposure: TransportExposure,
    ) -> Result<ValidatedArtifactContract, BindingError> {
        let TransportExposure {
            target,
            capabilities: requested_capabilities,
            operations: requested_operations,
            metadata: requested_metadata,
            payload_schemas,
            text_measurement_providers: requested_providers,
            constructor_services,
            runtime_policy,
        } = exposure;

        let operations = self.resolve_operations(target, requested_operations)?;
        let mut capabilities = self.resolve_capabilities(target, requested_capabilities)?;
        let mut compiled_prerequisites = BTreeSet::new();
        for operation in &operations {
            if let Some(capability) = operation.spec().capability {
                capabilities.insert(capability);
            }
            compiled_prerequisites.extend(operation.spec().compiled_prerequisites.iter().copied());
        }
        close_capability_implications(&mut capabilities);
        close_capability_implications(&mut compiled_prerequisites);
        validate_compiled_capabilities(target, &capabilities, &self.capabilities)?;
        validate_compiled_operation_requirements(
            target,
            &operations,
            &compiled_prerequisites,
            &self.capabilities,
        )?;
        let uses_svg_pipeline = capabilities.contains(&CapabilityKey::Svg)
            || compiled_prerequisites.contains(&CapabilityKey::Svg);

        if runtime_policy == RuntimePolicyExposure::BindingOptions && target != TargetKey::Native {
            return Err(invalid_artifact_contract(format!(
                "runtime-policy binding options are not valid for target `{}`",
                target.id()
            )));
        }
        let system_adapters = resolve_system_adapters(target, runtime_policy, &self.capabilities);
        capabilities.extend(system_adapters.iter().copied());

        let outputs = operations
            .iter()
            .filter_map(|operation| operation.spec().output)
            .collect::<BTreeSet<_>>();

        let metadata = resolve_metadata(requested_metadata, &capabilities, &self.metadata)?;
        let text_measurement_providers =
            self.resolve_providers(requested_providers, uses_svg_pipeline)?;
        validate_services(
            &constructor_services,
            &self.constructor_services,
            uses_svg_pipeline,
            &text_measurement_providers,
        )?;
        let option_groups = self.option_groups.clone();

        Ok(ValidatedArtifactContract {
            target,
            capabilities,
            outputs,
            operations,
            metadata,
            payload_schemas,
            option_groups,
            text_measurement_providers,
            constructor_services,
            system_adapters,
            runtime_policy,
        })
    }

    fn resolve_operations(
        &self,
        target: TargetKey,
        selection: CompiledSelection<OperationKey>,
    ) -> Result<BTreeSet<OperationKey>, BindingError> {
        let operations = match selection {
            CompiledSelection::AllCompiled => self
                .operations
                .iter()
                .copied()
                .filter(|operation| operation.spec().targets.contains(&target))
                .collect(),
            CompiledSelection::Explicit(operations) => operations,
        };
        for operation in &operations {
            if !self.operations.contains(operation) {
                return Err(invalid_artifact_contract(format!(
                    "transport operation `{}` is not compiled",
                    operation.id()
                )));
            }
            if !operation.spec().targets.contains(&target) {
                return Err(invalid_artifact_contract(format!(
                    "transport operation `{}` is not valid for target `{}`",
                    operation.id(),
                    target.id()
                )));
            }
        }
        Ok(operations)
    }

    fn resolve_capabilities(
        &self,
        target: TargetKey,
        selection: CompiledSelection<CapabilityKey>,
    ) -> Result<BTreeSet<CapabilityKey>, BindingError> {
        let capabilities = match selection {
            CompiledSelection::AllCompiled => self
                .capabilities
                .iter()
                .copied()
                .filter(|capability| {
                    is_supplemental_capability(*capability)
                        && capability.spec().targets.contains(&target)
                })
                .collect(),
            CompiledSelection::Explicit(capabilities) => capabilities,
        };
        for capability in &capabilities {
            if !is_supplemental_capability(*capability) {
                return Err(invalid_artifact_contract(format!(
                    "capability `{}` is derived from operations, runtime policy, or another owning contract and cannot be selected as supplemental",
                    capability.id()
                )));
            }
        }
        Ok(capabilities)
    }

    fn resolve_providers(
        &self,
        mut providers: BTreeSet<TextMeasurementProviderKey>,
        uses_svg_pipeline: bool,
    ) -> Result<BTreeSet<TextMeasurementProviderKey>, BindingError> {
        for provider in &providers {
            if !self.text_measurement_providers.contains(provider) {
                return Err(invalid_artifact_contract(format!(
                    "text-measurement provider `{}` is not compiled",
                    provider.id()
                )));
            }
            let spec = provider.spec();
            if spec.derived_from_svg_pipeline() {
                return Err(invalid_artifact_contract(format!(
                    "text-measurement provider `{}` is derived automatically from SVG exposure",
                    provider.id()
                )));
            }
            if spec.requires_svg_pipeline() && !uses_svg_pipeline {
                return Err(invalid_artifact_contract(format!(
                    "text-measurement provider `{}` requires an exposed operation backed by the SVG rendering pipeline",
                    provider.id()
                )));
            }
        }

        if uses_svg_pipeline {
            for provider in TextMeasurementProviderKey::ALL
                .iter()
                .copied()
                .filter(|provider| provider.spec().derived_from_svg_pipeline())
            {
                if !self.text_measurement_providers.contains(&provider) {
                    return Err(invalid_artifact_contract(format!(
                        "SVG is compiled without the required `{}` text-measurement provider",
                        provider.id()
                    )));
                }
                providers.insert(provider);
            }
        }
        Ok(providers)
    }
}

/// Immutable checked selection that owns discovery, operation admission, and metadata dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedArtifactContract {
    target: TargetKey,
    capabilities: BTreeSet<CapabilityKey>,
    outputs: BTreeSet<OutputKey>,
    operations: BTreeSet<OperationKey>,
    metadata: BTreeSet<MetadataKey>,
    payload_schemas: BTreeSet<BindingPayloadSchemaKey>,
    option_groups: BTreeSet<BindingOptionGroupKey>,
    text_measurement_providers: BTreeSet<TextMeasurementProviderKey>,
    constructor_services: BTreeSet<ConstructorServiceKey>,
    system_adapters: BTreeSet<CapabilityKey>,
    runtime_policy: RuntimePolicyExposure,
}

impl ValidatedArtifactContract {
    #[must_use]
    pub const fn target(&self) -> TargetKey {
        self.target
    }

    #[must_use]
    pub fn capability_keys(&self) -> impl Iterator<Item = CapabilityKey> + '_ {
        self.capabilities.iter().copied()
    }

    #[must_use]
    pub fn output_keys(&self) -> impl Iterator<Item = OutputKey> + '_ {
        self.outputs.iter().copied()
    }

    #[must_use]
    pub fn operation_keys(&self) -> impl Iterator<Item = OperationKey> + '_ {
        self.operations.iter().copied()
    }

    #[must_use]
    pub fn metadata_keys(&self) -> impl Iterator<Item = MetadataKey> + '_ {
        self.metadata.iter().copied()
    }

    #[must_use]
    pub fn payload_schema_keys(&self) -> impl Iterator<Item = BindingPayloadSchemaKey> + '_ {
        self.payload_schemas.iter().copied()
    }

    #[must_use]
    pub fn option_group_keys(&self) -> impl Iterator<Item = BindingOptionGroupKey> + '_ {
        self.option_groups.iter().copied()
    }

    #[must_use]
    pub fn text_measurement_provider_keys(
        &self,
    ) -> impl Iterator<Item = TextMeasurementProviderKey> + '_ {
        self.text_measurement_providers.iter().copied()
    }

    #[must_use]
    pub fn constructor_service_keys(&self) -> impl Iterator<Item = ConstructorServiceKey> + '_ {
        self.constructor_services.iter().copied()
    }

    #[must_use]
    pub fn system_adapter_keys(&self) -> impl Iterator<Item = CapabilityKey> + '_ {
        self.system_adapters.iter().copied()
    }

    #[must_use]
    pub const fn runtime_policy_exposure(&self) -> RuntimePolicyExposure {
        self.runtime_policy
    }

    pub(crate) fn exposes_metadata(&self, key: MetadataKey) -> bool {
        self.metadata.contains(&key)
    }

    pub(crate) fn exposes_operation(&self, key: OperationKey) -> bool {
        self.operations.contains(&key)
    }

    pub(crate) fn exposes_capability(&self, key: CapabilityKey) -> bool {
        self.capabilities.contains(&key)
    }

    pub(crate) fn validate_engine_services(
        &self,
        services: &BindingEngineServices,
    ) -> Result<(), BindingError> {
        for service in services.service_keys() {
            if !self.constructor_services.contains(&service) {
                return Err(BindingError::invalid_argument(format!(
                    "constructor service `{}` is not exposed by this artifact contract",
                    service.id()
                )));
            }
        }
        Ok(())
    }
}

static DEFAULT_ARTIFACT_CONTRACT: OnceLock<Arc<ValidatedArtifactContract>> = OnceLock::new();

pub(crate) fn default_artifact_contract() -> Arc<ValidatedArtifactContract> {
    Arc::clone(DEFAULT_ARTIFACT_CONTRACT.get_or_init(|| {
        let exposure = TransportExposure::for_target(TargetKey::Native)
            .with_all_compiled_operations()
            .and_then(TransportExposure::with_all_compiled_supplemental_capabilities)
            .and_then(TransportExposure::with_all_available_metadata)
            .and_then(|exposure| {
                exposure.with_payload_schemas(BindingPayloadSchemaKey::ALL.iter().copied())
            })
            .expect("the default Rust binding surface is declared once")
            .with_runtime_policy_exposure(RuntimePolicyExposure::BindingOptions);
        #[cfg(feature = "svg")]
        let exposure = exposure
            .with_text_measurement_providers([TextMeasurementProviderKey::HostCallback])
            .and_then(|exposure| {
                exposure.with_constructor_services([ConstructorServiceKey::HostTextMeasurement])
            })
            .expect("the default Rust SVG service surface is coherent");

        Arc::new(
            CompiledBindingSurface::current()
                .validate(exposure)
                .expect("the default Rust artifact contract matches its compiled surface"),
        )
    }))
}

#[cfg(test)]
#[path = "artifact_contract/tests.rs"]
mod tests;
