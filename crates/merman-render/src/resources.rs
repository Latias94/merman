//! Resource admission shared with the rendering core.

pub use merman_render_core::resources::{
    CLI_DEFAULT_RESOURCE_PROFILE, GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
    MAX_PORTABLE_ICON_BODY_XML_DEPTH, MAX_RESVG_TREE_DEPTH, MAX_RESVG_TREE_NODES, ModelComplexity,
    RESOURCE_LIMIT_COUNT, RESOURCE_LIMIT_DESCRIPTORS, RESOURCE_PROFILE_COUNT,
    RESOURCE_PROFILE_DESCRIPTORS, RenderResourceLimitId, RenderResourcePolicy,
    RenderResourceProfile, RenderResourceProfileDescriptor, ResourceLimitCause,
    ResourceLimitDescriptor, ResourceLimitExceeded, ResourceLimitId, ResourceLimitOverride,
    ResourceLimitOverrideError, ResourceLimitPhase, ResourcePolicyRestrictionError,
    SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID, SVG_BACKEND_TREE_NODES_HARD_CAP_ID,
    WASM_RESVG_TREE_DEPTH_HARD_CAP, resource_limit_descriptors, resource_profile_descriptors,
};

#[cfg(all(feature = "diagram-architecture", feature = "layout-shoal"))]
pub(crate) use merman_render_core::resources::PreparedTextRetainedReservation;
pub(crate) use merman_render_core::resources::{OperationWorkError, OperationWorkMeter};

#[cfg(feature = "diagram-class")]
pub use merman_render_core::resources::ClassComplexity;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub use merman_render_core::resources::FlowchartComplexity;
#[cfg(feature = "diagram-mindmap")]
pub use merman_render_core::resources::MindmapComplexity;
#[cfg(feature = "diagram-sequence")]
pub use merman_render_core::resources::SequenceComplexity;
#[cfg(feature = "diagram-zenuml")]
pub use merman_render_core::resources::ZenumlComplexity;
