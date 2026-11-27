use super::UsecaseEdgePlan;
use kurbo::{BezPath, Point, Rect, Vec2};
use merman_render_core::{LayoutPoint, graph_layout::Marker};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Point,
    Circle,
    Cross,
    Extension,
}

pub struct UsecaseMarker {
    kind: Kind,
    start: bool,
    width: f64,
    height: f64,
    pub stroke_width: f64,
    pub ref_x: f64,
    footprint: Rect,
}

impl UsecaseMarker {
    pub fn for_edge(plan: &UsecaseEdgePlan, start: bool, neo: bool) -> Option<Self> {
        let name = if start {
            plan.start_marker.as_deref()
        } else {
            plan.end_marker.as_deref()
        }?;
        let neo = neo && !plan.animated;
        let (kind, width, height, stroke_width) = match (name, neo, start) {
            ("arrow_point", false, _) => (Kind::Point, 8.0, 8.0, 0.8),
            ("arrow_point", true, true) => (Kind::Point, 11.5, 14.0, 0.0),
            ("arrow_point", true, false) => (Kind::Point, 10.5, 14.0 * 10.5 / 11.5, 0.0),
            ("arrow_circle", false, _) => (Kind::Circle, 11.0, 11.0, 1.1),
            ("arrow_circle", true, _) => (Kind::Circle, 14.0, 14.0, 0.0),
            ("arrow_cross", false, _) => (Kind::Cross, 9.0, 9.0, 2.0),
            ("arrow_cross", true, _) => (Kind::Cross, 10.4, 10.4, 2.0),
            ("extension", false, _) => (Kind::Extension, 17.0, 12.0, 1.0),
            ("extension", true, _) => (Kind::Extension, 8.0, 12.0, 2.0),
            _ => return None,
        };
        let stroke_radius = stroke_width / 2.0;
        Some(Self {
            kind,
            start,
            width,
            height,
            stroke_width,
            ref_x: if kind == Kind::Extension {
                if start { width } else { 0.0 }
            } else {
                width / 2.0
            },
            footprint: Rect::new(0.0, 0.0, width, height).inflate(stroke_radius, stroke_radius),
        })
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            Kind::Point => "point",
            Kind::Circle => "circle",
            Kind::Cross => "cross",
            Kind::Extension => "extension",
        }
    }

    pub fn ref_y(&self) -> f64 {
        self.height / 2.0
    }

    pub fn dimensions(&self) -> (f64, f64) {
        (self.footprint.width(), self.footprint.height())
    }

    pub fn radius(&self) -> Option<f64> {
        (self.kind == Kind::Circle).then_some(self.width / 2.0)
    }

    pub fn path(&self) -> BezPath {
        let mut path = BezPath::new();
        let (back, tip) = if self.start {
            (self.width, 0.0)
        } else {
            (0.0, self.width)
        };
        if self.kind == Kind::Cross {
            path.move_to((back, 0.0));
            path.line_to((tip, self.height));
            path.move_to((back, self.height));
            path.line_to((tip, 0.0));
        } else {
            path.move_to((back, 0.0));
            path.line_to((tip, self.ref_y()));
            path.line_to((back, self.height));
            path.close_path();
        }
        path
    }

    pub fn metrics(&self) -> Marker {
        let before = self.ref_x - self.footprint.x0;
        let after = self.footprint.x1 - self.ref_x;
        let (trim, depth) = if self.start {
            (before, after)
        } else {
            (after, before)
        };
        Marker {
            trim,
            depth,
            width: self.footprint.height(),
        }
    }

    pub fn paint_bounds(&self, reference: &LayoutPoint, shoulder: &LayoutPoint) -> Rect {
        let reference = Point::new(reference.x, reference.y);
        let shoulder = Point::new(shoulder.x, shoulder.y);
        let along = if self.start {
            shoulder - reference
        } else {
            reference - shoulder
        }
        .normalize();
        let across = Vec2::new(-along.y, along.x);
        let mut bounds = Rect::from_points(reference, reference);
        for x in [self.footprint.x0, self.footprint.x1] {
            for y in [self.footprint.y0, self.footprint.y1] {
                let point = reference + along * (x - self.ref_x) + across * (y - self.ref_y());
                bounds = bounds.union_pt(point);
            }
        }
        bounds
    }
}
