use merman_core::diagrams::flowchart::FlowchartModel;
use merman_core::diagrams::zenuml::{ZenumlDiagramRenderModel, ZenumlStatementKind};
use merman_core::models::class_diagram::ClassDiagram;

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

/// Maximum tree depth accepted by the sealed resvg-compatible SVG contract.
///
/// usvg, resvg, and krilla-svg all recurse through SVG groups. This is an implementation
/// capability rather than a tunable resource policy, so every profile retains a hard cap. Native
/// raster builds execute the recursive backend on a dedicated stack and exercise 256 levels in a
/// checked-in PNG smoke test. WebAssembly keeps a smaller cap because it cannot create that worker
/// stack. This is intentionally independent from any diagram grammar's nesting policy.
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_RESVG_TREE_DEPTH: usize = 256;

#[cfg(target_arch = "wasm32")]
pub const MAX_RESVG_TREE_DEPTH: usize = 64;

const BOUNDED_RESVG_TREE_DEPTH: usize = if MAX_RESVG_TREE_DEPTH < 128 {
    MAX_RESVG_TREE_DEPTH
} else {
    128
};

pub const RESOURCE_CONTRACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceLimitPhase {
    Source,
    LayoutModel,
    SvgOutput,
    SvgPostprocess,
}

impl ResourceLimitPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::LayoutModel => "layout_model",
            Self::SvgOutput => "svg_output",
            Self::SvgPostprocess => "svg_postprocess",
        }
    }
}

