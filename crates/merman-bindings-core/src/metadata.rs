use crate::artifact_contract::ValidatedArtifactContract;
use crate::capability as capability_descriptor;
use crate::common::{BindingError, internal_json_error};
use crate::metadata_registry::{MetadataHandlerKey, MetadataKey};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// First public schema for the artifact-owned runtime catalog.
pub const RUNTIME_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const THEME_CATALOG_SCHEMA_VERSION: u32 = 3;
pub const TEXT_MEASUREMENT_PROVIDER_HOST_CALLBACK: &str = "host-callback";
pub const TEXT_MEASUREMENT_PROVIDER_DETERMINISTIC: &str = "deterministic";

static SUPPORTED_DIAGRAMS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static ASCII_SUPPORTED_DIAGRAMS_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static ASCII_CAPABILITIES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static SUPPORTED_THEMES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
static DIAGRAM_FAMILY_CAPABILITIES_JSON: OnceLock<Vec<u8>> = OnceLock::new();
#[cfg(feature = "analysis")]
static LINT_RULE_CATALOG_JSON: OnceLock<Vec<u8>> = OnceLock::new();
#[cfg(feature = "analysis")]
static CONFIGURABLE_LINT_RULE_CATALOG_JSON: OnceLock<Vec<u8>> = OnceLock::new();

/// The text-measurement routes exposed by one artifact.
///
/// The protocol version belongs to the independently versioned text-measurement contract. Provider
/// IDs describe actual installation routes, not Cargo features: `deterministic` is the built-in
/// renderer measurer and `host-callback` means this transport accepts a host callback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct TextMeasurementCapabilities {
    pub protocol_version: u32,
    pub provider_ids: Vec<&'static str>,
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
    /// Canonical top-level option fields and objects accepted by this artifact.
    pub option_group_ids: Vec<&'static str>,
    /// Immutable host services accepted while constructing a reusable engine.
    pub constructor_service_ids: Vec<&'static str>,
    /// Structured contracts for the immutable constructor services exposed by this artifact.
    pub constructor_service_contracts: Vec<RuntimeConstructorServiceContract>,
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

/// Runtime contract for one immutable service accepted during reusable-engine construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimeConstructorServiceContract {
    pub id: &'static str,
    pub provided_text_measurement_provider_ids: Vec<&'static str>,
    pub resource_limits: Vec<RuntimeConstructorResourceLimit>,
}

/// One compiled, caller-immutable resource ceiling owned by a constructor service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuntimeConstructorResourceLimit {
    pub id: &'static str,
    pub phase: &'static str,
    pub unit: &'static str,
    pub description: &'static str,
    pub value: u64,
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
#[non_exhaustive]
pub struct BindingAsciiCapability {
    pub diagram_type: &'static str,
    pub display_name: &'static str,
    pub semantic_coverage: Option<&'static str>,
    pub primary_projection: &'static str,
    pub structured_text_fallback: bool,
    pub layout_profiles: Vec<&'static str>,
    pub width_profiles: Vec<&'static str>,
    pub encodings: Vec<&'static str>,
    pub fallback_encodings: Vec<&'static str>,
    /// Compatibility view derived from semantic coverage and the primary projection.
    pub support_level: &'static str,
    pub supported_semantics: &'static [&'static str],
    pub limits: &'static [&'static str],
    pub evidence: Vec<BindingAsciiCapabilityEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BindingAsciiCapabilityEvidence {
    pub kind: &'static str,
    pub source: &'static str,
    pub note: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RuleCatalogEntry {
    pub id: &'static str,
    pub description: &'static str,
    pub evidence: &'static [&'static str],
    pub default_severity: &'static str,
    pub category: &'static str,
    pub tags: Vec<&'static str>,
    pub default_enabled: bool,
    pub default_profile: &'static str,
    pub origin: &'static str,
    pub configurable: bool,
    pub fixable: bool,
}

#[derive(Debug, Serialize)]
struct BindingThemeCatalog {
    schema_version: u32,
    structured_spec_available: bool,
    supported_output_ids: Vec<&'static str>,
    #[cfg(feature = "svg")]
    presets: Vec<merman_theme_contract::ThemePresetMetadataV1>,
    #[cfg(not(feature = "svg"))]
    presets: [(); 0],
    known_capability_ids: Vec<&'static str>,
    known_text_capability_ids: Vec<&'static str>,
    known_font_container_ids: Vec<&'static str>,
    known_font_source_ids: Vec<&'static str>,
    known_semantic_target_ids: Vec<&'static str>,
    known_variant_ids: Vec<&'static str>,
    resource_limits: Vec<BindingThemeResourceLimit>,
}

#[derive(Debug, Serialize)]
struct BindingThemeResourceLimit {
    id: &'static str,
    phase: &'static str,
    description: &'static str,
    effective_value: Option<u64>,
    hard_cap: bool,
}

impl ValidatedArtifactContract {
    /// Produces the exact open-string capability DTO advertised by this transport.
    #[must_use]
    pub fn runtime_capabilities(&self) -> RuntimeCapabilities {
        let provider_ids = stable_ids(
            self.text_measurement_provider_keys(),
            crate::TextMeasurementProviderKey::id,
        );
        #[cfg(feature = "svg")]
        let text_measurement = (!provider_ids.is_empty()).then_some(TextMeasurementCapabilities {
            protocol_version: merman::svg::TEXT_MEASUREMENT_PROTOCOL_VERSION,
            provider_ids,
        });
        #[cfg(not(feature = "svg"))]
        let text_measurement = {
            debug_assert!(provider_ids.is_empty());
            None
        };

        RuntimeCapabilities {
            capability_ids: stable_ids(self.capability_keys(), crate::CapabilityKey::id),
            output_ids: stable_ids(self.output_keys(), crate::OutputKey::id),
            operation_ids: stable_ids(self.operation_keys(), crate::OperationKey::id),
            system_adapter_ids: stable_ids(self.system_adapter_keys(), crate::CapabilityKey::id),
            text_measurement,
        }
    }

