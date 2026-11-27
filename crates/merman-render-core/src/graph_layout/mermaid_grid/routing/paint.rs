//! After-paint Grid line jumps (grid/lineHops.ts and rendering-elements/lineJump.ts).

use super::{invalid_geometry, reserve_vec};
use crate::Result;
use crate::graph_layout::mermaid_grid::GridLineHops;
use crate::resources::OperationWorkMeter;
use crate::svg_edge_parity::line_hops::{
    LineHopConfig, LineHopEdge, LineHopStyle, curve_supports_line_hops,
    line_hop_overrides_with_radii,
};
use crate::text::is_ecmascript_whitespace;
use merman_model::OperationPhase;

const GRID_HOP_RADIUS: f64 = 6.0;
const DEFAULT_CORNER_RADIUS: f64 = 5.0;

#[derive(Clone, Copy, Debug)]
pub struct GridPaintedEdge<'a> {
    /// The unique rendered edge ID and its draw-projected `data-points` before marker offsets.
    pub geometry: LineHopEdge<'a>,
    /// Resolved per-edge corner radius from initial paint, including a possible zero radius.
    pub corner_radius: Option<f64>,
    /// Used to decide whether a path with no curve hint can be rewritten.
    pub original_path: &'a str,
}

/// Use the rendered paths in source edge order. Only returned slots get a new `d` attribute.
/// The painter retains marker attributes and can run the shared Neo stroke-mask rewrite on each
/// override. No projected result feeds back into router occupancy or label anchors.
pub fn grid_line_hop_overrides(
    setting: GridLineHops,
    rendered: &[GridPaintedEdge<'_>],
    work: &OperationWorkMeter,
) -> Result<Vec<(usize, String)>> {
    let jump_style = match setting {
        GridLineHops::Disabled => return Ok(Vec::new()),
        GridLineHops::Arc => LineHopStyle::Arc,
        GridLineHops::Gap => LineHopStyle::Gap,
    };
    work.charge_emit_work(rendered.len())?;
    let mut edges = reserve_vec(rendered.len())?;
    for edge in rendered {
        work.charge_emit_work(edge.geometry.points.len())?;
        if edge
            .geometry
            .points
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(invalid_geometry());
        }
        edges.push(edge.geometry);
    }
    line_hop_overrides_with_radii(
        &edges,
        LineHopConfig {
            jump_radius: GRID_HOP_RADIUS,
            jump_style,
        },
        |index, edge| {
            if let Some(curve) = edge.curve {
                Ok(curve.is_empty() || curve_supports_line_hops(Some(curve)))
            } else {
                let path = rendered[index].original_path;
                work.charge_emit_work(path.len())?;
                Ok(path.chars().all(|ch| {
                    ch.is_ascii_digit()
                        || is_ecmascript_whitespace(ch)
                        || matches!(ch, '+' | ',' | '.' | 'L' | 'M' | 'e' | 'l' | 'm' | '-')
                }))
            }
        },
        |index| {
            rendered[index]
                .corner_radius
                .filter(|&radius| radius.is_finite() && radius >= 0.0)
                .unwrap_or(DEFAULT_CORNER_RADIUS)
        },
        OperationPhase::Emit,
        work,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;
    use crate::{Error, LayoutPoint};

    const ORIGINAL: &str = "M4,20 L83.029,20 Q100,20 100,36.971 L100,96";

    fn meter() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    fn edge<'a>(id: &'a str, points: &'a [LayoutPoint]) -> GridPaintedEdge<'a> {
        GridPaintedEdge {
            geometry: LineHopEdge {
                id,
                points,
                curve: Some("rounded"),
                arrow_type_start: None,
                arrow_type_end: None,
            },
            corner_radius: Some(12.0),
            original_path: ORIGINAL,
        }
    }

    #[test]
    fn grid_arc_and_gap_rewrite_only_the_crossing_shaft_with_its_corner_radius() {
        let horizontal = [
            LayoutPoint { x: 0.0, y: 20.0 },
            LayoutPoint { x: 100.0, y: 20.0 },
            LayoutPoint { x: 100.0, y: 100.0 },
        ];
        let vertical = [
            LayoutPoint { x: 40.0, y: 0.0 },
            LayoutPoint { x: 40.0, y: 60.0 },
        ];
        let mut edge_a = edge("horizontal", &horizontal);
        edge_a.geometry.arrow_type_start = Some("arrow_point");
        edge_a.geometry.arrow_type_end = Some("arrow_point");
        let edges = [edge_a, edge("vertical", &vertical)];
        let arc = "M4,20 L34,20 A6,6 0 0 1 46,20 L83.029,20 Q100,20 100,36.971 L100,96";
        let gap = "M4,20 L34,20 M46,20 L83.029,20 Q100,20 100,36.971 L100,96";
        for (setting, expected) in [(GridLineHops::default(), arc), (GridLineHops::Gap, gap)] {
            assert_eq!(
                grid_line_hop_overrides(setting, &edges, &meter()).unwrap(),
                [(0, expected.to_owned())]
            );
        }
        let work = meter();
        assert!(
            grid_line_hop_overrides(GridLineHops::Disabled, &edges, &work)
                .unwrap()
                .is_empty()
        );
        assert_eq!(work.used(), 0);
    }

    #[test]
    fn grid_hops_skip_corner_cuts_and_decline_smoothing_rewrites() {
        let horizontal = [
            LayoutPoint { x: 0.0, y: 20.0 },
            LayoutPoint { x: 100.0, y: 20.0 },
            LayoutPoint { x: 100.0, y: 100.0 },
        ];
        let vertical = [
            LayoutPoint { x: 86.0, y: 0.0 },
            LayoutPoint { x: 86.0, y: 60.0 },
        ];
        let mut edge_a = edge("horizontal", &horizontal);
        let mut edge_b = edge("vertical", &vertical);
        assert!(
            grid_line_hop_overrides(GridLineHops::Arc, &[edge_a, edge_b], &meter())
                .unwrap()
                .is_empty()
        );

        edge_a.corner_radius = None;
        edge_b.geometry.curve = Some("basis");
        let overrides =
            grid_line_hop_overrides(GridLineHops::Arc, &[edge_a, edge_b], &meter()).unwrap();
        assert_eq!(overrides.as_slice(), &[(0, "M0,20 L81.071,20 A4.929,4.929 0 0 1 90.929,20 L92.929,20 Q100,20 100,27.071 L100,100".into())]);

        edge_a.geometry.curve = Some("basis");
        assert!(
            grid_line_hop_overrides(GridLineHops::Arc, &[edge_a, edge_b], &meter())
                .unwrap()
                .is_empty()
        );
        edge_a.geometry.curve = None;
        assert!(
            grid_line_hop_overrides(GridLineHops::Arc, &[edge_a, edge_b], &meter())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn grid_hops_exclude_endpoint_contacts_and_observe_emit_cancellation() {
        let horizontal = [
            LayoutPoint { x: 0.0, y: 20.0 },
            LayoutPoint { x: 100.0, y: 20.0 },
        ];
        let vertical = [
            LayoutPoint { x: 40.0, y: 20.0 },
            LayoutPoint { x: 40.0, y: 60.0 },
        ];
        let edges = [edge("horizontal", &horizontal), edge("vertical", &vertical)];
        assert!(
            grid_line_hop_overrides(GridLineHops::Arc, &edges, &meter())
                .unwrap()
                .is_empty()
        );
        let work = meter();
        work.control().cancel();
        assert!(
            matches!(grid_line_hop_overrides(GridLineHops::Arc, &edges, &work), Err(Error::Cancelled(error)) if error.phase == OperationPhase::Emit)
        );
    }
}
