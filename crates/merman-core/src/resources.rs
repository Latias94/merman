//! Backend-independent Mermaid source and semantic-model resource policy.

#[cfg(feature = "presentation")]
use crate::diagram::{ParsedDiagramRender, RenderSemanticModel};
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
use crate::diagrams::flowchart::FlowchartModel;
#[cfg(feature = "diagram-ishikawa")]
use crate::diagrams::ishikawa::IshikawaDiagramModel;
#[cfg(feature = "diagram-kanban")]
use crate::diagrams::kanban::KanbanDiagramModel;
#[cfg(all(feature = "diagram-kanban", feature = "presentation"))]
use crate::diagrams::kanban::KanbanDiagramRenderModel;
#[cfg(feature = "diagram-mindmap")]
use crate::diagrams::mindmap::MindmapDiagramModel;
#[cfg(all(feature = "diagram-mindmap", feature = "presentation"))]
use crate::diagrams::mindmap::MindmapDiagramRenderModel;
#[cfg(feature = "diagram-radar")]
use crate::diagrams::radar::RadarDiagramModel;
#[cfg(all(test, feature = "diagram-requirement"))]
use crate::diagrams::requirement::RequirementDiagramModel;
#[cfg(feature = "diagram-sequence")]
use crate::diagrams::sequence::SequenceDiagramModel;
#[cfg(feature = "diagram-treemap")]
use crate::diagrams::treemap::TreemapDiagramModel;
#[cfg(feature = "diagram-zenuml")]
use crate::diagrams::zenuml::{ZenumlDiagramModel, ZenumlStatementKind};
#[cfg(all(feature = "diagram-class", feature = "presentation"))]
use crate::models::class_diagram::ClassDiagram;
#[cfg(feature = "diagram-class")]
use crate::models::class_diagram::ClassTopologyView;

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

pub use merman_model::resources::{
    RESOURCE_PROFILE_COUNT, RESOURCE_PROFILE_DESCRIPTORS, ResourceProfile,
    ResourceProfileDescriptor,
};
pub const INPUT_RESOURCE_LIMIT_COUNT: usize = 4;

pub const GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE: ResourceProfile = ResourceProfile::Interactive;
pub const CLI_DEFAULT_RESOURCE_PROFILE: ResourceProfile = ResourceProfile::TrustedNative;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputResourceLimitPhase {
    Source,
    Model,
}

impl InputResourceLimitPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Model => "layout_model",
        }
    }
}

impl std::fmt::Display for InputResourceLimitPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputResourceLimitId {
    MaxSourceBytes,
    MaxModelItems,
    MaxModelTextBytes,
    MaxModelNestingDepth,
}

impl InputResourceLimitId {
    pub const ALL: [Self; INPUT_RESOURCE_LIMIT_COUNT] = [
        Self::MaxSourceBytes,
        Self::MaxModelItems,
        Self::MaxModelTextBytes,
        Self::MaxModelNestingDepth,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub fn from_stable_id(id: &str) -> Option<Self> {
        INPUT_RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.stable_id == id)
            .map(|descriptor| descriptor.id)
    }

    pub const fn descriptor(self) -> &'static InputResourceLimitDescriptor {
        &INPUT_RESOURCE_LIMIT_DESCRIPTORS[self.index()]
    }

    pub const fn as_str(self) -> &'static str {
        self.descriptor().stable_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct InputResourceLimitDescriptor {
    pub id: InputResourceLimitId,
    pub stable_id: &'static str,
    pub phase: InputResourceLimitPhase,
    pub description: &'static str,
    pub overridable: bool,
    pub minimum_value: usize,
}

macro_rules! input_limit_descriptors {
    ($($id:ident => ($stable:literal, $phase:ident, $description:literal)),+ $(,)?) => {
        pub static INPUT_RESOURCE_LIMIT_DESCRIPTORS:
            [InputResourceLimitDescriptor; INPUT_RESOURCE_LIMIT_COUNT] = [
                $(InputResourceLimitDescriptor {
                    id: InputResourceLimitId::$id,
                    stable_id: $stable,
                    phase: InputResourceLimitPhase::$phase,
                    description: $description,
                    overridable: true,
                    minimum_value: 1,
                }),+
            ];
    };
}

input_limit_descriptors! {
    MaxSourceBytes => ("max_source_bytes", Source, "Maximum UTF-8 Mermaid source bytes"),
    MaxModelItems => ("max_model_items", Model, "Maximum semantic model entities and relationships across all diagram families"),
    MaxModelTextBytes => ("max_model_text_bytes", Model, "Maximum aggregate UTF-8 text retained by a semantic model"),
    MaxModelNestingDepth => ("max_model_nesting_depth", Model, "Maximum semantic nesting depth accepted before layout"),
}

