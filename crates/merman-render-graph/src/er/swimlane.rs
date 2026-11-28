//! ER Swimlane lane paint geometry.

use merman_render_core::{LayoutLabel, SwimlaneDirection, SwimlaneLaneLayout};

/// Prepared ER lane paint data. Label anchors remain on the ER relationships; lane metadata
/// carries the content boundary and parent roles that ordinary LayoutCluster does not expose.
#[derive(Debug, Clone)]
pub struct ErSwimlanePaintLayout {
    pub direction: SwimlaneDirection,
    pub lanes: Vec<SwimlaneLaneLayout>,
}

pub struct ErSwimlaneLaneGeometry {
    pub width: f64,
    pub height: f64,
    pub title_band_width: f64,
    pub title_band_height: f64,
    pub title_label: LayoutLabel,
}

impl ErSwimlanePaintLayout {
    /// Geometry from rendering-elements/clusters/swimlane.js, shared by bounds and painting.
    pub fn lane_geometry(&self, lane: &SwimlaneLaneLayout) -> ErSwimlaneLaneGeometry {
        let label_width = lane.title_label_width.max(0.0);
        let label_height = lane.title_label_height.max(0.0);
        let is_lr = self.direction == SwimlaneDirection::Lr;
        let title_size = label_height + if is_lr { 8.0 } else { 0.0 };
        let width = lane.width.max(label_width + lane.padding.max(0.0));
        let height = lane.height.max(0.0);
        let top = lane.y - height / 2.0;
        if is_lr {
            ErSwimlaneLaneGeometry {
                width,
                height,
                title_band_width: title_size,
                title_band_height: height,
                title_label: LayoutLabel {
                    x: lane.x - width / 2.0 + title_size / 2.0,
                    y: lane.y,
                    width: label_height,
                    height: label_width,
                },
            }
        } else {
            let content_top = lane.content_top.unwrap_or(top + height / 3.0);
            let title_height = title_size.min((content_top - top).max(0.0));
            ErSwimlaneLaneGeometry {
                width,
                height,
                title_band_width: width,
                title_band_height: title_height,
                title_label: LayoutLabel {
                    x: lane.x,
                    y: top + title_height / 2.0,
                    width: label_width,
                    height: label_height,
                },
            }
        }
    }
}
