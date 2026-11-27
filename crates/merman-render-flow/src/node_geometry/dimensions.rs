use merman_render_core::{Result, text::TextMetrics};

use super::{
    FlowchartRectangleGeometry, ImageSquareGeometry, ImageSquareSizing,
    PreparedFlowchartNodeGeometry, resolve_flowchart_shape,
};
use crate::{
    shapes::{FlowchartShape, bang_geometry, cloud_geometry},
    svg_label::{FlowchartSvgLabelOwner, FlowchartSvgLabelSidecarBuilder},
};

// FlowDB supplies no pre-paint width for crossed-circle nodes.
// Pinned crossedCircle.ts therefore captures this radius before updateNodeBounds.
pub const CROSSED_CIRCLE_RADIUS: f64 = 30.0;

pub fn flowchart_brace_content_dimensions(
    shape: &str,
    label_width: f64,
    label_height: f64,
    padding: f64,
    look_is_neo: bool,
) -> (f64, f64) {
    // Public Flowchart configuration normalizes negative padding to zero.
    // Mermaid's brace shapes otherwise preserve a zero-sized label box.
    let padding = padding.max(0.0);
    let (padding_x, padding_y) = if look_is_neo {
        if matches!(shape, "comment" | "brace" | "brace-l") {
            (18.0, 12.0)
        } else {
            (36.0, 24.0)
        }
    } else {
        (padding, padding)
    };
    (
        label_width.max(0.0) + padding_x,
        label_height.max(0.0) + padding_y,
    )
}

pub struct StackedDocumentGeometry {
    pub outer_points: Vec<(f64, f64)>,
    pub inner_points: Vec<(f64, f64)>,
    pub group_dy: f64,
    pub label_dx: f64,
    pub label_dy: f64,
}

pub fn flowchart_stacked_document_geometry(
    label_width: f64,
    label_height: f64,
    padding: f64,
    look_is_neo: bool,
) -> StackedDocumentGeometry {
    // Pinned multiWaveEdgedRectangle.ts: keep the same unshifted outer polygon
    // for updateNodeBounds/intersection and a separate paint-group translation.
    let padding = padding.max(0.0);
    let (padding_x, padding_y) = if look_is_neo {
        (16.0, 12.0)
    } else {
        (padding, padding)
    };
    let w = label_width.max(0.0) + 2.0 * padding_x;
    let h = label_height.max(0.0) + 3.0 * padding_y;
    let amplitude = h / if look_is_neo { 4.0 } else { 8.0 };
    let final_h = h + amplitude / 2.0;
    let x = -w / 2.0;
    let y = -final_h / 2.0;
    let offset = 10.0;
    let wave_x = x - offset;
    let wave_y = y + final_h + offset;
    let delta_x = (x + w - offset) - wave_x;
    let cycle_length = delta_x / 0.8;
    // Preserve the layout's finite zero-span fallback. Upstream divides by
    // zero here for an empty classic label with zero padding.
    let frequency = if cycle_length == 0.0 {
        0.0
    } else {
        2.0 * std::f64::consts::PI / cycle_length
    };
    let mut outer_points = Vec::with_capacity(62);
    outer_points.push((x - offset, y + offset));
    outer_points.push((x - offset, y + final_h + offset));
    for i in 0..=50 {
        let px = wave_x + (i as f64 / 50.0) * delta_x;
        let py = wave_y + amplitude * (frequency * (px - wave_x)).sin();
        outer_points.push((px, py));
    }
    let last_y = outer_points[52].1;
    outer_points.extend([
        (x + w - offset, last_y - offset),
        (x + w, last_y - offset),
        (x + w, last_y - 2.0 * offset),
        (x + w + offset, last_y - 2.0 * offset),
        (x + w + offset, y - offset),
        (x + offset, y - offset),
        (x + offset, y),
        (x, y),
        (x, y + offset),
    ]);
    let inner_points = vec![
        (x, y + offset),
        (x + w - offset, y + offset),
        (x + w - offset, last_y - offset),
        (x + w, last_y - offset),
        (x + w, y),
        (x, y),
    ];
    StackedDocumentGeometry {
        outer_points,
        inner_points,
        group_dy: -amplitude / 2.0,
        label_dx: -offset,
        label_dy: offset - amplitude,
    }
}

#[derive(Clone, Copy)]
pub enum LeanKind {
    Right,
    Left,
    Trapezoid,
    InvertedTrapezoid,
}

pub struct LeanGeometry {
    pub width: f64,
    pub height: f64,
    pub points: [(f64, f64); 4],
    pub translate_x: f64,
    pub translate_y: f64,
}

impl LeanGeometry {
    pub fn from_label(
        kind: LeanKind,
        label_width: f64,
        label_height: f64,
        padding: f64,
        neo: bool,
    ) -> Self {
        let padding = padding.max(0.0);
        let scale = if matches!(kind, LeanKind::InvertedTrapezoid) {
            2.0
        } else {
            1.0
        };
        let h = label_height.max(0.0) + padding * scale;
        let w = label_width.max(0.0) + padding * scale * if neo { 2.0 } else { 1.0 };
        Self::from_bounds(kind, w + h, h)
    }

    pub fn from_bounds(kind: LeanKind, width: f64, height: f64) -> Self {
        let h = height.max(0.0);
        let w = (width - h).max(0.0);
        let dx = 3.0 * h / 6.0;
        let points = match kind {
            LeanKind::Right => [(-dx, 0.0), (w, 0.0), (w + dx, -h), (0.0, -h)],
            LeanKind::Left => [(0.0, 0.0), (w + dx, 0.0), (w, -h), (-dx, -h)],
            LeanKind::Trapezoid => [(-dx, 0.0), (w + dx, 0.0), (w, -h), (0.0, -h)],
            LeanKind::InvertedTrapezoid => [(0.0, 0.0), (w, 0.0), (w + dx, -h), (-dx, -h)],
        };
        Self {
            width: w + h,
            height: h,
            points,
            translate_x: -w / 2.0,
            translate_y: h / 2.0,
        }
    }
}

pub struct OddGeometry {
    pub width: f64,
    pub height: f64,
    pub points: [(f64, f64); 5],
    pub shift_x: f64,
}

impl OddGeometry {
    pub fn from_label(label_width: f64, label_height: f64, padding: f64, neo: bool) -> Self {
        let p = padding.max(0.0);
        let w = label_width.max(0.0) + if neo { 42.0 } else { p };
        let h = label_height.max(0.0) + if neo { 24.0 } else { p };
        Self::from_bounds(w + h / 4.0, h)
    }

    pub fn from_bounds(width: f64, height: f64) -> Self {
        let h = height.max(0.0);
        let w = (width - h / 4.0).max(0.0);
        let x = -w / 2.0;
        let y = -h / 2.0;
        let notch = y / 2.0;
        Self {
            width: w + h / 4.0,
            height: h,
            points: [(x + notch, y), (x, 0.0), (x + notch, -y), (-x, -y), (-x, y)],
            shift_x: -notch / 2.0,
        }
    }
}