const PROFILE_VALUES: [[Option<usize>; RESOURCE_PROFILE_COUNT]; INPUT_RESOURCE_LIMIT_COUNT] = [
    [Some(2 * MIB), Some(MIB), Some(16 * MIB), None],
    // Policy defaults, not Mermaid syntax limits. They deliberately preserve the
    // aggregate headroom of the previous per-family cardinality budgets.
    [Some(32_000), Some(16_000), Some(200_000), None],
    [Some(2 * MIB), Some(MIB), Some(16 * MIB), None],
    [Some(256), Some(128), Some(1_024), None],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputResourcePolicy {
    profile: ResourceProfile,
    base_values: [Option<usize>; INPUT_RESOURCE_LIMIT_COUNT],
    effective_values: [Option<usize>; INPUT_RESOURCE_LIMIT_COUNT],
    explicit_overrides: [Option<usize>; INPUT_RESOURCE_LIMIT_COUNT],
}

impl Default for InputResourcePolicy {
    fn default() -> Self {
        Self::for_profile(GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE)
    }
}

impl InputResourcePolicy {
    pub const fn for_profile(profile: ResourceProfile) -> Self {
        let mut values = [None; INPUT_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < INPUT_RESOURCE_LIMIT_COUNT {
            values[index] = PROFILE_VALUES[index][profile as usize];
            index += 1;
        }
        Self {
            profile,
            base_values: values,
            effective_values: values,
            explicit_overrides: [None; INPUT_RESOURCE_LIMIT_COUNT],
        }
    }

    pub const fn profile(self) -> ResourceProfile {
        self.profile
    }

    pub const fn value(self, id: InputResourceLimitId) -> Option<usize> {
        self.effective_values[id.index()]
    }

    pub const fn base_value(self, id: InputResourceLimitId) -> Option<usize> {
        self.base_values[id.index()]
    }

    pub const fn explicit_override(self, id: InputResourceLimitId) -> Option<usize> {
        self.explicit_overrides[id.index()]
    }

    pub fn explicit_overrides(&self) -> impl Iterator<Item = (InputResourceLimitId, usize)> + '_ {
        InputResourceLimitId::ALL
            .into_iter()
            .filter_map(|id| self.explicit_override(id).map(|value| (id, value)))
    }

    pub fn apply_override(
        &mut self,
        stable_id: &str,
        value: usize,
    ) -> Result<(), InputResourceLimitOverrideError> {
        let id = InputResourceLimitId::from_stable_id(stable_id)
            .ok_or_else(|| InputResourceLimitOverrideError::UnknownLimit(stable_id.to_string()))?;
        self.apply_limit(id, value)
    }

    pub fn apply_limit(
        &mut self,
        id: InputResourceLimitId,
        value: usize,
    ) -> Result<(), InputResourceLimitOverrideError> {
        if value == 0 {
            return Err(InputResourceLimitOverrideError::NonPositive(id.as_str()));
        }
        self.effective_values[id.index()] = Some(value);
        self.explicit_overrides[id.index()] = Some(value);
        Ok(())
    }

    pub fn with_limit(
        mut self,
        id: InputResourceLimitId,
        value: usize,
    ) -> Result<Self, InputResourceLimitOverrideError> {
        self.apply_limit(id, value)?;
        Ok(self)
    }

    /// Returns the pointwise minimum of two policies while preserving this policy's host profile.
    pub fn meet(&self, restriction: &Self) -> Self {
        let mut effective_values = [None; INPUT_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < INPUT_RESOURCE_LIMIT_COUNT {
            effective_values[index] = minimum_ceiling(
                self.effective_values[index],
                restriction.effective_values[index],
            );
            index += 1;
        }
        self.with_effective_values(effective_values)
    }

    /// Applies a request policy only when every requested ceiling is at least as strict.
    pub fn restrict_with(
        &self,
        restriction: &Self,
    ) -> Result<Self, InputResourcePolicyRestrictionError> {
        for id in InputResourceLimitId::ALL {
            let ceiling = self.value(id);
            let requested = restriction.value(id);
            if loosens_ceiling(ceiling, requested) {
                return Err(InputResourcePolicyRestrictionError {
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
        effective_values: [Option<usize>; INPUT_RESOURCE_LIMIT_COUNT],
    ) -> Self {
        let mut explicit_overrides = [None; INPUT_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < INPUT_RESOURCE_LIMIT_COUNT {
            if effective_values[index] != self.base_values[index] {
                explicit_overrides[index] = effective_values[index];
            }
            index += 1;
        }
        Self {
            profile: self.profile,
            base_values: self.base_values,
            effective_values,
            explicit_overrides,
        }
    }

    fn check_limit(
        &self,
        phase: InputResourceLimitPhase,
        id: InputResourceLimitId,
        actual: usize,
    ) -> Result<(), InputResourceLimitExceeded> {
        let Some(max) = self.value(id) else {
            return Ok(());
        };
        if actual <= max {
            return Ok(());
        }
        Err(InputResourceLimitExceeded {
            phase,
            limit: id.as_str(),
            actual,
            max,
            profile: self.profile,
            explicit_overrides: self
                .explicit_overrides()
                .map(|(id, value)| InputResourceLimitOverride { id, value })
                .collect(),
        })
    }

    pub fn check_source_bytes(&self, source: &str) -> Result<(), InputResourceLimitExceeded> {
        self.check_source_byte_count(source.len())
    }

    /// Checks the original byte length of an immutable prepared source.
    pub fn check_source_byte_count(
        &self,
        source_bytes: usize,
    ) -> Result<(), InputResourceLimitExceeded> {
        self.check_limit(
            InputResourceLimitPhase::Source,
            InputResourceLimitId::MaxSourceBytes,
            source_bytes,
        )
    }

    /// Checks only the public semantic-model projection.
    ///
    /// Use [`Self::check_parsed_render`] for a parsed built-in render operation so parser-owned
    /// render context is included in the text budget.
    #[cfg(feature = "presentation")]
    pub fn check_render_model(
        &self,
        model: &RenderSemanticModel,
    ) -> Result<(), InputResourceLimitExceeded> {
        self.check_model_complexity(model_complexity_from_render_model(model))
    }

    #[cfg(feature = "presentation")]
    pub fn check_parsed_render(
        &self,
        parsed: &ParsedDiagramRender,
    ) -> Result<(), InputResourceLimitExceeded> {
        self.check_model_complexity(model_complexity_from_parsed_render(parsed))
    }

    pub fn check_semantic_model(
        &self,
        model: &crate::SemanticDiagramModel,
    ) -> Result<(), InputResourceLimitExceeded> {
        self.check_model_complexity(model_complexity_from_semantic_model(model))
    }

    pub fn check_model_complexity(
        &self,
        complexity: ModelComplexity,
    ) -> Result<(), InputResourceLimitExceeded> {
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxModelItems,
            complexity.items,
        )?;
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxModelTextBytes,
            complexity.text_bytes,
        )?;
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxModelNestingDepth,
            complexity.nesting_depth,
        )
    }

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    pub fn check_flowchart_complexity(
        &self,
        model: &FlowchartModel,
    ) -> Result<FlowchartComplexity, InputResourceLimitExceeded> {
        let complexity = FlowchartComplexity::from_model(model);
        self.check_model_complexity(complexity.as_model_complexity())?;
        Ok(complexity)
    }

    #[cfg(all(feature = "diagram-class", feature = "presentation"))]
    pub fn check_class_complexity(
        &self,
        model: &ClassDiagram,
    ) -> Result<ClassComplexity, InputResourceLimitExceeded> {
        let complexity = ClassComplexity::from_model(model);
        self.check_model_complexity(complexity.as_model_complexity())?;
        Ok(complexity)
    }

    #[cfg(all(feature = "diagram-mindmap", feature = "presentation"))]
    pub fn check_mindmap_complexity(
        &self,
        model: &MindmapDiagramRenderModel,
    ) -> Result<MindmapComplexity, InputResourceLimitExceeded> {
        let complexity = MindmapComplexity::from_model(model);
        self.check_model_complexity(complexity.as_model_complexity())?;
        Ok(complexity)
    }

    #[cfg(feature = "diagram-zenuml")]
    pub fn check_zenuml_complexity(
        &self,
        model: &ZenumlDiagramModel,
    ) -> Result<ZenumlComplexity, InputResourceLimitExceeded> {
        let complexity = ZenumlComplexity::from_model(model);
        self.check_model_complexity(complexity.as_model_complexity())?;
        Ok(complexity)
    }

    #[cfg(feature = "diagram-sequence")]
    pub fn check_sequence_complexity(
        &self,
        model: &SequenceDiagramModel,
    ) -> Result<SequenceComplexity, InputResourceLimitExceeded> {
        let complexity = SequenceComplexity::from_model(model);
        self.check_model_complexity(complexity.as_model_complexity())?;
        Ok(complexity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InputResourceLimitOverrideError {
    #[error("resource limit id `{0}` is not part of input resource contract schema 1")]
    UnknownLimit(String),
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
pub struct InputResourcePolicyRestrictionError {
    pub id: InputResourceLimitId,
    pub requested: Option<usize>,
    pub ceiling: Option<usize>,
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("resource limit exceeded during {phase}: {limit} actual={actual} max={max}")]
#[non_exhaustive]
pub struct InputResourceLimitExceeded {
    pub phase: InputResourceLimitPhase,
    pub limit: &'static str,
    pub actual: usize,
    pub max: usize,
    pub profile: ResourceProfile,
    pub explicit_overrides: Vec<InputResourceLimitOverride>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputResourceLimitOverride {
    pub id: InputResourceLimitId,
    pub value: usize,
}

#[cfg(feature = "diagram-sequence")]
use merman_model::resources::JsonValueComplexity;
pub use merman_model::resources::ModelComplexity;
#[cfg(any(
    feature = "diagram-kanban",
    feature = "diagram-sequence",
    feature = "diagram-treemap"
))]
use merman_model::resources::json_value_complexity;

/// Counts canonical semantics and their paired parser-owned rendering text.
#[cfg(feature = "presentation")]
pub fn model_complexity_from_parsed_render(parsed: &ParsedDiagramRender) -> ModelComplexity {
    let mut complexity = model_complexity_from_render_model(parsed.model());
    complexity.text_bytes = complexity
        .text_bytes
        .saturating_add(parsed.retained_render_context_bytes());
    complexity
}

#[cfg(feature = "presentation")]
pub fn model_complexity_from_render_model(model: &RenderSemanticModel) -> ModelComplexity {
    match model {
        RenderSemanticModel::Error(model) => ModelComplexity::from_serializable(model),
        RenderSemanticModel::CustomJson(model) => {
            let mut complexity = ModelComplexity::from_json(model.value());
            complexity.text_bytes = complexity
                .text_bytes
                .saturating_add(model.model_name().len());
            complexity
        }
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(model) => {
            MindmapComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-state")]
        RenderSemanticModel::State(model) => model.model_complexity(),
        #[cfg(feature = "diagram-sequence")]
        RenderSemanticModel::Sequence(model) => {
            SequenceComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-zenuml")]
        RenderSemanticModel::Zenuml(model) => {
            ZenumlComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(model) => {
            FlowchartComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-architecture")]
        RenderSemanticModel::Architecture(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(model) => {
            let mut complexity = ClassComplexity::topology_from_model(model).as_model_complexity();
            complexity.text_bytes = ModelComplexity::from_serializable(model.model())
                .text_bytes
                .saturating_add(model.presentation().retained_bytes());
            complexity
        }
        #[cfg(feature = "diagram-c4")]
        RenderSemanticModel::C4(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-cynefin")]
        RenderSemanticModel::Cynefin(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-railroad")]
        RenderSemanticModel::Railroad(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-kanban")]
        RenderSemanticModel::Kanban(model) => model_complexity_from_kanban(model),
        #[cfg(feature = "diagram-gantt")]
        RenderSemanticModel::Gantt(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-pie")]
        RenderSemanticModel::Pie(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-packet")]
        RenderSemanticModel::Packet(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-timeline")]
        RenderSemanticModel::Timeline(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-journey")]
        RenderSemanticModel::Journey(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-requirement")]
        RenderSemanticModel::Requirement(model) => ModelComplexity::from_requirement(model),
        #[cfg(feature = "diagram-sankey")]
        RenderSemanticModel::Sankey(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-radar")]
        RenderSemanticModel::Radar(model) => model_complexity_from_radar(model.as_semantic_model()),
        #[cfg(feature = "diagram-info")]
        RenderSemanticModel::Info(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-treemap")]
        RenderSemanticModel::Treemap(model) => {
            TreemapComplexity::from_render_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-block")]
        RenderSemanticModel::Block(model) => model.model_complexity(),
        #[cfg(feature = "diagram-er")]
        RenderSemanticModel::Er(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-quadrant-chart")]
        RenderSemanticModel::QuadrantChart(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-xychart")]
        RenderSemanticModel::XyChart(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-git-graph")]
        RenderSemanticModel::GitGraph(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-tree-view")]
        RenderSemanticModel::TreeView(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-ishikawa")]
        RenderSemanticModel::Ishikawa(model) => {
            IshikawaComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-event-modeling")]
        RenderSemanticModel::EventModeling(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-venn")]
        RenderSemanticModel::Venn(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-wardley")]
        RenderSemanticModel::Wardley(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-usecase")]
        RenderSemanticModel::Usecase(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(model) => ModelComplexity::from_serializable(model),
    }
}

pub fn model_complexity_from_semantic_model(
    model: &crate::SemanticDiagramModel,
) -> ModelComplexity {
    match *model {
        crate::SemanticDiagramModel::Error(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-agentflow")]
        crate::SemanticDiagramModel::Agentflow(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-git-graph")]
        crate::SemanticDiagramModel::GitGraph(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-class")]
        crate::SemanticDiagramModel::Class(ref model) => {
            let mut complexity = ModelComplexity::from_serializable(model);
            complexity.nesting_depth = complexity
                .nesting_depth
                .max(class_namespace_depth(&model.namespaces));
            complexity
        }
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        crate::SemanticDiagramModel::Flowchart(ref model) => {
            FlowchartComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-sequence")]
        crate::SemanticDiagramModel::Sequence(ref model) => {
            SequenceComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-state")]
        crate::SemanticDiagramModel::State(ref model) => model.model_complexity(),
        #[cfg(feature = "diagram-cynefin")]
        crate::SemanticDiagramModel::Cynefin(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-info")]
        crate::SemanticDiagramModel::Info(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-pie")]
        crate::SemanticDiagramModel::Pie(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-packet")]
        crate::SemanticDiagramModel::Packet(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-tree-view")]
        crate::SemanticDiagramModel::TreeView(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-quadrant-chart")]
        crate::SemanticDiagramModel::QuadrantChart(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-radar")]
        crate::SemanticDiagramModel::Radar(ref model) => model_complexity_from_radar(model),
        #[cfg(feature = "diagram-sankey")]
        crate::SemanticDiagramModel::Sankey(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-venn")]
        crate::SemanticDiagramModel::Venn(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-er")]
        crate::SemanticDiagramModel::Er(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-architecture")]
        crate::SemanticDiagramModel::Architecture(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-gantt")]
        crate::SemanticDiagramModel::Gantt(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-c4")]
        crate::SemanticDiagramModel::C4(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-block")]
        crate::SemanticDiagramModel::Block(ref model) => model_complexity_from_block(model),
        #[cfg(feature = "diagram-kanban")]
        crate::SemanticDiagramModel::Kanban(ref model) => {
            model_complexity_from_kanban_semantic(model)
        }
        #[cfg(feature = "diagram-mindmap")]
        crate::SemanticDiagramModel::Mindmap(ref model) => {
            MindmapComplexity::from_semantic_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-xychart")]
        crate::SemanticDiagramModel::XyChart(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-ishikawa")]
        crate::SemanticDiagramModel::Ishikawa(ref model) => {
            IshikawaComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-treemap")]
        crate::SemanticDiagramModel::Treemap(ref model) => {
            TreemapComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-requirement")]
        crate::SemanticDiagramModel::Requirement(ref model) => {
            ModelComplexity::from_requirement(model)
        }
        #[cfg(feature = "diagram-usecase")]
        crate::SemanticDiagramModel::Usecase(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-event-modeling")]
        crate::SemanticDiagramModel::EventModeling(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-wardley")]
        crate::SemanticDiagramModel::Wardley(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-railroad")]
        crate::SemanticDiagramModel::Railroad(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-zenuml")]
        crate::SemanticDiagramModel::Zenuml(ref model) => {
            ZenumlComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-journey")]
        crate::SemanticDiagramModel::Journey(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-timeline")]
        crate::SemanticDiagramModel::Timeline(ref model) => {
            ModelComplexity::from_serializable(model)
        }
    }
}

#[cfg(feature = "diagram-block")]
pub fn model_complexity_from_block(
    model: &crate::diagrams::block::BlockDiagramModel,
) -> ModelComplexity {
    let mut complexity = ModelComplexity::from_serializable(model);
    let mut pending = model
        .root
        .map(|root| vec![(root, 0usize)])
        .unwrap_or_default();
    let mut visited = vec![false; model.blocks_flat.len()];
    while let Some((index, depth)) = pending.pop() {
        let Some(node) = model.blocks_flat.get(index) else {
            continue;
        };
        if std::mem::replace(&mut visited[index], true) {
            continue;
        }
        complexity.nesting_depth = complexity.nesting_depth.max(depth);
        pending.extend(
            node.children
                .iter()
                .map(|child| (*child, depth.saturating_add(1))),
        );
    }
    complexity
}

#[cfg(feature = "diagram-kanban")]
pub fn model_complexity_from_kanban_semantic(model: &KanbanDiagramModel) -> ModelComplexity {
    let mut complexity = ModelComplexity::new(
        model.nodes.len(),
        0,
        kanban_nesting_depth(
            model
                .nodes
                .iter()
                .map(|node| (node.id.as_str(), node.parent_id.as_deref())),
        ),
    );
    for node in &model.nodes {
        for text in [
            Some(node.id.as_str()),
            Some(node.label.as_str()),
            node.parent_id.as_deref(),
            node.ticket.as_deref(),
            node.priority.as_deref(),
            node.assigned.as_deref(),
            node.icon.as_deref(),
            node.css_classes.as_deref(),
            node.shape.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            complexity.text_bytes = complexity.text_bytes.saturating_add(text.len());
        }
        if let Some(metadata) = &node.metadata {
            let metadata = json_value_complexity(metadata.as_value(), 0);
            complexity.items = complexity.items.saturating_add(metadata.items);
            complexity.text_bytes = complexity.text_bytes.saturating_add(metadata.text_bytes);
            complexity.nesting_depth = complexity.nesting_depth.max(metadata.nesting_depth);
        }
    }
    complexity
}

#[cfg(all(feature = "diagram-kanban", feature = "presentation"))]
pub fn model_complexity_from_kanban(model: &KanbanDiagramRenderModel) -> ModelComplexity {
    let text_bytes = model.nodes.iter().fold(0usize, |total, node| {
        [
            Some(node.id.as_str()),
            Some(node.label.as_str()),
            node.parent_id.as_deref(),
            node.ticket.as_deref(),
            node.priority.as_deref(),
            node.assigned.as_deref(),
            node.icon.as_deref(),
        ]
        .into_iter()
        .flatten()
        .fold(total, |subtotal, value| {
            subtotal.saturating_add(value.len())
        })
    });
    let nesting_depth = kanban_nesting_depth(
        model
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.parent_id.as_deref())),
    );
    ModelComplexity::new(model.nodes.len(), text_bytes, nesting_depth)
}

#[cfg(feature = "diagram-radar")]
pub fn model_complexity_from_radar(model: &RadarDiagramModel) -> ModelComplexity {
    let common_text_bytes = [
        model.title.as_deref(),
        model.acc_title.as_deref(),
        model.acc_descr.as_deref(),
        Some(model.options.graticule.as_str()),
    ]
    .into_iter()
    .flatten()
    .fold(0usize, |total, value| total.saturating_add(value.len()));
    let axis_text_bytes = model.axes.iter().fold(0usize, |total, axis| {
        total
            .saturating_add(axis.name.len())
            .saturating_add(axis.label.len())
    });
    let (curve_items, curve_text_bytes) =
        model
            .curves
            .iter()
            .fold((0usize, 0usize), |(items, text_bytes), curve| {
                (
                    items.saturating_add(curve.entries.len()),
                    text_bytes
                        .saturating_add(curve.name.len())
                        .saturating_add(curve.label.len()),
                )
            });
    ModelComplexity::new(
        model
            .axes
            .len()
            .saturating_add(model.curves.len())
            .saturating_add(curve_items),
        common_text_bytes
            .saturating_add(axis_text_bytes)
            .saturating_add(curve_text_bytes),
        0,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub struct FlowchartComplexity {
    pub nodes: usize,
    pub edges: usize,
    pub subgraphs: usize,
    pub label_bytes: usize,
    pub subgraph_depth: usize,
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
impl FlowchartComplexity {
    pub fn from_model(model: &FlowchartModel) -> Self {
        Self {
            nodes: model.nodes.len().saturating_add(model.subgraphs.len()),
            edges: model.edges.len(),
            subgraphs: model.subgraphs.len(),
            label_bytes: flowchart_text_bytes(model),
            subgraph_depth: flowchart_subgraph_depth(model),
        }
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(
            self.nodes.saturating_add(self.edges),
            self.label_bytes,
            self.subgraph_depth,
        )
    }
}

/// Computes Treemap model complexity iteratively without serializing a user-controlled node tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "diagram-treemap")]
pub struct TreemapComplexity {
    pub nodes: usize,
    pub classes: usize,
    pub label_bytes: usize,
    pub nesting_depth: usize,
}

#[cfg(feature = "diagram-treemap")]
impl TreemapComplexity {
    pub fn from_model(model: &TreemapDiagramModel) -> Self {
        Self::from_model_with_styles(model, |_| None)
    }

    #[cfg(feature = "presentation")]
    pub fn from_render_model(model: &crate::diagrams::treemap::TreemapDiagramRenderModel) -> Self {
        Self::from_model_with_styles(&model.semantic, |index| model.compiled_styles(index))
    }

    fn from_model_with_styles<'a>(
        model: &'a TreemapDiagramModel,
        styles: impl Fn(Option<usize>) -> Option<&'a [String]>,
    ) -> Self {
        let mut nodes = 0usize;
        let mut classes = 0usize;
        let mut label_bytes = [
            model.acc_title.as_deref(),
            model.acc_descr.as_deref(),
            model.title.as_deref(),
        ]
        .into_iter()
        .flatten()
        .fold(0usize, |total, value| total.saturating_add(value.len()));
        let mut nesting_depth = 0usize;

        let mut pending = vec![(&model.root, None, 0usize)];
        let mut visited = vec![false; model.nodes.len()];
        while let Some((node, index, depth)) = pending.pop() {
            nodes = nodes.saturating_add(1);
            nesting_depth = nesting_depth.max(depth);
            label_bytes = label_bytes
                .saturating_add(node.name.len())
                .saturating_add(node.class_selector.as_deref().map_or(0, str::len))
                .saturating_add(
                    styles(index)
                        .unwrap_or(&[])
                        .iter()
                        .map(String::len)
                        .sum::<usize>(),
                );
            if let Some(value) = node.value.as_ref() {
                let value = json_value_complexity(value.as_value(), depth);
                label_bytes = label_bytes.saturating_add(value.text_bytes);
                nodes = nodes.saturating_add(value.items);
                nesting_depth = nesting_depth.max(value.nesting_depth);
            }
            if let Some(children) = node.children.as_ref() {
                for child in children.iter().rev() {
                    if let Some(record) = model.nodes.get(*child)
                        && !visited[*child]
                    {
                        visited[*child] = true;
                        pending.push((record, Some(*child), depth.saturating_add(1)));
                    }
                }
            }
        }

        for (class_name, class_def) in &model.classes {
            classes = classes.saturating_add(1);
            label_bytes = label_bytes
                .saturating_add(class_name.len())
                .saturating_add(class_def.id.len())
                .saturating_add(class_def.styles.iter().map(String::len).sum::<usize>())
                .saturating_add(class_def.text_styles.iter().map(String::len).sum::<usize>());
        }

        Self {
            nodes,
            classes,
            label_bytes,
            nesting_depth,
        }
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(
            self.nodes.saturating_add(self.classes),
            self.label_bytes,
            self.nesting_depth,
        )
    }
}

/// Computes Ishikawa model complexity iteratively without growing the stack with cause depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "diagram-ishikawa")]
pub struct IshikawaComplexity {
    pub nodes: usize,
    pub label_bytes: usize,
    pub nesting_depth: usize,
}

#[cfg(feature = "diagram-ishikawa")]
impl IshikawaComplexity {
    pub fn from_model(model: &IshikawaDiagramModel) -> Self {
        let mut complexity = Self {
            nodes: 0,
            label_bytes: [
                model.acc_title.as_deref(),
                model.acc_descr.as_deref(),
                model.title.as_deref(),
            ]
            .into_iter()
            .flatten()
            .fold(0usize, |total, value| total.saturating_add(value.len())),
            nesting_depth: 0,
        };
        let Some(root) = model.root.and_then(|id| model.nodes.get(id)) else {
            return complexity;
        };

        let mut pending = vec![(root, 0usize)];
        let mut visited = vec![false; model.nodes.len()];
        if let Some(root) = model.root {
            visited[root] = true;
        }
        while let Some((node, depth)) = pending.pop() {
            complexity.nodes = complexity.nodes.saturating_add(1);
            complexity.label_bytes = complexity.label_bytes.saturating_add(node.text.len());
            complexity.nesting_depth = complexity.nesting_depth.max(depth);
            for child in node.children.iter().rev() {
                if let Some(record) = model.nodes.get(*child)
                    && !visited[*child]
                {
                    visited[*child] = true;
                    pending.push((record, depth.saturating_add(1)));
                }
            }
        }
        complexity
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(self.nodes, self.label_bytes, self.nesting_depth)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "diagram-class")]
pub struct ClassComplexity {
    pub nodes: usize,
    pub edges: usize,
    pub namespaces: usize,
    pub label_bytes: usize,
    pub namespace_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "diagram-mindmap")]
pub struct MindmapComplexity {
    pub nodes: usize,
    pub edges: usize,
    pub label_bytes: usize,
    pub nesting_depth: usize,
}

#[cfg(feature = "diagram-mindmap")]
impl MindmapComplexity {
    pub fn from_semantic_model(model: &MindmapDiagramModel) -> Self {
        Self {
            nodes: model.nodes.len(),
            edges: model.nodes.iter().fold(0usize, |count, node| {
                count.saturating_add(node.children.len())
            }),
            label_bytes: model.nodes.iter().fold(0usize, |count, node| {
                [
                    Some(node.node_id.as_str()),
                    Some(node.descr.as_str()),
                    node.class.as_deref(),
                    node.icon.as_deref(),
                ]
                .into_iter()
                .flatten()
                .fold(count, |count, text| count.saturating_add(text.len()))
            }),
            nesting_depth: model
                .nodes
                .iter()
                .filter_map(|node| usize::try_from(node.level).ok())
                .max()
                .unwrap_or(0),
        }
    }

    #[cfg(feature = "presentation")]
    pub fn from_model(model: &MindmapDiagramRenderModel) -> Self {
        let node_label_bytes = model.nodes.iter().fold(0usize, |total, node| {
            let strings = [
                node.id.as_str(),
                node.dom_id.as_str(),
                node.label.as_str(),
                node.label_type.as_str(),
                node.shape.as_str(),
                node.css_classes.as_str(),
                node.look.as_str(),
                node.node_id.as_str(),
            ];
            let total = strings.into_iter().fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
            let total = node.css_styles.iter().fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
            total.saturating_add(node.icon.as_deref().map_or(0, str::len))
        });
        let edge_label_bytes = model.edges.iter().fold(0usize, |total, edge| {
            [
                edge.id.as_str(),
                edge.start.as_str(),
                edge.end.as_str(),
                edge.edge_type.as_str(),
                edge.curve.as_str(),
                edge.thickness.as_str(),
                edge.look.as_str(),
                edge.classes.as_str(),
            ]
            .into_iter()
            .fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            })
        });

        Self {
            nodes: model.nodes.len(),
            edges: model.edges.len(),
            label_bytes: node_label_bytes.saturating_add(edge_label_bytes),
            nesting_depth: model
                .nodes
                .iter()
                .filter_map(|node| usize::try_from(node.level).ok())
                .max()
                .unwrap_or(0),
        }
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(
            self.nodes.saturating_add(self.edges),
            self.label_bytes,
            self.nesting_depth,
        )
    }
}

#[cfg(feature = "diagram-class")]
impl ClassComplexity {
    #[cfg(feature = "presentation")]
    pub fn from_model(model: &ClassDiagram) -> Self {
        Self::from_view(&model.topology_view())
    }

    /// Counts the same effective Class projection without requiring a visual carrier.
    pub fn from_view(model: &ClassTopologyView<'_>) -> Self {
        let class_label_bytes = model
            .classes()
            .map(|(_, node)| {
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
            .relations()
            .map(|rel| {
                rel.id
                    .len()
                    .saturating_add(rel.id1.len())
                    .saturating_add(rel.id2.len())
                    .saturating_add(rel.title.len())
                    .saturating_add(rel.relation_title_1.as_deref().map_or(0, str::len))
                    .saturating_add(rel.relation_title_2.as_deref().map_or(0, str::len))
            })
            .sum::<usize>();
        let note_label_bytes = model
            .notes()
            .map(|note| {
                note.id
                    .len()
                    .saturating_add(note.text.len())
                    .saturating_add(note.class_id.as_deref().map(str::len).unwrap_or(0))
            })
            .sum::<usize>();
        let interface_label_bytes = model
            .interfaces()
            .map(|iface| {
                iface
                    .id
                    .len()
                    .saturating_add(iface.label.len())
                    .saturating_add(iface.class_id.len())
            })
            .sum::<usize>();
        let namespace_label_bytes = model
            .namespaces()
            .map(|(_, namespace)| {
                namespace
                    .id
                    .len()
                    .saturating_add(namespace.label.len())
                    .saturating_add(namespace.parent.as_deref().map(str::len).unwrap_or(0))
            })
            .sum::<usize>();

        Self {
            label_bytes: class_label_bytes
                .saturating_add(relation_label_bytes)
                .saturating_add(note_label_bytes)
                .saturating_add(interface_label_bytes)
                .saturating_add(namespace_label_bytes),
            ..Self::topology_from_view(model)
        }
    }

    #[cfg(feature = "presentation")]
    fn topology_from_model(model: &ClassDiagram) -> Self {
        Self::topology_from_view(&model.topology_view())
    }

    pub(crate) fn topology_from_view(model: &ClassTopologyView<'_>) -> Self {
        Self {
            nodes: model
                .classes()
                .len()
                .saturating_add(model.notes().len())
                .saturating_add(model.interfaces().len())
                .saturating_add(model.namespaces().len()),
            edges: model
                .relations()
                .len()
                .saturating_add(model.notes().filter(|note| note.class_id.is_some()).count()),
            namespaces: model.namespaces().len(),
            label_bytes: 0,
            namespace_depth: class_namespace_depth(model.effective_namespaces()),
        }
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(
            self.nodes.saturating_add(self.edges),
            self.label_bytes,
            self.namespace_depth,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "diagram-zenuml")]
pub struct ZenumlComplexity {
    pub participants: usize,
    pub groups: usize,
    pub statements: usize,
    pub fragments: usize,
    pub label_bytes: usize,
    pub nesting_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "diagram-sequence")]
pub struct SequenceComplexity {
    pub messages: usize,
    pub block_depth: usize,
    pub items: usize,
    pub text_bytes: usize,
    pub nesting_depth: usize,
}

#[cfg(feature = "diagram-sequence")]
impl SequenceComplexity {
    pub fn from_model(model: &SequenceDiagramModel) -> Self {
        const ACTOR_PROPERTY_VALUE_DEPTH: usize = 4;
        const NOTE_ACTOR_VALUE_DEPTH: usize = 3;

        let mut depth = 0usize;
        let mut block_depth = 0usize;
        for message in &model.messages {
            match message.message_type {
                10 | 12 | 15 | 19 | 22 | 27 | 30 | 32 => {
                    depth = depth.saturating_add(1);
                    block_depth = block_depth.max(depth);
                }
                11 | 14 | 16 | 21 | 23 | 29 | 31 => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }
        let common_text_bytes = [
            model.acc_title.as_deref(),
            model.acc_descr.as_deref(),
            model.title.as_deref(),
        ]
        .into_iter()
        .flatten()
        .fold(0usize, |total, value| total.saturating_add(value.len()));
        let actor_order_text_bytes = model
            .actor_order
            .iter()
            .fold(0usize, |total, value| total.saturating_add(value.len()));
        let mut arbitrary_json = JsonValueComplexity::default();
        let actor_text_bytes = model.actors.iter().fold(0usize, |total, (key, actor)| {
            let links = json_map_complexity(&actor.links, ACTOR_PROPERTY_VALUE_DEPTH);
            let properties = json_map_complexity(&actor.properties, ACTOR_PROPERTY_VALUE_DEPTH);
            arbitrary_json.merge(links);
            arbitrary_json.merge(properties);
            total
                .saturating_add(key.len())
                .saturating_add(actor.name.len())
                .saturating_add(actor.description.len())
                .saturating_add(actor.actor_type.len())
                .saturating_add(links.text_bytes)
                .saturating_add(properties.text_bytes)
        });
        let box_text_bytes = model.boxes.iter().fold(0usize, |total, sequence_box| {
            sequence_box.actor_keys.iter().fold(
                total
                    .saturating_add(sequence_box.fill.len())
                    .saturating_add(sequence_box.name.as_deref().map_or(0, str::len)),
                |subtotal, actor| subtotal.saturating_add(actor.len()),
            )
        });
        let message_text_bytes = model.messages.iter().fold(0usize, |total, message| {
            total
                .saturating_add(message.id.len())
                .saturating_add(message.from.as_deref().map_or(0, str::len))
                .saturating_add(message.to.as_deref().map_or(0, str::len))
                .saturating_add(message.message_text().len())
        });
        let note_text_bytes = model.notes.iter().fold(0usize, |total, note| {
            let actor = json_value_complexity(&note.actor, NOTE_ACTOR_VALUE_DEPTH);
            arbitrary_json.merge(actor);
            total
                .saturating_add(actor.text_bytes)
                .saturating_add(note.message.len())
        });
        let lifecycle_text_bytes = model
            .created_actors
            .keys()
            .chain(model.destroyed_actors.keys())
            .fold(0usize, |total, value| total.saturating_add(value.len()));
        Self {
            messages: model.messages.len(),
            block_depth,
            items: model
                .actors
                .len()
                .saturating_add(model.boxes.len())
                .saturating_add(model.messages.len())
                .saturating_add(model.notes.len())
                .saturating_add(arbitrary_json.items),
            text_bytes: common_text_bytes
                .saturating_add(actor_order_text_bytes)
                .saturating_add(actor_text_bytes)
                .saturating_add(box_text_bytes)
                .saturating_add(message_text_bytes)
                .saturating_add(note_text_bytes)
                .saturating_add(lifecycle_text_bytes),
            nesting_depth: block_depth.max(arbitrary_json.nesting_depth),
        }
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(self.items, self.text_bytes, self.nesting_depth)
    }
}

#[cfg(feature = "diagram-zenuml")]
impl ZenumlComplexity {
    pub fn from_model(model: &ZenumlDiagramModel) -> Self {
        let common_label_bytes = [model.title.as_deref(), model.starter.as_deref()]
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
            groups: model.groups.len(),
            statements: 0,
            fragments: 0,
            label_bytes: common_label_bytes
                .saturating_add(participant_label_bytes)
                .saturating_add(group_label_bytes),
            nesting_depth: 0,
        };
        let mut pending = vec![(model.statements.as_slice(), 0usize)];
        while let Some((statements, nesting_depth)) = pending.pop() {
            complexity.nesting_depth = complexity.nesting_depth.max(nesting_depth);
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
                        pending.push((body, nesting_depth.saturating_add(1)));
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
                        pending.push((body, nesting_depth.saturating_add(1)));
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
                            pending.push((&section.statements, nesting_depth.saturating_add(1)));
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

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(
            self.participants
                .saturating_add(self.groups)
                .saturating_add(self.statements),
            self.label_bytes,
            self.nesting_depth,
        )
    }
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
fn flowchart_text_bytes(model: &FlowchartModel) -> usize {
    let mut total = [
        Some(model.keyword.as_str()),
        model.acc_title.as_deref(),
        model.acc_descr.as_deref(),
        model.direction.as_deref(),
    ]
    .into_iter()
    .flatten()
    .fold(0usize, |subtotal, value| {
        subtotal.saturating_add(value.len())
    });
    for (id, styles) in &model.class_defs {
        total = styles
            .iter()
            .fold(total.saturating_add(id.len()), |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
    }
    total = model.vertex_calls.iter().fold(total, |subtotal, value| {
        subtotal.saturating_add(value.len())
    });
    if let Some(defaults) = &model.edge_defaults {
        total = total.saturating_add(defaults.interpolate.as_deref().map_or(0, str::len));
        total = defaults.style.iter().fold(total, |subtotal, value| {
            subtotal.saturating_add(value.len())
        });
    }
    for node in &model.nodes {
        for value in [
            Some(node.id.as_str()),
            node.label.as_deref(),
            node.label_type.as_deref(),
            node.layout_shape.as_deref(),
            node.shape.as_deref(),
            node.icon.as_deref(),
            node.form.as_deref(),
            node.pos.as_deref(),
            node.img.as_deref(),
            node.constraint.as_deref(),
            node.link.as_deref(),
            node.link_target.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            total = total.saturating_add(value.len());
        }
        total = node
            .classes
            .iter()
            .chain(&node.styles)
            .fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
    }
    for edge in &model.edges {
        for value in [
            Some(edge.id.as_str()),
            Some(edge.from.as_str()),
            Some(edge.to.as_str()),
            edge.label.as_deref(),
            edge.label_type.as_deref(),
            edge.edge_type.as_deref(),
            Some(edge.arrow.as_str()),
            edge.stroke.as_deref(),
            edge.interpolate.as_deref(),
            edge.animation.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            total = total.saturating_add(value.len());
        }
        total = edge
            .classes
            .iter()
            .chain(&edge.style)
            .fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
    }
    for subgraph in &model.subgraphs {
        for value in [
            Some(subgraph.id.as_str()),
            Some(subgraph.title.as_str()),
            subgraph.dir.as_deref(),
            subgraph.label_type.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            total = total.saturating_add(value.len());
        }
        total = subgraph
            .classes
            .iter()
            .chain(&subgraph.styles)
            .chain(&subgraph.nodes)
            .fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
    }
    total = model.tooltips.iter().fold(total, |subtotal, (id, value)| {
        subtotal
            .saturating_add(id.len())
            .saturating_add(value.len())
    });
    total.saturating_add(model.subgraph_vertex_styles.retained_bytes())
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
fn flowchart_subgraph_depth(model: &FlowchartModel) -> usize {
    if model.subgraphs.is_empty() {
        return 0;
    }
    let indices = model
        .subgraphs
        .iter()
        .enumerate()
        .map(|(index, subgraph)| (subgraph.id.as_str(), index))
        .collect::<std::collections::HashMap<_, _>>();
    let mut children = vec![Vec::new(); model.subgraphs.len()];
    let mut incoming = vec![0usize; model.subgraphs.len()];
    for (parent, subgraph) in model.subgraphs.iter().enumerate() {
        for member in &subgraph.nodes {
            let Some(&child) = indices.get(member.as_str()) else {
                continue;
            };
            children[parent].push(child);
            incoming[child] = incoming[child].saturating_add(1);
        }
    }
    let mut queue = std::collections::VecDeque::new();
    let mut depths = vec![1usize; model.subgraphs.len()];
    for (index, incoming) in incoming.iter().enumerate() {
        if *incoming == 0 {
            queue.push_back(index);
        }
    }
    while let Some(parent) = queue.pop_front() {
        for &child in &children[parent] {
            depths[child] = depths[child].max(depths[parent].saturating_add(1));
            incoming[child] = incoming[child].saturating_sub(1);
            if incoming[child] == 0 {
                queue.push_back(child);
            }
        }
    }
    depths.into_iter().max().unwrap_or(0)
}

#[cfg(feature = "diagram-class")]
fn class_namespace_depth(
    namespaces: &indexmap::IndexMap<String, crate::models::class_diagram::Namespace>,
) -> usize {
    if namespaces.is_empty() {
        return 0;
    }
    let indices = namespaces
        .keys()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect::<std::collections::HashMap<_, _>>();
    let parents = namespaces
        .values()
        .map(|namespace| {
            namespace
                .parent
                .as_deref()
                .and_then(|parent| indices.get(parent).copied())
        })
        .collect::<Vec<_>>();
    let mut depths = vec![0usize; parents.len()];
    let mut completed = vec![false; parents.len()];
    for start in 0..parents.len() {
        if completed[start] {
            continue;
        }
        let mut path = Vec::new();
        let mut positions = std::collections::HashMap::new();
        let mut cursor = Some(start);
        while let Some(index) = cursor {
            if completed[index] || positions.contains_key(&index) {
                break;
            }
            positions.insert(index, path.len());
            path.push(index);
            cursor = parents[index];
        }
        let mut depth = cursor
            .filter(|index| completed[*index])
            .map(|index| depths[index])
            .unwrap_or(0);
        for index in path.into_iter().rev() {
            depth = depth.saturating_add(1);
            depths[index] = depth;
            completed[index] = true;
        }
    }
    depths.into_iter().max().unwrap_or(0)
}

#[cfg(feature = "diagram-kanban")]
fn kanban_nesting_depth<'a>(
    nodes: impl Iterator<Item = (&'a str, Option<&'a str>)> + Clone,
) -> usize {
    let indices = nodes
        .clone()
        .enumerate()
        .map(|(index, (id, _))| (id, index))
        .collect::<std::collections::HashMap<_, _>>();
    let parents = nodes
        .map(|(_, parent)| parent.and_then(|parent| indices.get(parent).copied()))
        .collect::<Vec<_>>();
    let mut depths = vec![0usize; parents.len()];
    let mut completed = vec![false; parents.len()];
    for start in 0..parents.len() {
        if completed[start] {
            continue;
        }
        let mut path = Vec::new();
        let mut positions = std::collections::HashMap::new();
        let mut cursor = Some(start);
        while let Some(index) = cursor {
            if completed[index] || positions.contains_key(&index) {
                break;
            }
            positions.insert(index, path.len());
            path.push(index);
            cursor = parents[index];
        }
        let mut depth = cursor
            .filter(|index| completed[*index])
            .map(|index| depths[index])
            .unwrap_or(0);
        for index in path.into_iter().rev() {
            depth = depth.saturating_add(1);
            depths[index] = depth;
            completed[index] = true;
        }
    }
    depths.into_iter().max().unwrap_or(0)
}

#[cfg(feature = "diagram-sequence")]
fn json_map_complexity(
    map: &serde_json::Map<String, serde_json::Value>,
    value_depth: usize,
) -> JsonValueComplexity {
    let mut complexity = JsonValueComplexity {
        items: map.len(),
        text_bytes: map
            .keys()
            .fold(0usize, |total, key| total.saturating_add(key.len())),
        nesting_depth: 0,
        container_depth: 0,
    };
    for value in map.values() {
        complexity.merge(json_value_complexity(value, value_depth));
    }
    complexity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_profile_restriction_order_covers_every_profile_pair() {
        let expected = [
            [true, true, false, false],
            [false, true, false, false],
            [true, true, true, false],
            [true, true, true, true],
        ];

        for (ceiling_index, ceiling) in ResourceProfile::ALL.into_iter().enumerate() {
            for (requested_index, requested) in ResourceProfile::ALL.into_iter().enumerate() {
                assert_eq!(
                    requested.is_no_looser_than(ceiling),
                    expected[ceiling_index][requested_index],
                    "requested {requested} under {ceiling}",
                );
            }
        }
    }

    #[test]
    fn iterative_json_complexity_matches_existing_serialized_shape() {
        for value in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({"empty": [], "nested": {"name": ["value", {}]}}),
            serde_json::json!([1, "text", [[], {}]]),
        ] {
            assert_eq!(
                ModelComplexity::from_json(&value),
                ModelComplexity::from_serializable(&value),
            );
        }
    }

    #[test]
    #[cfg(feature = "compat-json")]
    fn generic_complexity_rejects_managed_json_depth_without_panicking() {
        let mut value = serde_json::Value::Null;
        for _ in 0..129 {
            value = serde_json::Value::Array(vec![value]);
        }
        let managed = crate::ManagedSemanticJson::from_value(value);
        assert_eq!(
            ModelComplexity::from_serializable(&managed),
            ModelComplexity::new(usize::MAX, usize::MAX, usize::MAX)
        );
    }

    #[test]
    #[cfg(feature = "presentation")]
    fn custom_json_complexity_preserves_deep_payload_and_model_name() {
        let mut value = serde_json::json!("leaf");
        for _ in 0..5_000 {
            value = serde_json::Value::Array(vec![value]);
        }
        let model =
            RenderSemanticModel::CustomJson(crate::CustomJsonRenderModel::new("custom", value));
        assert_eq!(
            model_complexity_from_render_model(&model),
            ModelComplexity::new(5_000, 10, 4_999),
        );
        drop(model);
    }

    #[test]
    fn profile_values_are_single_source_for_input_limits() {
        assert_eq!(
            RESOURCE_PROFILE_DESCRIPTORS.len(),
            ResourceProfile::ALL.len()
        );
        assert_eq!(
            INPUT_RESOURCE_LIMIT_DESCRIPTORS.len(),
            INPUT_RESOURCE_LIMIT_COUNT
        );
        for profile in ResourceProfile::ALL {
            let policy = InputResourcePolicy::for_profile(profile);
            for limit in InputResourceLimitId::ALL {
                assert_eq!(policy.base_value(limit), policy.value(limit));
            }
        }

        let interactive = InputResourcePolicy::for_profile(ResourceProfile::Interactive);
        for (limit, expected) in [
            (InputResourceLimitId::MaxModelItems, Some(32_000)),
            (InputResourceLimitId::MaxModelTextBytes, Some(2 * MIB)),
            (InputResourceLimitId::MaxModelNestingDepth, Some(256)),
        ] {
            assert_eq!(interactive.value(limit), expected, "{limit:?}");
        }
    }

    #[test]
    fn input_policy_meet_and_restriction_are_pointwise_monotonic() {
        let host = InputResourcePolicy::for_profile(ResourceProfile::Interactive)
            .with_limit(InputResourceLimitId::MaxSourceBytes, 100)
            .unwrap();
        let tighter = host
            .with_limit(InputResourceLimitId::MaxSourceBytes, 80)
            .unwrap()
            .with_limit(InputResourceLimitId::MaxModelItems, 10)
            .unwrap();

        let resolved = host.restrict_with(&tighter).unwrap();
        assert_eq!(
            resolved.value(InputResourceLimitId::MaxSourceBytes),
            Some(80)
        );
        assert_eq!(
            resolved.value(InputResourceLimitId::MaxModelItems),
            Some(10)
        );
        assert_eq!(resolved.profile(), ResourceProfile::Interactive);

        let looser = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let error = host.restrict_with(&looser).unwrap_err();
        assert_eq!(error.id, InputResourceLimitId::MaxSourceBytes);
        assert_eq!(error.requested, None);
        assert_eq!(error.ceiling, Some(100));

        assert_eq!(
            host.meet(&looser)
                .value(InputResourceLimitId::MaxSourceBytes),
            Some(100)
        );
    }

    #[test]
    fn constrained_policy_rejects_source_bytes() {
        let source_error = InputResourcePolicy::for_profile(ResourceProfile::Constrained)
            .apply_for_test(InputResourceLimitId::MaxSourceBytes, 4)
            .check_source_bytes("12345")
            .unwrap_err();
        assert_eq!(source_error.limit, "max_source_bytes");
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn constrained_policy_rejects_flowchart_cardinality() {
        let source = "flowchart TD\nA --> B";
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        let policy = InputResourcePolicy::for_profile(ResourceProfile::Constrained)
            .apply_for_test(InputResourceLimitId::MaxModelItems, 2);
        let model_error = policy.check_semantic_model(&parsed.model).unwrap_err();
        assert_eq!(model_error.limit, "max_model_items");

        #[cfg(feature = "presentation")]
        {
            let parsed = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap();
            assert_eq!(
                policy.check_render_model(parsed.model()).unwrap_err(),
                model_error
            );
        }
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn flowchart_subgraph_vertex_css_is_charged_once_by_its_canonical_owner() {
        let source =
            "flowchart TD\nsubgraph G\n A\nend\nclass G base\nstyle G fill:#f00\nclass G hot\n";
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        let crate::SemanticDiagramModel::Flowchart(model) = &parsed.model else {
            panic!("expected Flowchart model");
        };
        let complexity = FlowchartComplexity::from_model(model).as_model_complexity();
        let mut without_vertex_css = model.clone();
        without_vertex_css.subgraph_vertex_styles = Default::default();
        assert_eq!(
            complexity.text_bytes,
            FlowchartComplexity::from_model(&without_vertex_css)
                .as_model_complexity()
                .text_bytes
                + "G".len()
                + "hot".len()
                + "fill:#f00".len(),
        );
        let exact = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(
                InputResourceLimitId::MaxModelTextBytes,
                complexity.text_bytes,
            )
            .unwrap();
        exact.check_semantic_model(&parsed.model).unwrap();
        let too_small = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(
                InputResourceLimitId::MaxModelTextBytes,
                complexity.text_bytes - 1,
            )
            .unwrap();
        let error = too_small.check_semantic_model(&parsed.model).unwrap_err();
        assert_eq!(error.limit, "max_model_text_bytes");
        assert_eq!(error.actual, complexity.text_bytes);

        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap();
            assert_eq!(model_complexity_from_render_model(full.model()), complexity);
            assert_eq!(
                full.retained_render_context_bytes(),
                "G".len() + std::mem::size_of::<usize>(),
                "the presentation context retains only the group color ordinal, not another CSS owner",
            );
            assert_eq!(
                model_complexity_from_parsed_render(&full).text_bytes,
                complexity.text_bytes + full.retained_render_context_bytes(),
            );
        }
    }

    #[cfg(all(feature = "diagram-flowchart", feature = "presentation"))]
    #[test]
    fn parsed_render_resource_checks_include_parser_owned_flowchart_label_sources() {
        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA[\"&nbsp;entity&nbsp;\"] --> B\n",
                crate::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let model_complexity = model_complexity_from_render_model(parsed.model());
        assert!(parsed.retained_render_context_bytes() > 0);

        let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(
                InputResourceLimitId::MaxModelTextBytes,
                model_complexity.text_bytes,
            )
            .unwrap();
        policy
            .check_render_model(parsed.model())
            .expect("the public model alone fits its exact text budget");
        let error = policy.check_parsed_render(&parsed).unwrap_err();
        assert_eq!(error.limit, "max_model_text_bytes");
        assert!(error.actual > model_complexity.text_bytes);
    }

    #[cfg(feature = "diagram-sequence")]
    #[test]
    fn sequence_complexity_bounds_messages_and_nested_frames() {
        let source = "sequenceDiagram\nloop outer\nrect rgb(240,240,240)\nA->>B: hi\nend\nend";
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        let crate::SemanticDiagramModel::Sequence(model) = &parsed.model else {
            panic!("expected Sequence model");
        };
        let complexity = SequenceComplexity::from_model(model);
        assert_eq!(complexity.block_depth, 2);
        assert_eq!(complexity.messages, model.messages.len());

        let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(InputResourceLimitId::MaxModelNestingDepth, 1)
            .unwrap();
        let error = policy.check_semantic_model(&parsed.model).unwrap_err();
        assert_eq!(error.limit, "max_model_nesting_depth");
        assert_eq!(error.actual, 2);
        assert_eq!(error.max, 1);

        #[cfg(feature = "presentation")]
        {
            let parsed = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap();
            assert_eq!(
                policy.check_render_model(parsed.model()).unwrap_err(),
                error
            );
        }
    }

    #[cfg(feature = "diagram-sequence")]
    #[test]
    fn sequence_complexity_counts_arbitrary_json_items_and_depth_iteratively() {
        let mut parsed = crate::SemanticParser::new()
            .parse_diagram_sync("sequenceDiagram\nparticipant A")
            .unwrap();
        let crate::SemanticDiagramModel::Sequence(model) = &mut parsed.model else {
            panic!("expected Sequence model");
        };
        let mut nested = serde_json::Value::Null;
        for _ in 0..300 {
            nested = serde_json::Value::Array(vec![nested]);
        }
        model
            .actors
            .get_mut("A")
            .unwrap()
            .properties
            .insert("payload".to_string(), nested);

        let complexity = SequenceComplexity::from_model(model);
        assert_eq!(complexity.nesting_depth, 304);
        assert_eq!(complexity.items, 302);

        let exact = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(
                InputResourceLimitId::MaxModelNestingDepth,
                complexity.nesting_depth,
            )
            .unwrap();
        exact
            .check_semantic_model(&parsed.model)
            .expect("the exact JSON depth boundary should pass");

        let below = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(
                InputResourceLimitId::MaxModelNestingDepth,
                complexity.nesting_depth - 1,
            )
            .unwrap();
        let error = below
            .check_semantic_model(&parsed.model)
            .expect_err("the N-1 JSON depth boundary should fail");
        assert_eq!(error.limit, "max_model_nesting_depth");
        assert_eq!(error.actual, complexity.nesting_depth);
        assert_eq!(error.max, complexity.nesting_depth - 1);

        #[cfg(feature = "presentation")]
        {
            let crate::SemanticDiagramModel::Sequence(model) = parsed.model else {
                panic!("expected Sequence model");
            };
            let render_model = RenderSemanticModel::Sequence(model);
            exact
                .check_render_model(&render_model)
                .expect("the exact JSON depth boundary should pass");
            assert_eq!(below.check_render_model(&render_model).unwrap_err(), error);
        }
    }

    #[cfg(feature = "diagram-state")]
    #[test]
    fn state_complexity_bounds_semantic_nesting_in_flat_documents() {
        for depth in [32, 130] {
            let mut source = String::from("stateDiagram-v2\n");
            for index in 0..depth {
                source.push_str(&format!("state S{index} {{\n"));
            }
            source.push_str("Leaf\n");
            for _ in 0..depth {
                source.push_str("}\n");
            }
            let parsed = crate::SemanticParser::new()
                .parse_diagram_sync(&source)
                .unwrap();
            let complexity = model_complexity_from_semantic_model(&parsed.model);
            assert_eq!(complexity.nesting_depth, depth);
            let exact = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth)
                .unwrap();
            exact.check_semantic_model(&parsed.model).unwrap();
            let below = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth - 1)
                .unwrap();
            let error = below.check_semantic_model(&parsed.model).unwrap_err();
            assert_eq!(error.limit, "max_model_nesting_depth");
            assert_eq!(error.actual, depth);
            assert_eq!(error.max, depth - 1);

            #[cfg(feature = "presentation")]
            {
                let full = crate::MermaidEngine::new()
                    .parse_diagram_for_render_model_sync(&source, crate::ParseOptions::strict())
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    below.check_render_model(full.model()).unwrap_err().limit,
                    "max_model_nesting_depth"
                );
            }
        }
    }

    #[cfg(all(feature = "diagram-mindmap", feature = "presentation"))]
    #[test]
    fn mindmap_model_limits_cover_pre_layout_cardinality_and_labels() {
        let constrained = InputResourcePolicy::for_profile(ResourceProfile::Constrained);
        assert_eq!(
            constrained.value(InputResourceLimitId::MaxModelItems),
            Some(16_000)
        );

        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "mindmap\n  Root\n    First child\n    Second child\n",
                crate::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Mindmap(model) = parsed.model() else {
            panic!("expected Mindmap model");
        };
        let complexity = MindmapComplexity::from_model(model);
        assert_eq!(complexity.nodes, 3);
        assert_eq!(complexity.edges, 2);
        assert!(complexity.label_bytes >= "RootFirst childSecond child".len());

        for (id, max, expected_limit) in [
            (InputResourceLimitId::MaxModelItems, 4, "max_model_items"),
            (
                InputResourceLimitId::MaxModelTextBytes,
                1,
                "max_model_text_bytes",
            ),
        ] {
            let error = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(id, max)
                .unwrap()
                .check_render_model(parsed.model())
                .unwrap_err();
            assert_eq!(error.phase, InputResourceLimitPhase::Model);
            assert_eq!(error.limit, expected_limit);
        }
    }

    #[cfg(all(
        feature = "presentation",
        any(
            feature = "diagram-kanban",
            feature = "diagram-radar",
            feature = "diagram-requirement"
        )
    ))]
    #[test]
    fn generic_model_budget_covers_new_family_models_without_family_knobs() {
        let models = [
            #[cfg(feature = "diagram-kanban")]
            RenderSemanticModel::Kanban(KanbanDiagramRenderModel {
                nodes: vec![crate::diagrams::kanban::KanbanRenderNode::new(
                    "todo", "Todo",
                )],
            }),
            #[cfg(feature = "diagram-radar")]
            RenderSemanticModel::Radar({
                let mut model = crate::diagrams::radar::RadarDiagramRenderModel::default();
                model.axes = vec![crate::diagrams::radar::RadarAxis {
                    name: "speed".to_string(),
                    label: "Speed".to_string(),
                }];
                model
            }),
            #[cfg(feature = "diagram-requirement")]
            RenderSemanticModel::Requirement(RequirementDiagramModel {
                direction: "TB".to_string(),
                requirements: vec![crate::diagrams::requirement::RequirementNode {
                    name: "R1".to_string(),
                    node_type: "requirement".to_string(),
                    requirement_id: "1".to_string(),
                    text: "Must render".to_string(),
                    risk: "Low".to_string(),
                    verify_method: "Test".to_string(),
                    css_styles: Vec::new(),
                    classes: Vec::new(),
                }],
                acc_title: None,
                acc_descr: None,
                elements: Vec::new(),
                relationships: Vec::new(),
                classes: Default::default(),
            }),
        ];
        for model in models {
            let error = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(InputResourceLimitId::MaxModelTextBytes, 1)
                .unwrap()
                .check_render_model(&model)
                .unwrap_err();
            assert_eq!(error.limit, "max_model_text_bytes");
        }
    }

    #[cfg(all(feature = "diagram-kanban", feature = "presentation"))]
    #[test]
    fn kanban_complexity_accounts_for_parent_chain_depth_without_quadratic_scans() {
        let root = KanbanDiagramRenderModel {
            nodes: ["section", "lane", "card"]
                .into_iter()
                .enumerate()
                .map(|(index, id)| {
                    let mut node =
                        crate::diagrams::kanban::KanbanRenderNode::new(id, format!("Node {id}"));
                    if index > 0 {
                        node.parent_id = Some(["section", "lane"][index - 1].to_string());
                    }
                    node
                })
                .collect(),
        };

        assert_eq!(model_complexity_from_kanban(&root).nesting_depth, 3);
    }

    #[cfg(all(feature = "diagram-gantt", feature = "presentation"))]
    #[test]
    fn gantt_full_wrappers_preserve_static_field_resource_accounting() {
        use crate::diagrams::gantt::{
            GanttDiagramModel, GanttDiagramRenderModel, GanttRenderTask, GanttTask,
        };

        let empty = RenderSemanticModel::Gantt(GanttDiagramRenderModel::default());
        let complexity = model_complexity_from_render_model(&empty);
        assert_eq!(complexity.items, 1);
        assert_eq!(complexity.text_bytes, 0);
        InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(InputResourceLimitId::MaxModelItems, 1)
            .unwrap()
            .check_render_model(&empty)
            .unwrap();

        let task = GanttRenderTask {
            semantic: GanttTask {
                id: "build".to_string(),
                task: "Build".to_string(),
                ..Default::default()
            },
            render_end_ms: Some(42),
        };
        let expected = ModelComplexity::from_serializable(&GanttDiagramModel {
            tasks: vec![task.semantic.clone()],
            ..Default::default()
        });
        let mut full = GanttDiagramRenderModel::default();
        full.tasks.push(task);
        let full = RenderSemanticModel::Gantt(full);
        assert_eq!(model_complexity_from_render_model(&full), expected);
        InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(InputResourceLimitId::MaxModelItems, expected.items)
            .unwrap()
            .check_render_model(&full)
            .unwrap();
    }

    #[cfg(feature = "diagram-class")]
    #[test]
    fn class_semantic_complexity_counts_raw_namespace_depth() {
        let namespace = (0..20)
            .map(|index| format!("n{index}"))
            .collect::<Vec<_>>()
            .join(".");
        let source = format!("classDiagram\nnamespace {namespace} {{\nclass Service\n}}\n");
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(&source)
            .unwrap();
        let complexity = model_complexity_from_semantic_model(&parsed.model);
        assert_eq!(complexity.nesting_depth, 20);
        let error = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(InputResourceLimitId::MaxModelNestingDepth, 19)
            .unwrap()
            .check_semantic_model(&parsed.model)
            .unwrap_err();
        assert_eq!(error.limit, "max_model_nesting_depth");
        assert_eq!(error.actual, 20);
    }

    #[cfg(all(feature = "diagram-class", feature = "presentation"))]
    #[test]
    fn class_full_complexity_preserves_projection_and_charges_owned_text_once() {
        let source = concat!(
            "%%{init: {\"class\":{\"hierarchicalNamespaces\":false}}}%%\n",
            "classDiagram\nnamespace A.B {\nclass Service\nnote \"namespace note\"\n}\n",
            "class note0\nclass interface0\nIService ()-- Service\n",
        );
        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("Class model");
        };
        let projected = ClassComplexity::from_model(model);
        assert_eq!(projected.nodes, 6);
        assert_eq!(projected.edges, 1);
        assert_eq!(projected.namespaces, 1);
        assert_eq!(projected.namespace_depth, 1);
        let canonical_text = ModelComplexity::from_serializable(model.model()).text_bytes;
        // The published DOM ordinals, collapsed membership and collision remaps own 194 bytes.
        assert_eq!(
            model_complexity_from_render_model(parsed.model()).text_bytes,
            canonical_text + 194
        );
        let exact_text = model_complexity_from_parsed_render(&parsed).text_bytes;
        let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        policy
            .with_limit(InputResourceLimitId::MaxModelTextBytes, exact_text)
            .unwrap()
            .check_parsed_render(&parsed)
            .unwrap();
        let error = policy
            .with_limit(InputResourceLimitId::MaxModelTextBytes, exact_text - 1)
            .unwrap()
            .check_parsed_render(&parsed)
            .unwrap_err();
        assert_eq!(error.limit, "max_model_text_bytes");
        assert_eq!(error.actual, exact_text);
    }

    #[cfg(all(feature = "diagram-xychart", feature = "presentation"))]
    #[test]
    fn xychart_full_wrapper_preserves_static_field_resource_accounting() {
        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "xychart-beta\nx-axis [a, b]\nbar [1, 2]\n",
                crate::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let RenderSemanticModel::XyChart(model) = parsed.model() else {
            panic!("XYChart model")
        };
        let expected = ModelComplexity::from_serializable(&model.semantic);
        assert_eq!(model_complexity_from_render_model(parsed.model()), expected);
        InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(InputResourceLimitId::MaxModelItems, expected.items)
            .unwrap()
            .check_render_model(parsed.model())
            .unwrap();
    }

    #[cfg(feature = "diagram-kanban")]
    #[test]
    fn kanban_semantic_limits_count_arbitrary_metadata_iteratively() {
        use crate::diagrams::kanban::KanbanNodeModel;

        for depth in [32usize, 130] {
            let mut value = serde_json::Value::String("payload".to_string());
            for _ in 0..depth {
                value = serde_json::Value::Array(vec![value]);
            }
            let value = serde_json::Value::Object(serde_json::Map::from_iter([(
                "custom".to_string(),
                value,
            )]));
            let model = KanbanDiagramModel {
                nodes: vec![KanbanNodeModel {
                    id: "card".to_string(),
                    label: String::new(),
                    level: 0,
                    node_type: 0,
                    span: crate::SourceSpan::new(0, 0),
                    is_group: false,
                    parent_id: None,
                    ticket: None,
                    priority: None,
                    assigned: None,
                    icon: None,
                    css_classes: None,
                    shape: None,
                    metadata: Some(value.into()),
                }],
            };
            let complexity = model_complexity_from_kanban_semantic(&model);
            assert_eq!(complexity.items, depth + 2);
            assert_eq!(complexity.text_bytes, 17);
            assert_eq!(complexity.nesting_depth, depth + 1);
            InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth + 1)
                .unwrap()
                .check_model_complexity(complexity)
                .unwrap();
            assert!(
                InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                    .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth)
                    .unwrap()
                    .check_model_complexity(complexity)
                    .is_err()
            );
        }
    }

    #[cfg(feature = "diagram-treemap")]
    #[test]
    fn treemap_limits_include_arbitrary_value_nesting() {
        for depth in [32usize, 130] {
            let mut value = serde_json::Value::Null;
            for _ in 0..depth {
                value = serde_json::Value::Array(vec![value]);
            }
            let model = TreemapDiagramModel {
                root: crate::diagrams::treemap::TreemapNode {
                    value: Some(value.into()),
                    ..Default::default()
                },
                ..Default::default()
            };
            let complexity = TreemapComplexity::from_model(&model).as_model_complexity();
            assert_eq!(complexity.nesting_depth, depth);
            InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth)
                .unwrap()
                .check_model_complexity(complexity)
                .unwrap();
            assert!(
                InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                    .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth - 1)
                    .unwrap()
                    .check_model_complexity(complexity)
                    .is_err()
            );
            #[cfg(feature = "presentation")]
            {
                let full =
                    crate::diagrams::treemap::TreemapDiagramRenderModel::from_semantic_model(model);
                assert_eq!(
                    TreemapComplexity::from_render_model(&full).as_model_complexity(),
                    complexity
                );
            }
        }
    }

    #[cfg(feature = "diagram-treemap")]
    #[test]
    fn treemap_complexity_handles_deep_typed_trees_without_serde_recursion() {
        let mut nodes = vec![crate::diagrams::treemap::TreemapNode {
            name: "leaf".to_string(),
            ..Default::default()
        }];
        for index in (0..1_500).rev() {
            nodes.push(crate::diagrams::treemap::TreemapNode {
                name: format!("section{index}"),
                children: Some(vec![nodes.len() - 1]),
                ..Default::default()
            });
        }
        let model = TreemapDiagramModel {
            root: nodes.pop().unwrap(),
            nodes,
            ..Default::default()
        };

        let complexity = TreemapComplexity::from_model(&model);

        assert_eq!(complexity.nodes, 1_501);
        assert_eq!(complexity.nesting_depth, 1_500);
        assert!(complexity.label_bytes >= "leaf".len());
    }

    #[cfg(all(feature = "diagram-treemap", feature = "presentation"))]
    #[test]
    fn treemap_complexity_counts_reachable_presentation_overrides_only_for_full_models() {
        use crate::diagrams::treemap::{TreemapDiagramRenderModel, TreemapNode};

        let mut model = TreemapDiagramRenderModel::from_semantic_model(TreemapDiagramModel {
            root: TreemapNode {
                children: Some(vec![0]),
                ..Default::default()
            },
            nodes: vec![TreemapNode::default(), TreemapNode::default()],
            ..Default::default()
        });
        model.visuals.root_styles = Some(vec!["fill:red".to_string()]);
        model.visuals.node_styles = vec![
            Some(vec!["stroke:blue".to_string()]),
            Some(vec!["font-size:99px".to_string()]),
        ];
        let semantic = TreemapComplexity::from_model(&model.semantic);
        let full = TreemapComplexity::from_render_model(&model);

        assert_eq!(full.nodes, semantic.nodes);
        assert_eq!(full.nesting_depth, semantic.nesting_depth);
        assert_eq!(full.label_bytes, semantic.label_bytes + 19);
    }

    #[cfg(feature = "diagram-block")]
    #[test]
    fn block_semantic_complexity_retains_logical_nesting_after_flattening() {
        for depth in [32usize, 130] {
            let mut source = String::from("block\n");
            for index in 0..depth {
                source.push_str(&format!("block:G{index}\n"));
            }
            source.push_str("A\n");
            for _ in 0..depth {
                source.push_str("end\n");
            }
            let parsed = crate::SemanticParser::new()
                .parse_diagram_sync(&source)
                .unwrap();
            let complexity = model_complexity_from_semantic_model(&parsed.model);

            assert_eq!(complexity.nesting_depth, depth + 1);
        }
    }

    #[cfg(feature = "diagram-ishikawa")]
    #[test]
    fn ishikawa_complexity_handles_deep_typed_trees_without_serde_recursion() {
        let mut nodes = vec![crate::diagrams::ishikawa::IshikawaNode {
            text: "leaf".to_string(),
            children: Vec::new(),
        }];
        for index in (0..1_500).rev() {
            nodes.push(crate::diagrams::ishikawa::IshikawaNode {
                text: format!("cause{index}"),
                children: vec![nodes.len() - 1],
            });
        }
        let model = IshikawaDiagramModel {
            root: Some(nodes.len() - 1),
            nodes,
            ..Default::default()
        };

        let complexity = IshikawaComplexity::from_model(&model);

        assert_eq!(complexity.nodes, 1_501);
        assert_eq!(complexity.nesting_depth, 1_500);
        assert!(complexity.label_bytes >= "leaf".len());
    }

    impl InputResourcePolicy {
        fn apply_for_test(mut self, id: InputResourceLimitId, value: usize) -> Self {
            self.apply_limit(id, value).unwrap();
            self
        }
    }
}
