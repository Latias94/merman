//! Shared source-byte and model admission policy.

#[cfg(all(feature = "diagram-class", feature = "presentation-data"))]
use super::ClassComplexity;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
use super::FlowchartComplexity;
#[cfg(feature = "diagram-sequence")]
use super::SequenceComplexity;
#[cfg(feature = "diagram-zenuml")]
use super::ZenumlComplexity;
use super::{ModelComplexity, RESOURCE_PROFILE_COUNT, ResourceProfile};
#[cfg(all(feature = "diagram-class", feature = "presentation-data"))]
use crate::families::class::ClassDiagram;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
use crate::families::flowchart::FlowchartModel;
#[cfg(feature = "diagram-sequence")]
use crate::families::sequence::SequenceDiagramModel;
#[cfg(feature = "diagram-zenuml")]
use crate::families::zenuml::ZenumlDiagramModel;

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

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

    #[cfg(all(feature = "diagram-class", feature = "presentation-data"))]
    pub fn check_class_complexity(
        &self,
        model: &ClassDiagram,
    ) -> Result<ClassComplexity, InputResourceLimitExceeded> {
        let complexity = ClassComplexity::from_model(model);
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

#[cfg(test)]
mod tests {
    use super::super::RESOURCE_PROFILE_DESCRIPTORS;
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
            .with_limit(InputResourceLimitId::MaxSourceBytes, 4)
            .unwrap()
            .check_source_bytes("12345")
            .unwrap_err();
        assert_eq!(source_error.limit, "max_source_bytes");
    }
}
