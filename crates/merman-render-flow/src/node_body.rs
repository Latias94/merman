//! Self-contained Mermaid Flowchart node bodies shared by the Flow SVG producers.

use std::{borrow::Cow, time::Duration};

use merman_render_core::{
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    rough_svg::paths::{
        RoughGeometryTarget, RoughHachure, RoughPathPair, prepare_hachure_svg_path,
        roughjs_paths_for_svg_path, roughjs_prepare_paths_for_svg_path,
    },
    svg_output::{SvgOutput, timing::RenderTiming},
    xml::{EscapeAttrDisplay, escape_attr_display},
};
use roughr::core::RoughRandomness;

mod basic;
mod cylinders;
mod datastore;
mod depth;
pub mod document_shapes;
mod label_container;
mod lean_trapezoids;
mod no_label;
mod outer_path;
mod rectangle;
mod rectangle_with_divider;
mod rough_paint;
mod shapes;
mod subroutine;

pub use basic::{
    advance_basic_rough_geometry, advance_hexagon_rough_geometry, advance_odd_rough_geometry,
    advance_stadium_rough_geometry, write_circle, write_diamond, write_double_circle,
    write_hexagon, write_odd, write_stadium,
};
pub use cylinders::CylinderBody;
pub use datastore::{advance_datastore, write_datastore};
pub use depth::{native_node_silhouette, write_native_node_depth};
pub use label_container::{advance_hourglass, write_hourglass, write_notched_rectangle};
pub use lean_trapezoids::{advance_lean_hand_drawn, write_lean};
pub use no_label::{NoLabelStyle, advance_no_label_rough_geometry, try_write_no_label};
pub use outer_path::{OuterPathBody, OuterPathTranslation};
pub use rectangle::{RectangleBody, RectangleStyle};
pub use rectangle_with_divider::RectangleWithDivider;
pub(crate) use rough_paint::circle_outline;
pub use rough_paint::{grouped_shape_child_style, write_backed_hachure, write_hand_drawn_hachure};
pub use shapes::{LayoutNodeBody, MeasuredNodeBody};
pub use subroutine::{advance_subroutine, write_subroutine};

/// Layout dimensions and the operation's existing RoughJS inputs.
pub struct NodeBodyGeometry<'a> {
    pub width: f64,
    pub height: f64,
    pub hand_drawn: bool,
    #[cfg(feature = "layout-shoal")]
    pub(crate) compact_rough: bool,
    pub stroke_width: f32,
    pub stroke_dasharray: &'a str,
    pub randomness: &'a RoughRandomness,
    pub work_meter: &'a OperationWorkMeter,
}

impl NodeBodyGeometry<'_> {
    fn hachure(&self, weight: f32, gap: f32, roughness: f32) -> RoughHachure {
        #[cfg(feature = "layout-shoal")]
        if self.compact_rough {
            // Keep the texture legible over the solid backing.
            return RoughHachure::backed(weight, gap.max(2.8), roughness);
        }
        RoughHachure::mermaid(weight, gap, roughness)
    }

    pub(crate) fn write_rough_paths(
        &self,
        paths: merman_render_core::rough_svg::paths::RoughPathPair,
    ) -> Option<(String, String)> {
        #[cfg(feature = "layout-shoal")]
        if self.compact_rough {
            return paths.into_compact_svg_paths(self.work_meter);
        }
        paths.into_svg_paths(self.work_meter)
    }

    fn rough_circle(
        &self,
        target: RoughGeometryTarget,
        diameter: f64,
        stroke_width: f32,
        roughness: f32,
    ) -> Option<RoughPathPair> {
        use merman_render_core::rough_svg::paths::roughjs_prepare_paths_for_circle_with_roughness;

        #[cfg(feature = "layout-shoal")]
        if self.compact_rough {
            return merman_render_core::rough_svg::paths::prepare_backed_circle(
                target,
                diameter,
                stroke_width,
                self.stroke_dasharray,
                roughness,
                self.work_meter,
                self.randomness,
            );
        }
        roughjs_prepare_paths_for_circle_with_roughness(
            target,
            diameter,
            stroke_width,
            self.stroke_dasharray,
            true,
            roughness,
            self.work_meter,
            self.randomness,
        )
    }
}

/// Resolved body paint and the caller's shape timing counters.
pub struct NodeBodySvg<'a> {
    pub geometry: NodeBodyGeometry<'a>,
    pub style: &'a str,
    pub rough_group_style: &'a str,
    /// Canonical body fill opacity before group compositing, also used for Rough backing.
    pub rough_fill_opacity: Option<f64>,
    pub fill_color: &'a str,
    pub stroke_color: &'a str,
    pub timing: RenderTiming,
    pub rough_time: &'a mut Duration,
    pub rough_calls: &'a mut u32,
}

