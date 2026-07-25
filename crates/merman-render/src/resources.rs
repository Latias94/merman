use merman_core::RenderSemanticModel;
use merman_core::diagrams::flowchart::FlowchartModel;
use merman_core::diagrams::mindmap::MindmapDiagramRenderModel;
use merman_core::diagrams::zenuml::ZenumlDiagramRenderModel;
use merman_core::models::class_diagram::ClassDiagram;
pub use merman_core::resources::{
    ClassComplexity, FlowchartComplexity, MindmapComplexity, ModelComplexity,
    RESOURCE_PROFILE_DESCRIPTORS, ResourceProfile as RenderResourceProfile,
    ResourceProfileDescriptor as RenderResourceProfileDescriptor, SequenceComplexity,
    ZenumlComplexity,
};
use merman_core::resources::{
    InputResourceLimitExceeded, InputResourceLimitId, InputResourceLimitPhase, InputResourcePolicy,
};

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

#[cfg(not(target_arch = "wasm32"))]
pub const MAX_RESVG_TREE_DEPTH: usize = 256;
pub const SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID: &str = "svg_backend_tree_depth";

#[cfg(target_arch = "wasm32")]
pub const MAX_RESVG_TREE_DEPTH: usize = 64;

// Backend capability for recursively owned typed/compatibility trees, not a Mermaid syntax limit.
// It remains active when policy budgets are disabled because increasing it is not stack-safe.
#[cfg(not(target_arch = "wasm32"))]
const MAX_RECURSIVE_MODEL_TREE_DEPTH: usize = merman_core::MAX_DIAGRAM_NESTING_DEPTH;

#[cfg(target_arch = "wasm32")]
const MAX_RECURSIVE_MODEL_TREE_DEPTH: usize = 64;

pub const RESOURCE_PROFILE_COUNT: usize = merman_core::resources::RESOURCE_PROFILE_COUNT;
const RENDER_RESOURCE_LIMIT_COUNT: usize = 3;
pub const RESOURCE_LIMIT_COUNT: usize =
    merman_core::resources::INPUT_RESOURCE_LIMIT_COUNT + RENDER_RESOURCE_LIMIT_COUNT;

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

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderResourceLimitId {
    MaxSvgBytes,
    MaxSvgElements,
    MaxLayoutWorkUnits,
}

