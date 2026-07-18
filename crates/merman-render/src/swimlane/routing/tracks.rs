use super::super::config::EPSILON;
use super::super::working::WorkingEdge;
use crate::model::LayoutPoint;
use indexmap::{IndexMap, IndexSet};
use std::collections::{HashMap, HashSet};

const TRACK_SPACING: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy)]
struct SegmentRef {
    edge_index: usize,
    segment_index: usize,
    from: f64,
    to: f64,
}

#[derive(Debug, Default)]
struct Track {
    segments: Vec<SegmentRef>,
}

#[derive(Debug)]
struct Pipe {
    orientation: Orientation,
    coord: f64,
    tracks: Vec<Track>,
}

#[derive(Debug, Clone, Copy)]
struct RoutedSegment {
    edge_index: usize,
    segment_index: usize,
    orientation: Orientation,
    pipe_index: usize,
    track_index: usize,
    from: f64,
    to: f64,
}

#[derive(Debug, Clone, Copy)]
struct DestinationInfo {
    destination: f64,
    deviation: f64,
    delta: f64,
}

#[derive(Debug, Clone, Copy)]
struct RoutedLine {
    orientation: Orientation,
    coord: f64,
    from: f64,
    to: f64,
}

fn segments_overlap(left: SegmentRef, right: SegmentRef) -> bool {
    left.from < right.to && right.from < left.to
}

fn segment_ref(segment: RoutedSegment) -> SegmentRef {
    SegmentRef {
        edge_index: segment.edge_index,
        segment_index: segment.segment_index,
        from: segment.from,
        to: segment.to,
    }
}

fn point_on_line(line: RoutedLine, along: f64) -> LayoutPoint {
    match line.orientation {
        Orientation::Vertical => LayoutPoint {
            x: line.coord,
            y: along,
        },
        Orientation::Horizontal => LayoutPoint {
            x: along,
            y: line.coord,
        },
    }
}

fn shared_line_endpoint_coord(line: RoutedLine, next: RoutedLine) -> f64 {
    if (line.to - next.from).abs() < EPSILON || (line.to - next.to).abs() < EPSILON {
        line.to
    } else {
        line.from
    }
}

struct TrackAssignment<'a> {
    edges: &'a [WorkingEdge],
    pipes: Vec<Pipe>,
    segments: Vec<RoutedSegment>,
    by_edge: Vec<Vec<usize>>,
    destination_cache: HashMap<usize, DestinationInfo>,
}

impl<'a> TrackAssignment<'a> {
    fn new(edges: &'a [WorkingEdge]) -> Self {
        Self {
            edges,
            pipes: Vec::new(),
            segments: Vec::new(),
            by_edge: vec![Vec::new(); edges.len()],
            destination_cache: HashMap::new(),
        }
    }

    fn pipe_for(&mut self, orientation: Orientation, coord: f64) -> usize {
        if let Some(index) = self
            .pipes
            .iter()
            .position(|pipe| pipe.orientation == orientation && (pipe.coord - coord).abs() < 1.0)
        {
            return index;
        }
        self.pipes.push(Pipe {
            orientation,
            coord,
            tracks: vec![Track::default()],
        });
        self.pipes.len() - 1
    }

    fn add_edge(&mut self, edge_index: usize) {
        let points = &self.edges[edge_index].points;
        for (segment_index, pair) in points.windows(2).enumerate() {
            let dx = (pair[0].x - pair[1].x).abs();
            let dy = (pair[0].y - pair[1].y).abs();
            if dx <= EPSILON && dy <= EPSILON {
                continue;
            }
            let orientation = if dx <= EPSILON {
                Orientation::Vertical
            } else {
                Orientation::Horizontal
            };
            let coord = match orientation {
                Orientation::Vertical => pair[0].x,
                Orientation::Horizontal => pair[0].y,
            };
            let (from, to) = match orientation {
                Orientation::Vertical => (pair[0].y.min(pair[1].y), pair[0].y.max(pair[1].y)),
                Orientation::Horizontal => (pair[0].x.min(pair[1].x), pair[0].x.max(pair[1].x)),
            };
            let pipe_index = self.pipe_for(orientation, coord);
            let routed = RoutedSegment {
                edge_index,
                segment_index,
                orientation,
                pipe_index,
                track_index: 0,
                from,
                to,
            };
            let routed_index = self.segments.len();
            self.segments.push(routed);
            self.by_edge[edge_index].push(routed_index);
            self.pipes[pipe_index].tracks[0]
                .segments
                .push(segment_ref(routed));
        }
    }

