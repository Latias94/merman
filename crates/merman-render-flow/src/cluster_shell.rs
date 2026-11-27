//! Balanced container shells for Flowchart, Agentflow, and Swimlane.

use std::fmt::{self, Write as _};

use kurbo::{Point, Rect, Size, Vec2};
use merman_render_core::{
    mermaid_style::{ParsedStyleDeclaration, visit_parsed_style_declarations},
    number_format::canonical_number as number,
    resources::OperationWorkMeter,
    rough_svg::paths::{
        RoughGeometryTarget, roughjs_hachure_paths_for_rect, roughjs_hachure_paths_for_svg_path,
        roughjs_hand_drawn_stroke_path_for_svg_path, roughjs_prepare_hachure_paths_for_svg_path,
        roughjs_prepare_hand_drawn_stroke_path_for_svg_path,
    },
    svg_output::SvgOutput,
    xml::escape_attr_display,
};
use roughr::core::RoughRandomness;

mod flowchart;
mod geometry;
#[doc(hidden)]
pub mod swimlane;
pub use flowchart::{FlowchartClusterShape, FlowchartClusterSvg, FlowchartClusterTitle};
pub use geometry::{
    PaintedClusterFrame, SwimlaneTerminalGeometry, cluster_container_radius,
    swimlane_terminal_geometry,
};

const ROUGHNESS: f32 = 0.7;
const FILL_WEIGHT: f32 = 3.0;
const HACHURE_GAP: f32 = 1.5;

/// Resolved colors and admitted source declarations for a container shell.
#[derive(Clone, Copy)]
pub struct ClusterShellPaint<'a> {
    pub style: &'a str,
    pub fill: &'a str,
    pub stroke: &'a str,
}

/// The facade selects the form using the resolved theme, family, and route contract.
pub enum ClusterShellForm<'a> {
    Native {
        radius: f64,
        stroke_width: f64,
    },
    Agentflow {
        stroke: &'a str,
        canonical_fill: Option<(&'a str, f64)>,
    },
    Flowchart {
        inline_fill: bool,
    },
}

/// Authored stroke declarations and the operation's shared Rough state.
#[derive(Clone, Copy)]
pub struct ClusterHandDrawn<'a> {
    pub stroke_width: Option<&'a str>,
    pub stroke_dasharray: Option<&'a str>,
    pub solid_backing: bool,
    pub work_meter: &'a OperationWorkMeter,
    pub randomness: &'a RoughRandomness,
}

impl ClusterHandDrawn<'_> {
    fn stroke_width(&self) -> f32 {
        self.stroke_width
            .and_then(|raw| raw.trim_end_matches("px").trim().parse::<f32>().ok())
            .unwrap_or(1.3)
    }

    fn dash(&self) -> &str {
        self.stroke_dasharray.unwrap_or("0 0")
    }

    /// Advance the ordinary container's Rough state for a geometry-only query.
    /// The caller owns eligibility, frame resolution, and its surrounding Layout checkpoints.
    pub fn advance_cluster(&self, frame: PaintedClusterFrame, agentflow: bool) {
        let PaintedClusterFrame { origin, size } = frame;
        if agentflow {
            let path = rounded_rect_path_d(origin, size, 10.0);
            let _ = roughjs_prepare_hand_drawn_stroke_path_for_svg_path(
                RoughGeometryTarget::Query,
                &path,
                ROUGHNESS,
                self.work_meter,
                self.randomness,
            );
        } else {
            let path = rounded_rect_path_d(origin, size, 0.0);
            let _ = roughjs_prepare_hachure_paths_for_svg_path(
                RoughGeometryTarget::Query,
                &path,
                self.stroke_width(),
                self.dash().trim(),
                FILL_WEIGHT,
                HACHURE_GAP,
                ROUGHNESS,
                self.work_meter,
                self.randomness,
            );
        }
    }
}

