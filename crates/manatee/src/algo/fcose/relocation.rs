//! FCoSE component relocation from Cytoscape element bounds and layout-base rectangles.

use super::{SimGraph, SimNode};
use crate::graph::{Anchor, LayoutRect};

// Mermaid 11.16 leaves these Cytoscape 3.33.3 bbox phases at their defaults. Keep body, label,
// parent, and final antialiasing expansion separate because they do not share the same outset.
const CYTOSCAPE_EDGE_BODY_WIDTH_PX: f64 = 3.0;
const CYTOSCAPE_EDGE_LABEL_MARGIN_OF_ERROR_PX: f64 = 2.0;
const CYTOSCAPE_PARENT_BODY_BORDER_WIDTH_PX: f64 = 1.0;
const CYTOSCAPE_FINAL_ELEMENT_BBOX_EXPANSION_PX: f64 = 1.0;
const CYTOSCAPE_EDGE_BODY_HALF_WIDTH_PX: f64 = CYTOSCAPE_EDGE_BODY_WIDTH_PX / 2.0;
const CYTOSCAPE_PARENT_BODY_NON_PADDING_BBOX_OUTSET_PX: f64 =
    CYTOSCAPE_PARENT_BODY_BORDER_WIDTH_PX / 2.0 + CYTOSCAPE_FINAL_ELEMENT_BBOX_EXPANSION_PX;

impl SimNode {
    fn bound_left(&self) -> f64 {
        self.left - self.bounds_extras.left.max(0.0)
    }

    fn bound_right(&self) -> f64 {
        self.right() + self.bounds_extras.right.max(0.0)
    }

    fn bound_top(&self) -> f64 {
        self.top - self.bounds_extras.top.max(0.0)
    }

    fn bound_bottom(&self) -> f64 {
        self.bottom() + self.bounds_extras.bottom.max(0.0)
    }
}

impl SimGraph {
    pub(super) fn translate(&mut self, dx: f64, dy: f64) {
        for n in &mut self.nodes {
            n.left += dx;
            n.top += dy;
        }
    }

    pub(super) fn bounding_box_center_rects(&self) -> Option<(f64, f64)> {
        self.layout_rect_bbox()
            .map(|r| (r.left + (r.width / 2.0), r.top + (r.height / 2.0)))
    }

