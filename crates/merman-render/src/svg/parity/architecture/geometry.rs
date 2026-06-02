use super::super::{Bounds, fmt};
use crate::architecture_metrics::architecture_svg_group_bbox_padding_px;

pub(super) fn is_arch_dir_x(dir: char) -> bool {
    matches!(dir, 'L' | 'R')
}

pub(super) fn is_arch_dir_y(dir: char) -> bool {
    matches!(dir, 'T' | 'B')
}

pub(super) fn arrow_points(dir: char, arrow_size: f64) -> String {
    match dir {
        'L' => format!(
            "{s},{hs} 0,{s} 0,0",
            s = fmt(arrow_size),
            hs = fmt(arrow_size / 2.0)
        ),
        'R' => format!(
            "0,{hs} {s},0 {s},{s}",
            s = fmt(arrow_size),
            hs = fmt(arrow_size / 2.0)
        ),
        'T' => format!(
            "0,0 {s},0 {hs},{s}",
            s = fmt(arrow_size),
            hs = fmt(arrow_size / 2.0)
        ),
        'B' => format!(
            "{hs},0 {s},{s} 0,{s}",
            s = fmt(arrow_size),
            hs = fmt(arrow_size / 2.0)
        ),
        _ => arrow_points('R', arrow_size),
    }
}

pub(super) fn arrow_shift(dir: char, orig: f64, arrow_size: f64) -> f64 {
    // Mermaid@11.12.2 `ArchitectureDirectionArrowShift`.
    match dir {
        'L' | 'T' => orig - arrow_size + 2.0,
        'R' | 'B' => orig - 2.0,
        _ => orig,
    }
}

pub(super) fn edge_id(prefix: &str, from: &str, to: &str, counter: usize) -> String {
    // Mirrors Mermaid `getEdgeId(from, to, { prefix })` (counter defaults to 0).
    format!("{prefix}_{from}_{to}_{counter}")
}

pub(super) fn extend_bounds(bounds: &mut Option<Bounds>, other: Bounds) {
    let b = bounds.get_or_insert(other.clone());
    b.min_x = b.min_x.min(other.min_x);
    b.min_y = b.min_y.min(other.min_y);
    b.max_x = b.max_x.max(other.max_x);
    b.max_y = b.max_y.max(other.max_y);
}

pub(super) fn bounds_from_rect(x: f64, y: f64, w: f64, h: f64) -> Bounds {
    Bounds {
        min_x: x,
        min_y: y,
        max_x: x + w,
        max_y: y + h,
    }
}

#[derive(Clone, Copy)]
pub(super) struct GroupRect<'a> {
    pub(super) id: &'a str,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) w: f64,
    pub(super) h: f64,
    pub(super) icon: Option<&'a str>,
    pub(super) title: Option<&'a str>,
}

pub(super) struct GroupRectComputer<'a> {
    icon_size_px: f64,
    padding_px: f64,
    services_in_group: &'a rustc_hash::FxHashMap<&'a str, Vec<&'a str>>,
    junctions_in_group: &'a rustc_hash::FxHashMap<&'a str, Vec<&'a str>>,
    child_groups: &'a rustc_hash::FxHashMap<&'a str, Vec<&'a str>>,
    service_bounds: &'a rustc_hash::FxHashMap<&'a str, Bounds>,
    junction_bounds: &'a rustc_hash::FxHashMap<&'a str, Bounds>,
    group_rects: rustc_hash::FxHashMap<&'a str, Bounds>,
    visiting: rustc_hash::FxHashSet<&'a str>,
}

impl<'a> GroupRectComputer<'a> {
    pub(super) fn new(
        icon_size_px: f64,
        padding_px: f64,
        services_in_group: &'a rustc_hash::FxHashMap<&'a str, Vec<&'a str>>,
        junctions_in_group: &'a rustc_hash::FxHashMap<&'a str, Vec<&'a str>>,
        child_groups: &'a rustc_hash::FxHashMap<&'a str, Vec<&'a str>>,
        service_bounds: &'a rustc_hash::FxHashMap<&'a str, Bounds>,
        junction_bounds: &'a rustc_hash::FxHashMap<&'a str, Bounds>,
    ) -> Self {
        Self {
            icon_size_px,
            padding_px,
            services_in_group,
            junctions_in_group,
            child_groups,
            service_bounds,
            junction_bounds,
            group_rects: rustc_hash::FxHashMap::default(),
            visiting: rustc_hash::FxHashSet::default(),
        }
    }

    pub(super) fn get(&self, group_id: &str) -> Option<&Bounds> {
        self.group_rects.get(group_id)
    }

