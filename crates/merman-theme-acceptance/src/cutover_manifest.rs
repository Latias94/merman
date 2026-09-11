use std::collections::{BTreeMap, BTreeSet};

use merman::DiagramFamilyId;
use merman::svg::ThemeTarget;
use merman_render::__private::{
    ThemeRouteCutoverDescriptor, ThemeRouteCutoverFacet, ThemeRouteCutoverId,
    ThemeRouteCutoverProjection, ThemeRouteCutoverProjectionAction, ThemeRouteCutoverProjectionSet,
    ThemeRouteCutoverSelector, ThemeRouteCutoverValue,
};
use merman_render::diagram_theme::ThemeVariant;

use crate::runner::{C6ProofError, C6ProofResult};

const CUTOVER_AUTHORIZATION_MANIFEST_VERSION: u16 = 68;

// Acceptance-owned authority. Update this digest together with the manifest version only after
// reviewing the complete route inventory and its projection obligations.
pub(super) const EXPECTED_AUTHORIZED_MANIFEST_DIGEST: [u8; 32] = [
    253, 169, 118, 27, 43, 180, 213, 86, 176, 123, 44, 192, 183, 77, 146, 235, 49, 96, 175, 143,
    237, 167, 204, 228, 133, 14, 66, 233, 132, 115, 90, 79,
];

const PROJECTION_ACTIONS: [(
    ThemeRouteCutoverProjection,
    ThemeRouteCutoverProjectionAction,
); 50] = [
    (
        ThemeRouteCutoverProjection::RequirementTextFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::TimelineEventStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::TimelineTextFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::EdgeLabelFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::C4TitleFillFallback,
        ThemeRouteCutoverProjectionAction::RetireFallback,
    ),
    (
        ThemeRouteCutoverProjection::NodeFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::NodeStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::EdgeStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::MarkerPaintFromEdge,
        ThemeRouteCutoverProjectionAction::RetireFallback,
    ),
    (
        ThemeRouteCutoverProjection::ActorFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::ActorStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::ActorLabelFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::NoteFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::NoteStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::ActivationFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::ActivationStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::MessageStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::MessageLabelFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::LoopFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::LoopStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::LoopLabelFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::NoteLabelFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::ClusterFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::ClusterStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::LifelineStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::TitleFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::RequirementFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::RequirementStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::PieSliceStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::PieSliceFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::TextFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::GanttTaskDefaultFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::GanttTaskActiveFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::GanttTaskSuccessFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::GanttTaskErrorFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::NodeLabelFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::GanttTaskDefaultStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::GanttTaskActiveStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::GanttTaskSuccessStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::GanttTaskErrorStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::GanttTaskWarningStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::JourneyTaskFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::KanbanTaskStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::JourneyTaskStroke,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::TreeViewMarkerPaint,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::GitGraphCommitLabelBackgroundFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::TimelineEventFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::ErTableOddFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::ErTableEvenFill,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
    (
        ThemeRouteCutoverProjection::RequirementRelationPaint,
        ThemeRouteCutoverProjectionAction::Replace,
    ),
];

#[derive(Clone, Copy)]
struct RouteAuthorization {
    id: ThemeRouteCutoverId,
    projections: &'static [ThemeRouteCutoverProjection],
}

impl RouteAuthorization {
    const fn new(
        family: DiagramFamilyId,
        target: ThemeTarget,
        facet: ThemeRouteCutoverFacet,
        value: ThemeRouteCutoverValue,
        projections: &'static [ThemeRouteCutoverProjection],
    ) -> Self {
        Self {
            id: ThemeRouteCutoverId::new(
                family,
                target,
                ThemeRouteCutoverSelector::StaticUnqualified,
                facet,
                value,
            ),
            projections,
        }
    }

    const fn new_variant(
        family: DiagramFamilyId,
        target: ThemeTarget,
        variant: ThemeVariant,
        facet: ThemeRouteCutoverFacet,
        value: ThemeRouteCutoverValue,
        projections: &'static [ThemeRouteCutoverProjection],
    ) -> Self {
        Self {
            id: ThemeRouteCutoverId::new(
                family,
                target,
                ThemeRouteCutoverSelector::StaticVariant(variant),
                facet,
                value,
            ),
            projections,
        }
    }
}

const NODE_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::NodeFill];
const NODE_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::NodeStroke];
const NODE_LABEL_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::NodeLabelFill];
const EDGE_LABEL_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::EdgeLabelFill];
const EDGE_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] = &[
    ThemeRouteCutoverProjection::EdgeStroke,
    ThemeRouteCutoverProjection::MarkerPaintFromEdge,
];
const EDGE_STROKE_ONLY_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::EdgeStroke];
const GITGRAPH_COMMIT_LABEL_BACKGROUND_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::GitGraphCommitLabelBackgroundFill];
const ACTOR_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::ActorFill];
const ACTOR_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::ActorStroke];
const ACTOR_LABEL_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::ActorLabelFill];
const LIFELINE_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::LifelineStroke];
const NOTE_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::NoteFill];
const NOTE_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::NoteStroke];
const ACTIVATION_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::ActivationFill];
const ACTIVATION_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::ActivationStroke];
const MESSAGE_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::MessageStroke];
const MESSAGE_LABEL_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::MessageLabelFill];
const LOOP_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::LoopFill];
const LOOP_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::LoopStroke];
const LOOP_LABEL_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::LoopLabelFill];
const NOTE_LABEL_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::NoteLabelFill];
const CLUSTER_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::ClusterFill];
const CLUSTER_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::ClusterStroke];
const TITLE_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::TitleFill];
const REQUIREMENT_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::RequirementFill];
const REQUIREMENT_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::RequirementStroke];
const REQUIREMENT_RELATION_PAINT_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::RequirementRelationPaint];
const PIE_SLICE_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::PieSliceStroke];
const PIE_SLICE_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::PieSliceFill];
const TEXT_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::TextFill];
const GITGRAPH_TEXT_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] = &[
    ThemeRouteCutoverProjection::TextFill,
    ThemeRouteCutoverProjection::NodeLabelFill,
    ThemeRouteCutoverProjection::EdgeLabelFill,
];
const C4_TEXT_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] = &[
    ThemeRouteCutoverProjection::TextFill,
    ThemeRouteCutoverProjection::C4TitleFillFallback,
];
const GANTT_TEXT_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::TextFill];
const GANTT_TASK_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] = &[
    ThemeRouteCutoverProjection::GanttTaskDefaultFill,
    ThemeRouteCutoverProjection::GanttTaskActiveFill,
    ThemeRouteCutoverProjection::GanttTaskSuccessFill,
    ThemeRouteCutoverProjection::GanttTaskErrorFill,
];
const GANTT_TASK_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] = &[
    ThemeRouteCutoverProjection::GanttTaskDefaultStroke,
    ThemeRouteCutoverProjection::GanttTaskActiveStroke,
    ThemeRouteCutoverProjection::GanttTaskSuccessStroke,
    ThemeRouteCutoverProjection::GanttTaskErrorStroke,
];
const GANTT_TASK_DEFAULT_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::GanttTaskDefaultFill];
const GANTT_TASK_ACTIVE_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::GanttTaskActiveFill];
const GANTT_TASK_SUCCESS_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::GanttTaskSuccessFill];
const GANTT_TASK_ERROR_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::GanttTaskErrorFill];
const GANTT_TASK_DEFAULT_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::GanttTaskDefaultStroke];
const GANTT_TASK_ACTIVE_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::GanttTaskActiveStroke];
const GANTT_TASK_SUCCESS_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::GanttTaskSuccessStroke];
const GANTT_TASK_ERROR_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::GanttTaskErrorStroke];
const GANTT_TASK_WARNING_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::GanttTaskWarningStroke];
const JOURNEY_TASK_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::JourneyTaskFill];
const JOURNEY_TEXT_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::TextFill];
const JOURNEY_TASK_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::JourneyTaskStroke];
const KANBAN_TASK_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::KanbanTaskStroke];
const TREE_VIEW_MARKER_PAINT_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::TreeViewMarkerPaint];
const TIMELINE_EVENT_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::TimelineEventFill];
const TIMELINE_TEXT_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::TimelineTextFill];
const TIMELINE_EVENT_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::TimelineEventStroke];
const ER_TABLE_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] = &[
    ThemeRouteCutoverProjection::ErTableOddFill,
    ThemeRouteCutoverProjection::ErTableEvenFill,
];
const ER_TABLE_ODD_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::ErTableOddFill];
const ER_TABLE_EVEN_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::ErTableEvenFill];

