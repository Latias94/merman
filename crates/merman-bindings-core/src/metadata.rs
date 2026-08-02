use crate::common::{BindingError, internal_json_error};
use crate::operation::compiled_operation_kind_ids;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[allow(dead_code)]
mod capability_descriptor {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../capabilities/generated/capability_surface.rs"
    ));
}

/// First public schema for the artifact-owned runtime catalog.
pub const RUNTIME_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const TEXT_MEASUREMENT_PROVIDER_HOST_CALLBACK: &str = "host-callback";
pub const TEXT_MEASUREMENT_PROVIDER_VENDORED: &str = "vendored";
pub const BINDING_METADATA_IDS: [&str; 6] = [
    "supported-diagrams",
    "ascii-capabilities",
    "diagram-family-capabilities",
    "lint-rule-catalog",
    "supported-themes",
    "supported-host-theme-presets",
];

#[cfg(feature = "svg")]
const TEXT_MEASUREMENT_PROVIDER_IDS: &[&str] = &[
    TEXT_MEASUREMENT_PROVIDER_HOST_CALLBACK,
    TEXT_MEASUREMENT_PROVIDER_VENDORED,
];

static SUPPORTED_DIAGRAMS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static ASCII_SUPPORTED_DIAGRAMS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static ASCII_CAPABILITIES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static SUPPORTED_THEMES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static SUPPORTED_HOST_THEME_PRESETS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static DIAGRAM_FAMILY_CAPABILITIES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static RUNTIME_CAPABILITIES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
#[cfg(feature = "analysis")]
static LINT_RULE_CATALOG_JSON: OnceLock<Vec<u8>> = OnceLock::new();
#[cfg(feature = "analysis")]
static CONFIGURABLE_LINT_RULE_CATALOG_JSON: OnceLock<Vec<u8>> = OnceLock::new();

/// The text-measurement routes exposed by one artifact.
///
/// The protocol version belongs to the independently versioned text-measurement contract. Provider
/// IDs describe actual installation routes, not Cargo features: `vendored` is the built-in
/// renderer measurer and `host-callback` means this transport accepts a host callback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct TextMeasurementCapabilities {
    pub protocol_version: u32,
    pub provider_ids: Vec<&'static str>,
}

impl TextMeasurementCapabilities {
    #[cfg(feature = "svg")]
    pub fn new(provider_ids: Vec<&'static str>) -> Result<Self, BindingError> {
        validate_sorted_unique(
            "text measurement provider IDs",
            &provider_ids,
            TEXT_MEASUREMENT_PROVIDER_IDS,
        )?;
        if !provider_ids.contains(&TEXT_MEASUREMENT_PROVIDER_VENDORED) {
            return Err(invalid_capability_surface(
                "text measurement support must include the built-in `vendored` provider",
            ));
        }
        Ok(Self {
            protocol_version: merman::svg::TEXT_MEASUREMENT_PROTOCOL_VERSION,
            provider_ids,
        })
    }
}

/// Exact public capability selection owned by one compiled artifact.
///
/// This is deliberately a closed, validated vocabulary rather than a collection of transport
/// booleans. Artifact owners construct it from their callable endpoints and must not project or
/// mutate another artifact's report after the fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactCapabilitySurface {
    capability_ids: Vec<&'static str>,
    output_ids: Vec<&'static str>,
    operation_ids: Vec<&'static str>,
    system_adapter_ids: Vec<&'static str>,
    text_measurement: Option<TextMeasurementCapabilities>,
}

/// Selects which compiled text-measurement providers an artifact exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMeasurementProviderProjection {
    /// Keep every provider exposed by the compiled shared binding facade.
    PreserveCompiled,
    /// Expose only the built-in vendored provider.
    VendoredOnly,
}

impl ArtifactCapabilitySurface {
    pub fn new(
        capability_ids: Vec<&'static str>,
        output_ids: Vec<&'static str>,
        system_adapter_ids: Vec<&'static str>,
        text_measurement: Option<TextMeasurementCapabilities>,
    ) -> Result<Self, BindingError> {
        let operation_ids = binding_operation_ids_for_capabilities(&capability_ids);
        Self::new_with_operation_ids(
            capability_ids,
            output_ids,
            operation_ids,
            system_adapter_ids,
            text_measurement,
        )
    }

    /// Constructs an artifact-owned surface with its exact callable operation set.
    ///
    /// [`Self::new`] derives the full operation set implied by the selected capabilities. Use
    /// this constructor when a transport intentionally exposes a smaller endpoint set.
    pub fn new_with_operation_ids(
        capability_ids: Vec<&'static str>,
        output_ids: Vec<&'static str>,
        operation_ids: Vec<&'static str>,
        system_adapter_ids: Vec<&'static str>,
        text_measurement: Option<TextMeasurementCapabilities>,
    ) -> Result<Self, BindingError> {
        validate_sorted_unique(
            "runtime capability IDs",
            &capability_ids,
            capability_descriptor::CAPABILITY_IDS,
        )?;
        validate_sorted_unique(
            "public output IDs",
            &output_ids,
            capability_descriptor::OUTPUT_IDS,
        )?;
        validate_sorted_unique(
            "public binding operation IDs",
            &operation_ids,
            capability_descriptor::BINDING_OPERATION_IDS,
        )?;
        validate_sorted_unique(
            "system adapter IDs",
            &system_adapter_ids,
            capability_descriptor::CAPABILITY_IDS,
        )?;

        for capability_id in &capability_ids {
            let capability = capability_descriptor::CAPABILITIES
                .iter()
                .find(|capability| capability.id == *capability_id)
                .expect("validated capability ID must have a descriptor");
            for implication in capability.implications {
                if !capability_ids.contains(implication) {
                    return Err(invalid_capability_surface(format!(
                        "capability `{capability_id}` requires `{implication}`"
                    )));
                }
            }
        }

        for output_id in &output_ids {
            let output = capability_descriptor::OUTPUTS
                .iter()
                .find(|output| output.id == *output_id)
                .expect("validated output ID must have a descriptor");
            if !capability_ids.contains(&output.capability) {
                return Err(invalid_capability_surface(format!(
                    "public output `{output_id}` requires capability `{}`",
                    output.capability
                )));
            }
            if !operation_ids.contains(output_id) {
                return Err(invalid_capability_surface(format!(
                    "public output `{output_id}` must also be a public binding operation"
                )));
            }
        }

        for operation_id in &operation_ids {
            let operation = capability_descriptor::BINDING_OPERATIONS
                .iter()
                .find(|operation| operation.id == *operation_id)
                .expect("validated binding operation ID must have a descriptor");
            if let Some(capability_id) = operation.capability_id
                && !capability_ids.contains(&capability_id)
            {
                return Err(invalid_capability_surface(format!(
                    "public binding operation `{operation_id}` requires capability `{capability_id}`"
                )));
            }
        }

        for adapter_id in &system_adapter_ids {
            let capability = capability_descriptor::CAPABILITIES
                .iter()
                .find(|capability| capability.id == *adapter_id)
                .expect("validated system adapter ID must have a descriptor");
            if capability.kind != "adapter" {
                return Err(invalid_capability_surface(format!(
                    "system adapter ID `{adapter_id}` is not an adapter capability"
                )));
            }
            if !capability_ids.contains(adapter_id) {
                return Err(invalid_capability_surface(format!(
                    "system adapter ID `{adapter_id}` is not present in runtime capabilities"
                )));
            }
        }

        let svg_available = capability_ids.contains(&"svg");
        if svg_available != text_measurement.is_some() {
            return Err(invalid_capability_surface(
                "text-measurement support must be present exactly when the SVG capability is public",
            ));
        }

        Ok(Self {
            capability_ids,
            output_ids,
            operation_ids,
            system_adapter_ids,
            text_measurement,
        })
    }

