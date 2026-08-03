use crate::capability::CapabilityKey;
use std::collections::BTreeSet;

/// One stable catalog in the binding-owned metadata registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum MetadataKey {
    AsciiCapabilities,
    DiagramFamilyCapabilities,
    LintRuleCatalog,
    SupportedDiagrams,
    SupportedHostThemePresets,
    SupportedThemes,
}

impl MetadataKey {
    pub const ALL: &'static [Self] = &[
        Self::AsciiCapabilities,
        Self::DiagramFamilyCapabilities,
        Self::LintRuleCatalog,
        Self::SupportedDiagrams,
        Self::SupportedHostThemePresets,
        Self::SupportedThemes,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        self.spec().id
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|key| key.id() == id)
    }

    #[must_use]
    pub const fn spec(self) -> &'static MetadataSpec {
        match self {
            Self::AsciiCapabilities => &METADATA_SPECS[0],
            Self::DiagramFamilyCapabilities => &METADATA_SPECS[1],
            Self::LintRuleCatalog => &METADATA_SPECS[2],
            Self::SupportedDiagrams => &METADATA_SPECS[3],
            Self::SupportedHostThemePresets => &METADATA_SPECS[4],
            Self::SupportedThemes => &METADATA_SPECS[5],
        }
    }
}

/// Authoritative capability requirement and handler identity for one metadata catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct MetadataSpec {
    key: MetadataKey,
    id: &'static str,
    required_capability: Option<CapabilityKey>,
    handler: MetadataHandlerKey,
}

impl MetadataSpec {
    #[must_use]
    pub const fn key(&self) -> MetadataKey {
        self.key
    }

    #[must_use]
    pub const fn id(&self) -> &'static str {
        self.id
    }

    #[must_use]
    pub const fn required_capability(&self) -> Option<CapabilityKey> {
        self.required_capability
    }

    pub(crate) fn is_available(&self, capabilities: &BTreeSet<CapabilityKey>) -> bool {
        self.required_capability
            .is_none_or(|capability| capabilities.contains(&capability))
    }

    pub(crate) const fn handler(&self) -> MetadataHandlerKey {
        self.handler
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataHandlerKey {
    AsciiCapabilities,
    DiagramFamilyCapabilities,
    LintRuleCatalog,
    SupportedDiagrams,
    SupportedHostThemePresets,
    SupportedThemes,
}

const METADATA_SPECS: &[MetadataSpec] = &[
    MetadataSpec {
        key: MetadataKey::AsciiCapabilities,
        id: "ascii-capabilities",
        required_capability: Some(CapabilityKey::Ascii),
        handler: MetadataHandlerKey::AsciiCapabilities,
    },
    MetadataSpec {
        key: MetadataKey::DiagramFamilyCapabilities,
        id: "diagram-family-capabilities",
        required_capability: None,
        handler: MetadataHandlerKey::DiagramFamilyCapabilities,
    },
    MetadataSpec {
        key: MetadataKey::LintRuleCatalog,
        id: "lint-rule-catalog",
        required_capability: Some(CapabilityKey::Analysis),
        handler: MetadataHandlerKey::LintRuleCatalog,
    },
    MetadataSpec {
        key: MetadataKey::SupportedDiagrams,
        id: "supported-diagrams",
        required_capability: None,
        handler: MetadataHandlerKey::SupportedDiagrams,
    },
    MetadataSpec {
        key: MetadataKey::SupportedHostThemePresets,
        id: "supported-host-theme-presets",
        required_capability: Some(CapabilityKey::Svg),
        handler: MetadataHandlerKey::SupportedHostThemePresets,
    },
    MetadataSpec {
        key: MetadataKey::SupportedThemes,
        id: "supported-themes",
        required_capability: None,
        handler: MetadataHandlerKey::SupportedThemes,
    },
];

pub(crate) const fn metadata_spec(key: MetadataKey) -> &'static MetadataSpec {
    key.spec()
}