/// Write the complete painted child of a Flowchart or Agentflow cluster.
/// The caller owns the outer group, title, and end-of-layer checkpoint.
fn write_cluster_shell(
    out: &mut impl SvgOutput,
    frame: PaintedClusterFrame,
    paint: ClusterShellPaint<'_>,
    form: ClusterShellForm<'_>,
    rough: Option<ClusterHandDrawn<'_>>,
) {
    let PaintedClusterFrame { origin, size } = frame;
    match form {
        ClusterShellForm::Native {
            radius,
            stroke_width,
        } => {
            let _ = write!(
                out,
                r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" style="fill:{};stroke:{};stroke-width:{}px"#,
                number(origin.x),
                number(origin.y),
                number(size.width),
                number(size.height),
                number(radius),
                escape_attr_display(paint.fill),
                escape_attr_display(paint.stroke),
                number(stroke_width),
            );
            if !paint.style.is_empty() {
                let _ = write!(out, ";{}", escape_attr_display(paint.style));
            }
            out.push_str(r#""/>"#);
        }
        ClusterShellForm::Agentflow {
            stroke,
            canonical_fill,
        } => write_agentflow_shell(out, origin, size, paint, stroke, canonical_fill, rough),
        ClusterShellForm::Flowchart { inline_fill } => {
            write_flowchart_shell(out, origin, size, paint, inline_fill, rough);
        }
    }
}

fn write_agentflow_shell(
    out: &mut impl SvgOutput,
    origin: Point,
    size: Size,
    paint: ClusterShellPaint<'_>,
    stroke: &str,
    canonical_fill: Option<(&str, f64)>,
    rough: Option<ClusterHandDrawn<'_>>,
) {
    if let Some(rough) = rough {
        let path = rounded_rect_path_d(origin, size, 10.0);
        if let Some(stroke_d) = roughjs_hand_drawn_stroke_path_for_svg_path(
            &path,
            ROUGHNESS,
            rough.work_meter,
            rough.randomness,
        ) {
            out.push_str("<g>");
            if rough.solid_backing {
                let _ = write!(
                    out,
                    r#"<rect class="merman-rough-backing" x="{}" y="{}" width="{}" height="{}" rx="10" style=""#,
                    number(origin.x),
                    number(origin.y),
                    number(size.width),
                    number(size.height),
                );
                if let Some((fill, opacity)) = canonical_fill {
                    let _ = write!(
                        out,
                        "fill:{};fill-opacity:{};",
                        escape_attr_display(fill),
                        number(opacity)
                    );
                } else {
                    let _ = write!(out, "fill:{};", escape_attr_display(paint.fill));
                }
                write_backing_style(out, paint.style);
            }
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="0.75" fill="none" stroke-dasharray="{}"/></g>"#,
                escape_attr_display(&stroke_d),
                escape_attr_display(stroke),
                escape_attr_display(rough.dash()),
            );
            return;
        }
    }
    let _ = write!(
        out,
        r#"<rect rx="10" ry="10" x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{}" stroke-width="0.75px""#,
        number(origin.x),
        number(origin.y),
        number(size.width),
        number(size.height),
        escape_attr_display(stroke),
    );
    if let Some((fill, opacity)) = canonical_fill {
        let _ = write!(
            out,
            r#" style="fill:{};fill-opacity:{}""#,
            escape_attr_display(fill),
            number(opacity),
        );
    }
    out.push_str("/>");
}

