#[cfg(any(test, merman_internal_theme_acceptance))]
use std::fmt;

#[cfg(any(test, merman_internal_theme_acceptance))]
use crate::DiagramFamilyId;
#[cfg(merman_internal_theme_acceptance)]
use crate::diagram_theme::{
    FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind, FamilyThemeRuleFacet,
    ResolvedDiagramTheme,
};
#[cfg(any(test, merman_internal_theme_acceptance))]
use crate::diagram_theme::{ThemeTarget, ThemeVariant};
#[cfg(merman_internal_theme_acceptance)]
use sha2::{Digest as _, Sha256};

/// Renderer-owned visual facts for one terminal covered by an Architecture Text cutover.
///
/// This workspace-private projection deliberately carries the final writer value and measured
/// target rectangle outside the SVG DOM. The non-published acceptance harness can consume these
/// facts without reconstructing Architecture layout geometry or introducing `data-*` proof
/// attributes into stable SVG output.
#[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArchitectureTextCutoverRole {
    Service,
    GroupTitle,
    EdgeLabel,
}

#[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
impl ArchitectureTextCutoverRole {
    const fn digest_name(self) -> &'static str {
        match self {
            Self::Service => "service",
            Self::GroupTitle => "group-title",
            Self::EdgeLabel => "edge-label",
        }
    }
}

#[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureTextCutoverTerminal {
    role: ArchitectureTextCutoverRole,
    identity: Box<str>,
    value: Box<str>,
    region_bits: [u64; 4],
    writer_fragment_digest: Option<[u8; 32]>,
    writer_run_count: usize,
}

#[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
impl ArchitectureTextCutoverTerminal {
    pub(crate) fn new(
        role: ArchitectureTextCutoverRole,
        identity: impl Into<Box<str>>,
        value: impl Into<Box<str>>,
        region: [f64; 4],
    ) -> Option<Self> {
        (region.into_iter().all(f64::is_finite) && region[2] > 0.0 && region[3] > 0.0).then(|| {
            Self {
                role,
                identity: identity.into(),
                value: value.into(),
                region_bits: region.map(f64::to_bits),
                writer_fragment_digest: None,
                writer_run_count: 0,
            }
        })
    }

    pub(crate) fn with_writer_facts(
        role: ArchitectureTextCutoverRole,
        identity: impl Into<Box<str>>,
        value: impl Into<Box<str>>,
        region: [f64; 4],
        writer_fragment_digest: [u8; 32],
        writer_run_count: usize,
    ) -> Option<Self> {
        if writer_run_count == 0 {
            return None;
        }
        Some(
            Self::new(role, identity, value, region)?
                .with_facts(writer_fragment_digest, writer_run_count),
        )
    }

    fn with_facts(mut self, writer_fragment_digest: [u8; 32], writer_run_count: usize) -> Self {
        self.writer_fragment_digest = Some(writer_fragment_digest);
        self.writer_run_count = writer_run_count;
        self
    }

    pub const fn role(&self) -> ArchitectureTextCutoverRole {
        self.role
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn region(&self) -> [f64; 4] {
        self.region_bits.map(f64::from_bits)
    }
}

/// Opaque production receipt for the finalized Architecture Text cutover surface.
#[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureTextCutoverReceipt {
    terminals: Box<[ArchitectureTextCutoverTerminal]>,
    digest: [u8; 32],
}

#[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
impl ArchitectureTextCutoverReceipt {
    pub(crate) fn seal(terminals: Vec<ArchitectureTextCutoverTerminal>) -> Option<Self> {
        if terminals.is_empty() {
            return None;
        }
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, b"merman.architecture-text-cutover-receipt.v1");
        update_len(&mut hasher, terminals.len());
        for terminal in &terminals {
            update_len_prefixed(&mut hasher, terminal.role.digest_name().as_bytes());
            update_len_prefixed(&mut hasher, terminal.identity.as_bytes());
            update_len_prefixed(&mut hasher, terminal.value.as_bytes());
            for coordinate in terminal.region_bits {
                hasher.update(coordinate.to_be_bytes());
            }
            hasher.update([u8::from(terminal.writer_fragment_digest.is_some())]);
            if let Some(fragment_digest) = terminal.writer_fragment_digest {
                hasher.update(fragment_digest);
            }
            update_len(&mut hasher, terminal.writer_run_count);
        }
        Some(Self {
            terminals: terminals.into_boxed_slice(),
            digest: hasher.finalize().into(),
        })
    }

    pub fn terminals(&self) -> &[ArchitectureTextCutoverTerminal] {
        &self.terminals
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    #[cfg(test)]
    pub(crate) fn terminal_count(&self) -> usize {
        self.terminals.len()
    }
}

#[cfg(all(test, merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
mod architecture_text_receipt_tests {
    use super::*;

    #[test]
    fn receipt_digest_binds_terminal_role_identity_value_and_region() {
        let terminal = |role, identity, value, region| {
            ArchitectureTextCutoverTerminal::new(role, identity, value, region)
                .expect("valid Architecture text terminal")
        };
        let baseline = ArchitectureTextCutoverReceipt::seal(vec![terminal(
            ArchitectureTextCutoverRole::EdgeLabel,
            "api->worker#0",
            "#dc2626",
            [10.0, 20.0, 30.0, 40.0],
        )])
        .expect("seal receipt");

        assert_eq!(baseline.terminal_count(), 1);
        for changed in [
            terminal(
                ArchitectureTextCutoverRole::Service,
                "api->worker#0",
                "#dc2626",
                [10.0, 20.0, 30.0, 40.0],
            ),
            terminal(
                ArchitectureTextCutoverRole::EdgeLabel,
                "api->db#0",
                "#dc2626",
                [10.0, 20.0, 30.0, 40.0],
            ),
            terminal(
                ArchitectureTextCutoverRole::EdgeLabel,
                "api->worker#0",
                "transparent",
                [10.0, 20.0, 30.0, 40.0],
            ),
            terminal(
                ArchitectureTextCutoverRole::EdgeLabel,
                "api->worker#0",
                "#dc2626",
                [11.0, 20.0, 30.0, 40.0],
            ),
        ] {
            let changed =
                ArchitectureTextCutoverReceipt::seal(vec![changed]).expect("seal changed receipt");
            assert_ne!(baseline.digest(), changed.digest());
        }
    }

    #[test]
    fn invalid_or_empty_terminal_sets_do_not_seal() {
        assert!(ArchitectureTextCutoverReceipt::seal(Vec::new()).is_none());
        assert!(
            ArchitectureTextCutoverTerminal::new(
                ArchitectureTextCutoverRole::Service,
                "api",
                "#dc2626",
                [0.0, 0.0, 0.0, 10.0],
            )
            .is_none()
        );
    }
}

#[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
fn update_len(hasher: &mut Sha256, len: usize) {
    hasher.update(u64::try_from(len).unwrap_or(u64::MAX).to_be_bytes());
}

#[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    update_len(hasher, bytes.len());
    hasher.update(bytes);
}

/// Canonical selector class for one legacy bridge cutover witness.
#[cfg(any(test, merman_internal_theme_acceptance))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeRouteCutoverSelector {
    /// A static rule with neither an ordinal selector nor an explicit variant.
    StaticUnqualified,
    /// A static rule selected by one explicit, bounded theme variant.
    StaticVariant(ThemeVariant),
}

#[cfg(any(test, merman_internal_theme_acceptance))]
impl ThemeRouteCutoverSelector {
    pub fn id(self) -> String {
        match self {
            Self::StaticUnqualified => "static-unqualified".to_owned(),
            Self::StaticVariant(variant) => format!("static-variant-{}", variant.id()),
        }
    }
}

