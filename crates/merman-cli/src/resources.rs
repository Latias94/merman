#[cfg(feature = "ascii")]
use merman::ascii::{AsciiResourceLimitId, AsciiResourcePolicy};
#[cfg(not(feature = "svg"))]
use merman::resources::{InputResourceLimitId, InputResourceLimitOverrideError};
use merman::resources::{InputResourcePolicy, ResourceProfile};
#[cfg(feature = "svg")]
use merman::svg::{
    IconRegistryResourceLimitId, RenderResourcePolicy, ResourceLimitId as RenderResourceLimitId,
    ResourceLimitOverrideError as RenderResourceLimitOverrideError,
};
use std::num::NonZeroUsize;
use std::time::Duration;

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

#[cfg(feature = "svg")]
pub(crate) const HARD_MAX_ICON_PACKS: u64 = IconRegistryResourceLimitId::MaxPacks.fixed_value();
#[cfg(not(feature = "svg"))]
pub(crate) const HARD_MAX_ICON_PACKS: u64 = 16;
#[cfg(feature = "svg")]
const HARD_MAX_ICON_PACK_BYTES: u64 = IconRegistryResourceLimitId::MaxPackBytes.fixed_value();
#[cfg(not(feature = "svg"))]
const HARD_MAX_ICON_PACK_BYTES: u64 = 16 * MIB;
#[cfg(feature = "svg")]
const HARD_MAX_AGGREGATE_ICON_BYTES: u64 = IconRegistryResourceLimitId::MaxInputBytes.fixed_value();
#[cfg(not(feature = "svg"))]
const HARD_MAX_AGGREGATE_ICON_BYTES: u64 = 32 * MIB;
pub(crate) const HARD_MAX_JOBS: u64 = 64;
pub(crate) const HARD_MAX_REDIRECTS: u64 = 20;
pub(crate) const HARD_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const HARD_PER_HOP_TIMEOUT: Duration = Duration::from_secs(300);
pub(crate) const HARD_WORKFLOW_TIMEOUT: Duration = Duration::from_secs(900);

const CLI_RESOURCE_LIMIT_COUNT: usize = 17;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CliResourceLimitId {
    MaxMarkdownDocumentBytes,
    MaxConfigBytes,
    MaxRustdocInputBytes,
    MaxCssBytes,
    MaxPuppeteerConfigBytes,
    MaxLocalIconBodyBytes,
    MaxRemoteIconBodyBytes,
    MaxAggregateIconBytes,
    MaxIconPacks,
    MaxMarkdownCharts,
    MaxStagedBytes,
    MaxSchedulingWeightBytes,
    MaxJobs,
    MaxRedirects,
    ConnectTimeoutSeconds,
    PerHopTimeoutSeconds,
    WorkflowTimeoutSeconds,
}

impl CliResourceLimitId {
    #[cfg(test)]
    pub(crate) const ALL: [Self; CLI_RESOURCE_LIMIT_COUNT] = [
        Self::MaxMarkdownDocumentBytes,
        Self::MaxConfigBytes,
        Self::MaxRustdocInputBytes,
        Self::MaxCssBytes,
        Self::MaxPuppeteerConfigBytes,
        Self::MaxLocalIconBodyBytes,
        Self::MaxRemoteIconBodyBytes,
        Self::MaxAggregateIconBytes,
        Self::MaxIconPacks,
        Self::MaxMarkdownCharts,
        Self::MaxStagedBytes,
        Self::MaxSchedulingWeightBytes,
        Self::MaxJobs,
        Self::MaxRedirects,
        Self::ConnectTimeoutSeconds,
        Self::PerHopTimeoutSeconds,
        Self::WorkflowTimeoutSeconds,
    ];

    const fn index(self) -> usize {
        self as usize
    }