fn write_flowchart_shell(
    out: &mut impl SvgOutput,
    origin: Point,
    size: Size,
    paint: ClusterShellPaint<'_>,
    inline_fill: bool,
    rough: Option<ClusterHandDrawn<'_>>,
) {
    if let Some(rough) = rough {
        let stroke_width = rough.stroke_width();
        let stroke_dasharray = rough.dash().trim();
        let path = rounded_rect_path_d(origin, size, 0.0);
        if let Some((fill_d, stroke_d)) = roughjs_hachure_paths_for_svg_path(
            &path,
            stroke_width,
            stroke_dasharray,
            FILL_WEIGHT,
            HACHURE_GAP,
            ROUGHNESS,
            rough.work_meter,
            rough.randomness,
        ) {
            out.push_str("<g>");
            if rough.solid_backing {
                // The fill is the surface used by the contrast pass before emission.
                let _ = write!(
                    out,
                    r#"<rect class="merman-rough-backing" x="{}" y="{}" width="{}" height="{}" style="fill:{};"#,
                    number(origin.x),
                    number(origin.y),
                    number(size.width),
                    number(size.height),
                    escape_attr_display(paint.fill),
                );
                write_backing_style(out, paint.style);
            } else {
                let background_style = rough_background_style(paint.style);
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} />"#,
                    escape_attr_display(&fill_d),
                    escape_attr_display(paint.fill),
                    number(FILL_WEIGHT as f64),
                    rough_style_attr(&background_style),
                );
            }
            let border_style = rough_border_style(paint.style);
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"{} /></g>"#,
                escape_attr_display(&stroke_d),
                escape_attr_display(paint.stroke),
                number(stroke_width as f64),
                escape_attr_display(stroke_dasharray),
                rough_style_attr(&border_style),
            );
            return;
        }
    }

    out.push_str(r#"<rect style=""#);
    if inline_fill {
        // Inline fill keeps browser palette selectors and raster exports in agreement.
        let _ = write!(out, "fill:{};", escape_attr_display(paint.fill));
    }
    let _ = write!(
        out,
        r#"{}" x="{}" y="{}" width="{}" height="{}"/>"#,
        escape_attr_display(paint.style),
        number(origin.x),
        number(origin.y),
        number(size.width),
        number(size.height),
    );
}

/// Paint the body and title strip in that order inside the caller's Swimlane group.
fn write_swimlane_shell(
    out: &mut impl SvgOutput,
    [body, title]: [Rect; 2],
    translation: Vec2,
    group_origin: Point,
    is_pool: bool,
    paint: ClusterShellPaint<'_>,
    rough: Option<ClusterHandDrawn<'_>>,
) {
    let (body_class, title_class) = if is_pool {
        ("pool-body swimlane-body", "pool-title swimlane-title")
    } else {
        ("swimlane-body", "swimlane-title")
    };
    for (rect, class_name, filled) in [(body, body_class, false), (title, title_class, true)] {
        let origin = Point::new(
            rect.x0 + translation.x - group_origin.x,
            rect.y0 + translation.y - group_origin.y,
        );
        let size = Size::new(rect.width().max(0.0), rect.height().max(0.0));
        write_swimlane_rect(out, origin, size, class_name, paint, filled, rough);
    }
}

fn write_swimlane_rect(
    out: &mut impl SvgOutput,
    origin: Point,
    size: Size,
    class_name: &str,
    paint: ClusterShellPaint<'_>,
    filled: bool,
    rough: Option<ClusterHandDrawn<'_>>,
) {
    let fill = filled.then_some(paint.fill);
    if let Some(rough) = rough {
        let stroke_width = rough.stroke_width();
        let stroke_dasharray = rough.dash().trim();
        // RoughJS creates the outline before the fill. The unfilled body still generates both
        // paths to preserve the seeded outline and the next shape's Rough state.
        if let Some((fill_d, stroke_d)) = roughjs_hachure_paths_for_rect(
            origin.x,
            origin.y,
            size.width,
            size.height,
            stroke_width,
            stroke_dasharray,
            FILL_WEIGHT,
            HACHURE_GAP,
            ROUGHNESS,
            rough.work_meter,
            rough.randomness,
        ) {
            let _ = write!(out, r#"<g class="{}">"#, escape_attr_display(class_name));
            if rough.solid_backing {
                let _ = write!(
                    out,
                    r#"<rect class="{} merman-rough-backing" x="{}" y="{}" width="{}" height="{}" fill="{}" style=""#,
                    escape_attr_display(class_name),
                    number(origin.x),
                    number(origin.y),
                    number(size.width),
                    number(size.height),
                    escape_attr_display(fill.unwrap_or("none")),
                );
                write_backing_style(out, paint.style);
            } else if let Some(fill) = fill {
                let background_style = rough_background_style(paint.style);
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} />"#,
                    escape_attr_display(&fill_d),
                    escape_attr_display(fill),
                    number(FILL_WEIGHT as f64),
                    rough_style_attr(&background_style),
                );
            }
            let border_style = rough_border_style(paint.style);
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"{} /></g>"#,
                escape_attr_display(&stroke_d),
                escape_attr_display(paint.stroke),
                number(stroke_width as f64),
                escape_attr_display(stroke_dasharray),
                rough_style_attr(&border_style),
            );
            return;
        }
    }

    let _ = write!(
        out,
        r#"<rect class="{}" style="{}" x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}"/>"#,
        escape_attr_display(class_name),
        escape_attr_display(paint.style),
        number(origin.x),
        number(origin.y),
        number(size.width),
        number(size.height),
        escape_attr_display(fill.unwrap_or("none")),
        escape_attr_display(paint.stroke),
    );
}