impl std::fmt::Display for ResourceLimitPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimitDescriptor {
    pub id: ResourceLimitId,
    pub stable_id: &'static str,
    pub phase: ResourceLimitPhase,
    pub description: &'static str,
    pub overridable: bool,
    pub hard_cap: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderResourceProfileDescriptor {
    pub profile: RenderResourceProfile,
    pub id: &'static str,
    pub purpose: &'static str,
    pub trust_assumption: &'static str,
    pub recommended_binding_default: bool,
    pub limits: ResourceProfileValues,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceProfileValues {
    values: [Option<usize>; RESOURCE_LIMIT_COUNT],
}

impl ResourceProfileValues {
    pub const fn value(self, id: ResourceLimitId) -> Option<usize> {
        self.values[id.index()]
    }

    pub const fn values(self) -> [Option<usize>; RESOURCE_LIMIT_COUNT] {
        self.values
    }
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

macro_rules! define_resource_contract {
    (
        profiles {
            $(
                $profile:ident => {
                    id: $profile_id:literal,
                    purpose: $purpose:literal,
                    trust_assumption: $trust_assumption:literal,
                    recommended_binding_default: $recommended_binding_default:expr,
                }
            ),+ $(,)?
        }
        limits {
            $(
                $limit:ident => {
                    id: $limit_id:literal,
                    phase: $phase:ident,
                    description: $description:literal,
                    overridable: $overridable:expr,
                    hard_cap: $hard_cap:expr,
                    budgets: [$($budget:expr),+ $(,)?],
                }
            ),+ $(,)?
        }
    ) => {
        pub const RESOURCE_PROFILE_COUNT: usize = [$(stringify!($profile)),+].len();
        pub const RESOURCE_LIMIT_COUNT: usize = [$(stringify!($limit)),+].len();

        #[repr(usize)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum RenderResourceProfile {
            $($profile),+
        }

        impl RenderResourceProfile {
            pub const ALL: [Self; RESOURCE_PROFILE_COUNT] = [$(Self::$profile),+];
        }

        #[repr(usize)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum ResourceLimitId {
            $($limit),+
        }

        impl ResourceLimitId {
            pub const ALL: [Self; RESOURCE_LIMIT_COUNT] = [$(Self::$limit),+];

            pub const fn index(self) -> usize {
                self as usize
            }
        }

        const PROFILE_VALUES: [[Option<usize>; RESOURCE_PROFILE_COUNT]; RESOURCE_LIMIT_COUNT] = [
            $([$($budget),+]),+
        ];

        const fn profile_values(profile: RenderResourceProfile) -> ResourceProfileValues {
            let mut values = [None; RESOURCE_LIMIT_COUNT];
            let profile_index = profile as usize;
            let mut index = 0;
            while index < RESOURCE_LIMIT_COUNT {
                values[index] = PROFILE_VALUES[index][profile_index];
                index += 1;
            }
            ResourceProfileValues { values }
        }

        pub static RESOURCE_PROFILE_DESCRIPTORS:
            [RenderResourceProfileDescriptor; RESOURCE_PROFILE_COUNT] = [
                $(RenderResourceProfileDescriptor {
                    profile: RenderResourceProfile::$profile,
                    id: $profile_id,
                    purpose: $purpose,
                    trust_assumption: $trust_assumption,
                    recommended_binding_default: $recommended_binding_default,
                    limits: profile_values(RenderResourceProfile::$profile),
                }),+
            ];

        pub static RESOURCE_LIMIT_DESCRIPTORS:
            [ResourceLimitDescriptor; RESOURCE_LIMIT_COUNT] = [
                $(ResourceLimitDescriptor {
                    id: ResourceLimitId::$limit,
                    stable_id: $limit_id,
                    phase: ResourceLimitPhase::$phase,
                    description: $description,
                    overridable: $overridable,
                    hard_cap: $hard_cap,
                }),+
            ];
    };
}

// This is the authority for resource-limit identity, documentation, phase ownership, and profile
// budgets. The budget order is Interactive, Constrained, TrustedNative, then
// UnboundedForTrustedInput. Platform projections are generated from these descriptors.
define_resource_contract! {
    profiles {
        Interactive => {
            id: "interactive",
            purpose: "General interactive applications and public binding surfaces",
            trust_assumption: "Cooperative user-authored input; not a hostile or multi-tenant isolation boundary",
            recommended_binding_default: true,
        },
        Constrained => {
            id: "constrained",
            purpose: "Constrained rendering for untrusted or publicly submitted documents",
            trust_assumption: "The host must provide timeout, memory, concurrency, and preemption controls",
            recommended_binding_default: false,
        },
        TrustedNative => {
            id: "trusted-native",
            purpose: "Local CLI and controlled native batch rendering",
            trust_assumption: "Input is trusted and the native host controls the workload",
            recommended_binding_default: false,
        },
        UnboundedForTrustedInput => {
            id: "unbounded-for-trusted-input",
            purpose: "Explicitly disable policy budgets while retaining hard backend capabilities",
            trust_assumption: "Input is fully trusted and the host provides outer isolation",
            recommended_binding_default: false,
        },
    }
    limits {
        MaxSourceBytes => {
            id: "max_source_bytes",
            phase: Source,
            description: "Maximum UTF-8 Mermaid source bytes",
            overridable: true,
            hard_cap: false,
            budgets: [Some(2 * MIB), Some(MIB), Some(16 * MIB), None],
        },
        MaxSvgBytes => {
            id: "max_svg_bytes",
            phase: SvgOutput,
            description: "Maximum serialized SVG bytes",
            overridable: true,
            hard_cap: false,
            budgets: [Some(24 * MIB), Some(12 * MIB), Some(128 * MIB), None],
        },
        MaxSvgElements => {
            id: "max_svg_elements",
            phase: SvgPostprocess,
            description: "Maximum SVG element count",
            overridable: true,
            hard_cap: false,
            budgets: [Some(250_000), Some(125_000), Some(1_000_000), None],
        },
        MaxSvgTreeDepth => {
            id: "max_svg_tree_depth",
            phase: SvgPostprocess,
            description: "Maximum tree depth supported by recursive SVG backends",
            overridable: false,
            hard_cap: true,
            budgets: [
                Some(BOUNDED_RESVG_TREE_DEPTH),
                Some(BOUNDED_RESVG_TREE_DEPTH),
                Some(MAX_RESVG_TREE_DEPTH),
                Some(MAX_RESVG_TREE_DEPTH),
            ],
        },
        MaxFlowchartNodes => {
            id: "max_flowchart_nodes",
            phase: LayoutModel,
            description: "Maximum Flowchart nodes including subgraphs",
            overridable: true,
            hard_cap: false,
            budgets: [Some(8_000), Some(4_000), Some(50_000), None],
        },
        MaxFlowchartEdges => {
            id: "max_flowchart_edges",
            phase: LayoutModel,
            description: "Maximum Flowchart edges",
            overridable: true,
            hard_cap: false,
            budgets: [Some(16_000), Some(8_000), Some(100_000), None],
        },
        MaxFlowchartSubgraphs => {
            id: "max_flowchart_subgraphs",
            phase: LayoutModel,
            description: "Maximum Flowchart subgraphs",
            overridable: true,
            hard_cap: false,
            budgets: [Some(2_000), Some(1_000), Some(10_000), None],
        },
        MaxClassNodes => {
            id: "max_class_nodes",
            phase: LayoutModel,
            description: "Maximum Class diagram nodes",
            overridable: true,
            hard_cap: false,
            budgets: [Some(8_000), Some(4_000), Some(50_000), None],
        },
        MaxClassEdges => {
            id: "max_class_edges",
            phase: LayoutModel,
            description: "Maximum Class diagram edges",
            overridable: true,
            hard_cap: false,
            budgets: [Some(16_000), Some(8_000), Some(100_000), None],
        },
        MaxClassNamespaces => {
            id: "max_class_namespaces",
            phase: LayoutModel,
            description: "Maximum Class diagram namespaces",
            overridable: true,
            hard_cap: false,
            budgets: [Some(2_000), Some(1_000), Some(10_000), None],
        },
        MaxZenumlParticipants => {
            id: "max_zenuml_participants",
            phase: LayoutModel,
            description: "Maximum ZenUML participants",
            overridable: true,
            hard_cap: false,
            budgets: [Some(8_000), Some(4_000), Some(50_000), None],
        },
        MaxZenumlStatements => {
            id: "max_zenuml_statements",
            phase: LayoutModel,
            description: "Maximum ZenUML statements",
            overridable: true,
            hard_cap: false,
            budgets: [Some(16_000), Some(8_000), Some(100_000), None],
        },
        MaxZenumlFragments => {
            id: "max_zenuml_fragments",
            phase: LayoutModel,
            description: "Maximum ZenUML fragments and groups",
            overridable: true,
            hard_cap: false,
            budgets: [Some(2_000), Some(1_000), Some(10_000), None],
        },
        MaxVennAreas => {
            id: "max_venn_areas",
            phase: LayoutModel,
            description: "Maximum Venn source and synthesized layout areas",
            overridable: true,
            hard_cap: false,
            budgets: [Some(8_000), Some(4_000), Some(50_000), None],
        },
        MaxSwimlaneLineHopSegmentPairs => {
            id: "max_swimlane_line_hop_segment_pairs",
            phase: SvgOutput,
            description: "Maximum broad-phase segment pairs inspected for Swimlane line hops",
            overridable: true,
            hard_cap: false,
            budgets: [Some(250_000), Some(125_000), Some(1_000_000), None],
        },
        MaxLabelBytes => {
            id: "max_label_bytes",
            phase: LayoutModel,
            description: "Maximum aggregate model label bytes",
            overridable: true,
            hard_cap: false,
            budgets: [Some(2 * MIB), Some(MIB), Some(16 * MIB), None],
        },
    }
}

pub const GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE: RenderResourceProfile =
    RenderResourceProfile::Interactive;
pub const CLI_DEFAULT_RESOURCE_PROFILE: RenderResourceProfile =
    RenderResourceProfile::TrustedNative;

impl RenderResourceProfile {
    pub const fn descriptor(self) -> &'static RenderResourceProfileDescriptor {
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
}

impl std::fmt::Display for RenderResourceProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

impl std::str::FromStr for RenderResourceProfile {
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

impl ResourceLimitId {
    pub fn from_stable_id(id: &str) -> Option<Self> {
        RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.stable_id == id)
            .map(|descriptor| descriptor.id)
    }