pub struct HexagonGeometry {
    pub width: f64,
    pub height: f64,
    pub points: [(f64, f64); 6],
}

impl HexagonGeometry {
    fn shoulder(height: f64, neo: bool) -> f64 {
        height / if neo { 3.5 } else { 4.0 }
    }

    pub fn from_label(label_width: f64, label_height: f64, padding: f64, neo: bool) -> Self {
        let p = padding.max(0.0);
        // Pinned hexagon.ts intentionally uses 70 on the height and 32 on the width.
        let h = label_height.max(0.0) + if neo { 70.0 } else { p };
        let w = label_width.max(0.0) + 2.0 * Self::shoulder(h, neo) + if neo { 32.0 } else { p };
        Self::from_bounds(w, h, neo)
    }

    pub fn from_bounds(width: f64, height: f64, neo: bool) -> Self {
        let w = width.max(0.0);
        let h = height.max(0.0);
        let m = Self::shoulder(h, neo);
        Self {
            width: w,
            height: h,
            points: [
                (m, 0.0),
                (w - m, 0.0),
                (w, -h / 2.0),
                (w - m, -h),
                (m, -h),
                (0.0, -h / 2.0),
            ],
        }
    }
}

pub struct DoubleCircleGeometry {
    pub inner_radius: f64,
    pub outer_radius: f64,
}

impl DoubleCircleGeometry {
    fn gap(neo: bool) -> f64 {
        if neo { 12.0 } else { 5.0 }
    }

    pub fn from_label(label_width: f64, label_height: f64, padding: f64, neo: bool) -> Self {
        let w = label_width.max(0.0);
        let h = label_height.max(0.0);
        let inner_radius = (w * w + h * h).sqrt() / 2.0 + if neo { 16.0 } else { padding.max(0.0) };
        Self {
            inner_radius,
            outer_radius: inner_radius + Self::gap(neo),
        }
    }

    pub fn from_outer_diameter(diameter: f64, neo: bool) -> Self {
        let outer_radius = (diameter / 2.0).max(Self::gap(neo));
        Self {
            outer_radius,
            inner_radius: outer_radius - Self::gap(neo),
        }
    }
}

fn sampled_circle_points(
    center_x: f64,
    center_y: f64,
    radius: f64,
    count: usize,
    start_deg: f64,
    end_deg: f64,
    negate: bool,
) -> impl Iterator<Item = (f64, f64)> {
    let start = start_deg.to_radians();
    let step = (end_deg.to_radians() - start) / (count.saturating_sub(1).max(1) as f64);
    (0..count).map(move |i| {
        let angle = start + i as f64 * step;
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();
        if negate { (-x, -y) } else { (x, y) }
    })
}

pub struct StadiumGeometry {
    pub width: f64,
    pub height: f64,
}
impl StadiumGeometry {
    pub fn from_label(label_width: f64, label_height: f64, padding: f64, neo: bool) -> Self {
        let padding = padding.max(0.0);
        let padding_x = if neo { 40.0 } else { padding };
        let padding_y = if neo { 24.0 } else { padding };
        let h = label_height.max(0.0) + padding_y;
        let inscribed = h * (std::f64::consts::PI / (2.0 * 49.0)).cos();
        let cap = (inscribed * inscribed - label_height.max(0.0).powi(2))
            .max(0.0)
            .sqrt();
        let w = (label_width.max(0.0) + h / 4.0 + padding_x)
            .max(1.5 * h + (h - inscribed))
            .max(label_width.max(0.0) + padding_x + h - cap);
        // Layout uses the theoretical dimensions. Paint and intersection sample the outline
        // on demand because its narrower extrema would otherwise shrink the stadium twice.
        Self {
            width: w,
            height: h,
        }
    }

    pub fn outline(&self) -> Vec<(f64, f64)> {
        let w = self.width;
        let h = self.height;
        let radius = h / 2.0;
        const ARC_POINTS: usize = 50;
        let mut points = Vec::with_capacity(2 * ARC_POINTS + 3);
        points.extend([(-w / 2.0 + radius, -h / 2.0), (w / 2.0 - radius, -h / 2.0)]);
        points.extend(sampled_circle_points(
            -w / 2.0 + radius,
            0.0,
            radius,
            ARC_POINTS,
            90.0,
            270.0,
            true,
        ));
        points.push((w / 2.0 - radius, h / 2.0));
        points.extend(sampled_circle_points(
            w / 2.0 - radius,
            0.0,
            radius,
            ARC_POINTS,
            270.0,
            450.0,
            true,
        ));
        points
    }
}

pub struct DelayGeometry {
    pub width: f64,
    pub height: f64,
    pub points: Vec<(f64, f64)>,
}
impl DelayGeometry {
    pub fn from_label(label_width: f64, label_height: f64, padding: f64, neo: bool) -> Self {
        let padding = padding.max(0.0);
        let px = if neo { 16.0 } else { padding };
        let py = if neo { 12.0 } else { padding };
        let min_width = 15.0;
        let min_height = 10.0;
        let h = label_height.max(min_height) + py * 2.0;
        let radius = h / 2.0;
        let cap = radius
            - (radius * radius - (label_height.max(0.0) / 2.0).powi(2))
                .max(0.0)
                .sqrt();
        let w = (label_width.max(min_width) + cap * 2.0) + px * 2.0;
        let mut points = vec![(-w / 2.0, -h / 2.0), (w / 2.0 - radius, -h / 2.0)];
        points.extend(sampled_circle_points(
            -w / 2.0 + radius,
            0.0,
            radius,
            50,
            90.0,
            270.0,
            true,
        ));
        points.extend([(w / 2.0 - radius, h / 2.0), (-w / 2.0, h / 2.0)]);
        Self {
            width: w,
            height: h,
            points,
        }
    }
}

pub struct DisplayGeometry {
    pub width: f64,
    pub height: f64,
    pub tx: f64,
    pub ty: f64,
    pub points: Vec<(f64, f64)>,
}
impl DisplayGeometry {
    pub fn from_label(label_width: f64, label_height: f64, padding: f64, neo: bool) -> Self {
        let padding = padding.max(0.0);
        let px = if neo { 16.0 } else { padding };
        let py = if neo { 12.0 } else { padding };
        let min_width = 20.0;
        let min_height = 5.0;
        let h = (label_height.max(0.0) + py * 2.0).max(min_height);
        let radius = h / 2.0;
        let cap = radius
            - (radius * radius - (label_height.max(0.0) / 2.0).powi(2))
                .max(0.0)
                .sqrt();
        let side = (h / 4.0).max(cap);
        let w = (label_width.max(0.0) + px * 2.0 + side * 2.0)
            .max((label_width.max(0.0) + px * 2.0) * 1.25)
            .max(min_width);
        let rw = w - radius;
        let tw = h / 4.0;
        let mut points = vec![(rw, 0.0), (tw, 0.0), (0.0, h / 2.0), (tw, h), (rw, h)];
        points.extend(sampled_circle_points(
            -rw,
            -h / 2.0,
            radius,
            50,
            270.0,
            90.0,
            true,
        ));
        Self {
            width: w,
            height: h,
            tx: -w / 2.0,
            ty: -h / 2.0,
            points,
        }
    }
}