    fn adjacent_segments(&self, segment_index: usize) -> Vec<usize> {
        let segment = self.segments[segment_index];
        let indices = &self.by_edge[segment.edge_index];
        let Some(position) = indices.iter().position(|index| *index == segment_index) else {
            return Vec::new();
        };
        let mut adjacent = Vec::with_capacity(2);
        if position > 0 {
            adjacent.push(indices[position - 1]);
        }
        if position + 1 < indices.len() {
            adjacent.push(indices[position + 1]);
        }
        adjacent
    }

    fn segments_cross(&self, left: usize, right: usize) -> bool {
        let left = self.segments[left];
        let right = self.segments[right];
        if left.orientation == right.orientation {
            return false;
        }
        let (horizontal, vertical) = if left.orientation == Orientation::Horizontal {
            (left, right)
        } else {
            (right, left)
        };
        let horizontal_coord = self.pipes[horizontal.pipe_index].coord;
        let vertical_coord = self.pipes[vertical.pipe_index].coord;
        vertical_coord > horizontal.from
            && vertical_coord < horizontal.to
            && horizontal_coord > vertical.from
            && horizontal_coord < vertical.to
    }

    fn segments_conflict(&self, left_index: usize, right_index: usize) -> bool {
        let left = self.segments[left_index];
        let right = self.segments[right_index];
        if left.track_index == right.track_index {
            return segments_overlap(segment_ref(left), segment_ref(right));
        }
        let left_adjacent = self.adjacent_segments(left_index);
        let right_adjacent = self.adjacent_segments(right_index);
        left_adjacent.iter().any(|left| {
            right_adjacent
                .iter()
                .any(|right| self.segments_cross(*left, *right))
        })
    }

    fn remove_from_track(&mut self, segment_index: usize) {
        let segment = self.segments[segment_index];
        self.pipes[segment.pipe_index].tracks[segment.track_index]
            .segments
            .retain(|entry| {
                entry.edge_index != segment.edge_index
                    || entry.segment_index != segment.segment_index
            });
    }

    fn move_segment(&mut self, segment_index: usize, track_index: usize) {
        self.remove_from_track(segment_index);
        self.segments[segment_index].track_index = track_index;
        let segment = self.segments[segment_index];
        self.pipes[segment.pipe_index].tracks[track_index]
            .segments
            .push(segment_ref(segment));
    }

    fn move_segment_chain(&mut self, segment_index: usize, track_index: usize) {
        let segment = self.segments[segment_index];
        let chain: Vec<usize> = self.by_edge[segment.edge_index]
            .iter()
            .copied()
            .filter(|index| self.segments[*index].pipe_index == segment.pipe_index)
            .collect();
        for index in chain {
            self.move_segment(index, track_index);
        }
    }

    fn create_track(&mut self, pipe_index: usize) -> usize {
        let index = self.pipes[pipe_index].tracks.len();
        self.pipes[pipe_index].tracks.push(Track::default());
        index
    }

    fn available_track(&self, segment_index: usize) -> Option<usize> {
        let segment = self.segments[segment_index];
        self.pipes[segment.pipe_index]
            .tracks
            .iter()
            .enumerate()
            .find(|(_, track)| {
                !track.segments.iter().any(|entry| {
                    (entry.edge_index != segment.edge_index
                        || entry.segment_index != segment.segment_index)
                        && segments_overlap(*entry, segment_ref(segment))
                })
            })
            .map(|(index, _)| index)
    }

