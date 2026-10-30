//! Shared resource-profile identity and trust ordering.

pub const RESOURCE_PROFILE_COUNT: usize = 4;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceProfile {
    Interactive,
    Constrained,
    TrustedNative,
    UnboundedForTrustedInput,
}

impl ResourceProfile {
    pub const ALL: [Self; RESOURCE_PROFILE_COUNT] = [
        Self::Interactive,
        Self::Constrained,
        Self::TrustedNative,
        Self::UnboundedForTrustedInput,
    ];

    pub const fn descriptor(self) -> &'static ResourceProfileDescriptor {
        &RESOURCE_PROFILE_DESCRIPTORS[self as usize]
    }

    pub const fn id(self) -> &'static str {
        self.descriptor().id
    }

    pub fn from_id(id: &str) -> Option<Self> {
        RESOURCE_PROFILE_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.id == id)
            .map(|descriptor| descriptor.profile)
    }

    /// Returns whether this profile is no looser than a host-owned ceiling.
    ///
    /// Numeric overrides cover only the resource IDs exposed by a concrete artifact. This
    /// relation also protects resource owners that are not part of that generic vocabulary, such
    /// as theme compilation.
    pub const fn is_no_looser_than(self, ceiling: Self) -> bool {
        match ceiling {
            Self::Constrained => matches!(self, Self::Constrained),
            Self::Interactive => matches!(self, Self::Constrained | Self::Interactive),
            Self::TrustedNative => !matches!(self, Self::UnboundedForTrustedInput),
            Self::UnboundedForTrustedInput => true,
        }
    }
}

impl std::fmt::Display for ResourceProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

impl std::str::FromStr for ResourceProfile {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_id(value).ok_or_else(|| {
            let supported = RESOURCE_PROFILE_DESCRIPTORS
                .iter()
                .map(|descriptor| descriptor.id)
                .collect::<Vec<_>>()
                .join(", ");
            format!("unsupported resource profile `{value}`; expected one of: {supported}")
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResourceProfileDescriptor {
    pub profile: ResourceProfile,
    pub id: &'static str,
    pub purpose: &'static str,
    pub trust_assumption: &'static str,
    pub recommended_binding_default: bool,
}

pub static RESOURCE_PROFILE_DESCRIPTORS: [ResourceProfileDescriptor; RESOURCE_PROFILE_COUNT] = [
    ResourceProfileDescriptor {
        profile: ResourceProfile::Interactive,
        id: "interactive",
        purpose: "General interactive applications and public binding surfaces",
        trust_assumption: "Cooperative user-authored input; not a hostile or multi-tenant isolation boundary",
        recommended_binding_default: true,
    },
    ResourceProfileDescriptor {
        profile: ResourceProfile::Constrained,
        id: "constrained",
        purpose: "Constrained rendering for untrusted or publicly submitted documents",
        trust_assumption: "The host must provide timeout, memory, concurrency, and preemption controls",
        recommended_binding_default: false,
    },
    ResourceProfileDescriptor {
        profile: ResourceProfile::TrustedNative,
        id: "trusted-native",
        purpose: "Local CLI and controlled native batch rendering",
        trust_assumption: "Input is trusted and the native host controls the workload",
        recommended_binding_default: false,
    },
    ResourceProfileDescriptor {
        profile: ResourceProfile::UnboundedForTrustedInput,
        id: "unbounded-for-trusted-input",
        purpose: "Explicitly disable policy budgets while retaining hard backend capabilities",
        trust_assumption: "Input is fully trusted and the host provides outer isolation",
        recommended_binding_default: false,
    },
];
