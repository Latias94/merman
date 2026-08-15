mod batch;
mod boxes;
mod draw;
mod lanes;
mod route;
mod scene;

pub(super) use self::batch::plan_layered_relation_component_ref_result;
#[cfg(test)]
pub(super) use self::batch::{
    LayeredRouteBatchError, PairwiseValidationWork, measure_pairwise_validation_work,
    plan_layered_relation_component_result, plan_layered_route_batch,
    plan_layered_route_batch_with_probes,
};
pub(crate) use self::boxes::{
    LayeredRelationEdge, LayeredRelationError, RelationGraphComponent, relation_components,
};
#[cfg(test)]
pub(crate) use self::boxes::{PlacedRelationGraphBox, plan_layered_relation_boxes};
#[cfg(test)]
pub(crate) use self::draw::write_centered_relation_label;
pub(crate) use self::draw::{
    RelationLineChars, centered_label_lines_with_role, centered_text_line_with_role,
    label_lines_with_role, marker_line_with_role, put_relation_char,
};
#[cfg(test)]
pub(crate) use self::lanes::parallel_relation_lane_offsets;
pub(crate) use self::route::{
    LayeredRelationPhysicalPort, LayeredRelationPhysicalSide, LayeredRelationRouteGeometry,
    LayeredRelationRoutePlan, LayeredRelationRouteProfile, LayeredRelationRouteStyle,
    RelationOverlay,
};
#[cfg(test)]
pub(crate) use self::route::{
    LayeredRelationRouteRequest, plan_layered_relation_route,
    spanning_lane_offset_around_intermediate_boxes,
};
pub(crate) use self::scene::{LayeredRelationScene, LayeredRelationSummaryReason};
#[cfg(test)]
pub(crate) use self::scene::{LayeredRelationScenePlan, plan_layered_relation_scene};
