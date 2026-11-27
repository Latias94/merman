//! Pure Mermaid line-hop geometry.
//!
//! This is a DOM-free port of Mermaid's `lineJump.ts`. Callers supply the curve
//! eligibility rule and keep their original SVG paths wherever no override is emitted.

use crate::LayoutPoint;
use crate::resources::OperationWorkMeter;
#[cfg(test)]
use crate::resources::{RenderResourcePolicy, ResourceLimitId};
use std::cmp::Ordering;
use std::collections::{BTreeSet, BinaryHeap, HashMap};

// Kept in sync with Mermaid's `generateRoundedPath` radius.
const ROUNDED_CORNER_RADIUS: f64 = 5.0;
const CORNER_EPSILON: f64 = 1e-5;
const ENDPOINT_EPSILON: f64 = 1e-6;
const CORNER_JUMP_CLEARANCE: f64 = 2.0;
const MIN_USEFUL_RADIUS_RATIO: f64 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineHopStyle {
    Arc,
    Gap,
}

#[derive(Debug, Clone, Copy)]
pub struct LineHopConfig {
    pub jump_radius: f64,
    pub jump_style: LineHopStyle,
}

#[derive(Debug, Clone, Copy)]
pub struct LineHopEdge<'a> {
    pub id: &'a str,
    pub points: &'a [LayoutPoint],
    pub curve: Option<&'a str>,
    pub arrow_type_start: Option<&'a str>,
    pub arrow_type_end: Option<&'a str>,
}

#[derive(Debug, Clone)]
struct LineHopCrossing<'a> {
    pub jump_edge_id: &'a str,
    pub other_edge_id: &'a str,
    pub segment_index: usize,
    pub t: f64,
    pub point: LayoutPoint,
}

#[derive(Debug, Clone, Copy)]
struct Segment<'a> {
    start: &'a LayoutPoint,
    end: &'a LayoutPoint,
}

#[derive(Debug, Clone, Copy)]
struct IndexedSegment<'a> {
    edge_index: usize,
    segment_index: usize,
    segment: Segment<'a>,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

#[derive(Debug, Clone, Copy)]
struct CandidatePair {
    segment_a: usize,
    segment_b: usize,
}

#[derive(Debug, Clone, Copy)]
struct SegmentExpiry {
    max_x: f64,
    segment_index: usize,
}

impl PartialEq for SegmentExpiry {
    fn eq(&self, other: &Self) -> bool {
        self.max_x.total_cmp(&other.max_x) == Ordering::Equal
            && self.segment_index == other.segment_index
    }
}

impl Eq for SegmentExpiry {}

impl Ord for SegmentExpiry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .max_x
            .total_cmp(&self.max_x)
            .then_with(|| other.segment_index.cmp(&self.segment_index))
    }
}

impl PartialOrd for SegmentExpiry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
struct SegmentIntersection {
    point: LayoutPoint,
    t_a: f64,
    t_b: f64,
}

#[derive(Debug, Clone)]
struct JumpOnSegment {
    t: f64,
    point: LayoutPoint,
    distance: f64,
    radius: f64,
}

#[derive(Debug, Clone, Copy)]
struct RoundedCorner {
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
    control_x: f64,
    control_y: f64,
    cut_length: f64,
}

fn sorting_work_units(items: usize) -> usize {
    if items <= 1 {
        return 0;
    }
    let passes = (usize::BITS - items.saturating_sub(1).leading_zeros()) as usize;
    items.saturating_mul(passes)
}

fn segment_count(edges: &[LineHopEdge<'_>]) -> usize {
    edges
        .iter()
        .map(|edge| edge.points.len().saturating_sub(1))
        .fold(0usize, usize::saturating_add)
}

fn path_emission_work_units(edges: &[LineHopEdge<'_>]) -> usize {
    edges.iter().fold(0usize, |work, edge| {
        work.saturating_add(1).saturating_add(edge.points.len())
    })
}

fn find_edge_intersections<'a>(
    edges: &[LineHopEdge<'a>],
    work_meter: &OperationWorkMeter,
) -> crate::Result<Vec<LineHopCrossing<'a>>> {
    let segment_count = segment_count(edges);
    work_meter.charge(segment_count.saturating_add(sorting_work_units(segment_count)))?;
    let mut segments = Vec::new();
    for (edge_index, edge) in edges.iter().enumerate() {
        for (segment_index, pair) in edge.points.windows(2).enumerate() {
            let start = &pair[0];
            let end = &pair[1];
            segments.push(IndexedSegment {
                edge_index,
                segment_index,
                segment: Segment { start, end },
                min_x: start.x.min(end.x),
                max_x: start.x.max(end.x),
                min_y: start.y.min(end.y),
                max_y: start.y.max(end.y),
            });
        }
    }

    let mut by_min_x = (0..segments.len()).collect::<Vec<_>>();
    by_min_x.sort_by(|&left, &right| {
        segments[left]
            .min_x
            .total_cmp(&segments[right].min_x)
            .then_with(|| segments[left].edge_index.cmp(&segments[right].edge_index))
            .then_with(|| {
                segments[left]
                    .segment_index
                    .cmp(&segments[right].segment_index)
            })
    });

    // The active set is partitioned by edge so segments never compare against the same edge.
    // Expiration by max-x gives a source-preserving broad phase: excluded pairs cannot have a
    // proper segment intersection, while surviving pairs still use Mermaid's exact predicate.
    let mut active_by_edge: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); edges.len()];
    let mut active_edges: BTreeSet<usize> = BTreeSet::new();
    let mut expirations: BinaryHeap<SegmentExpiry> = BinaryHeap::new();
    let mut candidate_pairs = Vec::new();
    for segment_index in by_min_x {
        let current = segments[segment_index];
        while expirations
            .peek()
            .is_some_and(|expiry| expiry.max_x <= current.min_x)
        {
            let expiry = expirations.pop().expect("peeked expiry must exist");
            let expired_edge = segments[expiry.segment_index].edge_index;
            active_by_edge[expired_edge].remove(&expiry.segment_index);
            if active_by_edge[expired_edge].is_empty() {
                active_edges.remove(&expired_edge);
            }
        }

        for &other_edge_index in &active_edges {
            if other_edge_index == current.edge_index {
                continue;
            }
            work_meter.charge(active_by_edge[other_edge_index].len())?;
            for &other_segment_index in &active_by_edge[other_edge_index] {
                let other = segments[other_segment_index];
                if other.max_y <= current.min_y || current.max_y <= other.min_y {
                    continue;
                }
                let (segment_a, segment_b) = if other.edge_index < current.edge_index {
                    (other_segment_index, segment_index)
                } else {
                    (segment_index, other_segment_index)
                };
                candidate_pairs.push(CandidatePair {
                    segment_a,
                    segment_b,
                });
            }
        }

        active_by_edge[current.edge_index].insert(segment_index);
        active_edges.insert(current.edge_index);
        expirations.push(SegmentExpiry {
            max_x: current.max_x,
            segment_index,
        });
    }

    work_meter.charge(sorting_work_units(candidate_pairs.len()))?;
    candidate_pairs.sort_by_key(|candidate| {
        let a = segments[candidate.segment_a];
        let b = segments[candidate.segment_b];
        (a.edge_index, b.edge_index, a.segment_index, b.segment_index)
    });

    let mut crossings = Vec::new();
    for candidate in candidate_pairs {
        let segment_a = segments[candidate.segment_a];
        let segment_b = segments[candidate.segment_b];
        let Some(intersection) = segment_intersection(segment_a.segment, segment_b.segment) else {
            continue;
        };

        let edge_a = edges[segment_a.edge_index];
        let edge_b = edges[segment_b.edge_index];
        if crossing_sits_in_rounded_corner(edge_a, segment_a.segment_index, intersection.t_a)
            || crossing_sits_in_rounded_corner(edge_b, segment_b.segment_index, intersection.t_b)
        {
            continue;
        }
        let a_is_horizontal = is_horizontally_dominant(segment_a.segment);
        let b_is_horizontal = is_horizontally_dominant(segment_b.segment);
        let jump_on_a = a_is_horizontal != b_is_horizontal && a_is_horizontal;

        if jump_on_a {
            crossings.push(LineHopCrossing {
                jump_edge_id: edge_a.id,
                other_edge_id: edge_b.id,
                segment_index: segment_a.segment_index,
                t: intersection.t_a,
                point: intersection.point,
            });
        } else {
            crossings.push(LineHopCrossing {
                jump_edge_id: edge_b.id,
                other_edge_id: edge_a.id,
                segment_index: segment_b.segment_index,
                t: intersection.t_b,
                point: intersection.point,
            });
        }
    }

    Ok(crossings)
}

