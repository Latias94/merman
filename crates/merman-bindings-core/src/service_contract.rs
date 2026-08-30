/// One renderer text-measurement route advertised by a transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum TextMeasurementProviderKey {
    Deterministic,
    HostCallback,
}

impl TextMeasurementProviderKey {
    pub const ALL: &'static [Self] = &[Self::Deterministic, Self::HostCallback];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::HostCallback => "host-callback",
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
            Self::Deterministic => TextMeasurementProviderSource::SvgPipeline,
            Self::HostCallback => TextMeasurementProviderSource::ConstructorService(
                ConstructorServiceKey::HostTextMeasurement,
            ),
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

impl TextMeasurementProviderSource {
    /// Stable machine-readable provider-source identity used by generated SDK contracts.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::SvgPipeline => "svg-pipeline",
            Self::ConstructorService(_) => "constructor-service",
        }
    }

    /// Constructor service that owns this provider, when provider availability is service-owned.
    #[must_use]
    pub const fn constructor_service(self) -> Option<ConstructorServiceKey> {
        match self {
            Self::SvgPipeline => None,
            Self::ConstructorService(service) => Some(service),
        }
    }
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

    /// Whether this service can only be accepted by an artifact with an SVG pipeline.
    #[must_use]
    pub const fn requires_svg_pipeline(self) -> bool {
        match self.spec().availability {
            ConstructorServiceAvailability::SvgPipeline => true,
        }
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