    fn try_swap_tracks(&mut self, left_index: usize, right_index: usize) -> bool {
        let left = self.segments[left_index];
        let right = self.segments[right_index];
        let left_target = right.track_index;
        let right_target = left.track_index;
        let can_left_move = !self.pipes[left.pipe_index].tracks[left_target]
            .segments
            .iter()
            .any(|entry| {
                (entry.edge_index != right.edge_index || entry.segment_index != right.segment_index)
                    && segments_overlap(*entry, segment_ref(left))
            });
        let can_right_move = !self.pipes[right.pipe_index].tracks[right_target]
            .segments
            .iter()
            .any(|entry| {
                (entry.edge_index != left.edge_index || entry.segment_index != left.segment_index)
                    && segments_overlap(*entry, segment_ref(right))
            });
        if !can_left_move || !can_right_move {
            return false;
        }
        self.remove_from_track(left_index);
        self.remove_from_track(right_index);
        self.segments[left_index].track_index = left_target;
        self.segments[right_index].track_index = right_target;
        let left = self.segments[left_index];
        let right = self.segments[right_index];
        self.pipes[left.pipe_index].tracks[left.track_index]
            .segments
            .push(segment_ref(left));
        self.pipes[right.pipe_index].tracks[right.track_index]
            .segments
            .push(segment_ref(right));
        true
    }

    fn resolve_conflict(&mut self, left: usize, right: usize, move_chain: bool) {
        if self.try_swap_tracks(left, right) {
            return;
        }
        let pipe_index = self.segments[left].pipe_index;
        let track = self
            .available_track(right)
            .unwrap_or_else(|| self.create_track(pipe_index));
        if move_chain {
            self.move_segment_chain(right, track);
        } else {
            self.move_segment(right, track);
        }
    }

    fn resolve_handles(&mut self, handles: &[usize], move_chain: bool) -> usize {
        let mut conflicts = 0;
        for left in 0..handles.len() {
            for right in left + 1..handles.len() {
                let left_index = handles[left];
                let right_index = handles[right];
                if self.segments[left_index].pipe_index != self.segments[right_index].pipe_index {
                    continue;
                }
                if self.segments_conflict(left_index, right_index) {
                    conflicts += 1;
                    self.resolve_conflict(left_index, right_index, move_chain);
                }
            }
        }
        conflicts
    }

    fn destination_info(&mut self, edge_index: usize) -> DestinationInfo {
        if let Some(info) = self.destination_cache.get(&edge_index) {
            return *info;
        }
        let indices = &self.by_edge[edge_index];
        let info = if let Some(first_index) = indices.first() {
            let first = self.segments[*first_index];
            let base = self.pipes[first.pipe_index].coord;
            let mut destination = base;
            for segment_index in indices.iter().skip(1) {
                let segment = self.segments[*segment_index];
                if segment.orientation == Orientation::Horizontal {
                    destination = if (segment.from - base).abs() > (segment.to - base).abs() {
                        segment.from
                    } else {
                        segment.to
                    };
                    break;
                }
            }
            DestinationInfo {
                destination,
                deviation: (destination - base).abs(),
                delta: destination - base,
            }
        } else {
            DestinationInfo {
                destination: 0.0,
                deviation: 0.0,
                delta: 0.0,
            }
        };
        self.destination_cache.insert(edge_index, info);
        info
    }