fn node_render_dimensions(
    layout_shape: Option<&str>,
    metrics: TextMetrics,
    padding: f64,
    look_is_neo: bool,
) -> (f64, f64) {
    // This function mirrors pinned Mermaid node shape sizing rules at the "rendering-elements"
    // layer, but uses our headless `TextMeasurer` metrics instead of DOM `getBBox()`.
    //
    // References:
    // - `packages/mermaid/src/diagrams/flowchart/flowDb.ts` (shape assignment + padding)
    // - `packages/mermaid/src/rendering-util/rendering-elements/shapes/*.ts` (shape bounds)
    // Mermaid's DOM `getBBox()` can legitimately return 0 for empty/whitespace-only labels.
    // Do not clamp to 1px here, otherwise we skew layout widths (notably `max-width`) by 1px.
    let text_w = metrics.width.max(0.0);
    let text_h = metrics.height.max(0.0);
    let p = padding.max(0.0);

    let shape = layout_shape.unwrap_or("squareRect");
    let resolved_shape = resolve_flowchart_shape(shape)
        .unwrap_or_else(|error| panic!("unvalidated Flowchart shape reached layout: {error}"));
    if let FlowchartShape::Bpmn(shape) = resolved_shape {
        let geometry = shape.geometry(
            kurbo::Size::new(metrics.width, metrics.height),
            text_h > 0.0,
            false,
            false,
        );
        return (geometry.width, geometry.height);
    }

    fn circle_points(
        center_x: f64,
        center_y: f64,
        radius: f64,
        num_points: usize,
        start_deg: f64,
        end_deg: f64,
        negate: bool,
    ) -> Vec<(f64, f64)> {
        let start = start_deg.to_radians();
        let end = end_deg.to_radians();
        let angle_range = end - start;
        let angle_step = if num_points > 1 {
            angle_range / (num_points as f64 - 1.0)
        } else {
            0.0
        };
        let mut out: Vec<(f64, f64)> = Vec::with_capacity(num_points);
        for i in 0..num_points {
            let a = start + (i as f64) * angle_step;
            let x = center_x + radius * a.cos();
            let y = center_y + radius * a.sin();
            if negate {
                out.push((-x, -y));
            } else {
                out.push((x, y));
            }
        }
        out
    }

    fn bbox_of_points(points: &[(f64, f64)]) -> Option<(f64, f64, f64, f64)> {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for &(x, y) in points {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
            Some((min_x, min_y, max_x, max_y))
        } else {
            None
        }
    }

    fn generate_full_sine_wave_points(
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        amplitude: f64,
        num_cycles: f64,
    ) -> Vec<(f64, f64)> {
        // Ported from Mermaid `generateFullSineWavePoints` (50 segments).
        let steps: usize = 50;
        let delta_x = x2 - x1;
        let delta_y = y2 - y1;
        let cycle_length = if num_cycles.abs() < 1e-9 {
            delta_x
        } else {
            delta_x / num_cycles
        };
        let frequency = if cycle_length.abs() < 1e-9 {
            0.0
        } else {
            (2.0 * std::f64::consts::PI) / cycle_length
        };
        let mid_y = y1 + delta_y / 2.0;

        let mut points: Vec<(f64, f64)> = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = (i as f64) / (steps as f64);
            let x = x1 + t * delta_x;
            let y = mid_y + amplitude * (frequency * (x - x1)).sin();
            points.push((x, y));
        }
        points
    }

    fn arc_points(
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        rx: f64,
        ry: f64,
        clockwise: bool,
    ) -> Vec<(f64, f64)> {
        let num_points: usize = 20;

        let mid_x = (x1 + x2) / 2.0;
        let mid_y = (y1 + y2) / 2.0;
        let angle = (y2 - y1).atan2(x2 - x1);

        let dx = (x2 - x1) / 2.0;
        let dy = (y2 - y1) / 2.0;
        let transformed_x = dx / rx;
        let transformed_y = dy / ry;
        let distance = (transformed_x * transformed_x + transformed_y * transformed_y).sqrt();
        if distance > 1.0 {
            return vec![(x1, y1), (x2, y2)];
        }

        let scaled_center_distance = (1.0 - distance * distance).sqrt();
        let sign = if clockwise { -1.0 } else { 1.0 };
        let center_x = mid_x + scaled_center_distance * ry * angle.sin() * sign;
        let center_y = mid_y - scaled_center_distance * rx * angle.cos() * sign;

        let start_angle = ((y1 - center_y) / ry).atan2((x1 - center_x) / rx);
        let end_angle = ((y2 - center_y) / ry).atan2((x2 - center_x) / rx);

        let mut angle_range = end_angle - start_angle;
        if clockwise && angle_range < 0.0 {
            angle_range += 2.0 * std::f64::consts::PI;
        }
        if !clockwise && angle_range > 0.0 {
            angle_range -= 2.0 * std::f64::consts::PI;
        }

        let mut points: Vec<(f64, f64)> = Vec::with_capacity(num_points);
        for i in 0..num_points {
            let t = i as f64 / (num_points - 1) as f64;
            let a = start_angle + t * angle_range;
            let x = center_x + rx * a.cos();
            let y = center_y + ry * a.sin();
            points.push((x, y));
        }
        points
    }

    match shape {
        // Flowchart v2 anchor ignores its label and uses a fixed 1px source radius.
        "anchor" => (2.0, 2.0),

        // Default flowchart process node.
        "squareRect" | "rect" | "proc" | "process" | "rectangle" => {
            if look_is_neo {
                (text_w + 32.0, text_h + 24.0)
            } else {
                (text_w + 4.0 * p, text_h + 2.0 * p)
            }
        }
        "data-store" | "datastore" => (text_w + 4.0 * p, text_h + 2.0 * p),

        // Mermaid uses a few aliases for the same rounded-rectangle shape across layers.
        // In FlowDB output (flowchart-v2), this commonly appears as `rounded`.
        "roundedRect" | "rounded" | "event" => (text_w + 2.0 * p, text_h + 2.0 * p),

        // Note (rendering-elements/state note box).
        "note" => (text_w + 2.0 * p, text_h + 2.0 * p),

        // Diamond (decision/question).
        "diamond" | "question" | "diam" | "decision" => {
            let w = text_w + p;
            let h = text_h + p;
            let s = w + h;
            (s, s)
        }

        // Mermaid 11.17 folder/directory shape: a file-folder outline with a raised tab.
        // The tab is part of the rendered bounds, so include it in the layout height.
        "folder" | "directory" => {
            let w = (text_w + 2.0 * p).max(90.0);
            let content_height = text_h + 2.0 * p;
            let tab_height = (content_height * 0.16).clamp(8.0, 14.0);
            (w, content_height + tab_height)
        }

        // Mermaid 11.17 object-storage bucket: a tapered body plus an elliptical rim.
        "bucket" => {
            let w = (text_w + 2.0 * p).max(80.0);
            let rim_ry = (w * 0.08).clamp(5.0, 12.0);
            (w, text_h + 2.0 * p + rim_ry)
        }

        // Mermaid 11.17 console/browser windows reserve a top chrome band for their decoration.
        "console" => {
            let w = (text_w + 2.0 * p).max(90.0);
            (w, text_h + 2.0 * p + 20.0)
        }
        "browser" => {
            let w = (text_w + 2.0 * p).max(90.0);
            (w, text_h + 2.0 * p + 18.0)
        }

        // Mermaid 11.17 person: a C4-style head above a rounded body.
        "person" => {
            let w = (text_w + 2.0 * p).max(100.0);
            let head_radius = (w * 0.23).clamp(16.0, 56.0);
            let overlap = head_radius * 0.27;
            let body_height = text_h + 2.0 * p;
            (w, body_height + 2.0 * head_radius - overlap)
        }

        "hexagon" | "hex" | "prepare" => {
            let geometry = HexagonGeometry::from_label(text_w, text_h, p, look_is_neo);
            (geometry.width, geometry.height)
        }

        // Stadium/terminator.
        "stadium" | "terminal" | "pill" => {
            let geometry = StadiumGeometry::from_label(text_w, text_h, p, look_is_neo);
            (geometry.width, geometry.height)
        }

        // Subroutine/subprocess (framed rectangle): adds an 8px "frame" on both sides.
        "subroutine" | "fr-rect" | "subproc" | "subprocess" | "framed-rectangle" => {
            let w = text_w + if look_is_neo { 28.0 } else { p };
            let h = text_h + if look_is_neo { 12.0 } else { p };
            (w + 16.0, h)
        }

        // Cylinder/database.
        "cylinder" | "cyl" | "db" | "database" => {
            let label_padding = if look_is_neo { 24.0 } else { p };
            let w = text_w + label_padding;
            let rx = w / 2.0;
            let ry = rx / (2.5 + w / 50.0);
            // Mermaid's cylinder path height ends up including two extra `ry` from the ellipses.
            // See `createCylinderPathD` + `translate(..., -(h/2 + ry))`.
            let height = text_h + label_padding + 3.0 * ry;
            (w, height)
        }

        // Flowchart v2 tilted cylinder ("horizontal-cylinder").
        "h-cyl" | "das" | "horizontal-cylinder" => {
            // Mermaid `tiltedCylinder.ts`:
            // - `labelPadding` is 12 for Neo and `halfPadding` for Classic.
            // - `h = bbox.height + labelPadding`
            // - `ry = h / 2`, `rx = ry / (2.5 + h / 50)`
            // - `w = bbox.width + rx + labelPadding`
            // - the rendered `<path>` bbox expands by `rx` on both sides (arc extents), so Dagre
            //   sees `out_w = w + 2*rx` via `updateNodeBounds(...)`.
            let label_padding = if look_is_neo { 12.0 } else { p / 2.0 };
            let h = text_h + label_padding;
            let ry = h / 2.0;
            let rx = if ry == 0.0 {
                0.0
            } else {
                ry / (2.5 + h / 50.0)
            };
            let w = text_w + rx + label_padding;
            (w + 2.0 * rx, h)
        }

        // Flowchart v2 window-pane ("internal-storage").
        "win-pane" | "internal-storage" | "window-pane" => {
            // Mermaid `windowPane.ts` uses fixed 16px/12px axis padding in Neo. The final
            // rendered bbox adds the 10px frame offset on both axes.
            let padding_x = if look_is_neo { 16.0 } else { p };
            let padding_y = if look_is_neo { 12.0 } else { p };
            let w = (text_w + 2.0 * padding_x).max(0.0);
            let h = (text_h + 2.0 * padding_y).max(0.0);
            let rect_offset = 10.0;
            (w + rect_offset, h + rect_offset)
        }

        // Circle. Mermaid sizes the radius from the label bbox diagonal, then adds
        // the look-specific padding (circle.ts: labelRadius + labelPadding/halfPadding).
        // Neo adds one 16px radius padding step, matching a double circle's inner ring.
        "circle" | "circ" => {
            let label_diameter = text_w.hypot(text_h);
            let diameter = if look_is_neo {
                label_diameter + 32.0
            } else {
                label_diameter + p
            };
            (diameter, diameter)
        }

        // Organic arc paths use the browser-visible path bbox, not their construction box.
        "bang" => {
            let geometry = bang_geometry(text_w, text_h, p);
            (geometry.width(), geometry.height())
        }
        "cloud" => {
            let geometry = cloud_geometry(text_w, text_h, p);
            (geometry.width(), geometry.height())
        }

        // Collapsed subgraphs are compact two-row nodes with fixed 8px padding, an 8px
        // separator gap, and a 20px ellipsis row. Mermaid enforces an 80px minimum width.
        "collapsedGroup" | "collapsed-group" => {
            ((text_w + 16.0).max(80.0), text_h + 8.0 + 20.0 + 16.0)
        }

        "doublecircle" | "dbl-circ" | "double-circle" => {
            let geometry = DoubleCircleGeometry::from_label(text_w, text_h, p, look_is_neo);
            let diameter = 2.0 * geometry.outer_radius;
            (diameter, diameter)
        }

        // Small start circle (stateStart in rendering-elements).
        "sm-circ" | "small-circle" | "start" => (14.0, 14.0),

        // Stop framed circle (`stateEnd.ts`) normalizes both source dimensions to 14px.
        "fr-circ" | "framed-circle" | "stop" => (14.0, 14.0),

        // Fork/join bar (uses `lineColor` fill/stroke; no label).
        "fork" | "join" => (70.0, 10.0),

        // Choice diamond (stateChoice in rendering-elements).
        "choice" => (28.0, 28.0),

        // Flowchart v2 lightning bolt (Communication link). Mermaid clears `node.label`.
        "bolt" | "com-link" | "lightning-bolt" => (35.0, 70.0),

        // Flowchart v2 filled circle (`filledCircle.ts`) uses a fixed 7px source radius.
        "f-circ" | "junction" | "filled-circle" => (14.0, 14.0),

        // Flowchart v2 crossed circle (`crossedCircle.ts`) has a minimum 30px source radius.
        "cross-circ" | "summary" | "crossed-circle" => {
            (2.0 * CROSSED_CIRCLE_RADIUS, 2.0 * CROSSED_CIRCLE_RADIUS)
        }

        // Flowchart v2 delay / halfRoundedRectangle.
        "delay" | "half-rounded-rectangle" => {
            let geometry = DelayGeometry::from_label(text_w, text_h, p, look_is_neo);
            (geometry.width, geometry.height)
        }

        // Flowchart v2 lined cylinder (Disk storage).
        "lin-cyl" | "disk" | "lined-cylinder" => {
            // Mermaid `linedCylinder.ts` uses fixed axis padding for Neo and the configured
            // padding for Classic. The cylinder height includes the top, body, and bottom
            // ellipse radii from the rendered path.
            let (padding_x, padding_y) = if look_is_neo { (16.0, 24.0) } else { (p, p) };
            let w = text_w + 2.0 * padding_x;
            let rx = w / 2.0;
            let ry = rx / (2.5 + w / 50.0);
            let height = text_h + 2.0 * padding_y + 3.0 * ry;
            (w, height)
        }

        // Flowchart v2 curved trapezoid (Display).
        "curv-trap" | "display" | "curved-trapezoid" => {
            let geometry = DisplayGeometry::from_label(text_w, text_h, p, look_is_neo);
            (geometry.width, geometry.height)
        }

        // Flowchart v2 divided rectangle (Divided process).
        "div-rect" | "div-proc" | "divided-rectangle" | "divided-process" => {
            // Mermaid `dividedRect.ts` uses a single 16px Neo padding term before the
            // rendered 20% top band expands the final height.
            let padding = if look_is_neo { 16.0 } else { p };
            let w = text_w + padding;
            let h = text_h + padding;
            let rect_offset = h * 0.2;
            let x = -w / 2.0;
            let y = -h / 2.0 - rect_offset / 2.0;
            let points: Vec<(f64, f64)> = vec![
                (x, y + rect_offset),
                (-x, y + rect_offset),
                (-x, -y),
                (x, -y),
                (x, y),
                (-x, y),
                (-x, y + rect_offset),
            ];
            let (min_x, min_y, max_x, max_y) = bbox_of_points(&points).unwrap_or((x, y, -x, -y));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 triangle (Extract).
        "tri" | "extract" | "triangle" => {
            // Mermaid uses two-side horizontal padding for Neo and one-side padding for Classic.
            // The same width feeds both layout and SVG geometry.
            let w = text_w + if look_is_neo { 2.0 * p } else { p };
            let h = w + text_h;
            (h, h)
        }

        // Flowchart v2 flipped triangle (Manual file).
        "manual-file" | "flipped-triangle" | "flip-tri" => {
            let w = text_w + if look_is_neo { 2.0 * p } else { p };
            let h = w + text_h;
            (h, h)
        }

        // Flowchart v2 sloped rectangle (Manual input).
        "manual-input" | "sloped-rectangle" | "sl-rect" => {
            let padding_x = if look_is_neo { 16.0 } else { p };
            let padding_y = if look_is_neo { 12.0 } else { p };
            let w = (text_w + 2.0 * padding_x).max(0.0);
            let h = (text_h + 2.0 * padding_y).max(0.0);
            (w, (1.5 * h).max(0.0))
        }

        // Flowchart v2 document (wave-edged rectangle).
        "doc" | "document" => {
            let padding_x = if look_is_neo { 16.0 } else { p };
            let padding_y = if look_is_neo { 12.0 } else { p };
            let w = (text_w + 2.0 * padding_x).max(0.0);
            let h = (text_h + 2.0 * padding_y).max(0.0);
            let wave_amplitude = if look_is_neo { h / 4.0 } else { h / 8.0 };
            let final_h = h + wave_amplitude;
            let min_width = 14.0;
            let extra_w = if w < min_width {
                (min_width - w) / 2.0
            } else {
                0.0
            };

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0 - extra_w, final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                -w / 2.0 - extra_w,
                final_h / 2.0,
                w / 2.0 + extra_w,
                final_h / 2.0,
                wave_amplitude,
                0.8,
            ));
            points.push((w / 2.0 + extra_w, -final_h / 2.0));
            points.push((-w / 2.0 - extra_w, -final_h / 2.0));

            let (min_x, min_y, max_x, max_y) = bbox_of_points(&points).unwrap_or((
                -w / 2.0,
                -final_h / 2.0,
                w / 2.0,
                final_h / 2.0,
            ));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 stacked document (multi-wave edged rectangle).
        "docs" | "documents" | "st-doc" | "stacked-document" => {
            let geometry =
                flowchart_stacked_document_geometry(text_w, text_h, padding, look_is_neo);
            let (min_x, min_y, max_x, max_y) =
                bbox_of_points(&geometry.outer_points).unwrap_or((0.0, 0.0, 0.0, 0.0));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 stacked rectangle (multi-process).
        "st-rect" | "procs" | "processes" | "stacked-rectangle" => {
            // Mermaid `multiRect.ts` uses fixed Neo label padding and a larger layer offset.
            let padding_x = if look_is_neo { 16.0 } else { p };
            let padding_y = if look_is_neo { 12.0 } else { p };
            let w = (text_w + 2.0 * padding_x).max(0.0);
            let h = (text_h + 2.0 * padding_y).max(0.0);
            let rect_offset = if look_is_neo { 10.0 } else { 5.0 };
            (w + 2.0 * rect_offset, h + 2.0 * rect_offset)
        }

        // Flowchart v2 paper-tape / wave rectangle.
        "paper-tape" | "flag" => {
            // Mermaid `waveRectangle.ts` uses look-specific label padding: Neo reserves
            // 16px horizontally and 20px vertically, while Classic uses the configured padding.
            let padding_x = if look_is_neo { 16.0 } else { p };
            let padding_y = if look_is_neo { 20.0 } else { p };
            let w = (text_w + 2.0 * padding_x).max(0.0);
            let h = (text_h + padding_y).max(0.0);
            let wave_amplitude = h / 8.0;
            let final_h = h + wave_amplitude * 2.0;

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0, final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                -w / 2.0,
                final_h / 2.0,
                w / 2.0,
                final_h / 2.0,
                wave_amplitude,
                1.0,
            ));
            points.push((w / 2.0, -final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                w / 2.0,
                -final_h / 2.0,
                -w / 2.0,
                -final_h / 2.0,
                wave_amplitude,
                -1.0,
            ));
            let (min_x, min_y, max_x, max_y) = bbox_of_points(&points).unwrap_or((
                -w / 2.0,
                -final_h / 2.0,
                w / 2.0,
                final_h / 2.0,
            ));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 lined document.
        "lin-doc" | "lined-document" => {
            let padding_x = if look_is_neo { 16.0 } else { p };
            let padding_y = if look_is_neo { 12.0 } else { p };
            let w = (text_w + 2.0 * padding_x).max(0.0);
            let h = (text_h + 2.0 * padding_y).max(0.0);
            let wave_amplitude = if look_is_neo { h / 4.0 } else { h / 8.0 };
            let final_h = h + wave_amplitude;
            let extra = (w / 2.0) * 0.1;

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0 - extra, -final_h / 2.0));
            points.push((-w / 2.0 - extra, final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                -w / 2.0 - extra,
                final_h / 2.0,
                w / 2.0 + extra,
                final_h / 2.0,
                wave_amplitude,
                0.8,
            ));
            points.push((w / 2.0 + extra, -final_h / 2.0));
            points.push((-w / 2.0 - extra, -final_h / 2.0));
            points.push((-w / 2.0, -final_h / 2.0));
            points.push((-w / 2.0, (final_h / 2.0) * 1.1));
            points.push((-w / 2.0, -final_h / 2.0));

            let (min_x, min_y, max_x, max_y) = bbox_of_points(&points).unwrap_or((
                -w / 2.0,
                -final_h / 2.0,
                w / 2.0,
                final_h / 2.0,
            ));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 tagged rectangle.
        "tag-rect" | "tagged-rectangle" | "tag-proc" | "tagged-process" => {
            // Mermaid `taggedRect.ts` applies label padding before adding the tag width.
            let padding_x = if look_is_neo { 16.0 } else { p };
            let padding_y = if look_is_neo { 12.0 } else { p };
            let w = (text_w + 2.0 * padding_x).max(0.0);
            let h = (text_h + 2.0 * padding_y).max(0.0);
            let x = -w / 2.0;
            let y = -h / 2.0;
            let tag_width = 0.2 * h;
            let tag_height = 0.2 * h;
            let rect_points = vec![
                (x - tag_width / 2.0, y),
                (x + w + tag_width / 2.0, y),
                (x + w + tag_width / 2.0, y + h),
                (x - tag_width / 2.0, y + h),
            ];
            let tag_points = vec![
                (x + w - tag_width / 2.0, y + h),
                (x + w + tag_width / 2.0, y + h),
                (x + w + tag_width / 2.0, y + h - tag_height),
            ];
            let mut pts = rect_points;
            pts.extend(tag_points);
            let (min_x, min_y, max_x, max_y) = bbox_of_points(&pts).unwrap_or((x, y, x + w, y + h));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 tagged document.
        "tag-doc" | "tagged-document" => {
            let w = (text_w + 2.0 * p).max(0.0);
            let h = (text_h + 2.0 * p).max(0.0);
            let wave_amplitude = h / 8.0;
            let final_h = h + wave_amplitude;
            let extra = (w / 2.0) * 0.1;
            let tag_width = 0.2 * w;
            let tag_height = 0.2 * h;

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0 - extra, final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                -w / 2.0 - extra,
                final_h / 2.0,
                w / 2.0 + extra,
                final_h / 2.0,
                wave_amplitude,
                0.8,
            ));
            points.push((w / 2.0 + extra, -final_h / 2.0));
            points.push((-w / 2.0 - extra, -final_h / 2.0));

            let x = -w / 2.0 + extra;
            let y = -final_h / 2.0 - tag_height * 0.4;
            let mut tag_points: Vec<(f64, f64)> = Vec::new();
            tag_points.push((x + w - tag_width, (y + h) * 1.3));
            tag_points.push((x + w, y + h - tag_height));
            tag_points.push((x + w, (y + h) * 0.9));
            tag_points.extend(generate_full_sine_wave_points(
                x + w,
                (y + h) * 1.25,
                x + w - tag_width,
                (y + h) * 1.3,
                -h * 0.02,
                0.5,
            ));

            points.extend(tag_points);
            let (min_x, min_y, max_x, max_y) = bbox_of_points(&points).unwrap_or((
                -w / 2.0,
                -final_h / 2.0,
                w / 2.0,
                final_h / 2.0,
            ));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 trapezoidal pentagon (Loop limit).
        "notch-pent" | "loop-limit" | "notched-pentagon" => {
            // The source's 15x5 minimum only applies to an explicitly sized node.
            // Flowchart's automatic size comes directly from the measured label.
            let padding_x = if look_is_neo { 16.0 } else { p };
            let padding_y = if look_is_neo { 12.0 } else { p };
            (text_w + 2.0 * padding_x, text_h + 2.0 * padding_y)
        }

        // Flowchart v2 bow-tie rect (Stored data).
        "bow-rect" | "stored-data" | "bow-tie-rectangle" => {
            let padding_x = if look_is_neo { 16.0 } else { p };
            let padding_y = if look_is_neo { 12.0 } else { p };
            let w = text_w + 2.0 * padding_x;
            let h = text_h + padding_y;
            let ry = h / 2.0;
            let rx = ry / (2.5 + h / 50.0);
            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((w / 2.0, -h / 2.0));
            points.push((-w / 2.0, -h / 2.0));
            points.extend(arc_points(
                -w / 2.0,
                -h / 2.0,
                -w / 2.0,
                h / 2.0,
                rx,
                ry,
                false,
            ));
            points.push((w / 2.0, h / 2.0));
            points.extend(arc_points(
                w / 2.0,
                h / 2.0,
                w / 2.0,
                -h / 2.0,
                rx,
                ry,
                true,
            ));
            let (min_x, min_y, max_x, max_y) =
                bbox_of_points(&points).unwrap_or((-w / 2.0, -h / 2.0, w / 2.0, h / 2.0));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Hourglass/collate (label cleared, but label group still emitted).
        "hourglass" | "collate" => (30.0, 30.0),

        // Card/notched rectangle: Neo adds 28px horizontally and 24px vertically before drawing the notch.
        "notch-rect" | "notched-rectangle" | "card" => {
            if look_is_neo {
                (text_w + 56.0, text_h + 48.0)
            } else {
                (text_w + p + 12.0, text_h + p)
            }
        }

        // Mermaid `shadedProcess.ts` uses fixed Neo label padding and one 8px frame on
        // the outer width; Classic keeps two frame widths around the configured padding.
        "lin-rect" | "lined-rectangle" | "lined-process" | "lin-proc" | "shaded-process" => {
            let padding_x = if look_is_neo { 16.0 } else { p };
            let padding_y = if look_is_neo { 12.0 } else { p };
            let frame_width = 8.0;
            let frame_count = if look_is_neo { 1.0 } else { 2.0 };
            (
                text_w + 2.0 * padding_x + frame_count * frame_width,
                text_h + 2.0 * padding_y,
            )
        }

        // Text block: bbox + 1x padding (not 2x).
        "text" => (text_w + p, text_h + p),

        // Curly brace comment shapes (rendering-elements).
        "comment" | "brace" | "brace-l" => {
            let (w, h) =
                flowchart_brace_content_dimensions(shape, text_w, text_h, padding, look_is_neo);
            let radius = (h * 0.1).max(5.0);
            let group_tx = radius;
            let mut points: Vec<(f64, f64)> = Vec::new();
            points.extend(circle_points(
                w / 2.0,
                -h / 2.0,
                radius,
                30,
                -90.0,
                0.0,
                true,
            ));
            points.push((-w / 2.0 - radius, radius));
            points.extend(circle_points(
                w / 2.0 + radius * 2.0,
                -radius,
                radius,
                20,
                -180.0,
                -270.0,
                true,
            ));
            points.extend(circle_points(
                w / 2.0 + radius * 2.0,
                radius,
                radius,
                20,
                -90.0,
                -180.0,
                true,
            ));
            points.push((-w / 2.0 - radius, -h / 2.0));
            points.extend(circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true));

            let mut rect_points: Vec<(f64, f64)> = Vec::new();
            rect_points.extend([(w / 2.0, -h / 2.0 - radius), (-w / 2.0, -h / 2.0 - radius)]);
            rect_points.extend(circle_points(
                w / 2.0,
                -h / 2.0,
                radius,
                20,
                -90.0,
                0.0,
                true,
            ));
            rect_points.push((-w / 2.0 - radius, -radius));
            rect_points.extend(circle_points(
                w / 2.0 + w * 0.1,
                -radius,
                radius,
                20,
                -180.0,
                -270.0,
                true,
            ));
            rect_points.extend(circle_points(
                w / 2.0 + w * 0.1,
                radius,
                radius,
                20,
                -90.0,
                -180.0,
                true,
            ));
            rect_points.push((-w / 2.0 - radius, h / 2.0));
            rect_points.extend(circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true));
            rect_points.extend([(-w / 2.0, h / 2.0 + radius), (w / 2.0, h / 2.0 + radius)]);
            for p in points.iter_mut().chain(rect_points.iter_mut()) {
                p.0 += group_tx;
            }
            let mut all_points: Vec<(f64, f64)> =
                Vec::with_capacity(points.len() + rect_points.len());
            all_points.extend(points);
            all_points.extend(rect_points);
            let (min_x, min_y, max_x, max_y) =
                bbox_of_points(&all_points).unwrap_or((-w / 2.0, -h / 2.0, w / 2.0, h / 2.0));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }
        "brace-r" => {
            let (w, h) =
                flowchart_brace_content_dimensions(shape, text_w, text_h, padding, look_is_neo);
            let radius = (h * 0.1).max(5.0);
            let group_tx = -radius;
            let mut rect_points: Vec<(f64, f64)> = Vec::new();
            rect_points.extend([(-w / 2.0, -h / 2.0 - radius), (w / 2.0, -h / 2.0 - radius)]);
            rect_points.extend(circle_points(
                w / 2.0,
                -h / 2.0,
                radius,
                20,
                -90.0,
                0.0,
                false,
            ));
            rect_points.push((w / 2.0 + radius, -radius));
            rect_points.extend(circle_points(
                w / 2.0 + radius * 2.0,
                -radius,
                radius,
                20,
                -180.0,
                -270.0,
                false,
            ));
            rect_points.extend(circle_points(
                w / 2.0 + radius * 2.0,
                radius,
                radius,
                20,
                -90.0,
                -180.0,
                false,
            ));
            rect_points.push((w / 2.0 + radius, h / 2.0));
            rect_points.extend(circle_points(
                w / 2.0,
                h / 2.0,
                radius,
                20,
                0.0,
                90.0,
                false,
            ));
            rect_points.extend([(w / 2.0, h / 2.0 + radius), (-w / 2.0, h / 2.0 + radius)]);
            for p in &mut rect_points {
                p.0 += group_tx;
            }
            let (min_x, min_y, max_x, max_y) =
                bbox_of_points(&rect_points).unwrap_or((-w / 2.0, -h / 2.0, w / 2.0, h / 2.0));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }
        "braces" => {
            let (w, h) =
                flowchart_brace_content_dimensions(shape, text_w, text_h, padding, look_is_neo);
            let radius = (h * 0.1).max(5.0);
            let group_tx = radius - radius / 4.0;
            let mut rect_points: Vec<(f64, f64)> = Vec::new();
            rect_points.extend([(w / 2.0, -h / 2.0 - radius), (-w / 2.0, -h / 2.0 - radius)]);
            rect_points.extend(circle_points(
                w / 2.0,
                -h / 2.0,
                radius,
                20,
                -90.0,
                0.0,
                true,
            ));
            rect_points.push((-w / 2.0 - radius, -radius));
            rect_points.extend(circle_points(
                w / 2.0 + radius * 2.0,
                -radius,
                radius,
                20,
                -180.0,
                -270.0,
                true,
            ));
            rect_points.extend(circle_points(
                w / 2.0 + radius * 2.0,
                radius,
                radius,
                20,
                -90.0,
                -180.0,
                true,
            ));
            rect_points.push((-w / 2.0 - radius, h / 2.0));
            rect_points.extend(circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true));
            rect_points.extend([
                (-w / 2.0, h / 2.0 + radius),
                (w / 2.0 - radius - radius / 2.0, h / 2.0 + radius),
            ]);
            rect_points.extend(circle_points(
                -w / 2.0 + radius + radius / 2.0,
                -h / 2.0,
                radius,
                20,
                -90.0,
                -180.0,
                true,
            ));
            rect_points.push((w / 2.0 - radius / 2.0, radius));
            rect_points.extend(circle_points(
                -w / 2.0 - radius / 2.0,
                -radius,
                radius,
                20,
                0.0,
                90.0,
                true,
            ));
            rect_points.extend(circle_points(
                -w / 2.0 - radius / 2.0,
                radius,
                radius,
                20,
                -90.0,
                0.0,
                true,
            ));
            rect_points.push((w / 2.0 - radius / 2.0, -radius));
            rect_points.extend(circle_points(
                -w / 2.0 + radius + radius / 2.0,
                h / 2.0,
                radius,
                30,
                -180.0,
                -270.0,
                true,
            ));
            for p in &mut rect_points {
                p.0 += group_tx;
            }
            let (min_x, min_y, max_x, max_y) =
                bbox_of_points(&rect_points).unwrap_or((-w / 2.0, -h / 2.0, w / 2.0, h / 2.0));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // The first three variants share dimensions; only their vertex order differs.
        "lean_right" | "lean-r" | "lean-right" | "in-out" | "lean_left" | "lean-l"
        | "lean-left" | "out-in" | "trapezoid" | "trap-b" | "priority" | "trapezoid-bottom" => {
            let geometry =
                LeanGeometry::from_label(LeanKind::Right, text_w, text_h, p, look_is_neo);
            (geometry.width, geometry.height)
        }
        "inv_trapezoid" | "inv-trapezoid" | "trap-t" | "manual" | "trapezoid-top" => {
            let geometry = LeanGeometry::from_label(
                LeanKind::InvertedTrapezoid,
                text_w,
                text_h,
                p,
                look_is_neo,
            );
            (geometry.width, geometry.height)
        }
        "odd" | "rect_left_inv_arrow" => {
            let geometry = OddGeometry::from_label(text_w, text_h, p, look_is_neo);
            (geometry.width, geometry.height)
        }

        // Ellipses are currently broken upstream but still emitted by FlowDB.
        // Keep a reasonable headless size for layout stability.
        "ellipse" => (text_w + 2.0 * p, text_h + 2.0 * p),

        _ => panic!("Flowchart shape {shape} has no layout geometry"),
    }
}