/// Theme facet proven by one legacy bridge cutover witness.
#[cfg(any(test, merman_internal_theme_acceptance))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeRouteCutoverFacet {
    Fill,
    Stroke,
}

/// Admitted value class proven by one legacy bridge cutover witness.
#[cfg(any(test, merman_internal_theme_acceptance))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeRouteCutoverValue {
    Transparent,
    Solid,
}

/// Canonical identity for a typed route whose bridge ownership is being cut over.
///
/// Projection obligations deliberately do not participate in this identity. They are versioned
/// authorization facts about the route and can change only through an explicit cutover migration.
#[cfg(any(test, merman_internal_theme_acceptance))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeRouteCutoverId {
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    selector: ThemeRouteCutoverSelector,
    facet: ThemeRouteCutoverFacet,
    value: ThemeRouteCutoverValue,
}

#[cfg(any(test, merman_internal_theme_acceptance))]
impl ThemeRouteCutoverId {
    pub const fn new(
        family_id: DiagramFamilyId,
        target: ThemeTarget,
        selector: ThemeRouteCutoverSelector,
        facet: ThemeRouteCutoverFacet,
        value: ThemeRouteCutoverValue,
    ) -> Self {
        Self {
            family_id,
            target,
            selector,
            facet,
            value,
        }
    }

    pub const fn family_id(self) -> DiagramFamilyId {
        self.family_id
    }

    pub const fn target(self) -> ThemeTarget {
        self.target
    }

    pub const fn selector(self) -> ThemeRouteCutoverSelector {
        self.selector
    }

    pub const fn facet(self) -> ThemeRouteCutoverFacet {
        self.facet
    }

    pub const fn value(self) -> ThemeRouteCutoverValue {
        self.value
    }
}

/// One legacy projection affected by typed route ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ThemeRouteCutoverProjection {
    NodeFill = 0,
    NodeStroke = 1,
    EdgeStroke = 2,
    MarkerPaintFromEdge = 3,
    ActorFill = 4,
    ActorStroke = 5,
    NoteFill = 6,
    NoteStroke = 7,
    ActivationFill = 8,
    ActivationStroke = 9,
    MessageStroke = 10,
    ClusterFill = 11,
    ClusterStroke = 12,
    LifelineStroke = 13,
    TitleFill = 14,
    RequirementFill = 15,
    PieSliceStroke = 16,
    ActorLabelFill = 17,
    MessageLabelFill = 18,
    LoopFill = 19,
    LoopStroke = 20,
    LoopLabelFill = 21,
    NoteLabelFill = 22,
    TextFill = 23,
    GanttTaskDefaultFill = 24,
    GanttTaskActiveFill = 25,
    GanttTaskSuccessFill = 26,
    GanttTaskErrorFill = 27,
    NodeLabelFill = 28,
    PieSliceFill = 29,
    RequirementStroke = 30,
    GanttTaskDefaultStroke = 31,
    GanttTaskActiveStroke = 32,
    GanttTaskSuccessStroke = 33,
    GanttTaskErrorStroke = 34,
    JourneyTaskFill = 35,
    KanbanTaskStroke = 36,
    JourneyTaskStroke = 37,
    TreeViewMarkerPaint = 38,
    GitGraphCommitLabelBackgroundFill = 39,
    GanttTaskWarningStroke = 40,
    TimelineEventFill = 41,
    C4TitleFillFallback = 42,
    EdgeLabelFill = 43,
    ErTableOddFill = 44,
    ErTableEvenFill = 45,
    RequirementRelationPaint = 46,
    TimelineTextFill = 47,
    TimelineEventStroke = 48,
    RequirementTextFill = 49,
    RadarLinePaint = 50,
    RadarAxisPaint = 51,
    RadarTextPaint = 52,
    ClusterLabelFill = 53,
    EdgeLabelBackgroundFill = 54,
    ChartTextPaint = 55,
}

impl ThemeRouteCutoverProjection {
    #[cfg(any(test, merman_internal_theme_acceptance))]
    const ALL: [Self; 56] = [
        Self::NodeFill,
        Self::NodeStroke,
        Self::EdgeStroke,
        Self::MarkerPaintFromEdge,
        Self::ActorFill,
        Self::ActorStroke,
        Self::NoteFill,
        Self::NoteStroke,
        Self::ActivationFill,
        Self::ActivationStroke,
        Self::MessageStroke,
        Self::ClusterFill,
        Self::ClusterStroke,
        Self::LifelineStroke,
        Self::TitleFill,
        Self::RequirementFill,
        Self::PieSliceStroke,
        Self::ActorLabelFill,
        Self::MessageLabelFill,
        Self::LoopFill,
        Self::LoopStroke,
        Self::LoopLabelFill,
        Self::NoteLabelFill,
        Self::TextFill,
        Self::GanttTaskDefaultFill,
        Self::GanttTaskActiveFill,
        Self::GanttTaskSuccessFill,
        Self::GanttTaskErrorFill,
        Self::NodeLabelFill,
        Self::PieSliceFill,
        Self::RequirementStroke,
        Self::GanttTaskDefaultStroke,
        Self::GanttTaskActiveStroke,
        Self::GanttTaskSuccessStroke,
        Self::GanttTaskErrorStroke,
        Self::JourneyTaskFill,
        Self::KanbanTaskStroke,
        Self::JourneyTaskStroke,
        Self::TreeViewMarkerPaint,
        Self::GitGraphCommitLabelBackgroundFill,
        Self::GanttTaskWarningStroke,
        Self::TimelineEventFill,
        Self::C4TitleFillFallback,
        Self::EdgeLabelFill,
        Self::ErTableOddFill,
        Self::ErTableEvenFill,
        Self::RequirementRelationPaint,
        Self::TimelineTextFill,
        Self::TimelineEventStroke,
        Self::RequirementTextFill,
        Self::RadarLinePaint,
        Self::RadarAxisPaint,
        Self::RadarTextPaint,
        Self::ClusterLabelFill,
        Self::EdgeLabelBackgroundFill,
        Self::ChartTextPaint,
    ];