/// Emit after-paint overrides in edge order, indexed into the supplied edges. A crossing
/// still triggers an override when available-radius checks suppress every visible hop.
pub fn line_hop_overrides(
    edges: &[LineHopEdge<'_>],
    config: LineHopConfig,
    accepts_curve: impl Fn(Option<&str>) -> bool,
    work_meter: &OperationWorkMeter,
) -> crate::Result<Vec<(usize, String)>> {
    // Preserve the admission before intersection discovery, including on crossing-free graphs.
    work_meter.charge(path_emission_work_units(edges))?;
    let crossings = find_edge_intersections(edges, work_meter)?;
    if crossings.is_empty() {
        return Ok(Vec::new());
    }

    let mut jumps_by_edge: HashMap<&str, Vec<LineHopCrossing<'_>>> = HashMap::new();
    for crossing in crossings {
        debug_assert_ne!(crossing.jump_edge_id, crossing.other_edge_id);
        jumps_by_edge
            .entry(crossing.jump_edge_id)
            .or_default()
            .push(crossing);
    }

    Ok(edges
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, edge)| {
            let jumps = jumps_by_edge.get(edge.id)?;
            accepts_curve(edge.curve).then(|| (index, rewrite_edge_path(edge, jumps, config)))
        })
        .collect())
}

/// Return ELK paths rewritten for a crossing, even when nearby bends suppress every visible hop.
/// Callers supply draw-projected points before marker offsets and retain their
/// original paths for labels, marker placement, and all non-crossing edges.
/// Curve eligibility applies after intersection discovery, as in Mermaid.
pub fn elk_line_hop_paths<'a>(
    config: &serde_json::Value,
    edges: &[LineHopEdge<'a>],
    work_meter: &OperationWorkMeter,
) -> crate::Result<HashMap<&'a str, String>> {
    let line_hops = config.pointer("/elk/lineHops");
    if line_hops.and_then(serde_json::Value::as_bool) == Some(false) {
        return Ok(HashMap::new());
    }
    work_meter.charge(edges.len().saturating_mul(2))?;
    let rendered_edges: Vec<_> = edges
        .iter()
        .copied()
        .filter(|edge| edge.points.len() >= 2)
        .collect();
    let paths = line_hop_overrides(
        &rendered_edges,
        LineHopConfig {
            jump_radius: 6.0,
            jump_style: if line_hops.and_then(serde_json::Value::as_str) == Some("gap") {
                LineHopStyle::Gap
            } else {
                LineHopStyle::Arc
            },
        },
        curve_supports_line_hops,
        work_meter,
    )?;
    Ok(paths
        .into_iter()
        .map(|(index, path)| (rendered_edges[index].id, path))
        .collect())
}

/// Mirror lineJump.ts's after-paint mask rewrite, including its dashed-prefix behavior.
/// The first matching mask supplies the second and fourth numbers, even when that
/// fourth number belongs to a repeated dash pattern. Every declaration is replaced.
pub fn rewrite_style_after_line_hop<'a>(
    original_style: &'a str,
    hopped_d: &str,
    work_meter: &OperationWorkMeter,
) -> crate::Result<std::borrow::Cow<'a, str>> {
    work_meter.charge(original_style.len().saturating_add(hopped_d.len()))?;
    use std::borrow::Cow;
    const PROPERTY: &str = "stroke-dasharray";

    use crate::text::is_ecmascript_whitespace as js_space;

    fn value_start(style: &str, property_start: usize) -> Option<usize> {
        let suffix = style[property_start + PROPERTY.len()..].trim_start_matches(js_space);
        let value = suffix.strip_prefix(':')?;
        Some(style.len() - value.len())
    }
    fn mask_offsets(value: &str) -> Option<(f64, f64)> {
        let mut rest = value.trim_start_matches(js_space).strip_prefix('0')?;
        let mut fields = [0.0; 3];
        for field in &mut fields {
            let trimmed = rest.trim_start_matches(js_space);
            if trimmed.len() == rest.len() {
                return None;
            }
            let end = trimmed
                .find(|ch: char| !ch.is_ascii_digit() && ch != '.')
                .unwrap_or(trimmed.len());
            if end == 0 {
                return None;
            }
            let raw = &trimmed[..end];
            // The source uses parseFloat on [\d.]+, which accepts a numeric prefix.
            let numeric_end = raw.match_indices('.').nth(1).map_or(raw.len(), |(i, _)| i);
            *field = raw[..numeric_end].parse().unwrap_or(f64::NAN);
            rest = &trimmed[end..];
        }
        Some((fields[0], fields[2]))
    }

    let offsets = original_style
        .match_indices(PROPERTY)
        .find_map(|(start, _)| {
            mask_offsets(&original_style[value_start(original_style, start)?..])
        });
    let Some((start_offset, end_offset)) = offsets else {
        return Ok(Cow::Borrowed(original_style));
    };
    let Some(length) = super::svg_path_length_from_d(hopped_d) else {
        return Ok(Cow::Borrowed(original_style));
    };
    let remaining = length - start_offset - end_offset;
    let on_length = if remaining.is_nan() {
        f64::NAN
    } else {
        remaining.max(0.0)
    };
    let number = |value| ryu_js::Buffer::new().format(value).to_owned();
    let replacement = format!(
        "stroke-dasharray: 0 {} {} {};",
        number(start_offset),
        number(on_length),
        number(end_offset)
    );
    let mut rewritten = String::with_capacity(original_style.len());
    let mut copied = 0;
    let mut search = 0;
    while let Some(relative) = original_style[search..].find(PROPERTY) {
        let start = search + relative;
        let Some(value) = value_start(original_style, start) else {
            search = start + PROPERTY.len();
            continue;
        };
        let end = original_style[value..]
            .find(';')
            .map_or(original_style.len(), |end| value + end + 1);
        rewritten.push_str(&original_style[copied..start]);
        rewritten.push_str(&replacement);
        copied = end;
        search = end;
    }
    rewritten.push_str(&original_style[copied..]);

    // Single left-to-right replacement of /;\s*;+/g, not recursive CSS normalization.
    let mut cleaned = String::with_capacity(rewritten.len());
    let mut remaining = rewritten.as_str();
    while let Some(index) = remaining.find(';') {
        cleaned.push_str(&remaining[..=index]);
        remaining = &remaining[index + 1..];
        let trimmed = remaining.trim_start_matches(js_space);
        if trimmed.starts_with(';') {
            remaining = trimmed.trim_start_matches(';');
        }
    }
    cleaned.push_str(remaining);
    Ok(Cow::Owned(cleaned))
}

