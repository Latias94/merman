use std::fmt::{self, Write};

use kurbo::{BezPath, Rect, Shape};
use merman_theme_contract::v2::{Elevation, ShadowLayer};

use super::canvas_svg::write_color;
use crate::number_format::canonical_number as number;

fn layers(elevation: &Elevation) -> impl Iterator<Item = ShadowLayer> + '_ {
    let (stack, glow) = match elevation {
        Elevation::Flat => (&[][..], None),
        Elevation::Shadows { layers } => (layers.as_ref(), None),
        Elevation::Glow {
            color,
            blur,
            opacity,
        } => (
            &[][..],
            Some(ShadowLayer::new(0.0, 0.0, *blur, *color, *opacity)),
        ),
    };
    stack.iter().copied().chain(glow)
}

fn visible(layer: &ShadowLayer) -> bool {
    layer.opacity > 0.0 && layer.color.channels()[3] != 0
}

fn blur_region(bounds: Rect, blur: f64) -> Rect {
    bounds.inflate(4.0 * blur, 4.0 * blur)
}

/// Bounds of the depth layer in the surface's position-independent coordinates.
pub(crate) fn bounds(elevation: &Elevation, surface: Rect) -> Rect {
    layers(elevation)
        .filter(visible)
        .fold(surface, |bounds, layer| {
            bounds.union(blur_region(surface, layer.blur) + kurbo::Vec2::new(layer.dx, layer.dy))
        })
}

/// Emits a label-free copy of a silhouette below the graph's connectors and foreground.
pub(crate) fn write_depth(
    out: &mut impl Write,
    elevation: &Elevation,
    id: &str,
    silhouette: &BezPath,
    center: (f64, f64),
) -> fmt::Result {
    let bounds = silhouette.bounding_box();
    let path = silhouette.to_svg();
    let id = crate::svg::escape_attr(id);
    for (index, layer) in layers(elevation).filter(visible).enumerate() {
        let filter = format!("{id}-merman-depth-{index}");
        write!(
            out,
            r#"<g data-merman-theme-depth="node" aria-hidden="true" pointer-events="none" transform="translate({},{})">"#,
            number(center.0 + layer.dx),
            number(center.1 + layer.dy)
        )?;
        if layer.blur > 0.0 {
            let region = blur_region(bounds, layer.blur);
            write!(
                out,
                r#"<defs><filter id="{filter}" filterUnits="userSpaceOnUse" x="{}" y="{}" width="{}" height="{}" color-interpolation-filters="sRGB"><feGaussianBlur stdDeviation="{}"/></filter></defs>"#,
                number(region.x0),
                number(region.y0),
                number(region.width()),
                number(region.height()),
                number(layer.blur)
            )?;
        }
        write!(out, r#"<path d="{path}" stroke="none" "#)?;
        write_color(out, "fill", layer.color, layer.opacity)?;
        if layer.blur > 0.0 {
            write!(out, r#" filter="url(#{filter})""#)?;
        }
        out.write_str("/></g>")?;
    }
    Ok(())
}
