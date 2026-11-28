//! Body paths for the vertical, tilted, and lined Flowchart cylinders.

use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::paths::roughjs_hand_drawn_stroke_path_for_svg_path, svg_output::SvgOutput,
    xml::escape_attr,
};

use super::NodeBodySvg;
use crate::document::FlowchartDocumentId;

#[derive(Clone, Copy)]
enum CylinderShape {
    Vertical,
    Horizontal,
    Lined,
}

/// Prepared cylinder body geometry. Label measurement and emission belong to the caller.
pub struct CylinderBody {
    shape: CylinderShape,
    w: f64,
    h: f64,
    rx: f64,
    ry: f64,
}

impl CylinderBody {
    pub fn vertical(width: f64, height: f64) -> Self {
        Self::from_layout(CylinderShape::Vertical, width, height, 1.0)
    }

    /// Prepares the lined cylinder and returns its downward label offset.
    pub fn lined(width: f64, height: f64) -> (Self, f64) {
        let body = Self::from_layout(CylinderShape::Lined, width, height, 0.0);
        let label_dy = body.ry;
        (body, label_dy)
    }

    /// Prepares the tilted cylinder from the measured label and returns its horizontal offset.
    pub fn horizontal(
        label_width: f64,
        label_height: f64,
        node_padding: f64,
        neo: bool,
    ) -> (Self, f64) {
        let label_padding = if neo { 12.0 } else { node_padding / 2.0 };
        let h = (label_height + label_padding).max(1.0);
        let ry = h / 2.0;
        let rx = ry / (2.5 + h / 50.0);
        let w = (label_width + rx + label_padding).max(1.0);
        (
            Self {
                shape: CylinderShape::Horizontal,
                w,
                h,
                rx,
                ry,
            },
            -rx,
        )
    }

    fn from_layout(shape: CylinderShape, width: f64, height: f64, min_body_height: f64) -> Self {
        let w = width.max(1.0);
        let rx = w / 2.0;
        let ry = rx / (2.5 + w / 50.0);
        let h = (height.max(1.0) - 2.0 * ry).max(min_body_height);
        Self {
            shape,
            w,
            h,
            rx,
            ry,
        }
    }

