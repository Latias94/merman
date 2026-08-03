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

    #[must_use]
    pub const fn spec(self) -> &'static TextMeasurementProviderSpec {
        match self {
            Self::HostCallback => &TEXT_MEASUREMENT_PROVIDER_SPECS[0],
            Self::Vendored => &TEXT_MEASUREMENT_PROVIDER_SPECS[1],
        }
    }
}

/// Availability and selection rules for one text-measurement provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TextMeasurementProviderSpec {
    key: TextMeasurementProviderKey,
    requires_svg_pipeline: bool,
    derived_from_svg_pipeline: bool,
}

impl TextMeasurementProviderSpec {
    #[must_use]
    pub const fn key(&self) -> TextMeasurementProviderKey {
        self.key
    }

    #[must_use]
    pub const fn requires_svg_pipeline(&self) -> bool {
        self.requires_svg_pipeline
    }

    /// Whether artifact validation selects this provider automatically for SVG-backed operations.
    #[must_use]
    pub const fn derived_from_svg_pipeline(&self) -> bool {
        self.derived_from_svg_pipeline
    }

    pub(crate) fn is_compiled(&self, capabilities: &BTreeSet<CapabilityKey>) -> bool {
        !self.requires_svg_pipeline || capabilities.contains(&CapabilityKey::Svg)
    }
}

const TEXT_MEASUREMENT_PROVIDER_SPECS: &[TextMeasurementProviderSpec] = &[
    TextMeasurementProviderSpec {
        key: TextMeasurementProviderKey::HostCallback,
        requires_svg_pipeline: true,
        derived_from_svg_pipeline: false,
    },
    TextMeasurementProviderSpec {
        key: TextMeasurementProviderKey::Vendored,
        requires_svg_pipeline: true,
        derived_from_svg_pipeline: true,
    },
];

/// One immutable service accepted while constructing a reusable engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ConstructorServiceKey {
    HostTextMeasurement,
}

impl ConstructorServiceKey {
    pub const ALL: &'static [Self] = &[Self::HostTextMeasurement];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::HostTextMeasurement => "host-text-measurement",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|key| key.id() == id)
    }

    #[must_use]
    pub const fn spec(self) -> &'static ConstructorServiceSpec {
        match self {
            Self::HostTextMeasurement => &CONSTRUCTOR_SERVICE_SPECS[0],
        }
    }
}

/// Availability relation for one immutable constructor-owned service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConstructorServiceSpec {
    key: ConstructorServiceKey,
    required_providers: &'static [TextMeasurementProviderKey],
    requires_svg_pipeline: bool,
}

impl ConstructorServiceSpec {
    #[must_use]
    pub const fn key(&self) -> ConstructorServiceKey {
        self.key
    }

    #[must_use]
    pub const fn required_providers(&self) -> &'static [TextMeasurementProviderKey] {
        self.required_providers
    }

    #[must_use]
    pub const fn requires_svg_pipeline(&self) -> bool {
        self.requires_svg_pipeline
    }

    pub(crate) fn is_compiled(&self, capabilities: &BTreeSet<CapabilityKey>) -> bool {
        !self.requires_svg_pipeline || capabilities.contains(&CapabilityKey::Svg)
    }
}

const CONSTRUCTOR_SERVICE_SPECS: &[ConstructorServiceSpec] = &[ConstructorServiceSpec {
    key: ConstructorServiceKey::HostTextMeasurement,
    required_providers: &[TextMeasurementProviderKey::HostCallback],
    requires_svg_pipeline: true,
}];

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
