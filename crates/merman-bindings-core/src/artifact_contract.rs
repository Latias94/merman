use crate::capability::{
    CapabilityKey, OperationKey, OutputKey, TargetKey, TransportCompiledExtensionKey,
    compiled_capability_keys,
};
use crate::key_set::KeySet;
use crate::metadata_registry::{MetadataKey, metadata_spec};
use crate::option_contract::BindingOptionGroupKey;
use crate::payload_contract::BindingPayloadSchemaKey;
use crate::service_contract::{
    ConstructorServiceKey, RuntimePolicyExposure, TextMeasurementProviderKey,
    TextMeasurementProviderSource,
};
use crate::{BindingEngineServices, BindingError, BindingTransportKey};
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StaticMetadataSelection {
    Explicit(&'static [MetadataKey]),
    AllAvailable,
}

/// Static typed declaration of one transport's callable artifact surface.
///
/// The declaration contains no raw capability or operation IDs and can live in a `const` or
/// `static`. [`Self::materialize`] verifies and produces the immutable
/// [`ValidatedArtifactContract`] snapshot during constant evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactContractSpec {
    target: TargetKey,
    capabilities: &'static [CapabilityKey],
    operations: &'static [OperationKey],
    metadata: StaticMetadataSelection,
    transport: BindingTransportKey,
    constructor_services: &'static [ConstructorServiceKey],
    system_adapters: &'static [CapabilityKey],
    runtime_policy: RuntimePolicyExposure,
    transport_extensions: &'static [TransportCompiledExtensionKey],
    configured_fields: u8,
}

const OPERATIONS_CONFIGURED: u8 = 1 << 0;
const CAPABILITIES_CONFIGURED: u8 = 1 << 1;
const METADATA_CONFIGURED: u8 = 1 << 2;
const CONSTRUCTOR_SERVICES_CONFIGURED: u8 = 1 << 3;
const RUNTIME_POLICY_CONFIGURED: u8 = 1 << 4;
const TRANSPORT_EXTENSIONS_CONFIGURED: u8 = 1 << 5;
const SYSTEM_ADAPTERS_CONFIGURED: u8 = 1 << 6;

impl ArtifactContractSpec {
    #[must_use]
    pub const fn new(target: TargetKey, transport: BindingTransportKey) -> Self {
        if !transport.spec().supports_target(target) {
            panic!("binding transport does not support the selected target");
        }
        Self {
            target,
            capabilities: &[],
            operations: &[],
            metadata: StaticMetadataSelection::Explicit(&[]),
            transport,
            constructor_services: &[],
            system_adapters: &[],
            runtime_policy: RuntimePolicyExposure::DeterministicOnly,
            transport_extensions: &[],
            configured_fields: 0,
        }
    }

    #[must_use]
    pub const fn with_operations(mut self, operations: &'static [OperationKey]) -> Self {
        self.configure_once(OPERATIONS_CONFIGURED);
        self.operations = operations;
        self
    }

    #[must_use]
    pub const fn with_supplemental_capabilities(
        mut self,
        capabilities: &'static [CapabilityKey],
    ) -> Self {
        self.configure_once(CAPABILITIES_CONFIGURED);
        self.capabilities = capabilities;
        self
    }

    #[must_use]
    pub const fn with_metadata(mut self, metadata: &'static [MetadataKey]) -> Self {
        self.configure_once(METADATA_CONFIGURED);
        self.metadata = StaticMetadataSelection::Explicit(metadata);
        self
    }

    #[must_use]
    pub const fn with_all_available_metadata(mut self) -> Self {
        self.configure_once(METADATA_CONFIGURED);
        self.metadata = StaticMetadataSelection::AllAvailable;
        self
    }

    /// Selects the exact constructor services implemented by this artifact recipe.
    ///
    /// The concrete slice may narrow the transport registry's candidate services for feature or
    /// target availability, but cannot add a service the selected transport cannot install.
    #[must_use]
    pub const fn with_constructor_services(
        mut self,
        constructor_services: &'static [ConstructorServiceKey],
    ) -> Self {
        self.configure_once(CONSTRUCTOR_SERVICES_CONFIGURED);
        self.constructor_services = constructor_services;
        self
    }