fn write_backing_style(out: &mut impl SvgOutput, style: &str) {
    let style = style.trim_matches(|ch: char| ch == ';' || ch.is_ascii_whitespace());
    if !style.is_empty() {
        let _ = write!(out, "{};", escape_attr_display(style));
    }
    out.push_str(r#"stroke:none !important"/>"#);
}

fn rounded_rect_path_d(
    Point { x, y }: Point,
    Size {
        width: w,
        height: h,
    }: Size,
    r: f64,
) -> String {
    let mut out = String::new();
    let _ = write!(
        &mut out,
        "M {} {} H {} A {} {} 0 0 1 {} {} V {} A {} {} 0 0 1 {} {} H {} A {} {} 0 0 1 {} {} V {} A {} {} 0 0 1 {} {} Z",
        number(x + r),
        number(y),
        number(x + w - r),
        number(r),
        number(r),
        number(x + w),
        number(y + r),
        number(y + h - r),
        number(r),
        number(r),
        number(x + w - r),
        number(y + h),
        number(x + r),
        number(r),
        number(r),
        number(x),
        number(y + h - r),
        number(y + r),
        number(r),
        number(r),
        number(x + r),
        number(y),
    );
    out
}

fn rough_background_style(style: &str) -> String {
    rough_style_from_node_style(style, |declaration| {
        (declaration.property() == "fill").then_some("stroke")
    })
}

fn rough_border_style(style: &str) -> String {
    rough_style_from_node_style(style, |declaration| {
        declaration
            .property()
            .starts_with("stroke")
            .then(|| declaration.property_css())
    })
}

fn rough_style_from_node_style<'a>(
    style: &'a str,
    mut property: impl FnMut(&ParsedStyleDeclaration<'a>) -> Option<&'a str>,
) -> String {
    let mut out = String::new();
    visit_parsed_style_declarations(style, |declaration| {
        let Some(property) = property(&declaration) else {
            return;
        };
        if !out.is_empty() {
            out.push(';');
        }
        out.push_str(property);
        out.push(':');
        out.push_str(declaration.source_value());
    });
    out
}

fn rough_style_attr(style: &str) -> impl fmt::Display + '_ {
    fmt::from_fn(move |f| {
        let style = style.trim();
        if style.is_empty() {
            return Ok(());
        }
        write!(f, r#" style="{}""#, escape_attr_display(style))
    })
}

#[cfg(test)]
mod tests {
    use super::rough_background_style;

    #[test]
    fn rough_title_style_rewrites_only_the_property_and_keeps_quoted_css_intact() {
        let style = r#"fill:"R&amp;D;fill" !important;stroke:#123456"#;
        assert_eq!(
            rough_background_style(style),
            r#"stroke:"R&amp;D;fill" !important"#
        );
    }
}
