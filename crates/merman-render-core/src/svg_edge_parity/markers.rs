//! Shared Mermaid edge-marker geometry and DOM emission.
//!
//! The base order and attributes are fixed by Mermaid's shared rendering-elements helper.
//! Typed family writers reuse these geometries when a marker needs a separate paint instance.

use std::fmt::Write as _;

use crate::svg_css::SvgId;
use crate::xml::{escape_attr, escape_attr_display};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BaseEdgeMarkerKind {
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
pub struct BaseEdgeMarkerGeometry {
    pub attributes: &'static [(&'static str, &'static str)],
    pub shape: &'static str,
    pub shape_attributes: &'static [(&'static str, &'static str)],
    pub style: &'static str,
}

// Derive the fixed XML fragment from the same attributes as the structured writer.
// Flow appends this fragment in one write, including when counting bytes.
macro_rules! marker_geometry {
    (
        [$($name:literal => $value:literal),+ $(,)?],
        $shape:literal [$($shape_name:literal => $shape_value:literal),+ $(,)?],
        $style:literal $(, $tail:literal)?
    ) => {
        (
            BaseEdgeMarkerGeometry {
                attributes: &[$(($name, $value)),+],
                shape: $shape,
                shape_attributes: &[$(($shape_name, $shape_value)),+],
                style: $style,
            },
            concat!(
                $(" ", $name, "=\"", $value, "\"",)+
                "><", $shape,
                $(" ", $shape_name, "=\"", $shape_value, "\"",)+
                " class=\"arrowMarkerPath\" style=\"", $style, "\"" $(, $tail)?
            ),
        )
    };
}

impl BaseEdgeMarkerKind {
    #[cfg(test)]
    pub const ALL: [Self; 12] = [
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

    pub const fn suffix(self) -> &'static str {
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

    pub const fn cross(self) -> bool {
        matches!(
            self,
            Self::CrossEnd | Self::CrossStart | Self::CrossEndMargin | Self::CrossStartMargin
        )
    }

    pub fn from_arrow(arrow: Option<&str>, start: bool) -> Option<Self> {
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

    pub const fn geometry(self) -> BaseEdgeMarkerGeometry {
        self.geometry_and_fragment().0
    }

    /// The XML after a shared marker's class attribute and before optional shape paint.
    #[doc(hidden)]
    pub const fn shape_fragment(self) -> &'static str {
        self.geometry_and_fragment().1
    }

    const fn geometry_and_fragment(self) -> (BaseEdgeMarkerGeometry, &'static str) {
        match self {
            Self::PointEnd => marker_geometry!(
                [
                    "viewBox" => "0 0 10 10",
                    "refX" => "5",
                    "refY" => "5",
                    "markerUnits" => "userSpaceOnUse",
                    "markerWidth" => "8",
                    "markerHeight" => "8",
                    "orient" => "auto",
                ],
                "path" ["d" => "M 0 0 L 10 5 L 0 10 z"],
                "stroke-width: 1; stroke-dasharray: 1, 0;"
            ),
            Self::PointStart => marker_geometry!(
                [
                    "viewBox" => "0 0 10 10",
                    "refX" => "4.5",
                    "refY" => "5",
                    "markerUnits" => "userSpaceOnUse",
                    "markerWidth" => "8",
                    "markerHeight" => "8",
                    "orient" => "auto",
                ],
                "path" ["d" => "M 0 5 L 10 10 L 10 0 z"],
                "stroke-width: 1; stroke-dasharray: 1, 0;"
            ),
            Self::PointEndMargin => marker_geometry!(
                [
                    "viewBox" => "0 0 11.5 14",
                    "refX" => "11.5",
                    "refY" => "7",
                    "markerUnits" => "userSpaceOnUse",
                    "markerWidth" => "10.5",
                    "markerHeight" => "14",
                    "orient" => "auto",
                ],
                "path" ["d" => "M 0 0 L 11.5 7 L 0 14 z"],
                "stroke-width: 0; stroke-dasharray: 1, 0;"
            ),
            Self::PointStartMargin => marker_geometry!(
                [
                    "viewBox" => "0 0 11.5 14",
                    "refX" => "1",
                    "refY" => "7",
                    "markerUnits" => "userSpaceOnUse",
                    "markerWidth" => "11.5",
                    "markerHeight" => "14",
                    "orient" => "auto",
                ],
                "polygon" ["points" => "0,7 11.5,14 11.5,0"],
                "stroke-width: 0; stroke-dasharray: 1, 0;"
            ),
            Self::CircleEnd => marker_geometry!(
                [
                    "viewBox" => "0 0 10 10",
                    "refX" => "11",
                    "refY" => "5",
                    "markerUnits" => "userSpaceOnUse",
                    "markerWidth" => "11",
                    "markerHeight" => "11",
                    "orient" => "auto",
                ],
                "circle" ["cx" => "5", "cy" => "5", "r" => "5"],
                "stroke-width: 1; stroke-dasharray: 1, 0;"
            ),
            Self::CircleStart => marker_geometry!(
                [
                    "viewBox" => "0 0 10 10",
                    "refX" => "-1",
                    "refY" => "5",
                    "markerUnits" => "userSpaceOnUse",
                    "markerWidth" => "11",
                    "markerHeight" => "11",
                    "orient" => "auto",
                ],
                "circle" ["cx" => "5", "cy" => "5", "r" => "5"],
                "stroke-width: 1; stroke-dasharray: 1, 0;"
            ),
            Self::CircleEndMargin => marker_geometry!(
                [
                    "viewBox" => "0 0 10 10",
                    "refY" => "5",
                    "refX" => "12.25",
                    "markerUnits" => "userSpaceOnUse",
                    "markerWidth" => "14",
                    "markerHeight" => "14",
                    "orient" => "auto",
                ],
                "circle" ["cx" => "5", "cy" => "5", "r" => "5"],
                "stroke-width: 0; stroke-dasharray: 1, 0;"
            ),
            Self::CircleStartMargin => marker_geometry!(
                [
                    "viewBox" => "0 0 10 10",
                    "refX" => "-2",
                    "refY" => "5",
                    "markerUnits" => "userSpaceOnUse",
                    "markerWidth" => "14",
                    "markerHeight" => "14",
                    "orient" => "auto",
                ],
                "circle" ["cx" => "5", "cy" => "5", "r" => "5"],
                "stroke-width: 0; stroke-dasharray: 1, 0;"
            ),
            Self::CrossEnd => marker_geometry!(
                [
                    "viewBox" => "0 0 11 11",
                    "refX" => "12",
                    "refY" => "5.2",
                    "markerUnits" => "userSpaceOnUse",
                    "markerWidth" => "11",
                    "markerHeight" => "11",
                    "orient" => "auto",
                ],
                "path" ["d" => "M 1,1 l 9,9 M 10,1 l -9,9"],
                "stroke-width: 2; stroke-dasharray: 1, 0;"
            ),
            Self::CrossStart => marker_geometry!(
                [
                    "viewBox" => "0 0 11 11",
                    "refX" => "-1",
                    "refY" => "5.2",
                    "markerUnits" => "userSpaceOnUse",
                    "markerWidth" => "11",
                    "markerHeight" => "11",
                    "orient" => "auto",
                ],
                "path" ["d" => "M 1,1 l 9,9 M 10,1 l -9,9"],
                "stroke-width: 2; stroke-dasharray: 1, 0;"
            ),
            Self::CrossEndMargin => marker_geometry!(
                [
                    "viewBox" => "0 0 15 15",
                    "refX" => "17.7",
                    "refY" => "7.5",
                    "markerUnits" => "userSpaceOnUse",
                    "markerWidth" => "12",
                    "markerHeight" => "12",
                    "orient" => "auto",
                ],
                "path" ["d" => "M 1,1 L 14,14 M 1,14 L 14,1"],
                "stroke-width: 2.5;"
            ),
            Self::CrossStartMargin => marker_geometry!(
                [
                    "viewBox" => "0 0 15 15",
                    "refX" => "-3.5",
                    "refY" => "7.5",
                    "markerUnits" => "userSpaceOnUse",
                    "markerWidth" => "12",
                    "markerHeight" => "12",
                    "orient" => "auto",
                ],
                "path" ["d" => "M 1,1 L 14,14 M 1,14 L 14,1"],
                "stroke-width: 2.5; stroke-dasharray: 1, 0;"
            ),
        }
    }
}

const HIERARCHY_END: (BaseEdgeMarkerGeometry, &str) = marker_geometry!(
    [
        "viewBox" => "0 0 12 10",
        "refX" => "10",
        "refY" => "5",
        "markerUnits" => "userSpaceOnUse",
        "markerWidth" => "12",
        "markerHeight" => "10",
        "orient" => "auto",
    ],
    "path" ["d" => "M 0 0 L 6 5 L 0 10 M 4 0 L 10 5 L 4 10"],
    "stroke-width: 1; stroke-dasharray: 1, 0; fill: none;",
    "/></marker>"
);

const HIERARCHY_START: (BaseEdgeMarkerGeometry, &str) = marker_geometry!(
    [
        "viewBox" => "0 0 12 10",
        "refX" => "2",
        "refY" => "5",
        "markerUnits" => "userSpaceOnUse",
        "markerWidth" => "12",
        "markerHeight" => "10",
        "orient" => "auto",
    ],
    "path" ["d" => "M 12 0 L 6 5 L 12 10 M 8 0 L 2 5 L 8 10"],
    "stroke-width: 1; stroke-dasharray: 1, 0; fill: none;",
    "/></marker>"
);

/// The complete XML after an Agentflow hierarchy marker's class attribute.
#[doc(hidden)]
pub const fn hierarchy_edge_marker_body(start: bool) -> &'static str {
    if start {
        HIERARCHY_START.1
    } else {
        HIERARCHY_END.1
    }
}

pub fn push_base_edge_marker(
    out: &mut String,
    diagram_id: SvgId<'_>,
    diagram_type: &str,
    kind: BaseEdgeMarkerKind,
    suffix: &str,
    paint_style: &str,
) {
    let ty = escape_attr_display(diagram_type);
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

pub fn push_base_edge_markers(out: &mut String, diagram_id: SvgId<'_>, diagram_type: &str) {
    for ends in [
        ["pointEnd", "pointStart"],
        ["circleEnd", "circleStart"],
        ["crossEnd", "crossStart"],
    ] {
        for margin in [false, true] {
            for base in ends {
                push_edge_marker(out, diagram_id, diagram_type, base, margin, None);
            }
        }
    }
    if diagram_type == "agentflow" {
        // Agentflow registers hierarchy markers after the ordinary edge marker set.  These are
        // emitted even when a fixture has no hierarchy edge because Mermaid's marker registry is
        // populated eagerly by the family renderer.
        push_edge_marker(out, diagram_id, diagram_type, "hierarchyEnd", false, None);
        push_edge_marker(out, diagram_id, diagram_type, "hierarchyStart", false, None);
    }
}

/// `color` carries the raw-token ID suffix and the sanitized SVG color value separately.
pub fn push_edge_marker(
    out: &mut String,
    diagram_id: SvgId<'_>,
    diagram_type: &str,
    base: &str,
    margin: bool,
    color: Option<(&str, Option<&str>)>,
) {
    let kind = match (base, margin) {
        ("pointEnd", false) => Some(BaseEdgeMarkerKind::PointEnd),
        ("pointStart", false) => Some(BaseEdgeMarkerKind::PointStart),
        ("pointEnd", true) => Some(BaseEdgeMarkerKind::PointEndMargin),
        ("pointStart", true) => Some(BaseEdgeMarkerKind::PointStartMargin),
        ("circleEnd", false) => Some(BaseEdgeMarkerKind::CircleEnd),
        ("circleStart", false) => Some(BaseEdgeMarkerKind::CircleStart),
        ("circleEnd", true) => Some(BaseEdgeMarkerKind::CircleEndMargin),
        ("circleStart", true) => Some(BaseEdgeMarkerKind::CircleStartMargin),
        ("crossEnd", false) => Some(BaseEdgeMarkerKind::CrossEnd),
        ("crossStart", false) => Some(BaseEdgeMarkerKind::CrossStart),
        ("crossEnd", true) => Some(BaseEdgeMarkerKind::CrossEndMargin),
        ("crossStart", true) => Some(BaseEdgeMarkerKind::CrossStartMargin),
        _ => None,
    };
    let geometry = if let Some(kind) = kind {
        kind.geometry()
    } else {
        match (base, margin) {
            ("hierarchyEnd", false) => HIERARCHY_END.0,
            ("hierarchyStart", false) => HIERARCHY_START.0,
            _ => return,
        }
    };
    let _ = write!(
        out,
        r#"<marker id="{}_{}-{}{}"#,
        diagram_id,
        escape_attr_display(diagram_type),
        base,
        if margin { "-margin" } else { "" }
    );
    if let Some((color_id, _)) = color {
        let _ = write!(out, "_{}", escape_attr_display(color_id));
    }
    let _ = write!(
        out,
        r#"" class="marker {}{}""#,
        if base.starts_with("cross") {
            "cross "
        } else if base.starts_with("hierarchy") {
            "hierarchy "
        } else {
            ""
        },
        escape_attr_display(diagram_type),
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
        r#" class="arrowMarkerPath" style="{}""#,
        geometry.style
    );
    // Mermaid clones only path/circle/line children; the Neo start-point polygon stays intact.
    if geometry.shape != "polygon"
        && let Some((_, Some(color))) = color
    {
        let _ = write!(out, r#" stroke="{}""#, escape_attr_display(color));
        if base.starts_with("point") {
            let _ = write!(out, r#" fill="{}""#, escape_attr_display(color));
        }
    }
    out.push_str("/></marker>");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_marker_writers_share_exact_geometry_and_dom_order() {
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let projection = crate::svg_css::SvgDiagramIdProjection::new(&meter);
        let id = SvgId::new("markers").unwrap().with_projection(&projection);
        for kind in BaseEdgeMarkerKind::ALL {
            let suffix = kind.suffix();
            let (base, margin) = suffix
                .strip_suffix("-margin")
                .map_or((suffix, false), |base| (base, true));
            let mut typed = String::new();
            let mut legacy = String::new();
            push_base_edge_marker(&mut typed, id, "block", kind, suffix, "");
            push_edge_marker(&mut legacy, id, "block", base, margin, None);
            assert_eq!(typed, legacy, "kind={kind:?}");
            let mut colored = String::new();
            push_edge_marker(
                &mut colored,
                id,
                "block",
                base,
                margin,
                Some(("red", Some("#ff0000"))),
            );
            if kind == BaseEdgeMarkerKind::PointStartMargin {
                assert!(!colored.contains(r##" stroke="#ff0000""##));
                assert!(!colored.contains(r##" fill="#ff0000""##));
            } else {
                assert!(colored.contains(r##" stroke="#ff0000""##));
            }
        }
        projection.finish().unwrap();
    }
}