#[derive(Clone, Copy)]
struct RouteTombstone {
    id: ThemeRouteCutoverId,
    retired_in_version: u16,
    reason: &'static str,
}

struct CutoverAuthorizationManifest<'a> {
    version: u16,
    active: &'a [RouteAuthorization],
    tombstones: &'a [RouteTombstone],
}

const REQUIREMENT_TEXT_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::RequirementTextFill];

const ACTIVE_ROUTES: [RouteAuthorization; 370] = [
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Title,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Title,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Title,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Title,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        REQUIREMENT_TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        REQUIREMENT_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        REQUIREMENT_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        REQUIREMENT_TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TIMELINE,
        ThemeTarget::TimelineEvent,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        TIMELINE_EVENT_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TIMELINE,
        ThemeTarget::TimelineEvent,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        TIMELINE_EVENT_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TIMELINE,
        ThemeTarget::TimelineEvent,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        TIMELINE_EVENT_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TIMELINE,
        ThemeTarget::TimelineEvent,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        TIMELINE_EVENT_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TIMELINE,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TIMELINE_TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TIMELINE,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TIMELINE_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TIMELINE,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TIMELINE_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TIMELINE,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TIMELINE_TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Relation,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        REQUIREMENT_RELATION_PAINT_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Relation,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        REQUIREMENT_RELATION_PAINT_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Relation,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        REQUIREMENT_RELATION_PAINT_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Relation,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        REQUIREMENT_RELATION_PAINT_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Relation,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        REQUIREMENT_RELATION_PAINT_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Relation,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        REQUIREMENT_RELATION_PAINT_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Relation,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        REQUIREMENT_RELATION_PAINT_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Relation,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        REQUIREMENT_RELATION_PAINT_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::KANBAN,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::KANBAN,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::KANBAN,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::KANBAN,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        JOURNEY_TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        JOURNEY_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        JOURNEY_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        JOURNEY_TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::C4,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        C4_TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::C4,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        C4_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::C4,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        C4_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::C4,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        C4_TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NODE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NODE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NODE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NODE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::NodeLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::NodeLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::NodeLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::NodeLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Edge,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Edge,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Cluster,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        CLUSTER_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Cluster,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        CLUSTER_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Cluster,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        CLUSTER_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Cluster,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        CLUSTER_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Cluster,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        CLUSTER_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Cluster,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        CLUSTER_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Cluster,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        CLUSTER_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::FLOWCHART,
        ThemeTarget::Cluster,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        CLUSTER_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NODE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NODE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NODE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NODE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::NodeLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::NodeLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::NodeLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::NodeLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Edge,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Edge,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Cluster,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        CLUSTER_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Cluster,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        CLUSTER_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Cluster,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        CLUSTER_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Cluster,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        CLUSTER_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Cluster,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        CLUSTER_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Cluster,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        CLUSTER_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Cluster,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        CLUSTER_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SWIMLANE,
        ThemeTarget::Cluster,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        CLUSTER_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Actor,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        ACTOR_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Actor,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        ACTOR_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Actor,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        ACTOR_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Actor,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        ACTOR_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Actor,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        ACTOR_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Actor,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        ACTOR_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Actor,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        ACTOR_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Actor,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        ACTOR_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::ActorLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        ACTOR_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::ActorLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        ACTOR_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::ActorLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        ACTOR_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::ActorLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        ACTOR_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Lifeline,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        LIFELINE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Lifeline,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        LIFELINE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Lifeline,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        LIFELINE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Lifeline,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        LIFELINE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Lifeline,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        LIFELINE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Lifeline,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        LIFELINE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Lifeline,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        LIFELINE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Lifeline,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        LIFELINE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Note,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NOTE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Note,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NOTE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Note,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NOTE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Note,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NOTE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Note,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NOTE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Note,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NOTE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Note,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NOTE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Note,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NOTE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::NoteLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NOTE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::NoteLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NOTE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::NoteLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NOTE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::NoteLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NOTE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Activation,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        ACTIVATION_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Activation,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        ACTIVATION_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Activation,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        ACTIVATION_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Activation,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        ACTIVATION_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Activation,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        ACTIVATION_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Activation,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        ACTIVATION_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Activation,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        ACTIVATION_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Activation,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        ACTIVATION_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Message,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        MESSAGE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Message,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        MESSAGE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Message,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        MESSAGE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Message,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        MESSAGE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Message,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        MESSAGE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Message,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        MESSAGE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Message,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        MESSAGE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Message,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        MESSAGE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::MessageLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        MESSAGE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::MessageLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        MESSAGE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::MessageLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        MESSAGE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::MessageLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        MESSAGE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Loop,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        LOOP_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Loop,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        LOOP_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Loop,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        LOOP_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Loop,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        LOOP_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Loop,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        LOOP_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Loop,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        LOOP_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Loop,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        LOOP_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::Loop,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        LOOP_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::LoopLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        LOOP_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::LoopLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        LOOP_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::LoopLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        LOOP_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SEQUENCE,
        ThemeTarget::LoopLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        LOOP_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREEMAP,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREEMAP,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREEMAP,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREEMAP,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREEMAP,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREEMAP,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::RAILROAD,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::RAILROAD,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::RAILROAD,
        ThemeTarget::Title,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::RAILROAD,
        ThemeTarget::Title,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::RAILROAD,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::RAILROAD,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::RAILROAD,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SANKEY,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::SANKEY,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SANKEY,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::SANKEY,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::RAILROAD,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Requirement,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        REQUIREMENT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Requirement,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        REQUIREMENT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Requirement,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        REQUIREMENT_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Requirement,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        REQUIREMENT_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Requirement,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        REQUIREMENT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Requirement,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        REQUIREMENT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Requirement,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        REQUIREMENT_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::REQUIREMENT,
        ThemeTarget::Requirement,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        REQUIREMENT_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::PIE,
        ThemeTarget::PieSlice,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        PIE_SLICE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::PIE,
        ThemeTarget::PieSlice,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        PIE_SLICE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::PIE,
        ThemeTarget::PieSlice,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        PIE_SLICE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::PIE,
        ThemeTarget::PieSlice,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        PIE_SLICE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::PIE,
        ThemeTarget::PieSlice,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        PIE_SLICE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::PIE,
        ThemeTarget::PieSlice,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        PIE_SLICE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::PIE,
        ThemeTarget::PieSlice,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        PIE_SLICE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::PIE,
        ThemeTarget::PieSlice,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        PIE_SLICE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::PIE,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::PIE,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::PIE,
        ThemeTarget::Title,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::PIE,
        ThemeTarget::Title,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::PIE,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::PIE,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::PIE,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::PIE,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::BLOCK,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::BLOCK,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::BLOCK,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NODE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::BLOCK,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NODE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::BLOCK,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::BLOCK,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::BLOCK,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NODE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::BLOCK,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NODE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ZENUML,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ZENUML,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::VENN,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::VENN,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::VENN,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::VENN,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::VENN,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::VENN,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ISHIKAWA,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ISHIKAWA,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::EVENT_MODELING,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::EVENT_MODELING,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::CYNEFIN,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::CYNEFIN,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::CYNEFIN,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::CYNEFIN,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::CLASS,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::CLASS,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::CLASS,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::CLASS,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::CLASS,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NODE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::CLASS,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NODE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::CLASS,
        ThemeTarget::NodeLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::CLASS,
        ThemeTarget::NodeLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::CLASS,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::CLASS,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::CLASS,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NODE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::CLASS,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NODE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::CLASS,
        ThemeTarget::NodeLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::CLASS,
        ThemeTarget::NodeLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ER,
        ThemeTarget::Relation,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ER,
        ThemeTarget::Relation,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::ER,
        ThemeTarget::Relation,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::ER,
        ThemeTarget::Relation,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ER,
        ThemeTarget::Table,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        ER_TABLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ER,
        ThemeTarget::Table,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        ER_TABLE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::ER,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::ER,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ER,
        ThemeTarget::Relation,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ER,
        ThemeTarget::Relation,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::ER,
        ThemeTarget::Relation,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::ER,
        ThemeTarget::Relation,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::ER,
        ThemeTarget::Table,
        ThemeVariant::Odd,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        ER_TABLE_ODD_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::ER,
        ThemeTarget::Table,
        ThemeVariant::Odd,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        ER_TABLE_ODD_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::ER,
        ThemeTarget::Table,
        ThemeVariant::Even,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        ER_TABLE_EVEN_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::ER,
        ThemeTarget::Table,
        ThemeVariant::Even,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        ER_TABLE_EVEN_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ER,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::INFO,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::INFO,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::INFO,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::INFO,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ER,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ARCHITECTURE,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::ARCHITECTURE,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::ARCHITECTURE,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::ARCHITECTURE,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NODE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NODE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NODE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NODE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NODE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Node,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NODE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        NODE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Node,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        NODE_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::NodeLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::NodeLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::NodeLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::NodeLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::EdgeLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        EDGE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::EdgeLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        EDGE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::EdgeLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        EDGE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::EdgeLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        EDGE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        GITGRAPH_TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        GITGRAPH_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        GITGRAPH_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        GITGRAPH_TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Edge,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Edge,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Edge,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Edge,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::EdgeLabelBackground,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        GITGRAPH_COMMIT_LABEL_BACKGROUND_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::EdgeLabelBackground,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        GITGRAPH_COMMIT_LABEL_BACKGROUND_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::EdgeLabelBackground,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        GITGRAPH_COMMIT_LABEL_BACKGROUND_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::EdgeLabelBackground,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        GITGRAPH_COMMIT_LABEL_BACKGROUND_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TASK_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        GANTT_TASK_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TASK_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        GANTT_TASK_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GANTT,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GANTT,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Title,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GANTT,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TEXT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::GANTT,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        GANTT_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        GANTT_TEXT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Title,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TASK_DEFAULT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        GANTT_TASK_DEFAULT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Active,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TASK_ACTIVE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Active,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        GANTT_TASK_ACTIVE_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Success,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TASK_SUCCESS_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Success,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        GANTT_TASK_SUCCESS_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Error,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TASK_ERROR_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Error,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        GANTT_TASK_ERROR_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TASK_DEFAULT_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        GANTT_TASK_DEFAULT_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Active,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TASK_ACTIVE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Active,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        GANTT_TASK_ACTIVE_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Success,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TASK_SUCCESS_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Success,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        GANTT_TASK_SUCCESS_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Error,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TASK_ERROR_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Error,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        GANTT_TASK_ERROR_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Warning,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        GANTT_TASK_WARNING_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::GANTT,
        ThemeTarget::Task,
        ThemeVariant::Warning,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        GANTT_TASK_WARNING_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TIMELINE,
        ThemeTarget::TimelineEvent,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TIMELINE_EVENT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TIMELINE,
        ThemeTarget::TimelineEvent,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TIMELINE_EVENT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TIMELINE,
        ThemeTarget::TimelineEvent,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TIMELINE_EVENT_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TIMELINE,
        ThemeTarget::TimelineEvent,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TIMELINE_EVENT_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::JourneyTask,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        JOURNEY_TASK_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::JourneyTask,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        JOURNEY_TASK_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::JourneyTask,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        JOURNEY_TASK_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::JourneyTask,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        JOURNEY_TASK_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::JourneyTask,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        JOURNEY_TASK_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::JourneyTask,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        JOURNEY_TASK_STROKE_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::JourneyTask,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        JOURNEY_TASK_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::JourneyTask,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        JOURNEY_TASK_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Edge,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Edge,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Edge,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Edge,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Edge,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        EDGE_STROKE_ONLY_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Marker,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TREE_VIEW_MARKER_PAINT_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Marker,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TREE_VIEW_MARKER_PAINT_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Marker,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        TREE_VIEW_MARKER_PAINT_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Marker,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        TREE_VIEW_MARKER_PAINT_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Marker,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TREE_VIEW_MARKER_PAINT_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Marker,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TREE_VIEW_MARKER_PAINT_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Marker,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        TREE_VIEW_MARKER_PAINT_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Marker,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        TREE_VIEW_MARKER_PAINT_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::NodeLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::NodeLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::NodeLabel,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::NodeLabel,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route_variant(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Text,
        ThemeVariant::Default,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Text,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        NODE_LABEL_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::KANBAN,
        ThemeTarget::Task,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        KANBAN_TASK_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::KANBAN,
        ThemeTarget::Task,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        KANBAN_TASK_STROKE_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::RADAR,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::RADAR,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
    ),
];

