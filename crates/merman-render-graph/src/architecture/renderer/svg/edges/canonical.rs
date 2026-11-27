use super::super::{ArchitectureCanonicalEdge, ArchitectureCanonicalSvg};
use super::{
    ArchitectureArrowGeometry, ArchitectureArrowPoints, ArchitectureArrowTransform,
    ArchitectureEdgeLabelPlan, SvgDiagramId, SvgOutput, bounds_from_rect, fmt,
    write_architecture_edge_id_attr,
};
use crate::architecture::ArchitectureDiagramLayout;
use base64::Engine as _;
use merman_render_core::xml::escape_attr_into;
use merman_render_core::{Error, Result};
use merman_render_core::{LayoutEdge, LayoutPoint, route_paint::CanonicalPaintScratch};
use std::borrow::Cow;

pub(super) struct Shaft<'a> {
    pub(super) edge: &'a LayoutEdge,
    pub(super) painted: ArchitectureCanonicalEdge<'a>,
}

impl<'a> Shaft<'a> {
    pub(super) fn new(
        geometry: &dyn ArchitectureCanonicalSvg,
        layout: &'a ArchitectureDiagramLayout,
        ordinal: usize,
        work_meter: &merman_render_core::resources::OperationWorkMeter,
        scratch: &'a mut CanonicalPaintScratch,
    ) -> Result<Self> {
        let edge = layout
            .edges
            .get(ordinal)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("Missing Architecture route edge-{ordinal}"),
            })?;
        let painted = geometry.paint_edge(edge, work_meter, scratch)?;
        Ok(Self { edge, painted })
    }

    pub(super) fn write(
        &self,
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        ordinal: usize,
        points_json: &mut Vec<u8>,
        points_base64: &mut String,
    ) -> Result<()> {
        points_json.clear();
        points_json.reserve(128);
        serde_json::to_writer(&mut *points_json, self.painted.data_points).map_err(|error| {
            Error::InvalidModel {
                message: format!("Could not encode Architecture route: {error}"),
            }
        })?;
        points_base64.clear();
        base64::engine::general_purpose::STANDARD.encode_string(points_json, points_base64);
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
        let _ = write!(out, r#" data-points="{points_base64}""#);
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
        let arrow = |size, tip, lead| (size > 0.0).then(|| arrow_at(tip, lead, size, points));
        let painted = &self.painted;
        let ends = &painted.marker_points;
        (
            painted
                .data_points
                .first()
                .and_then(|tip| arrow(painted.markers.0.width, tip, &ends[1])),
            painted
                .data_points
                .last()
                .and_then(|tip| arrow(painted.markers.1.width, tip, &ends[2])),
        )
    }

    pub(super) fn label<'text>(
        &self,
        geometry: &'text dyn ArchitectureCanonicalSvg,
        ordinal: usize,
    ) -> Option<ArchitectureEdgeLabelPlan<'text>> {
        let text = geometry.edge_label(ordinal, self.edge.label.as_ref())?;
        let anchor = text.anchor;
        Some(ArchitectureEdgeLabelPlan {
            lines: Cow::Borrowed(text.lines),
            bounds: text.bounds,
            dominant_baseline: "middle",
            transform: format!("translate({}, {})", fmt(anchor.x), fmt(anchor.y)),
        })
    }
}

fn arrow_at<'a>(
    tip: &LayoutPoint,
    lead: &LayoutPoint,
    size: f64,
    points: &'a ArchitectureArrowPoints,
) -> ArchitectureArrowGeometry<'a> {
    let half = size / 2.0;
    let (side, x, y) = if lead.x < tip.x {
        ('L', tip.x - size, tip.y - half)
    } else if lead.x > tip.x {
        ('R', tip.x, tip.y - half)
    } else if lead.y < tip.y {
        ('T', tip.x - half, tip.y - size)
    } else {
        ('B', tip.x - half, tip.y)
    };
    ArchitectureArrowGeometry {
        points: points.get(side),
        transform: ArchitectureArrowTransform::Translate { x, y },
        bounds: bounds_from_rect(x, y, size, size),
    }
}