    pub(crate) fn from_stable_id(stable_id: &str) -> Option<Self> {
        CLI_RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.stable_id == stable_id)
            .map(|descriptor| descriptor.id)
    }

    pub(crate) const fn descriptor(self) -> &'static CliResourceLimitDescriptor {
        &CLI_RESOURCE_LIMIT_DESCRIPTORS[self.index()]
    }

    pub(crate) const fn as_str(self) -> &'static str {
        self.descriptor().stable_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliResourceUnit {
    AcquisitionBytes,
    Bytes,
    Count,
    SchedulingWeightBytes,
    Seconds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CliResourceLimitDescriptor {
    pub(crate) id: CliResourceLimitId,
    pub(crate) stable_id: &'static str,
    pub(crate) unit: CliResourceUnit,
    pub(crate) description: &'static str,
    pub(crate) hard_cap: Option<u64>,
}

macro_rules! cli_limit_descriptors {
    ($($id:ident => ($stable:literal, $unit:ident, $description:literal, $hard_cap:expr)),+ $(,)?) => {
        pub(crate) const CLI_RESOURCE_LIMIT_DESCRIPTORS:
            [CliResourceLimitDescriptor; CLI_RESOURCE_LIMIT_COUNT] = [
                $(CliResourceLimitDescriptor {
                    id: CliResourceLimitId::$id,
                    stable_id: $stable,
                    unit: CliResourceUnit::$unit,
                    description: $description,
                    hard_cap: $hard_cap,
                }),+
            ];
    };
}

cli_limit_descriptors! {
    MaxMarkdownDocumentBytes => (
        "max_markdown_document_bytes",
        AcquisitionBytes,
        "Maximum UTF-8 bytes in one Markdown document",
        None
    ),
    MaxConfigBytes => (
        "max_config_bytes",
        AcquisitionBytes,
        "Maximum UTF-8 bytes in one Mermaid configuration file",
        None
    ),
    MaxRustdocInputBytes => (
        "max_rustdoc_input_bytes",
        Bytes,
        "Maximum aggregate UTF-8 bytes acquired from unique Rustdoc sources and includes",
        None
    ),
    MaxCssBytes => (
        "max_css_bytes",
        AcquisitionBytes,
        "Maximum UTF-8 bytes in one stylesheet",
        None
    ),
    MaxPuppeteerConfigBytes => (
        "max_puppeteer_config_bytes",
        AcquisitionBytes,
        "Maximum UTF-8 bytes in one mmdc Puppeteer compatibility file",
        None
    ),
    MaxLocalIconBodyBytes => (
        "max_local_icon_body_bytes",
        AcquisitionBytes,
        "Maximum bytes in one local icon-pack body",
        Some(HARD_MAX_ICON_PACK_BYTES)
    ),
    MaxRemoteIconBodyBytes => (
        "max_remote_icon_body_bytes",
        AcquisitionBytes,
        "Maximum bytes in one remote icon-pack response body",
        Some(HARD_MAX_ICON_PACK_BYTES)
    ),
    MaxAggregateIconBytes => (
        "max_aggregate_icon_bytes",
        AcquisitionBytes,
        "Maximum aggregate bytes acquired for icon packs",
        Some(HARD_MAX_AGGREGATE_ICON_BYTES)
    ),
    MaxIconPacks => (
        "max_icon_packs",
        Count,
        "Maximum icon packs acquired by one invocation",
        Some(HARD_MAX_ICON_PACKS)
    ),
    MaxMarkdownCharts => (
        "max_markdown_charts",
        Count,
        "Maximum eligible Mermaid charts in one Markdown document",
        None
    ),
    MaxStagedBytes => (
        "max_staged_bytes",
        Bytes,
        "Maximum aggregate bytes held for staged publication",
        None
    ),
    MaxSchedulingWeightBytes => (
        "max_scheduling_weight_bytes",
        SchedulingWeightBytes,
        "Maximum aggregate conservative backend scheduling weight",
        None
    ),
    MaxJobs => (
        "max_jobs",
        Count,
        "Maximum concurrent Markdown render jobs",
        Some(HARD_MAX_JOBS)
    ),
    MaxRedirects => (
        "max_redirects",
        Count,
        "Maximum HTTP redirects followed for one icon request",
        Some(HARD_MAX_REDIRECTS)
    ),
    ConnectTimeoutSeconds => (
        "connect_timeout_seconds",
        Seconds,
        "Maximum duration allowed to establish one network connection",
        Some(HARD_CONNECT_TIMEOUT.as_secs())
    ),
    PerHopTimeoutSeconds => (
        "per_hop_timeout_seconds",
        Seconds,
        "Maximum duration allowed for one HTTP redirect hop",
        Some(HARD_PER_HOP_TIMEOUT.as_secs())
    ),
    WorkflowTimeoutSeconds => (
        "workflow_timeout_seconds",
        Seconds,
        "Maximum duration allowed for one network acquisition workflow",
        Some(HARD_WORKFLOW_TIMEOUT.as_secs())
    ),
}

// Profile order follows ResourceProfile's stable repr:
// interactive, constrained, trusted-native, unbounded-for-trusted-input.
//
// CSS, Puppeteer compatibility, staging, and scheduling values are initial engineering ceilings.
// Icon acquisition values are capped by the calibrated renderer-owned constructor capabilities.
const CLI_PROFILE_VALUES: [[Option<u64>; 4]; CLI_RESOURCE_LIMIT_COUNT] = [
    [Some(8 * MIB), Some(4 * MIB), Some(64 * MIB), None],
    [Some(512 * KIB), Some(256 * KIB), Some(4 * MIB), None],
    [Some(64 * MIB), Some(32 * MIB), Some(256 * MIB), None],
    [Some(MIB), Some(512 * KIB), Some(8 * MIB), None],
    [Some(512 * KIB), Some(256 * KIB), Some(2 * MIB), None],
    [
        Some(16 * MIB),
        Some(8 * MIB),
        Some(HARD_MAX_ICON_PACK_BYTES),
        Some(HARD_MAX_ICON_PACK_BYTES),
    ],
    [
        Some(16 * MIB),
        Some(8 * MIB),
        Some(HARD_MAX_ICON_PACK_BYTES),
        Some(HARD_MAX_ICON_PACK_BYTES),
    ],
    [
        Some(32 * MIB),
        Some(16 * MIB),
        Some(HARD_MAX_AGGREGATE_ICON_BYTES),
        Some(HARD_MAX_AGGREGATE_ICON_BYTES),
    ],
    [
        Some(16),
        Some(8),
        Some(HARD_MAX_ICON_PACKS),
        Some(HARD_MAX_ICON_PACKS),
    ],
    [Some(1_024), Some(256), Some(8_192), None],
    [Some(GIB), Some(512 * MIB), Some(8 * GIB), None],
    [Some(640 * MIB), Some(576 * MIB), Some(2 * GIB), None],
    [Some(4), Some(2), Some(32), Some(HARD_MAX_JOBS)],
    [Some(5), Some(3), Some(10), Some(HARD_MAX_REDIRECTS)],
    [
        Some(5),
        Some(5),
        Some(10),
        Some(HARD_CONNECT_TIMEOUT.as_secs()),
    ],
    [
        Some(30),
        Some(15),
        Some(60),
        Some(HARD_PER_HOP_TIMEOUT.as_secs()),
    ],
    [
        Some(60),
        Some(30),
        Some(300),
        Some(HARD_WORKFLOW_TIMEOUT.as_secs()),
    ],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CliAdjunctResourcePolicy {
    base_values: [Option<u64>; CLI_RESOURCE_LIMIT_COUNT],
    effective_values: [Option<u64>; CLI_RESOURCE_LIMIT_COUNT],
    explicit_overrides: [Option<u64>; CLI_RESOURCE_LIMIT_COUNT],
}

impl CliAdjunctResourcePolicy {
    const fn for_profile(profile: ResourceProfile) -> Self {
        let mut values = [None; CLI_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < CLI_RESOURCE_LIMIT_COUNT {
            values[index] = CLI_PROFILE_VALUES[index][profile_index(profile)];
            index += 1;
        }
        Self {
            base_values: values,
            effective_values: values,
            explicit_overrides: [None; CLI_RESOURCE_LIMIT_COUNT],
        }
    }

    const fn value(self, id: CliResourceLimitId) -> Option<u64> {
        self.effective_values[id.index()]
    }

    #[cfg(test)]
    const fn base_value(self, id: CliResourceLimitId) -> Option<u64> {
        self.base_values[id.index()]
    }

    #[cfg(test)]
    const fn explicit_override(self, id: CliResourceLimitId) -> Option<u64> {
        self.explicit_overrides[id.index()]
    }

    fn apply_limit(
        &mut self,
        id: CliResourceLimitId,
        value: u64,
    ) -> Result<(), ResourcePolicyOverrideError> {
        let descriptor = id.descriptor();
        if value == 0 {
            return Err(ResourcePolicyOverrideError::NonPositive(
                descriptor.stable_id,
            ));
        }
        if let Some(max) = descriptor.hard_cap
            && value > max
        {
            return Err(ResourcePolicyOverrideError::HardCap {
                limit: descriptor.stable_id,
                requested: value,
                max,
            });
        }
        if descriptor.unit == CliResourceUnit::AcquisitionBytes && usize::try_from(value).is_err() {
            return Err(ResourcePolicyOverrideError::ValueOutOfRange {
                limit: descriptor.stable_id,
                requested: value,
            });
        }
        self.effective_values[id.index()] = Some(value);
        self.explicit_overrides[id.index()] = Some(value);
        Ok(())
    }
}

const fn profile_index(profile: ResourceProfile) -> usize {
    match profile {
        ResourceProfile::Interactive => 0,
        ResourceProfile::Constrained => 1,
        ResourceProfile::TrustedNative => 2,
        ResourceProfile::UnboundedForTrustedInput => 3,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuxiliaryFileLimits {
    pub(crate) markdown_document_bytes: Option<usize>,
    pub(crate) config_bytes: Option<usize>,
    pub(crate) css_bytes: Option<usize>,
    pub(crate) puppeteer_config_bytes: Option<usize>,
}

#[cfg(any(test, feature = "icons"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IconResourceLimits {
    pub(crate) local_body_bytes: Option<usize>,
    #[cfg(feature = "network-icons")]
    pub(crate) remote_body_bytes: Option<usize>,
    #[cfg(test)]
    pub(crate) aggregate_bytes: Option<usize>,
    #[cfg(test)]
    pub(crate) pack_count: Option<u64>,
}

#[cfg(any(test, feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BatchResourceLimits {
    #[cfg(test)]
    pub(crate) markdown_charts: Option<u64>,
    #[cfg(test)]
    pub(crate) staged_bytes: Option<u64>,
    /// Conservative admission weight used by the scheduler. This is neither
    /// measured RSS nor a promise that a process will remain below this value.
    pub(crate) scheduling_weight_bytes: Option<u64>,
    #[cfg(any(test, feature = "parallel-markdown"))]
    pub(crate) default_jobs: usize,
    #[cfg(any(test, feature = "parallel-markdown"))]
    pub(crate) max_jobs: usize,
}

#[cfg(any(test, feature = "network-icons"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NetworkResourceLimits {
    pub(crate) max_redirects: usize,
    pub(crate) connect_timeout: Duration,
    pub(crate) per_hop_timeout: Duration,
    pub(crate) workflow_timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedResourcePolicy {
    #[cfg(not(feature = "svg"))]
    input: InputResourcePolicy,
    #[cfg(feature = "svg")]
    render: RenderResourcePolicy,
    #[cfg(feature = "ascii")]
    ascii: AsciiResourcePolicy,
    adjunct: CliAdjunctResourcePolicy,
    #[cfg(any(test, feature = "parallel-markdown"))]
    available_parallelism: NonZeroUsize,
}

impl ResolvedResourcePolicy {
    pub(crate) fn for_profile(profile: ResourceProfile) -> Self {
        let parallelism = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
        Self::for_profile_with_parallelism(profile, parallelism)
    }

    pub(crate) const fn for_profile_with_parallelism(
        profile: ResourceProfile,
        available_parallelism: NonZeroUsize,
    ) -> Self {
        #[cfg(not(any(test, feature = "parallel-markdown")))]
        let _ = available_parallelism;
        Self {
            #[cfg(not(feature = "svg"))]
            input: InputResourcePolicy::for_profile(profile),
            #[cfg(feature = "svg")]
            render: RenderResourcePolicy::for_profile(profile),
            #[cfg(feature = "ascii")]
            ascii: AsciiResourcePolicy::for_profile(profile),
            adjunct: CliAdjunctResourcePolicy::for_profile(profile),
            #[cfg(any(test, feature = "parallel-markdown"))]
            available_parallelism,
        }
    }

    #[cfg(any(test, feature = "svg", feature = "ascii"))]
    pub(crate) fn profile(&self) -> ResourceProfile {
        self.input_policy().profile()
    }

    pub(crate) fn input_policy(&self) -> &InputResourcePolicy {
        #[cfg(feature = "svg")]
        {
            self.render.input_policy()
        }
        #[cfg(not(feature = "svg"))]
        {
            &self.input
        }
    }

    #[cfg(feature = "svg")]
    pub(crate) const fn render_policy(&self) -> RenderResourcePolicy {
        self.render
    }

    #[cfg(feature = "ascii")]
    pub(crate) const fn ascii_policy(&self) -> AsciiResourcePolicy {
        self.ascii
    }

    pub(crate) fn files(&self) -> AuxiliaryFileLimits {
        AuxiliaryFileLimits {
            markdown_document_bytes: self
                .acquisition_byte_limit(CliResourceLimitId::MaxMarkdownDocumentBytes),
            config_bytes: self.acquisition_byte_limit(CliResourceLimitId::MaxConfigBytes),
            css_bytes: self.acquisition_byte_limit(CliResourceLimitId::MaxCssBytes),
            puppeteer_config_bytes: self
                .acquisition_byte_limit(CliResourceLimitId::MaxPuppeteerConfigBytes),
        }
    }

    #[cfg(any(test, feature = "icons"))]
    pub(crate) fn icons(&self) -> IconResourceLimits {
        IconResourceLimits {
            local_body_bytes: self
                .acquisition_byte_limit(CliResourceLimitId::MaxLocalIconBodyBytes),
            #[cfg(feature = "network-icons")]
            remote_body_bytes: self
                .acquisition_byte_limit(CliResourceLimitId::MaxRemoteIconBodyBytes),
            #[cfg(test)]
            aggregate_bytes: self.acquisition_byte_limit(CliResourceLimitId::MaxAggregateIconBytes),
            #[cfg(test)]
            pack_count: self.value(CliResourceLimitId::MaxIconPacks),
        }
    }

    #[cfg(any(test, feature = "svg", feature = "ascii"))]
    pub(crate) fn batch(&self) -> BatchResourceLimits {
        #[cfg(any(test, feature = "parallel-markdown"))]
        let maximum = self
            .value(CliResourceLimitId::MaxJobs)
            .expect("max_jobs always retains a hard guard");
        #[cfg(any(test, feature = "parallel-markdown"))]
        let max_jobs = usize::try_from(maximum).expect("max_jobs hard guard fits usize");
        #[cfg(any(test, feature = "parallel-markdown"))]
        let cpu_count = self.available_parallelism.get();
        #[cfg(any(test, feature = "parallel-markdown"))]
        let profile_default = match self.profile() {
            ResourceProfile::Constrained => 1,
            ResourceProfile::Interactive => cpu_count.min(2),
            ResourceProfile::TrustedNative => (cpu_count / 2).clamp(1, 8),
            ResourceProfile::UnboundedForTrustedInput => (cpu_count / 2).clamp(1, 32),
        };
        BatchResourceLimits {
            #[cfg(test)]
            markdown_charts: self.value(CliResourceLimitId::MaxMarkdownCharts),
            #[cfg(test)]
            staged_bytes: self.value(CliResourceLimitId::MaxStagedBytes),
            scheduling_weight_bytes: self.value(CliResourceLimitId::MaxSchedulingWeightBytes),
            #[cfg(any(test, feature = "parallel-markdown"))]
            default_jobs: profile_default.min(max_jobs),
            #[cfg(any(test, feature = "parallel-markdown"))]
            max_jobs,
        }
    }

    #[cfg(any(test, feature = "network-icons"))]
    pub(crate) fn network(&self) -> NetworkResourceLimits {
        NetworkResourceLimits {
            max_redirects: self
                .value_as_usize(CliResourceLimitId::MaxRedirects)
                .expect("max_redirects always retains a hard guard"),
            connect_timeout: self.duration(CliResourceLimitId::ConnectTimeoutSeconds),
            per_hop_timeout: self.duration(CliResourceLimitId::PerHopTimeoutSeconds),
            workflow_timeout: self.duration(CliResourceLimitId::WorkflowTimeoutSeconds),
        }
    }

    pub(crate) const fn value(&self, id: CliResourceLimitId) -> Option<u64> {
        self.adjunct.value(id)
    }

    #[cfg(test)]
    pub(crate) const fn base_value(&self, id: CliResourceLimitId) -> Option<u64> {
        self.adjunct.base_value(id)
    }

    #[cfg(test)]
    pub(crate) const fn explicit_override(&self, id: CliResourceLimitId) -> Option<u64> {
        self.adjunct.explicit_override(id)
    }

    pub(crate) fn apply_override(
        &mut self,
        stable_id: &str,
        value: u64,
    ) -> Result<(), ResourcePolicyOverrideError> {
        #[cfg(feature = "ascii")]
        if let Some(id) = AsciiResourceLimitId::from_stable_id(stable_id) {
            let requested = value;
            let value = usize::try_from(requested).map_err(|_| {
                ResourcePolicyOverrideError::ValueOutOfRange {
                    limit: id.as_str(),
                    requested,
                }
            })?;
            return self
                .ascii
                .apply_limit(id, value)
                .map_err(|_| ResourcePolicyOverrideError::NonPositive(id.as_str()));
        }

        #[cfg(feature = "svg")]
        if let Some(id) = RenderResourceLimitId::from_stable_id(stable_id) {
            let requested = value;
            let value = usize::try_from(requested).map_err(|_| {
                ResourcePolicyOverrideError::ValueOutOfRange {
                    limit: id.as_str(),
                    requested,
                }
            })?;
            return self
                .render
                .apply_limit(id, value)
                .map_err(|error| map_render_override_error(error, requested));
        }

        #[cfg(not(feature = "svg"))]
        if let Some(id) = InputResourceLimitId::from_stable_id(stable_id) {
            let value = usize::try_from(value).map_err(|_| {
                ResourcePolicyOverrideError::ValueOutOfRange {
                    limit: id.as_str(),
                    requested: value,
                }
            })?;
            return self
                .input
                .apply_limit(id, value)
                .map_err(map_input_override_error);
        }

        let id = CliResourceLimitId::from_stable_id(stable_id)
            .ok_or_else(|| ResourcePolicyOverrideError::UnknownLimit(stable_id.to_owned()))?;
        self.adjunct.apply_limit(id, value)
    }

    #[cfg(any(test, feature = "icons", feature = "markdown"))]
    pub(crate) fn checked_bytes(&self, kind: ByteLedgerKind) -> CheckedBytes {
        let id = kind.limit_id();
        CheckedBytes(CheckedLedger::new(self.profile(), id, self.value(id)))
    }

    #[cfg(any(test, feature = "icons", feature = "markdown"))]
    pub(crate) fn checked_count(&self, kind: CountLedgerKind) -> CheckedCount {
        let id = kind.limit_id();
        CheckedCount(CheckedLedger::new(self.profile(), id, self.value(id)))
    }

    #[cfg(any(test, feature = "svg", feature = "ascii"))]
    pub(crate) fn checked_scheduling_weight(&self) -> CheckedSchedulingWeight {
        let id = CliResourceLimitId::MaxSchedulingWeightBytes;
        CheckedSchedulingWeight(CheckedLedger::new(self.profile(), id, self.value(id)))
    }

    fn acquisition_byte_limit(&self, id: CliResourceLimitId) -> Option<usize> {
        self.value_as_usize(id)
    }

    fn value_as_usize(&self, id: CliResourceLimitId) -> Option<usize> {
        self.value(id).map(|value| {
            usize::try_from(value)
                .expect("validated acquisition and hard-guard values must fit usize")
        })
    }

    #[cfg(any(test, feature = "network-icons"))]
    fn duration(&self, id: CliResourceLimitId) -> Duration {
        Duration::from_secs(
            self.value(id)
                .expect("network durations always retain a hard guard"),
        )
    }
}

#[cfg(not(feature = "svg"))]
fn map_input_override_error(error: InputResourceLimitOverrideError) -> ResourcePolicyOverrideError {
    match error {
        InputResourceLimitOverrideError::UnknownLimit(id) => {
            ResourcePolicyOverrideError::UnknownLimit(id)
        }
        InputResourceLimitOverrideError::NonPositive(id) => {
            ResourcePolicyOverrideError::NonPositive(id)
        }
    }
}

#[cfg(feature = "svg")]
fn map_render_override_error(
    error: RenderResourceLimitOverrideError,
    requested: u64,
) -> ResourcePolicyOverrideError {
    match error {
        RenderResourceLimitOverrideError::UnknownLimit(id) => {
            ResourcePolicyOverrideError::UnknownLimit(id)
        }
        RenderResourceLimitOverrideError::HardCap(id) => {
            ResourcePolicyOverrideError::HardCapability {
                limit: id,
                requested,
            }
        }
        RenderResourceLimitOverrideError::NonPositive(id) => {
            ResourcePolicyOverrideError::NonPositive(id)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ResourcePolicyOverrideError {
    #[error("resource limit id `{0}` is not part of the CLI resource contract")]
    UnknownLimit(String),
    #[error("resource limit `{0}` must be a positive integer")]
    NonPositive(&'static str),
    #[error("resource limit `{limit}` value {requested} does not fit this target")]
    ValueOutOfRange { limit: &'static str, requested: u64 },
    #[error("resource limit `{limit}` value {requested} exceeds its hard capability of {max}")]
    HardCap {
        limit: &'static str,
        requested: u64,
        max: u64,
    },
    #[cfg(feature = "svg")]
    #[error("resource limit `{limit}` is a hard capability and cannot be set to {requested}")]
    HardCapability { limit: &'static str, requested: u64 },
}

#[cfg(any(test, feature = "icons", feature = "markdown"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ByteLedgerKind {
    #[cfg(any(test, feature = "icons"))]
    AggregateIcons,
    #[cfg(any(test, feature = "markdown"))]
    StagedOutput,
    #[cfg(any(test, feature = "rustdoc"))]
    RustdocInput,
}

#[cfg(any(test, feature = "icons", feature = "markdown"))]
impl ByteLedgerKind {
    const fn limit_id(self) -> CliResourceLimitId {
        match self {
            #[cfg(any(test, feature = "icons"))]
            Self::AggregateIcons => CliResourceLimitId::MaxAggregateIconBytes,
            #[cfg(any(test, feature = "markdown"))]
            Self::StagedOutput => CliResourceLimitId::MaxStagedBytes,
            #[cfg(any(test, feature = "rustdoc"))]
            Self::RustdocInput => CliResourceLimitId::MaxRustdocInputBytes,
        }
    }
}

#[cfg(any(test, feature = "icons", feature = "markdown"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CountLedgerKind {
    #[cfg(any(test, feature = "icons"))]
    IconPacks,
    #[cfg(any(test, feature = "markdown"))]
    MarkdownCharts,
}

#[cfg(any(test, feature = "icons", feature = "markdown"))]
impl CountLedgerKind {
    const fn limit_id(self) -> CliResourceLimitId {
        match self {
            #[cfg(any(test, feature = "icons"))]
            Self::IconPacks => CliResourceLimitId::MaxIconPacks,
            #[cfg(any(test, feature = "markdown"))]
            Self::MarkdownCharts => CliResourceLimitId::MaxMarkdownCharts,
        }
    }
}

#[cfg(any(test, feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedLedger {
    profile: ResourceProfile,
    id: CliResourceLimitId,
    used: u64,
    max: Option<u64>,
}

#[cfg(any(test, feature = "svg", feature = "ascii"))]
impl CheckedLedger {
    const fn new(profile: ResourceProfile, id: CliResourceLimitId, max: Option<u64>) -> Self {
        Self {
            profile,
            id,
            used: 0,
            max,
        }
    }

    fn try_add(&mut self, amount: u64) -> Result<(), ResourceLedgerError> {
        let actual =
            self.used
                .checked_add(amount)
                .ok_or(ResourceLedgerError::ArithmeticOverflow {
                    limit: self.id.as_str(),
                    current: self.used,
                    attempted_change: amount,
                })?;
        if let Some(max) = self.max
            && actual > max
        {
            return Err(ResourceLedgerError::LimitExceeded {
                profile: self.profile,
                limit: self.id.as_str(),
                actual,
                max,
            });
        }
        self.used = actual;
        Ok(())
    }

    fn try_release(&mut self, amount: u64) -> Result<(), ResourceLedgerError> {
        self.used =
            self.used
                .checked_sub(amount)
                .ok_or(ResourceLedgerError::ArithmeticUnderflow {
                    limit: self.id.as_str(),
                    current: self.used,
                    attempted_change: amount,
                })?;
        Ok(())
    }
}

#[cfg(any(test, feature = "icons", feature = "markdown"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedBytes(CheckedLedger);

#[cfg(any(test, feature = "icons", feature = "markdown"))]
impl CheckedBytes {
    #[cfg(test)]
    pub(crate) const fn used(&self) -> u64 {
        self.0.used
    }

    #[cfg(test)]
    pub(crate) const fn max(&self) -> Option<u64> {
        self.0.max
    }

    #[cfg(feature = "icons")]
    pub(crate) const fn remaining(&self) -> Option<u64> {
        match self.0.max {
            Some(max) => Some(max - self.0.used),
            None => None,
        }
    }

    pub(crate) fn try_add(&mut self, amount: u64) -> Result<(), ResourceLedgerError> {
        self.0.try_add(amount)
    }
}

#[cfg(any(test, feature = "icons", feature = "markdown"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedCount(CheckedLedger);

#[cfg(any(test, feature = "icons", feature = "markdown"))]
impl CheckedCount {
    #[cfg(test)]
    pub(crate) const fn used(&self) -> u64 {
        self.0.used
    }

    #[cfg(any(test, feature = "markdown"))]
    pub(crate) const fn max(&self) -> Option<u64> {
        self.0.max
    }

    pub(crate) fn try_add(&mut self, amount: u64) -> Result<(), ResourceLedgerError> {
        self.0.try_add(amount)
    }
}

#[cfg(any(test, feature = "svg", feature = "ascii"))]
/// Tracks conservative backend admission weight, not measured or reserved RSS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedSchedulingWeight(CheckedLedger);

#[cfg(any(test, feature = "svg", feature = "ascii"))]
impl CheckedSchedulingWeight {
    #[cfg(test)]
    pub(crate) const fn used(&self) -> u64 {
        self.0.used
    }

    pub(crate) const fn max(&self) -> Option<u64> {
        self.0.max
    }

    pub(crate) fn try_acquire(&mut self, amount: u64) -> Result<(), ResourceLedgerError> {
        self.0.try_add(amount)
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    pub(crate) fn check_single(&self, amount: u64) -> Result<(), ResourceLedgerError> {
        let mut empty = CheckedLedger::new(self.0.profile, self.0.id, self.0.max);
        empty.try_add(amount)
    }

    pub(crate) fn release(&mut self, amount: u64) -> Result<(), ResourceLedgerError> {
        self.0.try_release(amount)
    }
}

#[cfg(any(test, feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ResourceLedgerError {
    #[error("resource limit exceeded: {limit} actual={actual} max={max} profile={profile}")]
    LimitExceeded {
        profile: ResourceProfile,
        limit: &'static str,
        actual: u64,
        max: u64,
    },
    #[error(
        "resource accounting overflow for {limit}: current={current} change={attempted_change}"
    )]
    ArithmeticOverflow {
        limit: &'static str,
        current: u64,
        attempted_change: u64,
    },
    #[error(
        "resource accounting underflow for {limit}: current={current} change={attempted_change}"
    )]
    ArithmeticUnderflow {
        limit: &'static str,
        current: u64,
        attempted_change: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    const CPU_64: NonZeroUsize = NonZeroUsize::new(64).unwrap();

    #[test]
    fn profile_budgets_are_monotonic() {
        let constrained = ResolvedResourcePolicy::for_profile_with_parallelism(
            ResourceProfile::Constrained,
            CPU_64,
        );
        let interactive = ResolvedResourcePolicy::for_profile_with_parallelism(
            ResourceProfile::Interactive,
            CPU_64,
        );
        let trusted = ResolvedResourcePolicy::for_profile_with_parallelism(
            ResourceProfile::TrustedNative,
            CPU_64,
        );
        let unbounded = ResolvedResourcePolicy::for_profile_with_parallelism(
            ResourceProfile::UnboundedForTrustedInput,
            CPU_64,
        );

        for id in CliResourceLimitId::ALL {
            assert_budget_le(constrained.value(id), interactive.value(id), id);
            assert_budget_le(interactive.value(id), trusted.value(id), id);
            assert_budget_le(trusted.value(id), unbounded.value(id), id);
        }
        assert_eq!(constrained.batch().default_jobs, 1);
        assert_eq!(interactive.batch().default_jobs, 2);
        assert_eq!(trusted.batch().default_jobs, 8);
        assert_eq!(unbounded.batch().default_jobs, 32);

        let source = merman::resources::InputResourceLimitId::MaxSourceBytes;
        assert_budget_le(
            constrained
                .input_policy()
                .value(source)
                .map(|value| value as u64),
            interactive
                .input_policy()
                .value(source)
                .map(|value| value as u64),
            CliResourceLimitId::MaxMarkdownDocumentBytes,
        );
        assert_budget_le(
            interactive
                .input_policy()
                .value(source)
                .map(|value| value as u64),
            trusted
                .input_policy()
                .value(source)
                .map(|value| value as u64),
            CliResourceLimitId::MaxMarkdownDocumentBytes,
        );
        assert_eq!(unbounded.input_policy().value(source), None);
    }

    #[test]
    fn unbounded_profile_retains_protocol_renderer_and_execution_hard_guards() {
        let policy = ResolvedResourcePolicy::for_profile_with_parallelism(
            ResourceProfile::UnboundedForTrustedInput,
            CPU_64,
        );

        assert_eq!(policy.files().markdown_document_bytes, None);
        assert_eq!(
            policy.icons().local_body_bytes,
            usize::try_from(HARD_MAX_ICON_PACK_BYTES).ok()
        );
        assert_eq!(
            policy.icons().aggregate_bytes,
            usize::try_from(HARD_MAX_AGGREGATE_ICON_BYTES).ok()
        );
        assert_eq!(policy.batch().markdown_charts, None);
        assert_eq!(policy.batch().staged_bytes, None);
        assert_eq!(policy.batch().scheduling_weight_bytes, None);
        assert_eq!(policy.icons().pack_count, Some(HARD_MAX_ICON_PACKS));
        assert_eq!(policy.batch().max_jobs as u64, HARD_MAX_JOBS);
        assert_eq!(policy.network().max_redirects as u64, HARD_MAX_REDIRECTS);
        assert_eq!(policy.network().connect_timeout, HARD_CONNECT_TIMEOUT);
        assert_eq!(policy.network().per_hop_timeout, HARD_PER_HOP_TIMEOUT);
        assert_eq!(policy.network().workflow_timeout, HARD_WORKFLOW_TIMEOUT);

        let mut icon_packs = policy.checked_count(CountLedgerKind::IconPacks);
        icon_packs.try_add(HARD_MAX_ICON_PACKS).unwrap();
        assert_limit_plus_one(
            icon_packs.try_add(1),
            "max_icon_packs",
            HARD_MAX_ICON_PACKS + 1,
        );
    }

    #[test]
    fn stable_overrides_delegate_to_canonical_and_cli_limits() {
        let mut policy = ResolvedResourcePolicy::for_profile_with_parallelism(
            ResourceProfile::UnboundedForTrustedInput,
            CPU_64,
        );

        policy.apply_override("max_source_bytes", 17).unwrap();
        policy.apply_override("max_css_bytes", 23).unwrap();
        #[cfg(feature = "ascii")]
        for id in AsciiResourceLimitId::ALL {
            policy.apply_override(id.as_str(), 29).unwrap();
        }

        assert_eq!(
            policy
                .input_policy()
                .value(merman::resources::InputResourceLimitId::MaxSourceBytes),
            Some(17)
        );
        assert_eq!(policy.files().css_bytes, Some(23));
        assert_eq!(
            policy.explicit_override(CliResourceLimitId::MaxCssBytes),
            Some(23)
        );
        assert_eq!(policy.base_value(CliResourceLimitId::MaxCssBytes), None);
        #[cfg(feature = "ascii")]
        for id in AsciiResourceLimitId::ALL {
            assert_eq!(policy.ascii_policy().value(id), Some(29));
        }
    }

    #[test]
    fn stable_overrides_reject_unknown_zero_and_hard_cap_values() {
        let mut policy = ResolvedResourcePolicy::for_profile_with_parallelism(
            ResourceProfile::Interactive,
            CPU_64,
        );

        assert!(matches!(
            policy.apply_override("future_limit", 1),
            Err(ResourcePolicyOverrideError::UnknownLimit(id)) if id == "future_limit"
        ));
        assert_eq!(
            policy.apply_override("max_css_bytes", 0),
            Err(ResourcePolicyOverrideError::NonPositive("max_css_bytes"))
        );
        #[cfg(feature = "ascii")]
        assert_eq!(
            policy.apply_override("max_ascii_document_cells", 0),
            Err(ResourcePolicyOverrideError::NonPositive(
                "max_ascii_document_cells"
            ))
        );
        assert_eq!(
            policy.apply_override("max_jobs", HARD_MAX_JOBS + 1),
            Err(ResourcePolicyOverrideError::HardCap {
                limit: "max_jobs",
                requested: 65,
                max: HARD_MAX_JOBS,
            })
        );
        assert_eq!(
            policy.apply_override("max_local_icon_body_bytes", HARD_MAX_ICON_PACK_BYTES + 1),
            Err(ResourcePolicyOverrideError::HardCap {
                limit: "max_local_icon_body_bytes",
                requested: HARD_MAX_ICON_PACK_BYTES + 1,
                max: HARD_MAX_ICON_PACK_BYTES,
            })
        );
        assert_eq!(
            policy.apply_override(
                "max_aggregate_icon_bytes",
                HARD_MAX_AGGREGATE_ICON_BYTES + 1,
            ),
            Err(ResourcePolicyOverrideError::HardCap {
                limit: "max_aggregate_icon_bytes",
                requested: HARD_MAX_AGGREGATE_ICON_BYTES + 1,
                max: HARD_MAX_AGGREGATE_ICON_BYTES,
            })
        );
        assert_eq!(
            policy.apply_override("max_icon_packs", HARD_MAX_ICON_PACKS + 1),
            Err(ResourcePolicyOverrideError::HardCap {
                limit: "max_icon_packs",
                requested: HARD_MAX_ICON_PACKS + 1,
                max: HARD_MAX_ICON_PACKS,
            })
        );
    }

    #[test]
    fn byte_count_and_scheduling_ledgers_accept_exact_and_reject_plus_one() {
        let policy = ResolvedResourcePolicy::for_profile_with_parallelism(
            ResourceProfile::Constrained,
            CPU_64,
        );

        let mut bytes = policy.checked_bytes(ByteLedgerKind::AggregateIcons);
        let byte_limit = bytes.max().unwrap();
        bytes.try_add(byte_limit).unwrap();
        assert_eq!(bytes.used(), byte_limit);
        assert_limit_plus_one(bytes.try_add(1), "max_aggregate_icon_bytes", byte_limit + 1);

        let mut count = policy.checked_count(CountLedgerKind::MarkdownCharts);
        let count_limit = count.max().unwrap();
        count.try_add(count_limit).unwrap();
        assert_eq!(count.used(), count_limit);
        assert_limit_plus_one(count.try_add(1), "max_markdown_charts", count_limit + 1);

        let mut scheduling = policy.checked_scheduling_weight();
        let scheduling_limit = scheduling.max().unwrap();
        scheduling.try_acquire(scheduling_limit).unwrap();
        assert_eq!(scheduling.used(), scheduling_limit);
        assert_limit_plus_one(
            scheduling.try_acquire(1),
            "max_scheduling_weight_bytes",
            scheduling_limit + 1,
        );
    }

    #[test]
    fn unlimited_ledgers_still_reject_integer_overflow_and_release_underflow() {
        let policy = ResolvedResourcePolicy::for_profile_with_parallelism(
            ResourceProfile::UnboundedForTrustedInput,
            CPU_64,
        );
        let mut bytes = policy.checked_bytes(ByteLedgerKind::StagedOutput);
        bytes.try_add(u64::MAX).unwrap();
        assert!(matches!(
            bytes.try_add(1),
            Err(ResourceLedgerError::ArithmeticOverflow {
                limit: "max_staged_bytes",
                current: u64::MAX,
                attempted_change: 1,
            })
        ));

        let mut scheduling = policy.checked_scheduling_weight();
        assert!(matches!(
            scheduling.release(1),
            Err(ResourceLedgerError::ArithmeticUnderflow {
                limit: "max_scheduling_weight_bytes",
                current: 0,
                attempted_change: 1,
            })
        ));
    }

    #[test]
    fn release_makes_scheduling_capacity_available_again() {
        let policy = ResolvedResourcePolicy::for_profile_with_parallelism(
            ResourceProfile::Constrained,
            CPU_64,
        );
        let mut scheduling = policy.checked_scheduling_weight();
        let limit = scheduling.max().unwrap();

        scheduling.try_acquire(limit).unwrap();
        scheduling.release(1).unwrap();
        scheduling.try_acquire(1).unwrap();

        assert_eq!(scheduling.used(), limit);
    }

    fn assert_budget_le(lower: Option<u64>, upper: Option<u64>, id: CliResourceLimitId) {
        match (lower, upper) {
            (_, None) => {}
            (Some(lower), Some(upper)) => {
                assert!(lower <= upper, "{}: {lower} > {upper}", id.as_str());
            }
            (None, Some(upper)) => {
                panic!("{}: unlimited is not <= {upper}", id.as_str());
            }
        }
    }

    fn assert_limit_plus_one(
        result: Result<(), ResourceLedgerError>,
        expected_limit: &'static str,
        expected_actual: u64,
    ) {
        assert!(matches!(
            result,
            Err(ResourceLedgerError::LimitExceeded {
                limit,
                actual,
                ..
            }) if limit == expected_limit && actual == expected_actual
        ));
    }
}