    /// Builds one atomic runtime catalog from the same selection used for admission and dispatch.
    #[must_use]
    pub fn runtime_catalog(&self, transport_api_version: u32) -> RuntimeCatalog {
        let capabilities = self.runtime_capabilities();
        let constructor_service_contracts = runtime_constructor_service_contracts_for(self);
        let payload_schemas = self
            .payload_schema_keys()
            .map(|schema| RuntimePayloadSchema {
                id: schema.id(),
                version: schema.version(),
            })
            .collect::<Vec<_>>();

        let catalog = RuntimeCatalog {
            schema_version: RUNTIME_CATALOG_SCHEMA_VERSION,
            transport_api_version,
            package_version: env!("CARGO_PKG_VERSION"),
            options_schema_versions: vec![crate::BINDING_OPTIONS_SCHEMA_VERSION],
            payload_schemas,
            metadata_ids: stable_ids(self.metadata_keys(), MetadataKey::id),
            option_group_ids: stable_ids(
                self.option_group_keys(),
                crate::BindingOptionGroupKey::id,
            ),
            constructor_service_ids: stable_ids(
                self.constructor_service_keys(),
                crate::ConstructorServiceKey::id,
            ),
            constructor_service_contracts,
            output_contracts: runtime_output_contracts_for(&capabilities),
            registry: RuntimeRegistryContract {
                diagram_family_count: diagram_family_capabilities().len(),
            },
            resources: runtime_resource_contract_for(self, &capabilities),
            capabilities,
        };
        assert_runtime_catalog_json_safe_integers(&catalog);
        catalog
    }

    pub fn runtime_catalog_json(
        &self,
        transport_api_version: u32,
    ) -> Result<Vec<u8>, BindingError> {
        serde_json::to_vec(&self.runtime_catalog(transport_api_version))
            .map_err(internal_json_error)
    }

    /// Dispatches only metadata advertised by this exact transport contract.
    pub fn metadata_json(&self, id: &str) -> Result<Vec<u8>, BindingError> {
        let key = MetadataKey::from_id(id).ok_or_else(|| {
            BindingError::invalid_argument(format!("unknown binding metadata catalog `{id}`"))
        })?;
        if !self.exposes_metadata(key) {
            return Err(BindingError::unsupported_operation(format!(
                "binding metadata catalog `{id}` is not exposed by target `{}`",
                self.target().id()
            )));
        }
        collect_metadata(self, key)
    }

    /// Projects theme discovery through an explicit host-owned resource policy.
    #[cfg(feature = "svg")]
    pub fn theme_catalog_json_with_resource_policy(
        &self,
        policy: &merman::svg::ThemeResourcePolicy,
    ) -> Result<Vec<u8>, BindingError> {
        let compiler =
            merman::svg::DiagramThemeCompiler::new().with_resource_policy(policy.clone());
        self.metadata_json_with_theme_compiler(MetadataKey::ThemeCatalog.id(), &compiler)
    }

    #[cfg(feature = "svg")]
    pub(crate) fn metadata_json_with_theme_compiler(
        &self,
        id: &str,
        compiler: &merman::svg::DiagramThemeCompiler,
    ) -> Result<Vec<u8>, BindingError> {
        let key = MetadataKey::from_id(id).ok_or_else(|| {
            BindingError::invalid_argument(format!("unknown binding metadata catalog `{id}`"))
        })?;
        if !self.exposes_metadata(key) {
            return Err(BindingError::unsupported_operation(format!(
                "binding metadata catalog `{id}` is not exposed by target `{}`",
                self.target().id()
            )));
        }
        collect_metadata_with_theme_compiler(self, key, Some(compiler))
    }
}

fn assert_runtime_catalog_json_safe_integers(catalog: &RuntimeCatalog) {
    let maximum = u128::from(crate::RUNTIME_CATALOG_MAX_SAFE_INTEGER);
    let assert_safe = |value: u128, field: &str| {
        assert!(
            value <= maximum,
            "runtime catalog field `{field}` exceeds the JSON-safe integer maximum"
        );
    };

    assert_safe(u128::from(catalog.schema_version), "schema_version");
    assert_safe(
        u128::from(catalog.transport_api_version),
        "transport_api_version",
    );
    for version in &catalog.options_schema_versions {
        assert_safe(u128::from(*version), "options_schema_versions");
    }
    for schema in &catalog.payload_schemas {
        assert_safe(u128::from(schema.version), "payload_schemas.version");
    }
    if let Some(text_measurement) = &catalog.capabilities.text_measurement {
        assert_safe(
            u128::from(text_measurement.protocol_version),
            "capabilities.text_measurement.protocol_version",
        );
    }
    for service in &catalog.constructor_service_contracts {
        for limit in &service.resource_limits {
            assert_safe(
                u128::from(limit.value),
                "constructor_service_contracts.resource_limits.value",
            );
        }
    }
    for output in &catalog.output_contracts {
        if let Some(images) = &output.embedded_images {
            for limit in [
                images.limits.max_bytes_per_image,
                images.limits.max_total_bytes,
                images.limits.max_pixels_per_image,
                images.limits.max_total_pixels,
            ]
            .into_iter()
            .flatten()
            {
                assert_safe(u128::from(limit), "output_contracts.embedded_images.limits");
            }
        }
    }
    assert_safe(
        catalog.registry.diagram_family_count as u128,
        "registry.diagram_family_count",
    );
    for limit in &catalog.resources.limits {
        assert_safe(
            limit.minimum_value as u128,
            "resources.limits.minimum_value",
        );
    }
    for profile in &catalog.resources.profiles {
        for value in profile.limits.values().flatten() {
            assert_safe(*value as u128, "resources.profiles.limits");
        }
    }
}

fn stable_ids<T>(
    values: impl IntoIterator<Item = T>,
    id: impl Fn(T) -> &'static str,
) -> Vec<&'static str> {
    // Every typed vocabulary is declared in stable-ID order, and artifact selections retain that
    // order. Keeping the descriptor order avoids pulling generic sorting machinery into minimal
    // native artifacts while preserving deterministic catalogs.
    values.into_iter().map(id).collect()
}

