//! Geometric acceptance checks on the visible shafts of a rendered graph.
//!
//! Populate one body, edge and label per visible occurrence, keeping the indices for diagnostics.
//! Supply measured boxes and outer shape contours in the renderer's drawing frame, where the
//! layout is expected to have nonnegative coordinates. Read `Edge::d` from the final SVG and
//! supply its local-to-drawing transform for nested paths. Include every expected edge, even
//! if its `d` is missing (use an empty string).

use kurbo::{
    Affine, BezPath, Line, ParamCurve, ParamCurveArclen, ParamCurveExtrema, ParamCurveNearest,
    PathEl, PathSeg, Point, Rect, Shape, Size, Vec2,
};
use svgtypes::{SimplePathSegment, SimplifyingPathParser};

/// SVG drawing units. The archived gallery uses this flatness for box hits and crossings.
const FLATNESS: f64 = 0.15;
/// Permitted distance from a restored shaft tip to the painted outline, in drawing pixels.
const CONTACT_TOLERANCE: f64 = 0.25;
/// Angular allowance for directions reconstructed from three-decimal SVG coordinates.
const NORMAL_TOLERANCE_DEG: f64 = 0.1;
const BOX_EPSILON: f64 = 1e-6;
const ARCLEN_ACCURACY: f64 = 1e-5;

#[derive(Debug, Default)]
pub(crate) struct Scene<'a> {
    pub bodies: Vec<Body>,
    pub labels: Vec<Label>,
    pub edges: Vec<Edge<'a>>,
}

#[derive(Debug)]
pub(crate) struct Body {
    pub bounds: Rect,
    /// Index of the immediate enclosing group. Root-level siblings have `None`.
    pub parent: Option<usize>,
    /// Outer painted contour in drawing coordinates. `None` denotes a sharp rectangle.
    /// The gallery uses the measured rectangle for foreign-node hits, including shaped nodes.
    pub contour: Option<BezPath>,
    pub is_group: bool,
}

impl Body {
    pub fn node(bounds: Rect) -> Self {
        Self {
            bounds,
            parent: None,
            contour: None,
            is_group: false,
        }
    }