    pub const fn contribution_id(self) -> &'static str {
        match self {
            Self::NodeFill => "node.fill",
            Self::NodeStroke => "node.stroke",
            Self::EdgeStroke => "edge.stroke",
            Self::MarkerPaintFromEdge => "marker.paint-from-edge",
            Self::ActorFill => "actor.fill",
            Self::ActorStroke => "actor.stroke",
            Self::NoteFill => "note.fill",
            Self::NoteStroke => "note.stroke",
            Self::ActivationFill => "activation.fill",
            Self::ActivationStroke => "activation.stroke",
            Self::MessageStroke => "message.stroke",
            Self::ClusterFill => "cluster.fill",
            Self::ClusterStroke => "cluster.stroke",
            Self::LifelineStroke => "lifeline.stroke",
            Self::TitleFill | Self::C4TitleFillFallback => "title.fill",
            Self::RequirementFill => "requirement.fill",
            Self::PieSliceStroke => "slice.stroke",
            Self::ActorLabelFill => "actor-label.fill",
            Self::MessageLabelFill => "message-label.fill",
            Self::LoopFill => "loop.fill",
            Self::LoopStroke => "loop.stroke",
            Self::LoopLabelFill => "loop-label.fill",
            Self::NoteLabelFill => "note-label.fill",
            Self::TextFill => "text.fill",
            Self::ChartTextPaint => "chart.text-axis",
            Self::GanttTaskDefaultFill => "task.default.fill",
            Self::GanttTaskActiveFill => "task.active.fill",
            Self::GanttTaskSuccessFill => "task.success.fill",
            Self::GanttTaskErrorFill => "task.error.fill",
            Self::NodeLabelFill => "node-label.fill",
            Self::ClusterLabelFill => "cluster-label.fill",
            Self::EdgeLabelBackgroundFill => "edge-label-background.fill",
            Self::EdgeLabelFill => "edge-label.fill",
            Self::ErTableOddFill => "table.odd.fill",
            Self::ErTableEvenFill => "table.even.fill",
            Self::PieSliceFill => "slice.fill",
            Self::RequirementStroke => "requirement.paint",
            Self::GanttTaskDefaultStroke => "task.default.stroke",
            Self::GanttTaskActiveStroke => "task.active.stroke",
            Self::GanttTaskSuccessStroke => "task.success.stroke",
            Self::GanttTaskErrorStroke => "task.error.stroke",
            Self::JourneyTaskFill => "task.fill",
            Self::KanbanTaskStroke => "task.default.stroke",
            Self::JourneyTaskStroke => "task.stroke",
            Self::TreeViewMarkerPaint => "marker.paint",
            Self::GitGraphCommitLabelBackgroundFill => "commit-label-background.fill",
            Self::GanttTaskWarningStroke => "task.warning.stroke",
            Self::TimelineEventFill => "event.fill",
            Self::RequirementRelationPaint => "relation.paint",
            Self::TimelineTextFill => "event.text",
            Self::TimelineEventStroke => "event.stroke",
            Self::RequirementTextFill => "requirement.text",
            Self::RadarLinePaint => "chart.text",
            Self::RadarAxisPaint => "chart.axis",
            Self::RadarTextPaint => "chart.text",
        }
    }

    #[cfg(any(test, merman_internal_theme_acceptance))]
    pub const fn action(self) -> ThemeRouteCutoverProjectionAction {
        match self {
            Self::MarkerPaintFromEdge | Self::C4TitleFillFallback => {
                ThemeRouteCutoverProjectionAction::RetireFallback
            }
            Self::NodeFill
            | Self::NodeStroke
            | Self::EdgeStroke
            | Self::ActorFill
            | Self::ActorStroke
            | Self::NoteFill
            | Self::NoteStroke
            | Self::ActivationFill
            | Self::ActivationStroke
            | Self::MessageStroke
            | Self::ClusterFill
            | Self::ClusterStroke
            | Self::LifelineStroke
            | Self::TitleFill
            | Self::RequirementFill
            | Self::PieSliceStroke
            | Self::ActorLabelFill
            | Self::MessageLabelFill
            | Self::LoopFill
            | Self::LoopStroke
            | Self::LoopLabelFill
            | Self::NoteLabelFill
            | Self::TextFill
            | Self::ChartTextPaint
            | Self::GanttTaskDefaultFill
            | Self::GanttTaskActiveFill
            | Self::GanttTaskSuccessFill
            | Self::GanttTaskErrorFill
            | Self::NodeLabelFill
            | Self::ClusterLabelFill
            | Self::EdgeLabelBackgroundFill
            | Self::EdgeLabelFill
            | Self::PieSliceFill
            | Self::RequirementStroke
            | Self::GanttTaskDefaultStroke
            | Self::GanttTaskActiveStroke
            | Self::GanttTaskSuccessStroke
            | Self::GanttTaskErrorStroke
            | Self::KanbanTaskStroke
            | Self::JourneyTaskFill
            | Self::JourneyTaskStroke
            | Self::TreeViewMarkerPaint
            | Self::GitGraphCommitLabelBackgroundFill => ThemeRouteCutoverProjectionAction::Replace,
            Self::GanttTaskWarningStroke
            | Self::TimelineEventFill
            | Self::ErTableOddFill
            | Self::ErTableEvenFill
            | Self::TimelineTextFill
            | Self::TimelineEventStroke
            | Self::RequirementTextFill
            | Self::RadarLinePaint
            | Self::RadarAxisPaint
            | Self::RadarTextPaint
            | Self::RequirementRelationPaint => ThemeRouteCutoverProjectionAction::Replace,
        }
    }

    #[cfg(any(test, merman_internal_theme_acceptance))]
    const fn bit(self) -> u64 {
        1 << self as u8
    }
}

/// What typed ownership must prove about a legacy projection.
#[cfg(any(test, merman_internal_theme_acceptance))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeRouteCutoverProjectionAction {
    Replace,
    RetireFallback,
}

#[cfg(any(test, merman_internal_theme_acceptance))]
impl ThemeRouteCutoverProjectionAction {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::RetireFallback => "retire-fallback",
        }
    }
}

/// Fixed, canonical set of legacy projection obligations for one route.
#[cfg(any(test, merman_internal_theme_acceptance))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeRouteCutoverProjectionSet(u64);

#[cfg(any(test, merman_internal_theme_acceptance))]
impl ThemeRouteCutoverProjectionSet {
    pub const REPLACE_NODE_FILL: Self = Self::replacing(ThemeRouteCutoverProjection::NodeFill);
    pub const REPLACE_NODE_STROKE: Self = Self::replacing(ThemeRouteCutoverProjection::NodeStroke);
    pub const REPLACE_NODE_LABEL_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::NodeLabelFill);
    pub const REPLACE_EDGE_LABEL_BACKGROUND_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::EdgeLabelBackgroundFill);
    pub const REPLACE_EDGE_LABEL_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::EdgeLabelFill);
    pub const REPLACE_EDGE_STROKE: Self = Self::replacing(ThemeRouteCutoverProjection::EdgeStroke);
    pub const REPLACE_EDGE_STROKE_AND_RETIRE_MARKER_FALLBACK: Self = Self::edge_stroke();
    pub const REPLACE_ACTOR_FILL: Self = Self::replacing(ThemeRouteCutoverProjection::ActorFill);
    pub const REPLACE_ACTOR_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::ActorStroke);
    pub const REPLACE_NOTE_FILL: Self = Self::replacing(ThemeRouteCutoverProjection::NoteFill);
    pub const REPLACE_NOTE_STROKE: Self = Self::replacing(ThemeRouteCutoverProjection::NoteStroke);
    pub const REPLACE_ACTIVATION_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::ActivationFill);
    pub const REPLACE_ACTIVATION_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::ActivationStroke);
    pub const REPLACE_MESSAGE_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::MessageStroke);
    pub const REPLACE_CLUSTER_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::ClusterFill);
    pub const REPLACE_CLUSTER_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::ClusterStroke);
    pub const REPLACE_LIFELINE_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::LifelineStroke);
    pub const REPLACE_TITLE_FILL: Self = Self::replacing(ThemeRouteCutoverProjection::TitleFill);
    pub const REPLACE_CLUSTER_LABEL_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::ClusterLabelFill);
    pub const REPLACE_RADAR_TEXT_PAINT: Self =
        Self::replacing(ThemeRouteCutoverProjection::RadarTextPaint);
    pub const REPLACE_RADAR_AXIS_PAINT: Self = Self(
        ThemeRouteCutoverProjection::RadarLinePaint.bit()
            | ThemeRouteCutoverProjection::RadarAxisPaint.bit(),
    );
    pub const REPLACE_FLOWCHART_TEXT_FILL: Self = Self(
        ThemeRouteCutoverProjection::NodeLabelFill.bit()
            | ThemeRouteCutoverProjection::TitleFill.bit()
            | ThemeRouteCutoverProjection::ClusterLabelFill.bit(),
    );
    pub const REPLACE_CLASS_TEXT_FILL: Self = Self(
        ThemeRouteCutoverProjection::NodeLabelFill.bit()
            | ThemeRouteCutoverProjection::TitleFill.bit()
            | ThemeRouteCutoverProjection::ClusterLabelFill.bit(),
    );
    pub const REPLACE_CHART_TEXT_PAINT: Self =
        Self::replacing(ThemeRouteCutoverProjection::ChartTextPaint);
    pub const REPLACE_TEXT_FILL: Self = Self::replacing(ThemeRouteCutoverProjection::TextFill);
    pub const REPLACE_GITGRAPH_TEXT_FILL: Self = Self(
        ThemeRouteCutoverProjection::TextFill.bit()
            | ThemeRouteCutoverProjection::NodeLabelFill.bit()
            | ThemeRouteCutoverProjection::EdgeLabelFill.bit(),
    );
    pub const REPLACE_C4_TEXT_FILL: Self = Self(
        ThemeRouteCutoverProjection::TextFill.bit()
            | ThemeRouteCutoverProjection::C4TitleFillFallback.bit(),
    );
    pub const REPLACE_REQUIREMENT_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::RequirementFill);
    pub const REPLACE_REQUIREMENT_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::RequirementStroke);
    pub const REPLACE_REQUIREMENT_RELATION_PAINT: Self =
        Self::replacing(ThemeRouteCutoverProjection::RequirementRelationPaint);
    pub const REPLACE_PIE_SLICE_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::PieSliceStroke);
    pub const REPLACE_PIE_SLICE_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::PieSliceFill);
    pub const REPLACE_ACTOR_LABEL_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::ActorLabelFill);
    pub const REPLACE_MESSAGE_LABEL_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::MessageLabelFill);
    pub const REPLACE_LOOP_FILL: Self = Self::replacing(ThemeRouteCutoverProjection::LoopFill);
    pub const REPLACE_LOOP_STROKE: Self = Self::replacing(ThemeRouteCutoverProjection::LoopStroke);
    pub const REPLACE_LOOP_LABEL_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::LoopLabelFill);
    pub const REPLACE_NOTE_LABEL_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::NoteLabelFill);
    pub const REPLACE_GANTT_TASK_FILLS: Self = Self(
        ThemeRouteCutoverProjection::GanttTaskDefaultFill.bit()
            | ThemeRouteCutoverProjection::GanttTaskActiveFill.bit()
            | ThemeRouteCutoverProjection::GanttTaskSuccessFill.bit()
            | ThemeRouteCutoverProjection::GanttTaskErrorFill.bit(),
    );
    pub const REPLACE_GANTT_TASK_STROKES: Self = Self(
        ThemeRouteCutoverProjection::GanttTaskDefaultStroke.bit()
            | ThemeRouteCutoverProjection::GanttTaskActiveStroke.bit()
            | ThemeRouteCutoverProjection::GanttTaskSuccessStroke.bit()
            | ThemeRouteCutoverProjection::GanttTaskErrorStroke.bit(),
    );
    pub const REPLACE_GANTT_TASK_DEFAULT_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::GanttTaskDefaultFill);
    pub const REPLACE_GANTT_TASK_ACTIVE_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::GanttTaskActiveFill);
    pub const REPLACE_GANTT_TASK_SUCCESS_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::GanttTaskSuccessFill);
    pub const REPLACE_GANTT_TASK_ERROR_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::GanttTaskErrorFill);
    pub const REPLACE_GANTT_TASK_DEFAULT_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::GanttTaskDefaultStroke);
    pub const REPLACE_GANTT_TASK_ACTIVE_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::GanttTaskActiveStroke);
    pub const REPLACE_GANTT_TASK_SUCCESS_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::GanttTaskSuccessStroke);
    pub const REPLACE_GANTT_TASK_ERROR_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::GanttTaskErrorStroke);
    pub const REPLACE_JOURNEY_TASK_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::JourneyTaskFill);
    pub const REPLACE_JOURNEY_TASK_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::JourneyTaskStroke);
    pub const REPLACE_KANBAN_TASK_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::KanbanTaskStroke);
    pub const REPLACE_TREE_VIEW_MARKER_PAINT: Self =
        Self::replacing(ThemeRouteCutoverProjection::TreeViewMarkerPaint);
    pub const REPLACE_GITGRAPH_COMMIT_LABEL_BACKGROUND_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::GitGraphCommitLabelBackgroundFill);
    pub const REPLACE_GANTT_TASK_WARNING_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::GanttTaskWarningStroke);
    pub const REPLACE_TIMELINE_EVENT_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::TimelineEventFill);
    pub const REPLACE_TIMELINE_TEXT_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::TimelineTextFill);
    pub const REPLACE_TIMELINE_EVENT_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::TimelineEventStroke);
    pub const REPLACE_REQUIREMENT_TEXT_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::RequirementTextFill);
    pub const REPLACE_ER_TABLE_FILL: Self = Self(
        ThemeRouteCutoverProjection::ErTableOddFill.bit()
            | ThemeRouteCutoverProjection::ErTableEvenFill.bit(),
    );
    pub const REPLACE_ER_TABLE_ODD_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::ErTableOddFill);
    pub const REPLACE_ER_TABLE_EVEN_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::ErTableEvenFill);

    pub(crate) const fn replacing(projection: ThemeRouteCutoverProjection) -> Self {
        Self(projection.bit())
    }

    pub(crate) const fn edge_stroke() -> Self {
        Self(
            ThemeRouteCutoverProjection::EdgeStroke.bit()
                | ThemeRouteCutoverProjection::MarkerPaintFromEdge.bit(),
        )
    }

    pub const fn contains(self, projection: ThemeRouteCutoverProjection) -> bool {
        self.0 & projection.bit() != 0
    }

    pub fn iter(self) -> impl Iterator<Item = ThemeRouteCutoverProjection> {
        ThemeRouteCutoverProjection::ALL
            .into_iter()
            .filter(move |projection| self.contains(*projection))
    }

    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Workspace-private route plus the legacy projections that typed ownership replaces or retires.
///
/// This is an inventory projection, not terminal evidence. Only the non-published C6 harness can
/// combine it with finalized SVG and PNG receipts to authorize a cutover.
#[cfg(any(test, merman_internal_theme_acceptance))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeRouteCutoverDescriptor {
    id: ThemeRouteCutoverId,
    projections: ThemeRouteCutoverProjectionSet,
}

#[cfg(any(test, merman_internal_theme_acceptance))]
impl ThemeRouteCutoverDescriptor {
    pub(crate) const fn new(
        id: ThemeRouteCutoverId,
        projections: ThemeRouteCutoverProjectionSet,
    ) -> Self {
        Self { id, projections }
    }

    pub const fn id(self) -> ThemeRouteCutoverId {
        self.id
    }

    pub const fn family_id(self) -> DiagramFamilyId {
        self.id.family_id()
    }

    pub const fn target(self) -> ThemeTarget {
        self.id.target()
    }

    pub const fn selector(self) -> ThemeRouteCutoverSelector {
        self.id.selector()
    }

    pub const fn facet(self) -> ThemeRouteCutoverFacet {
        self.id.facet()
    }

    pub const fn value(self) -> ThemeRouteCutoverValue {
        self.id.value()
    }

    pub const fn projections(self) -> ThemeRouteCutoverProjectionSet {
        self.projections
    }

    /// Returns whether this route relies on renderer-owned terminal bindings for raster proof.
    ///
    /// Sequence emits Lifeline fill and stroke through the same native stroke channel. Once that
    /// mapping leaves the public SVG, the raster witness must not infer it from the final artifact.
    pub const fn requires_renderer_raster_binding_receipt(self) -> bool {
        self.projections
            .contains(ThemeRouteCutoverProjection::LifelineStroke)
    }

