//! Shared Mermaid edge-marker geometry and DOM emission.
//!
//! The base order and attributes are fixed by Mermaid's shared rendering-elements helper.
//! Typed family writers reuse these geometries when a marker needs a separate paint instance.

use std::fmt::Write as _;

use super::SvgDiagramId;
use super::util::{escape_attr, escape_xml};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BaseEdgeMarkerKind {
    PointEnd,
    PointStart,
    PointEndMargin,
    PointStartMargin,
    CircleEnd,
    CircleStart,
    CircleEndMargin,
    CircleStartMargin,
    CrossEnd,
    CrossStart,
    CrossEndMargin,
    CrossStartMargin,
}

#[derive(Debug)]
pub(crate) struct BaseEdgeMarkerGeometry {
    pub(crate) attributes: &'static [(&'static str, &'static str)],
    pub(crate) shape: &'static str,
    pub(crate) shape_attributes: &'static [(&'static str, &'static str)],
    pub(crate) style: &'static str,
}

impl BaseEdgeMarkerKind {
    pub(crate) const ALL: [Self; 12] = [
        Self::PointEnd,
        Self::PointStart,
        Self::PointEndMargin,
        Self::PointStartMargin,
        Self::CircleEnd,
        Self::CircleStart,
        Self::CircleEndMargin,
        Self::CircleStartMargin,
        Self::CrossEnd,
        Self::CrossStart,
        Self::CrossEndMargin,
        Self::CrossStartMargin,
    ];