impl RenderResourceLimitId {
    pub const ALL: [Self; RENDER_RESOURCE_LIMIT_COUNT] = [
        Self::MaxSvgBytes,
        Self::MaxSvgElements,
        Self::MaxLayoutWorkUnits,
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
    pub const MaxSvgBytes: Self = Self::Render(RenderResourceLimitId::MaxSvgBytes);
    pub const MaxSvgElements: Self = Self::Render(RenderResourceLimitId::MaxSvgElements);
    pub const MaxLayoutWorkUnits: Self = Self::Render(RenderResourceLimitId::MaxLayoutWorkUnits);

    pub const ALL: [Self; RESOURCE_LIMIT_COUNT] = [
        Self::MaxSourceBytes,
        Self::MaxModelItems,
        Self::MaxModelTextBytes,
        Self::MaxModelNestingDepth,
        Self::MaxLayoutWorkUnits,
        Self::MaxSvgBytes,
        Self::MaxSvgElements,
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
pub struct ResourceLimitDescriptor {
    pub id: ResourceLimitId,
    pub stable_id: &'static str,
    pub phase: ResourceLimitPhase,
    pub description: &'static str,
    pub overridable: bool,
    pub hard_cap: bool,
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
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxSvgElements,
        stable_id: "max_svg_elements",
        phase: ResourceLimitPhase::SvgPostprocess,
        description: "Maximum SVG element count",
        overridable: true,
        hard_cap: false,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxLayoutWorkUnits,
        stable_id: "max_layout_work_units",
        phase: ResourceLimitPhase::LayoutModel,
        description: "Maximum family-accounted layout work units before SVG generation",
        overridable: true,
        hard_cap: false,
    },
];

pub static RESOURCE_LIMIT_DESCRIPTORS: [ResourceLimitDescriptor; RESOURCE_LIMIT_COUNT] = [
    input_descriptor(InputResourceLimitId::MaxSourceBytes),
    input_descriptor(InputResourceLimitId::MaxModelItems),
    input_descriptor(InputResourceLimitId::MaxModelTextBytes),
    input_descriptor(InputResourceLimitId::MaxModelNestingDepth),
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[2],
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[0],
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[1],
];

const RENDER_PROFILE_VALUES: [[Option<usize>; RESOURCE_PROFILE_COUNT];
    RENDER_RESOURCE_LIMIT_COUNT] = [
    [Some(24 * MIB), Some(12 * MIB), Some(128 * MIB), None],
    [Some(250_000), Some(125_000), Some(1_000_000), None],
    // A policy budget, not a Mermaid limit. Families charge deterministic
    // units for derived geometry and inspected layout candidates.
    [Some(250_000), Some(125_000), Some(1_000_000), None],
];

pub const GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE: RenderResourceProfile =
    merman_core::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE;
pub const CLI_DEFAULT_RESOURCE_PROFILE: RenderResourceProfile =
    merman_core::resources::CLI_DEFAULT_RESOURCE_PROFILE;

pub const fn resource_profile_descriptors() -> &'static [RenderResourceProfileDescriptor] {
    &merman_core::resources::RESOURCE_PROFILE_DESCRIPTORS
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
                        merman_core::resources::InputResourceLimitOverrideError::UnknownLimit(
                            id,
                        ) => ResourceLimitOverrideError::UnknownLimit(id),
                        merman_core::resources::InputResourceLimitOverrideError::NonPositive(
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

    fn check_render_limit(
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
        let limit = ResourceLimitId::Render(id);
        Err(ResourceLimitExceeded {
            phase,
            limit: limit.as_str(),
            actual,
            max,
            profile: self.profile(),
            explicit_overrides: self
                .explicit_overrides()
                .map(|(id, value)| ResourceLimitOverride { id, value })
                .collect(),
        })
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
        let complexity = ModelComplexity::from_render_model(model);
        self.check_model_complexity(complexity)?;

        if matches!(
            model,
            RenderSemanticModel::Treemap(_) | RenderSemanticModel::Ishikawa(_)
        ) && complexity.nesting_depth > MAX_RECURSIVE_MODEL_TREE_DEPTH
        {
            return Err(ResourceLimitExceeded {
                phase: ResourceLimitPhase::LayoutModel,
                limit: "typed_model_tree_depth",
                actual: complexity.nesting_depth,
                max: MAX_RECURSIVE_MODEL_TREE_DEPTH,
                profile: self.profile(),
                explicit_overrides: self
                    .explicit_overrides()
                    .map(|(id, value)| ResourceLimitOverride { id, value })
                    .collect(),
            });
        }

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
        self.check_render_limit(phase, RenderResourceLimitId::MaxSvgBytes, svg.len())
    }

    pub fn check_svg_structure(
        &self,
        elements: usize,
        tree_depth: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_render_limit(
            ResourceLimitPhase::SvgPostprocess,
            RenderResourceLimitId::MaxSvgElements,
            elements,
        )?;
        if tree_depth <= MAX_RESVG_TREE_DEPTH {
            return Ok(());
        }
        Err(ResourceLimitExceeded {
            phase: ResourceLimitPhase::SvgPostprocess,
            limit: SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID,
            actual: tree_depth,
            max: MAX_RESVG_TREE_DEPTH,
            profile: self.profile(),
            explicit_overrides: self
                .explicit_overrides()
                .map(|(id, value)| ResourceLimitOverride { id, value })
                .collect(),
        })
    }

    pub fn check_flowchart_complexity(
        &self,
        model: &FlowchartModel,
    ) -> Result<FlowchartComplexity, ResourceLimitExceeded> {
        self.input
            .check_flowchart_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_class_complexity(
        &self,
        model: &ClassDiagram,
    ) -> Result<ClassComplexity, ResourceLimitExceeded> {
        self.input
            .check_class_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_mindmap_complexity(
        &self,
        model: &MindmapDiagramRenderModel,
    ) -> Result<MindmapComplexity, ResourceLimitExceeded> {
        self.input
            .check_mindmap_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_zenuml_complexity(
        &self,
        model: &ZenumlDiagramRenderModel,
    ) -> Result<ZenumlComplexity, ResourceLimitExceeded> {
        self.input
            .check_zenuml_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_sequence_complexity(
        &self,
        model: &merman_core::diagrams::sequence::SequenceDiagramRenderModel,
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

impl ResourceLimitExceeded {
    fn from_input(policy: &RenderResourcePolicy, error: InputResourceLimitExceeded) -> Self {
        Self {
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
mod tests {
    use super::*;
    use merman_core::diagrams::flowchart::{FlowEdge, FlowNode, FlowSubgraph};
    use merman_core::{Engine, ParseOptions, RenderSemanticModel};
    use std::collections::HashSet;

    #[test]
    fn resource_contract_is_complete_unique_and_drives_every_profile() {
        assert_eq!(
            RESOURCE_PROFILE_DESCRIPTORS.len(),
            RenderResourceProfile::ALL.len()
        );
        assert_eq!(RESOURCE_LIMIT_DESCRIPTORS.len(), RESOURCE_LIMIT_COUNT);

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
            let policy = RenderResourcePolicy::for_profile(profile);
            for limit in RESOURCE_LIMIT_DESCRIPTORS {
                assert_eq!(policy.profile(), profile);
                if limit.hard_cap {
                    assert!(!limit.overridable);
                    assert!(policy.value(limit.id).is_some());
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
            assert_eq!(descriptor.id.descriptor(), descriptor);
        }
    }

    #[test]
    fn resource_overrides_fail_closed_for_unknown_and_internal_ids() {
        let mut limits = RenderResourcePolicy::interactive();
        assert!(matches!(
            limits.apply_override("future_limit", 1),
            Err(ResourceLimitOverrideError::UnknownLimit(_))
        ));
        assert_eq!(
            limits.apply_override("max_svg_tree_depth", 1),
            Err(ResourceLimitOverrideError::UnknownLimit(
                "max_svg_tree_depth".to_string()
            ))
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
            .with_limit(ResourceLimitId::MaxSvgBytes, 123)
            .unwrap()
            .check_source_bytes("12345")
            .unwrap_err();

        assert_eq!(err.phase, ResourceLimitPhase::Source);
        assert_eq!(err.limit, "max_source_bytes");
        assert_eq!(err.actual, 5);
        assert_eq!(err.max, 4);
        assert_eq!(
            err.explicit_overrides,
            vec![
                ResourceLimitOverride {
                    id: ResourceLimitId::MaxSourceBytes,
                    value: 4,
                },
                ResourceLimitOverride {
                    id: ResourceLimitId::MaxSvgBytes,
                    value: 123,
                },
            ]
        );
    }

    #[test]
    fn derived_layout_work_limits_report_the_owned_phase_and_metric() {
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 7)
            .unwrap();

        let error = limits.check_layout_work_units(8).unwrap_err();
        assert_eq!(error.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, 8);
        assert_eq!(error.max, 7);
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
    fn zenuml_uses_the_shared_model_budget() {
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

        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxModelItems, 1)
            .unwrap();
        let error = limits.check_zenuml_complexity(model).unwrap_err();
        assert_eq!(error.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(error.limit, "max_model_items");
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