    #[must_use]
    pub fn runtime_capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            capability_ids: self.capability_ids.clone(),
            output_ids: self.output_ids.clone(),
            operation_ids: self.operation_ids.clone(),
            system_adapter_ids: self.system_adapter_ids.clone(),
            text_measurement: self.text_measurement.clone(),
        }
    }

    /// Projects this compiled surface onto one capability-descriptor target.
    ///
    /// This is the only valid way for a target-specific artifact to drop capabilities solely
    /// because the descriptor excludes that target. Additional endpoint differences must be
    /// expressed by constructing a separate checked surface rather than mutating a report.
    pub fn project_to_descriptor_target(
        &self,
        target_id: &str,
        text_measurement_projection: TextMeasurementProviderProjection,
    ) -> Result<Self, BindingError> {
        if !capability_descriptor::TARGET_IDS.contains(&target_id) {
            return Err(invalid_capability_surface(format!(
                "unknown capability-descriptor target `{target_id}`"
            )));
        }

        let capability_ids: Vec<_> = self
            .capability_ids
            .iter()
            .copied()
            .filter(|id| {
                capability_descriptor::CAPABILITIES
                    .iter()
                    .find(|capability| capability.id == *id)
                    .is_some_and(|capability| capability.targets.contains(&target_id))
            })
            .collect();
        let output_ids: Vec<_> = self
            .output_ids
            .iter()
            .copied()
            .filter(|id| {
                capability_descriptor::OUTPUTS
                    .iter()
                    .find(|output| output.id == *id)
                    .is_some_and(|output| output.targets.contains(&target_id))
            })
            .collect();
        let operation_ids: Vec<_> = self
            .operation_ids
            .iter()
            .copied()
            .filter(|id| {
                capability_descriptor::BINDING_OPERATIONS
                    .iter()
                    .find(|operation| operation.id == *id)
                    .is_some_and(|operation| operation.targets.contains(&target_id))
            })
            .collect();
        let system_adapter_ids: Vec<_> = self
            .system_adapter_ids
            .iter()
            .copied()
            .filter(|id| capability_ids.contains(id))
            .collect();

        #[cfg(feature = "svg")]
        let text_measurement = if capability_ids.contains(&"svg") {
            match text_measurement_projection {
                TextMeasurementProviderProjection::PreserveCompiled => {
                    self.text_measurement.clone()
                }
                TextMeasurementProviderProjection::VendoredOnly => {
                    Some(TextMeasurementCapabilities::new(vec![
                        TEXT_MEASUREMENT_PROVIDER_VENDORED,
                    ])?)
                }
            }
        } else {
            None
        };
        #[cfg(not(feature = "svg"))]
        let text_measurement = {
            let _ = text_measurement_projection;
            None
        };

        Self::new_with_operation_ids(
            capability_ids,
            output_ids,
            operation_ids,
            system_adapter_ids,
            text_measurement,
        )
    }

    fn project_to_binding_runtime_policy(self) -> Result<Self, BindingError> {
        let Self {
            mut capability_ids,
            output_ids,
            operation_ids,
            system_adapter_ids,
            text_measurement,
        } = self;
        let native_system_adapter_ids = merman::runtime::RuntimePolicy::NATIVE_SYSTEM_ADAPTER_IDS;
        let native_policy_available = native_system_adapter_ids
            .iter()
            .all(|id| system_adapter_ids.contains(id));
        let selected_system_adapter_ids = if native_policy_available {
            system_adapter_ids
                .iter()
                .copied()
                .filter(|id| native_system_adapter_ids.contains(id))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        capability_ids.retain(|id| {
            !system_adapter_ids.contains(id) || selected_system_adapter_ids.contains(id)
        });

        Self::new_with_operation_ids(
            capability_ids,
            output_ids,
            operation_ids,
            selected_system_adapter_ids,
            text_measurement,
        )
    }
}

/// Stable runtime capability report shared by every transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimeCapabilities {
    pub capability_ids: Vec<&'static str>,
    pub output_ids: Vec<&'static str>,
    /// Complete callable operation IDs, including invariant semantic operations.
    pub operation_ids: Vec<&'static str>,
    pub system_adapter_ids: Vec<&'static str>,
    pub text_measurement: Option<TextMeasurementCapabilities>,
}

impl RuntimeCapabilities {
    #[must_use]
    pub fn has_capability(&self, id: &str) -> bool {
        self.capability_ids.binary_search(&id).is_ok()
    }

    #[must_use]
    pub fn has_output(&self, id: &str) -> bool {
        self.output_ids.binary_search(&id).is_ok()
    }

    #[must_use]
    pub fn has_operation(&self, id: &str) -> bool {
        self.operation_ids.binary_search(&id).is_ok()
    }
}