fn runtime_constructor_service_contracts_for(
    artifact_contract: &ValidatedArtifactContract,
) -> Vec<RuntimeConstructorServiceContract> {
    artifact_contract
        .constructor_service_keys()
        .map(|service| RuntimeConstructorServiceContract {
            id: service.id(),
            provided_text_measurement_provider_ids: stable_ids(
                crate::TextMeasurementProviderKey::ALL
                    .iter()
                    .copied()
                    .filter(|provider| {
                        matches!(
                            provider.source(),
                            crate::TextMeasurementProviderSource::ConstructorService(owner)
                                if owner == service
                        )
                    }),
                crate::TextMeasurementProviderKey::id,
            ),
            resource_limits: runtime_constructor_resource_limits(service),
        })
        .collect()
}

/// Returns the descriptor-owned resource ceilings for one constructor service.
///
/// Runtime catalogs and generated SDK projections share this function so service ownership and
/// resource values cannot drift between the producer and its host-language contracts.
#[must_use]
pub fn runtime_constructor_resource_limits(
    service: crate::ConstructorServiceKey,
) -> Vec<RuntimeConstructorResourceLimit> {
    match service.spec().resource_catalog() {
        None => Vec::new(),
        Some(crate::service_contract::ConstructorServiceResourceCatalog::IconRegistry) => {
            #[cfg(feature = "svg")]
            {
                let mut limits = merman::svg::icon_registry_resource_limit_descriptors()
                    .iter()
                    .map(|descriptor| RuntimeConstructorResourceLimit {
                        id: descriptor.stable_id,
                        phase: descriptor.phase,
                        unit: descriptor.unit,
                        description: descriptor.description,
                        value: descriptor.hard_maximum,
                    })
                    .collect::<Vec<_>>();
                limits.sort_unstable_by_key(|limit| limit.id);
                limits
            }
            #[cfg(not(feature = "svg"))]
            unreachable!("validated no-SVG artifacts cannot expose the icon registry service")
        }
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

fn runtime_resource_contract_for(
    artifact_contract: &ValidatedArtifactContract,
    capabilities: &RuntimeCapabilities,
) -> RuntimeResourceContract {
    let contract = crate::binding_resource_contract();
    let limits = contract
        .limits
        .into_iter()
        .filter(|descriptor| artifact_contract.exposes_resource_limit(descriptor))
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
            .map(|profile| {
                let profile_id = merman::resources::ResourceProfile::from_id(profile.id)
                    .expect("compiled resource profile id");
                RuntimeResourceProfile {
                    id: profile.id,
                    purpose: profile.purpose,
                    trust_assumption: profile.trust_assumption,
                    recommended_binding_default: profile.recommended_binding_default,
                    limits: limits
                        .iter()
                        .map(|limit| {
                            let value =
                                crate::resource_contract::resource_profile_value_for_target(
                                    profile_id,
                                    limit.stable_id,
                                    artifact_contract.target(),
                                )
                                .expect(
                                    "compiled resource descriptor must have a target profile value",
                                );
                            (limit.stable_id, value)
                        })
                        .collect(),
                }
            })
            .collect(),
    }
}

pub fn diagram_family_capabilities() -> Vec<BindingDiagramFamilyCapability> {
    merman::diagram_family_capabilities().to_vec()
}

pub fn supported_themes() -> &'static [&'static str] {
    merman::supported_themes()
}

#[cfg(feature = "svg")]
fn theme_catalog_for(artifact_contract: &ValidatedArtifactContract) -> BindingThemeCatalog {
    theme_catalog_for_with_compiler(artifact_contract, None)
}

#[cfg(not(feature = "svg"))]
fn theme_catalog_for(_artifact_contract: &ValidatedArtifactContract) -> BindingThemeCatalog {
    BindingThemeCatalog {
        schema_version: THEME_CATALOG_SCHEMA_VERSION,
        structured_spec_available: false,
        supported_output_ids: Vec::new(),
        presets: [],
        known_capability_ids: Vec::new(),
        known_text_capability_ids: Vec::new(),
        known_font_container_ids: Vec::new(),
        known_font_source_ids: Vec::new(),
        known_semantic_target_ids: Vec::new(),
        known_variant_ids: Vec::new(),
        resource_limits: Vec::new(),
    }
}