    pub(super) fn compute(&mut self, group_id: &'a str) -> Option<Bounds> {
        if let Some(b) = self.group_rects.get(group_id) {
            return Some(b.clone());
        }
        if self.visiting.contains(group_id) {
            return None;
        }
        self.visiting.insert(group_id);

        let debug_group = std::env::var("MERMAN_ARCH_DEBUG_GROUP_RECT")
            .ok()
            .filter(|value| !value.is_empty());
        let debug_this_group = debug_group.as_deref() == Some(group_id);

        let mut content: Option<Bounds> = None;
        if let Some(svcs) = self.services_in_group.get(group_id) {
            for id in svcs {
                if let Some(b) = self.service_bounds.get(id) {
                    if debug_this_group {
                        eprintln!(
                            "[arch-group-rect] group={} service={} bounds=({}, {})-({}, {})",
                            group_id, id, b.min_x, b.min_y, b.max_x, b.max_y
                        );
                    }
                    let mut tmp = content;
                    extend_bounds(&mut tmp, b.clone());
                    content = tmp;
                }
            }
        }
        if let Some(junctions) = self.junctions_in_group.get(group_id) {
            for id in junctions {
                if let Some(b) = self.junction_bounds.get(id) {
                    if debug_this_group {
                        eprintln!(
                            "[arch-group-rect] group={} junction={} bounds=({}, {})-({}, {})",
                            group_id, id, b.min_x, b.min_y, b.max_x, b.max_y
                        );
                    }
                    let mut tmp = content;
                    extend_bounds(&mut tmp, b.clone());
                    content = tmp;
                }
            }
        }
        if let Some(children) = self.child_groups.get(group_id) {
            // Empirical correction for nested compounds:
            //
            // Mermaid draws group rects from Cytoscape `node.boundingBox()` values. When groups
            // nest, Cytoscape's compound bounds update uses a children bounding box that is not a
            // perfect "union of already-padded child group rects" in SVG space; treating child
            // group rects as fully-inclusive inputs makes parent groups slightly too large in
            // parity-root viewBox comparisons (notably in deep group chains).
            //
            // Approximate this by shrinking child group bounds by half the group border width
            // (2px / 2 == 1px) before unioning them into the parent's content bounds.
            let child_group_inset = 1.0;
            for child in children {
                if let Some(b) = self.compute(child) {
                    if debug_this_group {
                        eprintln!(
                            "[arch-group-rect] group={} child-group={} raw=({}, {})-({}, {})",
                            group_id, child, b.min_x, b.min_y, b.max_x, b.max_y
                        );
                    }
                    let b = if (b.max_x - b.min_x) > 2.0 * child_group_inset
                        && (b.max_y - b.min_y) > 2.0 * child_group_inset
                    {
                        Bounds {
                            min_x: b.min_x + child_group_inset,
                            min_y: b.min_y + child_group_inset,
                            max_x: b.max_x - child_group_inset,
                            max_y: b.max_y - child_group_inset,
                        }
                    } else {
                        b
                    };
                    if debug_this_group {
                        eprintln!(
                            "[arch-group-rect] group={} child-group={} inset=({}, {})-({}, {})",
                            group_id, child, b.min_x, b.min_y, b.max_x, b.max_y
                        );
                    }
                    let mut tmp = content;
                    extend_bounds(&mut tmp, b);
                    content = tmp;
                }
            }
        }

        // Upstream Mermaid draws group rectangles from Cytoscape `node.boundingBox()` values and
        // then offsets them by `halfIconSize`. This renderer-side approximation is intentionally
        // named separately from manatee's relocation/element-bbox policy; they are different
        // Cytoscape phases and should not silently share a generic compound-padding helper.
        let pad = architecture_svg_group_bbox_padding_px(self.padding_px);
        if debug_this_group {
            if let Some(content_bounds) = &content {
                eprintln!(
                    "[arch-group-rect] group={} content=({}, {})-({}, {}) pad={}",
                    group_id,
                    content_bounds.min_x,
                    content_bounds.min_y,
                    content_bounds.max_x,
                    content_bounds.max_y,
                    pad
                );
            } else {
                eprintln!(
                    "[arch-group-rect] group={} content=<empty> pad={}",
                    group_id, pad
                );
            }
        }
        let b = if let Some(content) = content {
            Bounds {
                min_x: content.min_x - pad,
                min_y: content.min_y - pad,
                max_x: content.max_x + pad,
                max_y: content.max_y + pad,
            }
        } else {
            // Empty group: match Mermaid's "no children" fallback sizing behavior.
            Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: self.icon_size_px.max(1.0),
                max_y: self.icon_size_px.max(1.0),
            }
        };
        if debug_this_group {
            eprintln!(
                "[arch-group-rect] group={} final=({}, {})-({}, {})",
                group_id, b.min_x, b.min_y, b.max_x, b.max_y
            );
        }

        self.group_rects.insert(group_id, b.clone());
        self.visiting.remove(group_id);
        Some(b)
    }
}