    /// Declares the exact native system adapters owned by this transport artifact.
    ///
    /// The selection is used for native-policy admission. The public capability catalog exposes
    /// the native adapter set only when all required adapters are present.
    #[must_use]
    pub const fn with_system_adapters(mut self, system_adapters: &'static [CapabilityKey]) -> Self {
        self.configure_once(SYSTEM_ADAPTERS_CONFIGURED);
        self.system_adapters = system_adapters;
        self
    }

    #[must_use]
    pub const fn with_runtime_policy_exposure(
        mut self,
        runtime_policy: RuntimePolicyExposure,
    ) -> Self {
        self.configure_once(RUNTIME_POLICY_CONFIGURED);
        self.runtime_policy = runtime_policy;
        self
    }

    #[must_use]
    pub const fn with_transport_extensions(
        mut self,
        transport_extensions: &'static [TransportCompiledExtensionKey],
    ) -> Self {
        self.configure_once(TRANSPORT_EXTENSIONS_CONFIGURED);
        self.transport_extensions = transport_extensions;
        self
    }

    const fn configure_once(&mut self, field: u8) {
        if self.configured_fields & field != 0 {
            panic!("artifact contract field configured more than once");
        }
        self.configured_fields |= field;
    }

    /// Materializes a verified, allocation-free snapshot.
    ///
    /// Declare the spec and snapshot in a `const` or `static` so invalid transport declarations
    /// fail during compilation rather than during process initialization.
    #[must_use]
    #[inline(always)]
    pub const fn materialize(self) -> ValidatedArtifactContract {
        let mut compiled_capabilities = compiled_capability_keys().bits();
        let extension_capabilities = transport_extension_bits(self.transport_extensions);
        if compiled_capabilities & extension_capabilities != 0 {
            panic!("transport extension capability was compiled more than once");
        }
        compiled_capabilities |= extension_capabilities;

        let operations = operation_slice_bits(self.operations);
        validate_operation_bits(operations, self.target, compiled_capabilities);

        let supplemental_capabilities = capability_slice_bits(self.capabilities);
        validate_supplemental_capability_bits(
            supplemental_capabilities,
            self.target,
            compiled_capabilities,
        );

        let mut capabilities = supplemental_capabilities;
        let mut compiled_prerequisites = 0;
        let mut operation_index = 0;
        while operation_index < OperationKey::ALL.len() {
            let operation = OperationKey::ALL[operation_index];
            if operations & operation.compact_bit() != 0 {
                let spec = operation.spec();
                if let Some(capability) = spec.capability {
                    capabilities |= capability.compact_bit();
                }
                compiled_prerequisites |= capability_slice_bits(spec.compiled_prerequisites);
            }
            operation_index += 1;
        }
        capabilities = close_capability_implication_bits(capabilities);
        compiled_prerequisites = close_capability_implication_bits(compiled_prerequisites);
        validate_capability_bits(capabilities, self.target, compiled_capabilities);
        validate_capability_bits(compiled_prerequisites, self.target, compiled_capabilities);

        let uses_svg_pipeline = capabilities & CapabilityKey::Svg.compact_bit() != 0
            || compiled_prerequisites & CapabilityKey::Svg.compact_bit() != 0;
        if matches!(self.runtime_policy, RuntimePolicyExposure::BindingOptions)
            && !matches!(self.target, TargetKey::Native)
        {
            panic!("binding-options runtime policy requires the native target");
        }
        let selected_system_adapters = capability_slice_bits(self.system_adapters);
        validate_system_adapter_bits(
            selected_system_adapters,
            self.target,
            self.runtime_policy,
            compiled_capabilities,
        );
        let exposed_system_adapters = exposed_system_adapter_bits(selected_system_adapters);
        capabilities |= exposed_system_adapters;

        let outputs = output_bits_for_operations(operations);
        let compiled_metadata = compiled_metadata_bits(compiled_capabilities);
        let metadata = match self.metadata {
            StaticMetadataSelection::Explicit(metadata) => {
                let selected = metadata_slice_bits(metadata);
                validate_metadata_bits(selected, capabilities, compiled_metadata);
                selected
            }
            StaticMetadataSelection::AllAvailable => {
                available_metadata_bits(compiled_metadata, capabilities)
            }
        };
        let exposure = self.transport.spec();
        let payload_schemas = payload_schema_slice_bits(exposure.payload_schemas());
        let candidate_services =
            constructor_service_slice_bits(exposure.constructor_service_candidates());
        let constructor_services = constructor_service_slice_bits(self.constructor_services);
        if constructor_services & !candidate_services != 0 {
            panic!("constructor service is not exposed by the selected transport");
        }
        validate_constructor_service_bits(
            constructor_services,
            compiled_capabilities,
            uses_svg_pipeline,
        );

        ValidatedArtifactContract {
            target: self.target,
            capabilities: KeySet::from_bits(capabilities),
            outputs: KeySet::from_bits(outputs),
            operations: KeySet::from_bits(operations),
            metadata: KeySet::from_bits(metadata),
            payload_schemas: KeySet::from_bits(payload_schemas),
            option_groups: KeySet::from_bits(option_group_bits(capabilities, uses_svg_pipeline)),
            text_measurement_providers: KeySet::from_bits(text_measurement_provider_bits(
                uses_svg_pipeline,
                constructor_services,
            )),
            constructor_services: KeySet::from_bits(constructor_services),
            selected_system_adapters: KeySet::from_bits(selected_system_adapters),
            system_adapters: KeySet::from_bits(exposed_system_adapters),
            runtime_policy: self.runtime_policy,
        }
    }
}