    /// Writes only the balanced shape body using the existing sink and RoughJS state.
    pub fn write(
        self,
        out: &mut impl SvgOutput,
        mut svg: NodeBodySvg<'_>,
        canonical_node_id: Option<FlowchartDocumentId<'_>>,
        security_level_loose: bool,
    ) {
        let translation = match self.shape {
            CylinderShape::Horizontal => (-self.w / 2.0, self.h / 2.0),
            CylinderShape::Vertical | CylinderShape::Lined => {
                (-self.w / 2.0, -(self.h / 2.0 + self.ry))
            }
        };
        let label_offset = match self.shape {
            CylinderShape::Horizontal => None,
            CylinderShape::Vertical | CylinderShape::Lined => {
                security_level_loose.then_some(self.ry)
            }
        };
        if svg.geometry.hand_drawn
            && let Some(id) = canonical_node_id
        {
            let (body, rims) = self.rough_source_paths();
            write_hand_drawn_cylinder(out, &mut svg, [&body, &rims], translation, label_offset, id);
            return;
        }

        let path_data = self.classic_path();
        let _ = write!(
            out,
            r#"<path d="{}" class="basic label-container outer-path" style="{}""#,
            escape_attr(&path_data),
            escape_attr(svg.style),
        );
        if svg.geometry.hand_drawn {
            let _ = write!(
                out,
                r#" fill="{}" stroke="{}" stroke-width="{}""#,
                escape_attr(svg.fill_color),
                escape_attr(svg.stroke_color),
                fmt(svg.geometry.stroke_width as f64),
            );
        }
        if let Some(offset) = label_offset {
            let _ = write!(out, r#" label-offset-y="{}""#, fmt(offset));
        }
        let _ = write!(
            out,
            r#" transform="translate({},{})"/>"#,
            fmt(translation.0),
            fmt(translation.1),
        );
    }

    fn classic_path(&self) -> String {
        let Self {
            shape,
            w,
            h,
            rx,
            ry,
        } = *self;
        match shape {
            CylinderShape::Vertical => format!(
                "M0,{ry} a{rx},{ry} 0,0,0 {w},0 a{rx},{ry} 0,0,0 {mw},0 l0,{h} a{rx},{ry} 0,0,0 {w},0 l0,{mh}",
                ry = fmt(ry),
                rx = fmt(rx),
                w = fmt(w),
                mw = fmt(-w),
                h = fmt(h),
                mh = fmt(-h),
            ),
            CylinderShape::Horizontal => format!(
                "M0,0 a{rx},{ry} 0,0,1 0,{neg_h} l{w},0 a{rx},{ry} 0,0,1 0,{h} M{w},{neg_h} a{rx},{ry} 0,0,0 0,{h} l{neg_w},0",
                rx = fmt(rx),
                ry = fmt(ry),
                neg_h = fmt(-h),
                w = fmt(w),
                h = fmt(h),
                neg_w = fmt(-w),
            ),
            CylinderShape::Lined => format!(
                "M0,{ry} a{rx},{ry} 0,0,0 {w},0 a{rx},{ry} 0,0,0 -{w},0 l0,{h} a{rx},{ry} 0,0,0 {w},0 l0,-{h} M0,{y2} a{rx},{ry} 0,0,0 {w},0",
                ry = fmt(ry),
                rx = fmt(rx),
                w = fmt(w),
                h = fmt(h),
                y2 = fmt(ry + h * 0.1),
            ),
        }
    }

    fn rough_source_paths(&self) -> (String, String) {
        let Self {
            shape,
            w,
            h,
            rx,
            ry,
        } = *self;
        if matches!(shape, CylinderShape::Horizontal) {
            let rim = format!(
                "M{},{} a{},{} 0,0,0 0,{}",
                fmt(w),
                fmt(-h),
                fmt(rx),
                fmt(ry),
                fmt(h)
            );
            let body = format!(
                "M0,0 a{rx},{ry} 0,0,1 0,{mh} h{w} a{rx},{ry} 0,0,1 0,{h} Z",
                rx = fmt(rx),
                ry = fmt(ry),
                w = fmt(w),
                h = fmt(h),
                mh = fmt(-h),
            );
            return (body, rim);
        }

        let mut rims = self.vertical_rim(ry);
        if matches!(shape, CylinderShape::Lined) {
            rims.push(' ');
            rims.push_str(&self.vertical_rim(ry + h * 0.1));
        }
        let body = format!(
            "M0,{ry} a{rx},{ry} 0,0,1 {w},0 v{h} a{rx},{ry} 0,0,1 {mw},0 Z",
            rx = fmt(rx),
            ry = fmt(ry),
            w = fmt(w),
            h = fmt(h),
            mw = fmt(-w),
        );
        (body, rims)
    }

    fn vertical_rim(&self, y: f64) -> String {
        format!(
            "M0,{} a{},{} 0,0,0 {},0",
            fmt(y),
            fmt(self.rx),
            fmt(self.ry),
            fmt(self.w)
        )
    }
}

fn write_hand_drawn_cylinder(
    out: &mut impl SvgOutput,
    svg: &mut NodeBodySvg<'_>,
    [body, rims]: [&str; 2],
    (tx, ty): (f64, f64),
    label_offset: Option<f64>,
    id: FlowchartDocumentId<'_>,
) {
    let _ = write!(
        out,
        r#"<g class="basic label-container outer-path" transform="translate({},{})" style="{}""#,
        fmt(tx),
        fmt(ty),
        escape_attr(svg.hand_drawn_group_style()),
    );
    if let Some(offset) = label_offset {
        let _ = write!(out, r#" label-offset-y="{}""#, fmt(offset));
    }
    out.push('>');
    let sw = fmt(svg.geometry.stroke_width as f64);
    if let Some((fill, stroke)) = svg.hand_drawn_path_pair(body) {
        if let Some(fill_opacity) = svg.rough_fill_opacity {
            // The rough hatch extends past its sampled polygon. Clip the fill at the actual
            // cylinder silhouette and keep the rough outline and open rim outside the clip.
            let _ = write!(
                out,
                r#"<defs><clipPath id="{id}-rough-fill"><path d="{}"/></clipPath></defs><g clip-path="url(#{id}-rough-fill)">"#,
                escape_attr(body),
            );
            super::write_backed_hachure(
                out,
                body,
                &fill,
                4.0,
                (svg.fill_color, svg.stroke_color),
                fill_opacity,
            );
            out.push_str("</g>");
        } else {
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0" style="fill:none !important;stroke:{} !important;stroke-width:4px !important"/>"#,
                escape_attr(&fill),
                escape_attr(svg.fill_color),
                escape_attr(svg.fill_color),
            );
        }
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="fill:none !important;stroke:{} !important;stroke-width:{}px !important"/>"#,
            escape_attr(&stroke),
            escape_attr(svg.stroke_color),
            sw,
            escape_attr(svg.geometry.stroke_dasharray),
            escape_attr(svg.stroke_color),
            sw,
        );
    } else {
        let _ = write!(
            out,
            r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{}" stroke-dasharray="{}" style="fill:{} !important;stroke:{} !important;stroke-width:{}px !important"/>"#,
            escape_attr(body),
            escape_attr(svg.fill_color),
            escape_attr(svg.stroke_color),
            sw,
            escape_attr(svg.geometry.stroke_dasharray),
            escape_attr(svg.fill_color),
            escape_attr(svg.stroke_color),
            sw,
        );
    }
    // The open rims use the Rough stroke generator so they do not fill each cap.
    let rough_rims = svg.timed_rough(|geometry| {
        roughjs_hand_drawn_stroke_path_for_svg_path(
            rims,
            0.5,
            geometry.work_meter,
            geometry.randomness,
        )
    });
    let rim_d = rough_rims.as_deref().unwrap_or(rims);
    let _ = write!(
        out,
        r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-dasharray="{}" style="fill:none !important;stroke:{} !important;stroke-width:{}px !important"/></g>"#,
        escape_attr(rim_d),
        escape_attr(svg.stroke_color),
        sw,
        escape_attr(svg.geometry.stroke_dasharray),
        escape_attr(svg.stroke_color),
        sw,
    );
}