    /// Returns the SVG paint channel used by the renderer for this semantic route.
    ///
    /// Sequence Message fill, Block/Flowchart/Swimlane/GitGraph/Class Edge fill, Radar Axis fill,
    /// and ER/Requirement Relation fill control native stroke colors. These routes
    /// are observed in the emitted stroke channel during raster admission, while their semantic
    /// facet remains `Fill` in the route identity and evidence.
    pub fn raster_paint_facet(self) -> ThemeRouteCutoverFacet {
        if self.family_id() == DiagramFamilyId::TREE_VIEW && self.target() == ThemeTarget::Marker {
            // Tree View exposes both semantic Marker facets through the icon's `currentColor`
            // fill terminal. The raster proof must inspect the emitted fill channel for either
            // route instead of pretending that an SVG stroke exists.
            ThemeRouteCutoverFacet::Fill
        } else if matches!(self.facet(), ThemeRouteCutoverFacet::Fill)
            && ((self.family_id() == DiagramFamilyId::SEQUENCE
                && self.target() == ThemeTarget::Message)
                || (matches!(
                    self.family_id(),
                    DiagramFamilyId::GIT_GRAPH
                        | DiagramFamilyId::BLOCK
                        | DiagramFamilyId::CLASS
                        | DiagramFamilyId::FLOWCHART
                        | DiagramFamilyId::SWIMLANE
                ) && self.target() == ThemeTarget::Edge)
                || (self.family_id() == DiagramFamilyId::RADAR
                    && self.target() == ThemeTarget::Axis)
                || (matches!(
                    self.family_id(),
                    DiagramFamilyId::ER | DiagramFamilyId::REQUIREMENT
                ) && self.target() == ThemeTarget::Relation))
        {
            ThemeRouteCutoverFacet::Stroke
        } else {
            self.facet()
        }
    }

    /// Whether this route can write both SVG fill and stroke channels.
    ///
    /// XY Chart Axis.fill colors axis text and falls back into axis lines and ticks.
    /// Explicit config can own either group independently, leaving only the other
    /// channel visible. The semantic route identity remains Axis.fill in both cases.
    pub fn raster_paint_allows_fill_or_stroke(self) -> bool {
        self.family_id() == DiagramFamilyId::XY_CHART
            && self.target() == ThemeTarget::Axis
            && self.facet() == ThemeRouteCutoverFacet::Fill
            && self
                .projections()
                .contains(ThemeRouteCutoverProjection::ChartTextPaint)
    }

    /// Returns the route-local solid paint used by the private raster cutover witness.
    ///
    /// The value is renderer-owned so the acceptance harness cannot silently choose a different
    /// control palette from the route receipt it is trying to authorize.
    pub fn raster_control_css(self) -> String {
        let mut hash = 0x811c9dc5_u32;
        for bytes in [
            self.family_id().as_str().as_bytes(),
            self.target().id().as_bytes(),
            self.selector().id().as_bytes(),
            route_facet_id(self.facet()).as_bytes(),
        ] {
            for byte in bytes {
                hash = (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193);
            }
        }
        let rgb = [
            ((hash >> 16) as u8 & 0x7f).saturating_add(0x40),
            ((hash >> 8) as u8 & 0x7f).saturating_add(0x40),
            (hash as u8 & 0x7f).saturating_add(0x40),
        ];
        format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
    }
}

/// Renderer-owned route fact captured after the family adapter reports a terminal application.
#[cfg(merman_internal_theme_acceptance)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ThemeRouteCutoverFact {
    descriptor: ThemeRouteCutoverDescriptor,
    mechanism_index: usize,
}

#[cfg(merman_internal_theme_acceptance)]
impl ThemeRouteCutoverFact {
    pub(crate) const fn descriptor(self) -> ThemeRouteCutoverDescriptor {
        self.descriptor
    }

    const fn mechanism_index(self) -> usize {
        self.mechanism_index
    }
}

/// Opaque production receipt for one finalized typed route that replaced a legacy projection.
///
/// The acceptance harness may correlate this receipt with the route manifest, but it cannot
/// reconstruct the writer evidence or manufacture a passing digest from expectation fields.
#[cfg(merman_internal_theme_acceptance)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeRouteCutoverReceipt {
    descriptor: ThemeRouteCutoverDescriptor,
    mechanism_index: usize,
    artifact_digest: [u8; 32],
    native_artifact_digest: [u8; 32],
    digest: [u8; 32],
}

#[cfg(merman_internal_theme_acceptance)]
impl ThemeRouteCutoverReceipt {
    pub(crate) fn seal(
        fact: ThemeRouteCutoverFact,
        artifact_digest: [u8; 32],
        native_artifact_digest: [u8; 32],
    ) -> Option<Self> {
        (artifact_digest != [0; 32] && native_artifact_digest != [0; 32]).then(|| {
            let digest =
                route_cutover_receipt_digest(fact, artifact_digest, native_artifact_digest);
            Self {
                descriptor: fact.descriptor(),
                mechanism_index: fact.mechanism_index(),
                artifact_digest,
                native_artifact_digest,
                digest,
            }
        })
    }

    pub const fn descriptor(self) -> ThemeRouteCutoverDescriptor {
        self.descriptor
    }

    pub const fn artifact_digest(self) -> [u8; 32] {
        self.artifact_digest
    }

    pub const fn native_artifact_digest(self) -> [u8; 32] {
        self.native_artifact_digest
    }

    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    pub fn proves_artifact(self, artifact_digest: [u8; 32]) -> bool {
        self.proves_artifacts(artifact_digest, self.native_artifact_digest)
    }

    pub fn proves_artifacts(
        self,
        artifact_digest: [u8; 32],
        native_artifact_digest: [u8; 32],
    ) -> bool {
        artifact_digest != [0; 32]
            && native_artifact_digest != [0; 32]
            && self.artifact_digest == artifact_digest
            && self.native_artifact_digest == native_artifact_digest
            && self.digest
                == route_cutover_receipt_digest(
                    ThemeRouteCutoverFact {
                        descriptor: self.descriptor,
                        mechanism_index: self.mechanism_index,
                    },
                    self.artifact_digest,
                    self.native_artifact_digest,
                )
    }
}

#[cfg(merman_internal_theme_acceptance)]
fn route_cutover_receipt_digest(
    fact: ThemeRouteCutoverFact,
    artifact_digest: [u8; 32],
    native_artifact_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"merman.theme-route-cutover-receipt.v2\0");
    hasher.update(fact.descriptor.family_id().as_str().as_bytes());
    hasher.update([0]);
    hasher.update(fact.descriptor.target().id().as_bytes());
    hasher.update([0]);
    hasher.update(fact.descriptor.selector().id().as_bytes());
    hasher.update([0]);
    hasher.update(route_facet_id(fact.descriptor.facet()).as_bytes());
    hasher.update([0]);
    hasher.update(route_value_id(fact.descriptor.value()).as_bytes());
    hasher.update([0]);
    hasher.update(fact.descriptor.projections().0.to_be_bytes());
    hasher.update((fact.mechanism_index() as u64).to_be_bytes());
    hasher.update(artifact_digest);
    hasher.update(native_artifact_digest);
    hasher.finalize().into()
}

#[cfg(any(test, merman_internal_theme_acceptance))]
const fn route_facet_id(facet: ThemeRouteCutoverFacet) -> &'static str {
    match facet {
        ThemeRouteCutoverFacet::Fill => "fill",
        ThemeRouteCutoverFacet::Stroke => "stroke",
    }
}

#[cfg(any(test, merman_internal_theme_acceptance))]
const fn route_value_id(value: ThemeRouteCutoverValue) -> &'static str {
    match value {
        ThemeRouteCutoverValue::Transparent => "transparent",
        ThemeRouteCutoverValue::Solid => "solid",
    }
}