pub fn curve_supports_line_hops(curve: Option<&str>) -> bool {
    matches!(
        curve,
        None | Some("linear" | "rounded" | "step" | "stepBefore" | "stepAfter")
    )
}

#[cfg(test)]
fn is_straight_path(path: &str) -> bool {
    path.chars().all(|character| {
        character.is_ascii_digit()
            || character.is_whitespace()
            || character == '\u{feff}'
            || matches!(
                character,
                '+' | ',' | '.' | 'L' | 'M' | 'e' | 'l' | 'm' | '-'
            )
    })
}

fn build_segments(points: &[LayoutPoint]) -> Vec<Segment<'_>> {
    points
        .windows(2)
        .map(|pair| Segment {
            start: &pair[0],
            end: &pair[1],
        })
        .collect()
}

fn segment_intersection(
    segment_a: Segment<'_>,
    segment_b: Segment<'_>,
) -> Option<SegmentIntersection> {
    let dx_a = segment_a.end.x - segment_a.start.x;
    let dy_a = segment_a.end.y - segment_a.start.y;
    let dx_b = segment_b.end.x - segment_b.start.x;
    let dy_b = segment_b.end.y - segment_b.start.y;
    let denominator = dx_a * dy_b - dy_a * dx_b;
    if denominator == 0.0 {
        return None;
    }

    let dx = segment_b.start.x - segment_a.start.x;
    let dy = segment_b.start.y - segment_a.start.y;
    let t_a = (dx * dy_b - dy * dx_b) / denominator;
    let t_b = (dx * dy_a - dy * dx_a) / denominator;

    if t_a <= ENDPOINT_EPSILON
        || t_a >= 1.0 - ENDPOINT_EPSILON
        || t_b <= ENDPOINT_EPSILON
        || t_b >= 1.0 - ENDPOINT_EPSILON
    {
        return None;
    }

    Some(SegmentIntersection {
        point: LayoutPoint {
            x: segment_a.start.x + t_a * dx_a,
            y: segment_a.start.y + t_a * dy_a,
        },
        t_a,
        t_b,
    })
}

fn is_horizontally_dominant(segment: Segment<'_>) -> bool {
    (segment.end.x - segment.start.x).abs() >= (segment.end.y - segment.start.y).abs()
}

// Rounded strokes leave their provider polylines around a bend. A crossing on
// either of those replaced stretches does not locate a crossing of the strokes.
fn crossing_sits_in_rounded_corner(edge: LineHopEdge<'_>, index: usize, t: f64) -> bool {
    if edge.curve != Some("rounded") {
        return false;
    }
    let points = edge.points;
    let (Some(start), Some(end)) = (points.get(index), points.get(index + 1)) else {
        return false;
    };
    let length = (end.x - start.x).hypot(end.y - start.y);
    let distance = t * length;
    let entering = index
        .checked_sub(1)
        .and_then(|previous| compute_rounded_corner(&points[previous], start, end));
    if entering.is_some_and(|corner| distance < corner.cut_length) {
        return true;
    }
    points
        .get(index + 2)
        .and_then(|next| compute_rounded_corner(start, end, next))
        .is_some_and(|corner| length - distance < corner.cut_length)
}

fn fmt_number(value: f64) -> String {
    let scaled = value * 1000.0;
    let rounded = if scaled.is_finite() {
        (scaled + 0.5).floor() / 1000.0
    } else {
        value
    };

    if rounded == 0.0 {
        "0".to_owned()
    } else if rounded == f64::INFINITY {
        "Infinity".to_owned()
    } else if rounded == f64::NEG_INFINITY {
        "-Infinity".to_owned()
    } else {
        rounded.to_string()
    }
}

fn point_to_string(point: &LayoutPoint) -> String {
    format!("{},{}", fmt_number(point.x), fmt_number(point.y))
}

fn arc_sweep_flag(segment: Segment<'_>) -> u8 {
    let dx = segment.end.x - segment.start.x;
    let dy = segment.end.y - segment.start.y;
    if dx.abs() >= dy.abs() {
        u8::from(dx >= 0.0)
    } else {
        u8::from(dy >= 0.0)
    }
}

// `lineJump.ts` imports this exact table from `utils/lineWithOffset.ts`.
// It is mirrored here because the existing flowchart offset helper is private
// to a sibling module and this geometry port must remain independently usable.
fn marker_offset_for(arrow_type: Option<&str>) -> Option<f64> {
    match arrow_type {
        Some("aggregation" | "extension" | "composition") => Some(17.25),
        Some("dependency") => Some(6.0),
        Some("lollipop") => Some(13.5),
        Some("arrow_point") => Some(4.0),
        Some("arrow_barb") => Some(0.0),
        Some("arrow_barb_neo") => Some(5.5),
        _ => None,
    }
}

fn apply_marker_offsets(points: &[LayoutPoint], edge: LineHopEdge<'_>) -> Vec<LayoutPoint> {
    if points.len() < 2 {
        return points.to_vec();
    }

    let mut offset_points = points.to_vec();
    if let Some(offset) = marker_offset_for(edge.arrow_type_start).filter(|offset| *offset != 0.0) {
        let start = &points[0];
        let next = &points[1];
        let angle = (next.y - start.y).atan2(next.x - start.x);
        offset_points[0].x = start.x + offset * angle.cos();
        offset_points[0].y = start.y + offset * angle.sin();
    }

    if let Some(offset) = marker_offset_for(edge.arrow_type_end).filter(|offset| *offset != 0.0) {
        let last_index = points.len() - 1;
        let previous = &points[last_index - 1];
        let end = &points[last_index];
        let angle = (end.y - previous.y).atan2(end.x - previous.x);
        offset_points[last_index].x = end.x - offset * angle.cos();
        offset_points[last_index].y = end.y - offset * angle.sin();
    }

    offset_points
}

fn compute_rounded_corner(
    previous: &LayoutPoint,
    current: &LayoutPoint,
    next: &LayoutPoint,
) -> Option<RoundedCorner> {
    let incoming_dx = current.x - previous.x;
    let incoming_dy = current.y - previous.y;
    let outgoing_dx = next.x - current.x;
    let outgoing_dy = next.y - current.y;
    let incoming_length = incoming_dx.hypot(incoming_dy);
    let outgoing_length = outgoing_dx.hypot(outgoing_dy);
    if incoming_length < CORNER_EPSILON || outgoing_length < CORNER_EPSILON {
        return None;
    }

    let incoming_x = incoming_dx / incoming_length;
    let incoming_y = incoming_dy / incoming_length;
    let outgoing_x = outgoing_dx / outgoing_length;
    let outgoing_y = outgoing_dy / outgoing_length;
    let dot_product = incoming_x * outgoing_x + incoming_y * outgoing_y;
    let clamped_dot = dot_product.clamp(-1.0, 1.0);
    let angle = clamped_dot.acos();
    if angle < CORNER_EPSILON || (std::f64::consts::PI - angle).abs() < CORNER_EPSILON {
        return None;
    }

    let cut_length = (ROUNDED_CORNER_RADIUS / (angle / 2.0).sin())
        .min(incoming_length / 2.0)
        .min(outgoing_length / 2.0);
    Some(RoundedCorner {
        start_x: current.x - incoming_x * cut_length,
        start_y: current.y - incoming_y * cut_length,
        end_x: current.x + outgoing_x * cut_length,
        end_y: current.y + outgoing_y * cut_length,
        control_x: current.x,
        control_y: current.y,
        cut_length,
    })
}

