use crate::model::LayoutPoint;
use crate::text::TextMetrics;

pub(crate) const EVENT_DIAMETER: f64 = 36.0;
pub(crate) const GATEWAY_SIZE: f64 = 50.0;
pub(crate) const ICON_SIZE: f64 = 16.0;
pub(crate) const LABEL_GAP: f64 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BpmnShape {
    Start,
    Intermediate,
    Boundary,
    End,
    Gateway,
    Activity,
    Data,
    DataStore,
    Annotation,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BpmnGeometry {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) drawn_width: f64,
    pub(crate) drawn_height: f64,
    pub(crate) label_center_y: f64,
}

pub(crate) fn wrapping_width(shape: Option<&str>, fallback: f64) -> f64 {
    if shape == Some("bpmn-annotation") {
        160.0
    } else {
        fallback
    }
}

impl BpmnShape {
    pub(crate) fn resolve(name: &str) -> Option<Self> {
        Some(match name {
            "bpmn-start" => Self::Start,
            "bpmn-intermediate" => Self::Intermediate,
            "bpmn-boundary" => Self::Boundary,
            "bpmn-end" => Self::End,
            "bpmn-gateway" => Self::Gateway,
            "bpmn-activity" => Self::Activity,
            "bpmn-data" => Self::Data,
            "bpmn-data-store" => Self::DataStore,
            "bpmn-annotation" => Self::Annotation,
            _ => return None,
        })
    }

    pub(crate) fn classes(self) -> &'static str {
        match self {
            Self::Start => "bpmn-event bpmn-event-start",
            Self::Intermediate => "bpmn-event bpmn-event-intermediate",
            Self::Boundary => "bpmn-event bpmn-event-boundary",
            Self::End => "bpmn-event bpmn-event-end",
            Self::Gateway => "bpmn-gateway",
            Self::Activity => "bpmn-activity",
            Self::Data => "bpmn-data-object",
            Self::DataStore => "bpmn-data-store",
            Self::Annotation => "bpmn-annotation",
        }
    }

    pub(crate) fn event_rings(self) -> Option<(usize, f64)> {
        match self {
            Self::Start => Some((1, 1.6)),
            Self::Intermediate | Self::Boundary => Some((2, 1.4)),
            Self::End => Some((1, 3.4)),
            _ => None,
        }
    }

    pub(crate) fn geometry(
        self,
        metrics: TextMetrics,
        has_label: bool,
        has_icon: bool,
        has_markers: bool,
    ) -> BpmnGeometry {
        let text_width = metrics.width.max(0.0);
        let text_height = metrics.height.max(0.0);
        if self == Self::Activity {
            let top_inset = if has_icon { ICON_SIZE + 6.0 } else { 10.0 };
            let bottom_inset = if has_markers { ICON_SIZE + 4.0 } else { 10.0 };
            let width = 100.0_f64.max(text_width + 16.0);
            let height = 80.0_f64.max(text_height + top_inset + bottom_inset);
            return BpmnGeometry {
                width,
                height,
                drawn_width: width,
                drawn_height: height,
                label_center_y: (top_inset - bottom_inset) / 2.0,
            };
        }
        if self == Self::Annotation {
            let width = text_width + 22.0;
            let height = 28.0_f64.max(text_height + 16.0);
            return BpmnGeometry {
                width,
                height,
                drawn_width: width,
                drawn_height: height,
                label_center_y: 0.0,
            };
        }
        let (drawn_width, drawn_height) = match self {
            Self::Start | Self::Intermediate | Self::Boundary | Self::End => {
                (EVENT_DIAMETER, EVENT_DIAMETER)
            }
            Self::Gateway => (GATEWAY_SIZE, GATEWAY_SIZE),
            Self::Data => (36.0, 50.0),
            Self::DataStore => (50.0, 50.0),
            Self::Activity | Self::Annotation => unreachable!(),
        };
        let has_caption = has_label && text_height > 0.0;
        let caption_band = if has_caption {
            LABEL_GAP + text_height
        } else {
            0.0
        };
        BpmnGeometry {
            width: drawn_width.max(if has_caption { text_width } else { 0.0 }),
            height: drawn_height + 2.0 * caption_band,
            drawn_width,
            drawn_height,
            label_center_y: if has_caption {
                drawn_height / 2.0 + LABEL_GAP + text_height / 2.0
            } else {
                0.0
            },
        }
    }

    pub(crate) fn intersect(
        self,
        center: &LayoutPoint,
        width: f64,
        height: f64,
        point: &LayoutPoint,
        annotation_right: bool,
    ) -> LayoutPoint {
        match self {
            Self::Start | Self::Intermediate | Self::Boundary | Self::End => {
                ring_intersect(center, EVENT_DIAMETER, point)
            }
            Self::Gateway => face_centre_intersect(center, GATEWAY_SIZE, GATEWAY_SIZE, point),
            Self::Data => face_project_intersect(center, 36.0, 50.0, point, None),
            Self::DataStore => face_project_intersect(center, 50.0, 50.0, point, None),
            Self::Activity => face_project_intersect(center, width, height, point, None),
            Self::Annotation => {
                face_project_intersect(center, width, height, point, Some(annotation_right))
            }
        }
    }
}

