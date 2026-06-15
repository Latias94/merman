//! Orthogonal hyperedge routing core.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/HyperEdgeSegment.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/HyperEdgeSegmentDependency.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/HyperEdgeCycleDetector.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/HyperEdgeSegmentSplitter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/OrthogonalRoutingGenerator.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/direction/BaseRoutingDirectionStrategy.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/direction/WestToEastRoutingStrategy.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/direction/NorthToSouthRoutingStrategy.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal/direction/SouthToNorthRoutingStrategy.java

use std::collections::{BTreeSet, HashMap, VecDeque};

use crate::graph::{LGraph, LPoint, PortRef, PortSide};
use crate::random::JavaRandom;

pub const TOLERANCE: f64 = 1e-3;
const CRITICAL_DEPENDENCY_WEIGHT: i32 = 1;
const CRITICAL_CONFLICTS_DETECTED: i32 = -1;
const CONFLICT_THRESHOLD_FACTOR: f64 = 0.5;
const CRITICAL_CONFLICT_THRESHOLD_FACTOR: f64 = 0.2;
const CONFLICT_PENALTY: i32 = 1;
const CROSSING_PENALTY: i32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingDirection {
    WestToEast,
    NorthToSouth,
    SouthToNorth,
}

impl RoutingDirection {
    fn source_port_side(self) -> PortSide {
        match self {
            Self::WestToEast => PortSide::East,
            Self::NorthToSouth => PortSide::South,
            Self::SouthToNorth => PortSide::North,
        }
    }

    fn target_port_side(self) -> PortSide {
        match self {
            Self::WestToEast => PortSide::West,
            Self::NorthToSouth => PortSide::North,
            Self::SouthToNorth => PortSide::South,
        }
    }

    fn port_position_on_hypernode(self, graph: &LGraph, port_ref: PortRef) -> f64 {
        let anchor = absolute_anchor(graph, port_ref);
        match self {
            Self::WestToEast => anchor.y,
            Self::NorthToSouth | Self::SouthToNorth => anchor.x,
        }
    }