    pub(crate) const fn suffix(self) -> &'static str {
        match self {
            Self::PointEnd => "pointEnd",
            Self::PointStart => "pointStart",
            Self::PointEndMargin => "pointEnd-margin",
            Self::PointStartMargin => "pointStart-margin",
            Self::CircleEnd => "circleEnd",
            Self::CircleStart => "circleStart",
            Self::CircleEndMargin => "circleEnd-margin",
            Self::CircleStartMargin => "circleStart-margin",
            Self::CrossEnd => "crossEnd",
            Self::CrossStart => "crossStart",
            Self::CrossEndMargin => "crossEnd-margin",
            Self::CrossStartMargin => "crossStart-margin",
        }
    }

    pub(crate) const fn cross(self) -> bool {
        matches!(
            self,
            Self::CrossEnd | Self::CrossStart | Self::CrossEndMargin | Self::CrossStartMargin
        )
    }

    pub(crate) fn from_arrow(arrow: Option<&str>, start: bool) -> Option<Self> {
        match (arrow.unwrap_or("").trim(), start) {
            ("arrow_point", false) => Some(Self::PointEnd),
            ("arrow_point", true) => Some(Self::PointStart),
            ("arrow_circle", false) => Some(Self::CircleEnd),
            ("arrow_circle", true) => Some(Self::CircleStart),
            ("arrow_cross", false) => Some(Self::CrossEnd),
            ("arrow_cross", true) => Some(Self::CrossStart),
            (_, false)
                if !arrow.unwrap_or("").trim().is_empty()
                    && arrow.unwrap_or("").trim() != "arrow_open" =>
            {
                Some(Self::PointEnd)
            }
            _ => None,
        }
    }

    pub(crate) const fn geometry(self) -> BaseEdgeMarkerGeometry {
        match self {
            Self::PointEnd => BaseEdgeMarkerGeometry {
                attributes: &[
                    ("viewBox", "0 0 10 10"),
                    ("refX", "5"),
                    ("refY", "5"),
                    ("markerUnits", "userSpaceOnUse"),
                    ("markerWidth", "8"),
                    ("markerHeight", "8"),
                    ("orient", "auto"),
                ],
                shape: "path",
                shape_attributes: &[("d", "M 0 0 L 10 5 L 0 10 z")],
                style: "stroke-width: 1; stroke-dasharray: 1, 0;",
            },
            Self::PointStart => BaseEdgeMarkerGeometry {
                attributes: &[
                    ("viewBox", "0 0 10 10"),
                    ("refX", "4.5"),
                    ("refY", "5"),
                    ("markerUnits", "userSpaceOnUse"),
                    ("markerWidth", "8"),
                    ("markerHeight", "8"),
                    ("orient", "auto"),
                ],
                shape: "path",
                shape_attributes: &[("d", "M 0 5 L 10 10 L 10 0 z")],
                style: "stroke-width: 1; stroke-dasharray: 1, 0;",
            },
            Self::PointEndMargin => BaseEdgeMarkerGeometry {
                attributes: &[
                    ("viewBox", "0 0 11.5 14"),
                    ("refX", "11.5"),
                    ("refY", "7"),
                    ("markerUnits", "userSpaceOnUse"),
                    ("markerWidth", "10.5"),
                    ("markerHeight", "14"),
                    ("orient", "auto"),
                ],
                shape: "path",
                shape_attributes: &[("d", "M 0 0 L 11.5 7 L 0 14 z")],
                style: "stroke-width: 0; stroke-dasharray: 1, 0;",
            },
            Self::PointStartMargin => BaseEdgeMarkerGeometry {
                attributes: &[
                    ("viewBox", "0 0 11.5 14"),
                    ("refX", "1"),
                    ("refY", "7"),
                    ("markerUnits", "userSpaceOnUse"),
                    ("markerWidth", "11.5"),
                    ("markerHeight", "14"),
                    ("orient", "auto"),
                ],
                shape: "polygon",
                shape_attributes: &[("points", "0,7 11.5,14 11.5,0")],
                style: "stroke-width: 0; stroke-dasharray: 1, 0;",
            },
            Self::CircleEnd => BaseEdgeMarkerGeometry {
                attributes: &[
                    ("viewBox", "0 0 10 10"),
                    ("refX", "11"),
                    ("refY", "5"),
                    ("markerUnits", "userSpaceOnUse"),
                    ("markerWidth", "11"),
                    ("markerHeight", "11"),
                    ("orient", "auto"),
                ],
                shape: "circle",
                shape_attributes: &[("cx", "5"), ("cy", "5"), ("r", "5")],
                style: "stroke-width: 1; stroke-dasharray: 1, 0;",
            },
            Self::CircleStart => BaseEdgeMarkerGeometry {
                attributes: &[
                    ("viewBox", "0 0 10 10"),
                    ("refX", "-1"),
                    ("refY", "5"),
                    ("markerUnits", "userSpaceOnUse"),
                    ("markerWidth", "11"),
                    ("markerHeight", "11"),
                    ("orient", "auto"),
                ],
                shape: "circle",
                shape_attributes: &[("cx", "5"), ("cy", "5"), ("r", "5")],
                style: "stroke-width: 1; stroke-dasharray: 1, 0;",
            },
            Self::CircleEndMargin => BaseEdgeMarkerGeometry {
                attributes: &[
                    ("viewBox", "0 0 10 10"),
                    ("refY", "5"),
                    ("refX", "12.25"),
                    ("markerUnits", "userSpaceOnUse"),
                    ("markerWidth", "14"),
                    ("markerHeight", "14"),
                    ("orient", "auto"),
                ],
                shape: "circle",
                shape_attributes: &[("cx", "5"), ("cy", "5"), ("r", "5")],
                style: "stroke-width: 0; stroke-dasharray: 1, 0;",
            },
            Self::CircleStartMargin => BaseEdgeMarkerGeometry {
                attributes: &[
                    ("viewBox", "0 0 10 10"),
                    ("refX", "-2"),
                    ("refY", "5"),
                    ("markerUnits", "userSpaceOnUse"),
                    ("markerWidth", "14"),
                    ("markerHeight", "14"),
                    ("orient", "auto"),
                ],
                shape: "circle",
                shape_attributes: &[("cx", "5"), ("cy", "5"), ("r", "5")],
                style: "stroke-width: 0; stroke-dasharray: 1, 0;",
            },
            Self::CrossEnd => BaseEdgeMarkerGeometry {
                attributes: &[
                    ("viewBox", "0 0 11 11"),
                    ("refX", "12"),
                    ("refY", "5.2"),
                    ("markerUnits", "userSpaceOnUse"),
                    ("markerWidth", "11"),
                    ("markerHeight", "11"),
                    ("orient", "auto"),
                ],
                shape: "path",
                shape_attributes: &[("d", "M 1,1 l 9,9 M 10,1 l -9,9")],
                style: "stroke-width: 2; stroke-dasharray: 1, 0;",
            },
            Self::CrossStart => BaseEdgeMarkerGeometry {
                attributes: &[
                    ("viewBox", "0 0 11 11"),
                    ("refX", "-1"),
                    ("refY", "5.2"),
                    ("markerUnits", "userSpaceOnUse"),
                    ("markerWidth", "11"),
                    ("markerHeight", "11"),
                    ("orient", "auto"),
                ],
                shape: "path",
                shape_attributes: &[("d", "M 1,1 l 9,9 M 10,1 l -9,9")],
                style: "stroke-width: 2; stroke-dasharray: 1, 0;",
            },
            Self::CrossEndMargin => BaseEdgeMarkerGeometry {
                attributes: &[
                    ("viewBox", "0 0 15 15"),
                    ("refX", "17.7"),
                    ("refY", "7.5"),
                    ("markerUnits", "userSpaceOnUse"),
                    ("markerWidth", "12"),
                    ("markerHeight", "12"),
                    ("orient", "auto"),
                ],
                shape: "path",
                shape_attributes: &[("d", "M 1,1 L 14,14 M 1,14 L 14,1")],
                style: "stroke-width: 2.5;",
            },
            Self::CrossStartMargin => BaseEdgeMarkerGeometry {
                attributes: &[
                    ("viewBox", "0 0 15 15"),
                    ("refX", "-3.5"),
                    ("refY", "7.5"),
                    ("markerUnits", "userSpaceOnUse"),
                    ("markerWidth", "12"),
                    ("markerHeight", "12"),
                    ("orient", "auto"),
                ],
                shape: "path",
                shape_attributes: &[("d", "M 1,1 L 14,14 M 1,14 L 14,1")],
                style: "stroke-width: 2.5; stroke-dasharray: 1, 0;",
            },
        }
    }
}