    fn layout_rect_bbox(&self) -> Option<LayoutRect> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for n in &self.nodes {
            min_x = min_x.min(n.left);
            min_y = min_y.min(n.top);
            max_x = max_x.max(n.right());
            max_y = max_y.max(n.bottom());
        }
        if !(min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()) {
            return None;
        }
        Some(LayoutRect {
            left: min_x,
            top: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        })
    }

    pub(super) fn bounding_box_center_eles(&self, run_idx: usize) -> Option<(f64, f64)> {
        if self.nodes.is_empty() {
            return None;
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        // Nodes (incl. labels). Cytoscape `eles.boundingBox()` treats compound nodes as wrappers
        // around their child graphs, and `compound-sizing-wrt-labels: include` causes descendant
        // label extents to affect compound bounds.
        //
        // Model this by:
        // - using leaf `bound_*` as the base primitive; Architecture supplies compound-child
        //   extras for grouped leaves and final-element extras for top-level leaves
        // - computing compound bboxes bottom-up from immediate children (so compound padding
        //   stacks across deep nesting, as observed in Mermaid/Cytoscape)
        // - applying layout-base/cose-base child-graph padding, then Cytoscape's centered parent
        //   half-border and final whole-bbox expansion
        //
        // This keeps layout rects (used by the spring embedder) unchanged while making the
        // relocation origin (`eles.boundingBox()` center) match upstream.
        #[derive(Debug, Clone, Copy)]
        struct Bb {
            x1: f64,
            y1: f64,
            x2: f64,
            y2: f64,
        }

        impl Bb {
            fn union(self, other: Bb) -> Bb {
                Bb {
                    x1: self.x1.min(other.x1),
                    y1: self.y1.min(other.y1),
                    x2: self.x2.max(other.x2),
                    y2: self.y2.max(other.y2),
                }
            }

            fn inflate(self, pad: f64) -> Bb {
                Bb {
                    x1: self.x1 - pad,
                    y1: self.y1 - pad,
                    x2: self.x2 + pad,
                    y2: self.y2 + pad,
                }
            }

            fn centered(x: f64, y: f64, outset: f64) -> Bb {
                Bb {
                    x1: x - outset,
                    y1: y - outset,
                    x2: x + outset,
                    y2: y + outset,
                }
            }
        }

        fn leaf_bbox(n: &SimNode) -> Bb {
            let x1 = n.bound_left();
            let y1 = n.bound_top();
            let x2 = n.bound_right();
            let y2 = n.bound_bottom();
            Bb { x1, y1, x2, y2 }
        }

        let mut bbs: Vec<Option<Bb>> = vec![None; self.nodes.len()];
        for (idx, n) in self.nodes.iter().enumerate() {
            if !n.is_compound {
                bbs[idx] = Some(leaf_bbox(n));
            }
        }

        for &cidx in &self.compounds_deep_first {
            let Some(n) = self.nodes.get(cidx) else {
                continue;
            };
            if !n.is_compound {
                continue;
            }
            let children = self
                .children_by_owner
                .get(cidx)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            if children.is_empty() {
                // Empty compound: fall back to its rect (no label extras tracked for compounds).
                bbs[cidx] = Some(Bb {
                    x1: n.left,
                    y1: n.top,
                    x2: n.right(),
                    y2: n.bottom(),
                });
                continue;
            }
            let mut bb: Option<Bb> = None;
            for &ch in children {
                let ch_bb = bbs.get(ch).and_then(|v| *v).unwrap_or_else(|| {
                    let Some(cn) = self.nodes.get(ch) else {
                        return Bb {
                            x1: 0.0,
                            y1: 0.0,
                            x2: 0.0,
                            y2: 0.0,
                        };
                    };
                    Bb {
                        x1: cn.left,
                        y1: cn.top,
                        x2: cn.right(),
                        y2: cn.bottom(),
                    }
                });
                bb = Some(bb.map(|b| b.union(ch_bb)).unwrap_or(ch_bb));
            }
            let compound_bbox_outset =
                n.padding.max(0.0) + CYTOSCAPE_PARENT_BODY_NON_PADDING_BBOX_OUTSET_PX;
            bbs[cidx] = bb.map(|b| b.inflate(compound_bbox_outset));
        }

        let top_level = self
            .children_by_owner
            .get(self.root_owner_idx)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        for &idx in top_level {
            let Some(bb) = bbs.get(idx).and_then(|v| *v).or_else(|| {
                self.nodes.get(idx).map(|n| Bb {
                    x1: n.left,
                    y1: n.top,
                    x2: n.right(),
                    y2: n.bottom(),
                })
            }) else {
                continue;
            };
            min_x = min_x.min(bb.x1);
            min_y = min_y.min(bb.y1);
            max_x = max_x.max(bb.x2);
            max_y = max_y.max(bb.y2);
        }

        // Edges: Cytoscape `eles.boundingBox()` includes edge geometry. For Mermaid Architecture,
        // edge endpoints are manually specified as `{ 0/50%/100% }` offsets (see
        // `source-endpoint`/`target-endpoint` in Mermaid's Cytoscape stylesheet).
        //
        // Cytoscape resolves those endpoints by adding the pixel offset to `node.position()`
        // (even though `position()` represents the node center for normal geometry). Mermaid then
        // reuses that same `position()` value as the SVG top-left `translate(x,y)` for the icon.
        //
        // In our port we mirror upstream by treating `SimNode.center_{x,y}` as that top-left
        // anchor, and compute endpoint points as offsets from it (not as shape intersections).
        fn endpoint(n: &SimNode, anchor: Option<Anchor>) -> (f64, f64) {
            let ox = n.center_x();
            let oy = n.center_y();
            let w = n.width;
            let h = n.height;
            match anchor {
                Some(Anchor::Left) => (ox, oy + (h / 2.0)),
                Some(Anchor::Right) => (ox + w, oy + (h / 2.0)),
                Some(Anchor::Top) => (ox + (w / 2.0), oy),
                Some(Anchor::Bottom) => (ox + (w / 2.0), oy + h),
                None => (ox + (w / 2.0), oy + (h / 2.0)),
            }
        }

        fn polyline_midpoint(points: &[(f64, f64)]) -> Option<(f64, f64)> {
            if points.len() < 2 {
                return None;
            }
            let mut total = 0.0f64;
            let mut seg_lens: Vec<f64> = Vec::with_capacity(points.len().saturating_sub(1));
            for w in points.windows(2) {
                let dx = w[1].0 - w[0].0;
                let dy = w[1].1 - w[0].1;
                let len = (dx * dx + dy * dy).sqrt();
                seg_lens.push(len);
                total += len;
            }
            if !total.is_finite() || total <= 0.0 {
                return Some(points[0]);
            }
            let target = total / 2.0;
            let mut acc = 0.0f64;
            for (i, &len) in seg_lens.iter().enumerate() {
                if !len.is_finite() || len <= 0.0 {
                    continue;
                }
                if acc + len >= target {
                    let t = ((target - acc) / len).clamp(0.0, 1.0);
                    let (x0, y0) = points[i];
                    let (x1, y1) = points[i + 1];
                    return Some((x0 + (x1 - x0) * t, y0 + (y1 - y0) * t));
                }
                acc += len;
            }
            points.last().copied()
        }

        for e in &self.edges {
            let Some(a) = self.nodes.get(e.a) else {
                continue;
            };
            let Some(b) = self.nodes.get(e.b) else {
                continue;
            };
            let (sx, sy) = endpoint(a, e.a_anchor);
            let (tx, ty) = endpoint(b, e.b_anchor);

            let mut path_points: Vec<(f64, f64)> = vec![(sx, sy), (tx, ty)];
            let mut label_point_override: Option<(f64, f64)> = None;
            let mut edge_bounds = Bb::centered(sx, sy, CYTOSCAPE_EDGE_BODY_HALF_WIDTH_PX)
                .union(Bb::centered(tx, ty, CYTOSCAPE_EDGE_BODY_HALF_WIDTH_PX));

            // Mermaid styles XY edges as Cytoscape `curve-style: segments` with
            // `segment-weights: 0` and `segment-distances: 0.5px` in the pre-layout state. Other
            // diagonal edges remain `curve-style: straight`; their labels stay at the straight
            // midpoint even after Mermaid writes segment weights/distances during run chaining.
            if e.curve_style_segments && run_idx == 0 && sx != tx && sy != ty {
                const SEG_DIST: f64 = 0.5;
                let dx = tx - sx;
                let dy = ty - sy;
                let len = (dx * dx + dy * dy).sqrt();
                if len.is_finite() && len > 0.0 {
                    // Left-hand perpendicular, normalized.
                    let off_x = (-dy / len) * SEG_DIST;
                    let off_y = (dx / len) * SEG_DIST;
                    // `segment-weights: 0` => base point at source endpoint.
                    let px = sx + off_x;
                    let py = sy + off_y;
                    path_points.insert(1, (px, py));
                    // Cytoscape's pre-layout `segments` curve places the edge label near the
                    // segment control point. Using that point for bbox purposes matches the
                    // upstream `edge.boundingBox()` extents for diagonal Architecture edges.
                    label_point_override = Some((px, py));
                    edge_bounds =
                        edge_bounds.union(Bb::centered(px, py, CYTOSCAPE_EDGE_BODY_HALF_WIDTH_PX));
                }
            }

            // After the first run, Mermaid updates segment weights/distances for `edge.segments`
            // so XY edges become orthogonal with a single bend at either `(sx, ty)` or `(tx, sy)`.
            if e.curve_style_segments && run_idx > 0 && sx != tx && sy != ty {
                let (bx, by) = match e.a_anchor {
                    Some(Anchor::Top) | Some(Anchor::Bottom) => (sx, ty),
                    _ => (tx, sy),
                };
                path_points.insert(1, (bx, by));
                // After Mermaid updates segment weights/distances, the SVG renderer treats this
                // bend point as the "midpoint" of the orthogonal polyline. Cytoscape's edge label
                // placement for the same style is closest to this bend as well, so retain it as
                // this relocation bbox phase's label point.
                label_point_override = Some((bx, by));
                edge_bounds =
                    edge_bounds.union(Bb::centered(bx, by, CYTOSCAPE_EDGE_BODY_HALF_WIDTH_PX));
            }

            // Edge labels: Cytoscape includes label geometry inside `edge.boundingBox()`, and
            // `eles.boundingBox()` unions it into the overall component bbox.
            if let (Some(lw), Some(lh)) = (e.label_width, e.label_height) {
                let lw = lw.max(0.0);
                let lh = lh.max(0.0);
                if lw.is_finite() && lw > 0.0 && lh.is_finite() && lh > 0.0 {
                    let mp = label_point_override.or_else(|| polyline_midpoint(&path_points));
                    if let Some((mx, my)) = mp {
                        let hw = lw / 2.0;
                        let hh = lh / 2.0;
                        edge_bounds = edge_bounds.union(Bb {
                            x1: mx - hw - CYTOSCAPE_EDGE_LABEL_MARGIN_OF_ERROR_PX,
                            y1: my - hh - CYTOSCAPE_EDGE_LABEL_MARGIN_OF_ERROR_PX,
                            x2: mx + hw + CYTOSCAPE_EDGE_LABEL_MARGIN_OF_ERROR_PX,
                            y2: my + hh + CYTOSCAPE_EDGE_LABEL_MARGIN_OF_ERROR_PX,
                        });
                    }
                }
            }

            // Cytoscape expands the completed edge element bbox after body and label union.
            let edge_bounds = edge_bounds.inflate(CYTOSCAPE_FINAL_ELEMENT_BBOX_EXPANSION_PX);
            min_x = min_x.min(edge_bounds.x1);
            min_y = min_y.min(edge_bounds.y1);
            max_x = max_x.max(edge_bounds.x2);
            max_y = max_y.max(edge_bounds.y2);
        }

        if !(min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()) {
            return None;
        }
        Some(((min_x + max_x) / 2.0, (min_y + max_y) / 2.0))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{IndexedCompound, IndexedEdge, IndexedGraph, IndexedNode, SimGraph};
    use crate::graph::{Anchor, BoundsExtras};

    #[test]
    fn eles_bbox_keeps_edge_label_stages_across_both_relocation_runs() {
        let graph = IndexedGraph {
            nodes: vec![
                IndexedNode {
                    parent: None,
                    width: 40.0,
                    height: 40.0,
                    x: 0.0,
                    y: 0.0,
                    bounds_extras: BoundsExtras::default(),
                },
                IndexedNode {
                    parent: None,
                    width: 40.0,
                    height: 40.0,
                    x: 100.0,
                    y: 100.0,
                    bounds_extras: BoundsExtras::default(),
                },
            ],
            edges: vec![IndexedEdge {
                source: 0,
                target: 1,
                label_width: Some(200.0),
                label_height: Some(20.0),
                source_anchor: Some(Anchor::Right),
                target_anchor: Some(Anchor::Left),
                curve_style_segments: false,
                ideal_length: 80.0,
                elasticity: 0.45,
            }],
            compounds: Vec::new(),
        };

        let sim = SimGraph::from_indexed(&graph);
        let (straight_center_x, _) = sim
            .bounding_box_center_eles(1)
            .expect("straight bbox center");
        assert!(
            (straight_center_x - 70.0).abs() < 1e-9,
            "straight edge label should stay centered on the straight midpoint, got {straight_center_x}"
        );

        let mut segments_graph = graph;
        segments_graph.edges[0].curve_style_segments = true;
        for (label_width, expected_center_x) in
            [(None, 50.0), (Some(20.0), 50.0), (Some(200.0), 91.5)]
        {
            let mut case = segments_graph.clone();
            case.edges[0].label_width = label_width;
            case.edges[0].label_height = label_width.map(|_| 20.0);
            let sim = SimGraph::from_indexed(&case);
            let (center_x, _) = sim
                .bounding_box_center_eles(1)
                .expect("segments bbox center");
            assert!(
                (center_x - expected_center_x).abs() < 1e-9,
                "unexpected second-run center for label width {label_width:?}: {center_x}"
            );
        }

        let mut sim = SimGraph::from_indexed(&segments_graph);
        for (run_idx, expected_center_x) in [(0, 39.57125353714373), (1, 81.07125353714373)] {
            let orig_center = sim
                .bounding_box_center_eles(run_idx)
                .expect("relocation origin");
            assert!((orig_center.0 - expected_center_x).abs() < 1e-9);
            let current_center = sim
                .bounding_box_center_rects()
                .expect("current rect center");
            sim.translate(
                orig_center.0 - current_center.0,
                orig_center.1 - current_center.1,
            );
            assert_eq!(sim.bounding_box_center_rects(), Some(orig_center));
        }
    }

    #[test]
    fn cytoscape_relocation_bbox_keeps_body_parent_and_label_outsets_separate() {
        assert_eq!(super::CYTOSCAPE_EDGE_BODY_HALF_WIDTH_PX, 1.5);
        assert_eq!(super::CYTOSCAPE_EDGE_LABEL_MARGIN_OF_ERROR_PX, 2.0);
        assert_eq!(super::CYTOSCAPE_FINAL_ELEMENT_BBOX_EXPANSION_PX, 1.0);
        assert_eq!(super::CYTOSCAPE_PARENT_BODY_NON_PADDING_BBOX_OUTSET_PX, 1.5);

        let compound_child_extras = BoundsExtras {
            left: 1.0,
            right: 1.0,
            top: 1.0,
            bottom: 18.0,
        };
        let final_element_extras = BoundsExtras {
            bottom: 19.0,
            ..compound_child_extras
        };
        let graph = IndexedGraph {
            nodes: vec![
                IndexedNode {
                    parent: Some(0),
                    width: 80.0,
                    height: 80.0,
                    x: 0.0,
                    y: 0.0,
                    bounds_extras: compound_child_extras,
                },
                IndexedNode {
                    parent: None,
                    width: 80.0,
                    height: 80.0,
                    x: 200.0,
                    y: 0.0,
                    bounds_extras: final_element_extras,
                },
            ],
            edges: Vec::new(),
            compounds: vec![IndexedCompound { parent: None }],
        };

        let mut sim = SimGraph::from_indexed(&graph);
        let compound_idx = graph.nodes.len();
        sim.nodes[compound_idx].padding = 10.0;
        let center = sim
            .bounding_box_center_eles(1)
            .expect("compound relocation bbox center");

        assert_eq!(center, (94.25, 8.5));
    }
}
