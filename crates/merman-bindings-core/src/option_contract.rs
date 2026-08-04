use crate::capability::{CapabilityKey, compiled_capability_keys};

/// Descriptor-backed availability rule for one top-level binding option group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct BindingOptionGroupSpec {
    key: BindingOptionGroupKey,
    any_capabilities: &'static [CapabilityKey],
    requires_svg_pipeline: bool,
}

impl BindingOptionGroupSpec {
    #[must_use]
    pub const fn key(&self) -> BindingOptionGroupKey {
        self.key
    }

    /// Capabilities where any one makes this option group available.
    #[must_use]
    pub const fn any_capabilities(&self) -> &'static [CapabilityKey] {
        self.any_capabilities
    }

    #[must_use]
    pub const fn requires_svg_pipeline(&self) -> bool {
        self.requires_svg_pipeline
    }
}

/// One capability-gated top-level options field or object accepted by the binding JSON contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum BindingOptionGroupKey {
    Ascii,
    Environment,
    Jpeg,
    Layout,
    Lint,
    Pdf,
    Presentation,
    Raster,
    Svg,
}

impl BindingOptionGroupKey {
    pub const ALL: &'static [Self] = &[
        Self::Ascii,
        Self::Environment,
        Self::Jpeg,
        Self::Layout,
        Self::Lint,
        Self::Pdf,
        Self::Presentation,
        Self::Raster,
        Self::Svg,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ascii => "ascii",
            Self::Environment => "environment",
            Self::Jpeg => "jpeg",
            Self::Layout => "layout",
            Self::Lint => "lint",
            Self::Pdf => "pdf",
            Self::Presentation => "presentation",
            Self::Raster => "raster",
            Self::Svg => "svg",
        }
    }

    #[must_use]
    pub const fn spec(self) -> &'static BindingOptionGroupSpec {
        match self {
            Self::Ascii => &OPTION_GROUP_SPECS[0],
            Self::Environment => &OPTION_GROUP_SPECS[1],
            Self::Jpeg => &OPTION_GROUP_SPECS[2],
            Self::Layout => &OPTION_GROUP_SPECS[3],
            Self::Lint => &OPTION_GROUP_SPECS[4],
            Self::Pdf => &OPTION_GROUP_SPECS[5],
            Self::Presentation => &OPTION_GROUP_SPECS[6],
            Self::Raster => &OPTION_GROUP_SPECS[7],
            Self::Svg => &OPTION_GROUP_SPECS[8],
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|key| key.id() == id)
    }

    #[must_use]
    pub fn is_compiled(self) -> bool {
        let compiled = compiled_capability_keys();
        let spec = self.spec();
        (spec.requires_svg_pipeline && compiled.contains(CapabilityKey::Svg))
            || spec
                .any_capabilities
                .iter()
                .any(|capability| compiled.contains(*capability))
    }
}

const OPTION_GROUP_SPECS: &[BindingOptionGroupSpec] = &[
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Ascii,
        any_capabilities: &[CapabilityKey::Ascii],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Environment,
        any_capabilities: &[],
        requires_svg_pipeline: true,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Jpeg,
        any_capabilities: &[CapabilityKey::Jpeg],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Layout,
        any_capabilities: &[],
        requires_svg_pipeline: true,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Lint,
        any_capabilities: &[CapabilityKey::Analysis],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Pdf,
        any_capabilities: &[CapabilityKey::Pdf],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Presentation,
        any_capabilities: &[],
        requires_svg_pipeline: true,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Raster,
        any_capabilities: &[CapabilityKey::Jpeg, CapabilityKey::Png],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Svg,
        any_capabilities: &[],
        requires_svg_pipeline: true,
    },
];