fn binding_operation_ids_for_capabilities(capability_ids: &[&'static str]) -> Vec<&'static str> {
    let mut operation_ids = capability_descriptor::BINDING_OPERATIONS
        .iter()
        .filter(|operation| match operation.capability_id {
            Some(capability_id) => capability_ids.contains(&capability_id),
            None => true,
        })
        .map(|operation| operation.id)
        .collect::<Vec<_>>();
    operation_ids.sort_unstable();
    operation_ids
}

fn invalid_capability_surface(message: impl Into<String>) -> BindingError {
    BindingError::new(
        crate::BindingStatus::InvalidArgument,
        format!("invalid runtime capability surface: {}", message.into()),
    )
}

fn validate_sorted_unique(
    label: &str,
    values: &[&'static str],
    vocabulary: &[&str],
) -> Result<(), BindingError> {
    let mut previous = None;
    for value in values {
        if !vocabulary.contains(value) {
            return Err(invalid_capability_surface(format!(
                "{label} contains unknown ID `{value}`"
            )));
        }
        if let Some(previous) = previous
            && previous >= *value
        {
            let reason = if previous == *value {
                "duplicate"
            } else {
                "not sorted"
            };
            return Err(invalid_capability_surface(format!(
                "{label} must be sorted and unique; `{value}` is {reason}"
            )));
        }
        previous = Some(*value);
    }
    Ok(())
}

/// Current capabilities and policies exposed by one concrete transport artifact.
///
/// This catalog intentionally excludes the global capability vocabulary and the bodies of detailed
/// language catalogs. It does expose the independently versioned binding schema identifiers a
/// generic host needs before it sends options or decodes a result. Consumers validate this
/// artifact-owned fact set by shape, ordering, and local relations, and must tolerate newly added
/// stable IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimeCatalog {
    pub schema_version: u32,
    /// Version of the transport that produced this catalog.
    ///
    /// This is deliberately not the native C ABI version. Each transport owns its own wire/API
    /// boundary and supplies its own version when constructing a catalog.
    pub transport_api_version: u32,
    pub package_version: &'static str,
    /// Options JSON schemas accepted by this artifact.
    pub options_schema_versions: Vec<u32>,
    /// Binding-owned result and operation metadata payload schemas.
    pub payload_schemas: Vec<RuntimePayloadSchema>,
    /// Detailed metadata catalog IDs available through the transport's metadata dispatcher.
    pub metadata_ids: Vec<&'static str>,
    pub capabilities: RuntimeCapabilities,
    pub output_contracts: Vec<RuntimeOutputContract>,
    pub registry: RuntimeRegistryContract,
    pub resources: RuntimeResourceContract,
}

/// One independently versioned binding payload advertised by [`RuntimeCatalog`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimePayloadSchema {
    pub id: &'static str,
    pub version: u32,
}

/// Runtime behavior of one output exposed by the concrete artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimeOutputContract {
    pub id: &'static str,
    pub media_type: &'static str,
    pub system_fonts: Option<RuntimeSystemFontContract>,
    pub embedded_images: Option<RuntimeEmbeddedImageContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimeSystemFontContract {
    pub source_id: &'static str,
    pub discovery: &'static str,
    pub cache_scope: &'static str,
    pub host_dependent: bool,
    pub caller_configurable: bool,
    pub resource_bounded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimeEmbeddedImageContract {
    pub source_ids: &'static [&'static str],
    pub filesystem_access: bool,
    pub network_access: bool,
    pub caller_configurable: bool,
    pub limits: RuntimeEmbeddedImageLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimeEmbeddedImageLimits {
    pub max_bytes_per_image: Option<u64>,
    pub max_total_bytes: Option<u64>,
    pub max_pixels_per_image: Option<u64>,
    pub max_total_pixels: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimeRegistryContract {
    pub diagram_family_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimeResourceContract {
    pub general_binding_default_profile: &'static str,
    pub cli_default_profile: &'static str,
    pub limits: Vec<RuntimeResourceLimit>,
    pub profiles: Vec<RuntimeResourceProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimeResourceLimit {
    pub id: &'static str,
    pub phase: &'static str,
    pub description: &'static str,
    pub overridable: bool,
    pub hard_cap: bool,
    pub minimum_value: usize,
    /// Transport-callable operations that accept this limit in `resources.limits`.
    pub operation_ids: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimeResourceProfile {
    pub id: &'static str,
    pub purpose: &'static str,
    pub trust_assumption: &'static str,
    pub recommended_binding_default: bool,
    pub limits: BTreeMap<&'static str, Option<usize>>,
}

pub use merman::DiagramFamilyCapability as BindingDiagramFamilyCapability;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BindingAsciiCapability {
    pub diagram_type: &'static str,
    pub display_name: &'static str,
    pub support_level: &'static str,
    pub summary_fallback: bool,
    pub supported_semantics: &'static [&'static str],
    pub limits: &'static [&'static str],
    pub evidence: Vec<BindingAsciiCapabilityEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BindingAsciiCapabilityEvidence {
    pub kind: &'static str,
    pub source: &'static str,
    pub note: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuleCatalogEntry {
    pub id: &'static str,
    pub description: &'static str,
    pub evidence: &'static [&'static str],
    pub default_severity: &'static str,
    pub category: &'static str,
    pub default_enabled: bool,
    pub default_profile: &'static str,
    pub origin: &'static str,
    pub configurable: bool,
    pub fixable: bool,
}

/// Reports the exact capability surface compiled into the shared binding facade.
///
/// This is the Rust owner's complete compiled fact set, including adapters selectable only through
/// a custom [`merman::runtime::RuntimePolicy`]. Foreign transports must use
/// [`binding_transport_capability_surface`] or construct their own checked
/// [`ArtifactCapabilitySurface`] when they expose a smaller callable endpoint set.
pub fn compiled_runtime_capability_surface() -> ArtifactCapabilitySurface {
    let mut capability_ids = Vec::new();
    let mut system_adapter_ids = merman::runtime::compiled_system_adapter_ids().to_vec();

    #[cfg(feature = "svg")]
    {
        capability_ids.push("svg");
        if compiled_layout_cytoscape_available() {
            capability_ids.push("layout-cytoscape");
        }
        if compiled_layout_elk_available() {
            capability_ids.push("layout-elk");
        }
    }
    #[cfg(feature = "analysis")]
    capability_ids.push("analysis");
    #[cfg(feature = "ascii")]
    capability_ids.push("ascii");
    #[cfg(feature = "png")]
    capability_ids.push("png");
    #[cfg(feature = "jpeg")]
    capability_ids.push("jpeg");
    #[cfg(feature = "pdf")]
    capability_ids.push("pdf");
    #[cfg(feature = "svg")]
    if compiled_math_available() {
        capability_ids.push("math");
    }
    capability_ids.extend(system_adapter_ids.iter().copied());
    capability_ids.sort_unstable();
    system_adapter_ids.sort_unstable();

    let mut operation_ids = compiled_operation_kind_ids();
    operation_ids.sort_unstable();

    let mut output_ids = operation_ids
        .iter()
        .copied()
        .filter(|id| capability_descriptor::OUTPUT_IDS.contains(id))
        .collect::<Vec<_>>();
    output_ids.sort_unstable();

    #[cfg(feature = "svg")]
    let text_measurement = Some(
        TextMeasurementCapabilities::new(vec![
            TEXT_MEASUREMENT_PROVIDER_HOST_CALLBACK,
            TEXT_MEASUREMENT_PROVIDER_VENDORED,
        ])
        .expect("the built-in binding surface has a valid text-measurement contract"),
    );
    #[cfg(not(feature = "svg"))]
    let text_measurement = None;

    ArtifactCapabilitySurface::new_with_operation_ids(
        capability_ids,
        output_ids,
        operation_ids,
        system_adapter_ids,
        text_measurement,
    )
    .expect("the compiled binding surface uses descriptor-owned capability IDs")
}

/// Reports the exact capability surface selectable through the shared binding JSON policy.
///
/// Cargo feature unification can compile additional system adapters for Rust callers that supply
/// a custom [`merman::runtime::RuntimePolicy`]. Foreign transports using [`crate::BindingEngine`]
/// JSON options can select only `deterministic` or `native`. The native policy requires clock,
/// time-zone, and randomness as one atomic set, while timing instrumentation is not selectable
/// through binding JSON. The transport catalog therefore exposes all three native adapters or none.
pub fn binding_transport_capability_surface() -> ArtifactCapabilitySurface {
    compiled_runtime_capability_surface()
        .project_to_binding_runtime_policy()
        .expect("the binding runtime policy uses descriptor-owned system adapter IDs")
}

#[must_use]
pub fn compiled_runtime_capabilities() -> RuntimeCapabilities {
    compiled_runtime_capability_surface().runtime_capabilities()
}

#[cfg(feature = "svg")]
const fn compiled_layout_cytoscape_available() -> bool {
    merman::svg::layout_cytoscape_available()
}

#[cfg(feature = "svg")]
const fn compiled_layout_elk_available() -> bool {
    merman::svg::layout_elk_available()
}

#[cfg(feature = "svg")]
const fn compiled_math_available() -> bool {
    merman::svg::math_available()
}

#[must_use]
pub fn runtime_catalog(transport_api_version: u32) -> RuntimeCatalog {
    runtime_catalog_for(
        transport_api_version,
        binding_transport_capability_surface(),
    )
}

#[must_use]
pub fn runtime_catalog_for(
    transport_api_version: u32,
    capability_surface: ArtifactCapabilitySurface,
) -> RuntimeCatalog {
    let capabilities = capability_surface.runtime_capabilities();
    let output_contracts = runtime_output_contracts_for(&capabilities);
    let mut metadata_ids = BINDING_METADATA_IDS.to_vec();
    metadata_ids.sort_unstable();
    RuntimeCatalog {
        schema_version: RUNTIME_CATALOG_SCHEMA_VERSION,
        transport_api_version,
        package_version: env!("CARGO_PKG_VERSION"),
        options_schema_versions: vec![crate::BINDING_OPTIONS_SCHEMA_VERSION],
        payload_schemas: vec![
            RuntimePayloadSchema {
                id: "binding-result",
                version: crate::BINDING_RESULT_PAYLOAD_VERSION,
            },
            RuntimePayloadSchema {
                id: "operation-metadata",
                version: crate::BINDING_OPERATION_SCHEMA_VERSION,
            },
        ],
        metadata_ids,
        output_contracts,
        registry: RuntimeRegistryContract {
            diagram_family_count: diagram_family_capabilities().len(),
        },
        resources: runtime_resource_contract_for(&capabilities),
        capabilities,
    }
}

fn runtime_output_contracts_for(capabilities: &RuntimeCapabilities) -> Vec<RuntimeOutputContract> {
    capability_descriptor::OUTPUTS
        .iter()
        .filter(|output| capabilities.has_output(output.id))
        .map(runtime_output_contract)
        .collect()
}

fn runtime_output_contract(
    output: &capability_descriptor::OutputDescriptor,
) -> RuntimeOutputContract {
    match output.id {
        "ascii" | "svg" => RuntimeOutputContract {
            id: output.id,
            media_type: output.media_type,
            system_fonts: None,
            embedded_images: None,
        },
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        "jpeg" | "pdf" | "png" => {
            let environment = merman::svg::export::output_environment_contract(output.id)
                .expect("a selected native export output must have an environment contract");
            let limits = environment.embedded_images.default_limits;
            RuntimeOutputContract {
                id: output.id,
                media_type: output.media_type,
                system_fonts: environment
                    .system_fonts
                    .map(|fonts| RuntimeSystemFontContract {
                        source_id: fonts.source_id,
                        discovery: fonts.discovery,
                        cache_scope: fonts.cache_scope,
                        host_dependent: fonts.host_dependent,
                        caller_configurable: false,
                        resource_bounded: fonts.resource_bounded,
                    }),
                embedded_images: Some(RuntimeEmbeddedImageContract {
                    source_ids: environment.embedded_images.source_ids,
                    filesystem_access: environment.embedded_images.filesystem_access,
                    network_access: environment.embedded_images.network_access,
                    caller_configurable: true,
                    limits: RuntimeEmbeddedImageLimits {
                        max_bytes_per_image: limits.max_bytes_per_image,
                        max_total_bytes: limits.max_total_bytes,
                        max_pixels_per_image: limits.max_pixels_per_image,
                        max_total_pixels: limits.max_total_pixels,
                    },
                }),
            }
        }
        _ => panic!(
            "descriptor output `{}` has no runtime output-contract owner",
            output.id
        ),
    }
}

pub fn runtime_catalog_json(transport_api_version: u32) -> Result<Vec<u8>, BindingError> {
    runtime_catalog_json_for(
        transport_api_version,
        binding_transport_capability_surface(),
    )
}

pub fn runtime_catalog_json_for(
    transport_api_version: u32,
    capability_surface: ArtifactCapabilitySurface,
) -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(&runtime_catalog_for(
        transport_api_version,
        capability_surface,
    ))
    .map_err(internal_json_error)
}

fn runtime_resource_contract_for(capabilities: &RuntimeCapabilities) -> RuntimeResourceContract {
    let contract = crate::binding_resource_contract();
    let limits = contract
        .limits
        .into_iter()
        .filter(|descriptor| {
            match crate::resource_contract::resource_limit_owner(descriptor.stable_id) {
                crate::resource_contract::BindingResourceOwner::Artifact => true,
                crate::resource_contract::BindingResourceOwner::Capability(capability) => {
                    capabilities.has_capability(capability)
                }
                crate::resource_contract::BindingResourceOwner::Outputs(outputs) => {
                    outputs.iter().any(|output| capabilities.has_output(output))
                }
            }
        })
        .collect::<Vec<_>>();
    RuntimeResourceContract {
        general_binding_default_profile: contract.general_binding_default_profile,
        cli_default_profile: contract.cli_default_profile,
        limits: limits
            .iter()
            .map(|descriptor| RuntimeResourceLimit {
                id: descriptor.stable_id,
                phase: descriptor.phase,
                description: descriptor.description,
                overridable: descriptor.overridable,
                hard_cap: descriptor.hard_cap,
                minimum_value: descriptor.minimum_value,
                operation_ids: capabilities
                    .operation_ids
                    .iter()
                    .copied()
                    .filter(|operation_id| {
                        crate::BindingOperationKind::from_id(operation_id).is_ok_and(|operation| {
                            operation.resource_scope().accepts(descriptor.stable_id)
                        })
                    })
                    .collect(),
            })
            .collect(),
        profiles: contract
            .profiles
            .into_iter()
            .map(|profile| RuntimeResourceProfile {
                id: profile.id,
                purpose: profile.purpose,
                trust_assumption: profile.trust_assumption,
                recommended_binding_default: profile.recommended_binding_default,
                limits: limits
                    .iter()
                    .map(|limit| (limit.stable_id, profile.limits[limit.stable_id]))
                    .collect(),
            })
            .collect(),
    }
}

pub fn diagram_family_capabilities() -> Vec<BindingDiagramFamilyCapability> {
    merman::diagram_family_capabilities().to_vec()
}

pub fn runtime_capabilities_json() -> Result<Vec<u8>, BindingError> {
    if let Some(bytes) = RUNTIME_CAPABILITIES_JSON.get() {
        return Ok(bytes.clone());
    }

    let bytes = runtime_capabilities_json_for(&compiled_runtime_capabilities())?;
    let _ = RUNTIME_CAPABILITIES_JSON.set(bytes.clone());
    Ok(bytes)
}

pub fn runtime_capabilities_json_for(
    capabilities: &RuntimeCapabilities,
) -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(capabilities).map_err(internal_json_error)
}

pub fn supported_themes() -> &'static [&'static str] {
    merman::supported_themes()
}

pub fn supported_host_theme_presets() -> &'static [&'static str] {
    #[cfg(feature = "svg")]
    {
        merman::supported_host_theme_presets()
    }
    #[cfg(not(feature = "svg"))]
    {
        &[]
    }
}

pub fn supported_diagrams() -> &'static [&'static str] {
    merman::supported_diagrams()
}

