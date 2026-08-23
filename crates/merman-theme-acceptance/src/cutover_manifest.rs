use std::collections::{BTreeMap, BTreeSet};

use merman::DiagramFamilyId;
use merman::svg::ThemeTarget;
use merman_render::__private::{
    ThemeRouteCutoverDescriptor, ThemeRouteCutoverFacet, ThemeRouteCutoverId,
    ThemeRouteCutoverProjection, ThemeRouteCutoverProjectionAction, ThemeRouteCutoverProjectionSet,
    ThemeRouteCutoverSelector, ThemeRouteCutoverValue,
};

use crate::runner::{C6ProofError, C6ProofResult};

const CUTOVER_AUTHORIZATION_MANIFEST_VERSION: u16 = 19;

const PROJECTION_ACTIONS: [(
    ThemeRouteCutoverProjection,
    ThemeRouteCutoverProjectionAction,
); 36] = [
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
        ThemeRouteCutoverProjection::JourneyTaskPaint,
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
}

const NODE_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::NodeFill];
const NODE_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::NodeStroke];
const NODE_LABEL_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::NodeLabelFill];
const EDGE_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] = &[
    ThemeRouteCutoverProjection::EdgeStroke,
    ThemeRouteCutoverProjection::MarkerPaintFromEdge,
];
const EDGE_STROKE_ONLY_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::EdgeStroke];
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
const PIE_SLICE_STROKE_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::PieSliceStroke];
const PIE_SLICE_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::PieSliceFill];
const TEXT_FILL_PROJECTIONS: &[ThemeRouteCutoverProjection] =
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
const JOURNEY_TASK_PAINT_PROJECTIONS: &[ThemeRouteCutoverProjection] =
    &[ThemeRouteCutoverProjection::JourneyTaskPaint];

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

const ACTIVE_ROUTES: [RouteAuthorization; 100] = [
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
    route(
        DiagramFamilyId::TREEMAP,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        TITLE_FILL_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::TREEMAP,
        ThemeTarget::Title,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        TITLE_FILL_PROJECTIONS,
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
        DiagramFamilyId::JOURNEY,
        ThemeTarget::JourneyTask,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Transparent,
        JOURNEY_TASK_PAINT_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::JourneyTask,
        ThemeRouteCutoverFacet::Fill,
        ThemeRouteCutoverValue::Solid,
        JOURNEY_TASK_PAINT_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::JourneyTask,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Transparent,
        JOURNEY_TASK_PAINT_PROJECTIONS,
    ),
    route(
        DiagramFamilyId::JOURNEY,
        ThemeTarget::JourneyTask,
        ThemeRouteCutoverFacet::Stroke,
        ThemeRouteCutoverValue::Solid,
        JOURNEY_TASK_PAINT_PROJECTIONS,
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
    use merman::DiagramFamilyId;
    use merman::svg::ThemeTarget;
    use merman_render::__private::{
        ThemeRouteCutoverFacet, ThemeRouteCutoverId, ThemeRouteCutoverProjection,
        ThemeRouteCutoverProjectionAction, ThemeRouteCutoverProjectionSet,
        ThemeRouteCutoverSelector, ThemeRouteCutoverValue, legacy_replacing_typed_theme_routes,
    };

    use super::{
        ACTIVE_ROUTES, ACTOR_FILL_PROJECTIONS, CUTOVER_AUTHORIZATION_MANIFEST_VERSION,
        CutoverAuthorizationManifest, MANIFEST, PROJECTION_ACTIONS, RouteAuthorization,
        RouteTombstone, authorize_cutover_routes, reconcile_manifest, validate_projection_actions,
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
    }
}