    pub const fn descriptor(self) -> &'static ResourceLimitDescriptor {
        &RESOURCE_LIMIT_DESCRIPTORS[self.index()]
    }

    pub const fn as_str(self) -> &'static str {
        self.descriptor().stable_id
    }
}

pub const fn resource_profile_descriptors() -> &'static [RenderResourceProfileDescriptor] {
    &RESOURCE_PROFILE_DESCRIPTORS
}

pub const fn resource_limit_descriptors() -> &'static [ResourceLimitDescriptor] {
    &RESOURCE_LIMIT_DESCRIPTORS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderResourcePolicy {
    profile: RenderResourceProfile,
    base_values: [Option<usize>; RESOURCE_LIMIT_COUNT],
    effective_values: [Option<usize>; RESOURCE_LIMIT_COUNT],
    explicit_overrides: [Option<usize>; RESOURCE_LIMIT_COUNT],
}

impl Default for RenderResourcePolicy {
    fn default() -> Self {
        Self::interactive()
    }
}

impl RenderResourcePolicy {
    pub const fn profile(self) -> RenderResourceProfile {
        self.profile
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
        let base_values = profile.descriptor().limits.values();
        Self {
            profile,
            base_values,
            effective_values: base_values,
            explicit_overrides: [None; RESOURCE_LIMIT_COUNT],
        }
    }