    fn fix_source_handles(&mut self) -> usize {
        let mut groups: IndexMap<&str, Vec<usize>> = IndexMap::new();
        for (index, edge) in self.edges.iter().enumerate() {
            if !self.by_edge[index].is_empty() {
                groups.entry(&edge.from).or_default().push(index);
            }
        }
        let mut conflicts = 0;
        for mut group in groups.into_values() {
            group.sort_by(|left, right| {
                let left_info = self.destination_info(*left);
                let right_info = self.destination_info(*right);
                if (left_info.deviation - right_info.deviation).abs() > 1.0 {
                    return left_info.deviation.total_cmp(&right_info.deviation);
                }
                if (left_info.destination - right_info.destination).abs() > 1.0 {
                    return left_info.destination.total_cmp(&right_info.destination);
                }
                let distance = |edge_index: usize| {
                    let points = &self.edges[edge_index].points;
                    points
                        .first()
                        .zip(points.last())
                        .map_or(0.0, |(start, end)| {
                            (start.x - end.x).abs() + (start.y - end.y).abs()
                        })
                };
                let left_distance = distance(*left);
                let right_distance = distance(*right);
                if (left_distance - right_distance).abs() > 1.0 {
                    return right_distance.total_cmp(&left_distance);
                }
                let left_len = self.by_edge[*left].len();
                let right_len = self.by_edge[*right].len();
                if left_len != right_len {
                    return left_len.cmp(&right_len);
                }
                if left_len == 1 {
                    let left_segment = self.segments[self.by_edge[*left][0]];
                    let right_segment = self.segments[self.by_edge[*right][0]];
                    let left_span = left_segment.to - left_segment.from;
                    let right_span = right_segment.to - right_segment.from;
                    if (left_span - right_span).abs() > 1.0 {
                        return left_span.total_cmp(&right_span);
                    }
                }
                left.cmp(right)
            });
            let handles: Vec<usize> = group
                .into_iter()
                .filter_map(|edge| self.by_edge[edge].first().copied())
                .collect();
            conflicts += self.resolve_handles(&handles, true);
        }
        conflicts
    }

    fn fix_target_handles(&mut self) -> usize {
        let mut groups: IndexMap<&str, Vec<usize>> = IndexMap::new();
        for (index, edge) in self.edges.iter().enumerate() {
            if !self.by_edge[index].is_empty() {
                groups.entry(&edge.to).or_default().push(index);
            }
        }
        let mut conflicts = 0;
        for mut group in groups.into_values() {
            group.sort_by(|left, right| {
                let perpendicular_span = |edge_index: usize| {
                    let indices = &self.by_edge[edge_index];
                    if indices.len() < 2 {
                        0.0
                    } else {
                        let segment = self.segments[indices[indices.len() - 2]];
                        (segment.to - segment.from).abs()
                    }
                };
                perpendicular_span(*left)
                    .total_cmp(&perpendicular_span(*right))
                    .then_with(|| left.cmp(right))
            });
            let handles: Vec<usize> = group
                .into_iter()
                .filter_map(|edge| self.by_edge[edge].last().copied())
                .collect();
            conflicts += self.resolve_handles(&handles, true);
        }
        conflicts
    }

    fn fix_pipe_conflicts(&mut self) -> usize {
        let mut conflicts = 0;
        for pipe_index in 0..self.pipes.len() {
            let mut segments: Vec<usize> = self
                .segments
                .iter()
                .enumerate()
                .filter(|(_, segment)| segment.pipe_index == pipe_index)
                .map(|(index, _)| index)
                .collect();
            segments.sort_by_key(|index| {
                let segment = self.segments[*index];
                (segment.edge_index, segment.segment_index)
            });
            for left in 0..segments.len() {
                for right in left + 1..segments.len() {
                    let left_index = segments[left];
                    let right_index = segments[right];
                    if self.segments_conflict(left_index, right_index) {
                        conflicts += 1;
                        self.resolve_conflict(left_index, right_index, false);
                    }
                }
            }
        }
        conflicts
    }

    fn reduce_conflicts(&mut self) {
        for _ in 0..10 {
            let changed =
                self.fix_source_handles() + self.fix_target_handles() + self.fix_pipe_conflicts();
            if changed == 0 {
                break;
            }
        }
    }

