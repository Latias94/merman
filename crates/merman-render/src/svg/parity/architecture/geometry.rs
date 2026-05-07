use super::super::{Bounds, fmt};

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

pub(super) fn compute_group_rects<'a>(
    group_id: &'a str,
    icon_size_px: f64,
    services_in_group: &rustc_hash::FxHashMap<&'a str, Vec<&'a str>>,
    junctions_in_group: &rustc_hash::FxHashMap<&'a str, Vec<&'a str>>,
    child_groups: &rustc_hash::FxHashMap<&'a str, Vec<&'a str>>,
    service_bounds: &rustc_hash::FxHashMap<&'a str, Bounds>,
    junction_bounds: &rustc_hash::FxHashMap<&'a str, Bounds>,
    group_rects: &mut rustc_hash::FxHashMap<&'a str, Bounds>,
    visiting: &mut rustc_hash::FxHashSet<&'a str>,
) -> Option<Bounds> {
    if let Some(b) = group_rects.get(group_id) {
        return Some(b.clone());
    }
    if visiting.contains(group_id) {
        return None;
    }
    visiting.insert(group_id);

    let mut content: Option<Bounds> = None;
    if let Some(svcs) = services_in_group.get(group_id) {
        for id in svcs {
            if let Some(b) = service_bounds.get(id) {
                let mut tmp = content;
                extend_bounds(&mut tmp, b.clone());
                content = tmp;
            }
        }
    }
    if let Some(junctions) = junctions_in_group.get(group_id) {
        for id in junctions {
            if let Some(b) = junction_bounds.get(id) {
                let mut tmp = content;
                extend_bounds(&mut tmp, b.clone());
                content = tmp;
            }
        }
    }
    if let Some(children) = child_groups.get(group_id) {
        // Empirical correction for nested compounds:
        //
        // Mermaid draws group rects from Cytoscape `node.boundingBox()` values. When groups nest,
        // Cytoscape's compound bounds update uses a children bounding box that is not a perfect
        // "union of already-padded child group rects" in SVG space; treating child group rects as
        // fully-inclusive inputs makes parent groups slightly too large in parity-root viewBox
        // comparisons (notably in deep group chains).
        //
        // Approximate this by shrinking child group bounds by half the group border width
        // (2px / 2 == 1px) before unioning them into the parent's content bounds.
        let child_group_inset = 1.0;
        for child in children {
            if let Some(b) = compute_group_rects(
                child,
                icon_size_px,
                services_in_group,
                junctions_in_group,
                child_groups,
                service_bounds,
                junction_bounds,
                group_rects,
                visiting,
            ) {
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
                let mut tmp = content;
                extend_bounds(&mut tmp, b);
                content = tmp;
            }
        }
    }

    // Upstream Mermaid draws group rectangles from `cytoscape-node.boundingBox()` (default includes
    // labels), then offsets by `halfIconSize`.
    //
    // The extra padding is a small empirical correction to approximate browser `boundingBox()`
    // behavior in headless mode.
    let _has_child_groups = child_groups.get(group_id).is_some_and(|v| !v.is_empty());
    let extra = 2.5;
    let pad = icon_size_px / 2.0 + extra;
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
            max_x: icon_size_px.max(1.0),
            max_y: icon_size_px.max(1.0),
        }
    };

    group_rects.insert(group_id, b.clone());
    visiting.remove(group_id);
    Some(b)
}