const TOMBSTONES: [RouteTombstone; 0] = [];

const MANIFEST: CutoverAuthorizationManifest<'static> = CutoverAuthorizationManifest {
    version: CUTOVER_AUTHORIZATION_MANIFEST_VERSION,
    active: &ACTIVE_ROUTES,
    tombstones: &TOMBSTONES,
};

pub(super) struct AuthorizedCutoverRoutes {
    manifest_version: u16,
    routes: Vec<ThemeRouteCutoverDescriptor>,
}

impl AuthorizedCutoverRoutes {
    pub(super) const fn manifest_version(&self) -> u16 {
        self.manifest_version
    }

    pub(super) fn routes(&self) -> &[ThemeRouteCutoverDescriptor] {
        &self.routes
    }

    pub(super) fn into_routes(self) -> Vec<ThemeRouteCutoverDescriptor> {
        self.routes
    }
}

const fn route(
    family: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeRouteCutoverFacet,
    value: ThemeRouteCutoverValue,
    projections: &'static [ThemeRouteCutoverProjection],
) -> RouteAuthorization {
    RouteAuthorization::new(family, target, facet, value, projections)
}

const fn route_variant(
    family: DiagramFamilyId,
    target: ThemeTarget,
    variant: ThemeVariant,
    facet: ThemeRouteCutoverFacet,
    value: ThemeRouteCutoverValue,
    projections: &'static [ThemeRouteCutoverProjection],
) -> RouteAuthorization {
    RouteAuthorization::new_variant(family, target, variant, facet, value, projections)
}