pub fn ascii_supported_diagrams() -> &'static [&'static str] {
    #[cfg(feature = "ascii")]
    {
        merman::ascii::ascii_supported_diagram_types()
    }
    #[cfg(not(feature = "ascii"))]
    {
        &[]
    }
}

pub fn ascii_capabilities() -> Vec<BindingAsciiCapability> {
    #[cfg(feature = "ascii")]
    {
        merman::ascii::ascii_capabilities()
            .iter()
            .map(|capability| BindingAsciiCapability {
                diagram_type: capability.diagram_type,
                display_name: capability.display_name,
                support_level: capability.support_level.as_str(),
                summary_fallback: capability.summary_fallback,
                supported_semantics: capability.supported_semantics,
                limits: capability.limits,
                evidence: capability
                    .evidence
                    .iter()
                    .map(|evidence| BindingAsciiCapabilityEvidence {
                        kind: evidence.kind.as_str(),
                        source: evidence.source,
                        note: evidence.note,
                    })
                    .collect(),
            })
            .collect()
    }
    #[cfg(not(feature = "ascii"))]
    {
        Vec::new()
    }
}

pub fn supported_diagrams_json() -> Result<Vec<u8>, BindingError> {
    cached_json(&SUPPORTED_DIAGRAMS_JSON, supported_diagrams)
}

pub fn ascii_supported_diagrams_json() -> Result<Vec<u8>, BindingError> {
    cached_json(&ASCII_SUPPORTED_DIAGRAMS_JSON, ascii_supported_diagrams)
}

pub fn ascii_capabilities_json() -> Result<Vec<u8>, BindingError> {
    if let Some(bytes) = ASCII_CAPABILITIES_JSON.get() {
        return Ok(bytes.clone());
    }

    let bytes = serde_json::to_vec(&ascii_capabilities()).map_err(internal_json_error)?;
    let _ = ASCII_CAPABILITIES_JSON.set(bytes.clone());
    Ok(bytes)
}

pub fn supported_themes_json() -> Result<Vec<u8>, BindingError> {
    cached_json(&SUPPORTED_THEMES_JSON, supported_themes)
}

pub fn supported_host_theme_presets_json() -> Result<Vec<u8>, BindingError> {
    cached_json(
        &SUPPORTED_HOST_THEME_PRESETS_JSON,
        supported_host_theme_presets,
    )
}

pub fn lint_rule_catalog() -> Result<Vec<RuleCatalogEntry>, BindingError> {
    #[cfg(feature = "analysis")]
    {
        Ok(merman_analysis::rule_catalog()
            .into_iter()
            .map(rule_catalog_entry)
            .collect())
    }
    #[cfg(not(feature = "analysis"))]
    {
        Err(crate::common::feature_required_error(
            "lint rule catalog",
            "analysis",
        ))
    }
}

pub fn configurable_lint_rule_catalog() -> Result<Vec<RuleCatalogEntry>, BindingError> {
    #[cfg(feature = "analysis")]
    {
        Ok(merman_analysis::configurable_rule_catalog()
            .into_iter()
            .map(rule_catalog_entry)
            .collect())
    }
    #[cfg(not(feature = "analysis"))]
    {
        Err(crate::common::feature_required_error(
            "configurable lint rule catalog",
            "analysis",
        ))
    }
}

pub fn lint_rule_catalog_json() -> Result<Vec<u8>, BindingError> {
    #[cfg(not(feature = "analysis"))]
    {
        Err(crate::common::feature_required_error(
            "lint rule catalog",
            "analysis",
        ))
    }

    #[cfg(feature = "analysis")]
    {
        if let Some(bytes) = LINT_RULE_CATALOG_JSON.get() {
            return Ok(bytes.clone());
        }

        let bytes =
            merman_analysis::rule_catalog_response_json_bytes().map_err(internal_json_error)?;
        let _ = LINT_RULE_CATALOG_JSON.set(bytes.clone());
        Ok(bytes)
    }
}

pub fn configurable_lint_rule_catalog_json() -> Result<Vec<u8>, BindingError> {
    #[cfg(not(feature = "analysis"))]
    {
        Err(crate::common::feature_required_error(
            "configurable lint rule catalog",
            "analysis",
        ))
    }

    #[cfg(feature = "analysis")]
    {
        if let Some(bytes) = CONFIGURABLE_LINT_RULE_CATALOG_JSON.get() {
            return Ok(bytes.clone());
        }

        let bytes = merman_analysis::configurable_rule_catalog_response_json_bytes()
            .map_err(internal_json_error)?;
        let _ = CONFIGURABLE_LINT_RULE_CATALOG_JSON.set(bytes.clone());
        Ok(bytes)
    }
}

pub fn diagram_family_capabilities_json() -> Result<Vec<u8>, BindingError> {
    if let Some(bytes) = DIAGRAM_FAMILY_CAPABILITIES_JSON.get() {
        return Ok(bytes.clone());
    }

    let bytes = serde_json::to_vec(&diagram_family_capabilities()).map_err(internal_json_error)?;
    let _ = DIAGRAM_FAMILY_CAPABILITIES_JSON.set(bytes.clone());
    Ok(bytes)
}