#[cfg(merman_internal_theme_acceptance)]
pub(crate) fn collect_theme_route_cutover_facts(
    theme: &ResolvedDiagramTheme,
    applied: &[FamilyThemeMechanismKey],
) -> Vec<ThemeRouteCutoverFact> {
    let Ok(descriptors) = crate::diagram_theme::legacy_replacing_typed_routes() else {
        return Vec::new();
    };
    let routes = theme.family_mechanism_routes();
    descriptors
        .into_iter()
        .filter(|descriptor| descriptor.family_id() == theme.family_id())
        .filter_map(|descriptor| {
            routes.iter().copied().find_map(|route| {
                let FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target,
                    selector,
                    facet,
                } = route.mechanism()
                else {
                    return None;
                };
                if route.disposition() != crate::diagram_theme::FamilyThemeDisposition::TypedAdapter
                    || target != descriptor.target()
                    || !route_selector_matches(selector, descriptor.selector())
                    || !route_facet_matches(facet, descriptor)
                {
                    return None;
                }
                let key = FamilyThemeMechanismKey::Rule {
                    index: rule_index,
                    target,
                };
                applied.contains(&key).then_some(ThemeRouteCutoverFact {
                    descriptor,
                    mechanism_index: rule_index,
                })
            })
        })
        .collect()
}

#[cfg(merman_internal_theme_acceptance)]
fn route_selector_matches(
    selector: crate::diagram_theme::FamilyThemeSelectorShape,
    expected: ThemeRouteCutoverSelector,
) -> bool {
    let expected_variant = match expected {
        ThemeRouteCutoverSelector::StaticUnqualified => None,
        ThemeRouteCutoverSelector::StaticVariant(variant) => Some(variant),
    };
    matches!(
        selector,
        crate::diagram_theme::FamilyThemeSelectorShape::Static { variant }
            if variant == expected_variant
    )
}

#[cfg(merman_internal_theme_acceptance)]
fn route_facet_matches(
    facet: FamilyThemeRuleFacet,
    descriptor: ThemeRouteCutoverDescriptor,
) -> bool {
    let expected = match descriptor.value() {
        ThemeRouteCutoverValue::Transparent => FamilyThemePaintKind::Transparent,
        ThemeRouteCutoverValue::Solid => FamilyThemePaintKind::Solid,
    };
    match (descriptor.facet(), facet) {
        (ThemeRouteCutoverFacet::Fill, FamilyThemeRuleFacet::Fill(kind))
        | (ThemeRouteCutoverFacet::Stroke, FamilyThemeRuleFacet::Stroke(kind)) => kind == expected,
        _ => false,
    }
}

#[cfg(merman_internal_theme_acceptance)]
pub(crate) fn seal_theme_route_cutover_receipts(
    facts: &[ThemeRouteCutoverFact],
    artifact_digest: [u8; 32],
    native_artifact_digest: [u8; 32],
) -> Vec<ThemeRouteCutoverReceipt> {
    facts
        .iter()
        .copied()
        .filter_map(|fact| {
            ThemeRouteCutoverReceipt::seal(fact, artifact_digest, native_artifact_digest)
        })
        .collect()
}

/// Failure to bind a typed legacy-replacing route to every bridge projection it owns.
#[cfg(any(test, merman_internal_theme_acceptance))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeRouteCutoverInventoryError {
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    selector: ThemeRouteCutoverSelector,
    facet: ThemeRouteCutoverFacet,
}

#[cfg(any(test, merman_internal_theme_acceptance))]
impl ThemeRouteCutoverInventoryError {
    pub(crate) const fn missing_projections(
        family_id: DiagramFamilyId,
        target: ThemeTarget,
        selector: ThemeRouteCutoverSelector,
        facet: ThemeRouteCutoverFacet,
    ) -> Self {
        Self {
            family_id,
            target,
            selector,
            facet,
        }
    }
}

#[cfg(any(test, merman_internal_theme_acceptance))]
impl fmt::Display for ThemeRouteCutoverInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "typed legacy-replacing route {}/{}/{}/{:?} lacks projection obligations",
            self.family_id,
            self.target.id(),
            self.selector.id(),
            self.facet
        )
    }
}