fn ring_intersect(center: &LayoutPoint, diameter: f64, point: &LayoutPoint) -> LayoutPoint {
    let radius = diameter / 2.0;
    let delta_x = point.x - center.x;
    let delta_y = point.y - center.y;
    let reach = radius * 0.85;
    let positive_sign = |value: f64| if value < 0.0 { -1.0 } else { 1.0 };
    if delta_y.abs() >= delta_x.abs() {
        let along = delta_x.clamp(-reach, reach);
        LayoutPoint {
            x: center.x + along,
            y: center.y + positive_sign(delta_y) * (radius * radius - along * along).sqrt(),
        }
    } else {
        let along = delta_y.clamp(-reach, reach);
        LayoutPoint {
            x: center.x + positive_sign(delta_x) * (radius * radius - along * along).sqrt(),
            y: center.y + along,
        }
    }
}

fn face_centre_intersect(
    center: &LayoutPoint,
    width: f64,
    height: f64,
    point: &LayoutPoint,
) -> LayoutPoint {
    let delta_x = point.x - center.x;
    let delta_y = point.y - center.y;
    if delta_x.abs() * height > delta_y.abs() * width {
        LayoutPoint {
            x: center.x + delta_x.signum() * width / 2.0,
            y: center.y,
        }
    } else {
        LayoutPoint {
            x: center.x,
            y: center.y
                + if delta_y < 0.0 {
                    -height / 2.0
                } else {
                    height / 2.0
                },
        }
    }
}