#[cfg(feature = "svg")]
fn theme_catalog_for_with_compiler(
    artifact_contract: &ValidatedArtifactContract,
    compiler: Option<&merman::svg::DiagramThemeCompiler>,
) -> BindingThemeCatalog {
    if !artifact_contract.exposes_capability(crate::CapabilityKey::Svg) {
        return BindingThemeCatalog {
            schema_version: THEME_CATALOG_SCHEMA_VERSION,
            structured_spec_available: false,
            supported_output_ids: Vec::new(),
            presets: Vec::new(),
            known_capability_ids: Vec::new(),
            known_text_capability_ids: Vec::new(),
            known_font_container_ids: Vec::new(),
            known_font_source_ids: Vec::new(),
            known_semantic_target_ids: Vec::new(),
            known_variant_ids: Vec::new(),
            resource_limits: Vec::new(),
        };
    }

    let default_compiler = merman::svg::DiagramThemeCompiler::new().with_resource_policy(
        merman::svg::ThemeResourcePolicy::for_profile(
            merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
        ),
    );
    let compiler = compiler.unwrap_or(&default_compiler);
    let presets = merman::svg::describe_theme_presets(compiler);
    let supported_output_ids = sorted_theme_ids(
        artifact_contract
            .output_keys()
            .filter(|output| {
                matches!(
                    output,
                    crate::OutputKey::Svg
                        | crate::OutputKey::Png
                        | crate::OutputKey::Jpeg
                        | crate::OutputKey::Pdf
                )
            })
            .map(crate::OutputKey::id)
            .collect(),
    );
    let resources = compiler.resource_policy();
    let mut resource_limits = merman::svg::theme_resource_limit_descriptors()
        .iter()
        .map(|descriptor| BindingThemeResourceLimit {
            id: descriptor.stable_id,
            phase: descriptor.phase.as_str(),
            description: descriptor.description,
            effective_value: resources
                .value(descriptor.id)
                .map(|value| u64::try_from(value).expect("theme limits fit in u64")),
            hard_cap: descriptor.hard_cap,
        })
        .collect::<Vec<_>>();
    resource_limits.sort_unstable_by_key(|limit| limit.id);
    BindingThemeCatalog {
        schema_version: THEME_CATALOG_SCHEMA_VERSION,
        structured_spec_available: true,
        supported_output_ids,
        presets,
        known_capability_ids: sorted_theme_ids(
            merman::svg::ThemeCapability::ALL
                .iter()
                .copied()
                .map(merman::svg::ThemeCapability::id)
                .collect(),
        ),
        known_text_capability_ids: sorted_theme_ids(
            merman::svg::TextLayoutCapability::ALL
                .iter()
                .copied()
                .map(merman::svg::TextLayoutCapability::id)
                .collect(),
        ),
        known_font_container_ids: sorted_theme_ids(
            merman::svg::FontContainer::ALL
                .iter()
                .copied()
                .map(merman::svg::FontContainer::id)
                .collect(),
        ),
        known_font_source_ids: sorted_theme_ids(
            merman::svg::FontSource::ALL
                .iter()
                .copied()
                .map(merman::svg::FontSource::id)
                .collect(),
        ),
        known_semantic_target_ids: sorted_theme_ids(
            merman::svg::ThemeTarget::ALL
                .iter()
                .copied()
                .map(merman::svg::ThemeTarget::id)
                .collect(),
        ),
        known_variant_ids: sorted_theme_ids(
            merman::svg::ThemeVariant::ALL
                .iter()
                .copied()
                .map(merman::svg::ThemeVariant::id)
                .collect(),
        ),
        resource_limits,
    }
}