pub(super) fn authorize_cutover_routes(
    inventory: Vec<ThemeRouteCutoverDescriptor>,
) -> C6ProofResult<AuthorizedCutoverRoutes> {
    let entries = inventory
        .iter()
        .map(|route| (route.id(), route.projections()))
        .collect::<Vec<_>>();
    let authorized_ids = reconcile_manifest(&entries, &MANIFEST)?;
    let mut routes = inventory
        .into_iter()
        .map(|route| (route.id(), route))
        .collect::<BTreeMap<_, _>>();
    let routes = authorized_ids
        .into_iter()
        .map(|id| {
            routes.remove(&id).ok_or_else(|| {
                C6ProofError::new(
                    "route-manifest",
                    format!(
                        "authorized route {} disappeared during reconciliation",
                        id_label(id)
                    ),
                )
            })
        })
        .collect::<C6ProofResult<Vec<_>>>()?;
    Ok(AuthorizedCutoverRoutes {
        manifest_version: MANIFEST.version,
        routes,
    })
}

fn reconcile_manifest(
    inventory: &[(ThemeRouteCutoverId, ThemeRouteCutoverProjectionSet)],
    manifest: &CutoverAuthorizationManifest<'_>,
) -> C6ProofResult<Vec<ThemeRouteCutoverId>> {
    c6_ensure!(
        "route-manifest",
        manifest.version == CUTOVER_AUTHORIZATION_MANIFEST_VERSION,
        "unsupported cutover authorization manifest version {}",
        manifest.version
    );
    c6_ensure!(
        "route-manifest",
        manifest.tombstones.is_empty(),
        "cutover tombstones require an explicit bridge-retirement verifier before admission"
    );
    validate_projection_actions(manifest, &PROJECTION_ACTIONS)?;

    let inventory = collect_unique(
        inventory
            .iter()
            .map(|(id, projections)| (*id, projections.iter().collect::<BTreeSet<_>>())),
        "matrix inventory",
    )?;
    let active = collect_unique(
        manifest.active.iter().map(|route| {
            (
                route.id,
                route.projections.iter().copied().collect::<BTreeSet<_>>(),
            )
        }),
        "authorization manifest",
    )?;
    c6_ensure!(
        "route-manifest",
        manifest.active.iter().all(|route| {
            route
                .projections
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                == route.projections.len()
        }),
        "cutover authorization manifest contains duplicate projections"
    );
    let tombstones = manifest
        .tombstones
        .iter()
        .map(|route| route.id)
        .collect::<BTreeSet<_>>();
    c6_ensure!(
        "route-manifest",
        tombstones.len() == manifest.tombstones.len(),
        "cutover authorization manifest contains duplicate tombstones"
    );
    c6_ensure!(
        "route-manifest",
        active.keys().all(|id| !tombstones.contains(id)),
        "one cutover route is both active and tombstoned"
    );
    for tombstone in manifest.tombstones {
        c6_ensure!(
            "route-manifest",
            tombstone.retired_in_version > 0
                && tombstone.retired_in_version <= manifest.version
                && !tombstone.reason.trim().is_empty(),
            "invalid cutover tombstone for {}",
            id_label(tombstone.id)
        );
    }

    let new_routes = inventory
        .keys()
        .filter(|id| !active.contains_key(id))
        .copied()
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-manifest",
        new_routes.is_empty(),
        "matrix added unauthorized cutover routes: {}",
        labels(&new_routes)
    );
    let removed_routes = active
        .keys()
        .filter(|id| !inventory.contains_key(id))
        .copied()
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-manifest",
        removed_routes.is_empty(),
        "matrix silently removed authorized cutover routes: {}",
        labels(&removed_routes)
    );
    let projection_drift = active
        .iter()
        .filter_map(|(id, expected)| {
            inventory
                .get(id)
                .filter(|actual| *actual != expected)
                .map(|_| *id)
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-manifest",
        projection_drift.is_empty(),
        "cutover projection obligations drifted for: {}",
        labels(&projection_drift)
    );

    Ok(active.keys().copied().collect())
}

fn validate_projection_actions(
    manifest: &CutoverAuthorizationManifest<'_>,
    expected: &[(
        ThemeRouteCutoverProjection,
        ThemeRouteCutoverProjectionAction,
    )],
) -> C6ProofResult<()> {
    let expected = expected.iter().copied().collect::<BTreeMap<_, _>>();
    c6_ensure!(
        "route-manifest",
        expected.len() == PROJECTION_ACTIONS.len(),
        "cutover authorization manifest contains duplicate projection actions"
    );

    let active_projections = manifest
        .active
        .iter()
        .flat_map(|route| route.projections.iter().copied())
        .collect::<BTreeSet<_>>();
    for projection in active_projections.iter().copied() {
        let action = expected.get(&projection).copied().ok_or_else(|| {
            C6ProofError::new(
                "route-manifest",
                format!(
                    "cutover projection {} has no manifest-owned action",
                    projection.contribution_id()
                ),
            )
        })?;
        c6_ensure!(
            "route-manifest",
            projection.action() == action,
            "cutover projection {} action drifted from {} to {}",
            projection.contribution_id(),
            action.id(),
            projection.action().id()
        );
    }

    let unreferenced_actions = expected
        .keys()
        .copied()
        .filter(|projection| !active_projections.contains(projection))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-manifest",
        unreferenced_actions.is_empty(),
        "projection actions are not referenced by the active cutover inventory: {}",
        unreferenced_actions
            .iter()
            .map(|projection| projection.contribution_id())
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(())
}

fn collect_unique(
    entries: impl IntoIterator<Item = (ThemeRouteCutoverId, BTreeSet<ThemeRouteCutoverProjection>)>,
    owner: &str,
) -> C6ProofResult<BTreeMap<ThemeRouteCutoverId, BTreeSet<ThemeRouteCutoverProjection>>> {
    let mut unique = BTreeMap::new();
    for (id, projections) in entries {
        c6_ensure!(
            "route-manifest",
            unique.insert(id, projections).is_none(),
            "{owner} contains duplicate route {}",
            id_label(id)
        );
    }
    Ok(unique)
}

fn labels(ids: &[ThemeRouteCutoverId]) -> String {
    ids.iter()
        .copied()
        .map(id_label)
        .collect::<Vec<_>>()
        .join(", ")
}

