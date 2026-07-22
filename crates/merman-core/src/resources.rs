//! Backend-independent Mermaid source and semantic-model resource policy.

use crate::diagram::RenderSemanticModel;
use crate::diagrams::flowchart::FlowchartModel;
use crate::diagrams::zenuml::{ZenumlDiagramRenderModel, ZenumlStatementKind};
use crate::models::class_diagram::ClassDiagram;

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

pub const INPUT_RESOURCE_CONTRACT_SCHEMA_VERSION: u32 = 1;
pub const RESOURCE_PROFILE_COUNT: usize = 4;
pub const INPUT_RESOURCE_LIMIT_COUNT: usize = 11;

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
    MaxFlowchartNodes,
    MaxFlowchartEdges,
    MaxFlowchartSubgraphs,
    MaxClassNodes,
    MaxClassEdges,
    MaxClassNamespaces,
    MaxZenumlParticipants,
    MaxZenumlStatements,
    MaxZenumlFragments,
    MaxLabelBytes,
}

impl InputResourceLimitId {
    pub const ALL: [Self; INPUT_RESOURCE_LIMIT_COUNT] = [
        Self::MaxSourceBytes,
        Self::MaxFlowchartNodes,
        Self::MaxFlowchartEdges,
        Self::MaxFlowchartSubgraphs,
        Self::MaxClassNodes,
        Self::MaxClassEdges,
        Self::MaxClassNamespaces,
        Self::MaxZenumlParticipants,
        Self::MaxZenumlStatements,
        Self::MaxZenumlFragments,
        Self::MaxLabelBytes,
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
pub struct InputResourceLimitDescriptor {
    pub id: InputResourceLimitId,
    pub stable_id: &'static str,
    pub phase: InputResourceLimitPhase,
    pub description: &'static str,
    pub overridable: bool,
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
                }),+
            ];
    };
}

input_limit_descriptors! {
    MaxSourceBytes => ("max_source_bytes", Source, "Maximum UTF-8 Mermaid source bytes"),
    MaxFlowchartNodes => ("max_flowchart_nodes", Model, "Maximum Flowchart nodes including subgraphs"),
    MaxFlowchartEdges => ("max_flowchart_edges", Model, "Maximum Flowchart edges"),
    MaxFlowchartSubgraphs => ("max_flowchart_subgraphs", Model, "Maximum Flowchart subgraphs"),
    MaxClassNodes => ("max_class_nodes", Model, "Maximum Class diagram nodes"),
    MaxClassEdges => ("max_class_edges", Model, "Maximum Class diagram edges"),
    MaxClassNamespaces => ("max_class_namespaces", Model, "Maximum Class diagram namespaces"),
    MaxZenumlParticipants => ("max_zenuml_participants", Model, "Maximum ZenUML participants"),
    MaxZenumlStatements => ("max_zenuml_statements", Model, "Maximum ZenUML statements"),
    MaxZenumlFragments => ("max_zenuml_fragments", Model, "Maximum ZenUML fragments and groups"),
    MaxLabelBytes => ("max_label_bytes", Model, "Maximum aggregate label bytes in Flowchart, Class, and ZenUML semantic models"),
}

const PROFILE_VALUES: [[Option<usize>; RESOURCE_PROFILE_COUNT]; INPUT_RESOURCE_LIMIT_COUNT] = [
    [Some(2 * MIB), Some(MIB), Some(16 * MIB), None],
    [Some(8_000), Some(4_000), Some(50_000), None],
    [Some(16_000), Some(8_000), Some(100_000), None],
    [Some(2_000), Some(1_000), Some(10_000), None],
    [Some(8_000), Some(4_000), Some(50_000), None],
    [Some(16_000), Some(8_000), Some(100_000), None],
    [Some(2_000), Some(1_000), Some(10_000), None],
    [Some(8_000), Some(4_000), Some(50_000), None],
    [Some(16_000), Some(8_000), Some(100_000), None],
    [Some(2_000), Some(1_000), Some(10_000), None],
    [Some(2 * MIB), Some(MIB), Some(16 * MIB), None],
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
        self.check_limit(
            InputResourceLimitPhase::Source,
            InputResourceLimitId::MaxSourceBytes,
            source.len(),
        )
    }

    pub fn check_render_model(
        &self,
        model: &RenderSemanticModel,
    ) -> Result<(), InputResourceLimitExceeded> {
        match model {
            RenderSemanticModel::Flowchart(model) => {
                self.check_flowchart_complexity(model).map(drop)
            }
            RenderSemanticModel::Class(model) => self.check_class_complexity(model).map(drop),
            RenderSemanticModel::Zenuml(model) => self.check_zenuml_complexity(model).map(drop),
            _ => Ok(()),
        }
    }

    pub fn check_flowchart_complexity(
        &self,
        model: &FlowchartModel,
    ) -> Result<FlowchartComplexity, InputResourceLimitExceeded> {
        let complexity = FlowchartComplexity::from_model(model);
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxFlowchartNodes,
            complexity.nodes,
        )?;
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxFlowchartEdges,
            complexity.edges,
        )?;
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxFlowchartSubgraphs,
            complexity.subgraphs,
        )?;
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxLabelBytes,
            complexity.label_bytes,
        )?;
        Ok(complexity)
    }

    pub fn check_class_complexity(
        &self,
        model: &ClassDiagram,
    ) -> Result<ClassComplexity, InputResourceLimitExceeded> {
        let complexity = ClassComplexity::from_model(model);
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxClassNodes,
            complexity.nodes,
        )?;
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxClassEdges,
            complexity.edges,
        )?;
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxClassNamespaces,
            complexity.namespaces,
        )?;
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxLabelBytes,
            complexity.label_bytes,
        )?;
        Ok(complexity)
    }

    pub fn check_zenuml_complexity(
        &self,
        model: &ZenumlDiagramRenderModel,
    ) -> Result<ZenumlComplexity, InputResourceLimitExceeded> {
        let complexity = ZenumlComplexity::from_model(model);
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxZenumlParticipants,
            complexity.participants,
        )?;
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxZenumlStatements,
            complexity.statements,
        )?;
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxZenumlFragments,
            complexity.fragments,
        )?;
        self.check_limit(
            InputResourceLimitPhase::Model,
            InputResourceLimitId::MaxLabelBytes,
            complexity.label_bytes,
        )?;
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("resource limit exceeded during {phase}: {limit} actual={actual} max={max}")]
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

fn optional_str_len(value: Option<&str>) -> usize {
    value.map(str::len).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_values_are_single_source_for_input_limits() {
        assert_eq!(INPUT_RESOURCE_CONTRACT_SCHEMA_VERSION, 1);
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
    }

    #[test]
    fn constrained_policy_rejects_source_and_flowchart_cardinality() {
        let source_error = InputResourcePolicy::for_profile(ResourceProfile::Constrained)
            .apply_for_test(InputResourceLimitId::MaxSourceBytes, 4)
            .check_source_bytes("12345")
            .unwrap_err();
        assert_eq!(source_error.limit, "max_source_bytes");

        let parsed = crate::Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nA --> B",
                crate::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let model_error = InputResourcePolicy::for_profile(ResourceProfile::Constrained)
            .apply_for_test(InputResourceLimitId::MaxFlowchartNodes, 1)
            .check_render_model(parsed.model())
            .unwrap_err();
        assert_eq!(model_error.limit, "max_flowchart_nodes");
    }

    impl InputResourcePolicy {
        fn apply_for_test(mut self, id: InputResourceLimitId, value: usize) -> Self {
            self.apply_limit(id, value).unwrap();
            self
        }
    }
}
