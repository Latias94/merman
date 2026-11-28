use std::{
    collections::HashMap,
    fmt::{self, Write},
};

use kurbo::{BezPath, Rect, Shape};
use merman_theme_contract::v2::{Elevation, ShadowLayer};

use super::canvas::write_color;
use crate::number_format::canonical_number as number;
#[cfg(feature = "paint-scene")]
use crate::paint_scene::{SceneBox, SceneColor, ScenePaint, SceneRect};

fn visible_layers(elevation: &Elevation) -> impl Iterator<Item = ShadowLayer> + '_ {
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
    stack.iter().copied().chain(glow).filter(visible)
}

fn visible(layer: &ShadowLayer) -> bool {
    layer.opacity > 0.0 && layer.color.channels()[3] != 0
}

fn blur_region(bounds: Rect, blur: f64) -> Rect {
    bounds.inflate(4.0 * blur, 4.0 * blur)
}

/// Bounds of the depth layer in the surface's position-independent coordinates.
pub fn bounds(elevation: &Elevation, surface: Rect) -> Rect {
    visible_layers(elevation).fold(surface, |bounds, layer| {
        bounds.union(blur_region(surface, layer.blur) + kurbo::Vec2::new(layer.dx, layer.dy))
    })
}

/// A filter-free shadow stack that can be painted with compact scene rectangles.
#[cfg(feature = "paint-scene")]
#[derive(Clone, Copy)]
pub struct HardShadows<'a>(&'a Elevation);

#[cfg(feature = "paint-scene")]
impl<'a> HardShadows<'a> {
    pub fn resolve(elevation: &'a Elevation) -> Option<Self> {
        match elevation {
            Elevation::Flat => Some(Self(elevation)),
            Elevation::Shadows { layers } if layers.iter().all(|layer| layer.blur == 0.0) => {
                Some(Self(elevation))
            }
            _ => None,
        }
    }

    pub fn layer_count(self) -> usize {
        visible_layers(self.0).count()
    }

    pub fn bounds(self, surface: SceneBox) -> Rect {
        bounds(
            self.0,
            Rect::from_origin_size((surface.x, surface.y), (surface.width, surface.height)),
        )
    }

    pub fn rectangles(
        self,
        surface: SceneBox,
        radius: f64,
    ) -> impl Iterator<Item = SceneRect> + 'a {
        visible_layers(self.0).map(move |layer| {
            let [red, green, blue, alpha] = layer.color.channels().map(f64::from);
            SceneRect {
                bounds: SceneBox {
                    x: surface.x + layer.dx,
                    y: surface.y + layer.dy,
                    ..surface
                },
                radius,
                paint: ScenePaint {
                    fill: Some(SceneColor {
                        red,
                        green,
                        blue,
                        alpha: alpha / 255.0 * layer.opacity,
                    }),
                    stroke: None,
                },
            }
        })
    }
}

/// Emits a label-free copy of a silhouette below the graph's connectors and foreground.
pub fn write_depth(
    out: &mut impl Write,
    elevation: &Elevation,
    id: &str,
    silhouette: &BezPath,
    center: (f64, f64),
) -> fmt::Result {
    write_surface_depth(out, elevation, id, silhouette, center, None)
}

/// Shares identical local blur regions across silhouettes in one SVG document.
#[derive(Default)]
pub struct DepthFilters(HashMap<[u64; 5], String>);

impl DepthFilters {
    pub fn write_depth(
        &mut self,
        out: &mut impl Write,
        elevation: &Elevation,
        id: &str,
        silhouette: &BezPath,
        center: (f64, f64),
    ) -> fmt::Result {
        write_surface_depth(out, elevation, id, silhouette, center, Some(self))
    }
}

fn write_surface_depth(
    out: &mut impl Write,
    elevation: &Elevation,
    id: &str,
    silhouette: &BezPath,
    center: (f64, f64),
    mut filters: Option<&mut DepthFilters>,
) -> fmt::Result {
    let bounds = silhouette.bounding_box();
    let path = silhouette.to_svg();
    let id = crate::xml::escape_attr(id);
    for (index, layer) in visible_layers(elevation).enumerate() {
        write!(
            out,
            r#"<g data-merman-theme-depth="node" aria-hidden="true" pointer-events="none" transform="translate({},{})">"#,
            number(center.0 + layer.dx),
            number(center.1 + layer.dy)
        )?;
        let unshared;
        let filter = if layer.blur > 0.0 {
            let region = blur_region(bounds, layer.blur);
            if let Some(cache) = filters.as_mut() {
                use std::collections::hash_map::Entry;
                let key = [
                    region.x0,
                    region.y0,
                    region.width(),
                    region.height(),
                    layer.blur,
                ]
                .map(f64::to_bits);
                Some(match cache.0.entry(key) {
                    Entry::Occupied(entry) => entry.into_mut().as_str(),
                    Entry::Vacant(entry) => {
                        let name = format!("{id}-merman-depth-{index}");
                        write_blur_filter(out, &name, region, layer.blur)?;
                        entry.insert(name).as_str()
                    }
                })
            } else {
                unshared = format!("{id}-merman-depth-{index}");
                write_blur_filter(out, &unshared, region, layer.blur)?;
                Some(unshared.as_str())
            }
        } else {
            None
        };
        write!(out, r#"<path d="{path}" stroke="none" "#)?;
        write_color(out, "fill", layer.color, layer.opacity)?;
        if let Some(filter) = filter {
            write!(out, r#" filter="url(#{filter})""#)?;
        }
        out.write_str("/></g>")?;
    }
    Ok(())
}

fn write_blur_filter(out: &mut impl Write, id: &str, region: Rect, blur: f64) -> fmt::Result {
    write!(
        out,
        r#"<defs><filter id="{id}" filterUnits="userSpaceOnUse" x="{}" y="{}" width="{}" height="{}" color-interpolation-filters="sRGB"><feGaussianBlur stdDeviation="{}"/></filter></defs>"#,
        number(region.x0),
        number(region.y0),
        number(region.width()),
        number(region.height()),
        number(blur)
    )
}