const fn transport_extension_bits(extensions: &[TransportCompiledExtensionKey]) -> u64 {
    let mut bits = 0;
    let mut index = 0;
    while index < extensions.len() {
        let bit = extensions[index].capability().compact_bit();
        if bits & bit != 0 {
            panic!("transport extension was declared more than once");
        }
        bits |= bit;
        index += 1;
    }
    bits
}

const fn capability_slice_bits(capabilities: &[CapabilityKey]) -> u64 {
    let mut bits = 0;
    let mut index = 0;
    while index < capabilities.len() {
        let bit = capabilities[index].compact_bit();
        if bits & bit != 0 {
            panic!("capability was declared more than once");
        }
        bits |= bit;
        index += 1;
    }
    bits
}

const fn operation_slice_bits(operations: &[OperationKey]) -> u64 {
    let mut bits = 0;
    let mut index = 0;
    while index < operations.len() {
        let bit = operations[index].compact_bit();
        if bits & bit != 0 {
            panic!("operation was declared more than once");
        }
        bits |= bit;
        index += 1;
    }
    bits
}

const fn metadata_slice_bits(metadata: &[MetadataKey]) -> u64 {
    let mut bits = 0;
    let mut index = 0;
    while index < metadata.len() {
        let bit = metadata[index].compact_bit();
        if bits & bit != 0 {
            panic!("metadata catalog was declared more than once");
        }
        bits |= bit;
        index += 1;
    }
    bits
}

const fn payload_schema_slice_bits(schemas: &[BindingPayloadSchemaKey]) -> u64 {
    let mut bits = 0;
    let mut index = 0;
    while index < schemas.len() {
        let bit = schemas[index].compact_bit();
        if bits & bit != 0 {
            panic!("payload schema was declared more than once");
        }
        bits |= bit;
        index += 1;
    }
    bits
}

const fn constructor_service_slice_bits(services: &[ConstructorServiceKey]) -> u64 {
    let mut bits = 0;
    let mut index = 0;
    while index < services.len() {
        let bit = services[index].compact_bit();
        if bits & bit != 0 {
            panic!("constructor service was declared more than once");
        }
        bits |= bit;
        index += 1;
    }
    bits
}

const fn validate_operation_bits(operations: u64, target: TargetKey, compiled_capabilities: u64) {
    let mut index = 0;
    while index < OperationKey::ALL.len() {
        let operation = OperationKey::ALL[index];
        if operations & operation.compact_bit() != 0 {
            if !operation_is_compiled(operation, compiled_capabilities) {
                panic!("transport operation is not compiled");
            }
            if !targets_contain(operation.spec().targets, target) {
                panic!("transport operation is not valid for the selected target");
            }
        }
        index += 1;
    }
}