fn face_project_intersect(
    center: &LayoutPoint,
    width: f64,
    height: f64,
    point: &LayoutPoint,
    fixed_right: Option<bool>,
) -> LayoutPoint {
    let half_width = width / 2.0;
    let half_height = height / 2.0;
    let delta_x = point.x - center.x;
    let delta_y = point.y - center.y;
    let reach = |half: f64| (half - 8.0_f64.min(half / 2.0)).max(0.0);
    if fixed_right.is_some() || delta_x.abs() * half_height > delta_y.abs() * half_width {
        let right = fixed_right.unwrap_or(delta_x >= 0.0);
        LayoutPoint {
            x: center.x + if right { half_width } else { -half_width },
            y: point
                .y
                .clamp(center.y - reach(half_height), center.y + reach(half_height)),
        }
    } else {
        LayoutPoint {
            x: point
                .x
                .clamp(center.x - reach(half_width), center.x + reach(half_width)),
            y: center.y
                + if delta_y >= 0.0 {
                    half_height
                } else {
                    -half_height
                },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(width: f64, height: f64) -> TextMetrics {
        TextMetrics {
            width,
            height,
            line_count: 1,
        }
    }

    #[test]
    fn bpmn_caption_bounds_and_body_sizes_follow_the_pinned_source() {
        for (shape, width, height) in [
            (BpmnShape::Start, 36.0, 82.359375),
            (BpmnShape::Intermediate, 36.0, 82.359375),
            (BpmnShape::Boundary, 36.0, 82.359375),
            (BpmnShape::End, 36.0, 82.359375),
            (BpmnShape::Gateway, 50.0, 96.359375),
            (BpmnShape::Activity, 100.0, 80.0),
            (BpmnShape::Data, 36.0, 96.359375),
            (BpmnShape::DataStore, 50.0, 96.359375),
            (BpmnShape::Annotation, 54.921875, 33.1796875),
        ] {
            let measured = shape.geometry(metrics(32.921875, 17.1796875), true, false, false);
            assert_eq!(
                (measured.width, measured.height),
                (width, height),
                "{shape:?}"
            );
        }
        let long_caption = BpmnShape::Start.geometry(metrics(500.0, 24.0), true, false, false);
        assert_eq!((long_caption.width, long_caption.height), (500.0, 96.0));
        assert_eq!(
            (long_caption.drawn_width, long_caption.drawn_height),
            (36.0, 36.0)
        );
        assert_eq!(long_caption.label_center_y, 36.0);
        let empty = BpmnShape::Start.geometry(metrics(0.0, 24.0), false, false, false);
        assert_eq!((empty.width, empty.height), (36.0, 36.0));
        let activity = BpmnShape::Activity.geometry(metrics(120.0, 100.0), true, true, true);
        assert_eq!((activity.width, activity.height), (136.0, 142.0));
        assert_eq!(activity.label_center_y, 1.0);
        assert_eq!(wrapping_width(Some("bpmn-annotation"), 80.0), 160.0);
        assert_eq!(wrapping_width(Some("bpmn-start"), 80.0), 80.0);
    }

    #[test]
    fn bpmn_ring_intersection_preserves_the_arrival_axis_and_source_ties() {
        let center = LayoutPoint { x: 10.0, y: 20.0 };
        for point in [
            LayoutPoint { x: 10.0, y: 200.0 },
            LayoutPoint { x: 10.0, y: -200.0 },
            LayoutPoint { x: 300.0, y: 20.0 },
            LayoutPoint { x: -300.0, y: 20.0 },
            LayoutPoint { x: 40.0, y: 220.0 },
            LayoutPoint { x: -20.0, y: 220.0 },
            LayoutPoint { x: 310.0, y: 40.0 },
            LayoutPoint { x: -290.0, y: 0.0 },
            center.clone(),
        ] {
            let hit = BpmnShape::Start.intersect(&center, 500.0, 200.0, &point, false);
            assert!(((hit.x - center.x).hypot(hit.y - center.y) - 18.0).abs() < 1e-9);
        }
        let offset = ring_intersect(&center, 36.0, &LayoutPoint { x: 15.0, y: 220.0 });
        assert_eq!(offset.x, 15.0);
        assert!((offset.y - 37.29161646579058).abs() < 1e-9);
        let diagonal = ring_intersect(&center, 36.0, &LayoutPoint { x: 110.0, y: 120.0 });
        assert!((diagonal.x - 25.3).abs() < 1e-9);
        let origin = ring_intersect(&center, 36.0, &center);
        assert_eq!((origin.x, origin.y), (10.0, 38.0));
    }

    #[test]
    fn bpmn_elk_caption_projection_matches_the_pinned_twenty_step_oracle() {
        use crate::elk_edge_geometry::{Outline, Shape, sanitize};
        use crate::model::LayoutNode;

        let center = LayoutPoint {
            x: 128.1328125,
            y: 75.4,
        };
        let bounds = LayoutNode {
            id: "event".to_owned(),
            x: center.x,
            y: center.y,
            width: 232.265625,
            height: 112.8,
            is_cluster: false,
            label_width: None,
            label_height: None,
        };
        let port = LayoutPoint {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y,
        };
        let next = LayoutPoint {
            x: port.x + 40.0,
            y: port.y,
        };
        let target = LayoutNode {
            id: "target".to_owned(),
            x: next.x + 20.0,
            width: 40.0,
            height: 36.0,
            ..bounds.clone()
        };
        let intersection = |point: &LayoutPoint| {
            BpmnShape::Start.intersect(&center, bounds.width, bounds.height, point, false)
        };
        let direct = intersection(&port);
        assert_eq!((direct.x, direct.y), (146.1328125, 75.4));
        let projected = sanitize(
            &[
                center.clone(),
                port,
                next,
                LayoutPoint {
                    x: target.x,
                    y: target.y,
                },
            ],
            Shape {
                node: &bounds,
                outline: Outline::Rect,
                intersection: Some(&intersection),
            },
            Shape {
                node: &target,
                outline: Outline::Rect,
                intersection: None,
            },
        );
        assert_eq!((projected[0].x, projected[0].y), (146.13270290941, 75.4));
    }

    #[test]
    fn bpmn_gateway_and_projected_faces_target_the_body_independently_of_captions() {
        let center = LayoutPoint { x: 100.0, y: 100.0 };
        let diagonal = BpmnShape::Gateway.intersect(
            &center,
            500.0,
            200.0,
            &LayoutPoint { x: 200.0, y: 200.0 },
            false,
        );
        assert_eq!((diagonal.x, diagonal.y), (100.0, 125.0));
        let corner = BpmnShape::Activity.intersect(
            &center,
            100.0,
            60.0,
            &LayoutPoint { x: 160.0, y: 400.0 },
            false,
        );
        assert_eq!((corner.x, corner.y), (142.0, 130.0));
        for (right, expected) in [(false, 50.0), (true, 150.0)] {
            let annotation = BpmnShape::Annotation.intersect(
                &center,
                100.0,
                60.0,
                &LayoutPoint { x: 400.0, y: 100.0 },
                right,
            );
            assert_eq!((annotation.x, annotation.y), (expected, 100.0));
        }
        let data = BpmnShape::Data.intersect(
            &center,
            500.0,
            200.0,
            &LayoutPoint {
                x: 1000.0,
                y: 100.0,
            },
            false,
        );
        assert_eq!((data.x, data.y), (118.0, 100.0));
    }
}
