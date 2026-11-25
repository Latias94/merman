//! Shared render budgets, resource admission, and cumulative operation accounting.

mod work;

#[doc(hidden)]
pub use work::{
    OperationWorkError, OperationWorkMeter, PreparedTextRetainedReservation, SvgByteBudget,
    SvgByteReservation,
};

#[cfg(feature = "resources-class")]
use merman_model::families::class::ClassDiagram;
#[cfg(any(
    feature = "resources-flowchart",
    feature = "resources-swimlane",
    feature = "resources-agentflow"
))]
use merman_model::families::flowchart::FlowchartModel;
#[cfg(feature = "resources-mindmap")]
use merman_model::families::mindmap::MindmapDiagramRenderModel;
#[cfg(feature = "resources-zenuml")]
use merman_model::families::zenuml::ZenumlDiagramModel;
use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
use merman_model::resources::{
    InputResourceLimitExceeded, InputResourceLimitId, InputResourceLimitPhase, InputResourcePolicy,
    InputResourcePolicyRestrictionError,
};
pub use merman_model::resources::{
    ModelComplexity, RESOURCE_PROFILE_DESCRIPTORS, ResourceProfile as RenderResourceProfile,
    ResourceProfileDescriptor as RenderResourceProfileDescriptor,
};

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

/// Hard recursion cap used by the SVG backend on WebAssembly targets.
pub const WASM_RESVG_TREE_DEPTH_HARD_CAP: usize = 64;
/// Maximum XML nesting accepted by icon-body input admission.
///
/// This bounds icon parsing and sanitization independently of the eventual output consumer.
/// Native conversion separately enforces its target's resolved SVG backend tree limits.
pub const MAX_PORTABLE_ICON_BODY_XML_DEPTH: usize = 32;

#[cfg(not(target_arch = "wasm32"))]
pub const MAX_RESVG_TREE_DEPTH: usize = 256;
pub const SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID: &str = "svg_backend_tree_depth";
/// Maximum resolved usvg nodes accepted independently of caller-selected policies.
///
/// This matches usvg's own parser capability while rejecting expansion before it can allocate the
/// upstream parser's full tree.
pub const MAX_RESVG_TREE_NODES: usize = 1_000_000;
pub const SVG_BACKEND_TREE_NODES_HARD_CAP_ID: &str = "svg_backend_tree_nodes";

// usvg, resvg, and tiny-skia eventually project SVG geometry into f32 coordinates. Keep the
// document coordinate space within the largest consecutive integer range representable by f32 so
// one-unit geometry remains distinguishable before target-specific raster limits run.
#[doc(hidden)]
pub const MAX_SVG_BACKEND_COORDINATE_MAGNITUDE: usize = 1 << 24;
#[doc(hidden)]
pub const SVG_BACKEND_COORDINATE_MAGNITUDE_HARD_CAP_ID: &str = "svg_backend_coordinate_magnitude";

#[cfg(target_arch = "wasm32")]
pub const MAX_RESVG_TREE_DEPTH: usize = WASM_RESVG_TREE_DEPTH_HARD_CAP;

pub const RESOURCE_PROFILE_COUNT: usize = merman_model::resources::RESOURCE_PROFILE_COUNT;
const RENDER_RESOURCE_LIMIT_COUNT: usize = 7;
pub const RESOURCE_LIMIT_COUNT: usize =
    merman_model::resources::INPUT_RESOURCE_LIMIT_COUNT + RENDER_RESOURCE_LIMIT_COUNT;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceLimitPhase {
    Source,
    LayoutModel,
    SvgOutput,
    SvgPostprocess,
    GeometryOutput,
}

/// Stable reason why a resource check failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResourceLimitCause {
    /// The requested work exceeded the configured policy ceiling.
    Ceiling,
    /// Computing cumulative work overflowed the platform counter.
    ArithmeticOverflow,
}

impl ResourceLimitCause {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ceiling => "ceiling",
            Self::ArithmeticOverflow => "arithmetic_overflow",
        }
    }
}