const fn operation_is_compiled(operation: OperationKey, compiled_capabilities: u64) -> bool {
    let spec = operation.spec();
    if let Some(capability) = spec.capability {
        if compiled_capabilities & capability.compact_bit() == 0 {
            return false;
        }
    }
    let prerequisites = capability_slice_bits(spec.compiled_prerequisites);
    compiled_capabilities & prerequisites == prerequisites
}

const fn validate_supplemental_capability_bits(
    capabilities: u64,
    target: TargetKey,
    compiled_capabilities: u64,
) {
    let mut index = 0;
    while index < CapabilityKey::ALL.len() {
        let capability = CapabilityKey::ALL[index];
        if capabilities & capability.compact_bit() != 0 {
            if !is_supplemental_capability_const(capability) {
                panic!("owned capability cannot be selected as supplemental");
            }
            if compiled_capabilities & capability.compact_bit() == 0 {
                panic!("supplemental capability is not compiled");
            }
            if !targets_contain(capability.spec().targets, target) {
                panic!("supplemental capability is not valid for the selected target");
            }
        }
        index += 1;
    }
}

const fn is_supplemental_capability_const(capability: CapabilityKey) -> bool {
    let kind = capability.spec().kind;
    (const_str_eq(kind, "engine") || const_str_eq(kind, "api"))
        && !capability_is_operation_owned(capability)
}

const fn capability_is_operation_owned(capability: CapabilityKey) -> bool {
    let expected = capability.compact_bit();
    let mut index = 0;
    while index < OperationKey::ALL.len() {
        if let Some(owner) = OperationKey::ALL[index].spec().capability
            && owner.compact_bit() == expected
        {
            return true;
        }
        index += 1;
    }
    false
}

const fn const_str_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

const fn close_capability_implication_bits(mut capabilities: u64) -> u64 {
    loop {
        let old_bits = capabilities;
        let mut index = 0;
        while index < CapabilityKey::ALL.len() {
            let capability = CapabilityKey::ALL[index];
            if old_bits & capability.compact_bit() != 0 {
                capabilities |= capability_slice_bits(capability.spec().implications);
            }
            index += 1;
        }
        if capabilities == old_bits {
            return capabilities;
        }
    }
}

const fn validate_capability_bits(
    capabilities: u64,
    target: TargetKey,
    compiled_capabilities: u64,
) {
    if capabilities & !compiled_capabilities != 0 {
        panic!("artifact capability requirement is not compiled");
    }
    let mut index = 0;
    while index < CapabilityKey::ALL.len() {
        let capability = CapabilityKey::ALL[index];
        if capabilities & capability.compact_bit() != 0
            && !targets_contain(capability.spec().targets, target)
        {
            panic!("artifact capability is not valid for the selected target");
        }
        index += 1;
    }
}

const fn validate_system_adapter_bits(
    system_adapters: u64,
    target: TargetKey,
    runtime_policy: RuntimePolicyExposure,
    compiled_capabilities: u64,
) {
    if system_adapters == 0 {
        return;
    }
    if !matches!(target, TargetKey::Native) {
        panic!("system adapters require the native target");
    }
    if !matches!(runtime_policy, RuntimePolicyExposure::BindingOptions) {
        panic!("system adapters require binding-options runtime policy exposure");
    }
    let allowed = CapabilityKey::SystemClock.compact_bit()
        | CapabilityKey::SystemRandom.compact_bit()
        | CapabilityKey::SystemTimezone.compact_bit();
    if system_adapters & !allowed != 0 {
        panic!("artifact system adapter selection contains a non-policy adapter");
    }
    if system_adapters & !compiled_capabilities != 0 {
        panic!("artifact system adapter is not compiled");
    }
}

const NATIVE_SYSTEM_ADAPTER_BITS: u64 = CapabilityKey::SystemClock.compact_bit()
    | CapabilityKey::SystemRandom.compact_bit()
    | CapabilityKey::SystemTimezone.compact_bit();

const fn exposed_system_adapter_bits(selected_system_adapters: u64) -> u64 {
    if selected_system_adapters == NATIVE_SYSTEM_ADAPTER_BITS {
        NATIVE_SYSTEM_ADAPTER_BITS
    } else {
        0
    }
}

