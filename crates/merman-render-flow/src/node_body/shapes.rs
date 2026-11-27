//! Terminal dispatch for bodies driven by a layout rectangle or an initial label measurement.

use merman_render_core::{svg_output::SvgOutput, text::TextMetrics};

use super::{
    NodeBodyGeometry, NodeBodySvg, RectangleWithDivider, advance_datastore, advance_hourglass,
    advance_subroutine, escape_attr, write_datastore, write_hourglass, write_notched_rectangle,
    write_subroutine,
};

mod browser;
mod bucket;
mod console;
mod folder;
mod note;
mod organic;
mod paper_tape;
mod person;
mod stacked_rectangle;
mod tag_rect;
mod window_pane;

/// A complete body whose inputs come from layout and resolved styling.
pub enum LayoutNodeBody<'a> {
    Datastore,
    Divider(RectangleWithDivider),
    Hourglass,
    NotchedRectangle,
    Note {
        fill: &'a str,
        stroke: &'a str,
    },
    Organic {
        cloud: bool,
        label_size: kurbo::Size,
        padding: f64,
        html_labels: bool,
    },
    StackedRectangle {
        neo: bool,
    },
    Subroutine,
    Text,
    WindowPane,
}

impl LayoutNodeBody<'_> {
    pub fn write(
        self,
        out: &mut impl SvgOutput,
        svg: NodeBodySvg<'_>,
        label_dx: &mut f64,
        label_dy: &mut f64,
    ) {
        match self {
            Self::Datastore => write_datastore(out, svg),
            Self::Divider(divider) => divider.write(out, svg, label_dx, label_dy),
            Self::Hourglass => write_hourglass(out, svg),
            Self::NotchedRectangle => {
                write_notched_rectangle(out, svg.geometry.width, svg.geometry.height, svg.style);
            }
            Self::Note { fill, stroke } => note::write(out, svg, fill, stroke),
            Self::Organic {
                cloud,
                label_size,
                padding,
                html_labels,
            } => organic::write(out, svg, cloud, label_size, padding, html_labels, label_dx),
            Self::StackedRectangle { neo } => {
                stacked_rectangle::write(out, svg, neo, label_dx, label_dy);
            }
            Self::Subroutine => write_subroutine(out, svg),
            Self::Text => {
                use merman_render_core::number_format::canonical_number as fmt;

                let w = svg.geometry.width.max(0.0);
                let h = svg.geometry.height.max(0.0);
                let _ = write!(
                    out,
                    r#"<rect class="text" style="{}" rx="0" ry="0" x="{}" y="{}" width="{}" height="{}"/>"#,
                    escape_attr(svg.style),
                    fmt(-w / 2.0),
                    fmt(-h / 2.0),
                    fmt(w),
                    fmt(h)
                );
            }
            Self::WindowPane => window_pane::write(out, svg, label_dx, label_dy),
        }
    }

    pub fn advance_rough_geometry(
        self,
        geometry: NodeBodyGeometry<'_>,
        label_dx: &mut f64,
        label_dy: &mut f64,
    ) {
        match self {
            Self::Datastore => advance_datastore(geometry),
            Self::Divider(divider) => divider.advance_rough_geometry(geometry, label_dx, label_dy),
            Self::Hourglass => advance_hourglass(geometry),
            Self::NotchedRectangle | Self::Text => {}
            Self::Note { .. } => note::advance_rough_geometry(geometry),
            Self::Organic {
                cloud,
                label_size,
                padding,
                ..
            } => organic::advance_rough_geometry(geometry, cloud, label_size, padding),
            Self::StackedRectangle { neo } => {
                stacked_rectangle::advance_rough_geometry(geometry, neo, label_dx, label_dy);
            }
            Self::Subroutine => advance_subroutine(geometry),
            Self::WindowPane => window_pane::advance_rough_geometry(geometry, label_dx, label_dy),
        }
    }
}