    fn segment_coordinates(&mut self) -> HashMap<(usize, usize), f64> {
        let mut coordinates = HashMap::new();
        for pipe_index in 0..self.pipes.len() {
            let mut entries: Vec<(usize, SegmentRef)> = self.pipes[pipe_index]
                .tracks
                .iter()
                .enumerate()
                .flat_map(|(track, value)| {
                    value
                        .segments
                        .iter()
                        .copied()
                        .map(move |segment| (track, segment))
                })
                .collect();
            entries.sort_by(|left, right| left.1.from.total_cmp(&right.1.from));
            let mut clusters: Vec<Vec<(usize, SegmentRef)>> = Vec::new();
            for entry in entries {
                if let Some(cluster) = clusters.last_mut() {
                    let end = cluster
                        .iter()
                        .map(|(_, segment)| segment.to)
                        .fold(f64::NEG_INFINITY, f64::max);
                    if entry.1.from < end {
                        cluster.push(entry);
                        continue;
                    }
                }
                clusters.push(vec![entry]);
            }
            for cluster in clusters {
                // JavaScript Set preserves first-seen insertion order. That order is
                // observable below whenever scores tie, because Array.sort is stable.
                let used: IndexSet<usize> = cluster.iter().map(|(track, _)| *track).collect();
                let mut scores: HashMap<usize, f64> = HashMap::new();
                for (track, segment) in &cluster {
                    let delta = self.destination_info(segment.edge_index).delta;
                    *scores.entry(*track).or_default() += delta;
                }
                let mut left: Vec<usize> = used
                    .iter()
                    .copied()
                    .filter(|track| scores.get(track).copied().unwrap_or(0.0) < -1.0)
                    .collect();
                let mut right: Vec<usize> = used
                    .iter()
                    .copied()
                    .filter(|track| scores.get(track).copied().unwrap_or(0.0) > 1.0)
                    .collect();
                let mut neutral: Vec<usize> = used
                    .iter()
                    .copied()
                    .filter(|track| scores.get(track).copied().unwrap_or(0.0).abs() <= 1.0)
                    .collect();
                left.sort_by(|a, b| {
                    scores
                        .get(b)
                        .copied()
                        .unwrap_or(0.0)
                        .total_cmp(&scores.get(a).copied().unwrap_or(0.0))
                });
                right.sort_by(|a, b| {
                    scores
                        .get(a)
                        .copied()
                        .unwrap_or(0.0)
                        .total_cmp(&scores.get(b).copied().unwrap_or(0.0))
                });
                if neutral.is_empty() && !used.is_empty() {
                    let mut closest: Vec<usize> = used.iter().copied().collect();
                    closest.sort_by(|a, b| {
                        scores
                            .get(a)
                            .copied()
                            .unwrap_or(0.0)
                            .abs()
                            .total_cmp(&scores.get(b).copied().unwrap_or(0.0).abs())
                    });
                    let best = closest[0];
                    left.retain(|track| *track != best);
                    right.retain(|track| *track != best);
                    neutral.push(best);
                }
                let pipe_coord = self.pipes[pipe_index].coord;
                let mut assign = |track: usize, coord: f64| {
                    for (_, segment) in cluster.iter().filter(|(candidate, _)| *candidate == track)
                    {
                        coordinates.insert((segment.edge_index, segment.segment_index), coord);
                    }
                };
                for (index, track) in left.iter().enumerate() {
                    assign(*track, pipe_coord - (index + 1) as f64 * TRACK_SPACING);
                }
                for (index, track) in neutral.iter().enumerate() {
                    let coord = if index == 0 {
                        pipe_coord
                    } else {
                        let direction = if index % 2 == 1 { 1.0 } else { -1.0 };
                        let magnitude = index.div_ceil(2) as f64;
                        pipe_coord + direction * magnitude * TRACK_SPACING * 0.5
                    };
                    assign(*track, coord);
                }
                for (index, track) in right.iter().enumerate() {
                    assign(*track, pipe_coord + (index + 1) as f64 * TRACK_SPACING);
                }
            }
        }
        coordinates
    }