    fn routing_slot_position(self, start_pos: f64, routing_slot: i32, edge_spacing: f64) -> f64 {
        match self {
            Self::WestToEast | Self::NorthToSouth => {
                start_pos + f64::from(routing_slot) * edge_spacing
            }
            Self::SouthToNorth => start_pos - f64::from(routing_slot) * edge_spacing,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyType {
    Regular,
    Critical,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HyperEdgeSegment {
    pub ports: Vec<PortRef>,
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

    pub fn simulate_split(&self) -> (Self, Self) {
        let mut split_segment = Self::new();
        split_segment.incoming_connection_coordinates =
            self.incoming_connection_coordinates.clone();
        split_segment.split_by = self.split_by;
        split_segment.split_partner = Some(1);
        split_segment.recompute_extent();

        let mut split_partner = Self::new();
        split_partner.outgoing_connection_coordinates =
            self.outgoing_connection_coordinates.clone();
        split_partner.split_partner = Some(0);
        split_partner.recompute_extent();

        (split_segment, split_partner)
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct FreeArea {
    start_position: f64,
    end_position: f64,
    size: f64,
}

impl FreeArea {
    fn new(start_position: f64, end_position: f64) -> Self {
        debug_assert!(end_position >= start_position);
        Self {
            start_position,
            end_position,
            size: end_position - start_position,
        }
    }

    fn center(&self) -> f64 {
        center(self.start_position, self.end_position)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AreaRating {
    dependencies: i32,
    crossings: i32,
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

pub fn route_edges_west_to_east(
    graph: &mut LGraph,
    source_layer_nodes: Option<&[usize]>,
    target_layer_nodes: Option<&[usize]>,
    start_pos: f64,
    edge_spacing: f64,
) -> usize {
    route_edges(
        graph,
        RoutingDirection::WestToEast,
        source_layer_nodes,
        target_layer_nodes,
        start_pos,
        edge_spacing,
    )
}

pub fn route_edges(
    graph: &mut LGraph,
    direction: RoutingDirection,
    source_layer_nodes: Option<&[usize]>,
    target_layer_nodes: Option<&[usize]>,
    start_pos: f64,
    edge_spacing: f64,
) -> usize {
    let mut hyper_graph = HyperEdgeGraph::default();
    let mut port_to_segment = HashMap::new();

    create_hyperedge_segments(
        graph,
        direction,
        source_layer_nodes,
        direction.source_port_side(),
        &mut hyper_graph,
        &mut port_to_segment,
    );
    create_hyperedge_segments(
        graph,
        direction,
        target_layer_nodes,
        direction.target_port_side(),
        &mut hyper_graph,
        &mut port_to_segment,
    );

    let thresholds = OrthogonalRoutingThresholds::from_edge_spacing_and_segments(
        edge_spacing,
        &hyper_graph.segments,
    );
    let mut critical_dependency_count = 0usize;

    for first in 0..hyper_graph.segments.len().saturating_sub(1) {
        for second in (first + 1)..hyper_graph.segments.len() {
            critical_dependency_count +=
                hyper_graph.create_dependency_if_necessary(first, second, thresholds);
        }
    }

    if critical_dependency_count >= 2 {
        break_critical_cycles(&mut hyper_graph, thresholds, &mut graph.random);
    }

    break_non_critical_cycles(&mut hyper_graph, &mut graph.random);
    topological_numbering(&mut hyper_graph);

    let mut rank_count = -1;
    for segment in 0..hyper_graph.segments.len() {
        if (hyper_graph.segments[segment].start_coordinate()
            - hyper_graph.segments[segment].end_coordinate())
        .abs()
            < TOLERANCE
        {
            continue;
        }

        rank_count = rank_count.max(hyper_graph.segments[segment].routing_slot);
        calculate_bend_points(
            graph,
            direction,
            &hyper_graph,
            segment,
            start_pos,
            edge_spacing,
        );
    }

    (rank_count + 1) as usize
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

fn create_hyperedge_segments(
    graph: &LGraph,
    direction: RoutingDirection,
    nodes: Option<&[usize]>,
    port_side: PortSide,
    hyper_graph: &mut HyperEdgeGraph,
    port_to_segment: &mut HashMap<PortRef, usize>,
) {
    let Some(nodes) = nodes else {
        return;
    };

    for node in nodes {
        let Some(lnode) = graph.layerless_nodes.get(*node) else {
            continue;
        };
        for port_index in 0..lnode.ports.len() {
            let port = &lnode.ports[port_index];
            let port_ref = PortRef {
                node: *node,
                port: port_index,
            };
            if !graph.port_has_output_type(port_ref) || port.side != port_side {
                continue;
            }

            if !port_to_segment.contains_key(&port_ref) {
                let segment = hyper_graph.add_segment(HyperEdgeSegment::new());
                add_port_positions(
                    graph,
                    direction,
                    port_ref,
                    hyper_graph,
                    segment,
                    port_to_segment,
                );
            }
        }
    }
}

fn add_port_positions(
    graph: &LGraph,
    direction: RoutingDirection,
    port_ref: PortRef,
    hyper_graph: &mut HyperEdgeGraph,
    segment: usize,
    port_to_segment: &mut HashMap<PortRef, usize>,
) {
    port_to_segment.insert(port_ref, segment);
    hyper_graph.segments[segment].ports.push(port_ref);
    let port_position = direction.port_position_on_hypernode(graph, port_ref);
    let port = &graph.layerless_nodes[port_ref.node].ports[port_ref.port];

    if port.side == direction.source_port_side() {
        hyper_graph.segments[segment].insert_incoming(port_position);
    } else {
        hyper_graph.segments[segment].insert_outgoing(port_position);
    }

    for other_port in connected_ports(graph, port_ref) {
        if !port_to_segment.contains_key(&other_port) {
            add_port_positions(
                graph,
                direction,
                other_port,
                hyper_graph,
                segment,
                port_to_segment,
            );
        }
    }
}

fn calculate_bend_points(
    graph: &mut LGraph,
    direction: RoutingDirection,
    hyper_graph: &HyperEdgeGraph,
    segment_index: usize,
    start_pos: f64,
    edge_spacing: f64,
) {
    let segment = &hyper_graph.segments[segment_index];
    if segment.is_dummy() {
        return;
    }

    let segment_position =
        direction.routing_slot_position(start_pos, segment.routing_slot, edge_spacing);

    for port_ref in &segment.ports {
        let source_anchor = absolute_anchor(graph, *port_ref);
        let outgoing_edges = graph.layerless_nodes[port_ref.node].ports[port_ref.port]
            .outgoing_edges
            .clone();

        for edge_index in outgoing_edges {
            if is_self_loop(graph, edge_index) {
                continue;
            }

            let target = graph.edges[edge_index].target;
            let target_anchor = absolute_anchor(graph, target);

            if bend_point_axis_delta(direction, source_anchor, target_anchor).abs() <= TOLERANCE {
                continue;
            }

            let mut current_position = segment_position;

            add_bend_point_if_needed(
                graph,
                edge_index,
                source_bend_point(direction, source_anchor, current_position),
            );

            if let Some(split_partner) = segment.split_partner {
                let split_position = hyper_graph.segments[split_partner]
                    .incoming_connection_coordinates
                    .first()
                    .copied()
                    .unwrap_or_else(|| direction.port_position_on_hypernode(graph, *port_ref));

                add_bend_point_if_needed(
                    graph,
                    edge_index,
                    split_bend_point(direction, split_position, current_position),
                );

                current_position = direction.routing_slot_position(
                    start_pos,
                    hyper_graph.segments[split_partner].routing_slot,
                    edge_spacing,
                );

                add_bend_point_if_needed(
                    graph,
                    edge_index,
                    split_bend_point(direction, split_position, current_position),
                );
            }

            add_bend_point_if_needed(
                graph,
                edge_index,
                target_bend_point(direction, target_anchor, current_position),
            );
        }
    }
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

pub fn break_critical_cycles(
    graph: &mut HyperEdgeGraph,
    thresholds: OrthogonalRoutingThresholds,
    random: &mut JavaRandom,
) {
    let cycle_dependencies = detect_cycles(graph, true, random);
    split_segments(graph, &cycle_dependencies, thresholds);
}

pub fn split_segments(
    graph: &mut HyperEdgeGraph,
    dependencies_to_resolve: &[usize],
    thresholds: OrthogonalRoutingThresholds,
) {
    if dependencies_to_resolve.is_empty() {
        return;
    }

    let mut free_areas = find_free_areas(&graph.segments, thresholds.critical_conflict_threshold);
    let segments_to_split = decide_which_segments_to_split(graph, dependencies_to_resolve);

    let mut ordered_segments = segments_to_split;
    ordered_segments.sort_by(|left, right| {
        graph.segments[*left]
            .length()
            .total_cmp(&graph.segments[*right].length())
    });

    for segment in ordered_segments {
        split_segment(graph, segment, &mut free_areas, thresholds);
    }
}

fn find_free_areas(
    segments: &[HyperEdgeSegment],
    critical_conflict_threshold: f64,
) -> Vec<FreeArea> {
    let mut sorted_coordinates = segments
        .iter()
        .flat_map(|segment| {
            segment
                .incoming_connection_coordinates
                .iter()
                .chain(segment.outgoing_connection_coordinates.iter())
                .copied()
        })
        .collect::<Vec<_>>();
    sorted_coordinates.sort_by(|left, right| left.total_cmp(right));

    let mut free_areas = Vec::new();
    for window in sorted_coordinates.windows(2) {
        let start = window[0];
        let end = window[1];
        if end - start >= 2.0 * critical_conflict_threshold {
            free_areas.push(FreeArea::new(
                start + critical_conflict_threshold,
                end - critical_conflict_threshold,
            ));
        }
    }

    free_areas
}

fn decide_which_segments_to_split(
    graph: &mut HyperEdgeGraph,
    dependencies: &[usize],
) -> Vec<usize> {
    let mut segments_to_split = Vec::new();

    for dependency in dependencies {
        let Some(source_segment) = graph.dependencies[*dependency].source else {
            continue;
        };
        let Some(target_segment) = graph.dependencies[*dependency].target else {
            continue;
        };

        if segments_to_split.contains(&source_segment)
            || segments_to_split.contains(&target_segment)
        {
            continue;
        }

        let mut segment_to_split = source_segment;
        let mut segment_causing_split = target_segment;

        if graph.segments[source_segment].represents_hyperedge()
            && !graph.segments[target_segment].represents_hyperedge()
        {
            segment_to_split = target_segment;
            segment_causing_split = source_segment;
        }

        segments_to_split.push(segment_to_split);
        graph.segments[segment_to_split].split_by = Some(segment_causing_split);
    }

    segments_to_split
}

fn split_segment(
    graph: &mut HyperEdgeGraph,
    segment: usize,
    free_areas: &mut Vec<FreeArea>,
    thresholds: OrthogonalRoutingThresholds,
) {
    let split_position = compute_position_to_split_and_update_free_areas(
        graph,
        &graph.segments[segment],
        free_areas,
        thresholds.critical_conflict_threshold,
    );
    let split_partner = graph.split_segment_at(segment, split_position);
    update_dependencies_after_split(graph, segment, split_partner, thresholds);
}

fn update_dependencies_after_split(
    graph: &mut HyperEdgeGraph,
    segment: usize,
    split_partner: usize,
    thresholds: OrthogonalRoutingThresholds,
) {
    let Some(split_causing_segment) = graph.segments[segment].split_by else {
        return;
    };

    graph.add_critical_dependency(segment, split_causing_segment);
    graph.add_critical_dependency(split_causing_segment, split_partner);

    let current_segment_count = graph.segments.len();
    for other_segment in 0..current_segment_count {
        if other_segment == split_causing_segment
            || other_segment == segment
            || other_segment == split_partner
        {
            continue;
        }

        graph.create_dependency_if_necessary(other_segment, segment, thresholds);
        graph.create_dependency_if_necessary(other_segment, split_partner, thresholds);
    }
}

fn compute_position_to_split_and_update_free_areas(
    graph: &HyperEdgeGraph,
    segment: &HyperEdgeSegment,
    free_areas: &mut Vec<FreeArea>,
    critical_conflict_threshold: f64,
) -> f64 {
    let mut first_possible_area_index = None;
    let mut last_possible_area_index = None;

    for (index, area) in free_areas.iter().enumerate() {
        if area.start_position > segment.end_coordinate() {
            break;
        } else if area.end_position >= segment.start_coordinate() {
            first_possible_area_index.get_or_insert(index);
            last_possible_area_index = Some(index);
        }
    }

    let Some(first_index) = first_possible_area_index else {
        return center(segment.start_coordinate(), segment.end_coordinate());
    };
    let last_index = last_possible_area_index.unwrap_or(first_index);
    let best_area_index =
        choose_best_area_index(graph, segment, free_areas, first_index, last_index);
    let split_position = free_areas[best_area_index].center();
    use_area(free_areas, best_area_index, critical_conflict_threshold);
    split_position
}

fn choose_best_area_index(
    graph: &HyperEdgeGraph,
    segment: &HyperEdgeSegment,
    free_areas: &[FreeArea],
    from_index: usize,
    to_index: usize,
) -> usize {
    let mut best_area_index = from_index;

    if from_index < to_index {
        let (mut split_segment, mut split_partner) = segment.simulate_split();
        let mut best_area = free_areas[best_area_index];
        let mut best_rating = rate_area(
            graph,
            segment,
            &mut split_segment,
            &mut split_partner,
            best_area,
        );

        for (index, area) in free_areas
            .iter()
            .enumerate()
            .take(to_index + 1)
            .skip(from_index + 1)
        {
            let current_rating = rate_area(
                graph,
                segment,
                &mut split_segment,
                &mut split_partner,
                *area,
            );
            if is_better_area(*area, current_rating, best_area, best_rating) {
                best_area = *area;
                best_rating = current_rating;
                best_area_index = index;
            }
        }
    }

    best_area_index
}

fn rate_area(
    graph: &HyperEdgeGraph,
    segment: &HyperEdgeSegment,
    split_segment: &mut HyperEdgeSegment,
    split_partner: &mut HyperEdgeSegment,
    area: FreeArea,
) -> AreaRating {
    let area_center = area.center();

    split_segment.outgoing_connection_coordinates.clear();
    split_segment
        .outgoing_connection_coordinates
        .push(area_center);
    split_segment.recompute_extent();

    split_partner.incoming_connection_coordinates.clear();
    split_partner
        .incoming_connection_coordinates
        .push(area_center);
    split_partner.recompute_extent();

    let mut rating = AreaRating::default();

    for dependency in &segment.incoming_segment_dependencies {
        let Some(other_segment) = graph.dependencies[*dependency].source else {
            continue;
        };
        let other_segment = &graph.segments[other_segment];
        update_considering_both_orderings(&mut rating, split_segment, other_segment);
        update_considering_both_orderings(&mut rating, split_partner, other_segment);
    }

    for dependency in &segment.outgoing_segment_dependencies {
        let Some(other_segment) = graph.dependencies[*dependency].target else {
            continue;
        };
        let other_segment = &graph.segments[other_segment];
        update_considering_both_orderings(&mut rating, split_segment, other_segment);
        update_considering_both_orderings(&mut rating, split_partner, other_segment);
    }

    rating.dependencies += 2;
    if let Some(split_by) = segment.split_by {
        let split_by_segment = &graph.segments[split_by];
        rating.crossings += count_crossings_for_single_ordering(split_segment, split_by_segment);
        rating.crossings += count_crossings_for_single_ordering(split_by_segment, split_partner);
    }

    rating
}

fn update_considering_both_orderings(
    rating: &mut AreaRating,
    left_candidate: &HyperEdgeSegment,
    right_candidate: &HyperEdgeSegment,
) {
    let crossings_left_right = count_crossings_for_single_ordering(left_candidate, right_candidate);
    let crossings_right_left = count_crossings_for_single_ordering(right_candidate, left_candidate);

    if crossings_left_right == crossings_right_left {
        if crossings_left_right > 0 {
            rating.dependencies += 2;
            rating.crossings += crossings_left_right;
        }
    } else {
        rating.dependencies += 1;
        rating.crossings += crossings_left_right.min(crossings_right_left);
    }
}

fn count_crossings_for_single_ordering(left: &HyperEdgeSegment, right: &HyperEdgeSegment) -> i32 {
    count_crossings(
        &left.outgoing_connection_coordinates,
        right.start_coordinate(),
        right.end_coordinate(),
    ) + count_crossings(
        &right.incoming_connection_coordinates,
        left.start_coordinate(),
        left.end_coordinate(),
    )
}

fn is_better_area(
    current_area: FreeArea,
    current_rating: AreaRating,
    best_area: FreeArea,
    best_rating: AreaRating,
) -> bool {
    if current_rating.crossings < best_rating.crossings {
        return true;
    }

    current_rating.crossings == best_rating.crossings
        && (current_rating.dependencies < best_rating.dependencies
            || (current_rating.dependencies == best_rating.dependencies
                && current_area.size > best_area.size))
}

fn use_area(
    free_areas: &mut Vec<FreeArea>,
    used_area_index: usize,
    critical_conflict_threshold: f64,
) {
    let old_area = free_areas.remove(used_area_index);

    if old_area.size / 2.0 >= critical_conflict_threshold {
        let mut insert_index = used_area_index;
        let old_area_center = old_area.center();

        let new_end_1 = old_area_center - critical_conflict_threshold;
        if old_area.start_position <= new_end_1 {
            free_areas.insert(
                insert_index,
                FreeArea::new(old_area.start_position, new_end_1),
            );
            insert_index += 1;
        }

        let new_start_2 = old_area_center + critical_conflict_threshold;
        if new_start_2 <= old_area.end_position {
            free_areas.insert(
                insert_index,
                FreeArea::new(new_start_2, old_area.end_position),
            );
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

fn connected_ports(graph: &LGraph, port_ref: PortRef) -> Vec<PortRef> {
    let Some(node) = graph.layerless_nodes.get(port_ref.node) else {
        return Vec::new();
    };
    let Some(port) = node.ports.get(port_ref.port) else {
        return Vec::new();
    };

    port.outgoing_edges
        .iter()
        .filter_map(|edge| graph.edges.get(*edge).map(|edge| edge.target))
        .chain(
            port.incoming_edges
                .iter()
                .filter_map(|edge| graph.edges.get(*edge).map(|edge| edge.source)),
        )
        .filter(|other| *other != port_ref)
        .collect()
}

fn absolute_anchor(graph: &LGraph, port_ref: PortRef) -> LPoint {
    let node = &graph.layerless_nodes[port_ref.node];
    let port = &node.ports[port_ref.port];
    LPoint {
        x: node.position.x + port.position.x + port.anchor.x,
        y: node.position.y + port.position.y + port.anchor.y,
    }
}

fn bend_point_axis_delta(
    direction: RoutingDirection,
    source_anchor: LPoint,
    target_anchor: LPoint,
) -> f64 {
    match direction {
        RoutingDirection::WestToEast => source_anchor.y - target_anchor.y,
        RoutingDirection::NorthToSouth | RoutingDirection::SouthToNorth => {
            source_anchor.x - target_anchor.x
        }
    }
}

fn source_bend_point(
    direction: RoutingDirection,
    source_anchor: LPoint,
    routing_position: f64,
) -> LPoint {
    match direction {
        RoutingDirection::WestToEast => LPoint {
            x: routing_position,
            y: source_anchor.y,
        },
        RoutingDirection::NorthToSouth | RoutingDirection::SouthToNorth => LPoint {
            x: source_anchor.x,
            y: routing_position,
        },
    }
}

fn split_bend_point(
    direction: RoutingDirection,
    split_position: f64,
    routing_position: f64,
) -> LPoint {
    match direction {
        RoutingDirection::WestToEast => LPoint {
            x: routing_position,
            y: split_position,
        },
        RoutingDirection::NorthToSouth | RoutingDirection::SouthToNorth => LPoint {
            x: split_position,
            y: routing_position,
        },
    }
}

fn target_bend_point(
    direction: RoutingDirection,
    target_anchor: LPoint,
    routing_position: f64,
) -> LPoint {
    match direction {
        RoutingDirection::WestToEast => LPoint {
            x: routing_position,
            y: target_anchor.y,
        },
        RoutingDirection::NorthToSouth | RoutingDirection::SouthToNorth => LPoint {
            x: target_anchor.x,
            y: routing_position,
        },
    }
}

fn is_self_loop(graph: &LGraph, edge_index: usize) -> bool {
    graph.edges[edge_index].source.node == graph.edges[edge_index].target.node
}

fn add_bend_point_if_needed(graph: &mut LGraph, edge_index: usize, point: LPoint) {
    if let Some(last) = graph.edges[edge_index].bend_points.last()
        && (last.x - point.x).abs() < TOLERANCE
        && (last.y - point.y).abs() < TOLERANCE
    {
        return;
    }

    graph.edges[edge_index].bend_points.push(point);
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

fn center(first: f64, second: f64) -> f64 {
    (first + second) / 2.0
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
    use crate::graph::{LGraph, LNode, LayeredEdge, PortType};
    use crate::options::LayeredOptions;

    fn segment(incoming: &[f64], outgoing: &[f64]) -> HyperEdgeSegment {
        HyperEdgeSegment::with_incoming_outgoing(incoming.to_vec(), outgoing.to_vec())
    }

    fn route_test_graph(source_y: f64, target_y: f64) -> LGraph {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        let source = graph.layerless_nodes.len();
        let mut source_node = LNode::new("A", 40.0, 20.0, Some(0));
        source_node.position = LPoint {
            x: 0.0,
            y: source_y,
        };
        graph.layerless_nodes.push(source_node);

        let target = graph.layerless_nodes.len();
        let mut target_node = LNode::new("B", 40.0, 20.0, Some(1));
        target_node.position = LPoint {
            x: 100.0,
            y: target_y,
        };
        graph.layerless_nodes.push(target_node);

        graph.set_node_layer(source, 0);
        graph.set_node_layer(target, 1);

        let source_port = graph
            .add_port(
                source,
                PortType::Output,
                PortSide::East,
                LPoint { x: 40.0, y: 5.0 },
            )
            .unwrap();
        let target_port = graph
            .add_port(
                target,
                PortType::Output,
                PortSide::West,
                LPoint { x: 0.0, y: 15.0 },
            )
            .unwrap();

        graph.add_edge(LayeredEdge {
            id: "A-B".to_string(),
            source: source_port,
            target: target_port,
            source_node_id: "A".to_string(),
            target_node_id: "B".to_string(),
            labels: Vec::new(),
            minlen: 1,
            reversed: false,
            bend_points: Vec::new(),
            model_order: Some(0),
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
            thickness: 0.0,
            original_opposite_port: None,
            compound_segment: None,
        });

        graph
    }

    fn vertical_route_test_graph(source_x: f64, target_x: f64, source_side: PortSide) -> LGraph {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        let source = graph.layerless_nodes.len();
        let mut source_node = LNode::new("A", 20.0, 40.0, Some(0));
        source_node.position = LPoint {
            x: source_x,
            y: 0.0,
        };
        graph.layerless_nodes.push(source_node);

        let target = graph.layerless_nodes.len();
        let mut target_node = LNode::new("B", 20.0, 40.0, Some(1));
        target_node.position = LPoint {
            x: target_x,
            y: 100.0,
        };
        graph.layerless_nodes.push(target_node);

        graph.set_node_layer(source, 0);
        graph.set_node_layer(target, 1);

        let source_port_side = source_side;
        let target_port_side = source_side.opposed();
        let source_port_position = match source_port_side {
            PortSide::North => LPoint { x: 5.0, y: 0.0 },
            PortSide::South => LPoint { x: 5.0, y: 40.0 },
            _ => unreachable!("vertical test only uses north/south source ports"),
        };
        let target_port_position = match target_port_side {
            PortSide::North => LPoint { x: 15.0, y: 0.0 },
            PortSide::South => LPoint { x: 15.0, y: 40.0 },
            _ => unreachable!("vertical test only uses north/south target ports"),
        };

        let source_port = graph
            .add_port(
                source,
                PortType::Output,
                source_port_side,
                source_port_position,
            )
            .unwrap();
        let target_port = graph
            .add_port(
                target,
                PortType::Output,
                target_port_side,
                target_port_position,
            )
            .unwrap();

        graph.add_edge(LayeredEdge {
            id: "A-B".to_string(),
            source: source_port,
            target: target_port,
            source_node_id: "A".to_string(),
            target_node_id: "B".to_string(),
            labels: Vec::new(),
            minlen: 1,
            reversed: false,
            bend_points: Vec::new(),
            model_order: Some(0),
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
            thickness: 0.0,
            original_opposite_port: None,
            compound_segment: None,
        });

        graph
    }

    fn has_active_dependency(
        graph: &HyperEdgeGraph,
        source: usize,
        target: usize,
        dependency_type: DependencyType,
    ) -> bool {
        graph.dependencies.iter().any(|dependency| {
            dependency.source == Some(source)
                && dependency.target == Some(target)
                && dependency.dependency_type == dependency_type
        })
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

    #[test]
    fn split_segments_uses_reachable_free_area_and_rebuilds_critical_chain() {
        let mut graph = HyperEdgeGraph::default();
        let obstacle = graph.add_segment(segment(&[0.0], &[20.0]));
        let split = graph.add_segment(segment(&[20.4], &[40.0]));
        let dependency = graph.add_critical_dependency(split, obstacle);
        let thresholds = OrthogonalRoutingThresholds {
            conflict_threshold: 5.0,
            critical_conflict_threshold: 1.0,
        };

        split_segments(&mut graph, &[dependency], thresholds);

        let partner = graph.segments[split].split_partner.unwrap();
        assert_eq!(graph.segments[split].split_by, Some(obstacle));
        assert_eq!(
            graph.segments[split].outgoing_connection_coordinates,
            vec![30.2]
        );
        assert_eq!(
            graph.segments[partner].incoming_connection_coordinates,
            vec![30.2]
        );
        assert_eq!(
            graph.segments[partner].outgoing_connection_coordinates,
            vec![40.0]
        );
        assert!(has_active_dependency(
            &graph,
            split,
            obstacle,
            DependencyType::Critical
        ));
        assert!(has_active_dependency(
            &graph,
            obstacle,
            partner,
            DependencyType::Critical
        ));
    }

    #[test]
    fn break_critical_cycles_splits_segment_and_removes_critical_cycle() {
        let mut graph = HyperEdgeGraph::default();
        let a = graph.add_segment(segment(&[0.0], &[20.0]));
        let b = graph.add_segment(segment(&[20.4], &[0.4]));
        graph.add_critical_dependency(a, b);
        graph.add_critical_dependency(b, a);
        let thresholds = OrthogonalRoutingThresholds {
            conflict_threshold: 5.0,
            critical_conflict_threshold: 1.0,
        };
        let mut random = JavaRandom::new(1);

        break_critical_cycles(&mut graph, thresholds, &mut random);

        assert_eq!(graph.segments.len(), 3);
        assert!(
            graph
                .segments
                .iter()
                .any(|segment| segment.split_by.is_some())
        );
        assert!(detect_cycles(&mut graph, true, &mut JavaRandom::new(1)).is_empty());
    }

    #[test]
    fn route_edges_west_to_east_writes_orthogonal_bendpoints_for_non_straight_edge() {
        let mut graph = route_test_graph(0.0, 30.0);
        let left = graph.layers[0].nodes.clone();
        let right = graph.layers[1].nodes.clone();

        let slots = route_edges_west_to_east(&mut graph, Some(&left), Some(&right), 50.0, 10.0);

        assert_eq!(slots, 1);
        assert_eq!(
            graph.edges[0].bend_points,
            vec![LPoint { x: 50.0, y: 5.0 }, LPoint { x: 50.0, y: 45.0 }]
        );
    }

    #[test]
    fn route_edges_west_to_east_uses_actual_outgoing_edges_for_output_ports() {
        let mut graph = route_test_graph(0.0, 30.0);
        let source_port = graph.edges[0].source;
        graph.layerless_nodes[source_port.node].ports[source_port.port].port_type = PortType::Input;
        let left = graph.layers[0].nodes.clone();
        let right = graph.layers[1].nodes.clone();

        let slots = route_edges_west_to_east(&mut graph, Some(&left), Some(&right), 50.0, 10.0);

        assert_eq!(slots, 1);
        assert_eq!(
            graph.edges[0].bend_points,
            vec![LPoint { x: 50.0, y: 5.0 }, LPoint { x: 50.0, y: 45.0 }]
        );
    }

    #[test]
    fn route_edges_west_to_east_leaves_straight_edge_without_bendpoints() {
        let mut graph = route_test_graph(0.0, -10.0);
        let left = graph.layers[0].nodes.clone();
        let right = graph.layers[1].nodes.clone();

        let slots = route_edges_west_to_east(&mut graph, Some(&left), Some(&right), 50.0, 10.0);

        assert_eq!(slots, 0);
        assert!(graph.edges[0].bend_points.is_empty());
    }

    #[test]
    fn route_edges_north_to_south_writes_orthogonal_bendpoints_for_non_straight_edge() {
        let mut graph = vertical_route_test_graph(0.0, 30.0, PortSide::South);
        let source = graph.layers[0].nodes.clone();
        let target = graph.layers[1].nodes.clone();

        let slots = route_edges(
            &mut graph,
            RoutingDirection::NorthToSouth,
            Some(&source),
            Some(&target),
            60.0,
            10.0,
        );

        assert_eq!(slots, 1);
        assert_eq!(
            graph.edges[0].bend_points,
            vec![LPoint { x: 5.0, y: 60.0 }, LPoint { x: 45.0, y: 60.0 }]
        );
    }

    #[test]
    fn route_edges_south_to_north_writes_orthogonal_bendpoints_for_non_straight_edge() {
        let mut graph = vertical_route_test_graph(0.0, 30.0, PortSide::North);
        let source = graph.layers[0].nodes.clone();
        let target = graph.layers[1].nodes.clone();

        let slots = route_edges(
            &mut graph,
            RoutingDirection::SouthToNorth,
            Some(&source),
            Some(&target),
            -20.0,
            10.0,
        );

        assert_eq!(slots, 1);
        assert_eq!(
            graph.edges[0].bend_points,
            vec![LPoint { x: 5.0, y: -20.0 }, LPoint { x: 45.0, y: -20.0 }]
        );
    }
}