/// Returns one transport-neutral binding metadata catalog by its stable identifier.
pub fn binding_metadata_json(id: &str) -> Result<Vec<u8>, BindingError> {
    match id {
        "supported-diagrams" => supported_diagrams_json(),
        "ascii-capabilities" => ascii_capabilities_json(),
        "diagram-family-capabilities" => diagram_family_capabilities_json(),
        "lint-rule-catalog" => lint_rule_catalog_json(),
        "supported-themes" => supported_themes_json(),
        "supported-host-theme-presets" => supported_host_theme_presets_json(),
        _ => Err(BindingError::new(
            crate::BindingStatus::InvalidArgument,
            format!("unknown binding metadata catalog `{id}`"),
        )),
    }
}

fn cached_json(
    cache: &OnceLock<Vec<u8>>,
    values: fn() -> &'static [&'static str],
) -> Result<Vec<u8>, BindingError> {
    if let Some(bytes) = cache.get() {
        return Ok(bytes.clone());
    }

    let bytes = serde_json::to_vec(values()).map_err(internal_json_error)?;
    let _ = cache.set(bytes.clone());
    Ok(bytes)
}

#[cfg(feature = "analysis")]
fn rule_catalog_entry(rule: merman_analysis::RuleCatalogEntry) -> RuleCatalogEntry {
    RuleCatalogEntry {
        id: rule.id,
        description: rule.description,
        evidence: rule.evidence,
        default_severity: rule.default_severity.as_str(),
        category: rule.category.as_str(),
        default_enabled: rule.default_enabled,
        default_profile: rule.default_profile.as_str(),
        origin: rule.origin.as_str(),
        configurable: rule.configurable,
        fixable: rule.fixable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BindingStatus;
    use serde_json::Value;

    #[test]
    fn supported_themes_exposes_core_theme_surface() {
        assert_eq!(
            supported_themes(),
            &[
                "default",
                "base",
                "dark",
                "forest",
                "neutral",
                "neo",
                "neo-dark",
                "redux",
                "redux-dark",
                "redux-color",
                "redux-dark-color"
            ]
        );
    }

    #[test]
    fn runtime_capabilities_follow_callable_feature_surface() {
        let capabilities = compiled_runtime_capabilities();

        assert_eq!(capabilities.has_capability("svg"), cfg!(feature = "svg"));
        assert_eq!(
            capabilities.has_capability("analysis"),
            cfg!(feature = "analysis")
        );
        assert_eq!(
            capabilities.has_capability("ascii"),
            cfg!(feature = "ascii")
        );
        let mut expected_system_adapter_ids =
            merman::runtime::compiled_system_adapter_ids().to_vec();
        expected_system_adapter_ids.sort_unstable();
        assert_eq!(capabilities.system_adapter_ids, expected_system_adapter_ids);
        #[cfg(feature = "svg")]
        let (expected_layout_cytoscape, expected_layout_elk, expected_math) = (
            compiled_layout_cytoscape_available(),
            compiled_layout_elk_available(),
            compiled_math_available(),
        );
        #[cfg(not(feature = "svg"))]
        let (expected_layout_cytoscape, expected_layout_elk, expected_math) = (false, false, false);
        assert_eq!(
            capabilities.has_capability("layout-cytoscape"),
            expected_layout_cytoscape
        );
        assert_eq!(
            capabilities.has_capability("layout-elk"),
            expected_layout_elk
        );
        assert_eq!(capabilities.has_capability("math"), expected_math);
        assert!(!capabilities.has_capability("editor"));
        assert_eq!(capabilities.has_output("svg"), cfg!(feature = "svg"));
        assert_eq!(capabilities.has_output("png"), cfg!(feature = "png"));
        assert_eq!(capabilities.has_output("jpeg"), cfg!(feature = "jpeg"));
        assert_eq!(capabilities.has_output("pdf"), cfg!(feature = "pdf"));
        assert_eq!(capabilities.has_output("ascii"), cfg!(feature = "ascii"));
        assert!(capabilities.has_operation("semantic-json"));
        assert_eq!(
            capabilities.has_operation("layout-json"),
            cfg!(feature = "svg")
        );
        assert_eq!(
            capabilities.has_operation("validation-json"),
            cfg!(feature = "analysis")
        );
        assert_eq!(
            capabilities.has_operation("document-analysis-json"),
            cfg!(feature = "analysis")
        );
        let mut compiled_operation_ids = compiled_operation_kind_ids();
        compiled_operation_ids.sort_unstable();
        assert_eq!(capabilities.operation_ids, compiled_operation_ids);
        assert!(!capabilities.has_capability("semantic"));

        #[cfg(feature = "svg")]
        {
            let text_measurement = capabilities
                .text_measurement
                .expect("SVG artifacts have text measurement support");
            assert_eq!(
                text_measurement.protocol_version,
                merman::svg::TEXT_MEASUREMENT_PROTOCOL_VERSION
            );
            assert_eq!(
                text_measurement.provider_ids,
                [
                    TEXT_MEASUREMENT_PROVIDER_HOST_CALLBACK,
                    TEXT_MEASUREMENT_PROVIDER_VENDORED,
                ]
            );
        }
        #[cfg(not(feature = "svg"))]
        assert!(capabilities.text_measurement.is_none());
    }

    #[test]
    fn runtime_capabilities_json_reports_stable_id_sets() {
        let capabilities: Value =
            serde_json::from_slice(&runtime_capabilities_json().unwrap()).unwrap();

        assert!(capabilities.get("render").is_none());
        assert!(capabilities.get("ratex_math").is_none());
        assert!(capabilities["capability_ids"].is_array());
        assert!(capabilities["output_ids"].is_array());
        assert!(capabilities["operation_ids"].is_array());
        assert!(capabilities["system_adapter_ids"].is_array());
        assert_eq!(
            capabilities,
            serde_json::to_value(compiled_runtime_capabilities()).unwrap()
        );
    }

    #[test]
    fn binding_transport_surface_matches_the_atomic_native_policy() {
        let compiled = compiled_runtime_capabilities();
        let transport = binding_transport_capability_surface().runtime_capabilities();
        let native_system_adapter_ids = merman::runtime::RuntimePolicy::NATIVE_SYSTEM_ADAPTER_IDS;
        let native_policy_available = native_system_adapter_ids
            .iter()
            .all(|id| compiled.system_adapter_ids.contains(id));
        let expected_system_adapter_ids = compiled
            .system_adapter_ids
            .iter()
            .copied()
            .filter(|id| native_policy_available && native_system_adapter_ids.contains(id))
            .collect::<Vec<_>>();
        let expected_capability_ids = compiled
            .capability_ids
            .iter()
            .copied()
            .filter(|id| {
                !compiled.system_adapter_ids.contains(id)
                    || expected_system_adapter_ids.contains(id)
            })
            .collect::<Vec<_>>();

        assert_eq!(transport.capability_ids, expected_capability_ids);
        assert_eq!(transport.output_ids, compiled.output_ids);
        assert_eq!(transport.operation_ids, compiled.operation_ids);
        assert_eq!(transport.text_measurement, compiled.text_measurement);
        assert_eq!(transport.system_adapter_ids, expected_system_adapter_ids);
        for adapter_id in &compiled.system_adapter_ids {
            assert_eq!(
                transport.has_capability(adapter_id),
                expected_system_adapter_ids.contains(adapter_id),
                "binding catalog must report adapter `{adapter_id}` exactly when JSON policy can select it"
            );
        }
        assert!(!transport.has_capability("system-timing"));
        assert!(!transport.system_adapter_ids.contains(&"system-timing"));
    }

    fn system_adapter_surface(system_adapter_ids: Vec<&'static str>) -> ArtifactCapabilitySurface {
        ArtifactCapabilitySurface::new(system_adapter_ids.clone(), vec![], system_adapter_ids, None)
            .expect("test system-adapter surface must be valid")
    }

    #[test]
    fn binding_transport_surface_exposes_the_complete_native_adapter_set() {
        let transport =
            system_adapter_surface(vec!["system-clock", "system-random", "system-timezone"])
                .project_to_binding_runtime_policy()
                .expect("the complete native adapter set must project")
                .runtime_capabilities();

        assert_eq!(
            transport.system_adapter_ids,
            ["system-clock", "system-random", "system-timezone"]
        );
        assert_eq!(
            transport.capability_ids,
            ["system-clock", "system-random", "system-timezone"]
        );
    }

    #[test]
    fn binding_transport_surface_hides_every_partial_native_adapter_set() {
        for partial_adapter_ids in [
            vec!["system-clock", "system-random"],
            vec!["system-clock", "system-timezone"],
            vec!["system-random", "system-timezone"],
        ] {
            let transport = system_adapter_surface(partial_adapter_ids)
                .project_to_binding_runtime_policy()
                .expect("a partial native adapter set must project to no selectable policy")
                .runtime_capabilities();

            assert!(transport.system_adapter_ids.is_empty());
            assert!(transport.capability_ids.is_empty());
        }
    }

    #[test]
    fn binding_transport_surface_hides_externally_unified_timing() {
        let transport = system_adapter_surface(vec![
            "system-clock",
            "system-random",
            "system-timezone",
            "system-timing",
        ])
        .project_to_binding_runtime_policy()
        .expect("externally unified timing must not invalidate transport projection")
        .runtime_capabilities();

        assert_eq!(
            transport.system_adapter_ids,
            ["system-clock", "system-random", "system-timezone"]
        );
        assert_eq!(
            transport.capability_ids,
            ["system-clock", "system-random", "system-timezone"]
        );
        assert!(!transport.has_capability("system-timing"));
    }

    #[test]
    fn capability_surface_rejects_unknown_duplicate_and_incoherent_ids() {
        let unknown =
            ArtifactCapabilitySurface::new(vec!["not-a-capability"], vec![], vec![], None)
                .expect_err("unknown capability must fail closed");
        assert_eq!(unknown.status(), BindingStatus::InvalidArgument);

        let duplicate =
            ArtifactCapabilitySurface::new(vec!["analysis", "analysis"], vec![], vec![], None)
                .expect_err("duplicate capability must fail closed");
        assert_eq!(duplicate.status(), BindingStatus::InvalidArgument);

        let output_without_capability =
            ArtifactCapabilitySurface::new(vec![], vec!["svg"], vec![], None)
                .expect_err("output without capability must fail closed");
        assert_eq!(
            output_without_capability.status(),
            BindingStatus::InvalidArgument
        );

        let engine_without_svg =
            ArtifactCapabilitySurface::new(vec!["layout-elk"], vec![], vec![], None)
                .expect_err("engine implication without SVG must fail closed");
        assert_eq!(engine_without_svg.status(), BindingStatus::InvalidArgument);

        let non_adapter =
            ArtifactCapabilitySurface::new(vec!["analysis"], vec![], vec!["analysis"], None)
                .expect_err("non-adapter system ID must fail closed");
        assert_eq!(non_adapter.status(), BindingStatus::InvalidArgument);

        let duplicate_operation = ArtifactCapabilitySurface::new_with_operation_ids(
            vec![],
            vec![],
            vec!["semantic-json", "semantic-json"],
            vec![],
            None,
        )
        .expect_err("duplicate operation must fail closed");
        assert_eq!(duplicate_operation.status(), BindingStatus::InvalidArgument);

        let operation_without_capability = ArtifactCapabilitySurface::new_with_operation_ids(
            vec![],
            vec![],
            vec!["validation-json"],
            vec![],
            None,
        )
        .expect_err("capability-gated operation must fail closed");
        assert_eq!(
            operation_without_capability.status(),
            BindingStatus::InvalidArgument
        );
    }

    #[test]
    fn runtime_catalog_is_versioned_and_projects_the_resource_descriptor() {
        let catalog = runtime_catalog(2);
        assert_eq!(catalog.schema_version, RUNTIME_CATALOG_SCHEMA_VERSION);
        assert_eq!(catalog.transport_api_version, 2);
        assert_eq!(catalog.package_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            catalog.capabilities,
            binding_transport_capability_surface().runtime_capabilities()
        );
        assert_eq!(
            catalog
                .output_contracts
                .iter()
                .map(|output| output.id)
                .collect::<Vec<_>>(),
            catalog.capabilities.output_ids
        );
        for output in &catalog.output_contracts {
            match output.id {
                "ascii" | "svg" => {
                    assert!(output.system_fonts.is_none());
                    assert!(output.embedded_images.is_none());
                }
                "jpeg" | "pdf" | "png" => {
                    let fonts = output.system_fonts.as_ref().expect("system font contract");
                    assert_eq!(fonts.source_id, "host-system");
                    assert_eq!(fonts.discovery, "first-use");
                    assert_eq!(fonts.cache_scope, "process-global");
                    assert!(fonts.host_dependent);
                    assert!(!fonts.caller_configurable);
                    assert!(!fonts.resource_bounded);
                    let images = output
                        .embedded_images
                        .as_ref()
                        .expect("embedded-image contract");
                    assert_eq!(images.source_ids, ["data-url"]);
                    assert!(!images.filesystem_access);
                    assert!(!images.network_access);
                    assert!(images.caller_configurable);
                    assert_eq!(images.limits.max_bytes_per_image, Some(16 * 1024 * 1024));
                    assert_eq!(images.limits.max_total_bytes, Some(32 * 1024 * 1024));
                    assert_eq!(images.limits.max_pixels_per_image, Some(16 * 1024 * 1024));
                    assert_eq!(images.limits.max_total_pixels, Some(32 * 1024 * 1024));
                }
                id => panic!("runtime output contract test does not own `{id}`"),
            }
        }
        assert_eq!(
            catalog.registry.diagram_family_count,
            diagram_family_capabilities().len()
        );

        let resources = &catalog.resources;
        let expected_limit_count = crate::binding_resource_contract().limits.len();
        assert_eq!(resources.profiles.len(), 4);
        assert_eq!(resources.limits.len(), expected_limit_count);
        assert!(
            resources
                .profiles
                .iter()
                .all(|profile| profile.limits.len() == expected_limit_count)
        );
        assert_eq!(resources.general_binding_default_profile, "interactive");
        assert_eq!(resources.cli_default_profile, "trusted-native");
        assert_eq!(
            resources
                .profiles
                .iter()
                .filter(|profile| profile.recommended_binding_default)
                .map(|profile| profile.id)
                .collect::<Vec<_>>(),
            vec![resources.general_binding_default_profile]
        );
        assert_eq!(
            catalog.options_schema_versions,
            vec![crate::BINDING_OPTIONS_SCHEMA_VERSION]
        );
        assert_eq!(
            catalog.payload_schemas,
            vec![
                RuntimePayloadSchema {
                    id: "binding-result",
                    version: crate::BINDING_RESULT_PAYLOAD_VERSION,
                },
                RuntimePayloadSchema {
                    id: "operation-metadata",
                    version: crate::BINDING_OPERATION_SCHEMA_VERSION,
                },
            ]
        );
        let mut expected_metadata_ids = BINDING_METADATA_IDS.to_vec();
        expected_metadata_ids.sort_unstable();
        assert_eq!(catalog.metadata_ids, expected_metadata_ids);
        assert_eq!(
            resources.limits.iter().any(|limit| limit.hard_cap),
            cfg!(any(feature = "png", feature = "jpeg", feature = "pdf"))
        );
        assert!(
            resources
                .limits
                .iter()
                .filter(|limit| limit.hard_cap)
                .all(|limit| !limit.overridable)
        );
        assert!(
            resources
                .limits
                .iter()
                .any(|limit| limit.id == "max_model_items")
        );
        assert!(resources.limits.iter().all(|limit| {
            limit.minimum_value == usize::from(limit.id != "max_document_diagrams")
        }));
        let interactive = resources
            .profiles
            .iter()
            .find(|profile| profile.id == "interactive")
            .expect("interactive profile");
        assert_eq!(interactive.limits["max_model_items"], Some(32_000));
        let source = resources
            .limits
            .iter()
            .find(|limit| limit.id == "max_source_bytes")
            .expect("source descriptor");
        assert_eq!(source.operation_ids, catalog.capabilities.operation_ids);
        let layout = resources
            .limits
            .iter()
            .find(|limit| limit.id == "max_layout_work_units");
        #[cfg(feature = "svg")]
        {
            let expected_operation_ids = ["jpeg", "layout-json", "pdf", "png", "svg"]
                .into_iter()
                .filter(|operation_id| catalog.capabilities.has_operation(operation_id))
                .collect::<Vec<_>>();
            assert_eq!(
                layout.expect("layout descriptor").operation_ids,
                expected_operation_ids
            );
        }
        #[cfg(not(feature = "svg"))]
        assert!(layout.is_none());
        #[cfg(feature = "svg")]
        assert_eq!(interactive.limits["max_layout_work_units"], Some(250_000));
        let json: Value = serde_json::from_slice(&runtime_catalog_json(2).unwrap()).unwrap();
        assert_eq!(json["schema_version"], RUNTIME_CATALOG_SCHEMA_VERSION);
        assert_eq!(json["transport_api_version"], 2);
        assert!(json.get("abi_version").is_none());
        assert!(json.get("features").is_none());
        assert!(json.get("runtime_contract").is_none());
        assert!(json.get("capability_vocabulary").is_none());
        assert_eq!(
            json["options_schema_versions"],
            serde_json::json!([crate::BINDING_OPTIONS_SCHEMA_VERSION])
        );
        assert_eq!(
            json["payload_schemas"],
            serde_json::to_value(&catalog.payload_schemas).unwrap()
        );
        assert_eq!(
            json["metadata_ids"],
            serde_json::to_value(&catalog.metadata_ids).unwrap()
        );
        assert_eq!(
            json["capabilities"],
            serde_json::to_value(&catalog.capabilities).unwrap()
        );
        assert_eq!(
            json["output_contracts"],
            serde_json::to_value(&catalog.output_contracts).unwrap()
        );
        assert_eq!(json, serde_json::to_value(catalog).unwrap());
    }

    #[test]
    fn every_descriptor_output_has_an_explicit_runtime_contract_owner() {
        for output in capability_descriptor::OUTPUTS {
            assert!(
                matches!(output.id, "ascii" | "jpeg" | "pdf" | "png" | "svg"),
                "descriptor output `{}` needs an explicit runtime output-contract owner",
                output.id
            );
        }
    }

    #[test]
    fn binding_operation_vocabulary_matches_the_abi_owned_operation_metadata() {
        let mut abi_operation_ids = crate::BindingOperationKind::all()
            .map(|operation| operation.operation_id())
            .collect::<Vec<_>>();
        abi_operation_ids.sort_unstable();
        assert_eq!(
            capability_descriptor::BINDING_OPERATION_IDS,
            abi_operation_ids.as_slice()
        );

        for descriptor in capability_descriptor::BINDING_OPERATIONS {
            let operation = crate::BindingOperationKind::from_id(descriptor.id)
                .expect("descriptor operation must be present in the ABI projection");
            assert_eq!(operation.required_capability_id(), descriptor.capability_id);
            assert_eq!(operation.media_type(), descriptor.media_type);
            assert_eq!(operation.requires_uri(), descriptor.requires_uri);
        }

        let semantic = capability_descriptor::BINDING_OPERATIONS
            .iter()
            .find(|operation| operation.id == "semantic-json")
            .expect("semantic operation must remain discoverable");
        assert_eq!(semantic.capability_id, None);
        assert!(
            capability_descriptor::BINDING_OPERATIONS
                .iter()
                .any(|operation| operation.id == "validation-json")
        );
        assert!(
            capability_descriptor::BINDING_OPERATIONS
                .iter()
                .any(|operation| operation.id == "document-analysis-json")
        );
    }

    #[cfg(all(feature = "svg", feature = "analysis"))]
    #[test]
    fn transport_owned_projection_hides_svg_resources_but_keeps_semantic_limits() {
        let surface = ArtifactCapabilitySurface::new(vec!["analysis"], vec![], vec![], None)
            .expect("analysis-only artifact surface");
        let capabilities = surface.runtime_capabilities();
        let catalog = runtime_catalog_for(2, surface);

        assert_eq!(catalog.capabilities, capabilities);
        let resources = catalog.resources;
        assert_eq!(resources.limits.len(), 5);
        assert!(
            resources
                .limits
                .iter()
                .any(|limit| limit.id == "max_model_items")
        );
        assert!(
            resources
                .profiles
                .iter()
                .all(|profile| profile.limits.len() == 5)
        );
        assert!(
            resources
                .limits
                .iter()
                .any(|limit| limit.id == "max_document_diagrams")
        );
    }

    #[test]
    fn supported_diagrams_exposes_binding_surface() {
        assert_eq!(supported_diagrams(), merman::supported_diagrams());
        assert!(supported_diagrams().contains(&"flowchart"));
        assert!(supported_diagrams().contains(&"sequence"));
        assert!(supported_diagrams().contains(&"requirement"));
    }

    #[test]
    fn diagram_family_capabilities_expose_the_complete_core_catalog() {
        let capabilities = diagram_family_capabilities();
        assert_eq!(capabilities, merman::diagram_family_capabilities());

        let flowchart = capabilities
            .iter()
            .find(|capability| capability.diagram_type == "flowchart")
            .expect("flowchart capability should be present");
        assert_eq!(flowchart.metadata_id, Some("flowchart"));
        assert_eq!(flowchart.logical_family_kind, "flowchart");
        assert_eq!(flowchart.render_model_kind, Some("flowchart"));
        assert!(flowchart.has_detector);
        assert!(flowchart.has_semantic_parser);
        assert!(flowchart.has_editor_parser);
        assert!(flowchart.has_combined_parser);
        assert!(flowchart.has_render_parser);
        assert!(!flowchart.has_header);
        assert_eq!(flowchart.config_namespace, Some("flowchart"));

        let swimlane = capabilities
            .iter()
            .find(|capability| capability.diagram_type == "swimlane")
            .expect("11.16 swimlane capability should be present");
        assert_eq!(swimlane.metadata_id, Some("swimlane"));
        assert_eq!(swimlane.logical_family_kind, "swimlane");
        assert_eq!(swimlane.render_model_kind, Some("flowchart"));
        assert!(swimlane.has_detector);
        assert!(swimlane.has_semantic_parser);
        assert!(swimlane.has_editor_parser);
        assert!(swimlane.has_combined_parser);
        assert!(swimlane.has_render_parser);
        assert!(swimlane.has_header);
        assert_eq!(swimlane.config_namespace, Some("swimlane"));

        let cynefin = capabilities
            .iter()
            .find(|capability| capability.diagram_type == "cynefin")
            .expect("11.16 cynefin capability should be present");
        assert_eq!(cynefin.metadata_id, Some("cynefin"));
        assert!(cynefin.has_semantic_parser);
        assert!(cynefin.has_render_parser);

        let railroad = capabilities
            .iter()
            .find(|capability| capability.diagram_type == "railroad")
            .expect("11.16 railroad capability should be present");
        assert_eq!(railroad.metadata_id, Some("railroad"));
        assert!(railroad.has_semantic_parser);
        assert!(railroad.has_render_parser);

        for diagram_type in ["railroadEbnf", "railroadAbnf", "railroadPeg"] {
            let railroad_variant = capabilities
                .iter()
                .find(|capability| capability.diagram_type == diagram_type)
                .unwrap_or_else(|| panic!("11.16 {diagram_type} capability should be present"));
            assert_eq!(railroad_variant.metadata_id, Some(diagram_type));
            assert!(railroad_variant.has_semantic_parser);
            assert!(railroad_variant.has_render_parser);
        }

        assert!(
            capabilities
                .iter()
                .any(|capability| capability.diagram_type == "mindmap")
        );
    }

    #[test]
    fn supported_host_theme_presets_exposes_render_theme_surface() {
        if cfg!(feature = "svg") {
            assert_eq!(
                supported_host_theme_presets(),
                &[
                    "editor-light",
                    "editor-dark",
                    "one-dark",
                    "gruvbox-light",
                    "gruvbox-dark",
                    "ayu-light",
                    "ayu-dark",
                    "merman-modern",
                    "mermaid"
                ]
            );
        } else {
            assert!(supported_host_theme_presets().is_empty());
        }
    }

    #[test]
    fn ascii_supported_diagrams_reflects_feature_surface() {
        if cfg!(feature = "ascii") {
            assert_eq!(
                ascii_supported_diagrams(),
                &[
                    "class",
                    "er",
                    "flowchart",
                    "gantt",
                    "gitgraph",
                    "journey",
                    "kanban",
                    "mindmap",
                    "packet",
                    "sequence",
                    "state",
                    "timeline",
                    "treeView",
                    "xychart",
                ]
            );
        } else {
            assert!(ascii_supported_diagrams().is_empty());
        }
    }

    #[test]
    fn ascii_supported_diagrams_are_derived_from_capability_records() {
        let capabilities = ascii_capabilities();

        if cfg!(feature = "ascii") {
            let supported: Vec<_> = capabilities
                .iter()
                .filter(|capability| capability.support_level != "unsupported")
                .map(|capability| capability.diagram_type)
                .collect();

            assert_eq!(ascii_supported_diagrams(), supported.as_slice());
            assert!(!supported.contains(&"zenuml"));
        } else {
            assert!(capabilities.is_empty());
            assert!(ascii_supported_diagrams().is_empty());
        }
    }

    #[test]
    fn ascii_capabilities_report_support_levels_limits_and_evidence() {
        let capabilities = ascii_capabilities();

        if !cfg!(feature = "ascii") {
            assert!(capabilities.is_empty());
            return;
        }

        let flowchart = ascii_capability(&capabilities, "flowchart");
        assert_eq!(flowchart.support_level, "full");
        assert!(!flowchart.summary_fallback);
        assert!(flowchart.supported_semantics.contains(&"root directions"));
        assert!(flowchart.evidence.iter().any(|evidence| {
            evidence.kind == "local_advantage" && evidence.note.contains("true RL/BT")
        }));

        let class = ascii_capability(&capabilities, "class");
        assert_eq!(class.support_level, "partial");
        assert!(class.summary_fallback);
        assert!(class.limits.iter().any(|limit| limit.contains("namespace")));
        assert!(class.evidence.iter().any(|evidence| {
            evidence.kind == "beautiful_mermaid_prior_art"
                && evidence.source.contains("repo-ref/beautiful-mermaid")
        }));

        let er = ascii_capability(&capabilities, "er");
        assert_eq!(er.support_level, "partial");
        assert!(er.summary_fallback);

        let gantt = ascii_capability(&capabilities, "gantt");
        assert_eq!(gantt.support_level, "summary");

        let xychart = ascii_capability(&capabilities, "xychart");
        assert_eq!(xychart.support_level, "partial");
        assert!(xychart.evidence.iter().any(|evidence| {
            evidence.kind == "beautiful_mermaid_prior_art"
                && evidence.source.contains("xychart-ascii.test.ts")
        }));

        assert!(
            capabilities
                .iter()
                .all(|capability| capability.diagram_type != "zenuml"),
            "ZenUML has no family-owned terminal projection"
        );
    }

    #[test]
    fn metadata_json_helpers_return_json_contracts() {
        let diagrams: Value = serde_json::from_slice(&supported_diagrams_json().unwrap()).unwrap();
        let ascii_diagrams: Value =
            serde_json::from_slice(&ascii_supported_diagrams_json().unwrap()).unwrap();
        let ascii_capabilities: Value =
            serde_json::from_slice(&ascii_capabilities_json().unwrap()).unwrap();
        let themes: Value = serde_json::from_slice(&supported_themes_json().unwrap()).unwrap();
        let host_presets: Value =
            serde_json::from_slice(&supported_host_theme_presets_json().unwrap()).unwrap();
        let family_capabilities: Value =
            serde_json::from_slice(&diagram_family_capabilities_json().unwrap()).unwrap();
        assert!(
            diagrams
                .as_array()
                .unwrap()
                .contains(&Value::String("flowchart".to_string()))
        );
        assert!(ascii_diagrams.is_array());
        assert!(ascii_capabilities.is_array());
        if cfg!(feature = "ascii") {
            let flowchart = ascii_capabilities
                .as_array()
                .unwrap()
                .iter()
                .find(|capability| capability["diagram_type"] == "flowchart")
                .expect("flowchart ASCII capability should be present");
            assert_eq!(flowchart["support_level"], "full");
            assert!(
                flowchart["evidence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|evidence| evidence["kind"] == "local_advantage")
            );
        }
        assert!(
            themes
                .as_array()
                .unwrap()
                .contains(&Value::String("default".to_string()))
        );
        assert!(host_presets.is_array());
        if cfg!(feature = "svg") {
            assert!(
                host_presets
                    .as_array()
                    .unwrap()
                    .contains(&Value::String("one-dark".to_string()))
            );
        }
        let flowchart = family_capabilities
            .as_array()
            .unwrap()
            .iter()
            .find(|capability| capability["diagram_type"] == "flowchart")
            .expect("flowchart family capability should be present");
        assert_eq!(flowchart["logical_family_kind"], "flowchart");
        assert_eq!(flowchart["render_model_kind"], "flowchart");
        assert_eq!(flowchart["has_detector"], true);
        assert_eq!(flowchart["has_editor_parser"], true);
        assert_eq!(flowchart["has_combined_parser"], true);
        assert_eq!(flowchart["has_header"], false);
        assert_eq!(flowchart["config_namespace"], "flowchart");
        if cfg!(feature = "analysis") {
            let lint_rules: Value =
                serde_json::from_slice(&lint_rule_catalog_json().unwrap()).unwrap();
            let configurable_lint_rules: Value =
                serde_json::from_slice(&configurable_lint_rule_catalog_json().unwrap()).unwrap();

            assert_eq!(lint_rules["version"], 1);
            let lint_rules = lint_rules["rules"].as_array().unwrap();
            assert!(lint_rules.iter().any(|rule| {
                rule["id"] == "merman.authoring.flowchart.explicit_direction"
                    && rule["origin"] == "merman_authoring"
                    && rule["evidence"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|value| value == "docs/adr/0072-lint-rule-governance.md")
            }));
            assert_eq!(configurable_lint_rules["version"], 1);
            assert!(
                configurable_lint_rules["rules"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|rule| rule["category"] != "internal")
            );
        } else {
            let lint_error = lint_rule_catalog().unwrap_err();
            assert_eq!(lint_error.status(), BindingStatus::UnsupportedOperation);
            assert_eq!(
                lint_error.kind(),
                crate::BindingErrorKind::MissingCapability
            );
            assert_eq!(lint_error.capability_id(), Some("analysis"));
            let configurable_error = configurable_lint_rule_catalog().unwrap_err();
            assert_eq!(
                configurable_error.status(),
                BindingStatus::UnsupportedOperation
            );
            assert_eq!(
                configurable_error.kind(),
                crate::BindingErrorKind::MissingCapability
            );
            assert_eq!(configurable_error.capability_id(), Some("analysis"));
            assert_eq!(
                lint_rule_catalog_json().unwrap_err().status(),
                BindingStatus::UnsupportedOperation
            );
            assert_eq!(
                configurable_lint_rule_catalog_json().unwrap_err().status(),
                BindingStatus::UnsupportedOperation
            );
        }
    }

    #[test]
    fn binding_metadata_json_dispatches_the_six_public_catalogs() {
        type MetadataProvider = fn() -> Result<Vec<u8>, BindingError>;
        let cases: [(&str, MetadataProvider); 6] = [
            ("supported-diagrams", supported_diagrams_json),
            ("ascii-capabilities", ascii_capabilities_json),
            (
                "diagram-family-capabilities",
                diagram_family_capabilities_json,
            ),
            ("lint-rule-catalog", lint_rule_catalog_json),
            ("supported-themes", supported_themes_json),
            (
                "supported-host-theme-presets",
                supported_host_theme_presets_json,
            ),
        ];
        assert_eq!(cases.map(|(id, _)| id), BINDING_METADATA_IDS);

        for (id, expected) in cases {
            match (binding_metadata_json(id), expected()) {
                (Ok(actual), Ok(expected)) => assert_eq!(actual, expected, "{id}"),
                (Err(actual), Err(expected)) => {
                    assert_eq!(actual.status(), expected.status(), "{id}");
                    assert_eq!(actual.kind(), expected.kind(), "{id}");
                    assert_eq!(actual.capability_id(), expected.capability_id(), "{id}");
                }
                (actual, expected) => {
                    panic!(
                        "metadata dispatcher drifted for `{id}`: actual={actual:?}, expected={expected:?}"
                    )
                }
            }
        }
    }

    #[test]
    fn binding_metadata_json_rejects_unknown_catalogs() {
        let error = binding_metadata_json("unknown-catalog").unwrap_err();

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert_eq!(error.kind(), crate::BindingErrorKind::Generic);
        assert_eq!(error.capability_id(), None);
        assert!(error.message().contains("unknown binding metadata catalog"));
    }

    fn ascii_capability<'a>(
        capabilities: &'a [BindingAsciiCapability],
        diagram_type: &str,
    ) -> &'a BindingAsciiCapability {
        capabilities
            .iter()
            .find(|capability| capability.diagram_type == diagram_type)
            .unwrap_or_else(|| panic!("missing ASCII capability for {diagram_type}"))
    }
}