/// A body sized from the original label measurement before any of its SVG is written.
pub enum MeasuredNodeBody<'a> {
    Browser { accent: &'a str },
    Bucket { accent: &'a str },
    Console { accent: &'a str },
    Folder,
    PaperTape { neo: bool },
    Person,
    TaggedRectangle { neo: bool },
}

impl MeasuredNodeBody<'_> {
    /// Selects the shapes whose query needs the caller's label measurement in this look.
    pub fn queries_rough(&self, hand_drawn: bool) -> bool {
        hand_drawn || matches!(self, Self::PaperTape { .. } | Self::TaggedRectangle { .. })
    }

    pub fn write(
        self,
        out: &mut impl SvgOutput,
        svg: NodeBodySvg<'_>,
        metrics: TextMetrics,
        padding: f64,
        label_dy: &mut f64,
    ) {
        match self {
            Self::Browser { accent } => {
                *label_dy = browser::render_browser(out, svg, metrics, padding, accent);
            }
            Self::Bucket { accent } => {
                *label_dy = bucket::render_bucket(out, svg, metrics, padding, accent);
            }
            Self::Console { accent } => {
                *label_dy = console::render_console(out, svg, metrics, padding, accent);
            }
            Self::Folder => *label_dy = folder::render_folder(out, svg, metrics, padding),
            Self::PaperTape { neo } => {
                paper_tape::render_paper_tape(out, svg, metrics, padding, neo)
            }
            Self::Person => *label_dy = person::render_person(out, svg, metrics, padding),
            Self::TaggedRectangle { neo } => {
                tag_rect::render_tag_rect(out, svg, metrics, padding, neo);
            }
        }
    }

    /// Advances the Rough stream after the caller has selected this shape with `queries_rough`.
    pub fn advance_rough_geometry(
        self,
        geometry: NodeBodyGeometry<'_>,
        metrics: TextMetrics,
        padding: f64,
    ) {
        match self {
            Self::Browser { .. } => browser::advance_rough_geometry(geometry, metrics, padding),
            Self::Bucket { .. } => bucket::advance_rough_geometry(geometry, metrics, padding),
            Self::Console { .. } => console::advance_rough_geometry(geometry, metrics, padding),
            Self::Folder => folder::advance_rough_geometry(geometry, metrics, padding),
            Self::PaperTape { neo } => {
                paper_tape::advance_rough_geometry(geometry, metrics, padding, neo);
            }
            Self::Person => person::advance_rough_geometry(geometry, metrics, padding),
            Self::TaggedRectangle { neo } => {
                tag_rect::advance_rough_geometry(geometry, metrics, padding, neo);
            }
        }
    }
}

/// Builds a bounded polygonal approximation of a rounded rectangle.
///
/// RoughJS accepts arbitrary SVG paths, but its path flattener recursively expands arc segments.
/// Flowchart object shapes are user-controlled, so keeping this hand-drawn input as a finite
/// polygon avoids an unbounded recursion path while preserving the rounded silhouette.
fn rounded_rect_points(x: f64, y: f64, width: f64, height: f64, radius: f64) -> Vec<(f64, f64)> {
    const ARC_SEGMENTS: usize = 8;

    let radius = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    if radius == 0.0 {
        return vec![
            (x, y),
            (x + width, y),
            (x + width, y + height),
            (x, y + height),
        ];
    }

    let mut points = Vec::with_capacity(4 * (ARC_SEGMENTS + 1));
    let append_arc = |points: &mut Vec<(f64, f64)>, cx: f64, cy: f64, start: f64, end: f64| {
        for step in 0..=ARC_SEGMENTS {
            if !points.is_empty() && step == 0 {
                continue;
            }
            let t = step as f64 / ARC_SEGMENTS as f64;
            let angle = start + (end - start) * t;
            points.push((cx + radius * angle.cos(), cy + radius * angle.sin()));
        }
    };

    // Start at the top-left tangent and walk clockwise in SVG's y-down coordinate system.
    points.push((x + radius, y));
    points.push((x + width - radius, y));
    append_arc(
        &mut points,
        x + width - radius,
        y + radius,
        -std::f64::consts::FRAC_PI_2,
        0.0,
    );
    append_arc(
        &mut points,
        x + width - radius,
        y + height - radius,
        0.0,
        std::f64::consts::FRAC_PI_2,
    );
    append_arc(
        &mut points,
        x + radius,
        y + height - radius,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    append_arc(
        &mut points,
        x + radius,
        y + radius,
        std::f64::consts::PI,
        std::f64::consts::PI * 1.5,
    );
    points
}
