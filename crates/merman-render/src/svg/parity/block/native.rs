use kurbo::{Affine, BezPath, Circle, Ellipse, Rect, Shape};
use merman_theme_contract::v2::{Elevation, MarkerStyle};

use super::super::*;
use crate::block::{BlockNativeStyle, BlockShapeBoundary, BlockShapeGeometry};
use crate::svg::BaseEdgeMarkerKind;
use crate::theme::{elevation_svg, marker_svg};

pub(super) fn write_markers(
    out: &mut impl SvgOutput,
    id: SvgDiagramId<'_>,
    style: MarkerStyle,
) -> Result<()> {
    let arrow = marker_svg::arrow_path(style);
    for kind in [
        BaseEdgeMarkerKind::PointEnd,
        BaseEdgeMarkerKind::PointStart,
        BaseEdgeMarkerKind::CircleEnd,
        BaseEdgeMarkerKind::CircleStart,
        BaseEdgeMarkerKind::CrossEnd,
        BaseEdgeMarkerKind::CrossStart,
    ] {
        let transform = match kind {
            BaseEdgeMarkerKind::PointStart => Some(("4.5", "translate(10 0) scale(-1 1)")),
            BaseEdgeMarkerKind::PointEnd => Some(("5", "translate(0 0)")),
            _ => None,
        };
        if let Some((path, (reference, transform))) = arrow.zip(transform) {
            let fill = if style == MarkerStyle::Open {
                "fill:none;"
            } else {
                ""
            };
            let suffix = kind.suffix();
            let _ = write!(
                out,
                r#"<marker id="{id}_block-{suffix}" class="marker block" viewBox="0 0 10 10" refX="{reference}" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="{path}" transform="{transform}" class="arrowMarkerPath" style="stroke-width:1;stroke-dasharray:none;stroke-linejoin:round;{fill}"/></marker>"#,
            );
        } else {
            let mut marker = String::new();
            let paint = if kind.cross() { "fill:none;" } else { "" };
            markers::push_base_edge_marker(&mut marker, id, "block", kind, kind.suffix(), paint);
            out.push_str(&marker);
        }
        out.checkpoint()?;
    }
    Ok(())
}

pub(super) fn cylinder_path(width: f64, body_height: f64, radius_x: f64, radius_y: f64) -> String {
    format!(
        "M{},{} a{},{} 0,0,0 {} 0 a{},{} 0,0,0 {} 0 l0,{} a{},{} 0,0,0 {} 0 l0,{}",
        fmt_display(0.0),
        fmt_display(radius_y),
        fmt_display(radius_x),
        fmt_display(radius_y),
        fmt_display(width),
        fmt_display(radius_x),
        fmt_display(radius_y),
        fmt_display(-width),
        fmt_display(body_height),
        fmt_display(radius_x),
        fmt_display(radius_y),
        fmt_display(width),
        fmt_display(-body_height),
    )
}

fn silhouette(boundary: &BlockShapeBoundary) -> Option<BezPath> {
    let path = match boundary {
        BlockShapeBoundary::Rectangle {
            width,
            height,
            radius,
            ..
        } => Rect::from_center_size((0.0, 0.0), (*width, *height))
            .to_rounded_rect(*radius)
            .to_path(0.05),
        BlockShapeBoundary::Circle { radius, .. }
        | BlockShapeBoundary::DoubleCircle {
            outer_radius: radius,
            ..
        } => Circle::new((0.0, 0.0), *radius).to_path(0.05),
        BlockShapeBoundary::Stadium { width, height } if width < height => {
            Ellipse::new((0.0, 0.0), (*width / 2.0, *height / 2.0), 0.0).to_path(0.05)
        }
        BlockShapeBoundary::Stadium { width, height } => {
            Rect::from_center_size((0.0, 0.0), (*width, *height))
                .to_rounded_rect(*height / 2.0)
                .to_path(0.05)
        }
        BlockShapeBoundary::Polygon {
            points,
            translation,
        } => {
            let (first, rest) = points.split_first()?;
            let mut path = BezPath::new();
            path.move_to((first.x, first.y));
            for point in rest {
                path.line_to((point.x, point.y));
            }
            path.close_path();
            Affine::translate((translation.x, translation.y)) * path
        }
        BlockShapeBoundary::Cylinder {
            width,
            body_height,
            radius_x,
            radius_y,
        } => {
            let mut path =
                BezPath::from_svg(&cylinder_path(*width, *body_height, *radius_x, *radius_y))
                    .ok()?;
            path.close_path();
            Affine::translate((-width / 2.0, -(body_height / 2.0 + radius_y))) * path
        }
    };
    Some(path)
}

pub(super) fn write_node_depth(
    out: &mut impl SvgOutput,
    native: &BlockNativeStyle,
    id: &str,
    geometry: &BlockShapeGeometry,
    options: &SvgExecution<'_>,
) -> Result<Option<Rect>> {
    let (min_x, min_y, max_x, max_y) = geometry.rendered_extents();
    if matches!(native.elevation, Elevation::Flat)
        || ![min_x, min_y, max_x, max_y].into_iter().all(f64::is_finite)
    {
        return Ok(None);
    }
    let Some(path) = silhouette(&geometry.boundary) else {
        return Ok(None);
    };
    let origin = (geometry.allocated.x, geometry.allocated.y);
    options
        .work_meter()
        .charge_emit_work(path.elements().len())?;
    let _ = elevation_svg::write_depth(out, &native.elevation, id, &path, origin);
    out.checkpoint()?;
    Ok(Some(
        elevation_svg::bounds(&native.elevation, path.bounding_box())
            + kurbo::Vec2::new(origin.0, origin.1),
    ))
}
