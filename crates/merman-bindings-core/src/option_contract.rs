use crate::capability::{CapabilityKey, compiled_capability_keys};

/// Descriptor-backed availability rule for one top-level binding option group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct BindingOptionGroupSpec {
    key: BindingOptionGroupKey,
    always_available: bool,
    any_capabilities: &'static [CapabilityKey],
    requires_svg_pipeline: bool,
}

impl BindingOptionGroupSpec {
    #[must_use]
    pub const fn key(&self) -> BindingOptionGroupKey {
        self.key
    }

    /// Whether every artifact accepts this schema-owned top-level field.
    #[must_use]
    pub const fn always_available(&self) -> bool {
        self.always_available
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

/// One canonical top-level options field or object accepted by the binding JSON contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum BindingOptionGroupKey {
    Ascii,
    Environment,
    FixedLocalOffsetMinutes,
    FixedToday,
    Jpeg,
    Layout,
    Lint,
    Parse,
    Pdf,
    Presentation,
    Raster,
    Resources,
    RuntimePolicy,
    SiteConfig,
    Svg,
    Version,
}

impl BindingOptionGroupKey {
    pub const ALL: &'static [Self] = &[
        Self::Ascii,
        Self::Environment,
        Self::FixedLocalOffsetMinutes,
        Self::FixedToday,
        Self::Jpeg,
        Self::Layout,
        Self::Lint,
        Self::Parse,
        Self::Pdf,
        Self::Presentation,
        Self::Raster,
        Self::Resources,
        Self::RuntimePolicy,
        Self::SiteConfig,
        Self::Svg,
        Self::Version,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ascii => "ascii",
            Self::Environment => "environment",
            Self::FixedLocalOffsetMinutes => "fixed_local_offset_minutes",
            Self::FixedToday => "fixed_today",
            Self::Jpeg => "jpeg",
            Self::Layout => "layout",
            Self::Lint => "lint",
            Self::Parse => "parse",
            Self::Pdf => "pdf",
            Self::Presentation => "presentation",
            Self::Raster => "raster",
            Self::Resources => "resources",
            Self::RuntimePolicy => "runtime_policy",
            Self::SiteConfig => "site_config",
            Self::Svg => "svg",
            Self::Version => "version",
        }
    }

    #[must_use]
    pub const fn spec(self) -> &'static BindingOptionGroupSpec {
        match self {
            Self::Ascii => &OPTION_GROUP_SPECS[0],
            Self::Environment => &OPTION_GROUP_SPECS[1],
            Self::FixedLocalOffsetMinutes => &OPTION_GROUP_SPECS[2],
            Self::FixedToday => &OPTION_GROUP_SPECS[3],
            Self::Jpeg => &OPTION_GROUP_SPECS[4],
            Self::Layout => &OPTION_GROUP_SPECS[5],
            Self::Lint => &OPTION_GROUP_SPECS[6],
            Self::Parse => &OPTION_GROUP_SPECS[7],
            Self::Pdf => &OPTION_GROUP_SPECS[8],
            Self::Presentation => &OPTION_GROUP_SPECS[9],
            Self::Raster => &OPTION_GROUP_SPECS[10],
            Self::Resources => &OPTION_GROUP_SPECS[11],
            Self::RuntimePolicy => &OPTION_GROUP_SPECS[12],
            Self::SiteConfig => &OPTION_GROUP_SPECS[13],
            Self::Svg => &OPTION_GROUP_SPECS[14],
            Self::Version => &OPTION_GROUP_SPECS[15],
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
        spec.always_available
            || (spec.requires_svg_pipeline && compiled.contains(CapabilityKey::Svg))
            || spec
                .any_capabilities
                .iter()
                .any(|capability| compiled.contains(*capability))
    }
}

const OPTION_GROUP_SPECS: &[BindingOptionGroupSpec] = &[
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Ascii,
        always_available: false,
        any_capabilities: &[CapabilityKey::Ascii],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Environment,
        always_available: false,
        any_capabilities: &[],
        requires_svg_pipeline: true,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::FixedLocalOffsetMinutes,
        always_available: true,
        any_capabilities: &[],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::FixedToday,
        always_available: true,
        any_capabilities: &[],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Jpeg,
        always_available: false,
        any_capabilities: &[CapabilityKey::Jpeg],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Layout,
        always_available: false,
        any_capabilities: &[],
        requires_svg_pipeline: true,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Lint,
        always_available: false,
        any_capabilities: &[CapabilityKey::Analysis],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Parse,
        always_available: true,
        any_capabilities: &[],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Pdf,
        always_available: false,
        any_capabilities: &[CapabilityKey::Pdf],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Presentation,
        always_available: false,
        any_capabilities: &[],
        requires_svg_pipeline: true,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Raster,
        always_available: false,
        any_capabilities: &[CapabilityKey::Jpeg, CapabilityKey::Png],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Resources,
        always_available: true,
        any_capabilities: &[],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::RuntimePolicy,
        always_available: true,
        any_capabilities: &[],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::SiteConfig,
        always_available: true,
        any_capabilities: &[],
        requires_svg_pipeline: false,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Svg,
        always_available: false,
        any_capabilities: &[],
        requires_svg_pipeline: true,
    },
    BindingOptionGroupSpec {
        key: BindingOptionGroupKey::Version,
        always_available: true,
        any_capabilities: &[],
        requires_svg_pipeline: false,
    },
];
