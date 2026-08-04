use crate::capability::CapabilityKey;

/// One stable catalog in the binding-owned metadata registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
#[repr(u8)]
pub enum MetadataKey {
    AsciiCapabilities,
    DiagramFamilyCapabilities,
    LintRuleCatalog,
    PresentationCatalog,
    SupportedDiagrams,
    SupportedThemes,
}

impl MetadataKey {
    pub const ALL: &'static [Self] = &[
        Self::AsciiCapabilities,
        Self::DiagramFamilyCapabilities,
        Self::LintRuleCatalog,
        Self::PresentationCatalog,
        Self::SupportedDiagrams,
        Self::SupportedThemes,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        self.spec().id
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        METADATA_SPECS
            .iter()
            .find(|spec| spec.id == id)
            .map(|spec| spec.key)
    }

    #[must_use]
    pub const fn spec(self) -> &'static MetadataSpec {
        match self {
            Self::AsciiCapabilities => &METADATA_SPECS[0],
            Self::DiagramFamilyCapabilities => &METADATA_SPECS[1],
            Self::LintRuleCatalog => &METADATA_SPECS[2],
            Self::PresentationCatalog => &METADATA_SPECS[3],
            Self::SupportedDiagrams => &METADATA_SPECS[4],
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

    pub(crate) const fn handler(&self) -> MetadataHandlerKey {
        self.handler
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataHandlerKey {
    AsciiCapabilities,
    DiagramFamilyCapabilities,
    LintRuleCatalog,
    PresentationCatalog,
    SupportedDiagrams,
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
        key: MetadataKey::PresentationCatalog,
        id: "presentation-catalog",
        required_capability: None,
        handler: MetadataHandlerKey::PresentationCatalog,
    },
    MetadataSpec {
        key: MetadataKey::SupportedDiagrams,
        id: "supported-diagrams",
        required_capability: None,
        handler: MetadataHandlerKey::SupportedDiagrams,
    },
    MetadataSpec {
        key: MetadataKey::SupportedThemes,
        id: "supported-themes",
        required_capability: None,
        handler: MetadataHandlerKey::SupportedThemes,
    },
];

const _: () = {
    assert!(METADATA_SPECS.len() == MetadataKey::ALL.len());
    let mut index = 0;
    while index < METADATA_SPECS.len() {
        assert!(METADATA_SPECS[index].key as usize == index);
        index += 1;
    }
};

pub(crate) const fn metadata_spec(key: MetadataKey) -> &'static MetadataSpec {
    key.spec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_keys_specs_and_ids_are_bijective() {
        for (index, key) in MetadataKey::ALL.iter().copied().enumerate() {
            let spec = key.spec();
            assert_eq!(key as usize, index);
            assert_eq!(spec.key(), key);
            assert_eq!(MetadataKey::from_id(spec.id()), Some(key));
        }

        assert_eq!(MetadataKey::from_id("future-metadata-catalog"), None);
    }
}
