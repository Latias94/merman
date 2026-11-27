//! Self-contained Mermaid Flowchart node bodies shared by the Flow SVG producers.

use std::time::Duration;

use merman_render_core::{
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    rough_svg::paths::{
        RoughGeometryTarget, RoughPathPair, roughjs_paths_for_svg_path,
        roughjs_prepare_paths_for_hand_drawn_svg_path, roughjs_prepare_paths_for_svg_path,
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
pub use shapes::{LayoutNodeBody, MeasuredNodeBody};
pub use subroutine::{advance_subroutine, write_subroutine};

/// Layout dimensions and the operation's existing RoughJS inputs.
pub struct NodeBodyGeometry<'a> {
    pub width: f64,
    pub height: f64,
    pub hand_drawn: bool,
    pub stroke_width: f32,
    pub stroke_dasharray: &'a str,
    pub randomness: &'a RoughRandomness,
    pub work_meter: &'a OperationWorkMeter,
}

/// Resolved body paint and the caller's shape timing counters.
pub struct NodeBodySvg<'a> {
    pub geometry: NodeBodyGeometry<'a>,
    pub style: &'a str,
    pub rough_group_style: &'a str,
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
                .and_then(|paths| paths.into_svg_paths(geometry.work_meter))
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
}

fn prepare_node_path(
    geometry: &NodeBodyGeometry<'_>,
    path: &str,
    target: RoughGeometryTarget,
) -> Option<RoughPathPair> {
    if geometry.hand_drawn {
        roughjs_prepare_paths_for_hand_drawn_svg_path(
            target,
            path,
            geometry.stroke_width,
            geometry.stroke_dasharray,
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
