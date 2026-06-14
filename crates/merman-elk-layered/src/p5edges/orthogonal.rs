//! Orthogonal hyperedge routing core.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/HyperEdgeSegment.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/HyperEdgeSegmentDependency.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/HyperEdgeCycleDetector.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/OrthogonalRoutingGenerator.java

use std::collections::{BTreeSet, VecDeque};

use crate::random::JavaRandom;

pub const TOLERANCE: f64 = 1e-3;
const CRITICAL_DEPENDENCY_WEIGHT: i32 = 1;
const CRITICAL_CONFLICTS_DETECTED: i32 = -1;
const CONFLICT_THRESHOLD_FACTOR: f64 = 0.5;
const CRITICAL_CONFLICT_THRESHOLD_FACTOR: f64 = 0.2;
const CONFLICT_PENALTY: i32 = 1;
const CROSSING_PENALTY: i32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyType {
    Regular,
    Critical,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HyperEdgeSegment {
    pub ports: Vec<usize>,
    pub mark: i32,
    pub routing_slot: i32,
    start_position: f64,
    end_position: f64,
    pub incoming_connection_coordinates: Vec<f64>,
    pub outgoing_connection_coordinates: Vec<f64>,
    pub outgoing_segment_dependencies: Vec<usize>,
    pub out_dep_weight: i32,
    pub critical_out_dep_weight: i32,
    pub incoming_segment_dependencies: Vec<usize>,
    pub in_dep_weight: i32,
    pub critical_in_dep_weight: i32,
    pub split_partner: Option<usize>,
    pub split_by: Option<usize>,
}

impl HyperEdgeSegment {
    pub fn new() -> Self {
        Self {
            ports: Vec::new(),
            mark: 0,
            routing_slot: 0,
            start_position: f64::NAN,
            end_position: f64::NAN,
            incoming_connection_coordinates: Vec::new(),
            outgoing_connection_coordinates: Vec::new(),
            outgoing_segment_dependencies: Vec::new(),
            out_dep_weight: 0,
            critical_out_dep_weight: 0,
            incoming_segment_dependencies: Vec::new(),
            in_dep_weight: 0,
            critical_in_dep_weight: 0,
            split_partner: None,
            split_by: None,
        }
    }

    pub fn with_incoming_outgoing(
        incoming: impl Into<Vec<f64>>,
        outgoing: impl Into<Vec<f64>>,
    ) -> Self {
        let mut segment = Self::new();
        segment.incoming_connection_coordinates = incoming.into();
        segment.outgoing_connection_coordinates = outgoing.into();
        sort_dedup_f64(&mut segment.incoming_connection_coordinates);
        sort_dedup_f64(&mut segment.outgoing_connection_coordinates);
        segment.recompute_extent();
        segment
    }

    pub fn start_coordinate(&self) -> f64 {
        self.start_position
    }

    pub fn end_coordinate(&self) -> f64 {
        self.end_position
    }

    pub fn length(&self) -> f64 {
        self.end_coordinate() - self.start_coordinate()
    }

    pub fn represents_hyperedge(&self) -> bool {
        self.incoming_connection_coordinates.len() + self.outgoing_connection_coordinates.len() > 2
    }

    pub fn is_dummy(&self) -> bool {
        self.split_partner.is_some() && self.split_by.is_none()
    }

    pub fn insert_incoming(&mut self, value: f64) {
        insert_sorted_unique(&mut self.incoming_connection_coordinates, value);
        self.recompute_extent();
    }

    pub fn insert_outgoing(&mut self, value: f64) {
        insert_sorted_unique(&mut self.outgoing_connection_coordinates, value);
        self.recompute_extent();
    }

    pub fn recompute_extent(&mut self) {
        self.start_position = f64::NAN;
        self.end_position = f64::NAN;
        self.recompute_extent_for(true);
        self.recompute_extent_for(false);
    }

    fn recompute_extent_for(&mut self, incoming: bool) {
        let positions = if incoming {
            &self.incoming_connection_coordinates
        } else {
            &self.outgoing_connection_coordinates
        };
        let Some(first) = positions.first().copied() else {
            return;
        };
        let last = positions.last().copied().unwrap_or(first);

        if self.start_position.is_nan() {
            self.start_position = first;
        } else {
            self.start_position = self.start_position.min(first);
        }

        if self.end_position.is_nan() {
            self.end_position = last;
        } else {
            self.end_position = self.end_position.max(last);
        }
    }
}

impl Default for HyperEdgeSegment {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyperEdgeSegmentDependency {
    pub dependency_type: DependencyType,
    pub source: Option<usize>,
    pub target: Option<usize>,
    pub weight: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HyperEdgeGraph {
    pub segments: Vec<HyperEdgeSegment>,
    pub dependencies: Vec<HyperEdgeSegmentDependency>,
}

impl HyperEdgeGraph {
    pub fn add_segment(&mut self, segment: HyperEdgeSegment) -> usize {
        let index = self.segments.len();
        self.segments.push(segment);
        index
    }

    pub fn add_regular_dependency(&mut self, source: usize, target: usize, weight: i32) -> usize {
        self.add_dependency(DependencyType::Regular, source, target, weight)
    }

    pub fn add_critical_dependency(&mut self, source: usize, target: usize) -> usize {
        self.add_dependency(
            DependencyType::Critical,
            source,
            target,
            CRITICAL_DEPENDENCY_WEIGHT,
        )
    }

    fn add_dependency(
        &mut self,
        dependency_type: DependencyType,
        source: usize,
        target: usize,
        weight: i32,
    ) -> usize {
        let index = self.dependencies.len();
        self.dependencies.push(HyperEdgeSegmentDependency {
            dependency_type,
            source: None,
            target: None,
            weight,
        });
        self.set_dependency_source(index, Some(source));
        self.set_dependency_target(index, Some(target));
        index
    }

    pub fn remove_dependency(&mut self, dependency: usize) {
        self.set_dependency_source(dependency, None);
        self.set_dependency_target(dependency, None);
    }

    pub fn reverse_dependency(&mut self, dependency: usize) {
        let source = self.dependencies[dependency].source;
        let target = self.dependencies[dependency].target;
        self.set_dependency_source(dependency, target);
        self.set_dependency_target(dependency, source);
    }

    fn set_dependency_source(&mut self, dependency: usize, new_source: Option<usize>) {
        if let Some(source) = self.dependencies[dependency].source {
            remove_value(
                &mut self.segments[source].outgoing_segment_dependencies,
                dependency,
            );
        }

        self.dependencies[dependency].source = new_source;

        if let Some(source) = new_source {
            self.segments[source]
                .outgoing_segment_dependencies
                .push(dependency);
        }
    }

    fn set_dependency_target(&mut self, dependency: usize, new_target: Option<usize>) {
        if let Some(target) = self.dependencies[dependency].target {
            remove_value(
                &mut self.segments[target].incoming_segment_dependencies,
                dependency,
            );
        }

        self.dependencies[dependency].target = new_target;

        if let Some(target) = new_target {
            self.segments[target]
                .incoming_segment_dependencies
                .push(dependency);
        }
    }

    pub fn split_segment_at(&mut self, segment: usize, split_position: f64) -> usize {
        let split_partner = self.segments.len();
        let mut partner = HyperEdgeSegment::new();
        partner.split_partner = Some(segment);
        partner.outgoing_connection_coordinates =
            std::mem::take(&mut self.segments[segment].outgoing_connection_coordinates);
        partner.incoming_connection_coordinates.push(split_position);
        self.segments[segment]
            .outgoing_connection_coordinates
            .push(split_position);
        self.segments[segment].split_partner = Some(split_partner);

        self.segments[segment].recompute_extent();
        partner.recompute_extent();
        self.segments.push(partner);

        let incoming = self.segments[segment].incoming_segment_dependencies.clone();
        let outgoing = self.segments[segment].outgoing_segment_dependencies.clone();

        for dependency in incoming.into_iter().chain(outgoing.into_iter()) {
            self.remove_dependency(dependency);
        }

        split_partner
    }

    pub fn create_dependency_if_necessary(
        &mut self,
        first: usize,
        second: usize,
        thresholds: OrthogonalRoutingThresholds,
    ) -> usize {
        let he1 = &self.segments[first];
        let he2 = &self.segments[second];
        if (he1.start_coordinate() - he1.end_coordinate()).abs() < TOLERANCE
            || (he2.start_coordinate() - he2.end_coordinate()).abs() < TOLERANCE
        {
            return 0;
        }

        let conflicts1 = count_conflicts(
            &he1.outgoing_connection_coordinates,
            &he2.incoming_connection_coordinates,
            thresholds,
        );
        let conflicts2 = count_conflicts(
            &he2.outgoing_connection_coordinates,
            &he1.incoming_connection_coordinates,
            thresholds,
        );
        let critical_conflicts_detected =
            conflicts1 == CRITICAL_CONFLICTS_DETECTED || conflicts2 == CRITICAL_CONFLICTS_DETECTED;
        let mut critical_dependency_count = 0usize;

        if critical_conflicts_detected {
            if conflicts1 == CRITICAL_CONFLICTS_DETECTED {
                self.add_critical_dependency(second, first);
                critical_dependency_count += 1;
            }

            if conflicts2 == CRITICAL_CONFLICTS_DETECTED {
                self.add_critical_dependency(first, second);
                critical_dependency_count += 1;
            }
        } else {
            let crossings1 = count_crossings(
                &he1.outgoing_connection_coordinates,
                he2.start_coordinate(),
                he2.end_coordinate(),
            ) + count_crossings(
                &he2.incoming_connection_coordinates,
                he1.start_coordinate(),
                he1.end_coordinate(),
            );
            let crossings2 = count_crossings(
                &he2.outgoing_connection_coordinates,
                he1.start_coordinate(),
                he1.end_coordinate(),
            ) + count_crossings(
                &he1.incoming_connection_coordinates,
                he2.start_coordinate(),
                he2.end_coordinate(),
            );

            let dep_value1 = CONFLICT_PENALTY * conflicts1 + CROSSING_PENALTY * crossings1;
            let dep_value2 = CONFLICT_PENALTY * conflicts2 + CROSSING_PENALTY * crossings2;

            if dep_value1 < dep_value2 {
                self.add_regular_dependency(first, second, dep_value2 - dep_value1);
            } else if dep_value1 > dep_value2 {
                self.add_regular_dependency(second, first, dep_value1 - dep_value2);
            } else if dep_value1 > 0 && dep_value2 > 0 {
                self.add_regular_dependency(first, second, 0);
                self.add_regular_dependency(second, first, 0);
            }
        }

        critical_dependency_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrthogonalRoutingThresholds {
    pub conflict_threshold: f64,
    pub critical_conflict_threshold: f64,
}

impl OrthogonalRoutingThresholds {
    pub fn from_edge_spacing_and_segments(
        edge_spacing: f64,
        segments: &[HyperEdgeSegment],
    ) -> Self {
        Self {
            conflict_threshold: CONFLICT_THRESHOLD_FACTOR * edge_spacing,
            critical_conflict_threshold: CRITICAL_CONFLICT_THRESHOLD_FACTOR
                * minimum_horizontal_segment_distance(segments),
        }
    }
}

pub fn count_crossings(positions: &[f64], start: f64, end: f64) -> i32 {
    let mut crossings = 0;
    for position in positions {
        if *position > end {
            break;
        } else if *position >= start {
            crossings += 1;
        }
    }
    crossings
}

pub fn minimum_horizontal_segment_distance(segments: &[HyperEdgeSegment]) -> f64 {
    let incoming = minimum_difference(
        segments
            .iter()
            .flat_map(|segment| segment.incoming_connection_coordinates.iter().copied()),
    );
    let outgoing = minimum_difference(
        segments
            .iter()
            .flat_map(|segment| segment.outgoing_connection_coordinates.iter().copied()),
    );
    incoming.min(outgoing)
}

pub fn detect_cycles(
    graph: &mut HyperEdgeGraph,
    critical_only: bool,
    random: &mut JavaRandom,
) -> Vec<usize> {
    let mut result = Vec::new();
    let mut sources = VecDeque::new();
    let mut sinks = VecDeque::new();

    initialize_cycle_detector(graph, &mut sources, &mut sinks, critical_only);
    compute_linear_ordering_marks(graph, &mut sources, &mut sinks, critical_only, random);

    for source in 0..graph.segments.len() {
        for dependency in graph.segments[source].outgoing_segment_dependencies.clone() {
            let dep = &graph.dependencies[dependency];
            if dep.source.is_none() || dep.target.is_none() {
                continue;
            }
            if (!critical_only || dep.dependency_type == DependencyType::Critical)
                && graph.segments[source].mark > graph.segments[dep.target.unwrap()].mark
            {
                result.push(dependency);
            }
        }
    }

    result
}

pub fn break_non_critical_cycles(graph: &mut HyperEdgeGraph, random: &mut JavaRandom) {
    let cycle_dependencies = detect_cycles(graph, false, random);

    for dependency in cycle_dependencies {
        if graph.dependencies[dependency].weight == 0 {
            graph.remove_dependency(dependency);
        } else {
            graph.reverse_dependency(dependency);
        }
    }
}

pub fn topological_numbering(graph: &mut HyperEdgeGraph) {
    let mut sources = VecDeque::new();
    let mut rightward_targets = VecDeque::new();

    for node in 0..graph.segments.len() {
        graph.segments[node].in_dep_weight = graph.segments[node]
            .incoming_segment_dependencies
            .iter()
            .filter(|dependency| graph.dependencies[**dependency].source.is_some())
            .count() as i32;
        graph.segments[node].out_dep_weight = graph.segments[node]
            .outgoing_segment_dependencies
            .iter()
            .filter(|dependency| graph.dependencies[**dependency].target.is_some())
            .count() as i32;

        if graph.segments[node].in_dep_weight == 0 {
            sources.push_back(node);
        }

        if graph.segments[node].out_dep_weight == 0
            && graph.segments[node]
                .incoming_connection_coordinates
                .is_empty()
        {
            rightward_targets.push_back(node);
        }
    }

    let mut max_rank = -1;

    while let Some(node) = sources.pop_front() {
        for dependency in graph.segments[node].outgoing_segment_dependencies.clone() {
            let Some(target) = graph.dependencies[dependency].target else {
                continue;
            };
            graph.segments[target].routing_slot = graph.segments[target]
                .routing_slot
                .max(graph.segments[node].routing_slot + 1);
            max_rank = max_rank.max(graph.segments[target].routing_slot);

            graph.segments[target].in_dep_weight -= 1;
            if graph.segments[target].in_dep_weight == 0 {
                sources.push_back(target);
            }
        }
    }

    if max_rank > -1 {
        for node in &rightward_targets {
            graph.segments[*node].routing_slot = max_rank;
        }

        while let Some(node) = rightward_targets.pop_front() {
            for dependency in graph.segments[node].incoming_segment_dependencies.clone() {
                let Some(source) = graph.dependencies[dependency].source else {
                    continue;
                };
                if !graph.segments[source]
                    .incoming_connection_coordinates
                    .is_empty()
                {
                    continue;
                }

                graph.segments[source].routing_slot = graph.segments[source]
                    .routing_slot
                    .min(graph.segments[node].routing_slot - 1);
                graph.segments[source].out_dep_weight -= 1;
                if graph.segments[source].out_dep_weight == 0 {
                    rightward_targets.push_back(source);
                }
            }
        }
    }
}

fn initialize_cycle_detector(
    graph: &mut HyperEdgeGraph,
    sources: &mut VecDeque<usize>,
    sinks: &mut VecDeque<usize>,
    critical_only: bool,
) {
    let mut next_mark = -1;
    for segment in 0..graph.segments.len() {
        graph.segments[segment].mark = next_mark;
        next_mark -= 1;

        let critical_in_weight = dependency_weight(
            graph,
            &graph.segments[segment].incoming_segment_dependencies,
            Some(DependencyType::Critical),
        );
        let critical_out_weight = dependency_weight(
            graph,
            &graph.segments[segment].outgoing_segment_dependencies,
            Some(DependencyType::Critical),
        );

        let mut in_weight = critical_in_weight;
        let mut out_weight = critical_out_weight;

        if !critical_only {
            in_weight = dependency_weight(
                graph,
                &graph.segments[segment].incoming_segment_dependencies,
                None,
            );
            out_weight = dependency_weight(
                graph,
                &graph.segments[segment].outgoing_segment_dependencies,
                None,
            );
        }

        graph.segments[segment].in_dep_weight = in_weight;
        graph.segments[segment].critical_in_dep_weight = critical_in_weight;
        graph.segments[segment].out_dep_weight = out_weight;
        graph.segments[segment].critical_out_dep_weight = critical_out_weight;

        if out_weight == 0 {
            sinks.push_back(segment);
        } else if in_weight == 0 {
            sources.push_back(segment);
        }
    }
}

fn compute_linear_ordering_marks(
    graph: &mut HyperEdgeGraph,
    sources: &mut VecDeque<usize>,
    sinks: &mut VecDeque<usize>,
    critical_only: bool,
    random: &mut JavaRandom,
) {
    let mut unprocessed = (0..graph.segments.len()).collect::<BTreeSet<_>>();
    let mut max_segments = Vec::new();
    let mark_base = graph.segments.len() as i32;
    let mut next_sink_mark = mark_base - 1;
    let mut next_source_mark = mark_base + 1;

    while !unprocessed.is_empty() {
        while let Some(sink) = sinks.pop_front() {
            unprocessed.remove(&sink);
            graph.segments[sink].mark = next_sink_mark;
            next_sink_mark -= 1;
            update_neighbors(graph, sink, sources, sinks, critical_only);
        }

        while let Some(source) = sources.pop_front() {
            unprocessed.remove(&source);
            graph.segments[source].mark = next_source_mark;
            next_source_mark += 1;
            update_neighbors(graph, source, sources, sinks, critical_only);
        }

        let mut max_outflow = i32::MIN;
        for segment in &unprocessed {
            if !critical_only
                && graph.segments[*segment].critical_out_dep_weight > 0
                && graph.segments[*segment].critical_in_dep_weight <= 0
            {
                max_segments.clear();
                max_segments.push(*segment);
                break;
            }

            let outflow =
                graph.segments[*segment].out_dep_weight - graph.segments[*segment].in_dep_weight;
            if outflow >= max_outflow {
                if outflow > max_outflow {
                    max_segments.clear();
                    max_outflow = outflow;
                }
                max_segments.push(*segment);
            }
        }

        if !max_segments.is_empty() {
            let random_index = random.next_int(max_segments.len()).unwrap_or(0);
            let max_node = max_segments[random_index];
            unprocessed.remove(&max_node);
            graph.segments[max_node].mark = next_source_mark;
            next_source_mark += 1;
            update_neighbors(graph, max_node, sources, sinks, critical_only);
            max_segments.clear();
        }
    }

    let shift_base = graph.segments.len() as i32 + 1;
    for node in &mut graph.segments {
        if node.mark < mark_base {
            node.mark += shift_base;
        }
    }
}

fn update_neighbors(
    graph: &mut HyperEdgeGraph,
    node: usize,
    sources: &mut VecDeque<usize>,
    sinks: &mut VecDeque<usize>,
    critical_only: bool,
) {
    for dependency in graph.segments[node].outgoing_segment_dependencies.clone() {
        let dep = graph.dependencies[dependency].clone();
        if dep.source.is_none() || dep.target.is_none() {
            continue;
        }
        if !critical_only || dep.dependency_type == DependencyType::Critical {
            let target = dep.target.unwrap();
            if graph.segments[target].mark < 0 && dep.weight > 0 {
                graph.segments[target].in_dep_weight -= dep.weight;
                if dep.dependency_type == DependencyType::Critical {
                    graph.segments[target].critical_in_dep_weight -= dep.weight;
                }

                if graph.segments[target].in_dep_weight <= 0
                    && graph.segments[target].out_dep_weight > 0
                {
                    sources.push_back(target);
                }
            }
        }
    }

    for dependency in graph.segments[node].incoming_segment_dependencies.clone() {
        let dep = graph.dependencies[dependency].clone();
        if dep.source.is_none() || dep.target.is_none() {
            continue;
        }
        if !critical_only || dep.dependency_type == DependencyType::Critical {
            let source = dep.source.unwrap();
            if graph.segments[source].mark < 0 && dep.weight > 0 {
                graph.segments[source].out_dep_weight -= dep.weight;
                if dep.dependency_type == DependencyType::Critical {
                    graph.segments[source].critical_out_dep_weight -= dep.weight;
                }

                if graph.segments[source].out_dep_weight <= 0
                    && graph.segments[source].in_dep_weight > 0
                {
                    sinks.push_back(source);
                }
            }
        }
    }
}

fn dependency_weight(
    graph: &HyperEdgeGraph,
    dependencies: &[usize],
    dependency_type: Option<DependencyType>,
) -> i32 {
    dependencies
        .iter()
        .filter_map(|dependency| graph.dependencies.get(*dependency))
        .filter(|dependency| dependency.source.is_some() && dependency.target.is_some())
        .filter(|dependency| {
            dependency_type
                .map(|kind| dependency.dependency_type == kind)
                .unwrap_or(true)
        })
        .map(|dependency| dependency.weight)
        .sum()
}

fn count_conflicts(
    positions1: &[f64],
    positions2: &[f64],
    thresholds: OrthogonalRoutingThresholds,
) -> i32 {
    let mut conflicts = 0;

    if !positions1.is_empty() && !positions2.is_empty() {
        let mut index1 = 0usize;
        let mut index2 = 0usize;
        let mut pos1 = positions1[index1];
        let mut pos2 = positions2[index2];
        let mut has_more = true;

        while has_more {
            if pos1 > pos2 - thresholds.critical_conflict_threshold
                && pos1 < pos2 + thresholds.critical_conflict_threshold
            {
                return CRITICAL_CONFLICTS_DETECTED;
            } else if pos1 > pos2 - thresholds.conflict_threshold
                && pos1 < pos2 + thresholds.conflict_threshold
            {
                conflicts += 1;
            }

            if pos1 <= pos2 && index1 + 1 < positions1.len() {
                index1 += 1;
                pos1 = positions1[index1];
            } else if pos2 <= pos1 && index2 + 1 < positions2.len() {
                index2 += 1;
                pos2 = positions2[index2];
            } else {
                has_more = false;
            }
        }
    }

    conflicts
}

fn minimum_difference(numbers: impl Iterator<Item = f64>) -> f64 {
    let mut numbers = numbers.collect::<Vec<_>>();
    sort_dedup_f64(&mut numbers);

    let mut min_difference = f64::MAX;
    if numbers.len() >= 2 {
        let mut current = numbers[0];
        for next in numbers.into_iter().skip(1) {
            min_difference = min_difference.min(next - current);
            current = next;
        }
    }
    min_difference
}

fn insert_sorted_unique(list: &mut Vec<f64>, value: f64) {
    match list.binary_search_by(|probe| probe.total_cmp(&value)) {
        Ok(_) => {}
        Err(index) => list.insert(index, value),
    }
}

fn sort_dedup_f64(values: &mut Vec<f64>) {
    values.sort_by(|left, right| left.total_cmp(right));
    values.dedup_by(|left, right| left == right);
}

fn remove_value(values: &mut Vec<usize>, value: usize) {
    if let Some(index) = values.iter().position(|candidate| *candidate == value) {
        values.remove(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(incoming: &[f64], outgoing: &[f64]) -> HyperEdgeSegment {
        HyperEdgeSegment::with_incoming_outgoing(incoming.to_vec(), outgoing.to_vec())
    }

    #[test]
    fn segment_extent_tracks_sorted_unique_connection_coordinates() {
        let mut segment = HyperEdgeSegment::new();

        segment.insert_incoming(30.0);
        segment.insert_incoming(10.0);
        segment.insert_incoming(10.0);
        segment.insert_outgoing(20.0);

        assert_eq!(segment.incoming_connection_coordinates, vec![10.0, 30.0]);
        assert_eq!(segment.outgoing_connection_coordinates, vec![20.0]);
        assert_eq!(segment.start_coordinate(), 10.0);
        assert_eq!(segment.end_coordinate(), 30.0);
        assert_eq!(segment.length(), 20.0);
    }

    #[test]
    fn dependency_reverse_and_remove_keep_segment_adjacency_lists_in_sync() {
        let mut graph = HyperEdgeGraph::default();
        let a = graph.add_segment(segment(&[0.0], &[30.0]));
        let b = graph.add_segment(segment(&[10.0], &[40.0]));
        let dependency = graph.add_regular_dependency(a, b, 7);

        assert_eq!(
            graph.segments[a].outgoing_segment_dependencies,
            vec![dependency]
        );
        assert_eq!(
            graph.segments[b].incoming_segment_dependencies,
            vec![dependency]
        );

        graph.reverse_dependency(dependency);

        assert_eq!(graph.dependencies[dependency].source, Some(b));
        assert_eq!(graph.dependencies[dependency].target, Some(a));
        assert!(graph.segments[a].outgoing_segment_dependencies.is_empty());
        assert_eq!(
            graph.segments[a].incoming_segment_dependencies,
            vec![dependency]
        );
        assert_eq!(
            graph.segments[b].outgoing_segment_dependencies,
            vec![dependency]
        );
        assert!(graph.segments[b].incoming_segment_dependencies.is_empty());

        graph.remove_dependency(dependency);

        assert!(graph.dependencies[dependency].source.is_none());
        assert!(graph.dependencies[dependency].target.is_none());
        assert!(graph.segments[a].incoming_segment_dependencies.is_empty());
        assert!(graph.segments[b].outgoing_segment_dependencies.is_empty());
    }

    #[test]
    fn create_dependency_if_necessary_adds_critical_dependency_for_overlaps() {
        let mut graph = HyperEdgeGraph::default();
        let a = graph.add_segment(segment(&[0.0], &[20.0]));
        let b = graph.add_segment(segment(&[20.4], &[40.0]));
        let thresholds = OrthogonalRoutingThresholds {
            conflict_threshold: 5.0,
            critical_conflict_threshold: 1.0,
        };

        let critical_count = graph.create_dependency_if_necessary(a, b, thresholds);

        assert_eq!(critical_count, 1);
        assert_eq!(graph.dependencies.len(), 1);
        assert_eq!(
            graph.dependencies[0].dependency_type,
            DependencyType::Critical
        );
        assert_eq!(graph.dependencies[0].source, Some(b));
        assert_eq!(graph.dependencies[0].target, Some(a));
    }

    #[test]
    fn break_non_critical_cycles_reverses_positive_weight_back_edges() {
        let mut graph = HyperEdgeGraph::default();
        let a = graph.add_segment(segment(&[0.0], &[30.0]));
        let b = graph.add_segment(segment(&[10.0], &[40.0]));
        let c = graph.add_segment(segment(&[20.0], &[50.0]));
        graph.add_regular_dependency(a, b, 2);
        graph.add_regular_dependency(b, c, 2);
        let back_edge = graph.add_regular_dependency(c, a, 1);
        let mut random = JavaRandom::new(1);

        break_non_critical_cycles(&mut graph, &mut random);

        assert_eq!(graph.dependencies[back_edge].source, Some(a));
        assert_eq!(graph.dependencies[back_edge].target, Some(c));
    }

    #[test]
    fn topological_numbering_assigns_increasing_slots() {
        let mut graph = HyperEdgeGraph::default();
        let a = graph.add_segment(segment(&[0.0], &[30.0]));
        let b = graph.add_segment(segment(&[10.0], &[40.0]));
        let c = graph.add_segment(segment(&[20.0], &[50.0]));
        graph.add_regular_dependency(a, b, 1);
        graph.add_regular_dependency(b, c, 1);

        topological_numbering(&mut graph);

        assert_eq!(graph.segments[a].routing_slot, 0);
        assert_eq!(graph.segments[b].routing_slot, 1);
        assert_eq!(graph.segments[c].routing_slot, 2);
    }

    #[test]
    fn split_segment_at_moves_outgoing_coordinates_and_clears_dependencies() {
        let mut graph = HyperEdgeGraph::default();
        let a = graph.add_segment(segment(&[0.0, 10.0], &[30.0, 40.0]));
        let b = graph.add_segment(segment(&[20.0], &[50.0]));
        graph.add_regular_dependency(a, b, 1);

        let partner = graph.split_segment_at(a, 25.0);

        assert_eq!(
            graph.segments[a].outgoing_connection_coordinates,
            vec![25.0]
        );
        assert_eq!(
            graph.segments[partner].incoming_connection_coordinates,
            vec![25.0]
        );
        assert_eq!(
            graph.segments[partner].outgoing_connection_coordinates,
            vec![30.0, 40.0]
        );
        assert_eq!(graph.segments[a].split_partner, Some(partner));
        assert_eq!(graph.segments[partner].split_partner, Some(a));
        assert!(graph.segments[a].outgoing_segment_dependencies.is_empty());
        assert!(graph.segments[b].incoming_segment_dependencies.is_empty());
    }
}