fn id_label(id: ThemeRouteCutoverId) -> String {
    format!(
        "{}/{}/{:?}/{:?}/{:?}",
        id.family_id().as_str(),
        id.target().id(),
        id.selector(),
        id.facet(),
        id.value()
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use merman::DiagramFamilyId;
    use merman::svg::ThemeTarget;
    use merman_render::__private::{
        ThemeRouteCutoverFacet, ThemeRouteCutoverId, ThemeRouteCutoverProjection,
        ThemeRouteCutoverProjectionAction, ThemeRouteCutoverProjectionSet,
        ThemeRouteCutoverSelector, ThemeRouteCutoverValue, legacy_replacing_typed_theme_routes,
    };
    use merman_render::diagram_theme::ThemeVariant;

    use super::{
        ACTIVE_ROUTES, ACTOR_FILL_PROJECTIONS, CLUSTER_FILL_PROJECTIONS,
        CLUSTER_STROKE_PROJECTIONS, CUTOVER_AUTHORIZATION_MANIFEST_VERSION,
        CutoverAuthorizationManifest, EDGE_STROKE_ONLY_PROJECTIONS, EDGE_STROKE_PROJECTIONS,
        GANTT_TASK_ACTIVE_FILL_PROJECTIONS, GANTT_TASK_WARNING_STROKE_PROJECTIONS, MANIFEST,
        NODE_FILL_PROJECTIONS, NODE_LABEL_FILL_PROJECTIONS, NODE_STROKE_PROJECTIONS,
        PIE_SLICE_FILL_PROJECTIONS, PIE_SLICE_STROKE_PROJECTIONS, PROJECTION_ACTIONS,
        RouteAuthorization, RouteTombstone, TEXT_FILL_PROJECTIONS, TIMELINE_EVENT_FILL_PROJECTIONS,
        TIMELINE_EVENT_STROKE_PROJECTIONS, TITLE_FILL_PROJECTIONS,
        TREE_VIEW_MARKER_PAINT_PROJECTIONS, authorize_cutover_routes, reconcile_manifest,
        validate_projection_actions,
    };

    fn current_inventory() -> Vec<(ThemeRouteCutoverId, ThemeRouteCutoverProjectionSet)> {
        legacy_replacing_typed_theme_routes()
            .expect("derive current route inventory")
            .into_iter()
            .map(|route| (route.id(), route.projections()))
            .collect()
    }

    #[test]
    fn manifest_authorizes_the_exact_current_matrix_inventory() {
        let routes = authorize_cutover_routes(
            legacy_replacing_typed_theme_routes().expect("derive current route inventory"),
        )
        .expect("authorize current route inventory");

        assert_eq!(routes.routes().len(), ACTIVE_ROUTES.len());
    }

    #[test]
    fn timeline_event_stroke_authority_replaces_only_static_scalar_routes() {
        let routes = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::TIMELINE
                    && route.id.target() == ThemeTarget::TimelineEvent
                    && route.id.facet() == ThemeRouteCutoverFacet::Stroke
            })
            .collect::<Vec<_>>();
        assert_eq!(routes.len(), 4);
        for route in routes {
            assert!(matches!(
                route.id.selector(),
                ThemeRouteCutoverSelector::StaticUnqualified
                    | ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
            ));
            assert!(matches!(
                route.id.value(),
                ThemeRouteCutoverValue::Transparent | ThemeRouteCutoverValue::Solid
            ));
            assert_eq!(route.projections, TIMELINE_EVENT_STROKE_PROJECTIONS);
        }
    }

    #[test]
    fn requirement_text_authority_replaces_only_the_four_static_fill_routes() {
        let routes = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::REQUIREMENT
                    && route.id.target() == ThemeTarget::Text
            })
            .collect::<Vec<_>>();
        assert_eq!(routes.len(), 4);
        for selector in [
            ThemeRouteCutoverSelector::StaticUnqualified,
            ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
        ] {
            for value in [
                ThemeRouteCutoverValue::Solid,
                ThemeRouteCutoverValue::Transparent,
            ] {
                assert!(routes.iter().any(|route| route.id.selector() == selector
                    && route.id.value() == value
                    && route.id.facet() == ThemeRouteCutoverFacet::Fill
                    && route.projections == super::REQUIREMENT_TEXT_FILL_PROJECTIONS));
            }
        }
    }

    #[test]
    fn requirement_relation_authority_replaces_the_exact_static_paint_domain() {
        let actual = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::REQUIREMENT
                    && route.id.target() == ThemeTarget::Relation
            })
            .map(|route| {
                assert_eq!(
                    route.projections,
                    [ThemeRouteCutoverProjection::RequirementRelationPaint]
                );
                (route.id.selector(), route.id.facet(), route.id.value())
            })
            .collect::<std::collections::BTreeSet<_>>();
        let mut expected = std::collections::BTreeSet::new();
        for selector in [
            ThemeRouteCutoverSelector::StaticUnqualified,
            ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
        ] {
            for facet in [ThemeRouteCutoverFacet::Fill, ThemeRouteCutoverFacet::Stroke] {
                for value in [
                    ThemeRouteCutoverValue::Transparent,
                    ThemeRouteCutoverValue::Solid,
                ] {
                    expected.insert((selector, facet, value));
                }
            }
        }
        assert_eq!(actual, expected);
    }

    #[test]
    fn er_route_authority_keeps_row_and_relation_projection_identity() {
        let routes = ACTIVE_ROUTES
            .iter()
            .filter(|route| route.id.family_id() == DiagramFamilyId::ER)
            .map(|route| {
                (
                    route.id.target(),
                    route.id.selector(),
                    route.id.facet(),
                    route.id.value(),
                    route.projections.to_vec(),
                )
            })
            .collect::<BTreeSet<_>>();
        let mut expected = BTreeSet::new();
        for value in [
            ThemeRouteCutoverValue::Transparent,
            ThemeRouteCutoverValue::Solid,
        ] {
            for selector in [
                ThemeRouteCutoverSelector::StaticUnqualified,
                ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
            ] {
                expected.insert((
                    ThemeTarget::Text,
                    selector,
                    ThemeRouteCutoverFacet::Fill,
                    value,
                    vec![ThemeRouteCutoverProjection::TextFill],
                ));
                for facet in [ThemeRouteCutoverFacet::Fill, ThemeRouteCutoverFacet::Stroke] {
                    expected.insert((
                        ThemeTarget::Relation,
                        selector,
                        facet,
                        value,
                        vec![ThemeRouteCutoverProjection::EdgeStroke],
                    ));
                }
            }
            expected.insert((
                ThemeTarget::Table,
                ThemeRouteCutoverSelector::StaticUnqualified,
                ThemeRouteCutoverFacet::Fill,
                value,
                vec![
                    ThemeRouteCutoverProjection::ErTableOddFill,
                    ThemeRouteCutoverProjection::ErTableEvenFill,
                ],
            ));
            for (variant, projection) in [
                (
                    ThemeVariant::Odd,
                    ThemeRouteCutoverProjection::ErTableOddFill,
                ),
                (
                    ThemeVariant::Even,
                    ThemeRouteCutoverProjection::ErTableEvenFill,
                ),
            ] {
                expected.insert((
                    ThemeTarget::Table,
                    ThemeRouteCutoverSelector::StaticVariant(variant),
                    ThemeRouteCutoverFacet::Fill,
                    value,
                    vec![projection],
                ));
            }
        }
        assert_eq!(routes, expected);
    }

    #[test]
    fn manifest_keeps_variant_authority_bounded_and_projection_local() {
        let sequence_default = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::SEQUENCE
                    && route.id.selector()
                        == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
            })
            .collect::<Vec<_>>();
        assert_eq!(sequence_default.len(), 32);
        for (target, facet, projection) in [
            (
                ThemeTarget::Actor,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverProjection::ActorFill,
            ),
            (
                ThemeTarget::Actor,
                ThemeRouteCutoverFacet::Stroke,
                ThemeRouteCutoverProjection::ActorStroke,
            ),
            (
                ThemeTarget::ActorLabel,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverProjection::ActorLabelFill,
            ),
            (
                ThemeTarget::Lifeline,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverProjection::LifelineStroke,
            ),
            (
                ThemeTarget::Lifeline,
                ThemeRouteCutoverFacet::Stroke,
                ThemeRouteCutoverProjection::LifelineStroke,
            ),
            (
                ThemeTarget::Note,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverProjection::NoteFill,
            ),
            (
                ThemeTarget::Note,
                ThemeRouteCutoverFacet::Stroke,
                ThemeRouteCutoverProjection::NoteStroke,
            ),
            (
                ThemeTarget::NoteLabel,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverProjection::NoteLabelFill,
            ),
            (
                ThemeTarget::Activation,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverProjection::ActivationFill,
            ),
            (
                ThemeTarget::Activation,
                ThemeRouteCutoverFacet::Stroke,
                ThemeRouteCutoverProjection::ActivationStroke,
            ),
            (
                ThemeTarget::Message,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverProjection::MessageStroke,
            ),
            (
                ThemeTarget::Message,
                ThemeRouteCutoverFacet::Stroke,
                ThemeRouteCutoverProjection::MessageStroke,
            ),
            (
                ThemeTarget::MessageLabel,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverProjection::MessageLabelFill,
            ),
            (
                ThemeTarget::Loop,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverProjection::LoopFill,
            ),
            (
                ThemeTarget::Loop,
                ThemeRouteCutoverFacet::Stroke,
                ThemeRouteCutoverProjection::LoopStroke,
            ),
            (
                ThemeTarget::LoopLabel,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverProjection::LoopLabelFill,
            ),
        ] {
            let routes = sequence_default
                .iter()
                .filter(|route| route.id.target() == target && route.id.facet() == facet)
                .collect::<Vec<_>>();
            assert_eq!(routes.len(), 2, "target={target:?}, facet={facet:?}");
            assert!(
                routes
                    .iter()
                    .all(|route| route.projections == &[projection])
            );
            assert_eq!(
                routes
                    .iter()
                    .map(|route| route.id.value())
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([
                    ThemeRouteCutoverValue::Transparent,
                    ThemeRouteCutoverValue::Solid,
                ])
            );
        }
        assert!(
            !sequence_default
                .iter()
                .any(|route| { route.id.target() == ThemeTarget::SequenceNumberLabel })
        );

        for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
            let cluster_default = ACTIVE_ROUTES
                .iter()
                .filter(|route| {
                    route.id.family_id() == family
                        && route.id.target() == ThemeTarget::Cluster
                        && route.id.selector()
                            == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
                })
                .collect::<Vec<_>>();
            assert_eq!(cluster_default.len(), 4, "family={family:?}");
            assert!(cluster_default.iter().all(|route| {
                route.projections
                    == match route.id.facet() {
                        ThemeRouteCutoverFacet::Fill => CLUSTER_FILL_PROJECTIONS,
                        ThemeRouteCutoverFacet::Stroke => CLUSTER_STROKE_PROJECTIONS,
                    }
            }));

            let node_default = ACTIVE_ROUTES
                .iter()
                .filter(|route| {
                    route.id.family_id() == family
                        && route.id.target() == ThemeTarget::Node
                        && route.id.selector()
                            == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
                })
                .collect::<Vec<_>>();
            assert_eq!(node_default.len(), 4, "family={family:?}");
            assert!(node_default.iter().all(|route| {
                let expected_projections = match route.id.facet() {
                    ThemeRouteCutoverFacet::Fill => NODE_FILL_PROJECTIONS,
                    ThemeRouteCutoverFacet::Stroke => NODE_STROKE_PROJECTIONS,
                };
                route.projections == expected_projections
                    && matches!(
                        route.id.value(),
                        ThemeRouteCutoverValue::Transparent | ThemeRouteCutoverValue::Solid
                    )
            }));

            let edge_default = ACTIVE_ROUTES
                .iter()
                .filter(|route| {
                    route.id.family_id() == family
                        && route.id.target() == ThemeTarget::Edge
                        && route.id.facet() == ThemeRouteCutoverFacet::Stroke
                        && route.id.selector()
                            == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
                })
                .collect::<Vec<_>>();
            assert_eq!(edge_default.len(), 2, "family={family:?}");
            assert!(edge_default.iter().all(|route| {
                route.projections == EDGE_STROKE_PROJECTIONS
                    && matches!(
                        route.id.value(),
                        ThemeRouteCutoverValue::Transparent | ThemeRouteCutoverValue::Solid
                    )
            }));
        }

        let gantt_routes = ACTIVE_ROUTES
            .iter()
            .filter(|route| route.id.family_id() == DiagramFamilyId::GANTT)
            .collect::<Vec<_>>();
        assert_eq!(gantt_routes.len(), 30);

        let gantt_title_routes = gantt_routes
            .iter()
            .filter(|route| route.id.target() == ThemeTarget::Title)
            .collect::<Vec<_>>();
        assert_eq!(gantt_title_routes.len(), 4);
        assert!(gantt_title_routes.iter().all(|route| {
            route.projections == &[ThemeRouteCutoverProjection::TitleFill]
                && matches!(
                    route.id.selector(),
                    ThemeRouteCutoverSelector::StaticUnqualified
                        | ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
                )
        }));

        let gantt_text_routes = gantt_routes
            .iter()
            .filter(|route| route.id.target() == ThemeTarget::Text)
            .collect::<Vec<_>>();
        assert_eq!(gantt_text_routes.len(), 4);
        assert!(gantt_text_routes.iter().all(|route| {
            route.projections == &[ThemeRouteCutoverProjection::TextFill]
                && matches!(
                    route.id.selector(),
                    ThemeRouteCutoverSelector::StaticUnqualified
                        | ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
                )
        }));

        for (variant, fill_projection, stroke_projection) in [
            (
                ThemeVariant::Default,
                ThemeRouteCutoverProjection::GanttTaskDefaultFill,
                ThemeRouteCutoverProjection::GanttTaskDefaultStroke,
            ),
            (
                ThemeVariant::Active,
                ThemeRouteCutoverProjection::GanttTaskActiveFill,
                ThemeRouteCutoverProjection::GanttTaskActiveStroke,
            ),
            (
                ThemeVariant::Success,
                ThemeRouteCutoverProjection::GanttTaskSuccessFill,
                ThemeRouteCutoverProjection::GanttTaskSuccessStroke,
            ),
            (
                ThemeVariant::Error,
                ThemeRouteCutoverProjection::GanttTaskErrorFill,
                ThemeRouteCutoverProjection::GanttTaskErrorStroke,
            ),
        ] {
            for (facet, projection) in [
                (ThemeRouteCutoverFacet::Fill, fill_projection),
                (ThemeRouteCutoverFacet::Stroke, stroke_projection),
            ] {
                for value in [
                    ThemeRouteCutoverValue::Transparent,
                    ThemeRouteCutoverValue::Solid,
                ] {
                    let route = gantt_routes
                        .iter()
                        .find(|route| {
                            route.id.target() == ThemeTarget::Task
                                && route.id.selector()
                                    == ThemeRouteCutoverSelector::StaticVariant(variant)
                                && route.id.facet() == facet
                                && route.id.value() == value
                        })
                        .expect("bounded Gantt variant route");
                    assert_eq!(route.projections, &[projection]);
                }
            }
        }

        let warning_routes = gantt_routes
            .iter()
            .filter(|route| {
                route.id.target() == ThemeTarget::Task
                    && route.id.selector()
                        == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Warning)
                    && route.id.facet() == ThemeRouteCutoverFacet::Stroke
            })
            .collect::<Vec<_>>();
        assert_eq!(warning_routes.len(), 2);
        assert!(warning_routes.iter().all(|route| {
            route.projections == GANTT_TASK_WARNING_STROKE_PROJECTIONS
                && matches!(
                    route.id.value(),
                    ThemeRouteCutoverValue::Transparent | ThemeRouteCutoverValue::Solid
                )
        }));

        let timeline_event_fill_routes = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::TIMELINE
                    && route.id.target() == ThemeTarget::TimelineEvent
                    && route.id.facet() == ThemeRouteCutoverFacet::Fill
            })
            .collect::<Vec<_>>();
        assert_eq!(timeline_event_fill_routes.len(), 4);
        assert!(timeline_event_fill_routes.iter().all(|route| {
            route.id.facet() == ThemeRouteCutoverFacet::Fill
                && matches!(
                    route.id.selector(),
                    ThemeRouteCutoverSelector::StaticUnqualified
                        | ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
                )
                && matches!(
                    route.id.value(),
                    ThemeRouteCutoverValue::Transparent | ThemeRouteCutoverValue::Solid
                )
                && route.projections == TIMELINE_EVENT_FILL_PROJECTIONS
        }));

        for family in [DiagramFamilyId::REQUIREMENT, DiagramFamilyId::BLOCK] {
            let qualified = ACTIVE_ROUTES
                .iter()
                .filter(|route| {
                    route.id.family_id() == family
                        && route.id.selector()
                            == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
                })
                .collect::<Vec<_>>();
            assert_eq!(
                qualified.len(),
                if family == DiagramFamilyId::REQUIREMENT {
                    10
                } else {
                    4
                }
            );
        }

        let er_relation_default = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::ER
                    && route.id.target() == ThemeTarget::Relation
                    && route.id.selector()
                        == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
            })
            .collect::<Vec<_>>();
        assert_eq!(er_relation_default.len(), 4);
        assert!(er_relation_default.iter().all(|route| {
            matches!(
                route.id.facet(),
                ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke
            ) && route.projections == EDGE_STROKE_ONLY_PROJECTIONS
        }));

        let pie_slice_default = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::PIE
                    && route.id.target() == ThemeTarget::PieSlice
                    && route.id.selector()
                        == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
            })
            .collect::<Vec<_>>();
        assert_eq!(pie_slice_default.len(), 4);
        assert!(pie_slice_default.iter().all(|route| {
            route.projections
                == match route.id.facet() {
                    ThemeRouteCutoverFacet::Fill => PIE_SLICE_FILL_PROJECTIONS,
                    ThemeRouteCutoverFacet::Stroke => PIE_SLICE_STROKE_PROJECTIONS,
                }
        }));

        let pie_text_routes = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::PIE
                    && route.id.target() == ThemeTarget::Text
            })
            .collect::<Vec<_>>();
        assert_eq!(pie_text_routes.len(), 4);
        assert!(pie_text_routes.iter().all(|route| {
            matches!(
                route.id.selector(),
                ThemeRouteCutoverSelector::StaticUnqualified
                    | ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
            ) && route.id.facet() == ThemeRouteCutoverFacet::Fill
                && route.projections == TEXT_FILL_PROJECTIONS
        }));

        let pie_title_and_text_default = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::PIE
                    && matches!(route.id.target(), ThemeTarget::Title | ThemeTarget::Text)
                    && route.id.selector()
                        == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
            })
            .collect::<Vec<_>>();
        assert_eq!(pie_title_and_text_default.len(), 4);
        assert!(pie_title_and_text_default.iter().all(|route| {
            route.id.facet() == ThemeRouteCutoverFacet::Fill
                && route.projections
                    == match route.id.target() {
                        ThemeTarget::Title => TITLE_FILL_PROJECTIONS,
                        ThemeTarget::Text => TEXT_FILL_PROJECTIONS,
                        _ => return false,
                    }
        }));

        let info_text_default = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::INFO
                    && route.id.target() == ThemeTarget::Text
                    && route.id.selector()
                        == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
            })
            .collect::<Vec<_>>();
        assert_eq!(info_text_default.len(), 2);
        assert!(info_text_default.iter().all(|route| {
            route.id.facet() == ThemeRouteCutoverFacet::Fill
                && route.projections == TEXT_FILL_PROJECTIONS
        }));

        let class_node_default = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::CLASS
                    && matches!(
                        route.id.target(),
                        ThemeTarget::Node | ThemeTarget::NodeLabel
                    )
                    && route.id.selector()
                        == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
            })
            .collect::<Vec<_>>();
        assert_eq!(class_node_default.len(), 6);
        assert!(class_node_default.iter().all(|route| {
            route.projections
                == match (route.id.target(), route.id.facet()) {
                    (ThemeTarget::Node, ThemeRouteCutoverFacet::Fill) => NODE_FILL_PROJECTIONS,
                    (ThemeTarget::Node, ThemeRouteCutoverFacet::Stroke) => NODE_STROKE_PROJECTIONS,
                    (ThemeTarget::NodeLabel, ThemeRouteCutoverFacet::Fill) => {
                        NODE_LABEL_FILL_PROJECTIONS
                    }
                    _ => return false,
                }
        }));

        let tree_view_default = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::TREE_VIEW
                    && route.id.selector()
                        == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
            })
            .collect::<Vec<_>>();
        assert_eq!(tree_view_default.len(), 12);
        assert!(tree_view_default.iter().all(|route| {
            route.projections
                == match (route.id.target(), route.id.facet()) {
                    (ThemeTarget::Edge, ThemeRouteCutoverFacet::Fill)
                    | (ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke) => {
                        EDGE_STROKE_ONLY_PROJECTIONS
                    }
                    (ThemeTarget::NodeLabel, ThemeRouteCutoverFacet::Fill)
                    | (ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
                        NODE_LABEL_FILL_PROJECTIONS
                    }
                    (ThemeTarget::Marker, ThemeRouteCutoverFacet::Fill)
                    | (ThemeTarget::Marker, ThemeRouteCutoverFacet::Stroke) => {
                        TREE_VIEW_MARKER_PAINT_PROJECTIONS
                    }
                    _ => return false,
                }
                && matches!(
                    route.id.value(),
                    ThemeRouteCutoverValue::Transparent | ThemeRouteCutoverValue::Solid
                )
        }));
    }

    #[test]
    fn manifest_keeps_gitgraph_edge_fill_on_the_branch_stroke_projection() {
        let routes = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::GIT_GRAPH
                    && route.id.target() == ThemeTarget::Edge
                    && route.id.facet() == ThemeRouteCutoverFacet::Fill
            })
            .collect::<Vec<_>>();
        assert_eq!(routes.len(), 4);
        for selector in [
            ThemeRouteCutoverSelector::StaticUnqualified,
            ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
        ] {
            for value in [
                ThemeRouteCutoverValue::Transparent,
                ThemeRouteCutoverValue::Solid,
            ] {
                assert!(routes.iter().any(|route| route.id.selector() == selector
                    && route.id.value() == value
                    && route.projections == super::EDGE_STROKE_ONLY_PROJECTIONS));
            }
        }
    }

    #[test]
    fn manifest_keeps_gitgraph_text_inheritance_and_specific_label_routes_separate() {
        for (target, projections) in [
            (ThemeTarget::Text, super::GITGRAPH_TEXT_FILL_PROJECTIONS),
            (ThemeTarget::NodeLabel, super::NODE_LABEL_FILL_PROJECTIONS),
            (ThemeTarget::EdgeLabel, super::EDGE_LABEL_FILL_PROJECTIONS),
        ] {
            let routes = ACTIVE_ROUTES
                .iter()
                .filter(|route| {
                    route.id.family_id() == DiagramFamilyId::GIT_GRAPH
                        && route.id.target() == target
                })
                .collect::<Vec<_>>();
            assert_eq!(routes.len(), 4);
            for selector in [
                ThemeRouteCutoverSelector::StaticUnqualified,
                ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
            ] {
                for value in [
                    ThemeRouteCutoverValue::Transparent,
                    ThemeRouteCutoverValue::Solid,
                ] {
                    assert!(routes.iter().any(|route| route.id.selector() == selector
                        && route.id.value() == value
                        && route.id.facet() == ThemeRouteCutoverFacet::Fill
                        && route.projections == projections));
                }
            }
        }
    }

    #[test]
    fn manifest_keeps_kanban_text_fill_projection_local() {
        let routes = ACTIVE_ROUTES
            .iter()
            .filter(|route| {
                route.id.family_id() == DiagramFamilyId::KANBAN
                    && route.id.target() == ThemeTarget::Text
            })
            .collect::<Vec<_>>();
        assert_eq!(routes.len(), 4);
        for selector in [
            ThemeRouteCutoverSelector::StaticUnqualified,
            ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
        ] {
            for value in [
                ThemeRouteCutoverValue::Solid,
                ThemeRouteCutoverValue::Transparent,
            ] {
                assert!(routes.iter().any(|route| {
                    route.id.selector() == selector
                        && route.id.facet() == ThemeRouteCutoverFacet::Fill
                        && route.id.value() == value
                        && route.projections == TEXT_FILL_PROJECTIONS
                }));
            }
        }
    }

    #[test]
    fn manifest_keeps_journey_fill_and_stroke_projection_local() {
        let journey_routes = ACTIVE_ROUTES
            .iter()
            .filter(|route| route.id.family_id() == DiagramFamilyId::JOURNEY)
            .collect::<Vec<_>>();
        assert_eq!(journey_routes.len(), 12);
        for (target, facet, projection) in [
            (
                ThemeTarget::JourneyTask,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverProjection::JourneyTaskFill,
            ),
            (
                ThemeTarget::JourneyTask,
                ThemeRouteCutoverFacet::Stroke,
                ThemeRouteCutoverProjection::JourneyTaskStroke,
            ),
            (
                ThemeTarget::Text,
                ThemeRouteCutoverFacet::Fill,
                ThemeRouteCutoverProjection::TextFill,
            ),
        ] {
            let routes = journey_routes
                .iter()
                .filter(|route| route.id.target() == target && route.id.facet() == facet)
                .collect::<Vec<_>>();
            assert_eq!(routes.len(), 4);
            assert!(
                routes
                    .iter()
                    .all(|route| route.projections == &[projection])
            );
            assert!(routes.iter().any(|route| {
                route.id.selector() == ThemeRouteCutoverSelector::StaticUnqualified
            }));
            assert!(routes.iter().any(|route| {
                route.id.selector()
                    == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default)
            }));
        }
    }

    #[test]
    fn manifest_rejects_new_removed_or_projection_drifted_routes() {
        let current = current_inventory();
        let unknown = ThemeRouteCutoverId::new(
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::Edge,
            ThemeRouteCutoverSelector::StaticUnqualified,
            ThemeRouteCutoverFacet::Fill,
            ThemeRouteCutoverValue::Solid,
        );
        let mut added = current.clone();
        added.push((unknown, ThemeRouteCutoverProjectionSet::REPLACE_NODE_FILL));
        assert!(reconcile_manifest(&added, &MANIFEST).is_err());

        assert!(reconcile_manifest(&current[1..], &MANIFEST).is_err());

        let mut drifted = current;
        let drift_index = drifted
            .iter()
            .position(|(_, projections)| {
                *projections != ThemeRouteCutoverProjectionSet::REPLACE_NODE_STROKE
            })
            .expect("current inventory contains a non-node-stroke projection");
        drifted[drift_index].1 = ThemeRouteCutoverProjectionSet::REPLACE_NODE_STROKE;
        assert!(reconcile_manifest(&drifted, &MANIFEST).is_err());
    }

    #[test]
    fn manifest_rejects_duplicates_and_unverified_tombstones() {
        let current = current_inventory();
        let duplicate_active = [ACTIVE_ROUTES[0], ACTIVE_ROUTES[0]];
        let duplicate_manifest = CutoverAuthorizationManifest {
            version: CUTOVER_AUTHORIZATION_MANIFEST_VERSION,
            active: &duplicate_active,
            tombstones: &[],
        };
        assert!(reconcile_manifest(&current, &duplicate_manifest).is_err());

        let tombstone = [RouteTombstone {
            id: ACTIVE_ROUTES[0].id,
            retired_in_version: 1,
            reason: "test",
        }];
        let tombstoned_manifest = CutoverAuthorizationManifest {
            version: CUTOVER_AUTHORIZATION_MANIFEST_VERSION,
            active: &ACTIVE_ROUTES[1..],
            tombstones: &tombstone,
        };
        assert!(reconcile_manifest(&current[1..], &tombstoned_manifest).is_err());
    }

    #[test]
    fn manifest_rejects_projection_action_drift() {
        let mut drifted = PROJECTION_ACTIONS;
        let marker = drifted
            .iter_mut()
            .find(|(projection, _)| *projection == ThemeRouteCutoverProjection::MarkerPaintFromEdge)
            .expect("marker projection action");
        marker.1 = ThemeRouteCutoverProjectionAction::Replace;

        assert!(validate_projection_actions(&MANIFEST, &drifted).is_err());
    }

    #[test]
    fn manifest_rejects_unreferenced_projection_action() {
        let empty_projections: &[ThemeRouteCutoverProjection] = &[];
        let active: [RouteAuthorization; ACTIVE_ROUTES.len()] = std::array::from_fn(|index| {
            if ACTIVE_ROUTES[index]
                .projections
                .contains(&ThemeRouteCutoverProjection::NodeFill)
            {
                RouteAuthorization {
                    id: ACTIVE_ROUTES[index].id,
                    projections: empty_projections,
                }
            } else {
                ACTIVE_ROUTES[index]
            }
        });
        let manifest = CutoverAuthorizationManifest {
            version: CUTOVER_AUTHORIZATION_MANIFEST_VERSION,
            active: &active,
            tombstones: &[],
        };

        assert!(validate_projection_actions(&manifest, &PROJECTION_ACTIONS).is_err());
    }

    #[test]
    fn route_authorization_constructor_is_const_usable() {
        const ROUTE: RouteAuthorization = RouteAuthorization::new(
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Actor,
            ThemeRouteCutoverFacet::Fill,
            ThemeRouteCutoverValue::Solid,
            ACTOR_FILL_PROJECTIONS,
        );
        assert_eq!(ROUTE.id.family_id(), DiagramFamilyId::SEQUENCE);

        const QUALIFIED_ROUTE: RouteAuthorization = RouteAuthorization::new_variant(
            DiagramFamilyId::GANTT,
            ThemeTarget::Task,
            ThemeVariant::Active,
            ThemeRouteCutoverFacet::Fill,
            ThemeRouteCutoverValue::Solid,
            GANTT_TASK_ACTIVE_FILL_PROJECTIONS,
        );
        assert_eq!(
            QUALIFIED_ROUTE.id.selector(),
            ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Active)
        );
    }
}