pub(in crate::svg::parity) fn push_base_edge_marker(
    out: &mut String,
    diagram_id: SvgDiagramId<'_>,
    diagram_type: &str,
    kind: BaseEdgeMarkerKind,
    suffix: &str,
    paint_style: &str,
) {
    let ty = escape_xml(diagram_type);
    let geometry = kind.geometry();
    let cross = if kind.cross() { "cross " } else { "" };
    let _ = write!(
        out,
        r#"<marker id="{}_{}-{}" class="marker {}{}""#,
        diagram_id,
        ty,
        escape_attr(suffix),
        cross,
        ty
    );
    for (name, value) in geometry.attributes {
        let _ = write!(out, r#" {}="{}""#, name, value);
    }
    let _ = write!(out, "><{}", geometry.shape);
    for (name, value) in geometry.shape_attributes {
        let _ = write!(out, r#" {}="{}""#, name, value);
    }
    let _ = write!(
        out,
        r#" class="arrowMarkerPath" style="{}{}"/></marker>"#,
        geometry.style,
        escape_attr(paint_style)
    );
}

pub(in crate::svg::parity) fn push_base_edge_markers(
    out: &mut String,
    diagram_id: SvgDiagramId<'_>,
    diagram_type: &str,
) {
    for kind in BaseEdgeMarkerKind::ALL {
        push_base_edge_marker(out, diagram_id, diagram_type, kind, kind.suffix(), "");
    }
}
