//! Native node depth silhouettes shared by viewport preparation and SVG emission.

use kurbo::{BezPath, Shape};
use merman_render_core::{
    LayoutNode, Result, resources::OperationWorkMeter, svg_output::SvgOutput, theme_svg::elevation,
};
use merman_theme_contract::v2::Elevation;

use crate::{
    document::FlowchartDocumentId, node_geometry::FlowchartRectangleGeometry,
    shapes::FlowchartShape, style::node::PreparedNodeSelection,
    svg_label::FlowchartSvgLabelSidecar,
};

pub fn native_node_silhouette(
    node: &LayoutNode,
    shape: &str,
    selection: &PreparedNodeSelection,
    svg_labels: Option<&FlowchartSvgLabelSidecar>,
    swimlane: bool,
) -> Option<BezPath> {
    let width = node.width;
    let height = node.height;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    match FlowchartShape::resolve(shape)? {
        FlowchartShape::Process | FlowchartShape::RoundedRectangle => {
            let rect = svg_labels
                .and_then(|labels| {
                    FlowchartRectangleGeometry::from_sidecar(labels, &node.id, swimlane)
                })
                .unwrap_or_else(|| FlowchartRectangleGeometry::from_layout_node(node))
                .shape;
            let radius = selection.source_radii[0]
                .or_else(|| selection.native_radius.map(f64::from))
                .unwrap_or(0.0);
            Some(rect.to_rounded_rect(radius).to_path(0.05))
        }
        FlowchartShape::Stadium => Some(
            kurbo::Rect::from_center_size((0.0, 0.0), (width, height))
                .to_rounded_rect(height / 2.0)
                .to_path(0.05),
        ),
        FlowchartShape::Circle | FlowchartShape::DoubleCircle => {
            Some(kurbo::Circle::new((0.0, 0.0), width.min(height) / 2.0).to_path(0.05))
        }
        FlowchartShape::Diamond => {
            let (dx, dy) = (width / 2.0, height / 2.0);
            let mut path = BezPath::new();
            path.move_to((0.5, -dy));
            path.line_to((0.5 + dx, 0.0));
            path.line_to((0.5, dy));
            path.line_to((0.5 - dx, 0.0));
            path.close_path();
            Some(path)
        }
        _ => None,
    }
}

pub fn write_native_node_depth(
    out: &mut impl SvgOutput,
    elevation: &Elevation,
    id: FlowchartDocumentId<'_>,
    silhouette: &BezPath,
    center: (f64, f64),
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    work_meter.charge(silhouette.elements().len())?;
    let _ = elevation::write_depth(out, elevation, &id.to_string(), silhouette, center);
    Ok(())
}
