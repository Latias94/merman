use super::{
    ArchitectureArrowGeometry, ArchitectureArrowPoints, ArchitectureArrowTransform,
    ArchitectureEdgeLabelPlan, SvgDiagramId, SvgOutput, bounds_from_rect, escape_attr_into, fmt,
    write_architecture_edge_id_attr,
};
use crate::architecture::{ArchitectureShoalGeometry, ArchitectureShoalText};
use crate::graph_layout::canonical::CanonicalPaintedEdge;
use crate::graph_layout::route_paint::RouteStyle;
use crate::model::{ArchitectureDiagramLayout, LayoutEdge, LayoutPoint};
use crate::{Error, Result};
use base64::Engine as _;
use std::borrow::Cow;

pub(super) struct Shaft<'a> {
    pub(super) edge: &'a LayoutEdge,
    pub(super) painted: CanonicalPaintedEdge,
}

impl<'a> Shaft<'a> {
    pub(super) fn new(
        geometry: &ArchitectureShoalGeometry,
        layout: &'a ArchitectureDiagramLayout,
        ordinal: usize,
    ) -> Result<Self> {
        let edge = layout
            .edges
            .get(ordinal)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("Missing Architecture route edge-{ordinal}"),
            })?;
        let painted = geometry
            .routes
            .paint(edge, kurbo::Vec2::ZERO, 0.0, RouteStyle::Step)?;
        Ok(Self { edge, painted })
    }

    pub(super) fn write(
        &self,
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        ordinal: usize,
        style: Option<&str>,
    ) -> Result<()> {
        let points =
            serde_json::to_vec(&self.painted.data_points).map_err(|error| Error::InvalidModel {
                message: format!("Could not encode Architecture route: {error}"),
            })?;
        let points = base64::engine::general_purpose::STANDARD.encode(points);
        out.push_str(r#"<path d=""#);
        escape_attr_into(out, &self.painted.path.d);
        out.push_str(r#"" class="edge" id=""#);
        write_architecture_edge_id_attr(
            out,
            diagram_id,
            "L",
            &self.edge.from,
            &self.edge.to,
            ordinal,
        );
        out.push('"');
        super::super::write_geometry_attrs(out, "edge", &self.edge.id);
        let _ = write!(out, r#" data-points="{points}""#);
        if let Some(style) = style {
            out.push_str(r#" style=""#);
            escape_attr_into(out, style);
            out.push('"');
        }
        out.push_str("/>");
        Ok(())
    }

    pub(super) fn arrows<'points>(
        &self,
        points: &'points ArchitectureArrowPoints,
    ) -> (
        Option<ArchitectureArrowGeometry<'points>>,
        Option<ArchitectureArrowGeometry<'points>>,
    ) {
        let arrow = |size, tip, base| (size > 0.0).then(|| arrow_at(tip, base, size, points));
        let painted = &self.painted;
        let ends = &painted.marker_points;
        (
            painted
                .data_points
                .first()
                .and_then(|tip| arrow(painted.markers.0.width, tip, &ends[0])),
            painted
                .data_points
                .last()
                .and_then(|tip| arrow(painted.markers.1.width, tip, &ends[3])),
        )
    }

    pub(super) fn label<'text>(
        &self,
        text: Option<&'text ArchitectureShoalText>,
    ) -> Option<ArchitectureEdgeLabelPlan<'text>> {
        let (label, text) = self.edge.label.as_ref().zip(text)?;
        let anchor = text.centered_at(label);
        Some(ArchitectureEdgeLabelPlan {
            lines: Cow::Borrowed(&text.lines),
            bounds: text.bounds_at(anchor),
            dominant_baseline: "middle",
            transform: format!("translate({}, {})", fmt(anchor.x), fmt(anchor.y)),
        })
    }
}

fn arrow_at<'a>(
    tip: &LayoutPoint,
    base: &LayoutPoint,
    size: f64,
    points: &'a ArchitectureArrowPoints,
) -> ArchitectureArrowGeometry<'a> {
    let half = size / 2.0;
    let (side, x, y) = if base.x < tip.x {
        ('L', base.x, tip.y - half)
    } else if base.x > tip.x {
        ('R', tip.x, tip.y - half)
    } else if base.y < tip.y {
        ('T', tip.x - half, base.y)
    } else {
        ('B', tip.x - half, tip.y)
    };
    ArchitectureArrowGeometry {
        points: points.get(side),
        transform: ArchitectureArrowTransform::Translate { x, y },
        bounds: bounds_from_rect(x, y, size, size),
    }
}