    pub fn group(bounds: Rect) -> Self {
        Self {
            is_group: true,
            ..Self::node(bounds)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LabelOwner {
    /// Node label or group title.
    Body(usize),
    /// Middle or terminal label of this edge occurrence.
    Edge(usize),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Label {
    pub owner: LabelOwner,
    /// Measured bounds at the painted label position.
    pub bounds: Rect,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CrossAxis {
    /// Top-to-bottom or bottom-to-top flow.
    X,
    /// Left-to-right or right-to-left flow.
    Y,
}

impl CrossAxis {
    fn coordinate(self, point: Point) -> f64 {
        match self {
            Self::X => point.x,
            Self::Y => point.y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum End {
    Start,
    End,
}

/// Opening behind the shaft endpoint in drawing units, measured from the referenced marker.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PointMarker {
    pub depth: f64,
    pub half_width: f64,
}

impl PointMarker {
    /// A vertically centered point marker with SVG's default `meet` aspect ratio. Supply its
    /// effective viewport in drawing units (including stroke width for `markerUnits=strokeWidth`).
    pub fn from_view_box(end: End, view_box: Rect, viewport: Size, ref_x: f64) -> Option<Self> {
        if !view_box.is_finite()
            || !viewport.is_finite()
            || view_box.width() <= 0.0
            || view_box.height() <= 0.0
            || viewport.width <= 0.0
            || viewport.height <= 0.0
        {
            return None;
        }
        let scale = (viewport.width / view_box.width()).min(viewport.height / view_box.height());
        let back = match end {
            End::Start => view_box.x1 - ref_x,
            End::End => ref_x - view_box.x0,
        };
        let marker = Self {
            depth: back * scale,
            half_width: view_box.height() * 0.5 * scale,
        };
        marker.is_valid().then_some(marker)
    }

    fn is_valid(self) -> bool {
        self.depth.is_finite()
            && self.depth > 0.0
            && self.half_width.is_finite()
            && self.half_width > 0.0
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Endpoint {
    pub body: usize,
    /// Declared port normal in drawing coordinates, pointing from the body into the shaft at
    /// either end. Its magnitude is immaterial. Leave absent when the port has no declaration.
    pub outward_normal: Option<Vec2>,
    /// Distance removed from the shaft at this end by the painter, in drawing units.
    pub setback: f64,
    /// Only point markers have the gallery's angular-opening check. Setback applies to any end.
    pub marker: Option<PointMarker>,
}

impl Endpoint {
    fn new(body: usize) -> Self {
        Self {
            body,
            outward_normal: None,
            setback: 0.0,
            marker: None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Edge<'a> {
    pub start: Endpoint,
    pub end: Endpoint,
    pub d: &'a str,
    pub transform: Affine,
    pub cross_axis: CrossAxis,
}

impl<'a> Edge<'a> {
    pub fn new(from: usize, to: usize, d: &'a str, cross_axis: CrossAxis) -> Self {
        Self {
            start: Endpoint::new(from),
            end: Endpoint::new(to),
            d,
            transform: Affine::IDENTITY,
            cross_axis,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    Body(usize),
    Edge(usize),
    Label(usize),
}

#[derive(Debug, PartialEq)]
pub(crate) enum Violation {
    InvalidOwner {
        item: Item,
    },
    InvalidGeometry {
        item: Item,
    },
    InvalidPath {
        edge: usize,
        reason: String,
    },
    SiblingOverlap {
        first: usize,
        second: usize,
    },
    LabelOverlap {
        first: usize,
        second: usize,
    },
    LabelNodeOverlap {
        label: usize,
        node: usize,
    },
    OutsideGroup {
        item: Item,
        group: usize,
    },
    ForeignNodeHit {
        edge: usize,
        node: usize,
    },
    ForeignGroupHit {
        edge: usize,
        group: usize,
    },
    ForeignLabelHit {
        edge: usize,
        label: usize,
    },
    EndpointContour {
        edge: usize,
        end: End,
        gap: f64,
    },
    InvalidNormal {
        edge: usize,
        end: End,
    },
    EndpointNormal {
        edge: usize,
        end: End,
        deviation_deg: f64,
    },
    MarkerAngle {
        edge: usize,
        end: End,
        sample_angle_deg: Option<f64>,
        half_opening_deg: f64,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EdgeMetrics {
    pub length: f64,
    pub sideways_return: f64,
}

#[derive(Debug, Default)]
pub(crate) struct Report {
    pub hard: Vec<Violation>,
    /// Number of shape contacts tested, including failed checks.
    pub contacts_checked: usize,
    /// Number of declared port normals checked against final shaft tangents, including failures.
    pub normals_checked: usize,
    /// Number of point-marker openings sampled, including failed checks.
    pub markers_checked: usize,
    /// Properly intersecting pairs of distinct shafts, once per pair, in input order.
    pub crossings: Vec<(usize, usize)>,
    /// Indexed by edge occurrence. An unreadable or empty shaft has no metrics.
    pub edges: Vec<Option<EdgeMetrics>>,
    /// All historical box pairs, including overlaps involving only body labels. Every overlap
    /// involving an edge label also has a corresponding violation in `hard`.
    pub label_overlaps: Vec<(usize, usize)>,
    pub label_node_overlaps: Vec<(usize, usize)>,
}

/// Absolute offsets from the midpoint of the two outermost branch centers, in drawing units.
/// Supply the fixture's distinct, adjacent-rank branches between this fork and join.
pub(crate) fn fork_join_offsets(
    scene: &Scene<'_>,
    fork: usize,
    join: usize,
    branches: &[usize],
    axis: CrossAxis,
) -> Option<(f64, f64)> {
    let center = |id: usize| {
        let rect = scene.bodies.get(id)?.bounds;
        valid_rect(rect).then(|| axis.coordinate(rect.center()))
    };
    if fork == join || branches.len() < 2 {
        return None;
    }
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for (i, &branch) in branches.iter().enumerate() {
        if branch == fork
            || branch == join
            || branches[..i].contains(&branch)
            || !scene
                .edges
                .iter()
                .any(|e| e.start.body == fork && e.end.body == branch)
            || !scene
                .edges
                .iter()
                .any(|e| e.start.body == branch && e.end.body == join)
        {
            return None;
        }
        let pos = center(branch)?;
        low = low.min(pos);
        high = high.max(pos);
    }
    let midpoint = low.midpoint(high);
    Some((
        (center(fork)? - midpoint).abs(),
        (center(join)? - midpoint).abs(),
    ))
}

fn valid_rect(r: Rect) -> bool {
    r.is_finite() && r.x0 <= r.x1 && r.y0 <= r.y1
}

fn normalized_rect(r: Rect) -> bool {
    valid_rect(r) && r.x0 >= -BOX_EPSILON && r.y0 >= -BOX_EPSILON
}

fn interiors_overlap(a: Rect, b: Rect) -> bool {
    let area = a.intersect(b);
    area.width() > BOX_EPSILON && area.height() > BOX_EPSILON
}

fn check_containment(
    scene: &Scene<'_>,
    item: Item,
    bounds: Rect,
    group: usize,
    report: &mut Report,
) {
    let outer = scene.bodies[group].bounds;
    if !outer
        .inflate(BOX_EPSILON, BOX_EPSILON)
        .contains_rect(bounds)
    {
        report.hard.push(Violation::OutsideGroup { item, group });
    }
}

/// Each list runs from the immediate parent to the root. Malformed gallery ownership is a
/// failed check even if all boxes happen to lie inside one another.
fn check_bodies(scene: &Scene<'_>, report: &mut Report) -> Vec<Vec<usize>> {
    let mut ancestry = Vec::with_capacity(scene.bodies.len());
    for (id, body) in scene.bodies.iter().enumerate() {
        let item = Item::Body(id);
        if !normalized_rect(body.bounds)
            || body.contour.as_ref().is_some_and(|p| {
                !p.is_finite()
                    || p.segments().next().is_none()
                    || !normalized_rect(p.bounding_box())
            })
        {
            report.hard.push(Violation::InvalidGeometry { item });
        }
        let mut parents = Vec::new();
        let mut next = body.parent;
        while let Some(parent) = next {
            let group = scene.bodies.get(parent).filter(|b| b.is_group);
            let Some(group) = group.filter(|_| parent != id && !parents.contains(&parent)) else {
                report.hard.push(Violation::InvalidOwner { item });
                break;
            };
            parents.push(parent);
            next = group.parent;
        }
        if let Some(&parent) = parents.first() {
            check_containment(scene, item, body.bounds, parent, report);
        }
        ancestry.push(parents);
        for (other, sibling) in scene.bodies[..id].iter().enumerate() {
            if body.parent == sibling.parent && interiors_overlap(body.bounds, sibling.bounds) {
                report.hard.push(Violation::SiblingOverlap {
                    first: other,
                    second: id,
                });
            }
        }
    }
    ancestry
}

fn edge_group(edge: &Edge<'_>, ancestry: &[Vec<usize>]) -> Option<usize> {
    let (from, to) = (edge.start.body, edge.end.body);
    let a = ancestry.get(from)?;
    let b = ancestry.get(to)?;
    if a.contains(&to) {
        Some(to)
    } else if b.contains(&from) {
        Some(from)
    } else {
        a.iter().copied().find(|g| b.contains(g))
    }
}

fn check_labels(scene: &Scene<'_>, ancestry: &[Vec<usize>], report: &mut Report) {
    for (id, label) in scene.labels.iter().enumerate() {
        let item = Item::Label(id);
        if !normalized_rect(label.bounds) {
            report.hard.push(Violation::InvalidGeometry { item });
        }
        let owner_group = match label.owner {
            LabelOwner::Body(i) => scene.bodies.get(i).map(|b| {
                if b.is_group {
                    Some(i)
                } else {
                    ancestry[i].first().copied()
                }
            }),
            LabelOwner::Edge(i) => scene.edges.get(i).map(|e| edge_group(e, ancestry)),
        };
        match owner_group {
            None => report.hard.push(Violation::InvalidOwner { item }),
            Some(Some(group)) => check_containment(scene, item, label.bounds, group, report),
            Some(None) => {}
        }
        let edge_label = matches!(label.owner, LabelOwner::Edge(_));
        for (other, previous) in scene.labels[..id].iter().enumerate() {
            if interiors_overlap(label.bounds, previous.bounds) {
                report.label_overlaps.push((other, id));
                if edge_label || matches!(previous.owner, LabelOwner::Edge(_)) {
                    report.hard.push(Violation::LabelOverlap {
                        first: other,
                        second: id,
                    });
                }
            }
        }
        for (node, body) in scene.bodies.iter().enumerate() {
            if !body.is_group
                && label.owner != LabelOwner::Body(node)
                && interiors_overlap(label.bounds, body.bounds)
            {
                report.label_node_overlaps.push((id, node));
                if edge_label {
                    report
                        .hard
                        .push(Violation::LabelNodeOverlap { label: id, node });
                }
            }
        }
    }
}

struct Shaft {
    bounds: Rect,
    lines: Vec<Line>,
    segments: Vec<(PathSeg, f64)>,
    metrics: EdgeMetrics,
}

impl Shaft {
    fn read(edge: &Edge<'_>) -> Result<Self, String> {
        let mut path = BezPath::new();
        for command in SimplifyingPathParser::from(edge.d) {
            let el = match command.map_err(|err| err.to_string())? {
                SimplePathSegment::MoveTo { x, y } => PathEl::MoveTo((x, y).into()),
                SimplePathSegment::LineTo { x, y } => PathEl::LineTo((x, y).into()),
                SimplePathSegment::Quadratic { x1, y1, x, y } => {
                    PathEl::QuadTo((x1, y1).into(), (x, y).into())
                }
                SimplePathSegment::CurveTo {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                } => PathEl::CurveTo((x1, y1).into(), (x2, y2).into(), (x, y).into()),
                SimplePathSegment::ClosePath => PathEl::ClosePath,
            };
            path.push(edge.transform * el);
        }
        if !path.is_finite() {
            return Err("non-finite transformed path".into());
        }

        let axis = edge.cross_axis;
        let mut segments = Vec::new();
        let mut lines = Vec::new();
        let mut length = 0.0;
        let mut increasing = 0.0;
        let mut decreasing = 0.0;
        for seg in path.segments() {
            let len = seg.arclen(ARCLEN_ACCURACY);
            if !len.is_finite() {
                return Err("non-finite segment length".into());
            }
            if len == 0.0 {
                continue;
            }
            segments.push((seg, len));
            length += len;
            let mut previous = axis.coordinate(seg.start());
            let extrema = seg.extrema();
            append_flat_lines(seg, &extrema, &mut lines);
            for t in extrema.into_iter().chain([1.0]) {
                let pos = axis.coordinate(seg.eval(t));
                increasing += (pos - previous).max(0.0);
                decreasing += (previous - pos).max(0.0);
                previous = pos;
            }
        }
        if segments.is_empty() {
            return Err("no visible shaft".into());
        }
        let bounds = path.bounding_box();
        if !length.is_finite()
            || !increasing.is_finite()
            || !decreasing.is_finite()
            || !valid_rect(bounds)
        {
            return Err("non-finite curve geometry".into());
        }

        // min(forward, backward) = (total variation - |net displacement|) / 2. This also
        // gives exact zero on a monotone shaft and excludes pen-up jumps between subpaths.
        let sideways_return = increasing.min(decreasing);
        Ok(Self {
            bounds,
            lines,
            segments,
            metrics: EdgeMetrics {
                length,
                sideways_return,
            },
        })
    }

    /// Orient a segment away from the probed endpoint, including at the target of the edge.
    fn segment_from(&self, end: End, offset: usize) -> (PathSeg, f64) {
        match end {
            End::Start => self.segments[offset],
            End::End => {
                let (seg, len) = self.segments[self.segments.len() - 1 - offset];
                (seg.reverse(), len)
            }
        }
    }

    fn sample(&self, end: End, mut distance: f64) -> Point {
        for i in 0..self.segments.len() {
            let (seg, len) = self.segment_from(end, i);
            if distance < len {
                return seg.eval(seg.inv_arclen(distance, ARCLEN_ACCURACY));
            }
            if distance == len || i + 1 == self.segments.len() {
                return seg.end();
            }
            distance -= len;
        }
        unreachable!("a shaft has at least one positive-length segment")
    }

    fn hits(&self, rect: Rect) -> bool {
        self.bounds.overlaps(rect) && self.lines.iter().any(|&l| enters_rect(l, rect))
    }

    fn crosses(&self, other: &Self) -> bool {
        self.bounds.overlaps(other.bounds)
            && self
                .lines
                .iter()
                .any(|&a| other.lines.iter().any(|&b| proper_intersection(a, b)))
    }
}

fn append_flat_lines(seg: PathSeg, extrema: &[f64], lines: &mut Vec<Line>) {
    if let PathSeg::Line(line) = seg {
        lines.push(line);
        return;
    }
    let flatten = |curve: PathSeg, lines: &mut Vec<Line>| {
        let mut previous = curve.start();
        kurbo::flatten(curve.path_elements(FLATNESS), FLATNESS, |el| {
            if let PathEl::LineTo(point) = el
                && previous != point
            {
                lines.push(Line::new(previous, point));
                previous = point;
            }
        });
    };
    let first = lines.len();
    flatten(seg, lines);
    // Kurbo can reduce a collinear Bézier to its endpoint chord even when it reverses far
    // beyond that chord. Split only if a true extremum lies outside the flattening tolerance.
    if extrema.iter().any(|&t| {
        let point = seg.eval(t);
        !lines[first..]
            .iter()
            .any(|l| l.nearest(point, FLATNESS).distance_sq <= FLATNESS * FLATNESS)
    }) {
        lines.truncate(first);
        let mut lo = 0.0;
        for hi in extrema.iter().copied().chain([1.0]) {
            if hi > lo {
                flatten(seg.subsegment(lo..hi), lines);
                lo = hi;
            }
        }
    }
}

/// Positive length inside the open measured box, with the gallery's 1e-6 inset.
fn enters_rect(line: Line, rect: Rect) -> bool {
    let inner = rect.inflate(-BOX_EPSILON, -BOX_EPSILON);
    if inner.width() <= 0.0 || inner.height() <= 0.0 {
        return false;
    }
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    let delta = line.p1 - line.p0;
    for (origin, step, min, max) in [
        (line.p0.x, delta.x, inner.x0, inner.x1),
        (line.p0.y, delta.y, inner.y0, inner.y1),
    ] {
        if step.abs() < 1e-9 {
            if origin <= min || origin >= max {
                return false;
            }
        } else {
            let a = (min - origin) / step;
            let b = (max - origin) / step;
            lo = lo.max(a.min(b));
            hi = hi.min(a.max(b));
            if lo >= hi {
                return false;
            }
        }
    }
    true
}

/// Historical proper segment-interior intersection, including for edges incident to one node.
fn proper_intersection(a: Line, b: Line) -> bool {
    const EPS: f64 = 1e-8;
    let da = a.p1 - a.p0;
    let db = b.p1 - b.p0;
    let det = da.cross(db);
    if det.abs() < EPS {
        return false;
    }
    let gap = b.p0 - a.p0;
    let t = gap.cross(db) / det;
    let u = gap.cross(da) / det;
    t > EPS && t < 1.0 - EPS && u > EPS && u < 1.0 - EPS
}

fn start_tangent(seg: PathSeg) -> Vec2 {
    let start = seg.start();
    let towards = match seg {
        PathSeg::Quad(q) if q.p1 != start => q.p1,
        PathSeg::Cubic(c) if c.p1 != start => c.p1,
        PathSeg::Cubic(c) if c.p2 != start => c.p2,
        _ => seg.end(),
    };
    (towards - start).normalize()
}

fn contour_gap(body: &Body, point: Point) -> f64 {
    let nearest = |seg: PathSeg| seg.nearest(point, ARCLEN_ACCURACY).distance_sq;
    match &body.contour {
        Some(path) => path.segments().map(nearest).fold(f64::INFINITY, f64::min),
        None => body
            .bounds
            .path_segments(FLATNESS)
            .map(nearest)
            .fold(f64::INFINITY, f64::min),
    }
    .sqrt()
}

fn check_endpoint(
    edge: usize,
    end: End,
    endpoint: Endpoint,
    body: &Body,
    shaft: &Shaft,
    report: &mut Report,
) {
    let (first, _) = shaft.segment_from(end, 0);
    let origin = first.start();
    let tangent = start_tangent(first);
    if let Some(normal) = endpoint.outward_normal {
        report.normals_checked += 1;
        if !normal.is_finite() || normal == Vec2::ZERO {
            report.hard.push(Violation::InvalidNormal { edge, end });
        } else {
            // A signed dot distinguishes an inward shaft from an outward shaft on the same axis.
            let normal = normal / normal.x.abs().max(normal.y.abs());
            let deviation_deg = tangent
                .cross(normal)
                .abs()
                .atan2(tangent.dot(normal))
                .to_degrees();
            if !deviation_deg.is_finite() || deviation_deg > NORMAL_TOLERANCE_DEG {
                report.hard.push(Violation::EndpointNormal {
                    edge,
                    end,
                    deviation_deg,
                });
            }
        }
    }
    if !endpoint.setback.is_finite() || endpoint.setback < 0.0 {
        report.hard.push(Violation::InvalidGeometry {
            item: Item::Edge(edge),
        });
    } else if valid_rect(body.bounds) && body.contour.as_ref().is_none_or(BezPath::is_finite) {
        report.contacts_checked += 1;
        let contact = origin - tangent * endpoint.setback;
        let gap = contour_gap(body, contact);
        if !gap.is_finite() || gap > CONTACT_TOLERANCE {
            report
                .hard
                .push(Violation::EndpointContour { edge, end, gap });
        }
    }
    if let Some(marker) = endpoint.marker {
        report.markers_checked += 1;
        if !marker.is_valid() {
            report.hard.push(Violation::InvalidGeometry {
                item: Item::Edge(edge),
            });
            return;
        }
        // Match Chromium's two getPointAtLength calls in the archived point-marker probe.
        let near = shaft.sample(end, 0.08) - origin;
        let axis = near.normalize();
        let sample = shaft.sample(end, marker.depth) - origin;
        let axial = sample.dot(axis);
        let lateral = sample.cross(axis).abs();
        let slope = marker.half_width / marker.depth;
        if !axis.is_finite() || axial <= 0.0 || lateral > axial * slope + 0.02 {
            let sample_angle_deg = (axis.is_finite() && sample != Vec2::ZERO)
                .then(|| lateral.atan2(axial).to_degrees());
            report.hard.push(Violation::MarkerAngle {
                edge,
                end,
                sample_angle_deg,
                half_opening_deg: slope.atan().to_degrees(),
            });
        }
    }
}

fn check_edge(
    scene: &Scene<'_>,
    ancestry: &[Vec<usize>],
    id: usize,
    report: &mut Report,
) -> Option<Shaft> {
    let edge = &scene.edges[id];
    let item = Item::Edge(id);
    let shaft = match Shaft::read(edge) {
        Ok(shaft) => shaft,
        Err(reason) => {
            report
                .hard
                .push(Violation::InvalidPath { edge: id, reason });
            return None;
        }
    };
    if !normalized_rect(shaft.bounds) {
        report.hard.push(Violation::InvalidGeometry { item });
    }
    let (from, to) = (edge.start.body, edge.end.body);
    if from >= scene.bodies.len() || to >= scene.bodies.len() {
        report.hard.push(Violation::InvalidOwner { item });
        return Some(shaft);
    }
    for (end, endpoint) in [(End::Start, edge.start), (End::End, edge.end)] {
        check_endpoint(
            id,
            end,
            endpoint,
            &scene.bodies[endpoint.body],
            &shaft,
            report,
        );
    }
    if let Some(group) = edge_group(edge, ancestry) {
        check_containment(scene, item, shaft.bounds, group, report);
    }

    for (body_id, body) in scene.bodies.iter().enumerate() {
        if body_id == from
            || body_id == to
            || ancestry[from].contains(&body_id)
            || ancestry[to].contains(&body_id)
        {
            continue;
        }
        if shaft.hits(body.bounds) {
            report.hard.push(if body.is_group {
                Violation::ForeignGroupHit {
                    edge: id,
                    group: body_id,
                }
            } else {
                Violation::ForeignNodeHit {
                    edge: id,
                    node: body_id,
                }
            });
        }
    }
    for (label_id, label) in scene.labels.iter().enumerate() {
        let own = match label.owner {
            LabelOwner::Edge(owner) => owner == id,
            // A group title occupies a routing obstacle even for an edge attached to the group.
            LabelOwner::Body(owner) => {
                (owner == from || owner == to) && !scene.bodies[owner].is_group
            }
        };
        if !own && shaft.hits(label.bounds) {
            report.hard.push(Violation::ForeignLabelHit {
                edge: id,
                label: label_id,
            });
        }
    }
    Some(shaft)
}

/// Run the section 6 geometric checks. An empty `hard` accepts the invariants. Compare crossing
/// pairs, fork/join offsets and per-edge return against the selected fixture's aesthetic targets.
pub(crate) fn validate(scene: &Scene<'_>) -> Report {
    let mut report = Report::default();
    let ancestry = check_bodies(scene, &mut report);
    check_labels(scene, &ancestry, &mut report);
    let shafts: Vec<_> = (0..scene.edges.len())
        .map(|id| check_edge(scene, &ancestry, id, &mut report))
        .collect();
    for (a, shaft) in shafts.iter().enumerate() {
        report.edges.push(shaft.as_ref().map(|s| s.metrics));
        if let Some(shaft) = shaft {
            for (b, other) in shafts.iter().enumerate().skip(a + 1) {
                if other.as_ref().is_some_and(|p| shaft.crosses(p)) {
                    report.crossings.push((a, b));
                }
            }
        }
    }
    report
}

#[cfg(test)]
mod gallery;

#[cfg(test)]
#[path = "validate/tests.rs"]
mod tests;