pub fn flowchart_node_render_dimensions(
    layout_shape: Option<&str>,
    metrics: TextMetrics,
    padding: f64,
    look_is_neo: bool,
) -> (f64, f64) {
    node_render_dimensions(layout_shape, metrics, padding, look_is_neo)
}

pub struct NodeLayoutDimensionsRequest<'a> {
    pub layout_shape: Option<&'a str>,
    pub layout_direction: &'a str,
    pub metrics: TextMetrics,
    pub has_label: bool,
    pub wrapping_width: f64,
    pub node_constraint: Option<&'a str>,
    pub padding: f64,
    pub look_is_neo: bool,
    pub state_padding: f64,
    pub node_icon: Option<&'a str>,
    pub node_img: Option<&'a str>,
    pub node_pos: Option<&'a str>,
    pub node_asset_width: Option<f64>,
    pub node_asset_height: Option<f64>,
}

impl NodeLayoutDimensionsRequest<'_> {
    pub fn measure(
        self,
        rectangle_size: [Option<f64>; 2],
        hand_drawn: bool,
        prepared: Option<(&FlowchartSvgLabelSidecarBuilder, FlowchartSvgLabelOwner)>,
        charge_work: impl FnMut(usize) -> Result<()>,
    ) -> Result<((f64, f64), Option<PreparedFlowchartNodeGeometry>)> {
        let shape = resolve_flowchart_shape(self.layout_shape.unwrap_or("squareRect"))?;
        let (geometry, size) = if shape == FlowchartShape::ImageSquare
            && let Some(href) = self.node_img.filter(|href| !href.trim().is_empty())
        {
            let image = ImageSquareGeometry::resolve(
                ImageSquareSizing {
                    href,
                    width: self.node_asset_width,
                    height: self.node_asset_height,
                    constraint_on: self.node_constraint == Some("on"),
                },
                self.metrics,
                self.has_label,
                self.wrapping_width,
                self.node_pos == Some("t"),
                charge_work,
            )?;
            (
                PreparedFlowchartNodeGeometry::ImageSquare(image),
                image.outer,
            )
        } else {
            let label = self.metrics;
            let natural = node_layout_dimensions(self, shape);
            if rectangle_size == [None; 2]
                || hand_drawn
                || !matches!(
                    shape,
                    FlowchartShape::Process | FlowchartShape::RoundedRectangle
                )
            {
                return Ok((natural, None));
            }
            let rectangle = FlowchartRectangleGeometry::from_style(natural, label, rectangle_size);
            (
                PreparedFlowchartNodeGeometry::Rectangle(rectangle),
                rectangle.group.size(),
            )
        };
        if let Some((builder, owner)) = prepared {
            builder.record_node_geometry(owner, geometry)?;
        }
        Ok(((size.width, size.height), Some(geometry)))
    }
}