fn emit_jump(
    jump: &JumpOnSegment,
    unit_x: f64,
    unit_y: f64,
    sweep: u8,
    style: LineHopStyle,
    parts: &mut Vec<String>,
) {
    let before = LayoutPoint {
        x: jump.point.x - unit_x * jump.radius,
        y: jump.point.y - unit_y * jump.radius,
    };
    let after = LayoutPoint {
        x: jump.point.x + unit_x * jump.radius,
        y: jump.point.y + unit_y * jump.radius,
    };
    parts.push(format!("L{}", point_to_string(&before)));
    match style {
        LineHopStyle::Arc => parts.push(format!(
            "A{},{} 0 0 {} {}",
            fmt_number(jump.radius),
            fmt_number(jump.radius),
            sweep,
            point_to_string(&after)
        )),
        LineHopStyle::Gap => parts.push(format!("M{}", point_to_string(&after))),
    }
}

fn rewrite_edge_path(
    edge: LineHopEdge<'_>,
    jumps: &[LineHopCrossing<'_>],
    config: LineHopConfig,
) -> String {
    if edge.points.len() < 2 {
        return String::new();
    }

    let points = apply_marker_offsets(edge.points, edge);
    let segments = build_segments(&points);
    let mut jumps_by_segment: HashMap<usize, Vec<JumpOnSegment>> = HashMap::new();
    for jump in jumps {
        let Some(segment) = segments.get(jump.segment_index) else {
            continue;
        };
        let segment_length =
            (segment.end.x - segment.start.x).hypot(segment.end.y - segment.start.y);
        jumps_by_segment
            .entry(jump.segment_index)
            .or_default()
            .push(JumpOnSegment {
                t: jump.t,
                point: jump.point.clone(),
                distance: jump.t * segment_length,
                radius: config.jump_radius,
            });
    }

    let rounded = edge.curve == Some("rounded");
    let mut parts = vec![format!("M{}", point_to_string(&points[0]))];

    for (segment_index, segment) in segments.iter().enumerate() {
        let segment_length =
            (segment.end.x - segment.start.x).hypot(segment.end.y - segment.start.y);
        let (unit_x, unit_y) = if segment_length == 0.0 {
            (0.0, 0.0)
        } else {
            (
                (segment.end.x - segment.start.x) / segment_length,
                (segment.end.y - segment.start.y) / segment_length,
            )
        };
        let sweep = arc_sweep_flag(*segment);

        let segment_start_consumed = if rounded && segment_index > 0 {
            compute_rounded_corner(
                &points[segment_index - 1],
                &points[segment_index],
                points
                    .get(segment_index + 1)
                    .unwrap_or(&points[segment_index]),
            )
            .map_or(0.0, |corner| corner.cut_length)
        } else {
            0.0
        };

        let upcoming_corner = if rounded && segment_index < segments.len() - 1 {
            compute_rounded_corner(
                &points[segment_index],
                &points[segment_index + 1],
                points
                    .get(segment_index + 2)
                    .unwrap_or(&points[segment_index + 1]),
            )
        } else {
            None
        };
        let segment_end_stop =
            upcoming_corner.map_or(segment_length, |corner| segment_length - corner.cut_length);

        let mut segment_jumps = jumps_by_segment.remove(&segment_index).unwrap_or_default();
        segment_jumps
            .sort_by(|left, right| left.t.partial_cmp(&right.t).unwrap_or(Ordering::Equal));
        // Discard corner-squeezed hops before adjacency so they cannot shrink a
        // usable neighbour. Mermaid reserves the same clearance on linear edges.
        let min_useful_radius = config.jump_radius * MIN_USEFUL_RADIUS_RATIO;
        segment_jumps.retain_mut(|jump| {
            let room = (jump.distance - segment_start_consumed)
                .min(segment_end_stop - jump.distance)
                - CORNER_JUMP_CLEARANCE;
            jump.radius = jump.radius.min(room);
            jump.radius >= min_useful_radius
        });
        for index in 0..segment_jumps.len().saturating_sub(1) {
            let (before, after) = segment_jumps.split_at_mut(index + 1);
            let first = &mut before[index];
            let second = &mut after[0];
            let gap = second.distance - first.distance;
            if first.radius + second.radius > gap {
                let half_gap = gap / 2.0;
                first.radius = first.radius.min(half_gap);
                second.radius = second.radius.min(half_gap);
            }
        }

        for jump in &segment_jumps {
            if jump.radius < min_useful_radius {
                continue;
            }
            emit_jump(jump, unit_x, unit_y, sweep, config.jump_style, &mut parts);
        }

        if let Some(corner) = upcoming_corner {
            parts.push(format!(
                "L{},{}",
                fmt_number(corner.start_x),
                fmt_number(corner.start_y)
            ));
            parts.push(format!(
                "Q{},{} {},{}",
                fmt_number(corner.control_x),
                fmt_number(corner.control_y),
                fmt_number(corner.end_x),
                fmt_number(corner.end_y)
            ));
        } else {
            parts.push(format!("L{}", point_to_string(segment.end)));
        }
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_edge_intersections<'a>(edges: &[LineHopEdge<'a>]) -> Vec<LineHopCrossing<'a>> {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        super::find_edge_intersections(edges, &meter).expect("unbounded line-hop intersection scan")
    }

    fn line_hop_overrides<'a>(
        edges: &[LineHopEdge<'a>],
        config: LineHopConfig,
    ) -> HashMap<&'a str, String> {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        super::line_hop_overrides(edges, config, |_| true, &meter)
            .expect("unbounded line-hop processing")
            .into_iter()
            .map(|(index, path)| (edges[index].id, path))
            .collect()
    }

    fn rewrite_style_after_line_hop<'a>(style: &'a str, path: &str) -> std::borrow::Cow<'a, str> {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        super::rewrite_style_after_line_hop(style, path, &meter).unwrap()
    }

    fn brute_force_edge_intersections<'a>(edges: &[LineHopEdge<'a>]) -> Vec<LineHopCrossing<'a>> {
        let mut crossings = Vec::new();
        for (edge_a_index, edge_a) in edges.iter().enumerate() {
            for edge_b in &edges[edge_a_index + 1..] {
                for (segment_a_index, pair_a) in edge_a.points.windows(2).enumerate() {
                    let segment_a = Segment {
                        start: &pair_a[0],
                        end: &pair_a[1],
                    };
                    for (segment_b_index, pair_b) in edge_b.points.windows(2).enumerate() {
                        let segment_b = Segment {
                            start: &pair_b[0],
                            end: &pair_b[1],
                        };
                        let Some(intersection) = segment_intersection(segment_a, segment_b) else {
                            continue;
                        };
                        if crossing_sits_in_rounded_corner(
                            *edge_a,
                            segment_a_index,
                            intersection.t_a,
                        ) || crossing_sits_in_rounded_corner(
                            *edge_b,
                            segment_b_index,
                            intersection.t_b,
                        ) {
                            continue;
                        }
                        let a_is_horizontal = is_horizontally_dominant(segment_a);
                        let b_is_horizontal = is_horizontally_dominant(segment_b);
                        let jump_on_a = a_is_horizontal != b_is_horizontal && a_is_horizontal;
                        crossings.push(if jump_on_a {
                            LineHopCrossing {
                                jump_edge_id: edge_a.id,
                                other_edge_id: edge_b.id,
                                segment_index: segment_a_index,
                                t: intersection.t_a,
                                point: intersection.point,
                            }
                        } else {
                            LineHopCrossing {
                                jump_edge_id: edge_b.id,
                                other_edge_id: edge_a.id,
                                segment_index: segment_b_index,
                                t: intersection.t_b,
                                point: intersection.point,
                            }
                        });
                    }
                }
            }
        }
        crossings
    }

    fn assert_crossings_equal(actual: &[LineHopCrossing<'_>], expected: &[LineHopCrossing<'_>]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.jump_edge_id, expected.jump_edge_id);
            assert_eq!(actual.other_edge_id, expected.other_edge_id);
            assert_eq!(actual.segment_index, expected.segment_index);
            assert_eq!(actual.t, expected.t);
            assert_eq!(actual.point.x, expected.point.x);
            assert_eq!(actual.point.y, expected.point.y);
        }
    }

    fn point(x: f64, y: f64) -> LayoutPoint {
        LayoutPoint { x, y }
    }

    fn edge<'a>(id: &'a str, points: &'a [LayoutPoint]) -> LineHopEdge<'a> {
        LineHopEdge {
            id,
            points,
            curve: None,
            arrow_type_start: None,
            arrow_type_end: None,
        }
    }

    fn arc_config(radius: f64) -> LineHopConfig {
        LineHopConfig {
            jump_radius: radius,
            jump_style: LineHopStyle::Arc,
        }
    }

    #[test]
    fn elk_hop_modes_only_override_crossings_and_preserve_terminal_offsets() {
        let horizontal = [point(0.0, 50.0), point(100.0, 50.0)];
        let vertical = [point(50.0, 0.0), point(50.0, 100.0)];
        let separate = [point(0.0, 150.0), point(100.0, 150.0)];
        let mut edges = [
            edge("horizontal", &horizontal),
            edge("vertical", &vertical),
            edge("separate", &separate),
        ];
        edges[0].curve = Some("rounded");
        edges[0].arrow_type_start = Some("dependency");
        edges[0].arrow_type_end = Some("arrow_point");
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let default_paths = elk_line_hop_paths(&serde_json::json!({}), &edges, &meter).unwrap();
        let arc_paths = elk_line_hop_paths(
            &serde_json::json!({"elk": {"lineHops": true}}),
            &edges,
            &meter,
        )
        .unwrap();
        let gap_paths = elk_line_hop_paths(
            &serde_json::json!({"elk": {"lineHops": "gap"}}),
            &edges,
            &meter,
        )
        .unwrap();
        assert_eq!(default_paths, arc_paths);
        assert_eq!(arc_paths.len(), 1);
        assert_eq!(gap_paths.len(), 1);
        let arc = &arc_paths["horizontal"];
        let gap = &gap_paths["horizontal"];
        assert!(arc.contains("A6,6"), "{arc}");
        assert!(!gap.contains('A'), "{gap}");
        assert_eq!(gap.matches('M').count(), 2, "{gap}");
        for path in [arc, gap] {
            assert!(path.starts_with("M6,50"), "{path}");
            assert!(path.ends_with("L96,50"), "{path}");
        }
        assert_eq!(horizontal[0].x, 0.0);
        assert_eq!(horizontal[1].x, 100.0);
    }

    #[test]
    fn elk_hop_neo_masks_follow_after_paint_for_arcs_gaps_and_markers() {
        let horizontal = [point(0.0, 50.0), point(100.0, 50.0)];
        let vertical = [point(50.0, 0.0), point(50.0, 100.0)];
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        for gap in [false, true] {
            for marker in [None, Some("arrow_point")] {
                let mut edges = [edge("horizontal", &horizontal), edge("vertical", &vertical)];
                edges[0].curve = Some("rounded");
                edges[0].arrow_type_start = marker;
                edges[0].arrow_type_end = marker;
                let config = serde_json::json!({"elk": {"lineHops":
                    if gap { serde_json::json!("gap") } else { serde_json::json!(true) }
                }});
                let paths = elk_line_hop_paths(&config, &edges, &meter).unwrap();
                let hopped = &paths["horizontal"];
                let original = super::super::edge_path::render_path(
                    &mut horizontal.to_vec(),
                    "rounded",
                    marker,
                    marker,
                );
                let original_length = super::super::svg_path_length_from_d(&original).unwrap();
                let new_length = super::super::svg_path_length_from_d(hopped).unwrap();
                let offset = if marker.is_some() { 4.0 } else { 0.0 };
                let expected_length = 100.0 - 2.0 * offset - 12.0
                    + if gap { 0.0 } else { 6.0 * std::f64::consts::PI };
                assert!((new_length - expected_length).abs() < 0.01);
                for dashed in [false, true] {
                    let mut style = String::new();
                    super::super::edge_path::write_neo_edge_mask(
                        &mut style,
                        original_length,
                        marker,
                        marker,
                        dashed,
                        false,
                    );
                    style.push_str(";;fill:none;stroke-dasharray:10,7;");
                    let rewritten = rewrite_style_after_line_hop(&style, hopped);
                    assert!(!rewritten.contains(";;"));
                    let masks: Vec<Vec<f64>> = rewritten
                        .split(';')
                        .filter_map(|part| part.trim().strip_prefix("stroke-dasharray:"))
                        .map(|mask| {
                            mask.split_whitespace()
                                .map(|n| n.parse().unwrap())
                                .collect()
                        })
                        .collect();
                    assert_eq!(masks.len(), 2);
                    assert_eq!(masks[0], masks[1]);
                    let expected_end = if dashed { 2.0 } else { offset };
                    assert_eq!(masks[0].len(), 4);
                    assert_eq!(masks[0][0], 0.0);
                    assert_eq!(masks[0][1], offset);
                    assert_eq!(masks[0][3], expected_end);
                    assert!((masks[0][2] - (new_length - offset - expected_end)).abs() < 1e-9);
                }
            }
        }
    }

    #[test]
    fn after_hop_style_charges_input_before_scanning_or_parsing_the_path() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let error = super::rewrite_style_after_line_hop(
            "stroke-dasharray:0 0 100 0;",
            "M0,0 L100,0",
            &meter,
        )
        .unwrap_err();
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected line-hop style resource limit");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(meter.used(), 0);
    }

    #[test]
    fn after_hop_style_matches_the_source_prefix_and_single_pass_cleanup() {
        let path = "M0,0 L100,0";
        for (style, expected) in [
            ("stroke-dasharray:3;", "stroke-dasharray:3;"),
            ("stroke-dasharray:0 1e2 3 4;", "stroke-dasharray:0 1e2 3 4;"),
            ("stroke-dasharray:0 1 -2 3;", "stroke-dasharray:0 1 -2 3;"),
            ("stroke-dasharray:0 1 2 3e2;", "stroke-dasharray: 0 1 96 3;"),
            (
                "stroke-dasharray:0 .5.9 2 3.5.7;",
                "stroke-dasharray: 0 0.5 96 3.5;",
            ),
            (
                "stroke-dasharray:0 60 1 60;",
                "stroke-dasharray: 0 60 0 60;",
            ),
            (
                "stroke-dasharray:0 0 100 0; ; ;",
                "stroke-dasharray: 0 0 100 0; ;",
            ),
            (
                "stroke-dasharray:3;stroke-dasharray:0 4 92 4;stroke-dasharray:10,7",
                "stroke-dasharray: 0 4 92 4;stroke-dasharray: 0 4 92 4;stroke-dasharray: 0 4 92 4;",
            ),
        ] {
            assert_eq!(
                rewrite_style_after_line_hop(style, path),
                expected,
                "{style}"
            );
        }
    }

    #[test]
    fn elk_hop_curve_eligibility_does_not_remove_crossing_partners() {
        let horizontal = [point(0.0, 50.0), point(100.0, 50.0)];
        let vertical = [point(50.0, 0.0), point(50.0, 100.0)];
        let mut edges = [edge("horizontal", &horizontal), edge("vertical", &vertical)];
        edges[0].curve = Some("rounded");
        edges[1].curve = Some("basis");
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let paths = elk_line_hop_paths(&serde_json::json!({}), &edges, &meter).unwrap();
        assert!(paths.contains_key("horizontal"));
        assert!(!paths.contains_key("vertical"));
        edges[0].curve = Some("basis");
        assert!(
            elk_line_hop_paths(&serde_json::json!({}), &edges, &meter)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn elk_hop_wrapper_honors_disabled_mode_and_the_shared_work_budget() {
        let horizontal = [point(0.0, 50.0), point(100.0, 50.0)];
        let vertical = [point(50.0, 0.0), point(50.0, 100.0)];
        let edges = [edge("horizontal", &horizontal), edge("vertical", &vertical)];
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        assert!(
            elk_line_hop_paths(
                &serde_json::json!({"elk": {"lineHops": false}}),
                &edges,
                &meter,
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(meter.used(), 0);
        let error = elk_line_hop_paths(&serde_json::json!({}), &edges, &meter).unwrap_err();
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected ELK hop collection resource limit");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, 4);
        assert_eq!(meter.used(), 0);
    }

    #[test]
    fn horizontal_segment_hops_over_vertical_regardless_of_edge_order() {
        let horizontal = [point(0.0, 5.0), point(10.0, 5.0)];
        let vertical = [point(5.0, 0.0), point(5.0, 10.0)];

        for edges in [
            [edge("horizontal", &horizontal), edge("vertical", &vertical)],
            [edge("vertical", &vertical), edge("horizontal", &horizontal)],
        ] {
            let crossings = find_edge_intersections(&edges);
            assert_eq!(crossings.len(), 1);
            assert_eq!(crossings[0].jump_edge_id, "horizontal");
            assert_eq!(crossings[0].other_edge_id, "vertical");
            assert_eq!(crossings[0].segment_index, 0);
            assert_eq!(crossings[0].t, 0.5);
            assert_eq!(crossings[0].point.x, 5.0);
            assert_eq!(crossings[0].point.y, 5.0);
        }
    }

    #[test]
    fn same_orientation_crossing_assigns_hop_to_later_edge() {
        let first = [point(0.0, 0.0), point(10.0, 2.0)];
        let second = [point(0.0, 2.0), point(10.0, 0.0)];
        let crossings = find_edge_intersections(&[edge("first", &first), edge("second", &second)]);

        assert_eq!(crossings.len(), 1);
        assert_eq!(crossings[0].jump_edge_id, "second");
        assert_eq!(crossings[0].other_edge_id, "first");
    }

    #[test]
    fn endpoint_touches_shared_endpoints_and_parallel_segments_are_not_crossings() {
        let backbone = [point(0.0, 5.0), point(10.0, 5.0)];
        let t_junction = [point(5.0, 0.0), point(5.0, 5.0)];
        assert!(
            find_edge_intersections(&[edge("backbone", &backbone), edge("touch", &t_junction),])
                .is_empty()
        );

        let fan_a = [point(0.0, 0.0), point(10.0, 5.0)];
        let fan_b = [point(0.0, 0.0), point(10.0, -5.0)];
        assert!(find_edge_intersections(&[edge("a", &fan_a), edge("b", &fan_b)]).is_empty());

        let converge_a = [point(0.0, 0.0), point(10.0, 5.0)];
        let converge_b = [point(0.0, 10.0), point(10.0, 5.0)];
        assert!(
            find_edge_intersections(&[edge("a", &converge_a), edge("b", &converge_b),]).is_empty()
        );

        let parallel_a = [point(0.0, 0.0), point(10.0, 0.0)];
        let parallel_b = [point(0.0, 5.0), point(10.0, 5.0)];
        assert!(
            find_edge_intersections(&[edge("a", &parallel_a), edge("b", &parallel_b),]).is_empty()
        );
    }

    #[test]
    fn dense_segment_candidates_fail_at_the_explicit_work_boundary() {
        let point_sets = (0..100)
            .map(|index| vec![point(0.0, index as f64), point(10.0, 100.0 - index as f64)])
            .collect::<Vec<_>>();
        let ids = (0..point_sets.len())
            .map(|index| format!("edge-{index}"))
            .collect::<Vec<_>>();
        let edges = ids
            .iter()
            .zip(&point_sets)
            .map(|(id, points)| edge(id, points))
            .collect::<Vec<_>>();
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 850)
            .unwrap();
        let meter = OperationWorkMeter::new(limits);

        let error = super::find_edge_intersections(&edges, &meter).unwrap_err();
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected line-hop segment-pair resource limit");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, 851);
        assert_eq!(error.max, 850);
        assert_eq!(meter.used(), 850);
    }

    #[test]
    fn segment_pair_budget_is_inclusive() {
        let point_sets = (0..3)
            .map(|index| vec![point(0.0, index as f64), point(10.0, 3.0 - index as f64)])
            .collect::<Vec<_>>();
        let ids = (0..point_sets.len())
            .map(|index| format!("edge-{index}"))
            .collect::<Vec<_>>();
        let edges = ids
            .iter()
            .zip(&point_sets)
            .map(|(id, points)| edge(id, points))
            .collect::<Vec<_>>();
        let exact_limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 18)
            .unwrap();
        let exact_meter = OperationWorkMeter::new(exact_limits);

        super::find_edge_intersections(&edges, &exact_meter)
            .expect("three segments have exactly three cross-edge candidate pairs");

        let narrow_limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 17)
            .unwrap();
        let narrow_meter = OperationWorkMeter::new(narrow_limits);
        let error = super::find_edge_intersections(&edges, &narrow_meter).unwrap_err();
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected line-hop segment-pair resource limit");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, 18);
        assert_eq!(error.max, 17);
    }

    #[test]
    fn segment_pairs_share_work_already_consumed_by_layout() {
        let point_sets = (0..3)
            .map(|index| vec![point(0.0, index as f64), point(10.0, 3.0 - index as f64)])
            .collect::<Vec<_>>();
        let ids = (0..point_sets.len())
            .map(|index| format!("edge-{index}"))
            .collect::<Vec<_>>();
        let edges = ids
            .iter()
            .zip(&point_sets)
            .map(|(id, points)| edge(id, points))
            .collect::<Vec<_>>();
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 19)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        meter.charge(2).unwrap();

        let error = super::find_edge_intersections(&edges, &meter).unwrap_err();
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected cumulative line-hop resource limit");
        };
        assert_eq!(error.actual, 20);
        assert_eq!(error.max, 19);
        assert_eq!(meter.used(), 14);
    }

    #[test]
    fn spatially_separate_edges_still_pay_index_and_sort_cost() {
        let point_sets = (0..2_000)
            .map(|index| {
                let x = index as f64 * 2.0;
                vec![point(x, 0.0), point(x + 1.0, 1.0)]
            })
            .collect::<Vec<_>>();
        let ids = (0..point_sets.len())
            .map(|index| format!("edge-{index}"))
            .collect::<Vec<_>>();
        let edges = ids
            .iter()
            .zip(&point_sets)
            .map(|(id, points)| edge(id, points))
            .collect::<Vec<_>>();
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let meter = OperationWorkMeter::new(limits);

        let error = super::find_edge_intersections(&edges, &meter).unwrap_err();
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected line-hop index resource limit");
        };
        assert_eq!(error.actual, 24_000);
        assert_eq!(error.max, 1);
        assert_eq!(meter.used(), 0);
    }

    #[test]
    fn sweep_matches_the_pinned_mermaid_pairwise_predicate() {
        let mut state = 0x4d59_5df4_d0f3_3173u64;
        let mut next_coordinate = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % 101) as f64
        };
        let point_sets = (0..24)
            .map(|_| {
                (0..6)
                    .map(|_| point(next_coordinate(), next_coordinate()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let ids = (0..point_sets.len())
            .map(|index| format!("edge-{index}"))
            .collect::<Vec<_>>();
        let edges = ids
            .iter()
            .zip(&point_sets)
            .map(|(id, points)| edge(id, points))
            .collect::<Vec<_>>();

        let expected = brute_force_edge_intersections(&edges);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let actual = super::find_edge_intersections(&edges, &meter).expect("unbounded sweep");
        assert_crossings_equal(&actual, &expected);
    }

    #[test]
    fn emits_arc_and_gap_paths_without_rewriting_the_other_edge() {
        let horizontal = [point(0.0, 5.0), point(10.0, 5.0)];
        let vertical = [point(5.0, 0.0), point(5.0, 10.0)];
        let edges = [edge("horizontal", &horizontal), edge("vertical", &vertical)];

        let arc_paths = line_hop_overrides(&edges, arc_config(1.0));
        assert_eq!(arc_paths["horizontal"], "M0,5 L4,5 A1,1 0 0 1 6,5 L10,5");
        assert!(!arc_paths.contains_key("vertical"));

        let gap_paths = line_hop_overrides(
            &edges,
            LineHopConfig {
                jump_style: LineHopStyle::Gap,
                ..arc_config(1.0)
            },
        );
        assert_eq!(gap_paths["horizontal"], "M0,5 L4,5 M6,5 L10,5");
        assert!(!gap_paths.contains_key("vertical"));
    }

    #[test]
    fn crossings_are_emitted_in_segment_order_and_adjacent_radii_are_clamped() {
        let vertical_a = [point(4.2, 0.0), point(4.2, 10.0)];
        let vertical_b = [point(5.8, 0.0), point(5.8, 10.0)];
        let horizontal = [point(0.0, 5.0), point(10.0, 5.0)];
        let edges = [
            edge("vertical-a", &vertical_a),
            edge("vertical-b", &vertical_b),
            edge("horizontal", &horizontal),
        ];

        assert_eq!(
            line_hop_overrides(&edges, arc_config(1.0))["horizontal"],
            "M0,5 L3.4,5 A0.8,0.8 0 0 1 5,5 L5,5 A0.8,0.8 0 0 1 6.6,5 L10,5"
        );
    }

    #[test]
    fn requested_radius_is_clamped_to_segment_boundaries_and_tiny_hops_are_omitted() {
        let close_vertical = [point(0.5, -1.0), point(0.5, 1.0)];
        let horizontal = [point(0.0, 0.0), point(10.0, 0.0)];
        let edges = [
            edge("vertical", &close_vertical),
            edge("horizontal", &horizontal),
        ];
        assert_eq!(
            line_hop_overrides(&edges, arc_config(2.0))["horizontal"],
            "M0,0 L10,0"
        );

        let almost_at_start = [point(0.0005, -1.0), point(0.0005, 1.0)];
        let tiny_edges = [
            edge("vertical", &almost_at_start),
            edge("horizontal", &horizontal),
        ];
        let paths = line_hop_overrides(&tiny_edges, arc_config(2.0));
        assert_eq!(paths["horizontal"], "M0,0 L10,0");
        assert!(!paths.contains_key("vertical"));
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let overrides = elk_line_hop_paths(&serde_json::json!({}), &tiny_edges, &meter).unwrap();
        assert_eq!(overrides["horizontal"], "M0,0 L10,0");
    }

    #[test]
    fn rounded_edges_preserve_corners_when_a_hop_has_too_little_room() {
        let vertical = [point(14.0, 0.0), point(14.0, 10.0)];
        let rounded_points = [point(0.0, 5.0), point(20.0, 5.0), point(20.0, 15.0)];
        let rounded = LineHopEdge {
            curve: Some("rounded"),
            ..edge("rounded", &rounded_points)
        };
        let paths = line_hop_overrides(&[edge("vertical", &vertical), rounded], arc_config(5.0));

        assert_eq!(paths["rounded"], "M0,5 L15,5 Q20,5 20,10 L20,15");
        assert_eq!(ROUNDED_CORNER_RADIUS, 5.0);
    }

    #[test]
    fn state_api_014_crossing_inside_either_direction_of_other_corner_is_ignored() {
        // These are the exact data-points of edge1 and edge5 in the pinned State
        // upstream_pkgtests_mermaidapi_spec_014 SVG. The crossing is only 2px
        // from edge1's final bend, inside its 7.071px rounded stretch.
        let edge1_points = [
            point(202.0, 41.125),
            point(222.0, 41.125),
            point(222.0, 12.0),
            point(330.0, 12.0),
            point(506.0, 12.0),
            point(614.0, 12.0),
            point(614.0, 35.875),
            point(634.0, 35.875),
        ];
        let edge5_points = [
            point(574.0, 51.5),
            point(594.0, 51.5),
            point(594.0, 33.875),
            point(634.0, 33.875),
        ];
        let hopping = LineHopEdge {
            curve: Some("rounded"),
            arrow_type_end: Some("arrow_barb_neo"),
            ..edge("edge5", &edge5_points)
        };
        for reverse in [false, true] {
            let mut points = edge1_points.to_vec();
            if reverse {
                points.reverse();
            }
            for curve in [Some("rounded"), Some("linear")] {
                let other = LineHopEdge {
                    curve,
                    ..edge("edge1", &points)
                };
                for edges in [[other, hopping], [hopping, other]] {
                    let crossings = find_edge_intersections(&edges);
                    let paths = line_hop_overrides(&edges, arc_config(6.0));
                    if curve == Some("rounded") {
                        assert!(crossings.is_empty());
                        assert!(paths.is_empty());
                    } else {
                        assert_eq!(crossings.len(), 1);
                        assert!(paths["edge5"].contains("A6,6"));
                    }
                }
            }
        }

        let mut clear_points = edge1_points;
        clear_points[6].y = 45.875;
        clear_points[7].y = 45.875;
        let clear = LineHopEdge {
            curve: Some("rounded"),
            ..edge("edge1", &clear_points)
        };
        let edges = [clear, hopping];
        assert_eq!(find_edge_intersections(&edges).len(), 1);
        assert!(line_hop_overrides(&edges, arc_config(6.0))["edge5"].contains("A6,6"));
    }

    #[test]
    fn rounded_corner_crossing_boundary_is_strict() {
        let points = [point(0.0, 0.0), point(20.0, 0.0), point(20.0, 10.0)];
        let rounded = LineHopEdge {
            curve: Some("rounded"),
            ..edge("rounded", &points)
        };
        assert!(!crossing_sits_in_rounded_corner(rounded, 0, 0.75));
        assert!(crossing_sits_in_rounded_corner(rounded, 0, 0.8));
        assert!(crossing_sits_in_rounded_corner(rounded, 1, 0.4));
        assert!(!crossing_sits_in_rounded_corner(rounded, 1, 0.5));
    }

    #[test]
    fn rounded_hops_keep_two_pixels_clear_of_the_corner() {
        // Port of lineJump.spec.ts's just-roomy-enough radius-6 example.
        let points = [point(100.0, 0.0), point(100.0, 100.0), point(300.0, 100.0)];
        let vertical = [point(113.0, 0.0), point(113.0, 200.0)];
        let rounded = LineHopEdge {
            curve: Some("rounded"),
            ..edge("rounded", &points)
        };
        let paths = line_hop_overrides(&[rounded, edge("vertical", &vertical)], arc_config(6.0));
        assert!(paths["rounded"].contains("L109.071,100 A3.929,3.929 0 0 1 116.929,100"));
    }

    #[test]
    fn corner_squeezed_hop_is_removed_before_it_can_shrink_its_neighbour() {
        let points = [point(0.0, -20.0), point(0.0, 0.0), point(100.0, 0.0)];
        let squeezed = [point(10.0, -10.0), point(10.0, 10.0)];
        let usable = [point(17.0, -10.0), point(17.0, 10.0)];
        let rounded = LineHopEdge {
            curve: Some("rounded"),
            ..edge("rounded", &points)
        };
        let edges = [
            rounded,
            edge("squeezed", &squeezed),
            edge("usable", &usable),
        ];
        assert_eq!(find_edge_intersections(&edges).len(), 2);
        let paths = line_hop_overrides(&edges, arc_config(6.0));
        let path = &paths["rounded"];
        assert_eq!(path.matches('A').count(), 1);
        assert!(path.contains("L11,0 A6,6 0 0 1 23,0"));
    }

    #[test]
    fn adjacency_squeezed_hops_are_filtered_again_before_emission() {
        let horizontal = [point(0.0, 5.0), point(10.0, 5.0)];
        let first = [point(4.5, 0.0), point(4.5, 10.0)];
        let second = [point(5.5, 0.0), point(5.5, 10.0)];
        let edges = [
            edge("h", &horizontal),
            edge("a", &first),
            edge("b", &second),
        ];
        assert_eq!(find_edge_intersections(&edges).len(), 2);
        let paths = line_hop_overrides(&edges, arc_config(1.0));
        assert_eq!(paths["h"], "M0,5 L10,5");
    }

    #[test]
    fn marker_offsets_match_upstream_table_and_shift_rewritten_endpoints() {
        let expected = [
            ("aggregation", 17.25),
            ("extension", 17.25),
            ("composition", 17.25),
            ("dependency", 6.0),
            ("lollipop", 13.5),
            ("arrow_point", 4.0),
            ("arrow_barb", 0.0),
            ("arrow_barb_neo", 5.5),
        ];
        for (name, offset) in expected {
            assert_eq!(marker_offset_for(Some(name)), Some(offset));
        }
        assert_eq!(marker_offset_for(Some("arrow_cross")), None);

        let horizontal_points = [point(0.0, 5.0), point(20.0, 5.0)];
        let horizontal = LineHopEdge {
            arrow_type_end: Some("arrow_point"),
            ..edge("horizontal", &horizontal_points)
        };
        let vertical = [point(10.0, 0.0), point(10.0, 20.0)];
        let paths = line_hop_overrides(&[horizontal, edge("vertical", &vertical)], arc_config(1.0));
        assert_eq!(paths["horizontal"], "M0,5 L9,5 A1,1 0 0 1 11,5 L16,5");

        let long_horizontal_points = [point(0.0, 5.0), point(40.0, 5.0)];
        let both_markers = LineHopEdge {
            arrow_type_start: Some("arrow_point"),
            arrow_type_end: Some("dependency"),
            ..edge("both-markers", &long_horizontal_points)
        };
        let vertical = [point(20.0, 0.0), point(20.0, 20.0)];
        let paths = line_hop_overrides(
            &[both_markers, edge("vertical", &vertical)],
            arc_config(1.0),
        );
        assert_eq!(paths["both-markers"], "M4,5 L19,5 A1,1 0 0 1 21,5 L34,5");
    }

    #[test]
    fn reverse_segments_use_the_upward_or_rightward_arc_sweep() {
        let descending_horizontal = [point(10.0, 5.0), point(0.0, 5.0)];
        let vertical = [point(5.0, 0.0), point(5.0, 10.0)];
        let paths = line_hop_overrides(
            &[
                edge("horizontal", &descending_horizontal),
                edge("vertical", &vertical),
            ],
            arc_config(1.0),
        );
        assert_eq!(paths["horizontal"], "M10,5 L6,5 A1,1 0 0 0 4,5 L0,5");

        let down_start = point(5.0, 0.0);
        let down_end = point(5.0, 10.0);
        let up_start = point(5.0, 10.0);
        let up_end = point(5.0, 0.0);
        assert_eq!(
            arc_sweep_flag(Segment {
                start: &down_start,
                end: &down_end,
            }),
            1
        );
        assert_eq!(
            arc_sweep_flag(Segment {
                start: &up_start,
                end: &up_end,
            }),
            0
        );
    }

    #[test]
    fn crossing_free_edges_have_no_overrides_and_keep_their_work_admission() {
        let left = [point(0.0, 5.0), point(10.0, 5.0)];
        let right = [point(20.0, 5.0), point(30.0, 5.0)];
        let edges = [edge("left", &left), edge("right", &right)];
        for (limit, accepted_work, expected) in [(5, 0, 6), (9, 6, 10), (10, 10, 10)] {
            let policy = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap();
            let meter = OperationWorkMeter::new(policy);
            let result = super::line_hop_overrides(&edges, arc_config(1.0), |_| true, &meter);
            if limit < expected {
                let crate::Error::ResourceLimitExceeded(error) = result.unwrap_err() else {
                    panic!("expected line-hop resource limit");
                };
                assert_eq!(error.limit, "max_layout_work_units");
                assert_eq!(error.actual, expected);
            } else {
                assert!(result.unwrap().is_empty());
            }
            assert_eq!(meter.used(), accepted_work);
        }
    }

    #[test]
    fn curve_and_rendered_path_guards_match_upstream_whitelists() {
        for curve in [
            None,
            Some("linear"),
            Some("rounded"),
            Some("step"),
            Some("stepBefore"),
            Some("stepAfter"),
        ] {
            assert!(curve_supports_line_hops(curve), "curve={curve:?}");
        }
        for curve in [Some("basis"), Some("monotoneX"), Some("cardinal")] {
            assert!(!curve_supports_line_hops(curve), "curve={curve:?}");
        }

        for path in ["", "M0,5 L10,5", "M5,0 L5,4 L5,10", "m1e-3,+2 l.5,-4"] {
            assert!(is_straight_path(path), "path={path}");
        }
        for path in [
            "M0,0 C1,1 2,2 3,3",
            "M0,0 Q1,1 2,2",
            "M0,0 L1,1 A1,1 0 0 1 2,2",
            "M0,0 H2",
        ] {
            assert!(!is_straight_path(path), "path={path}");
        }
    }
}
