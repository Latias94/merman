mod edge_geometry;
#[cfg(feature = "layout-shoal")]
mod orthogonal;

pub use merman_render_structured::block::{
    BlockAllocatedBounds, BlockRectangleKind, BlockShapeBoundary, BlockShapeGeometry,
};

pub(crate) use edge_geometry::BlockEdgeRoutes;
pub(crate) use merman_render_structured::block::{
    BlockNodeBindings, BlockPreparedLabels, BlockTypographyThemePlan, prepare_block_layout,
};
#[cfg(feature = "layout-shoal")]
use merman_render_structured::block::{
    block_label_is_effectively_empty, block_render_edge_points, decode_block_label_html,
    resolve_block_node_sources,
};