impl std::fmt::Display for ResourceLimitCause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl ResourceLimitPhase {
    const ALL: [Self; 5] = [
        Self::Source,
        Self::LayoutModel,
        Self::SvgOutput,
        Self::SvgPostprocess,
        Self::GeometryOutput,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::LayoutModel => "layout_model",
            Self::SvgOutput => "svg_output",
            Self::SvgPostprocess => "svg_postprocess",
            Self::GeometryOutput => "geometry_output",
        }
    }

    fn from_stable_id(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|phase| phase.as_str() == value)
    }
}

impl std::fmt::Display for ResourceLimitPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderResourceLimitId {
    MaxSvgBytes,
    /// SVG input, postprocessing, and native reference-expansion budget.
    ///
    /// Ordinary diagram producers use source, model, layout-work, and serialized-byte bounds;
    /// they do not count their emitted XML elements.
    MaxSvgElements,
    MaxLayoutWorkUnits,
    MaxPreparedTextRetainedBytes,
    SvgBackendTreeNodes,
    SvgBackendTreeDepth,
    MaxGeometryJsonBytes,
}

impl RenderResourceLimitId {
    pub const ALL: [Self; RENDER_RESOURCE_LIMIT_COUNT] = [
        Self::MaxSvgBytes,
        Self::MaxSvgElements,
        Self::MaxLayoutWorkUnits,
        Self::MaxPreparedTextRetainedBytes,
        Self::SvgBackendTreeNodes,
        Self::SvgBackendTreeDepth,
        Self::MaxGeometryJsonBytes,
    ];

    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceLimitId {
    Input(InputResourceLimitId),
    Render(RenderResourceLimitId),
}

#[allow(non_upper_case_globals)]
impl ResourceLimitId {
    pub const MaxSourceBytes: Self = Self::Input(InputResourceLimitId::MaxSourceBytes);
    pub const MaxModelItems: Self = Self::Input(InputResourceLimitId::MaxModelItems);
    pub const MaxModelTextBytes: Self = Self::Input(InputResourceLimitId::MaxModelTextBytes);
    pub const MaxModelNestingDepth: Self = Self::Input(InputResourceLimitId::MaxModelNestingDepth);
    pub const MaxGeometryJsonBytes: Self =
        Self::Render(RenderResourceLimitId::MaxGeometryJsonBytes);
    pub const MaxSvgBytes: Self = Self::Render(RenderResourceLimitId::MaxSvgBytes);
    pub const MaxSvgElements: Self = Self::Render(RenderResourceLimitId::MaxSvgElements);
    pub const MaxLayoutWorkUnits: Self = Self::Render(RenderResourceLimitId::MaxLayoutWorkUnits);
    pub const MaxPreparedTextRetainedBytes: Self =
        Self::Render(RenderResourceLimitId::MaxPreparedTextRetainedBytes);
    pub const SvgBackendTreeNodes: Self = Self::Render(RenderResourceLimitId::SvgBackendTreeNodes);
    pub const SvgBackendTreeDepth: Self = Self::Render(RenderResourceLimitId::SvgBackendTreeDepth);

    pub const ALL: [Self; RESOURCE_LIMIT_COUNT] = [
        Self::MaxSourceBytes,
        Self::MaxModelItems,
        Self::MaxModelTextBytes,
        Self::MaxModelNestingDepth,
        Self::MaxLayoutWorkUnits,
        Self::MaxPreparedTextRetainedBytes,
        Self::MaxSvgBytes,
        Self::MaxSvgElements,
        Self::SvgBackendTreeNodes,
        Self::SvgBackendTreeDepth,
        Self::MaxGeometryJsonBytes,
    ];

    pub fn from_stable_id(id: &str) -> Option<Self> {
        InputResourceLimitId::from_stable_id(id)
            .map(Self::Input)
            .or_else(|| {
                RENDER_RESOURCE_LIMIT_DESCRIPTORS
                    .iter()
                    .find(|descriptor| descriptor.stable_id == id)
                    .map(|descriptor| descriptor.id)
            })
    }

    pub const fn descriptor(self) -> ResourceLimitDescriptor {
        match self {
            Self::Input(id) => input_descriptor(id),
            Self::Render(id) => RENDER_RESOURCE_LIMIT_DESCRIPTORS[id.index()],
        }
    }