#[cfg(any(test, merman_internal_theme_acceptance))]
impl std::error::Error for ThemeRouteCutoverInventoryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_message_stroke_projection_is_an_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::MessageStroke;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_MESSAGE_STROKE
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "message.stroke");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);

        let descriptor = ThemeRouteCutoverDescriptor::new(
            ThemeRouteCutoverId::new(
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Message,
                ThemeRouteCutoverSelector::StaticUnqualified,
                ThemeRouteCutoverFacet::Stroke,
                ThemeRouteCutoverValue::Solid,
            ),
            ThemeRouteCutoverProjectionSet::REPLACE_MESSAGE_STROKE,
        );
        assert!(!descriptor.requires_renderer_raster_binding_receipt());
    }

    #[test]
    fn sequence_message_fill_is_observed_in_the_shared_stroke_channel() {
        let descriptor = ThemeRouteCutoverDescriptor::new(
            ThemeRouteCutoverId::new(
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Message,
                ThemeRouteCutoverSelector::StaticUnqualified,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverValue::Solid,
            ),
            ThemeRouteCutoverProjectionSet::REPLACE_MESSAGE_STROKE,
        );

        assert_eq!(
            descriptor.raster_paint_facet(),
            ThemeRouteCutoverFacet::Stroke
        );
    }

    #[test]
    fn only_xychart_axis_fill_allows_both_native_paint_channels() {
        for family in [
            DiagramFamilyId::XY_CHART,
            DiagramFamilyId::QUADRANT_CHART,
            DiagramFamilyId::RADAR,
        ] {
            for target in [ThemeTarget::Text, ThemeTarget::Title, ThemeTarget::Axis] {
                for facet in [ThemeRouteCutoverFacet::Fill, ThemeRouteCutoverFacet::Stroke] {
                    for selector in [
                        ThemeRouteCutoverSelector::StaticUnqualified,
                        ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
                    ] {
                        for value in [
                            ThemeRouteCutoverValue::Solid,
                            ThemeRouteCutoverValue::Transparent,
                        ] {
                            let descriptor = ThemeRouteCutoverDescriptor::new(
                                ThemeRouteCutoverId::new(family, target, selector, facet, value),
                                ThemeRouteCutoverProjectionSet::REPLACE_CHART_TEXT_PAINT,
                            );
                            assert_eq!(
                                descriptor.raster_paint_allows_fill_or_stroke(),
                                family == DiagramFamilyId::XY_CHART
                                    && target == ThemeTarget::Axis
                                    && facet == ThemeRouteCutoverFacet::Fill,
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn edge_fill_is_observed_in_the_native_stroke_channel() {
        for family in [
            DiagramFamilyId::BLOCK,
            DiagramFamilyId::GIT_GRAPH,
            DiagramFamilyId::CLASS,
            DiagramFamilyId::FLOWCHART,
            DiagramFamilyId::SWIMLANE,
        ] {
            for selector in [
                ThemeRouteCutoverSelector::StaticUnqualified,
                ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
            ] {
                for value in [
                    ThemeRouteCutoverValue::Solid,
                    ThemeRouteCutoverValue::Transparent,
                ] {
                    let descriptor = ThemeRouteCutoverDescriptor::new(
                        ThemeRouteCutoverId::new(
                            family,
                            ThemeTarget::Edge,
                            selector,
                            ThemeRouteCutoverFacet::Fill,
                            value,
                        ),
                        if family == DiagramFamilyId::GIT_GRAPH {
                            ThemeRouteCutoverProjectionSet::REPLACE_EDGE_STROKE
                        } else {
                            ThemeRouteCutoverProjectionSet::REPLACE_EDGE_STROKE_AND_RETIRE_MARKER_FALLBACK
                        },
                    );
                    assert_eq!(descriptor.facet(), ThemeRouteCutoverFacet::Fill);
                    assert_eq!(
                        descriptor.raster_paint_facet(),
                        ThemeRouteCutoverFacet::Stroke
                    );
                }
            }
        }
    }

    #[test]
    fn er_relation_fill_is_observed_in_the_relation_stroke_channel() {
        for selector in [
            ThemeRouteCutoverSelector::StaticUnqualified,
            ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
        ] {
            for value in [
                ThemeRouteCutoverValue::Solid,
                ThemeRouteCutoverValue::Transparent,
            ] {
                let descriptor = ThemeRouteCutoverDescriptor::new(
                    ThemeRouteCutoverId::new(
                        DiagramFamilyId::ER,
                        ThemeTarget::Relation,
                        selector,
                        ThemeRouteCutoverFacet::Fill,
                        value,
                    ),
                    ThemeRouteCutoverProjectionSet::REPLACE_EDGE_STROKE,
                );
                assert_eq!(descriptor.facet(), ThemeRouteCutoverFacet::Fill);
                assert_eq!(
                    descriptor.raster_paint_facet(),
                    ThemeRouteCutoverFacet::Stroke
                );
            }
        }
    }

    #[test]
    fn tree_view_marker_paint_is_observed_in_the_native_fill_channel() {
        for facet in [ThemeRouteCutoverFacet::Fill, ThemeRouteCutoverFacet::Stroke] {
            let descriptor = ThemeRouteCutoverDescriptor::new(
                ThemeRouteCutoverId::new(
                    DiagramFamilyId::TREE_VIEW,
                    ThemeTarget::Marker,
                    ThemeRouteCutoverSelector::StaticUnqualified,
                    facet,
                    ThemeRouteCutoverValue::Solid,
                ),
                ThemeRouteCutoverProjectionSet::REPLACE_TREE_VIEW_MARKER_PAINT,
            );

            assert_eq!(
                descriptor.raster_paint_facet(),
                ThemeRouteCutoverFacet::Fill
            );
        }
    }

    #[test]
    fn sequence_lifeline_paint_projection_is_one_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::LifelineStroke;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_LIFELINE_STROKE
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "lifeline.stroke");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);

        let descriptor = ThemeRouteCutoverDescriptor::new(
            ThemeRouteCutoverId::new(
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Lifeline,
                ThemeRouteCutoverSelector::StaticUnqualified,
                ThemeRouteCutoverFacet::Stroke,
                ThemeRouteCutoverValue::Solid,
            ),
            ThemeRouteCutoverProjectionSet::REPLACE_LIFELINE_STROKE,
        );
        assert!(descriptor.requires_renderer_raster_binding_receipt());
    }

    #[test]
    fn cluster_paint_projections_are_exact_replacements() {
        for (projection, set, contribution_id) in [
            (
                ThemeRouteCutoverProjection::ClusterFill,
                ThemeRouteCutoverProjectionSet::REPLACE_CLUSTER_FILL,
                "cluster.fill",
            ),
            (
                ThemeRouteCutoverProjection::ClusterStroke,
                ThemeRouteCutoverProjectionSet::REPLACE_CLUSTER_STROKE,
                "cluster.stroke",
            ),
        ] {
            assert_eq!(projection.contribution_id(), contribution_id);
            assert_eq!(
                projection.action(),
                ThemeRouteCutoverProjectionAction::Replace
            );
            assert_eq!(set.iter().collect::<Vec<_>>(), vec![projection]);
        }
    }

    #[test]
    fn title_fill_projection_is_an_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::TitleFill;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_TITLE_FILL
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "title.fill");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);
    }

    #[test]
    fn text_fill_projection_is_an_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::TextFill;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_TEXT_FILL
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "text.fill");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);
    }

    #[test]
    fn gitgraph_text_fill_replaces_all_three_inherited_projections() {
        for (set, projection) in [
            (
                ThemeRouteCutoverProjectionSet::REPLACE_NODE_LABEL_FILL,
                ThemeRouteCutoverProjection::NodeLabelFill,
            ),
            (
                ThemeRouteCutoverProjectionSet::REPLACE_EDGE_LABEL_FILL,
                ThemeRouteCutoverProjection::EdgeLabelFill,
            ),
        ] {
            assert_eq!(set.iter().collect::<Vec<_>>(), vec![projection]);
            assert_eq!(
                projection.action(),
                ThemeRouteCutoverProjectionAction::Replace
            );
        }
        assert_eq!(
            ThemeRouteCutoverProjectionSet::REPLACE_GITGRAPH_TEXT_FILL
                .iter()
                .map(|projection| (projection.contribution_id(), projection.action()))
                .collect::<Vec<_>>(),
            vec![
                ("text.fill", ThemeRouteCutoverProjectionAction::Replace),
                (
                    "node-label.fill",
                    ThemeRouteCutoverProjectionAction::Replace
                ),
                (
                    "edge-label.fill",
                    ThemeRouteCutoverProjectionAction::Replace
                ),
            ]
        );
    }

    #[test]
    fn c4_text_fill_replaces_root_paint_and_retires_the_unused_title_fallback() {
        assert_eq!(
            ThemeRouteCutoverProjectionSet::REPLACE_C4_TEXT_FILL
                .iter()
                .map(|projection| (projection.contribution_id(), projection.action()))
                .collect::<Vec<_>>(),
            vec![
                ("text.fill", ThemeRouteCutoverProjectionAction::Replace),
                (
                    "title.fill",
                    ThemeRouteCutoverProjectionAction::RetireFallback
                ),
            ]
        );
    }

    #[test]
    fn er_unqualified_table_fill_replaces_both_row_projections() {
        assert_eq!(
            ThemeRouteCutoverProjectionSet::REPLACE_ER_TABLE_FILL
                .iter()
                .map(|projection| (projection.contribution_id(), projection.action()))
                .collect::<Vec<_>>(),
            vec![
                ("table.odd.fill", ThemeRouteCutoverProjectionAction::Replace),
                (
                    "table.even.fill",
                    ThemeRouteCutoverProjectionAction::Replace
                ),
            ],
        );
    }

    #[test]
    fn flowchart_generic_text_replaces_all_inherited_color_projections() {
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_FLOWCHART_TEXT_FILL
            .iter()
            .map(|projection| (projection.contribution_id(), projection.action()))
            .collect::<Vec<_>>();
        assert_eq!(
            projections,
            vec![
                ("title.fill", ThemeRouteCutoverProjectionAction::Replace),
                (
                    "node-label.fill",
                    ThemeRouteCutoverProjectionAction::Replace
                ),
                (
                    "cluster-label.fill",
                    ThemeRouteCutoverProjectionAction::Replace
                ),
            ]
        );
        assert_eq!(ThemeRouteCutoverProjection::ClusterLabelFill as u8, 53);
    }

    #[test]
    fn appended_projection_discriminants_keep_existing_private_slots() {
        assert_eq!(ThemeRouteCutoverProjection::JourneyTaskFill as u8, 35);
        assert_eq!(ThemeRouteCutoverProjection::KanbanTaskStroke as u8, 36);
        assert_eq!(ThemeRouteCutoverProjection::JourneyTaskStroke as u8, 37);
        assert_eq!(ThemeRouteCutoverProjection::TreeViewMarkerPaint as u8, 38);
        assert_eq!(
            ThemeRouteCutoverProjection::GitGraphCommitLabelBackgroundFill as u8,
            39
        );
        assert_eq!(
            ThemeRouteCutoverProjection::GanttTaskWarningStroke as u8,
            40
        );
        assert_eq!(ThemeRouteCutoverProjection::TimelineEventFill as u8, 41);
        assert_eq!(ThemeRouteCutoverProjection::C4TitleFillFallback as u8, 42);
        assert_eq!(ThemeRouteCutoverProjection::EdgeLabelFill as u8, 43);
        assert_eq!(ThemeRouteCutoverProjection::ErTableOddFill as u8, 44);
        assert_eq!(ThemeRouteCutoverProjection::ErTableEvenFill as u8, 45);
        assert_eq!(
            ThemeRouteCutoverProjection::RequirementRelationPaint as u8,
            46
        );
        assert_eq!(ThemeRouteCutoverProjection::TimelineTextFill as u8, 47);
        assert_eq!(ThemeRouteCutoverProjection::TimelineEventStroke as u8, 48);
        assert_eq!(ThemeRouteCutoverProjection::RequirementTextFill as u8, 49);
        assert_eq!(ThemeRouteCutoverProjection::RadarLinePaint as u8, 50);
        assert_eq!(ThemeRouteCutoverProjection::RadarAxisPaint as u8, 51);
        assert_eq!(ThemeRouteCutoverProjection::RadarTextPaint as u8, 52);
    }

    #[test]
    fn gitgraph_commit_label_background_projection_is_an_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::GitGraphCommitLabelBackgroundFill;
        let projections =
            ThemeRouteCutoverProjectionSet::REPLACE_GITGRAPH_COMMIT_LABEL_BACKGROUND_FILL
                .iter()
                .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "commit-label-background.fill");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);
    }

    #[test]
    fn journey_paint_projections_are_property_local() {
        for (projection, set, contribution_id) in [
            (
                ThemeRouteCutoverProjection::JourneyTaskFill,
                ThemeRouteCutoverProjectionSet::REPLACE_JOURNEY_TASK_FILL,
                "task.fill",
            ),
            (
                ThemeRouteCutoverProjection::JourneyTaskStroke,
                ThemeRouteCutoverProjectionSet::REPLACE_JOURNEY_TASK_STROKE,
                "task.stroke",
            ),
        ] {
            assert_eq!(projection.contribution_id(), contribution_id);
            assert_eq!(
                projection.action(),
                ThemeRouteCutoverProjectionAction::Replace
            );
            assert_eq!(set.iter().collect::<Vec<_>>(), vec![projection]);
        }
    }

    #[test]
    fn timeline_paint_projections_are_property_local() {
        for (projection, set, contribution_id) in [
            (
                ThemeRouteCutoverProjection::TimelineEventFill,
                ThemeRouteCutoverProjectionSet::REPLACE_TIMELINE_EVENT_FILL,
                "event.fill",
            ),
            (
                ThemeRouteCutoverProjection::TimelineTextFill,
                ThemeRouteCutoverProjectionSet::REPLACE_TIMELINE_TEXT_FILL,
                "event.text",
            ),
            (
                ThemeRouteCutoverProjection::TimelineEventStroke,
                ThemeRouteCutoverProjectionSet::REPLACE_TIMELINE_EVENT_STROKE,
                "event.stroke",
            ),
        ] {
            assert_eq!(projection.contribution_id(), contribution_id);
            assert_eq!(
                projection.action(),
                ThemeRouteCutoverProjectionAction::Replace
            );
            assert_eq!(set.iter().collect::<Vec<_>>(), vec![projection]);
        }
    }

    #[test]
    fn gantt_task_fill_projections_preserve_state_specific_bridge_identity() {
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_GANTT_TASK_FILLS
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(
            projections,
            vec![
                ThemeRouteCutoverProjection::GanttTaskDefaultFill,
                ThemeRouteCutoverProjection::GanttTaskActiveFill,
                ThemeRouteCutoverProjection::GanttTaskSuccessFill,
                ThemeRouteCutoverProjection::GanttTaskErrorFill,
            ]
        );
        assert_eq!(
            projections
                .iter()
                .map(|projection| projection.contribution_id())
                .collect::<Vec<_>>(),
            vec![
                "task.default.fill",
                "task.active.fill",
                "task.success.fill",
                "task.error.fill",
            ]
        );
        assert!(projections.iter().all(|projection| {
            projection.action() == ThemeRouteCutoverProjectionAction::Replace
        }));
    }

    #[test]
    fn requirement_fill_projection_is_an_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::RequirementFill;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_REQUIREMENT_FILL
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "requirement.fill");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);
    }

    #[test]
    fn requirement_stroke_projection_is_an_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::RequirementStroke;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_REQUIREMENT_STROKE
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "requirement.paint");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);
    }

    #[test]
    fn requirement_relation_paint_replaces_one_contribution_in_the_stroke_channel() {
        let projection = ThemeRouteCutoverProjection::RequirementRelationPaint;
        assert_eq!(projection.contribution_id(), "relation.paint");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        for selector in [
            ThemeRouteCutoverSelector::StaticUnqualified,
            ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
        ] {
            for facet in [ThemeRouteCutoverFacet::Fill, ThemeRouteCutoverFacet::Stroke] {
                for value in [
                    ThemeRouteCutoverValue::Transparent,
                    ThemeRouteCutoverValue::Solid,
                ] {
                    let id = ThemeRouteCutoverId::new(
                        DiagramFamilyId::REQUIREMENT,
                        ThemeTarget::Relation,
                        selector,
                        facet,
                        value,
                    );
                    let descriptor = ThemeRouteCutoverDescriptor::new(
                        id,
                        ThemeRouteCutoverProjectionSet::REPLACE_REQUIREMENT_RELATION_PAINT,
                    );
                    assert_eq!(
                        descriptor.raster_paint_facet(),
                        ThemeRouteCutoverFacet::Stroke
                    );
                    assert_eq!(
                        descriptor.projections().iter().collect::<Vec<_>>(),
                        [projection]
                    );
                }
            }
        }
    }

    #[test]
    fn pie_slice_stroke_projection_is_an_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::PieSliceStroke;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_PIE_SLICE_STROKE
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "slice.stroke");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);
    }

    #[test]
    fn pie_slice_fill_projection_is_an_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::PieSliceFill;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_PIE_SLICE_FILL
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "slice.fill");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);
    }

    #[test]
    fn sequence_role_paint_projections_are_exact_replacements() {
        for (projection, set, contribution_id) in [
            (
                ThemeRouteCutoverProjection::ActorLabelFill,
                ThemeRouteCutoverProjectionSet::REPLACE_ACTOR_LABEL_FILL,
                "actor-label.fill",
            ),
            (
                ThemeRouteCutoverProjection::MessageLabelFill,
                ThemeRouteCutoverProjectionSet::REPLACE_MESSAGE_LABEL_FILL,
                "message-label.fill",
            ),
            (
                ThemeRouteCutoverProjection::LoopFill,
                ThemeRouteCutoverProjectionSet::REPLACE_LOOP_FILL,
                "loop.fill",
            ),
            (
                ThemeRouteCutoverProjection::LoopStroke,
                ThemeRouteCutoverProjectionSet::REPLACE_LOOP_STROKE,
                "loop.stroke",
            ),
            (
                ThemeRouteCutoverProjection::LoopLabelFill,
                ThemeRouteCutoverProjectionSet::REPLACE_LOOP_LABEL_FILL,
                "loop-label.fill",
            ),
            (
                ThemeRouteCutoverProjection::NoteLabelFill,
                ThemeRouteCutoverProjectionSet::REPLACE_NOTE_LABEL_FILL,
                "note-label.fill",
            ),
        ] {
            assert_eq!(projection.contribution_id(), contribution_id);
            assert_eq!(
                projection.action(),
                ThemeRouteCutoverProjectionAction::Replace
            );
            assert_eq!(set.iter().collect::<Vec<_>>(), vec![projection]);
        }
    }
}
