//! Container geometry shared by layout projection, route queries, and SVG emission.

use kurbo::{Point, Rect, Size, Vec2};
use merman_render_core::{LayoutCluster, LayoutLabel, SwimlaneDirection, SwimlaneLaneLayout};

/// Painted extents shared by the cluster shell and the geometry-only Rough advance.
pub struct PaintedClusterFrame {
    pub origin: Point,
    pub size: Size,
}

impl PaintedClusterFrame {
    pub fn resolve(
        cluster: &LayoutCluster,
        width: f64,
        lane_or_pool: bool,
        [css_width, css_height]: [Option<f64>; 2],
    ) -> Self {
        let natural = Size::new(width, cluster.height);
        let origin = Point::new(
            cluster.x - natural.width / 2.0,
            cluster.y - natural.height / 2.0,
        );
        // CSS dimensions resize the shape at the original layout position.
        let size = if lane_or_pool {
            natural
        } else {
            Size::new(
                css_width.unwrap_or_else(|| natural.width.max(1.0)),
                css_height.unwrap_or_else(|| natural.height.max(1.0)),
            )
        };
        Self { origin, size }
    }

    pub fn in_root(mut self, translation: Vec2, root_origin: Point) -> Self {
        self.origin.x = self.origin.x + translation.x - root_origin.x;
        self.origin.y = self.origin.y + translation.y - root_origin.y;
        self
    }

    pub fn bounds(&self) -> Rect {
        Rect::from_origin_size(self.origin, self.size)
    }
}

pub fn cluster_container_radius(native_radius: Option<f64>, agentflow: bool, size: Size) -> f64 {
    let radius = native_radius.unwrap_or(if agentflow { 10.0 } else { 0.0 });
    radius.min(size.width.min(size.height) * 0.5).max(0.0)
}

/// Visible lane and title geometry after applying the selected layout contract.
#[derive(Debug, Clone)]
pub struct SwimlaneTerminalGeometry {
    pub width: f64,
    pub height: f64,
    title_band_width: f64,
    title_band_height: f64,
    pub title_label: LayoutLabel,
}

impl SwimlaneTerminalGeometry {
    pub fn frame_parts(
        &self,
        center: (f64, f64),
        direction: SwimlaneDirection,
        canonical: bool,
    ) -> (Rect, Rect) {
        use SwimlaneDirection::{Bt, Lr, Rl};

        let frame = Rect::from_center_size(center, (self.width, self.height));
        let mut body = frame;
        let mut title = frame;
        match direction {
            Lr => {
                title.x1 = frame.x0 + self.title_band_width;
                body.x0 = title.x1;
            }
            Rl if canonical => {
                title.x0 = frame.x1 - self.title_band_width;
                body.x1 = title.x0;
            }
            Bt if canonical => {
                title.y0 = frame.y1 - self.title_band_height;
                body.y1 = title.y0;
            }
            _ => {
                title.y1 = frame.y0 + self.title_band_height;
                body.y0 = title.y1;
            }
        }
        (body, title)
    }
}

pub fn swimlane_terminal_geometry(
    lane: &SwimlaneLaneLayout,
    direction: SwimlaneDirection,
    canonical: bool,
) -> SwimlaneTerminalGeometry {
    if canonical {
        use SwimlaneDirection::*;
        let (width, height) = (lane.width, lane.height);
        let (label, title_band_width, title_band_height) = lane.title_rect.as_ref().map_or_else(
            || {
                (
                    LayoutLabel {
                        x: lane.x,
                        y: lane.y,
                        width: 0.0,
                        height: 0.0,
                    },
                    0.0,
                    0.0,
                )
            },
            |rect| {
                let label = LayoutLabel {
                    x: rect.left.midpoint(rect.right),
                    y: rect.top.midpoint(rect.bottom),
                    width: rect.right - rect.left,
                    height: rect.bottom - rect.top,
                };
                let x = label.x - lane.x;
                let y = label.y - lane.y;
                let (w, h) = match direction {
                    Lr => (width + 2.0 * x, height),
                    Rl => (width - 2.0 * x, height),
                    Tb => (width, height + 2.0 * y),
                    Bt => (width, height - 2.0 * y),
                };
                (label, w.clamp(0.0, width), h.clamp(0.0, height))
            },
        );
        return SwimlaneTerminalGeometry {
            width,
            height,
            title_band_width,
            title_band_height,
            title_label: label,
        };
    }
    let label_width = lane.title_label_width.max(0.0);
    let label_height = lane.title_label_height.max(0.0);
    let padding = lane.padding.max(0.0);
    let is_lr = direction == SwimlaneDirection::Lr;
    let title_padding_y = if is_lr { 4.0 } else { 0.0 };
    let desired_title_size = label_height + 2.0 * title_padding_y;
    let width =
        lane.width
            .max(label_width + padding)
            .max(if is_lr { desired_title_size } else { 0.0 });
    let height = lane.height.max(0.0);
    let lane_top = lane.y - height / 2.0;

    if is_lr {
        let title_band_width = desired_title_size;
        SwimlaneTerminalGeometry {
            width,
            height,
            title_band_width,
            title_band_height: height,
            title_label: LayoutLabel {
                x: lane.x - width / 2.0 + title_band_width / 2.0,
                y: lane.y,
                width: label_height,
                height: label_width,
            },
        }
    } else {
        let content_top = lane.content_top.unwrap_or(lane_top + height / 3.0);
        let title_band_height = desired_title_size.min((content_top - lane_top).max(0.0));
        SwimlaneTerminalGeometry {
            width,
            height,
            title_band_width: width,
            title_band_height,
            title_label: LayoutLabel {
                x: lane.x,
                y: lane_top + title_band_height / 2.0,
                width: label_width,
                height: label_height,
            },
        }
    }
}