    fn rebuild_edges(&self, edges: &mut [WorkingEdge], coordinates: &HashMap<(usize, usize), f64>) {
        for (edge_index, edge) in edges.iter_mut().enumerate() {
            let indices = &self.by_edge[edge_index];
            if indices.is_empty() || edge.points.len() < 2 {
                continue;
            }
            let source_port = edge.points[0].clone();
            let target_port = edge.points.last().expect("target port").clone();
            let lines: Vec<RoutedLine> = indices
                .iter()
                .map(|index| {
                    let segment = self.segments[*index];
                    RoutedLine {
                        orientation: segment.orientation,
                        coord: coordinates
                            .get(&(segment.edge_index, segment.segment_index))
                            .copied()
                            .unwrap_or(self.pipes[segment.pipe_index].coord),
                        from: segment.from,
                        to: segment.to,
                    }
                })
                .collect();
            let mut points = vec![source_port];
            for (index, line) in lines.iter().copied().enumerate() {
                let previous = points.last().expect("route point");
                let previous_along = match line.orientation {
                    Orientation::Vertical => previous.y,
                    Orientation::Horizontal => previous.x,
                };
                let previous_coord = match line.orientation {
                    Orientation::Vertical => previous.x,
                    Orientation::Horizontal => previous.y,
                };
                if (previous_coord - line.coord).abs() > EPSILON {
                    points.push(point_on_line(line, previous_along));
                }
                let next = lines.get(index + 1).copied();
                match next {
                    Some(next) if next.orientation == line.orientation => {
                        if (line.coord - next.coord).abs() > EPSILON {
                            let junction = if line.orientation == Orientation::Vertical {
                                (previous_along + next.from) / 2.0
                            } else {
                                shared_line_endpoint_coord(line, next)
                            };
                            points.push(point_on_line(line, junction));
                            points.push(point_on_line(next, junction));
                        } else if index == 0 || index + 2 == lines.len() {
                            points
                                .push(point_on_line(line, shared_line_endpoint_coord(line, next)));
                        }
                    }
                    Some(next) => points.push(point_on_line(line, next.coord)),
                    None => {
                        let end_along = if (line.from - previous_along).abs()
                            < (line.to - previous_along).abs()
                        {
                            line.to
                        } else {
                            line.from
                        };
                        points.push(point_on_line(line, end_along));
                    }
                }
            }
            if points.last().is_none_or(|last| {
                (last.x - target_port.x).abs() > EPSILON || (last.y - target_port.y).abs() > EPSILON
            }) {
                points.push(target_port);
            }
            points.dedup_by(|left, right| {
                (left.x - right.x).abs() <= EPSILON && (left.y - right.y).abs() <= EPSILON
            });
            edge.points = points;
        }
    }
}

pub(super) fn assign_tracks(
    edges: &mut [WorkingEdge],
    routing_order: &[usize],
    centered_straight_edges: &HashSet<usize>,
) {
    let mut assignment = TrackAssignment::new(edges);
    for edge_index in routing_order.iter().copied() {
        if !centered_straight_edges.contains(&edge_index) {
            assignment.add_edge(edge_index);
        }
    }
    assignment.reduce_conflicts();
    let coordinates = assignment.segment_coordinates();

    // TrackAssignment borrows the pre-rebuild edge geometry. Rebuild through a
    // temporary vector so source/target ports stay stable during materialization.
    let mut rebuilt = edges.to_vec();
    assignment.rebuild_edges(&mut rebuilt, &coordinates);
    for (edge, next) in edges.iter_mut().zip(rebuilt) {
        edge.points = next.points;
    }
}