    pub const fn as_str(self) -> &'static str {
        self.descriptor().stable_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResourceLimitDescriptor {
    pub id: ResourceLimitId,
    pub stable_id: &'static str,
    pub phase: ResourceLimitPhase,
    pub description: &'static str,
    pub overridable: bool,
    pub hard_cap: bool,
    pub minimum_value: usize,
}

const fn input_descriptor(id: InputResourceLimitId) -> ResourceLimitDescriptor {
    let descriptor = id.descriptor();
    ResourceLimitDescriptor {
        id: ResourceLimitId::Input(id),
        stable_id: descriptor.stable_id,
        phase: match descriptor.phase {
            InputResourceLimitPhase::Source => ResourceLimitPhase::Source,
            InputResourceLimitPhase::Model => ResourceLimitPhase::LayoutModel,
        },
        description: descriptor.description,
        overridable: descriptor.overridable,
        hard_cap: false,
        minimum_value: descriptor.minimum_value,
    }
}

const RENDER_RESOURCE_LIMIT_DESCRIPTORS: [ResourceLimitDescriptor; RENDER_RESOURCE_LIMIT_COUNT] = [
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxSvgBytes,
        stable_id: "max_svg_bytes",
        phase: ResourceLimitPhase::SvgOutput,
        description: "Maximum serialized SVG bytes",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxSvgElements,
        stable_id: "max_svg_elements",
        phase: ResourceLimitPhase::SvgPostprocess,
        description: "Maximum admitted SVG elements or reference-expanded work during SVG input admission, postprocessing, and native conversion",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxLayoutWorkUnits,
        stable_id: "max_layout_work_units",
        phase: ResourceLimitPhase::LayoutModel,
        description: "Maximum family-accounted derived layout and render geometry work units",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxPreparedTextRetainedBytes,
        stable_id: "max_prepared_text_retained_bytes",
        phase: ResourceLimitPhase::LayoutModel,
        description: "Maximum operation-local bytes retained by prepared text artifacts",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::SvgBackendTreeNodes,
        stable_id: SVG_BACKEND_TREE_NODES_HARD_CAP_ID,
        phase: ResourceLimitPhase::SvgPostprocess,
        description: "Maximum SVG backend tree nodes accepted by rendering backends",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::SvgBackendTreeDepth,
        stable_id: SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID,
        phase: ResourceLimitPhase::SvgPostprocess,
        description: "Maximum SVG backend tree depth accepted by rendering backends",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxGeometryJsonBytes,
        stable_id: "max_geometry_json_bytes",
        phase: ResourceLimitPhase::GeometryOutput,
        description: "Maximum encoded edge geometry JSON bytes",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
];

pub static RESOURCE_LIMIT_DESCRIPTORS: [ResourceLimitDescriptor; RESOURCE_LIMIT_COUNT] = [
    input_descriptor(InputResourceLimitId::MaxSourceBytes),
    input_descriptor(InputResourceLimitId::MaxModelItems),
    input_descriptor(InputResourceLimitId::MaxModelTextBytes),
    input_descriptor(InputResourceLimitId::MaxModelNestingDepth),
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[2],
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[3],
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[0],
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[1],
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[4],
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[5],
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[6],
];

const RENDER_PROFILE_VALUES: [[Option<usize>; RESOURCE_PROFILE_COUNT];
    RENDER_RESOURCE_LIMIT_COUNT] = [
    [Some(24 * MIB), Some(12 * MIB), Some(128 * MIB), None],
    [Some(250_000), Some(125_000), Some(1_000_000), None],
    // A deterministic work-admission budget, not a Mermaid limit or response-time bound.
    // Interactive and trusted-native admit ordinary diagrams, including nested ELK layouts;
    // constrained remains a tighter host policy. Hosts control latency through cancellation,
    // deadlines, and concurrency. See the dated layout-work calibration receipts in
    // docs/performance for the corpus and the profile-specific rounding rules.
    [Some(14_100_000), Some(125_000), Some(15_000_000), None],
    [Some(24 * MIB), Some(12 * MIB), Some(128 * MIB), None],
    [Some(MAX_RESVG_TREE_NODES); RESOURCE_PROFILE_COUNT],
    [Some(MAX_RESVG_TREE_DEPTH); RESOURCE_PROFILE_COUNT],
    [Some(24 * MIB), Some(12 * MIB), Some(128 * MIB), None],
];

pub const GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE: RenderResourceProfile =
    merman_model::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE;
pub const CLI_DEFAULT_RESOURCE_PROFILE: RenderResourceProfile =
    merman_model::resources::CLI_DEFAULT_RESOURCE_PROFILE;

pub const fn resource_profile_descriptors() -> &'static [RenderResourceProfileDescriptor] {
    &merman_model::resources::RESOURCE_PROFILE_DESCRIPTORS
}

pub const fn resource_limit_descriptors() -> &'static [ResourceLimitDescriptor] {
    &RESOURCE_LIMIT_DESCRIPTORS
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResourceLimitOverrideError {
    #[error("resource limit id `{0}` is not part of resource contract schema 1")]
    UnknownLimit(String),
    #[error("resource limit `{0}` is a hard implementation capability and cannot be overridden")]
    HardCap(&'static str),
    #[error("resource limit `{0}` must be a positive integer")]
    NonPositive(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "resource policy would loosen `{}`: requested {:?}, ceiling {:?}",
    .id.as_str(),
    .requested,
    .ceiling
)]
pub struct ResourcePolicyRestrictionError {
    pub id: ResourceLimitId,
    pub requested: Option<usize>,
    pub ceiling: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderResourcePolicy {
    input: InputResourcePolicy,
    render_base_values: [Option<usize>; RENDER_RESOURCE_LIMIT_COUNT],
    render_effective_values: [Option<usize>; RENDER_RESOURCE_LIMIT_COUNT],
    render_explicit_overrides: [Option<usize>; RENDER_RESOURCE_LIMIT_COUNT],
}

impl Default for RenderResourcePolicy {
    fn default() -> Self {
        Self::interactive()
    }
}

impl RenderResourcePolicy {
    pub const fn profile(self) -> RenderResourceProfile {
        self.input.profile()
    }

    pub const fn interactive() -> Self {
        Self::for_profile(RenderResourceProfile::Interactive)
    }

    pub const fn constrained() -> Self {
        Self::for_profile(RenderResourceProfile::Constrained)
    }

    pub const fn trusted_native() -> Self {
        Self::for_profile(RenderResourceProfile::TrustedNative)
    }

    pub const fn unbounded_for_trusted_input() -> Self {
        Self::for_profile(RenderResourceProfile::UnboundedForTrustedInput)
    }

    pub const fn for_profile(profile: RenderResourceProfile) -> Self {
        let mut render_values = [None; RENDER_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < RENDER_RESOURCE_LIMIT_COUNT {
            render_values[index] = RENDER_PROFILE_VALUES[index][profile as usize];
            index += 1;
        }
        Self {
            input: InputResourcePolicy::for_profile(profile),
            render_base_values: render_values,
            render_effective_values: render_values,
            render_explicit_overrides: [None; RENDER_RESOURCE_LIMIT_COUNT],
        }
    }

    pub const fn input_policy(&self) -> &InputResourcePolicy {
        &self.input
    }

    pub const fn value(self, id: ResourceLimitId) -> Option<usize> {
        match id {
            ResourceLimitId::Input(id) => self.input.value(id),
            ResourceLimitId::Render(id) => self.render_effective_values[id.index()],
        }
    }

    pub const fn base_value(self, id: ResourceLimitId) -> Option<usize> {
        match id {
            ResourceLimitId::Input(id) => self.input.base_value(id),
            ResourceLimitId::Render(id) => self.render_base_values[id.index()],
        }
    }

    pub const fn explicit_override(self, id: ResourceLimitId) -> Option<usize> {
        match id {
            ResourceLimitId::Input(id) => self.input.explicit_override(id),
            ResourceLimitId::Render(id) => self.render_explicit_overrides[id.index()],
        }
    }

    pub fn explicit_overrides(&self) -> impl Iterator<Item = (ResourceLimitId, usize)> + '_ {
        ResourceLimitId::ALL
            .into_iter()
            .filter_map(|id| self.explicit_override(id).map(|value| (id, value)))
    }

    pub fn apply_override(
        &mut self,
        stable_id: &str,
        value: usize,
    ) -> Result<(), ResourceLimitOverrideError> {
        let id = ResourceLimitId::from_stable_id(stable_id)
            .ok_or_else(|| ResourceLimitOverrideError::UnknownLimit(stable_id.to_string()))?;
        self.apply_limit(id, value)
    }

    pub fn apply_limit(
        &mut self,
        id: ResourceLimitId,
        value: usize,
    ) -> Result<(), ResourceLimitOverrideError> {
        match id {
            ResourceLimitId::Input(id) => {
                self.input
                    .apply_limit(id, value)
                    .map_err(|error| match error {
                        merman_model::resources::InputResourceLimitOverrideError::UnknownLimit(
                            id,
                        ) => ResourceLimitOverrideError::UnknownLimit(id),
                        merman_model::resources::InputResourceLimitOverrideError::NonPositive(
                            id,
                        ) => ResourceLimitOverrideError::NonPositive(id),
                    })
            }
            ResourceLimitId::Render(id) => {
                let descriptor = RENDER_RESOURCE_LIMIT_DESCRIPTORS[id.index()];
                if descriptor.hard_cap || !descriptor.overridable {
                    return Err(ResourceLimitOverrideError::HardCap(descriptor.stable_id));
                }
                if value == 0 {
                    return Err(ResourceLimitOverrideError::NonPositive(
                        descriptor.stable_id,
                    ));
                }
                self.render_effective_values[id.index()] = Some(value);
                self.render_explicit_overrides[id.index()] = Some(value);
                Ok(())
            }
        }
    }

    /// Returns the pointwise minimum of two policies while preserving this policy's host profile.
    pub fn meet(&self, restriction: &Self) -> Self {
        let input = self.input.meet(&restriction.input);
        let mut render_effective_values = [None; RENDER_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < RENDER_RESOURCE_LIMIT_COUNT {
            render_effective_values[index] = minimum_ceiling(
                self.render_effective_values[index],
                restriction.render_effective_values[index],
            );
            index += 1;
        }
        self.with_effective_values(input, render_effective_values)
    }

    /// Applies a request policy only when every requested ceiling is at least as strict.
    pub fn restrict_with(
        &self,
        restriction: &Self,
    ) -> Result<Self, ResourcePolicyRestrictionError> {
        self.input
            .restrict_with(&restriction.input)
            .map_err(map_input_restriction_error)?;
        for id in ResourceLimitId::ALL {
            if matches!(id, ResourceLimitId::Input(_)) {
                continue;
            }
            let ceiling = self.value(id);
            let requested = restriction.value(id);
            if loosens_ceiling(ceiling, requested) {
                return Err(ResourcePolicyRestrictionError {
                    id,
                    requested,
                    ceiling,
                });
            }
        }
        Ok(self.meet(restriction))
    }

    fn with_effective_values(
        &self,
        input: InputResourcePolicy,
        render_effective_values: [Option<usize>; RENDER_RESOURCE_LIMIT_COUNT],
    ) -> Self {
        let mut render_explicit_overrides = [None; RENDER_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < RENDER_RESOURCE_LIMIT_COUNT {
            if render_effective_values[index] != self.render_base_values[index] {
                render_explicit_overrides[index] = render_effective_values[index];
            }
            index += 1;
        }
        Self {
            input,
            render_base_values: self.render_base_values,
            render_effective_values,
            render_explicit_overrides,
        }
    }

    pub fn with_override(
        mut self,
        stable_id: &str,
        value: usize,
    ) -> Result<Self, ResourceLimitOverrideError> {
        self.apply_override(stable_id, value)?;
        Ok(self)
    }

    pub fn with_limit(
        mut self,
        id: ResourceLimitId,
        value: usize,
    ) -> Result<Self, ResourceLimitOverrideError> {
        self.apply_limit(id, value)?;
        Ok(self)
    }

    pub(super) fn check_render_limit(
        &self,
        phase: ResourceLimitPhase,
        id: RenderResourceLimitId,
        actual: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        let Some(max) = self.render_effective_values[id.index()] else {
            return Ok(());
        };
        if actual <= max {
            return Ok(());
        }
        Err(self.ceiling_error(phase, ResourceLimitId::Render(id).as_str(), actual, max))
    }

    #[doc(hidden)]
    pub fn ceiling_error(
        &self,
        phase: ResourceLimitPhase,
        limit: &'static str,
        actual: usize,
        max: usize,
    ) -> ResourceLimitExceeded {
        ResourceLimitExceeded {
            cause: ResourceLimitCause::Ceiling,
            phase,
            limit,
            actual,
            max,
            profile: self.profile(),
            explicit_overrides: self
                .explicit_overrides()
                .map(|(id, value)| ResourceLimitOverride { id, value })
                .collect(),
        }
    }

    pub fn check_source_bytes(&self, source: &str) -> Result<(), ResourceLimitExceeded> {
        self.input
            .check_source_bytes(source)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_render_model(
        &self,
        model: &RenderSemanticModel,
    ) -> Result<(), ResourceLimitExceeded> {
        let complexity = merman_model::resources::model_complexity_from_render_model(model);
        self.check_model_complexity(complexity)
    }

    pub fn check_parsed_render(
        &self,
        parsed: &PresentationDiagram,
    ) -> Result<(), ResourceLimitExceeded> {
        let mut complexity =
            merman_model::resources::model_complexity_from_render_model(parsed.model());
        complexity.text_bytes = complexity
            .text_bytes
            .saturating_add(parsed.retained_render_context_bytes());
        self.check_model_complexity(complexity)?;
        Ok(())
    }

    pub fn check_model_complexity(
        &self,
        complexity: ModelComplexity,
    ) -> Result<(), ResourceLimitExceeded> {
        self.input
            .check_model_complexity(complexity)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_svg_bytes(
        &self,
        svg: &str,
        phase: ResourceLimitPhase,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_svg_byte_count(svg.len(), phase)
    }

    #[doc(hidden)]
    pub fn check_svg_byte_count(
        &self,
        bytes: usize,
        phase: ResourceLimitPhase,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_render_limit(phase, RenderResourceLimitId::MaxSvgBytes, bytes)
    }

    /// Checks native SVG backend tree admission after reference expansion.
    ///
    /// Ordinary SVG producers and browser publication must not inherit these backend hard caps.
    /// Their explicit SVG admission and postprocessing stages use `max_svg_elements` separately.
    pub fn check_svg_structure(
        &self,
        elements: usize,
        tree_depth: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_svg_element_count(elements)?;
        self.check_render_limit(
            ResourceLimitPhase::SvgPostprocess,
            RenderResourceLimitId::SvgBackendTreeNodes,
            elements,
        )?;
        self.check_render_limit(
            ResourceLimitPhase::SvgPostprocess,
            RenderResourceLimitId::SvgBackendTreeDepth,
            tree_depth,
        )
    }

    #[doc(hidden)]
    pub fn check_svg_element_count(&self, elements: usize) -> Result<(), ResourceLimitExceeded> {
        self.check_render_limit(
            ResourceLimitPhase::SvgPostprocess,
            RenderResourceLimitId::MaxSvgElements,
            elements,
        )
    }

    #[doc(hidden)]
    pub fn check_svg_backend_coordinate_magnitude(
        &self,
        value: f64,
    ) -> Result<(), ResourceLimitExceeded> {
        if !value.is_finite() || value.abs() <= MAX_SVG_BACKEND_COORDINATE_MAGNITUDE as f64 {
            return Ok(());
        }
        Err(self.ceiling_error(
            ResourceLimitPhase::SvgPostprocess,
            SVG_BACKEND_COORDINATE_MAGNITUDE_HARD_CAP_ID,
            value.abs().ceil() as usize,
            MAX_SVG_BACKEND_COORDINATE_MAGNITUDE,
        ))
    }

    #[cfg(any(
        feature = "resources-flowchart",
        feature = "resources-swimlane",
        feature = "resources-agentflow"
    ))]
    pub fn check_flowchart_complexity(
        &self,
        model: &FlowchartModel,
    ) -> Result<FlowchartComplexity, ResourceLimitExceeded> {
        self.input
            .check_flowchart_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    #[cfg(feature = "resources-class")]
    pub fn check_class_complexity(
        &self,
        model: &ClassDiagram,
    ) -> Result<ClassComplexity, ResourceLimitExceeded> {
        self.input
            .check_class_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    #[cfg(feature = "resources-mindmap")]
    pub fn check_mindmap_complexity(
        &self,
        model: &MindmapDiagramRenderModel,
    ) -> Result<MindmapComplexity, ResourceLimitExceeded> {
        let complexity = MindmapComplexity::from_model(model);
        self.check_model_complexity(complexity.as_model_complexity())?;
        Ok(complexity)
    }

    #[cfg(feature = "resources-zenuml")]
    pub fn check_zenuml_complexity(
        &self,
        model: &ZenumlDiagramModel,
    ) -> Result<ZenumlComplexity, ResourceLimitExceeded> {
        self.input
            .check_zenuml_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    #[cfg(feature = "resources-sequence")]
    pub fn check_sequence_complexity(
        &self,
        model: &merman_model::families::sequence::SequenceDiagramModel,
    ) -> Result<SequenceComplexity, ResourceLimitExceeded> {
        self.input
            .check_sequence_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_layout_work_units(&self, work_units: usize) -> Result<(), ResourceLimitExceeded> {
        self.check_render_limit(
            ResourceLimitPhase::LayoutModel,
            RenderResourceLimitId::MaxLayoutWorkUnits,
            work_units,
        )
    }

    pub fn check_prepared_text_retained_bytes(
        &self,
        retained_bytes: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_render_limit(
            ResourceLimitPhase::LayoutModel,
            RenderResourceLimitId::MaxPreparedTextRetainedBytes,
            retained_bytes,
        )
    }
}

const fn minimum_ceiling(left: Option<usize>, right: Option<usize>) -> Option<usize> {
    match (left, right) {
        (Some(left), Some(right)) => Some(if left < right { left } else { right }),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

const fn loosens_ceiling(ceiling: Option<usize>, requested: Option<usize>) -> bool {
    match (ceiling, requested) {
        (Some(_), None) => true,
        (Some(ceiling), Some(requested)) => requested > ceiling,
        (None, _) => false,
    }
}

fn map_input_restriction_error(
    error: InputResourcePolicyRestrictionError,
) -> ResourcePolicyRestrictionError {
    ResourcePolicyRestrictionError {
        id: ResourceLimitId::Input(error.id),
        requested: error.requested,
        ceiling: error.ceiling,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResourceLimitExceeded {
    pub cause: ResourceLimitCause,
    pub phase: ResourceLimitPhase,
    pub limit: &'static str,
    pub actual: usize,
    pub max: usize,
    pub profile: RenderResourceProfile,
    pub explicit_overrides: Vec<ResourceLimitOverride>,
}

impl std::fmt::Display for ResourceLimitExceeded {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "resource limit exceeded during {}: {}",
            self.phase, self.limit
        )?;
        if self.cause == ResourceLimitCause::ArithmeticOverflow {
            write!(formatter, " cause={}", self.cause)?;
        }
        write!(formatter, " actual={} max={}", self.actual, self.max)
    }
}

impl std::error::Error for ResourceLimitExceeded {}

impl ResourceLimitExceeded {
    #[doc(hidden)]
    pub fn is_svg_backend_compatibility_ceiling(&self) -> bool {
        matches!(
            self.limit,
            SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID
                | SVG_BACKEND_TREE_NODES_HARD_CAP_ID
                | SVG_BACKEND_COORDINATE_MAGNITUDE_HARD_CAP_ID
        )
    }

    fn from_input(policy: &RenderResourcePolicy, error: InputResourceLimitExceeded) -> Self {
        Self {
            cause: ResourceLimitCause::Ceiling,
            phase: match error.phase {
                InputResourceLimitPhase::Source => ResourceLimitPhase::Source,
                InputResourceLimitPhase::Model => ResourceLimitPhase::LayoutModel,
            },
            limit: error.limit,
            actual: error.actual,
            max: error.max,
            profile: error.profile,
            explicit_overrides: policy
                .explicit_overrides()
                .map(|(id, value)| ResourceLimitOverride { id, value })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimitOverride {
    pub id: ResourceLimitId,
    pub value: usize,
}

#[cfg(test)]
mod tests;

#[cfg(feature = "resources-class")]
pub use merman_model::resources::ClassComplexity;

#[cfg(any(
    feature = "resources-flowchart",
    feature = "resources-swimlane",
    feature = "resources-agentflow"
))]
pub use merman_model::resources::FlowchartComplexity;

#[cfg(feature = "resources-mindmap")]
pub use merman_model::resources::MindmapComplexity;

#[cfg(feature = "resources-sequence")]
pub use merman_model::resources::SequenceComplexity;

#[cfg(feature = "resources-zenuml")]
pub use merman_model::resources::ZenumlComplexity;