impl<'a> NodeBodySvg<'a> {
    fn timed_rough<T>(&mut self, generate: impl FnOnce(&NodeBodyGeometry<'a>) -> T) -> T {
        if let Some(start) = self.timing.start() {
            *self.rough_calls += 1;
            let result = generate(&self.geometry);
            *self.rough_time += start.elapsed();
            result
        } else {
            generate(&self.geometry)
        }
    }

    fn hand_drawn_path_pair(&mut self, path: &str) -> Option<(String, String)> {
        if !self.geometry.hand_drawn {
            return None;
        }
        self.timed_rough(|geometry| {
            prepare_node_path(geometry, path, RoughGeometryTarget::Svg)
                .and_then(|paths| geometry.write_rough_paths(paths))
        })
        .filter(|(fill, stroke)| !fill.is_empty() && !stroke.is_empty())
    }

    fn solid_path_pair(&mut self, path: &str) -> Option<(String, String)> {
        self.timed_rough(|geometry| {
            roughjs_paths_for_svg_path(
                path,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.work_meter,
                geometry.randomness,
            )
        })
    }

    fn hand_drawn_group_style(&self) -> &'a str {
        if self.rough_fill_opacity.is_some() {
            self.style
        } else {
            self.rough_group_style
        }
    }

    fn grouped_child_style(&self) -> Cow<'a, str> {
        if self.rough_fill_opacity.is_some() {
            grouped_shape_child_style(self.style)
        } else {
            Cow::Borrowed(self.style)
        }
    }

    fn write_hachure(&self, out: &mut impl SvgOutput, outline: &str, hatch: &str, width: f64) {
        write_hand_drawn_hachure(
            out,
            outline,
            hatch,
            width,
            (self.fill_color, self.stroke_color),
            self.rough_fill_opacity,
        );
    }

    fn write_hand_drawn_path_pair(
        &self,
        out: &mut impl SvgOutput,
        outline: &str,
        hatch: &str,
        stroke: &str,
        hatch_width: f64,
    ) {
        self.write_hachure(out, outline, hatch, hatch_width);
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
            escape_attr(stroke),
            escape_attr(self.stroke_color),
            fmt(f64::from(self.geometry.stroke_width)),
            escape_attr(self.geometry.stroke_dasharray),
        );
    }
}

fn rectangle_backing(body: &NodeBodySvg<'_>, width: f64, height: f64) -> String {
    use kurbo::Shape as _;

    if body.rough_fill_opacity.is_some() {
        kurbo::Rect::from_center_size((0.0, 0.0), (width, height))
            .to_path(0.05)
            .to_svg()
    } else {
        String::new()
    }
}

fn circle_backing(body: &NodeBodySvg<'_>, radius: f64) -> String {
    if body.rough_fill_opacity.is_some() {
        circle_outline(radius)
    } else {
        String::new()
    }
}

pub(crate) fn prepare_node_path(
    geometry: &NodeBodyGeometry<'_>,
    path: &str,
    target: RoughGeometryTarget,
) -> Option<RoughPathPair> {
    if geometry.hand_drawn {
        #[cfg(feature = "layout-shoal")]
        let hachure = RoughHachure::flowchart_path(geometry.compact_rough);
        #[cfg(not(feature = "layout-shoal"))]
        let hachure = RoughHachure::flowchart_path(false);
        prepare_hachure_svg_path(
            target,
            path,
            geometry.stroke_width,
            geometry.stroke_dasharray,
            hachure,
            geometry.work_meter,
            geometry.randomness,
        )
    } else {
        roughjs_prepare_paths_for_svg_path(
            target,
            path,
            geometry.stroke_width,
            geometry.stroke_dasharray,
            geometry.work_meter,
            geometry.randomness,
        )
    }
}

fn write_raw_filled_stroked_path(out: &mut impl SvgOutput, path: &str, body: &NodeBodySvg<'_>) {
    let _ = write!(
        out,
        r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{}" style="{}"/>"#,
        escape_attr(path),
        escape_attr(body.fill_color),
        escape_attr(body.stroke_color),
        fmt(body.geometry.stroke_width as f64),
        escape_attr(body.style),
    );
}

fn escape_attr(text: &str) -> EscapeAttrDisplay<&str> {
    escape_attr_display(text)
}

struct OptionalStyleAttr<'a>(&'a str);

impl std::fmt::Display for OptionalStyleAttr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.trim().is_empty() {
            return Ok(());
        }
        write!(f, r#" style="{}""#, escape_attr_display(self.0))
    }
}