const fn output_bits_for_operations(operations: u64) -> u64 {
    let mut outputs = 0;
    let mut index = 0;
    while index < OperationKey::ALL.len() {
        let operation = OperationKey::ALL[index];
        if operations & operation.compact_bit() != 0 {
            if let Some(output) = operation.spec().output {
                outputs |= output.compact_bit();
            }
        }
        index += 1;
    }
    outputs
}

const fn compiled_metadata_bits(compiled_capabilities: u64) -> u64 {
    let mut metadata = 0;
    let mut index = 0;
    while index < MetadataKey::ALL.len() {
        let key = MetadataKey::ALL[index];
        if metadata_is_available(key, compiled_capabilities) {
            metadata |= key.compact_bit();
        }
        index += 1;
    }
    metadata
}

const fn available_metadata_bits(compiled_metadata: u64, capabilities: u64) -> u64 {
    let mut metadata = 0;
    let mut index = 0;
    while index < MetadataKey::ALL.len() {
        let key = MetadataKey::ALL[index];
        if compiled_metadata & key.compact_bit() != 0 && metadata_is_available(key, capabilities) {
            metadata |= key.compact_bit();
        }
        index += 1;
    }
    metadata
}

const fn validate_metadata_bits(metadata: u64, capabilities: u64, compiled_metadata: u64) {
    if metadata & !compiled_metadata != 0 {
        panic!("metadata catalog is not compiled");
    }
    let mut index = 0;
    while index < MetadataKey::ALL.len() {
        let key = MetadataKey::ALL[index];
        if metadata & key.compact_bit() != 0 && !metadata_is_available(key, capabilities) {
            panic!("metadata catalog is unavailable for the selected capabilities");
        }
        index += 1;
    }
}

const fn metadata_is_available(key: MetadataKey, capabilities: u64) -> bool {
    match metadata_spec(key).required_capability() {
        Some(capability) => capabilities & capability.compact_bit() != 0,
        None => true,
    }
}

const fn validate_constructor_service_bits(
    services: u64,
    compiled_capabilities: u64,
    uses_svg_pipeline: bool,
) {
    let compiled_svg = compiled_capabilities & CapabilityKey::Svg.compact_bit() != 0;
    let mut index = 0;
    while index < ConstructorServiceKey::ALL.len() {
        let service = ConstructorServiceKey::ALL[index];
        if services & service.compact_bit() != 0 {
            if !service.spec().is_available(compiled_svg) {
                panic!("constructor service is not compiled");
            }
            if !service.spec().is_available(uses_svg_pipeline) {
                panic!("constructor service requires the SVG pipeline");
            }
        }
        index += 1;
    }
}

const fn option_group_bits(capabilities: u64, uses_svg_pipeline: bool) -> u64 {
    let mut groups = 0;
    let mut index = 0;
    while index < BindingOptionGroupKey::ALL.len() {
        let key = BindingOptionGroupKey::ALL[index];
        let spec = key.spec();
        let mut available =
            spec.always_available() || (spec.requires_svg_pipeline() && uses_svg_pipeline);
        let any_capabilities = spec.any_capabilities();
        let mut capability_index = 0;
        while capability_index < any_capabilities.len() {
            if capabilities & any_capabilities[capability_index].compact_bit() != 0 {
                available = true;
            }
            capability_index += 1;
        }
        if available {
            groups |= key.compact_bit();
        }
        index += 1;
    }
    groups
}

const fn text_measurement_provider_bits(uses_svg_pipeline: bool, constructor_services: u64) -> u64 {
    let mut providers = 0;
    let mut index = 0;
    while index < TextMeasurementProviderKey::ALL.len() {
        let provider = TextMeasurementProviderKey::ALL[index];
        let exposed = match provider.source() {
            TextMeasurementProviderSource::SvgPipeline => uses_svg_pipeline,
            TextMeasurementProviderSource::ConstructorService(service) => {
                constructor_services & service.compact_bit() != 0
            }
        };
        if exposed {
            providers |= provider.compact_bit();
        }
        index += 1;
    }
    providers
}

