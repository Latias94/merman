use super::BlockShapeGeometry;
use merman_render_core::{Bounds, LayoutEdge, LayoutNode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    #[serde(
        default,
        rename = "shapeGeometries",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub shape_geometries: Vec<BlockShapeGeometry>,
    pub bounds: Option<Bounds>,
}