fn node_layout_dimensions(
    req: NodeLayoutDimensionsRequest<'_>,
    resolved_shape: FlowchartShape,
) -> (f64, f64) {
    let NodeLayoutDimensionsRequest {
        layout_shape,
        layout_direction,
        metrics,
        has_label,
        padding,
        look_is_neo,
        state_padding,
        node_icon,
        node_asset_width,
        node_asset_height,
        ..
    } = req;

    let shape = layout_shape.unwrap_or("squareRect");

    if let FlowchartShape::Bpmn(shape) = resolved_shape {
        let geometry = shape.geometry(
            kurbo::Size::new(metrics.width, metrics.height),
            has_label,
            node_icon.is_some_and(|icon| !icon.is_empty()),
            false,
        );
        return (geometry.width, geometry.height);
    }

    if matches!(
        resolved_shape,
        FlowchartShape::Icon
            | FlowchartShape::IconCircle
            | FlowchartShape::IconRounded
            | FlowchartShape::IconSquare
    ) {
        let label_padding = if has_label { 8.0 } else { 0.0 };
        let label_bbox_w = if has_label { metrics.width + 4.0 } else { 0.0 };
        let label_bbox_h = if has_label { metrics.height + 4.0 } else { 0.0 };

        let asset_h = node_asset_height.unwrap_or(48.0);
        let asset_w = node_asset_width.unwrap_or(48.0);
        let icon_size = asset_h.max(asset_w);
        let has_icon = node_icon.is_some_and(|icon| !icon.trim().is_empty());
        let icon_outer_size = match resolved_shape {
            FlowchartShape::IconCircle => {
                let icon_bbox_size = if has_icon { icon_size } else { 0.0 };
                icon_bbox_size * std::f64::consts::SQRT_2 + 40.0
            }
            FlowchartShape::IconRounded | FlowchartShape::IconSquare => icon_size + padding,
            FlowchartShape::Icon => icon_size,
            _ => unreachable!("the icon branch excludes non-icon shapes"),
        };

        let outer_w = icon_outer_size.max(label_bbox_w);
        let outer_h = icon_outer_size + label_padding + label_bbox_h;

        return (outer_w, outer_h);
    }

    // Mermaid `forkJoin.ts` inflates the Dagre node dimensions by `state.padding / 2` after
    // `updateNodeBounds(...)`, but does not re-render the rectangle with the inflated size.
    // Horizontal flow (LR or RL) uses a vertical bar, perpendicular to the flow.
    // Keep our layout spacing consistent with upstream by applying both rules here.
    if matches!(shape, "fork" | "join") {
        let extra = (state_padding / 2.0).max(0.0);
        let (render_w, render_h) = if layout_direction.eq_ignore_ascii_case("LR")
            || layout_direction.eq_ignore_ascii_case("RL")
        {
            (10.0, 70.0)
        } else {
            (70.0, 10.0)
        };
        return (render_w + extra, render_h + extra);
    }

    let (render_w, render_h) = node_render_dimensions(Some(shape), metrics, padding, look_is_neo);

    // Mermaid flowchart-v2 renders nodes using the "rendering-elements" layer:
    // 1) it generates SVG paths (roughjs-based even for non-handDrawn look),
    // 2) calls `updateNodeBounds(node, shapeElem)` which sets `node.width/height` from `getBBox()`,
    // 3) then feeds those updated dimensions into Dagre for layout.
    //
    // StadiumGeometry already owns the source dimensions and sampled points. Keep its
    // theoretical width here; re-sampling would shrink the same source geometry twice.

    (render_w, render_h)
}

#[cfg(test)]
#[path = "dimensions/tests.rs"]
mod render_dimension_tests;