const fn targets_contain(targets: &[TargetKey], target: TargetKey) -> bool {
    let expected = target_bit(target);
    let mut index = 0;
    while index < targets.len() {
        if target_bit(targets[index]) == expected {
            return true;
        }
        index += 1;
    }
    false
}

const fn target_bit(target: TargetKey) -> u8 {
    1_u8 << (target as u32)
}

/// Immutable checked selection that owns discovery, operation admission, and metadata dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedArtifactContract {
    target: TargetKey,
    capabilities: KeySet<CapabilityKey>,
    outputs: KeySet<OutputKey>,
    operations: KeySet<OperationKey>,
    metadata: KeySet<MetadataKey>,
    payload_schemas: KeySet<BindingPayloadSchemaKey>,
    option_groups: KeySet<BindingOptionGroupKey>,
    text_measurement_providers: KeySet<TextMeasurementProviderKey>,
    constructor_services: KeySet<ConstructorServiceKey>,
    selected_system_adapters: KeySet<CapabilityKey>,
    system_adapters: KeySet<CapabilityKey>,
    runtime_policy: RuntimePolicyExposure,
}

impl ValidatedArtifactContract {
    #[must_use]
    pub const fn target(&self) -> TargetKey {
        self.target
    }

    #[must_use]
    pub fn capability_keys(&self) -> impl Iterator<Item = CapabilityKey> + '_ {
        self.capabilities.iter()
    }

    #[must_use]
    pub fn output_keys(&self) -> impl Iterator<Item = OutputKey> + '_ {
        self.outputs.iter()
    }

    #[must_use]
    pub fn operation_keys(&self) -> impl Iterator<Item = OperationKey> + '_ {
        self.operations.iter()
    }

    #[must_use]
    pub fn metadata_keys(&self) -> impl Iterator<Item = MetadataKey> + '_ {
        self.metadata.iter()
    }

    #[must_use]
    pub fn payload_schema_keys(&self) -> impl Iterator<Item = BindingPayloadSchemaKey> + '_ {
        self.payload_schemas.iter()
    }

    #[must_use]
    pub fn option_group_keys(&self) -> impl Iterator<Item = BindingOptionGroupKey> + '_ {
        self.option_groups.iter()
    }

    #[must_use]
    pub fn text_measurement_provider_keys(
        &self,
    ) -> impl Iterator<Item = TextMeasurementProviderKey> + '_ {
        self.text_measurement_providers.iter()
    }

    #[must_use]
    pub fn constructor_service_keys(&self) -> impl Iterator<Item = ConstructorServiceKey> + '_ {
        self.constructor_services.iter()
    }

    #[must_use]
    pub fn system_adapter_keys(&self) -> impl Iterator<Item = CapabilityKey> + '_ {
        self.system_adapters.iter()
    }

    #[must_use]
    pub const fn runtime_policy_exposure(&self) -> RuntimePolicyExposure {
        self.runtime_policy
    }

    pub(crate) fn exposes_metadata(&self, key: MetadataKey) -> bool {
        self.metadata.contains(key)
    }

    pub(crate) fn exposes_operation(&self, key: OperationKey) -> bool {
        self.operations.contains(key)
    }

    pub(crate) fn exposes_capability(&self, key: CapabilityKey) -> bool {
        self.capabilities.contains(key)
    }

    pub(crate) fn exposes_option_group(&self, key: BindingOptionGroupKey) -> bool {
        self.option_groups.contains(key)
    }

    /// Applies the resource owner's capability/output closure to this exact artifact selection.
    pub(crate) fn exposes_resource_limit(
        &self,
        descriptor: &crate::resource_contract::BindingResourceLimitDescriptor,
    ) -> bool {
        match crate::resource_contract::resource_limit_owner(descriptor.stable_id) {
            crate::resource_contract::BindingResourceOwner::Artifact => true,
            crate::resource_contract::BindingResourceOwner::Capability(capability_id) => {
                CapabilityKey::from_id(capability_id)
                    .is_some_and(|capability| self.capabilities.contains(capability))
            }
            crate::resource_contract::BindingResourceOwner::Outputs(output_ids) => {
                output_ids.iter().any(|output_id| {
                    OutputKey::from_id(output_id)
                        .is_some_and(|output| self.outputs.contains(output))
                })
            }
        }
    }

    pub(crate) fn validate_native_runtime_policy(&self) -> Result<(), BindingError> {
        for capability in [
            CapabilityKey::SystemClock,
            CapabilityKey::SystemTimezone,
            CapabilityKey::SystemRandom,
        ] {
            if !self.selected_system_adapters.contains(capability) {
                return Err(BindingError::missing_capability(
                    capability.id(),
                    format!(
                        "runtime capability `{}` is not compiled into this artifact",
                        capability.id()
                    ),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_engine_services(
        &self,
        services: &BindingEngineServices,
    ) -> Result<(), BindingError> {
        for service in services.service_keys() {
            if !self.constructor_services.contains(service) {
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

const DEFAULT_CONSTRUCTOR_SERVICES: &[ConstructorServiceKey] = &[
    #[cfg(feature = "svg")]
    ConstructorServiceKey::HostTextMeasurement,
    #[cfg(feature = "svg")]
    ConstructorServiceKey::IconRegistry,
];
const DEFAULT_SYSTEM_ADAPTERS: &[CapabilityKey] = &[
    #[cfg(feature = "system-clock")]
    CapabilityKey::SystemClock,
    #[cfg(feature = "system-random")]
    CapabilityKey::SystemRandom,
    #[cfg(feature = "system-timezone")]
    CapabilityKey::SystemTimezone,
];
const DEFAULT_OPERATIONS: &[OperationKey] = &[
    #[cfg(feature = "analysis")]
    OperationKey::AnalysisFactsJson,
    #[cfg(feature = "analysis")]
    OperationKey::AnalysisJson,
    #[cfg(feature = "ascii")]
    OperationKey::Ascii,
    #[cfg(feature = "analysis")]
    OperationKey::DocumentAnalysisFactsJson,
    #[cfg(feature = "analysis")]
    OperationKey::DocumentAnalysisJson,
    #[cfg(feature = "jpeg")]
    OperationKey::Jpeg,
    #[cfg(feature = "svg")]
    OperationKey::LayoutJson,
    #[cfg(feature = "pdf")]
    OperationKey::Pdf,
    #[cfg(feature = "png")]
    OperationKey::Png,
    OperationKey::SemanticJson,
    #[cfg(feature = "svg")]
    OperationKey::Svg,
    #[cfg(feature = "svg")]
    OperationKey::SvgPlanJson,
    #[cfg(feature = "analysis")]
    OperationKey::ValidationJson,
];
const DEFAULT_SUPPLEMENTAL_CAPABILITIES: &[CapabilityKey] = &[
    #[cfg(feature = "layout-cytoscape")]
    CapabilityKey::LayoutCytoscape,
    #[cfg(feature = "layout-elk")]
    CapabilityKey::LayoutElk,
    #[cfg(feature = "math")]
    CapabilityKey::Math,
];
pub(crate) const DEFAULT_RUNTIME_POLICY: RuntimePolicyExposure =
    RuntimePolicyExposure::BindingOptions;

pub(crate) const DEFAULT_ARTIFACT_SNAPSHOT: ValidatedArtifactContract =
    ArtifactContractSpec::new(TargetKey::Native, BindingTransportKey::Rust)
        .with_operations(DEFAULT_OPERATIONS)
        .with_supplemental_capabilities(DEFAULT_SUPPLEMENTAL_CAPABILITIES)
        .with_all_available_metadata()
        .with_constructor_services(DEFAULT_CONSTRUCTOR_SERVICES)
        .with_system_adapters(DEFAULT_SYSTEM_ADAPTERS)
        .with_runtime_policy_exposure(DEFAULT_RUNTIME_POLICY)
        .materialize();

pub(crate) fn default_artifact_contract() -> Arc<ValidatedArtifactContract> {
    Arc::clone(DEFAULT_ARTIFACT_CONTRACT.get_or_init(|| Arc::new(DEFAULT_ARTIFACT_SNAPSHOT)))
}

#[cfg(test)]
#[path = "artifact_contract/tests.rs"]
mod tests;
