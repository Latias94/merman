use crate::capability::CapabilityKey;
use std::collections::BTreeSet;

/// One renderer text-measurement route advertised by a transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum TextMeasurementProviderKey {
    HostCallback,
    Vendored,
}

impl TextMeasurementProviderKey {
    pub const ALL: &'static [Self] = &[Self::HostCallback, Self::Vendored];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::HostCallback => "host-callback",
            Self::Vendored => "vendored",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|key| key.id() == id)
    }

    /// The single authority that makes this provider available to an artifact.
    #[must_use]
    pub const fn source(self) -> TextMeasurementProviderSource {
        match self {
            Self::HostCallback => TextMeasurementProviderSource::ConstructorService(
                ConstructorServiceKey::HostTextMeasurement,
            ),
            Self::Vendored => TextMeasurementProviderSource::SvgPipeline,
        }
    }

    pub(crate) fn is_compiled(self, capabilities: &BTreeSet<CapabilityKey>) -> bool {
        match self.source() {
            TextMeasurementProviderSource::SvgPipeline => {
                capabilities.contains(&CapabilityKey::Svg)
            }
            TextMeasurementProviderSource::ConstructorService(service) => {
                service.spec().is_compiled(capabilities)
            }
        }
    }

    pub(crate) fn is_exposed(
        self,
        uses_svg_pipeline: bool,
        constructor_services: &BTreeSet<ConstructorServiceKey>,
    ) -> bool {
        match self.source() {
            TextMeasurementProviderSource::SvgPipeline => uses_svg_pipeline,
            TextMeasurementProviderSource::ConstructorService(service) => {
                constructor_services.contains(&service)
            }
        }
    }
}

/// The artifact fact that provides one text-measurement route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextMeasurementProviderSource {
    SvgPipeline,
    ConstructorService(ConstructorServiceKey),
}

/// One immutable service accepted while constructing a reusable engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ConstructorServiceKey {
    HostTextMeasurement,
    IconRegistry,
}

/// Descriptor-owned resource catalog attached to a constructor service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ConstructorServiceResourceCatalog {
    IconRegistry,
}

impl ConstructorServiceKey {
    pub const ALL: &'static [Self] = &[Self::HostTextMeasurement, Self::IconRegistry];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::HostTextMeasurement => "host-text-measurement",
            Self::IconRegistry => "icon-registry",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|key| key.id() == id)
    }

    #[must_use]
    pub(crate) const fn spec(self) -> &'static ConstructorServiceSpec {
        match self {
            Self::HostTextMeasurement => &CONSTRUCTOR_SERVICE_SPECS[0],
            Self::IconRegistry => &CONSTRUCTOR_SERVICE_SPECS[1],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConstructorServiceAvailability {
    SvgPipeline,
}

/// Internal availability and resource relation for one immutable constructor-owned service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConstructorServiceSpec {
    availability: ConstructorServiceAvailability,
    resource_catalog: Option<ConstructorServiceResourceCatalog>,
}

impl ConstructorServiceSpec {
    pub(crate) const fn resource_catalog(&self) -> Option<ConstructorServiceResourceCatalog> {
        self.resource_catalog
    }

    pub(crate) fn is_compiled(&self, capabilities: &BTreeSet<CapabilityKey>) -> bool {
        match self.availability {
            ConstructorServiceAvailability::SvgPipeline => {
                capabilities.contains(&CapabilityKey::Svg)
            }
        }
    }

    pub(crate) const fn is_available(&self, uses_svg_pipeline: bool) -> bool {
        match self.availability {
            ConstructorServiceAvailability::SvgPipeline => uses_svg_pipeline,
        }
    }
}

const CONSTRUCTOR_SERVICE_SPECS: &[ConstructorServiceSpec] = &[
    ConstructorServiceSpec {
        availability: ConstructorServiceAvailability::SvgPipeline,
        resource_catalog: None,
    },
    ConstructorServiceSpec {
        availability: ConstructorServiceAvailability::SvgPipeline,
        resource_catalog: Some(ConstructorServiceResourceCatalog::IconRegistry),
    },
];

/// Runtime-policy choices a transport actually accepts through binding options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimePolicyExposure {
    /// Only deterministic policy is accepted. Compiled native adapters remain private.
    #[default]
    DeterministicOnly,
    /// Binding JSON accepts deterministic and, when the complete adapter trio exists, native.
    BindingOptions,
}