#[cfg(feature = "svg")]
fn sorted_theme_ids(mut ids: Vec<&'static str>) -> Vec<&'static str> {
    ids.sort_unstable();
    ids.dedup();
    ids
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

pub fn ascii_diagrammatic_diagrams() -> &'static [&'static str] {
    #[cfg(feature = "ascii")]
    {
        merman::ascii::ascii_diagrammatic_diagram_types()
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
                semantic_coverage: capability
                    .semantic_coverage
                    .map(|coverage| coverage.as_str()),
                primary_projection: capability.primary_projection.as_str(),
                structured_text_fallback: capability.structured_text_fallback,
                layout_profiles: capability
                    .layout_profiles
                    .iter()
                    .map(|profile| profile.as_str())
                    .collect(),
                width_profiles: capability
                    .width_profiles
                    .iter()
                    .map(|profile| profile.as_str())
                    .collect(),
                encodings: capability
                    .encodings
                    .iter()
                    .map(|encoding| encoding.as_str())
                    .collect(),
                fallback_encodings: capability
                    .fallback_encodings
                    .iter()
                    .map(|encoding| encoding.as_str())
                    .collect(),
                support_level: capability.support_level.as_str(),
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

fn theme_catalog_json_for(
    artifact_contract: &ValidatedArtifactContract,
) -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(&theme_catalog_for(artifact_contract)).map_err(internal_json_error)
}

#[cfg(feature = "svg")]
pub(crate) fn theme_catalog_json_for_compiler(
    artifact_contract: &ValidatedArtifactContract,
    compiler: &merman::svg::DiagramThemeCompiler,
) -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(&theme_catalog_for_with_compiler(
        artifact_contract,
        Some(compiler),
    ))
    .map_err(internal_json_error)
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

fn collect_metadata(
    artifact_contract: &ValidatedArtifactContract,
    key: MetadataKey,
) -> Result<Vec<u8>, BindingError> {
    #[cfg(feature = "svg")]
    {
        return collect_metadata_with_theme_compiler(artifact_contract, key, None);
    }

    #[cfg(not(feature = "svg"))]
    {
        match key.spec().handler() {
            MetadataHandlerKey::AsciiCapabilities => ascii_capabilities_json(),
            MetadataHandlerKey::DiagramFamilyCapabilities => diagram_family_capabilities_json(),
            MetadataHandlerKey::LintRuleCatalog => lint_rule_catalog_json(),
            MetadataHandlerKey::SupportedDiagrams => supported_diagrams_json(),
            MetadataHandlerKey::SupportedThemes => supported_themes_json(),
            MetadataHandlerKey::ThemeCatalog => theme_catalog_json_for(artifact_contract),
        }
    }
}

#[cfg(feature = "svg")]
pub(crate) fn collect_metadata_with_theme_compiler(
    artifact_contract: &ValidatedArtifactContract,
    key: MetadataKey,
    compiler: Option<&merman::svg::DiagramThemeCompiler>,
) -> Result<Vec<u8>, BindingError> {
    match key.spec().handler() {
        MetadataHandlerKey::AsciiCapabilities => ascii_capabilities_json(),
        MetadataHandlerKey::DiagramFamilyCapabilities => diagram_family_capabilities_json(),
        MetadataHandlerKey::LintRuleCatalog => lint_rule_catalog_json(),
        MetadataHandlerKey::SupportedDiagrams => supported_diagrams_json(),
        MetadataHandlerKey::SupportedThemes => supported_themes_json(),
        MetadataHandlerKey::ThemeCatalog => match compiler {
            Some(compiler) => theme_catalog_json_for_compiler(artifact_contract, compiler),
            None => theme_catalog_json_for(artifact_contract),
        },
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
        tags: rule.tags.iter().map(|tag| tag.as_str()).collect(),
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
    use crate::{ArtifactContractSpec, BindingStatus, OperationKey, TargetKey};
    use serde_json::Value;

    fn full_native_contract() -> ValidatedArtifactContract {
        crate::artifact_contract::DEFAULT_ARTIFACT_SNAPSHOT
    }

    fn semantic_contract() -> ValidatedArtifactContract {
        ArtifactContractSpec::new(TargetKey::Native, crate::BindingTransportKey::Rust)
            .with_operations(&[OperationKey::SemanticJson])
            .materialize()
    }

    fn expected_native_system_adapter_ids() -> Vec<&'static str> {
        let mut ids = if cfg!(feature = "native-runtime") {
            vec!["system-clock", "system-random", "system-timezone"]
        } else {
            Vec::new()
        };
        ids.sort_unstable();
        ids
    }

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
        let capabilities = full_native_contract().runtime_capabilities();

        assert_eq!(capabilities.has_capability("svg"), cfg!(feature = "svg"));
        assert_eq!(
            capabilities.has_capability("analysis"),
            cfg!(feature = "analysis")
        );
        assert_eq!(
            capabilities.has_capability("ascii"),
            cfg!(feature = "ascii")
        );
        assert_eq!(
            capabilities.system_adapter_ids,
            expected_native_system_adapter_ids()
        );
        let (expected_layout_cytoscape, expected_layout_elk, expected_math) = (
            cfg!(feature = "layout-cytoscape"),
            cfg!(feature = "layout-elk"),
            cfg!(feature = "math"),
        );
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
        let mut compiled_operation_ids = crate::compiled_operation_kind_ids();
        compiled_operation_ids.retain(|id| {
            crate::OperationKey::from_id(id).is_some_and(|operation| {
                operation.spec().targets.contains(&crate::TargetKey::Native)
            })
        });
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
                    TEXT_MEASUREMENT_PROVIDER_DETERMINISTIC,
                    TEXT_MEASUREMENT_PROVIDER_HOST_CALLBACK,
                ]
            );
        }
        #[cfg(not(feature = "svg"))]
        assert!(capabilities.text_measurement.is_none());
    }

    #[test]
    fn runtime_catalog_json_reports_stable_id_sets() {
        let contract = full_native_contract();
        let catalog: Value =
            serde_json::from_slice(&contract.runtime_catalog_json(2).unwrap()).unwrap();
        let capabilities = &catalog["capabilities"];

        assert!(capabilities.get("render").is_none());
        assert!(capabilities.get("ratex_math").is_none());
        assert!(capabilities["capability_ids"].is_array());
        assert!(capabilities["output_ids"].is_array());
        assert!(capabilities["operation_ids"].is_array());
        assert!(capabilities["system_adapter_ids"].is_array());
        assert_eq!(
            capabilities,
            &serde_json::to_value(contract.runtime_capabilities()).unwrap()
        );
    }

    #[test]
    fn native_contract_exposes_the_exact_transport_owned_runtime_adapters() {
        let transport = full_native_contract().runtime_capabilities();
        let compiled_system_adapter_ids = merman::runtime::compiled_system_adapter_ids();
        let expected_system_adapter_ids = expected_native_system_adapter_ids();

        assert_eq!(transport.system_adapter_ids, expected_system_adapter_ids);
        for adapter_id in compiled_system_adapter_ids {
            assert_eq!(
                transport.has_capability(adapter_id),
                expected_system_adapter_ids.contains(adapter_id),
                "binding catalog must report adapter `{adapter_id}` exactly when JSON policy can select it"
            );
        }
        assert!(!transport.has_capability("system-timing"));
        assert!(!transport.system_adapter_ids.contains(&"system-timing"));
    }

    #[test]
    fn runtime_catalog_is_versioned_and_projects_the_resource_descriptor() {
        let contract = full_native_contract();
        let catalog = contract.runtime_catalog(2);
        assert_eq!(catalog.schema_version, RUNTIME_CATALOG_SCHEMA_VERSION);
        assert_eq!(catalog.transport_api_version, 2);
        assert_eq!(catalog.package_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(catalog.capabilities, contract.runtime_capabilities());
        assert_eq!(
            catalog.constructor_service_ids,
            catalog
                .constructor_service_contracts
                .iter()
                .map(|service| service.id)
                .collect::<Vec<_>>()
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
        let expected_metadata_ids = contract
            .metadata_keys()
            .map(MetadataKey::id)
            .collect::<Vec<_>>();
        assert_eq!(catalog.metadata_ids, expected_metadata_ids);
        let options_limit = resources
            .limits
            .iter()
            .find(|limit| limit.id == "max_options_json_bytes")
            .expect("binding options document hard cap");
        assert!(options_limit.hard_cap);
        assert!(!options_limit.overridable);
        assert!(options_limit.operation_ids.is_empty());
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
        assert_eq!(
            interactive.limits["max_options_json_bytes"],
            Some(crate::BINDING_OPTIONS_JSON_MAX_BYTES)
        );
        let source = resources
            .limits
            .iter()
            .find(|limit| limit.id == "max_source_bytes")
            .expect("source descriptor");
        let expected_source_operations = catalog
            .capabilities
            .operation_ids
            .iter()
            .copied()
            .filter(|operation_id| {
                crate::BindingOperationKind::from_id(operation_id)
                    .is_ok_and(|operation| operation.input_kind() == "mermaid-source")
            })
            .collect::<Vec<_>>();
        assert_eq!(source.operation_ids, expected_source_operations);
        let layout = resources
            .limits
            .iter()
            .find(|limit| limit.id == "max_layout_work_units");
        #[cfg(feature = "svg")]
        {
            let expected_operation_ids =
                ["jpeg", "layout-json", "pdf", "png", "svg", "svg-plan-json"]
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
        assert_eq!(interactive.limits["max_layout_work_units"], Some(800_000));
        let json: Value =
            serde_json::from_slice(&contract.runtime_catalog_json(2).unwrap()).unwrap();
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

    #[cfg(feature = "svg")]
    #[test]
    fn svg_catalog_projects_fixed_icon_registry_resources_without_cli_acquisition_ids() {
        const CLI_ICON_ACQUISITION_AND_NETWORK_RESOURCE_IDS: &[&str] = &[
            "connect_timeout_seconds",
            "max_aggregate_icon_bytes",
            "max_icon_packs",
            "max_local_icon_body_bytes",
            "max_redirects",
            "max_remote_icon_body_bytes",
            "per_hop_timeout_seconds",
            "workflow_timeout_seconds",
        ];

        let catalog = full_native_contract().runtime_catalog(2);
        assert!(
            catalog
                .constructor_service_contracts
                .windows(2)
                .all(|services| services[0].id < services[1].id)
        );

        let host_text_measurement = catalog
            .constructor_service_contracts
            .iter()
            .find(|service| service.id == "host-text-measurement")
            .expect("host text measurement constructor service");
        assert_eq!(
            host_text_measurement.provided_text_measurement_provider_ids,
            ["host-callback"]
        );
        assert!(host_text_measurement.resource_limits.is_empty());

        let icon_registry = catalog
            .constructor_service_contracts
            .iter()
            .find(|service| service.id == "icon-registry")
            .expect("icon registry constructor service");
        assert!(
            icon_registry
                .provided_text_measurement_provider_ids
                .is_empty()
        );
        let descriptors = merman::svg::icon_registry_resource_limit_descriptors();
        assert!(!icon_registry.resource_limits.is_empty());
        assert_eq!(icon_registry.resource_limits.len(), descriptors.len());
        assert!(
            icon_registry
                .resource_limits
                .windows(2)
                .all(|resources| resources[0].id < resources[1].id)
        );
        for resource in &icon_registry.resource_limits {
            let descriptor = descriptors
                .iter()
                .find(|descriptor| descriptor.stable_id == resource.id)
                .expect("resource descriptor");
            assert_eq!(resource.id, descriptor.stable_id);
            assert_eq!(resource.phase, descriptor.phase);
            assert_eq!(resource.unit, descriptor.unit);
            assert_eq!(resource.description, descriptor.description);
            assert_eq!(resource.value, descriptor.hard_maximum);
        }
        assert!(icon_registry.resource_limits.iter().all(|resource| {
            !CLI_ICON_ACQUISITION_AND_NETWORK_RESOURCE_IDS.contains(&resource.id)
        }));

        let json = serde_json::to_value(&catalog).expect("runtime catalog serializes");
        let service_contracts = json["constructor_service_contracts"]
            .as_array()
            .expect("constructor service contracts array");
        for service in service_contracts {
            assert!(service.get("required_provider_ids").is_none());
            assert!(service.get("requires_svg_pipeline").is_none());
            assert!(service.get("fixed_resources").is_none());
            assert!(service.get("caller_configurable").is_none());
        }
    }

    #[test]
    fn catalog_without_svg_exposure_omits_icon_registry_service_contract() {
        let catalog = semantic_contract().runtime_catalog(2);

        assert!(!catalog.capabilities.has_capability("svg"));
        assert!(!catalog.constructor_service_ids.contains(&"icon-registry"));
        assert!(
            catalog
                .constructor_service_contracts
                .iter()
                .all(|service| service.id != "icon-registry")
        );
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
            assert_eq!(
                operation.availability_capability_id(),
                descriptor.capability.map(crate::CapabilityKey::id)
            );
            assert_eq!(operation.media_type(), descriptor.media_type);
            assert_eq!(operation.maturity(), descriptor.maturity);
            assert_eq!(operation.requires_uri(), descriptor.requires_uri);
        }

        let semantic = capability_descriptor::BINDING_OPERATIONS
            .iter()
            .find(|operation| operation.id == "semantic-json")
            .expect("semantic operation must remain discoverable");
        assert_eq!(semantic.capability, None);
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
        const ANALYSIS_OPERATIONS: &[OperationKey] = &[
            OperationKey::AnalysisFactsJson,
            OperationKey::AnalysisJson,
            OperationKey::DocumentAnalysisFactsJson,
            OperationKey::DocumentAnalysisJson,
            OperationKey::SemanticJson,
            OperationKey::ValidationJson,
        ];
        let contract =
            ArtifactContractSpec::new(TargetKey::Native, crate::BindingTransportKey::Rust)
                .with_operations(ANALYSIS_OPERATIONS)
                .materialize();
        let capabilities = contract.runtime_capabilities();
        let catalog = contract.runtime_catalog(2);

        assert_eq!(catalog.capabilities, capabilities);
        let resources = catalog.resources;
        assert_eq!(resources.limits.len(), 6);
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
                .all(|profile| profile.limits.len() == 6)
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
        assert_eq!(flowchart.family_id, merman::DiagramFamilyId::FLOWCHART);
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
        assert_eq!(swimlane.family_id, merman::DiagramFamilyId::SWIMLANE);
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

    #[cfg(feature = "svg")]
    #[test]
    fn preset_qualification_projection_preserves_host_conditions_and_future_ids() {
        #[derive(serde::Deserialize)]
        struct Cell<'a> {
            family_id: &'a str,
            output_id: &'a str,
            profile_id: &'a str,
            admission_status: &'a str,
        }
        #[derive(serde::Deserialize)]
        struct Vector<'a> {
            id: &'a str,
            #[serde(borrow)]
            cell: Cell<'a>,
        }
        const VECTORS: &str = include_str!(
            "../../merman-theme-authoring-fixtures/fixtures/authoring-v1/qualified-cells.json"
        );
        let vectors: Vec<Vector<'static>> = serde_json::from_str(VECTORS).unwrap();
        let expected: Vec<Value> = serde_json::from_str(VECTORS).unwrap();
        for (vector, expected) in vectors.into_iter().zip(expected) {
            let cell = merman::svg::ThemePresetQualifiedCell::new(
                vector.cell.family_id,
                vector.cell.output_id,
                vector.cell.profile_id,
                vector.cell.admission_status,
            );
            let json =
                serde_json::to_value(merman::svg::ThemePresetQualifiedCellV1::from(&cell)).unwrap();
            assert_eq!(json, expected["cell"], "{}", vector.id);
        }
    }

    #[test]
    fn theme_catalog_projects_the_artifact_surface() {
        let empty_contract = semantic_contract();
        let empty: Value =
            serde_json::from_slice(&theme_catalog_json_for(&empty_contract).unwrap()).unwrap();
        assert_eq!(
            empty,
            serde_json::json!({
                "schema_version": THEME_CATALOG_SCHEMA_VERSION,
                "structured_spec_available": false,
                "supported_output_ids": [],
                "presets": [],
                "known_capability_ids": [],
                "known_text_capability_ids": [],
                "known_font_container_ids": [],
                "known_font_source_ids": [],
                "known_semantic_target_ids": [],
                "known_variant_ids": [],
                "resource_limits": [],
            })
        );

        #[cfg(feature = "svg")]
        {
            let svg_contract =
                ArtifactContractSpec::new(TargetKey::Native, crate::BindingTransportKey::Rust)
                    .with_operations(&[OperationKey::Svg])
                    .materialize();
            let catalog: Value =
                serde_json::from_slice(&theme_catalog_json_for(&svg_contract).unwrap()).unwrap();
            assert_eq!(catalog["schema_version"], THEME_CATALOG_SCHEMA_VERSION);
            assert_eq!(catalog["structured_spec_available"], true);
            assert_eq!(catalog["supported_output_ids"], serde_json::json!(["svg"]));
            let compiler = merman::svg::DiagramThemeCompiler::new().with_resource_policy(
                merman::svg::ThemeResourcePolicy::for_profile(
                    merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
                ),
            );
            assert_eq!(
                catalog["presets"],
                serde_json::to_value(merman::svg::describe_theme_presets(&compiler)).unwrap()
            );
            let expected_presets: Value = serde_json::from_str(include_str!(
                "../../merman-theme-authoring-fixtures/fixtures/authoring-v1/preset-catalog.json"
            ))
            .unwrap();
            assert_eq!(catalog["presets"], expected_presets);
            assert!(
                catalog["known_capability_ids"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("semantic-rules"))
            );
            assert!(
                catalog["known_text_capability_ids"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("opentype-shaping"))
            );
            assert_eq!(
                catalog["known_font_container_ids"],
                serde_json::json!(["collection", "opentype", "truetype", "woff2"])
            );
            assert_eq!(
                catalog["known_font_source_ids"],
                serde_json::json!(["embedded", "system"])
            );
            assert!(
                catalog["known_semantic_target_ids"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("state-label"))
            );
            assert!(
                catalog["known_variant_ids"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("success"))
            );
            let encoded_limit = catalog["resource_limits"]
                .as_array()
                .unwrap()
                .iter()
                .find(|limit| limit["id"] == "max_theme_encoded_bytes")
                .expect("theme encoded input limit");
            assert_eq!(encoded_limit["phase"], "theme_input");
            assert_eq!(encoded_limit["effective_value"], 2 * 1024 * 1024);
            assert_eq!(encoded_limit["hard_cap"], false);
            assert!(encoded_limit.get("overridable").is_none());
            assert!(encoded_limit.get("minimum_value").is_none());

            let effect_graph_limit = catalog["resource_limits"]
                .as_array()
                .unwrap()
                .iter()
                .find(|limit| limit["id"] == "max_effect_graphs")
                .expect("effect graph compile limit");
            assert_eq!(effect_graph_limit["phase"], "effect_compile");
            assert_eq!(effect_graph_limit["effective_value"], 16);
            assert_eq!(effect_graph_limit["hard_cap"], false);

            let empty_again: Value =
                serde_json::from_slice(&theme_catalog_json_for(&empty_contract).unwrap()).unwrap();
            assert_eq!(empty_again, empty);
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn theme_preset_availability_uses_the_effective_compiler_policy() {
        let restrictive_policy = merman::svg::ThemeResourcePolicy::interactive()
            .with_limit(merman::svg::ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
            .expect("valid restrictive theme policy");
        let svg_contract =
            ArtifactContractSpec::new(TargetKey::Native, crate::BindingTransportKey::Rust)
                .with_operations(&[OperationKey::Svg])
                .materialize();
        assert!(
            svg_contract
                .theme_catalog_json_with_resource_policy(&restrictive_policy)
                .is_err()
        );
        let svg_contract =
            ArtifactContractSpec::new(TargetKey::Native, crate::BindingTransportKey::Rust)
                .with_operations(&[OperationKey::Svg])
                .with_metadata(&[MetadataKey::ThemeCatalog])
                .materialize();
        let catalog: Value = serde_json::from_slice(
            &svg_contract
                .theme_catalog_json_with_resource_policy(&restrictive_policy)
                .unwrap(),
        )
        .unwrap();
        let presets = catalog["presets"].as_array().unwrap();

        assert_eq!(presets.len(), 10);
        assert!(presets.iter().all(|preset| preset["available"] == false));
        assert!(presets.iter().all(|preset| {
            preset["availability_reason_ids"]
                == serde_json::json!(["theme-preset.resource-policy-rejected"])
        }));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn theme_catalog_contains_every_semantic_target_id() {
        let svg_contract =
            ArtifactContractSpec::new(TargetKey::Native, crate::BindingTransportKey::Rust)
                .with_operations(&[OperationKey::Svg])
                .materialize();
        let catalog = theme_catalog_for(&svg_contract);
        let expected = sorted_theme_ids(
            merman::svg::ThemeTarget::ALL
                .iter()
                .copied()
                .map(merman::svg::ThemeTarget::id)
                .collect(),
        );

        assert_eq!(catalog.known_semantic_target_ids, expected);
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
        assert_eq!(flowchart.semantic_coverage, Some("partial"));
        assert_eq!(flowchart.primary_projection, "diagrammatic");
        assert_eq!(flowchart.support_level, "partial");
        assert!(flowchart.structured_text_fallback);
        assert_eq!(flowchart.layout_profiles, ["canonical", "compact"]);
        assert_eq!(flowchart.width_profiles, ["unicode", "cjk"]);
        assert_eq!(
            flowchart.encodings,
            ["plain", "ansi16", "ansi256", "truecolor", "html"]
        );
        assert_eq!(flowchart.fallback_encodings, ["plain"]);
        assert!(flowchart.supported_semantics.contains(&"root directions"));
        assert!(flowchart.evidence.iter().any(|evidence| {
            evidence.kind == "local_advantage" && evidence.note.contains("true RL/BT")
        }));

        let class = ascii_capability(&capabilities, "class");
        assert_eq!(class.support_level, "partial");
        assert_eq!(class.primary_projection, "diagrammatic");
        assert!(class.structured_text_fallback);
        assert!(class.limits.iter().any(|limit| limit.contains("namespace")));
        assert!(class.evidence.iter().any(|evidence| {
            evidence.kind == "beautiful_mermaid_prior_art"
                && evidence.source
                    == "crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison"
        }));

        let er = ascii_capability(&capabilities, "er");
        assert_eq!(er.support_level, "partial");
        assert!(er.structured_text_fallback);

        let gantt = ascii_capability(&capabilities, "gantt");
        assert_eq!(gantt.support_level, "summary");
        assert_eq!(gantt.semantic_coverage, Some("partial"));
        assert_eq!(gantt.primary_projection, "structured_text");
        assert!(!gantt.supported_semantics.contains(&"dependencies"));

        let sequence = ascii_capability(&capabilities, "sequence");
        assert_eq!(sequence.layout_profiles, ["canonical", "compact"]);

        let state = ascii_capability(&capabilities, "state");
        assert_eq!(state.layout_profiles, ["canonical"]);

        let zenuml = ascii_capability(&capabilities, "zenuml");
        assert!(zenuml.layout_profiles.is_empty());
        assert!(zenuml.width_profiles.is_empty());
        assert!(zenuml.encodings.is_empty());
        assert!(zenuml.fallback_encodings.is_empty());

        let xychart = ascii_capability(&capabilities, "xychart");
        assert_eq!(xychart.support_level, "partial");
        assert!(xychart.evidence.iter().any(|evidence| {
            evidence.kind == "beautiful_mermaid_prior_art"
                && evidence.source
                    == "crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md#family-comparison"
        }));

        assert_eq!(capabilities.len(), 31);
        let zenuml = ascii_capability(&capabilities, "zenuml");
        assert_eq!(zenuml.semantic_coverage, None);
        assert_eq!(zenuml.primary_projection, "none");
        assert_eq!(zenuml.support_level, "unsupported");

        let json: Value = serde_json::from_slice(&ascii_capabilities_json().unwrap()).unwrap();
        let first = &json.as_array().unwrap()[0];
        assert!(first.get("semantic_coverage").is_some());
        assert!(first.get("primary_projection").is_some());
        assert!(first.get("structured_text_fallback").is_some());
        assert!(first.get("summary_fallback").is_none());
    }

    #[test]
    fn metadata_json_helpers_return_json_contracts() {
        let diagrams: Value = serde_json::from_slice(&supported_diagrams_json().unwrap()).unwrap();
        let ascii_diagrams: Value =
            serde_json::from_slice(&ascii_supported_diagrams_json().unwrap()).unwrap();
        let ascii_capabilities: Value =
            serde_json::from_slice(&ascii_capabilities_json().unwrap()).unwrap();
        let themes: Value = serde_json::from_slice(&supported_themes_json().unwrap()).unwrap();
        let theme_catalog: Value =
            serde_json::from_slice(&theme_catalog_json_for(&full_native_contract()).unwrap())
                .unwrap();
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
            assert_eq!(flowchart["semantic_coverage"], "partial");
            assert_eq!(flowchart["primary_projection"], "diagrammatic");
            assert_eq!(flowchart["support_level"], "partial");
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
        assert_eq!(
            theme_catalog["schema_version"],
            THEME_CATALOG_SCHEMA_VERSION
        );
        assert!(theme_catalog["presets"].is_array());
        assert!(theme_catalog["known_semantic_target_ids"].is_array());
        let flowchart = family_capabilities
            .as_array()
            .unwrap()
            .iter()
            .find(|capability| capability["diagram_type"] == "flowchart")
            .expect("flowchart family capability should be present");
        assert_eq!(flowchart["family_id"], "flowchart");
        assert!(flowchart.get("logical_family_kind").is_none());
        assert!(flowchart.get("render_model_kind").is_none());
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
            assert!(lint_rules.iter().any(|rule| {
                rule["id"] == "merman.compatibility.config.deprecated_flowchart_html_labels"
                    && rule["tags"] == serde_json::json!(["deprecated"])
            }));
            let typed_lint_rules = lint_rule_catalog().unwrap();
            assert!(typed_lint_rules.iter().any(|rule| {
                rule.id == "merman.compatibility.config.deprecated_flowchart_html_labels"
                    && rule.tags == vec!["deprecated"]
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
    fn artifact_metadata_dispatches_every_advertised_catalog() {
        let contract = full_native_contract();
        for key in contract.metadata_keys() {
            let id = key.id();
            let expected = match key {
                MetadataKey::AsciiCapabilities => ascii_capabilities_json(),
                MetadataKey::DiagramFamilyCapabilities => diagram_family_capabilities_json(),
                MetadataKey::LintRuleCatalog => lint_rule_catalog_json(),
                MetadataKey::SupportedDiagrams => supported_diagrams_json(),
                MetadataKey::SupportedThemes => supported_themes_json(),
                MetadataKey::ThemeCatalog => theme_catalog_json_for(&contract),
            };
            match (contract.metadata_json(id), expected) {
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
    fn artifact_metadata_rejects_known_but_unadvertised_catalogs() {
        let contract = semantic_contract();
        let error = contract
            .metadata_json(MetadataKey::SupportedDiagrams.id())
            .unwrap_err();

        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::Generic);
        assert_eq!(error.capability_id(), None);
        assert!(error.message().contains("is not exposed"));
    }

    #[test]
    fn artifact_metadata_rejects_removed_host_theme_catalog() {
        let error = full_native_contract()
            .metadata_json("supported-host-theme-presets")
            .unwrap_err();

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