    pub const fn value(self, id: ResourceLimitId) -> Option<usize> {
        self.effective_values[id.index()]
    }

    pub const fn base_value(self, id: ResourceLimitId) -> Option<usize> {
        self.base_values[id.index()]
    }

    pub const fn explicit_override(self, id: ResourceLimitId) -> Option<usize> {
        self.explicit_overrides[id.index()]
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
        let descriptor = id.descriptor();
        if descriptor.hard_cap || !descriptor.overridable {
            return Err(ResourceLimitOverrideError::HardCap(descriptor.stable_id));
        }
        if value == 0 {
            return Err(ResourceLimitOverrideError::NonPositive(
                descriptor.stable_id,
            ));
        }
        self.effective_values[id.index()] = Some(value);
        self.explicit_overrides[id.index()] = Some(value);
        Ok(())
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

    fn check_limit(
        &self,
        phase: ResourceLimitPhase,
        id: ResourceLimitId,
        actual: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        let Some(max) = self.value(id) else {
            return Ok(());
        };
        if actual <= max {
            return Ok(());
        }
        Err(ResourceLimitExceeded {
            phase,
            limit: id.as_str(),
            actual,
            max,
            profile: self.profile,
            explicit_overrides: self
                .explicit_overrides()
                .map(|(id, value)| ResourceLimitOverride { id, value })
                .collect(),
        })
    }

    pub fn check_source_bytes(&self, source: &str) -> Result<(), ResourceLimitExceeded> {
        self.check_limit(
            ResourceLimitPhase::Source,
            ResourceLimitId::MaxSourceBytes,
            source.len(),
        )
    }

    pub fn check_svg_bytes(
        &self,
        svg: &str,
        phase: ResourceLimitPhase,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_limit(phase, ResourceLimitId::MaxSvgBytes, svg.len())
    }

    pub fn check_svg_structure(
        &self,
        elements: usize,
        tree_depth: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_limit(
            ResourceLimitPhase::SvgPostprocess,
            ResourceLimitId::MaxSvgElements,
            elements,
        )?;
        self.check_limit(
            ResourceLimitPhase::SvgPostprocess,
            ResourceLimitId::MaxSvgTreeDepth,
            tree_depth,
        )
    }

    pub fn check_flowchart_complexity(
        &self,
        model: &FlowchartModel,
    ) -> Result<FlowchartComplexity, ResourceLimitExceeded> {
        let complexity = FlowchartComplexity::from_model(model);
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxFlowchartNodes,
            complexity.nodes,
        )?;
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxFlowchartEdges,
            complexity.edges,
        )?;
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxFlowchartSubgraphs,
            complexity.subgraphs,
        )?;
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxLabelBytes,
            complexity.label_bytes,
        )?;
        Ok(complexity)
    }

    pub fn check_class_complexity(
        &self,
        model: &ClassDiagram,
    ) -> Result<ClassComplexity, ResourceLimitExceeded> {
        let complexity = ClassComplexity::from_model(model);
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxClassNodes,
            complexity.nodes,
        )?;
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxClassEdges,
            complexity.edges,
        )?;
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxClassNamespaces,
            complexity.namespaces,
        )?;
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxLabelBytes,
            complexity.label_bytes,
        )?;
        Ok(complexity)
    }

    pub fn check_zenuml_complexity(
        &self,
        model: &ZenumlDiagramRenderModel,
    ) -> Result<ZenumlComplexity, ResourceLimitExceeded> {
        let complexity = ZenumlComplexity::from_model(model);
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxZenumlParticipants,
            complexity.participants,
        )?;
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxZenumlStatements,
            complexity.statements,
        )?;
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxZenumlFragments,
            complexity.fragments,
        )?;
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxLabelBytes,
            complexity.label_bytes,
        )?;
        Ok(complexity)
    }

    pub fn check_venn_areas(&self, areas: usize) -> Result<(), ResourceLimitExceeded> {
        self.check_limit(
            ResourceLimitPhase::LayoutModel,
            ResourceLimitId::MaxVennAreas,
            areas,
        )
    }

    pub fn check_swimlane_line_hop_segment_pairs(
        &self,
        segment_pairs: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_limit(
            ResourceLimitPhase::SvgOutput,
            ResourceLimitId::MaxSwimlaneLineHopSegmentPairs,
            segment_pairs,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowchartComplexity {
    pub nodes: usize,
    pub edges: usize,
    pub subgraphs: usize,
    pub label_bytes: usize,
}

impl FlowchartComplexity {
    pub fn from_model(model: &FlowchartModel) -> Self {
        let node_label_bytes = model
            .nodes
            .iter()
            .map(|node| optional_str_len(node.label.as_deref()) + node.id.len())
            .sum::<usize>();
        let edge_label_bytes = model
            .edges
            .iter()
            .map(|edge| {
                optional_str_len(edge.label.as_deref())
                    + edge.id.len()
                    + edge.from.len()
                    + edge.to.len()
            })
            .sum::<usize>();
        let subgraph_label_bytes = model
            .subgraphs
            .iter()
            .map(|subgraph| subgraph.id.len() + subgraph.title.len())
            .sum::<usize>();
        let tooltip_bytes = model.tooltips.values().map(String::len).sum::<usize>();

        Self {
            nodes: model.nodes.len().saturating_add(model.subgraphs.len()),
            edges: model.edges.len(),
            subgraphs: model.subgraphs.len(),
            label_bytes: node_label_bytes
                .saturating_add(edge_label_bytes)
                .saturating_add(subgraph_label_bytes)
                .saturating_add(tooltip_bytes),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassComplexity {
    pub nodes: usize,
    pub edges: usize,
    pub namespaces: usize,
    pub label_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZenumlComplexity {
    pub participants: usize,
    pub statements: usize,
    pub fragments: usize,
    pub label_bytes: usize,
}

impl ZenumlComplexity {
    pub fn from_model(model: &ZenumlDiagramRenderModel) -> Self {
        let common_label_bytes = [
            model.title.as_deref(),
            model.acc_title.as_deref(),
            model.acc_descr.as_deref(),
            model.starter.as_deref(),
        ]
        .into_iter()
        .flatten()
        .fold(0usize, |total, value| total.saturating_add(value.len()));
        let participant_label_bytes =
            model
                .participants
                .iter()
                .fold(0usize, |total, participant| {
                    [
                        Some(participant.name.as_str()),
                        participant.label.as_deref(),
                        participant.participant_type.as_deref(),
                        participant.stereotype.as_deref(),
                        participant.emoji.as_deref(),
                        participant.width_source.as_deref(),
                        participant.color.as_deref(),
                        participant.group_id.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    .fold(total, |subtotal, value| {
                        subtotal.saturating_add(value.len())
                    })
                });
        let group_label_bytes = model.groups.iter().fold(0usize, |total, group| {
            group.participant_names.iter().fold(
                total.saturating_add(group.id.as_deref().map_or(0, str::len)),
                |subtotal, name| subtotal.saturating_add(name.len()),
            )
        });
        let mut complexity = Self {
            participants: model.participants.len(),
            statements: 0,
            fragments: 0,
            label_bytes: common_label_bytes
                .saturating_add(participant_label_bytes)
                .saturating_add(group_label_bytes),
        };
        let mut pending = vec![model.statements.as_slice()];
        while let Some(statements) = pending.pop() {
            for statement in statements {
                complexity.statements = complexity.statements.saturating_add(1);
                complexity.label_bytes = complexity
                    .label_bytes
                    .saturating_add(statement.comment.as_deref().map_or(0, str::len));
                match &statement.kind {
                    ZenumlStatementKind::Message {
                        explicit_from,
                        resolved_from,
                        resolved_to,
                        label,
                        assignment,
                        body,
                        ..
                    } => {
                        complexity.label_bytes = complexity
                            .label_bytes
                            .saturating_add(explicit_from.as_deref().map_or(0, str::len))
                            .saturating_add(resolved_from.as_deref().map_or(0, str::len))
                            .saturating_add(resolved_to.as_deref().map_or(0, str::len))
                            .saturating_add(label.len())
                            .saturating_add(assignment.as_deref().map_or(0, str::len));
                        pending.push(body);
                    }
                    ZenumlStatementKind::Creation {
                        resolved_from,
                        resolved_to,
                        constructor,
                        parameters,
                        assignment,
                        label,
                        body,
                        ..
                    } => {
                        complexity.label_bytes = complexity
                            .label_bytes
                            .saturating_add(resolved_from.as_deref().map_or(0, str::len))
                            .saturating_add(resolved_to.len())
                            .saturating_add(constructor.len())
                            .saturating_add(parameters.len())
                            .saturating_add(assignment.as_deref().map_or(0, str::len))
                            .saturating_add(label.len());
                        pending.push(body);
                    }
                    ZenumlStatementKind::Return {
                        explicit_from,
                        resolved_from,
                        explicit_to,
                        resolved_to,
                        label,
                    } => {
                        complexity.label_bytes = complexity
                            .label_bytes
                            .saturating_add(explicit_from.as_deref().map_or(0, str::len))
                            .saturating_add(resolved_from.as_deref().map_or(0, str::len))
                            .saturating_add(explicit_to.as_deref().map_or(0, str::len))
                            .saturating_add(resolved_to.as_deref().map_or(0, str::len))
                            .saturating_add(label.len());
                    }
                    ZenumlStatementKind::Fragment {
                        label, sections, ..
                    } => {
                        complexity.fragments = complexity.fragments.saturating_add(1);
                        complexity.label_bytes = complexity
                            .label_bytes
                            .saturating_add(label.as_deref().map_or(0, str::len));
                        for section in sections {
                            complexity.label_bytes = complexity
                                .label_bytes
                                .saturating_add(section.label.as_deref().map_or(0, str::len));
                            pending.push(&section.statements);
                        }
                    }
                    ZenumlStatementKind::Reference {
                        participants,
                        label,
                    } => {
                        complexity.label_bytes = participants.iter().fold(
                            complexity.label_bytes.saturating_add(label.len()),
                            |total, participant| total.saturating_add(participant.len()),
                        );
                    }
                    ZenumlStatementKind::Divider { label } => {
                        complexity.label_bytes = complexity.label_bytes.saturating_add(label.len());
                    }
                }
            }
        }
        complexity
    }
}

impl ClassComplexity {
    pub fn from_model(model: &ClassDiagram) -> Self {
        let class_label_bytes = model
            .classes
            .values()
            .map(|node| {
                node.id
                    .len()
                    .saturating_add(node.label.len())
                    .saturating_add(node.text.len())
                    .saturating_add(node.type_param.len())
                    .saturating_add(node.css_classes.len())
                    .saturating_add(node.tooltip.as_deref().map(str::len).unwrap_or(0))
                    .saturating_add(node.link.as_deref().map(str::len).unwrap_or(0))
                    .saturating_add(
                        node.members
                            .iter()
                            .chain(node.methods.iter())
                            .map(|member| {
                                member
                                    .display_text
                                    .len()
                                    .saturating_add(member.id.len())
                                    .saturating_add(member.parameters.len())
                                    .saturating_add(member.return_type.len())
                            })
                            .sum::<usize>(),
                    )
                    .saturating_add(node.annotations.iter().map(String::len).sum::<usize>())
                    .saturating_add(node.styles.iter().map(String::len).sum::<usize>())
            })
            .sum::<usize>();
        let relation_label_bytes = model
            .relations
            .iter()
            .map(|rel| {
                rel.id
                    .len()
                    .saturating_add(rel.id1.len())
                    .saturating_add(rel.id2.len())
                    .saturating_add(rel.title.len())
                    .saturating_add(rel.relation_title_1.len())
                    .saturating_add(rel.relation_title_2.len())
            })
            .sum::<usize>();
        let note_label_bytes = model
            .notes
            .iter()
            .map(|note| {
                note.id
                    .len()
                    .saturating_add(note.text.len())
                    .saturating_add(note.class_id.as_deref().map(str::len).unwrap_or(0))
            })
            .sum::<usize>();
        let interface_label_bytes = model
            .interfaces
            .iter()
            .map(|iface| {
                iface
                    .id
                    .len()
                    .saturating_add(iface.label.len())
                    .saturating_add(iface.class_id.len())
            })
            .sum::<usize>();
        let namespace_label_bytes = model
            .namespaces
            .values()
            .map(|namespace| {
                namespace
                    .id
                    .len()
                    .saturating_add(namespace.label.len())
                    .saturating_add(namespace.parent.as_deref().map(str::len).unwrap_or(0))
            })
            .sum::<usize>();

        Self {
            nodes: model
                .classes
                .len()
                .saturating_add(model.notes.len())
                .saturating_add(model.interfaces.len())
                .saturating_add(model.namespaces.len()),
            edges: model.relations.len().saturating_add(
                model
                    .notes
                    .iter()
                    .filter(|note| note.class_id.is_some())
                    .count(),
            ),
            namespaces: model.namespaces.len(),
            label_bytes: class_label_bytes
                .saturating_add(relation_label_bytes)
                .saturating_add(note_label_bytes)
                .saturating_add(interface_label_bytes)
                .saturating_add(namespace_label_bytes),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("resource limit exceeded during {phase}: {limit} actual={actual} max={max}")]
pub struct ResourceLimitExceeded {
    pub phase: ResourceLimitPhase,
    pub limit: &'static str,
    pub actual: usize,
    pub max: usize,
    pub profile: RenderResourceProfile,
    pub explicit_overrides: Vec<ResourceLimitOverride>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimitOverride {
    pub id: ResourceLimitId,
    pub value: usize,
}

fn optional_str_len(value: Option<&str>) -> usize {
    value.map(str::len).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::diagrams::flowchart::{FlowEdge, FlowNode, FlowSubgraph};
    use merman_core::{Engine, ParseOptions, RenderSemanticModel};
    use std::collections::HashSet;

    struct ZenumlLimitCase {
        name: &'static str,
        id: ResourceLimitId,
        value: usize,
    }

    #[test]
    fn resource_contract_is_complete_unique_and_drives_every_profile() {
        assert_eq!(RESOURCE_CONTRACT_SCHEMA_VERSION, 1);
        assert_eq!(
            RESOURCE_PROFILE_DESCRIPTORS.len(),
            RenderResourceProfile::ALL.len()
        );
        assert_eq!(RESOURCE_LIMIT_DESCRIPTORS.len(), 16);

        let profile_ids = RESOURCE_PROFILE_DESCRIPTORS
            .iter()
            .map(|descriptor| descriptor.id)
            .collect::<HashSet<_>>();
        assert_eq!(profile_ids.len(), RESOURCE_PROFILE_DESCRIPTORS.len());
        assert_eq!(
            RESOURCE_PROFILE_DESCRIPTORS
                .iter()
                .filter(|descriptor| descriptor.recommended_binding_default)
                .map(|descriptor| descriptor.profile)
                .collect::<Vec<_>>(),
            vec![GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE]
        );
        for profile in RenderResourceProfile::ALL {
            let descriptor = profile.descriptor();
            assert_eq!(RenderResourceProfile::from_id(descriptor.id), Some(profile));
            for limit in RESOURCE_LIMIT_DESCRIPTORS {
                assert_eq!(
                    RenderResourcePolicy::for_profile(profile).value(limit.id),
                    descriptor.limits.value(limit.id)
                );
                if limit.hard_cap {
                    assert!(!limit.overridable);
                    assert!(descriptor.limits.value(limit.id).is_some());
                }
            }
        }

        let limit_ids = RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .map(|descriptor| descriptor.stable_id)
            .collect::<HashSet<_>>();
        assert_eq!(limit_ids.len(), RESOURCE_LIMIT_DESCRIPTORS.len());
        for descriptor in RESOURCE_LIMIT_DESCRIPTORS {
            assert_eq!(
                ResourceLimitId::from_stable_id(descriptor.stable_id),
                Some(descriptor.id)
            );
            assert_eq!(descriptor.id.descriptor(), &descriptor);
        }
    }

    #[test]
    fn resource_overrides_fail_closed_for_unknown_ids_and_hard_caps() {
        let mut limits = RenderResourcePolicy::interactive();
        assert!(matches!(
            limits.apply_override("future_limit", 1),
            Err(ResourceLimitOverrideError::UnknownLimit(_))
        ));
        assert_eq!(
            limits.apply_override("max_svg_tree_depth", 1),
            Err(ResourceLimitOverrideError::HardCap("max_svg_tree_depth"))
        );
        assert_eq!(
            limits.apply_override("max_svg_elements", 0),
            Err(ResourceLimitOverrideError::NonPositive("max_svg_elements"))
        );
        limits.apply_override("max_svg_elements", 7).unwrap();
        assert_eq!(limits.value(ResourceLimitId::MaxSvgElements), Some(7));
    }

    #[test]
    fn source_limit_reports_structured_error() {
        let err = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSourceBytes, 4)
            .unwrap()
            .check_source_bytes("12345")
            .unwrap_err();

        assert_eq!(err.phase, ResourceLimitPhase::Source);
        assert_eq!(err.limit, "max_source_bytes");
        assert_eq!(err.actual, 5);
        assert_eq!(err.max, 4);
    }

    #[test]
    fn derived_layout_work_limits_report_the_owned_phase_and_metric() {
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxVennAreas, 2)
            .unwrap()
            .with_limit(ResourceLimitId::MaxSwimlaneLineHopSegmentPairs, 3)
            .unwrap();

        let venn = limits.check_venn_areas(3).unwrap_err();
        assert_eq!(venn.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(venn.limit, "max_venn_areas");
        assert_eq!(venn.actual, 3);
        assert_eq!(venn.max, 2);

        let line_hops = limits.check_swimlane_line_hop_segment_pairs(4).unwrap_err();
        assert_eq!(line_hops.phase, ResourceLimitPhase::SvgOutput);
        assert_eq!(line_hops.limit, "max_swimlane_line_hop_segment_pairs");
        assert_eq!(line_hops.actual, 4);
        assert_eq!(line_hops.max, 3);
    }

    #[test]
    fn zenuml_complexity_includes_common_and_inline_decorations() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "zenuml\naccTitle: Access title\naccDescr: Access description\nA->[rocket]B.call()\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Zenuml(model) = parsed.model() else {
            panic!("expected ZenUML model");
        };
        let complexity = ZenumlComplexity::from_model(model);

        assert_eq!(complexity.participants, 2);
        assert_eq!(complexity.statements, 1);
        let required = ["Access title", "Access description", "rocket", "call()"]
            .into_iter()
            .map(str::len)
            .sum::<usize>();
        assert!(complexity.label_bytes >= required);
    }

    #[test]
    fn zenuml_structural_limits_are_owned_and_independent() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "zenuml\nA.call() {\n  if(ok) {\n    if(inner) {\n      B.work()\n    }\n  }\n}\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Zenuml(model) = parsed.model() else {
            panic!("expected ZenUML model");
        };

        let cases = [
            ZenumlLimitCase {
                name: "max_zenuml_participants",
                id: ResourceLimitId::MaxZenumlParticipants,
                value: 1,
            },
            ZenumlLimitCase {
                name: "max_zenuml_statements",
                id: ResourceLimitId::MaxZenumlStatements,
                value: 1,
            },
            ZenumlLimitCase {
                name: "max_zenuml_fragments",
                id: ResourceLimitId::MaxZenumlFragments,
                value: 1,
            },
        ];
        for case in cases {
            let limits = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(case.id, case.value)
                .unwrap();
            let error = limits.check_zenuml_complexity(model).unwrap_err();
            assert_eq!(error.phase, ResourceLimitPhase::LayoutModel);
            assert_eq!(error.limit, case.name);
        }
    }

    #[test]
    fn flowchart_complexity_counts_layout_nodes_and_labels() {
        let model = FlowchartModel {
            keyword: "graph".to_string(),
            acc_descr: None,
            acc_title: None,
            class_defs: Default::default(),
            direction: None,
            edge_defaults: None,
            vertex_calls: Vec::new(),
            nodes: vec![FlowNode {
                id: "A".to_string(),
                label: Some("Alpha".to_string()),
                label_type: None,
                layout_shape: None,
                shape: None,
                icon: None,
                form: None,
                pos: None,
                img: None,
                constraint: None,
                asset_width: None,
                asset_height: None,
                classes: Vec::new(),
                styles: Vec::new(),
                link: None,
                link_target: None,
                have_callback: false,
            }],
            edges: vec![FlowEdge {
                id: "L-A-B".to_string(),
                from: "A".to_string(),
                to: "B".to_string(),
                label: Some("edge".to_string()),
                label_type: None,
                edge_type: None,
                arrow: "-->".to_string(),
                is_user_defined_id: false,
                stroke: None,
                interpolate: None,
                classes: Vec::new(),
                style: Vec::new(),
                animate: None,
                animation: None,
                length: 1,
            }],
            subgraphs: vec![FlowSubgraph {
                id: "cluster".to_string(),
                title: "Cluster".to_string(),
                dir: None,
                has_explicit_dir: false,
                label_type: None,
                classes: Vec::new(),
                styles: Vec::new(),
                nodes: vec!["A".to_string()],
            }],
            tooltips: Default::default(),
            warning_facts: Vec::new(),
        };

        let complexity = FlowchartComplexity::from_model(&model);
        assert_eq!(complexity.nodes, 2);
        assert_eq!(complexity.edges, 1);
        assert_eq!(complexity.subgraphs, 1);
        assert!(complexity.label_bytes >= "AlphaedgeCluster".len());
    }
}
