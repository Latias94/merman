use merman_render_core::{
    Bounds, LayoutEdge, LayoutLabel, LayoutPoint, Result,
    resources::OperationWorkMeter,
    route_paint::{CanonicalPaintScratch, PaintedRoute},
    route_types::Marker,
};

use super::super::labels::SvgLine;

pub struct ArchitectureCanonicalGroup {
    pub parent: Option<u32>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub icon_origin: Option<LayoutPoint>,
}

pub struct ArchitecturePlacedText<'a> {
    pub lines: &'a [SvgLine],
    pub anchor: LayoutPoint,
    pub bounds: Bounds,
}

pub struct ArchitectureCanonicalEdge<'points> {
    pub path: PaintedRoute,
    pub data_points: &'points [LayoutPoint],
    pub marker_points: [LayoutPoint; 4],
    pub markers: (Marker, Marker),
}

/// Views of the prepared groups and labels in semantic occurrence order, plus late route paint.
/// The same artifact must supply the layout and this sidecar for the duration of emission.
pub trait ArchitectureCanonicalSvg {
    fn group_count(&self) -> usize;
    fn group(&self, ordinal: usize) -> ArchitectureCanonicalGroup;
    fn service_title(&self, ordinal: usize) -> Option<(&[SvgLine], LayoutPoint)>;
    fn group_title(&self, ordinal: usize) -> Option<ArchitecturePlacedText<'_>>;
    fn edge_label(
        &self,
        ordinal: usize,
        label: Option<&LayoutLabel>,
    ) -> Option<ArchitecturePlacedText<'_>>;

    /// Paints the prepared route for the matching layout edge. The returned points borrow the
    /// writer's scratch buffer until that edge has been encoded.
    fn paint_edge<'points>(
        &self,
        edge: &LayoutEdge,
        work_meter: &OperationWorkMeter,
        scratch: &'points mut CanonicalPaintScratch,
    ) -> Result<ArchitectureCanonicalEdge<'points>>;
}
