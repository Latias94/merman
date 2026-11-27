//! Child SVG for the Flowchart divided rectangle and shaded process.

use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::{
        closed_path_d_from_points,
        paths::{
            RoughGeometryTarget, roughjs_paths_for_polygon, roughjs_prepare_paths_for_polygon,
        },
    },
    svg_output::SvgOutput,
};

use super::{
    NodeBodyGeometry, NodeBodySvg, escape_attr, prepare_node_path, write_raw_filled_stroked_path,
};

/// The divided rectangle and shaded process, both drawn from the layout bounds.
#[derive(Clone, Copy)]
pub enum RectangleWithDivider {
    Divided { authored_padding: f64, neo: bool },
    Shaded,
}

impl RectangleWithDivider {
    fn prepare(
        self,
        geometry: &NodeBodyGeometry<'_>,
        label_dx: &mut f64,
        label_dy: &mut f64,
    ) -> ([(f64, f64); 7], String) {
        let out_w = geometry.width.max(1.0);
        let out_h = geometry.height.max(1.0);
        let points = match self {
            Self::Divided {
                authored_padding,
                neo,
            } => {
                // Mermaid's rendered bbox is 1.2 times the polygon's base height.
                let h = out_h / 1.2;
                let rect_offset = h * 0.2;
                let x = -out_w / 2.0;
                let y = -h / 2.0 - rect_offset / 2.0;

                // The label transform retains the authored padding when Neo uses fixed padding.
                let shape_padding = if neo { 16.0 } else { authored_padding };
                let padding_delta = (authored_padding - shape_padding) / 2.0;
                *label_dx = padding_delta;
                *label_dy = padding_delta + rect_offset / 2.0;

                [
                    (x, y + rect_offset),
                    (-x, y + rect_offset),
                    (-x, -y),
                    (x, -y),
                    (x, y),
                    (-x, y),
                    (-x, y + rect_offset),
                ]
            }
            Self::Shaded => {
                // The outer bbox includes an extra 8px on each side of Mermaid's base width.
                let w = (out_w - 16.0).max(1.0);
                let x = -out_w / 2.0 + 8.0;
                let y = -out_h / 2.0;
                *label_dx = 4.0;

                [
                    (x, y),
                    (x + w + 8.0, y),
                    (x + w + 8.0, y + out_h),
                    (x - 8.0, y + out_h),
                    (x - 8.0, y),
                    (x, y),
                    (x, y + out_h),
                ]
            }
        };
        let path = closed_path_d_from_points(&points);
        (points, path)
    }

    /// Writes the complete body and updates the existing label's placement.
    pub fn write(
        self,
        out: &mut impl SvgOutput,
        mut body: NodeBodySvg<'_>,
        label_dx: &mut f64,
        label_dy: &mut f64,
    ) {
        let (points, path) = self.prepare(&body.geometry, label_dx, label_dy);
        if let Some((fill_d, stroke_d)) = body.hand_drawn_path_pair(&path) {
            let _ = write!(
                out,
                r#"<g class="basic label-container outer-path" style="{}">"#,
                escape_attr(body.hand_drawn_group_style()),
            );
            body.write_hand_drawn_path_pair(out, &path, &fill_d, &stroke_d, 4.0);
            out.push_str("</g>");
            return;
        }
        if body.geometry.hand_drawn {
            match self {
                Self::Divided { .. } => {
                    let _ = write!(
                        out,
                        r#"<path d="{}" class="basic label-container outer-path" style="{}"/>"#,
                        escape_attr(&path),
                        escape_attr(body.style),
                    );
                }
                Self::Shaded => {
                    let _ = write!(out, r#"<g class="basic label-container outer-path">"#);
                    write_raw_filled_stroked_path(out, &path, &body);
                    out.push_str("</g>");
                }
            }
            return;
        }

        let (fill_d, stroke_d) = body
            .timed_rough(|geometry| {
                roughjs_paths_for_polygon(
                    &points,
                    geometry.stroke_width,
                    geometry.work_meter,
                    geometry.randomness,
                )
            })
            .unwrap_or_else(|| (path.clone(), path));

        // Join the literal at compile time to retain the original sink append boundaries.
        macro_rules! write_polygon_group {
            ($group_style:literal) => {
                write!(
                    out,
                    concat!(
                        r#"<g class="basic label-container outer-path""#,
                        $group_style,
                        r#"><path d="{}" stroke="none" stroke-width="0" fill="{}" fill-rule="evenodd" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g>"#,
                    ),
                    escape_attr(&fill_d),
                    escape_attr(body.fill_color),
                    escape_attr(body.style),
                    escape_attr(&stroke_d),
                    escape_attr(body.stroke_color),
                    fmt(body.geometry.stroke_width as f64),
                    escape_attr(body.geometry.stroke_dasharray),
                    escape_attr(body.style),
                )
            };
        }
        let _ = match self {
            Self::Divided { .. } => write_polygon_group!(""),
            Self::Shaded => write_polygon_group!(r#" style="""#),
        };
    }

    /// Advances the layout-phase Rough stream and updates the existing label's placement.
    pub fn advance_rough_geometry(
        self,
        geometry: NodeBodyGeometry<'_>,
        label_dx: &mut f64,
        label_dy: &mut f64,
    ) {
        let (points, path) = self.prepare(&geometry, label_dx, label_dy);
        let target = RoughGeometryTarget::Query;
        let _ = if geometry.hand_drawn {
            prepare_node_path(&geometry, &path, target)
        } else {
            roughjs_prepare_paths_for_polygon(
                target,
                &points,
                geometry.stroke_width,
                geometry.work_meter,
                geometry.randomness,
            )
        };
    }
}
