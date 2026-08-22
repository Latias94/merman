use super::super::{RelationGraphBox, RelationResourceCheckpointCursor, find_box_ref};
use super::lanes::parallel_lane_margin;
use crate::AsciiError;
use crate::canvas::Canvas;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayeredRelationEdge {
    from_id: String,
    to_id: String,
    route_reversed: bool,
    label_width: usize,
    label_line_count: usize,
}

impl LayeredRelationEdge {
    pub(crate) fn new(
        from_id: impl Into<String>,
        to_id: impl Into<String>,
        label_width: usize,
        label_line_count: usize,
    ) -> Self {
        Self {
            from_id: from_id.into(),
            to_id: to_id.into(),
            route_reversed: false,
            label_width,
            label_line_count,
        }
    }

    /// Reverses only the physical route/rank projection while preserving semantic endpoints.
    #[must_use]
    pub(crate) const fn with_reversed_route(mut self, route_reversed: bool) -> Self {
        self.route_reversed = route_reversed;
        self
    }

    pub(crate) fn source_id(&self) -> &str {
        self.from_id.as_str()
    }

    pub(crate) fn target_id(&self) -> &str {
        self.to_id.as_str()
    }

    pub(crate) fn route_source_id(&self) -> &str {
        if self.route_reversed {
            self.target_id()
        } else {
            self.source_id()
        }
    }

    pub(crate) fn route_target_id(&self) -> &str {
        if self.route_reversed {
            self.source_id()
        } else {
            self.target_id()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayeredRelationError {
    MissingEndpoint,
    UnrelatedBoxes,
    Crossing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LayeredRelationPlanningError {
    Semantic(LayeredRelationError),
    Resource(AsciiError),
}

impl LayeredRelationPlanningError {
    pub(crate) fn into_ascii_error(
        self,
        map_semantic: impl FnOnce(LayeredRelationError) -> AsciiError,
    ) -> AsciiError {
        match self {
            Self::Semantic(error) => map_semantic(error),
            Self::Resource(error) => error,
        }
    }
}

impl From<AsciiError> for LayeredRelationPlanningError {
    fn from(error: AsciiError) -> Self {
        Self::Resource(error)
    }
}

impl From<LayeredRelationError> for LayeredRelationPlanningError {
    fn from(error: LayeredRelationError) -> Self {
        Self::Semantic(error)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PlacedRelationGraphBox<'a> {
    pub(crate) id: &'a str,
    pub(crate) relation_box: &'a RelationGraphBox,
    pub(crate) x: usize,
    pub(crate) y: usize,
    center_x: usize,
    right: usize,
    bottom: usize,
}

impl PlacedRelationGraphBox<'_> {
    pub(crate) fn id(&self) -> &str {
        self.id
    }

    pub(crate) fn x(&self) -> usize {
        self.x
    }

    pub(crate) fn width(&self) -> usize {
        self.relation_box.width()
    }

    pub(crate) fn height(&self) -> usize {
        self.relation_box.height()
    }

    pub(crate) fn y(&self) -> usize {
        self.y
    }

    pub(crate) fn center_x(&self) -> usize {
        self.center_x
    }

    pub(crate) fn right(&self) -> usize {
        self.right
    }

    pub(crate) fn bottom(&self) -> usize {
        self.bottom
    }

    pub(crate) fn draw_at(
        &self,
        canvas: &mut Canvas,
        resources: &ResourceContext,
    ) -> crate::Result<()> {
        self.relation_box.draw_at(canvas, self.x, self.y, resources)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayeredRelationLayoutKind {
    Standard,
    PlanarK2x2,
}

#[cfg(test)]
impl<'a> PlacedRelationGraphBox<'a> {
    pub(crate) fn for_test(
        id: &'a str,
        relation_box: &'a RelationGraphBox,
        x: usize,
        y: usize,
    ) -> Self {
        let right = x + relation_box.width().saturating_sub(1);
        let bottom = y + relation_box.height().saturating_sub(1);
        Self {
            id,
            relation_box,
            x,
            y,
            center_x: x + relation_box.width() / 2,
            right,
            bottom,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LayeredRelationPlan<'a> {
    placed: Vec<PlacedRelationGraphBox<'a>>,
    width: usize,
    height: usize,
    layout_kind: LayeredRelationLayoutKind,
}

impl<'a> LayeredRelationPlan<'a> {
    pub(crate) fn placed_boxes(&self) -> &[PlacedRelationGraphBox<'a>] {
        &self.placed
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn height(&self) -> usize {
        self.height
    }

    pub(crate) fn layout_kind(&self) -> LayeredRelationLayoutKind {
        self.layout_kind
    }
}

#[derive(Debug)]
pub(crate) struct RelationGraphComponent<'a> {
    boxes: Vec<&'a RelationGraphBox>,
    edge_indices: Vec<usize>,
}

impl<'a> RelationGraphComponent<'a> {
    pub(crate) fn boxes(&self) -> &[&'a RelationGraphBox] {
        &self.boxes
    }

    pub(crate) fn edge_indices(&self) -> &[usize] {
        &self.edge_indices
    }

    pub(crate) fn into_parts(self) -> (Vec<&'a RelationGraphBox>, Vec<usize>) {
        (self.boxes, self.edge_indices)
    }
}

fn index_relation_boxes<'a>(
    boxes: impl IntoIterator<Item = &'a RelationGraphBox>,
    capacity: usize,
    resources: &ResourceContext,
    checkpoints: &mut RelationResourceCheckpointCursor,
) -> Result<HashMap<&'a str, usize>, LayeredRelationPlanningError> {
    let mut index_by_id = HashMap::new();
    index_by_id
        .try_reserve(capacity)
        .map_err(|_| layout_allocation_failed())?;
    for (index, relation_box) in boxes.into_iter().enumerate() {
        checkpoints.tick(resources)?;
        index_by_id.entry(relation_box.id()).or_insert(index);
    }
    Ok(index_by_id)
}

fn relation_component_admission_work(
    box_count: usize,
    edge_count: usize,
    resources: &ResourceContext,
) -> Result<usize, LayeredRelationPlanningError> {
    // Five complete box-indexed passes cover id indexing, adjacency allocation, component
    // membership, component allocation, and final box materialization. Seven edge-indexed passes
    // cover endpoint indexing, adjacency materialization, component seeding, both undirected
    // adjacency visits, component counting, and final edge materialization. At most two edge
    // endpoints per edge become distinct BFS vertices.
    let box_passes = resources.checked_work_mul(box_count, 5)?;
    let edge_passes = resources.checked_work_mul(edge_count, 7)?;
    let endpoint_count = resources.checked_work_mul(edge_count, 2)?;
    let incident_vertex_bound = box_count.min(endpoint_count);
    Ok(resources.checked_work_add(
        resources.checked_work_add(box_passes, edge_passes)?,
        incident_vertex_bound,
    )?)
}

pub(crate) fn relation_components<'a>(
    boxes: &'a [RelationGraphBox],
    edges: &[LayeredRelationEdge],
    resources: &mut ResourceContext,
) -> std::result::Result<Vec<RelationGraphComponent<'a>>, LayeredRelationPlanningError> {
    let admission_work = relation_component_admission_work(boxes.len(), edges.len(), resources)?;
    resources.charge_layout_work(admission_work)?;

    let mut checkpoints = RelationResourceCheckpointCursor::new();
    let box_index_by_id =
        index_relation_boxes(boxes.iter(), boxes.len(), resources, &mut checkpoints)?;

    let mut degrees = Vec::new();
    degrees
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    degrees.resize(boxes.len(), 0usize);
    let mut edge_endpoints = Vec::new();
    edge_endpoints
        .try_reserve_exact(edges.len())
        .map_err(|_| layout_allocation_failed())?;
    for edge in edges {
        checkpoints.tick(resources)?;
        let source_index = box_index_by_id
            .get(edge.route_source_id())
            .copied()
            .ok_or(LayeredRelationError::MissingEndpoint)?;
        let target_index = box_index_by_id
            .get(edge.route_target_id())
            .copied()
            .ok_or(LayeredRelationError::MissingEndpoint)?;
        degrees[source_index] = degrees[source_index]
            .checked_add(1)
            .ok_or_else(|| work_overflow(resources))?;
        degrees[target_index] = degrees[target_index]
            .checked_add(1)
            .ok_or_else(|| work_overflow(resources))?;
        edge_endpoints.push((source_index, target_index));
    }

    let mut neighbors = Vec::new();
    neighbors
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    for degree in degrees {
        checkpoints.tick(resources)?;
        let mut adjacent = Vec::new();
        adjacent
            .try_reserve_exact(degree)
            .map_err(|_| layout_allocation_failed())?;
        neighbors.push(adjacent);
    }
    for &(source_index, target_index) in &edge_endpoints {
        checkpoints.tick(resources)?;
        neighbors
            .get_mut(source_index)
            .ok_or(LayeredRelationError::MissingEndpoint)?
            .push(target_index);
        neighbors
            .get_mut(target_index)
            .ok_or(LayeredRelationError::MissingEndpoint)?
            .push(source_index);
    }

    let mut component_by_box_index = Vec::new();
    component_by_box_index
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    component_by_box_index.resize(boxes.len(), None);
    let mut queue = VecDeque::new();
    queue
        .try_reserve(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut connected_component_count = 0usize;

    for &(source_index, _) in &edge_endpoints {
        checkpoints.tick(resources)?;
        if component_by_box_index[source_index].is_some() {
            continue;
        }

        let component_index = connected_component_count;
        connected_component_count = connected_component_count
            .checked_add(1)
            .ok_or_else(|| work_overflow(resources))?;
        component_by_box_index[source_index] = Some(component_index);
        queue.push_back(source_index);

        while let Some(box_index) = queue.pop_front() {
            checkpoints.tick(resources)?;
            for &neighbor_index in neighbors
                .get(box_index)
                .ok_or(LayeredRelationError::MissingEndpoint)?
            {
                checkpoints.tick(resources)?;
                if component_by_box_index[neighbor_index].is_none() {
                    component_by_box_index[neighbor_index] = Some(component_index);
                    queue.push_back(neighbor_index);
                }
            }
        }
    }

    let mut box_component_indices = Vec::new();
    box_component_indices
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut component_box_counts = Vec::new();
    component_box_counts
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    component_box_counts.resize(connected_component_count, 0usize);
    for relation_box in boxes {
        checkpoints.tick(resources)?;
        let indexed_box = box_index_by_id
            .get(relation_box.id())
            .copied()
            .ok_or(LayeredRelationError::MissingEndpoint)?;
        let component_index = match component_by_box_index[indexed_box] {
            Some(component_index) => component_index,
            None => {
                let component_index = component_box_counts.len();
                component_box_counts.push(0);
                component_index
            }
        };
        component_box_counts[component_index] = component_box_counts[component_index]
            .checked_add(1)
            .ok_or_else(|| work_overflow(resources))?;
        box_component_indices.push(component_index);
    }

    let mut component_edge_counts = Vec::new();
    component_edge_counts
        .try_reserve_exact(component_box_counts.len())
        .map_err(|_| layout_allocation_failed())?;
    component_edge_counts.resize(component_box_counts.len(), 0usize);
    for &(source_index, target_index) in &edge_endpoints {
        checkpoints.tick(resources)?;
        let source_component =
            component_by_box_index[source_index].ok_or(LayeredRelationError::MissingEndpoint)?;
        let target_component =
            component_by_box_index[target_index].ok_or(LayeredRelationError::MissingEndpoint)?;
        if source_component != target_component {
            return Err(LayeredRelationError::MissingEndpoint.into());
        }
        component_edge_counts[source_component] = component_edge_counts[source_component]
            .checked_add(1)
            .ok_or_else(|| work_overflow(resources))?;
    }

    let mut components = Vec::new();
    components
        .try_reserve_exact(component_box_counts.len())
        .map_err(|_| layout_allocation_failed())?;
    for (box_count, edge_count) in component_box_counts.into_iter().zip(component_edge_counts) {
        checkpoints.tick(resources)?;
        let mut component_boxes = Vec::new();
        component_boxes
            .try_reserve_exact(box_count)
            .map_err(|_| layout_allocation_failed())?;
        let mut component_edge_indices = Vec::new();
        component_edge_indices
            .try_reserve_exact(edge_count)
            .map_err(|_| layout_allocation_failed())?;
        components.push(RelationGraphComponent {
            boxes: component_boxes,
            edge_indices: component_edge_indices,
        });
    }

    for (relation_box, component_index) in boxes.iter().zip(box_component_indices) {
        checkpoints.tick(resources)?;
        components
            .get_mut(component_index)
            .ok_or(LayeredRelationError::MissingEndpoint)?
            .boxes
            .push(relation_box);
    }
    for (edge_index, &(source_index, _)) in edge_endpoints.iter().enumerate() {
        checkpoints.tick(resources)?;
        let component_index =
            component_by_box_index[source_index].ok_or(LayeredRelationError::MissingEndpoint)?;
        components
            .get_mut(component_index)
            .ok_or(LayeredRelationError::MissingEndpoint)?
            .edge_indices
            .push(edge_index);
    }

    Ok(components)
}

pub(crate) fn plan_layered_relation_boxes<'a>(
    boxes: &[&'a RelationGraphBox],
    edges: &[LayeredRelationEdge],
    horizontal_gap: usize,
    resources: &mut ResourceContext,
) -> std::result::Result<LayeredRelationPlan<'a>, LayeredRelationPlanningError> {
    if let Some(level_groups) = strict_k2_2_cycle_groups(boxes, edges, resources)? {
        let levels = HashMap::new();
        let (placed, width, height) = place_layered_boxes(
            &level_groups,
            edges,
            &levels,
            horizontal_gap,
            LayeredRelationLayoutKind::PlanarK2x2,
            resources,
        )?;
        return Ok(LayeredRelationPlan {
            placed,
            width,
            height,
            layout_kind: LayeredRelationLayoutKind::PlanarK2x2,
        });
    }

    let levels = layered_relation_levels(boxes, edges, resources)?;
    let (level_groups, layout_kind) =
        choose_ordered_layered_groups(boxes, edges, &levels, resources)?;
    let (placed, width, height) = place_layered_boxes(
        &level_groups,
        edges,
        &levels,
        horizontal_gap,
        layout_kind,
        resources,
    )?;
    Ok(LayeredRelationPlan {
        placed,
        width,
        height,
        layout_kind,
    })
}

fn layered_relation_levels(
    boxes: &[&RelationGraphBox],
    edges: &[LayeredRelationEdge],
    resources: &mut ResourceContext,
) -> std::result::Result<HashMap<String, usize>, LayeredRelationPlanningError> {
    // The fixed admission covers id indexing, endpoint indexing, adjacency allocation and
    // materialization, initial queue construction, and final level-map materialization. Queue and
    // adjacency traversal remain exact incremental charges because a node may be relaxed more than
    // once while discovering a longer path.
    let box_work = resources.checked_work_mul(boxes.len(), 4)?;
    let edge_work = resources.checked_work_mul(edges.len(), 2)?;
    resources.charge_layout_work(resources.checked_work_add(box_work, edge_work)?)?;

    let mut checkpoints = RelationResourceCheckpointCursor::new();
    let box_index_by_id = index_relation_boxes(
        boxes.iter().copied(),
        boxes.len(),
        resources,
        &mut checkpoints,
    )?;
    let mut incident = HashSet::new();
    incident
        .try_reserve(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut out_degrees = Vec::new();
    out_degrees
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    out_degrees.resize(boxes.len(), 0usize);
    let mut edge_endpoints = Vec::new();
    edge_endpoints
        .try_reserve_exact(edges.len())
        .map_err(|_| layout_allocation_failed())?;
    for edge in edges {
        checkpoints.tick(resources)?;
        let source_index = box_index_by_id
            .get(edge.route_source_id())
            .copied()
            .ok_or(LayeredRelationError::MissingEndpoint)?;
        let target_index = box_index_by_id
            .get(edge.route_target_id())
            .copied()
            .ok_or(LayeredRelationError::MissingEndpoint)?;
        incident.insert(source_index);
        incident.insert(target_index);
        out_degrees[source_index] = out_degrees[source_index]
            .checked_add(1)
            .ok_or_else(|| work_overflow(resources))?;
        edge_endpoints.push((source_index, target_index));
    }

    if incident.len() != boxes.len() {
        return Err(LayeredRelationError::UnrelatedBoxes.into());
    }

    let mut outgoing = Vec::new();
    outgoing
        .try_reserve(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    for degree in out_degrees {
        checkpoints.tick(resources)?;
        let mut children = Vec::new();
        children
            .try_reserve_exact(degree)
            .map_err(|_| layout_allocation_failed())?;
        outgoing.push(children);
    }
    for &(source_index, target_index) in &edge_endpoints {
        checkpoints.tick(resources)?;
        outgoing
            .get_mut(source_index)
            .ok_or(LayeredRelationError::MissingEndpoint)?
            .push(target_index);
    }

    let mut level_by_box_index = Vec::new();
    level_by_box_index
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    level_by_box_index.resize(boxes.len(), 0usize);
    let mut queue = VecDeque::new();
    queue
        .try_reserve(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut in_queue = Vec::new();
    in_queue
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    in_queue.resize(boxes.len(), true);
    for box_index in 0..boxes.len() {
        checkpoints.tick(resources)?;
        queue.push_back(box_index);
    }

    let level_cap = boxes.len().saturating_sub(1);
    while let Some(box_index) = queue.pop_front() {
        checkpoints.tick(resources)?;
        resources.charge_layout_work(1)?;
        *in_queue
            .get_mut(box_index)
            .ok_or(LayeredRelationError::MissingEndpoint)? = false;
        let current_level = level_by_box_index
            .get(box_index)
            .copied()
            .ok_or(LayeredRelationError::MissingEndpoint)?;
        for &child_index in outgoing
            .get(box_index)
            .ok_or(LayeredRelationError::MissingEndpoint)?
        {
            checkpoints.tick(resources)?;
            resources.charge_layout_work(1)?;
            let next_level = current_level
                .checked_add(1)
                .ok_or_else(|| nesting_overflow(resources))?;
            resources.check_nesting_depth(next_level)?;
            if next_level > level_cap {
                continue;
            }
            let child_level = level_by_box_index
                .get_mut(child_index)
                .ok_or(LayeredRelationError::MissingEndpoint)?;
            if *child_level < next_level {
                *child_level = next_level;
                let child_in_queue = in_queue
                    .get_mut(child_index)
                    .ok_or(LayeredRelationError::MissingEndpoint)?;
                if !*child_in_queue {
                    queue.push_back(child_index);
                    *child_in_queue = true;
                }
            }
        }
    }

    let mut levels = HashMap::<String, usize>::new();
    levels
        .try_reserve(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    for (box_index, relation_box) in boxes.iter().enumerate() {
        checkpoints.tick(resources)?;
        let level = level_by_box_index
            .get(box_index)
            .copied()
            .ok_or(LayeredRelationError::MissingEndpoint)?;
        levels.insert(relation_box.id().to_string(), level);
    }

    Ok(levels)
}

fn choose_ordered_layered_groups<'a>(
    boxes: &[&'a RelationGraphBox],
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    resources: &mut ResourceContext,
) -> std::result::Result<
    (Vec<Vec<&'a RelationGraphBox>>, LayeredRelationLayoutKind),
    LayeredRelationPlanningError,
> {
    let mut checkpoints = RelationResourceCheckpointCursor::new();
    let base = initial_layered_groups(boxes, levels, resources, &mut checkpoints)?;
    let max_sweeps = level_sweep_candidate_count(base.len(), resources)?;
    let mut seen = HashSet::new();
    let candidate_capacity = max_sweeps
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| work_overflow(resources))?;
    seen.try_reserve(candidate_capacity)
        .map_err(|_| layout_allocation_failed())?;
    let mut best: Option<(usize, Vec<Vec<&RelationGraphBox>>)> = None;

    if let Some(level_groups) = score_ordered_layered_group_candidate(
        &base,
        edges,
        levels,
        &mut seen,
        &mut best,
        resources,
        &mut checkpoints,
    )? {
        return Ok((level_groups, LayeredRelationLayoutKind::Standard));
    }

    for first_sweep in [LayeredRelationSweep::Downward, LayeredRelationSweep::Upward] {
        checkpoints.tick(resources)?;
        let mut groups = try_clone_level_groups(&base, resources, &mut checkpoints)?;
        for index in 0..max_sweeps {
            checkpoints.tick(resources)?;
            groups = apply_layered_relation_sweep(
                groups,
                first_sweep.alternating(index),
                edges,
                levels,
                resources,
                &mut checkpoints,
            )?;
            if let Some(level_groups) = score_ordered_layered_group_candidate(
                &groups,
                edges,
                levels,
                &mut seen,
                &mut best,
                resources,
                &mut checkpoints,
            )? {
                return Ok((level_groups, LayeredRelationLayoutKind::Standard));
            }
        }
    }

    let Some((crossings, level_groups)) = best else {
        return Ok((Vec::new(), LayeredRelationLayoutKind::Standard));
    };
    if crossings == 0 {
        Ok((level_groups, LayeredRelationLayoutKind::Standard))
    } else {
        Err(LayeredRelationError::Crossing.into())
    }
}

fn strict_k2_2_cycle_groups<'a>(
    boxes: &[&'a RelationGraphBox],
    edges: &[LayeredRelationEdge],
    resources: &ResourceContext,
) -> Result<Option<Vec<Vec<&'a RelationGraphBox>>>, LayeredRelationPlanningError> {
    if boxes.len() != 4 || edges.len() != 4 {
        return Ok(None);
    }

    let work = boxes
        .len()
        .checked_add(
            edges
                .len()
                .checked_mul(boxes.len())
                .ok_or_else(|| work_overflow(resources))?,
        )
        .ok_or_else(|| work_overflow(resources))?;
    resources.charge_layout_work(work)?;

    let mut ordered_boxes = Vec::new();
    ordered_boxes
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    ordered_boxes.extend(boxes.iter().copied());
    ordered_boxes.sort_by_key(|relation_box| relation_box.id());
    if ordered_boxes
        .windows(2)
        .any(|pair| pair[0].id() == pair[1].id())
    {
        return Ok(None);
    }

    let mut pairs = HashSet::new();
    pairs
        .try_reserve(edges.len())
        .map_err(|_| layout_allocation_failed())?;
    for edge in edges {
        if find_box_ref(boxes, edge.route_source_id()).is_none()
            || find_box_ref(boxes, edge.route_target_id()).is_none()
        {
            return Err(LayeredRelationError::MissingEndpoint.into());
        }
        if edge.route_source_id() == edge.route_target_id()
            || !pairs.insert(ordered_endpoint_pair(
                edge.route_source_id(),
                edge.route_target_id(),
            ))
        {
            return Ok(None);
        }
    }

    let first = ordered_boxes[0];
    let mut neighbors = Vec::new();
    neighbors
        .try_reserve_exact(2)
        .map_err(|_| layout_allocation_failed())?;
    for edge in edges {
        if edge.route_source_id() == first.id() {
            neighbors.push(edge.route_target_id());
        } else if edge.route_target_id() == first.id() {
            neighbors.push(edge.route_source_id());
        }
    }
    neighbors.sort_unstable();
    neighbors.dedup();
    let [first_neighbor_id, second_neighbor_id] = neighbors.as_slice() else {
        return Ok(None);
    };
    let first_neighbor_id = *first_neighbor_id;
    let second_neighbor_id = *second_neighbor_id;
    let Some(first_neighbor) = find_box_ref(boxes, first_neighbor_id) else {
        return Err(LayeredRelationError::MissingEndpoint.into());
    };
    let Some(second_neighbor) = find_box_ref(boxes, second_neighbor_id) else {
        return Err(LayeredRelationError::MissingEndpoint.into());
    };
    let mut opposite_candidates = ordered_boxes.iter().copied().filter(|relation_box| {
        relation_box.id() != first.id()
            && relation_box.id() != first_neighbor_id
            && relation_box.id() != second_neighbor_id
    });
    let Some(opposite) = opposite_candidates.next() else {
        return Ok(None);
    };
    if opposite_candidates.next().is_some()
        || ![
            ordered_endpoint_pair(first.id(), first_neighbor.id()),
            ordered_endpoint_pair(first.id(), second_neighbor.id()),
            ordered_endpoint_pair(opposite.id(), first_neighbor.id()),
            ordered_endpoint_pair(opposite.id(), second_neighbor.id()),
        ]
        .iter()
        .all(|pair| pairs.contains(pair))
    {
        return Ok(None);
    }

    Ok(Some(vec![
        vec![first, first_neighbor],
        vec![second_neighbor, opposite],
    ]))
}

fn ordered_endpoint_pair<'a>(left: &'a str, right: &'a str) -> (&'a str, &'a str) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn score_ordered_layered_group_candidate<'a>(
    candidate: &[Vec<&'a RelationGraphBox>],
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    seen: &mut HashSet<Vec<Vec<&'a str>>>,
    best: &mut Option<(usize, Vec<Vec<&'a RelationGraphBox>>)>,
    resources: &mut ResourceContext,
    checkpoints: &mut RelationResourceCheckpointCursor,
) -> Result<Option<Vec<Vec<&'a RelationGraphBox>>>, LayeredRelationPlanningError> {
    if !seen.insert(layered_group_candidate_key(
        candidate,
        resources,
        checkpoints,
    )?) {
        return Ok(None);
    }

    let crossings =
        crossing_layered_relation_count(edges, levels, candidate, resources, checkpoints)?;
    if crossings == 0 {
        return Ok(Some(try_clone_level_groups(
            candidate,
            resources,
            checkpoints,
        )?));
    }

    let should_replace = best
        .as_ref()
        .is_none_or(|(best_crossings, _)| crossings < *best_crossings);
    if should_replace {
        *best = Some((
            crossings,
            try_clone_level_groups(candidate, resources, checkpoints)?,
        ));
    }

    Ok(None)
}

fn layered_group_candidate_key<'a>(
    level_groups: &[Vec<&'a RelationGraphBox>],
    resources: &ResourceContext,
    checkpoints: &mut RelationResourceCheckpointCursor,
) -> Result<Vec<Vec<&'a str>>, LayeredRelationPlanningError> {
    let node_count = layered_group_node_count(level_groups, resources, checkpoints)?;
    charge_layered_group_copy_work(level_groups.len(), node_count, resources)?;
    let mut key = Vec::new();
    key.try_reserve_exact(level_groups.len())
        .map_err(|_| layout_allocation_failed())?;
    for group in level_groups {
        checkpoints.tick(resources)?;
        let mut ids = Vec::new();
        ids.try_reserve_exact(group.len())
            .map_err(|_| layout_allocation_failed())?;
        for relation_box in group {
            checkpoints.tick(resources)?;
            ids.push(relation_box.id());
        }
        key.push(ids);
    }
    Ok(key)
}

fn try_clone_level_groups<'a>(
    level_groups: &[Vec<&'a RelationGraphBox>],
    resources: &ResourceContext,
    checkpoints: &mut RelationResourceCheckpointCursor,
) -> Result<Vec<Vec<&'a RelationGraphBox>>, LayeredRelationPlanningError> {
    let node_count = layered_group_node_count(level_groups, resources, checkpoints)?;
    charge_layered_group_copy_work(level_groups.len(), node_count, resources)?;
    let mut cloned = Vec::new();
    cloned
        .try_reserve_exact(level_groups.len())
        .map_err(|_| layout_allocation_failed())?;
    for group in level_groups {
        checkpoints.tick(resources)?;
        let mut cloned_group = Vec::new();
        cloned_group
            .try_reserve_exact(group.len())
            .map_err(|_| layout_allocation_failed())?;
        for relation_box in group {
            checkpoints.tick(resources)?;
            cloned_group.push(*relation_box);
        }
        cloned.push(cloned_group);
    }
    Ok(cloned)
}

fn layered_group_node_count(
    level_groups: &[Vec<&RelationGraphBox>],
    resources: &ResourceContext,
    checkpoints: &mut RelationResourceCheckpointCursor,
) -> Result<usize, LayeredRelationPlanningError> {
    resources.charge_layout_work(level_groups.len())?;
    let mut node_count = 0usize;
    for group in level_groups {
        checkpoints.tick(resources)?;
        node_count = node_count
            .checked_add(group.len())
            .ok_or_else(|| work_overflow(resources))?;
    }
    Ok(node_count)
}

fn charge_layered_group_copy_work(
    group_count: usize,
    node_count: usize,
    resources: &ResourceContext,
) -> Result<(), LayeredRelationPlanningError> {
    let copy_work = resources.checked_work_add(group_count, node_count)?;
    resources.charge_layout_work(copy_work)?;
    Ok(())
}

fn level_sweep_candidate_count(
    level_count: usize,
    resources: &ResourceContext,
) -> Result<usize, LayeredRelationPlanningError> {
    Ok(level_count
        .checked_mul(2)
        .ok_or_else(|| work_overflow(resources))?
        .max(1))
}

#[derive(Debug, Clone, Copy)]
enum LayeredRelationSweep {
    Downward,
    Upward,
}

impl LayeredRelationSweep {
    fn alternating(self, index: usize) -> Self {
        if index.is_multiple_of(2) {
            self
        } else {
            self.opposite()
        }
    }

    fn opposite(self) -> Self {
        match self {
            Self::Downward => Self::Upward,
            Self::Upward => Self::Downward,
        }
    }
}

fn apply_layered_relation_sweep<'a>(
    level_groups: Vec<Vec<&'a RelationGraphBox>>,
    sweep: LayeredRelationSweep,
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    resources: &mut ResourceContext,
    checkpoints: &mut RelationResourceCheckpointCursor,
) -> Result<Vec<Vec<&'a RelationGraphBox>>, LayeredRelationPlanningError> {
    let node_count = level_groups.iter().try_fold(0usize, |total, group| {
        total
            .checked_add(group.len())
            .ok_or_else(|| work_overflow(resources))
    })?;
    let level_work = level_groups.len();
    let node_map_work = resources.checked_work_mul(node_count, 2)?;
    let edge_scan_work = resources.checked_work_mul(node_count, edges.len())?;
    let barycenter_work = edges.len();
    let sort_work = super::comparison_sort_work(node_count, resources)?;
    let work = resources.checked_work_add(
        resources.checked_work_add(level_work, node_map_work)?,
        resources.checked_work_add(
            edge_scan_work,
            resources.checked_work_add(barycenter_work, sort_work)?,
        )?,
    )?;
    resources.charge_layout_work(work.max(1))?;
    match sweep {
        LayeredRelationSweep::Downward => {
            order_layered_groups_downward(level_groups, edges, levels, resources, checkpoints)
        }
        LayeredRelationSweep::Upward => {
            order_layered_groups_upward(level_groups, edges, levels, resources, checkpoints)
        }
    }
}

fn crossing_layered_relation_count(
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    level_groups: &[Vec<&RelationGraphBox>],
    resources: &mut ResourceContext,
    checkpoints: &mut RelationResourceCheckpointCursor,
) -> Result<usize, LayeredRelationPlanningError> {
    let pair_count = edges
        .len()
        .checked_mul(edges.len().saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| work_overflow(resources))?;
    let node_count = level_groups.iter().try_fold(0usize, |total, group| {
        total
            .checked_add(group.len())
            .ok_or_else(|| work_overflow(resources))
    })?;
    let work = resources.checked_work_add(
        resources.checked_work_add(level_groups.len(), node_count)?,
        resources.checked_work_add(edges.len(), pair_count)?,
    )?;
    resources.charge_layout_work(work.max(1))?;
    let mut order_by_id = HashMap::new();
    order_by_id
        .try_reserve(node_count)
        .map_err(|_| layout_allocation_failed())?;
    for group in level_groups {
        checkpoints.tick(resources)?;
        for (index, relation_box) in group.iter().enumerate() {
            checkpoints.tick(resources)?;
            order_by_id.insert(relation_box.id(), index);
        }
    }

    let mut crossings = 0usize;
    for (left_index, left) in edges.iter().enumerate() {
        checkpoints.tick(resources)?;
        let left_from_level = levels.get(left.route_source_id()).copied().unwrap_or(0);
        let left_to_level = levels.get(left.route_target_id()).copied().unwrap_or(0);
        let left_from_order = order_by_id
            .get(left.route_source_id())
            .copied()
            .unwrap_or(0);
        let left_to_order = order_by_id
            .get(left.route_target_id())
            .copied()
            .unwrap_or(0);
        for right in edges.iter().skip(left_index + 1) {
            checkpoints.tick(resources)?;
            if levels.get(right.route_source_id()).copied().unwrap_or(0) != left_from_level
                || levels.get(right.route_target_id()).copied().unwrap_or(0) != left_to_level
            {
                continue;
            }

            let right_from_order = order_by_id
                .get(right.route_source_id())
                .copied()
                .unwrap_or(0);
            let right_to_order = order_by_id
                .get(right.route_target_id())
                .copied()
                .unwrap_or(0);

            let crosses_left_to_right =
                left_from_order < right_from_order && left_to_order > right_to_order;
            let crosses_right_to_left =
                left_from_order > right_from_order && left_to_order < right_to_order;
            if crosses_left_to_right || crosses_right_to_left {
                crossings = crossings
                    .checked_add(1)
                    .ok_or_else(|| work_overflow(resources))?;
            }
        }
    }

    Ok(crossings)
}

fn initial_layered_groups<'a>(
    boxes: &[&'a RelationGraphBox],
    levels: &HashMap<String, usize>,
    resources: &ResourceContext,
    checkpoints: &mut RelationResourceCheckpointCursor,
) -> Result<Vec<Vec<&'a RelationGraphBox>>, LayeredRelationPlanningError> {
    resources.charge_layout_work(levels.len())?;
    let mut max_level = 0usize;
    for level in levels.values().copied() {
        checkpoints.tick(resources)?;
        max_level = max_level.max(level);
    }
    let level_count = max_level
        .checked_add(1)
        .ok_or_else(|| nesting_overflow(resources))?;
    resources.check_nesting_depth(max_level)?;
    let box_work = resources.checked_work_mul(boxes.len(), 2)?;
    let level_work = resources.checked_work_mul(level_count, 2)?;
    resources.charge_layout_work(resources.checked_work_add(box_work, level_work)?)?;
    let mut level_groups = Vec::new();
    level_groups
        .try_reserve_exact(level_count)
        .map_err(|_| layout_allocation_failed())?;
    let mut level_sizes = Vec::<usize>::new();
    level_sizes
        .try_reserve_exact(level_count)
        .map_err(|_| layout_allocation_failed())?;
    for _ in 0..level_count {
        checkpoints.tick(resources)?;
        level_sizes.push(0);
    }
    for relation_box in boxes {
        checkpoints.tick(resources)?;
        if let Some(level) = levels.get(relation_box.id()).copied() {
            level_sizes[level] = level_sizes[level]
                .checked_add(1)
                .ok_or_else(|| work_overflow(resources))?;
        }
    }
    for level_size in level_sizes {
        checkpoints.tick(resources)?;
        let mut group = Vec::new();
        group
            .try_reserve_exact(level_size)
            .map_err(|_| layout_allocation_failed())?;
        level_groups.push(group);
    }
    for relation_box in boxes {
        checkpoints.tick(resources)?;
        if let Some(level) = levels.get(relation_box.id()).copied() {
            level_groups[level].push(*relation_box);
        }
    }

    Ok(level_groups)
}

fn order_layered_groups_downward<'a>(
    mut level_groups: Vec<Vec<&'a RelationGraphBox>>,
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    resources: &ResourceContext,
    checkpoints: &mut RelationResourceCheckpointCursor,
) -> Result<Vec<Vec<&'a RelationGraphBox>>, LayeredRelationPlanningError> {
    let max_level = level_groups.len().saturating_sub(1);
    for level in 1..=max_level {
        checkpoints.tick(resources)?;
        let (previous_levels, current_levels) = level_groups.split_at_mut(level);
        let previous_group = &previous_levels[level - 1];
        let current_group = &mut current_levels[0];
        let mut previous_order = HashMap::new();
        previous_order
            .try_reserve(previous_group.len())
            .map_err(|_| layout_allocation_failed())?;
        for (index, relation_box) in previous_group.iter().enumerate() {
            checkpoints.tick(resources)?;
            previous_order.insert(relation_box.id(), index);
        }
        let mut original_order = HashMap::new();
        original_order
            .try_reserve(current_group.len())
            .map_err(|_| layout_allocation_failed())?;
        let mut neighbor_order = HashMap::new();
        neighbor_order
            .try_reserve(current_group.len())
            .map_err(|_| layout_allocation_failed())?;
        for (index, relation_box) in current_group.iter().enumerate() {
            checkpoints.tick(resources)?;
            original_order.insert(relation_box.id(), index);
            let mut neighbor_orders = Vec::new();
            neighbor_orders
                .try_reserve_exact(edges.len())
                .map_err(|_| layout_allocation_failed())?;
            for edge in edges {
                checkpoints.tick(resources)?;
                if edge.route_target_id() != relation_box.id()
                    || levels.get(edge.route_source_id()).copied() != Some(level - 1)
                {
                    continue;
                }
                if let Some(order) = previous_order.get(edge.route_source_id()).copied() {
                    neighbor_orders.push(order);
                }
            }
            neighbor_order.insert(
                relation_box.id(),
                barycenter_order(&neighbor_orders, resources, checkpoints)?,
            );
        }

        resources.checkpoint()?;
        current_group.sort_by_key(|relation_box| {
            (
                neighbor_order
                    .get(relation_box.id())
                    .copied()
                    .unwrap_or(usize::MAX),
                original_order
                    .get(relation_box.id())
                    .copied()
                    .unwrap_or(usize::MAX),
            )
        });
        resources.checkpoint()?;
    }

    Ok(level_groups)
}

fn order_layered_groups_upward<'a>(
    mut level_groups: Vec<Vec<&'a RelationGraphBox>>,
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    resources: &ResourceContext,
    checkpoints: &mut RelationResourceCheckpointCursor,
) -> Result<Vec<Vec<&'a RelationGraphBox>>, LayeredRelationPlanningError> {
    let max_level = level_groups.len().saturating_sub(1);
    for level in (0..max_level).rev() {
        checkpoints.tick(resources)?;
        let (through_current, next_levels) = level_groups.split_at_mut(level + 1);
        let current_group = &mut through_current[level];
        let next_group = &next_levels[0];
        let mut next_order = HashMap::new();
        next_order
            .try_reserve(next_group.len())
            .map_err(|_| layout_allocation_failed())?;
        for (index, relation_box) in next_group.iter().enumerate() {
            checkpoints.tick(resources)?;
            next_order.insert(relation_box.id(), index);
        }
        let mut original_order = HashMap::new();
        original_order
            .try_reserve(current_group.len())
            .map_err(|_| layout_allocation_failed())?;
        let mut neighbor_order = HashMap::new();
        neighbor_order
            .try_reserve(current_group.len())
            .map_err(|_| layout_allocation_failed())?;
        for (index, relation_box) in current_group.iter().enumerate() {
            checkpoints.tick(resources)?;
            original_order.insert(relation_box.id(), index);
            let mut neighbor_orders = Vec::new();
            neighbor_orders
                .try_reserve_exact(edges.len())
                .map_err(|_| layout_allocation_failed())?;
            for edge in edges {
                checkpoints.tick(resources)?;
                if edge.route_source_id() != relation_box.id()
                    || levels.get(edge.route_target_id()).copied() != Some(level + 1)
                {
                    continue;
                }
                if let Some(order) = next_order.get(edge.route_target_id()).copied() {
                    neighbor_orders.push(order);
                }
            }
            neighbor_order.insert(
                relation_box.id(),
                barycenter_order(&neighbor_orders, resources, checkpoints)?,
            );
        }

        resources.checkpoint()?;
        current_group.sort_by_key(|relation_box| {
            (
                neighbor_order
                    .get(relation_box.id())
                    .copied()
                    .unwrap_or(usize::MAX),
                original_order
                    .get(relation_box.id())
                    .copied()
                    .unwrap_or(usize::MAX),
            )
        });
        resources.checkpoint()?;
    }

    Ok(level_groups)
}

fn barycenter_order(
    neighbor_orders: &[usize],
    resources: &ResourceContext,
    checkpoints: &mut RelationResourceCheckpointCursor,
) -> Result<usize, LayeredRelationPlanningError> {
    if neighbor_orders.is_empty() {
        return Ok(usize::MAX);
    }

    let mut sum = 0usize;
    for order in neighbor_orders {
        checkpoints.tick(resources)?;
        sum = sum
            .checked_add(*order)
            .ok_or_else(|| work_overflow(resources))?;
    }
    let whole = sum / neighbor_orders.len();
    let remainder = sum % neighbor_orders.len();
    whole
        .checked_mul(2)
        .and_then(|value| {
            remainder
                .checked_mul(2)
                .and_then(|fraction| value.checked_add(fraction / neighbor_orders.len()))
        })
        .ok_or_else(|| work_overflow(resources).into())
}

fn place_layered_boxes<'a>(
    level_groups: &[Vec<&'a RelationGraphBox>],
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    horizontal_gap: usize,
    layout_kind: LayeredRelationLayoutKind,
    resources: &mut ResourceContext,
) -> Result<(Vec<PlacedRelationGraphBox<'a>>, usize, usize), LayeredRelationPlanningError> {
    if layout_kind == LayeredRelationLayoutKind::PlanarK2x2 {
        return place_planar_k2_2_boxes(level_groups, edges, horizontal_gap, resources);
    }

    let max_level = level_groups.len().saturating_sub(1);

    let mut group_widths = Vec::new();
    group_widths
        .try_reserve_exact(level_groups.len())
        .map_err(|_| layout_allocation_failed())?;
    for group in level_groups {
        let boxes_width = group.iter().try_fold(0usize, |width, relation_box| {
            resources.checked_grid_add(width, relation_box.width())
        })?;
        let gaps_width =
            resources.checked_grid_mul(horizontal_gap, group.len().saturating_sub(1))?;
        group_widths.push(resources.checked_grid_add(boxes_width, gaps_width)?);
    }
    let max_label_width = edges.iter().map(|edge| edge.label_width).max().unwrap_or(0);
    let spanning_lane_margin = spanning_lane_margin(level_groups, edges, levels, resources)?;
    let spanning_margin = resources.checked_grid_mul(spanning_lane_margin, 2)?;
    let parallel_lane_margin = parallel_lane_margin(
        edges
            .iter()
            .map(|edge| (edge.route_source_id(), edge.route_target_id())),
        resources,
    )?;
    let parallel_margin = resources.checked_grid_mul(parallel_lane_margin, 2)?;
    let content_width = resources.checked_grid_add(
        resources.checked_grid_add(
            group_widths
                .iter()
                .copied()
                .max()
                .unwrap_or(0)
                .max(max_label_width),
            spanning_margin,
        )?,
        parallel_margin,
    )?;
    let global_center = content_width / 2;

    let gap_heights = precompute_layered_relation_gap_heights(edges, levels, max_level, resources)?;
    let height = level_groups.iter().enumerate().try_fold(
        0usize,
        |height, (level, group)| -> Result<usize, LayeredRelationPlanningError> {
            let row_height = group
                .iter()
                .map(|relation_box| relation_box.height())
                .max()
                .unwrap_or(0);
            let height = resources.checked_grid_add(height, row_height)?;
            if level < max_level {
                Ok(resources.checked_grid_add(height, gap_heights[level])?)
            } else {
                Ok(height)
            }
        },
    )?;
    resources.grid_extent(content_width, height)?;

    let mut placed = Vec::new();
    let placed_count = level_groups.iter().try_fold(0usize, |total, group| {
        total
            .checked_add(group.len())
            .ok_or_else(|| work_overflow(resources))
    })?;
    resources.charge_layout_work(placed_count.max(1))?;
    placed
        .try_reserve_exact(placed_count)
        .map_err(|_| layout_allocation_failed())?;
    let mut y = 0;
    for (level, group) in level_groups.iter().enumerate() {
        let group_width = group_widths[level];
        let mut x = global_center
            .checked_sub(group_width / 2)
            .ok_or_else(|| grid_overflow(resources))?;
        for relation_box in group {
            let right_exclusive = resources.checked_grid_add(x, relation_box.width())?;
            let bottom_exclusive = resources.checked_grid_add(y, relation_box.height())?;
            placed.push(PlacedRelationGraphBox {
                id: relation_box.id(),
                relation_box,
                x,
                y,
                center_x: resources.checked_grid_add(x, relation_box.width() / 2)?,
                right: right_exclusive.checked_sub(1).unwrap_or(x),
                bottom: bottom_exclusive.checked_sub(1).unwrap_or(y),
            });
            x = resources.checked_grid_add(right_exclusive, horizontal_gap)?;
        }

        let row_height = group
            .iter()
            .map(|relation_box| relation_box.height())
            .max()
            .unwrap_or(0);
        y = resources.checked_grid_add(y, row_height)?;
        if level < max_level {
            y = resources.checked_grid_add(y, gap_heights[level])?;
        }
    }

    debug_assert_eq!(y, height);
    Ok((placed, content_width, height))
}

fn place_planar_k2_2_boxes<'a>(
    rows: &[Vec<&'a RelationGraphBox>],
    edges: &[LayeredRelationEdge],
    horizontal_gap: usize,
    resources: &ResourceContext,
) -> Result<(Vec<PlacedRelationGraphBox<'a>>, usize, usize), LayeredRelationPlanningError> {
    let [top, bottom] = rows else {
        return Err(LayeredRelationError::Crossing.into());
    };
    let ([top_left, top_right], [bottom_left, bottom_right]) = (top.as_slice(), bottom.as_slice())
    else {
        return Err(LayeredRelationError::Crossing.into());
    };

    let left_width = top_left.width().max(bottom_left.width());
    let right_width = top_right.width().max(bottom_right.width());
    let column_width = resources.checked_grid_add(
        resources.checked_grid_add(left_width, horizontal_gap)?,
        right_width,
    )?;
    let max_label_width = edges.iter().map(|edge| edge.label_width).max().unwrap_or(0);
    let label_margin = resources.checked_grid_add(max_label_width / 2, 1)?;
    let width =
        resources.checked_grid_add(column_width, resources.checked_grid_mul(label_margin, 2)?)?;

    let max_label_lines = edges
        .iter()
        .map(|edge| edge.label_line_count)
        .max()
        .unwrap_or(0);
    let exterior_margin = resources.checked_grid_add(max_label_lines, 3)?;
    let middle_gap = resources.checked_grid_add(max_label_lines, 3)?.max(5);
    let top_height = top_left.height().max(top_right.height());
    let bottom_height = bottom_left.height().max(bottom_right.height());
    let bottom_y = resources.checked_grid_add(
        resources.checked_grid_add(exterior_margin, top_height)?,
        middle_gap,
    )?;
    let height = resources.checked_grid_add(
        resources.checked_grid_add(bottom_y, bottom_height)?,
        exterior_margin,
    )?;
    resources.grid_extent(width, height)?;
    resources.charge_layout_work(4)?;

    let left_center = resources.checked_grid_add(label_margin, left_width / 2)?;
    let right_center = resources.checked_grid_add(
        resources.checked_grid_add(label_margin, left_width)?,
        resources.checked_grid_add(horizontal_gap, right_width / 2)?,
    )?;
    let mut placed = Vec::new();
    placed
        .try_reserve_exact(4)
        .map_err(|_| layout_allocation_failed())?;
    for (relation_box, center_x, y) in [
        (*top_left, left_center, exterior_margin),
        (*top_right, right_center, exterior_margin),
        (*bottom_left, left_center, bottom_y),
        (*bottom_right, right_center, bottom_y),
    ] {
        let x = center_x
            .checked_sub(relation_box.width() / 2)
            .ok_or_else(|| grid_overflow(resources))?;
        let right_exclusive = resources.checked_grid_add(x, relation_box.width())?;
        let bottom_exclusive = resources.checked_grid_add(y, relation_box.height())?;
        placed.push(PlacedRelationGraphBox {
            id: relation_box.id(),
            relation_box,
            x,
            y,
            center_x,
            right: right_exclusive.checked_sub(1).unwrap_or(x),
            bottom: bottom_exclusive.checked_sub(1).unwrap_or(y),
        });
    }

    Ok((placed, width, height))
}

fn spanning_lane_margin(
    level_groups: &[Vec<&RelationGraphBox>],
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    resources: &ResourceContext,
) -> Result<usize, LayeredRelationPlanningError> {
    let has_spanning_edge = edges.iter().any(|edge| {
        let from_level = levels.get(edge.route_source_id()).copied().unwrap_or(0);
        let to_level = levels.get(edge.route_target_id()).copied().unwrap_or(0);
        from_level.abs_diff(to_level) > 1
    });
    if !has_spanning_edge {
        return Ok(0);
    }

    Ok(resources.checked_grid_add(
        level_groups
            .iter()
            .flatten()
            .map(|relation_box| relation_box.width() / 2)
            .max()
            .unwrap_or(0),
        3,
    )?)
}

fn precompute_layered_relation_gap_heights(
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    gap_count: usize,
    resources: &ResourceContext,
) -> Result<Vec<usize>, LayeredRelationPlanningError> {
    if gap_count == 0 {
        return Ok(Vec::new());
    }

    resources.transaction(
        |resources| -> Result<Vec<usize>, LayeredRelationPlanningError> {
            // First measure the exact number of crossed-gap updates without allocating the table.
            // The measurement pass itself is real work, so admit it before scanning the edges.
            resources.charge_layout_work(edges.len())?;
            let mut crossed_gap_visits = 0usize;
            let mut measurement_checkpoints = RelationResourceCheckpointCursor::new();
            for edge in edges {
                measurement_checkpoints.tick(resources)?;
                let from_level = levels.get(edge.route_source_id()).copied().unwrap_or(0);
                let to_level = levels.get(edge.route_target_id()).copied().unwrap_or(0);
                let min_level = from_level.min(to_level);
                let end = from_level.max(to_level).min(gap_count);
                crossed_gap_visits = resources
                    .checked_work_add(crossed_gap_visits, end.saturating_sub(min_level))?;
            }

            // Admit table initialization, the second edge pass, and every actual crossed-gap
            // update before allocating. This avoids the previous levels×edges overcharge while
            // still rejecting a tight budget before the reusable table exists.
            let fill_work = resources.checked_work_add(
                gap_count,
                resources.checked_work_add(edges.len(), crossed_gap_visits)?,
            )?;
            resources.charge_layout_work(fill_work)?;

            let mut gap_heights = Vec::new();
            gap_heights
                .try_reserve_exact(gap_count)
                .map_err(|_| layout_allocation_failed())?;
            gap_heights.resize(gap_count, 3);

            let mut fill_checkpoints = RelationResourceCheckpointCursor::new();
            for edge in edges {
                fill_checkpoints.tick(resources)?;
                let from_level = levels.get(edge.route_source_id()).copied().unwrap_or(0);
                let to_level = levels.get(edge.route_target_id()).copied().unwrap_or(0);
                let min_level = from_level.min(to_level);
                let end = from_level.max(to_level).min(gap_count);
                if min_level >= end {
                    continue;
                }
                let edge_gap_height = resources.checked_grid_add(edge.label_line_count, 3)?;
                for height in gap_heights[min_level..end].iter_mut() {
                    fill_checkpoints.tick(resources)?;
                    *height = (*height).max(edge_gap_height);
                }
            }

            Ok(gap_heights)
        },
    )
}

fn grid_overflow(resources: &ResourceContext) -> AsciiError {
    resources.grid_overflow()
}

fn work_overflow(resources: &ResourceContext) -> AsciiError {
    resources.work_overflow()
}

fn nesting_overflow(resources: &ResourceContext) -> AsciiError {
    resources.nesting_overflow()
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;
    use merman_core::{CancelReason, OperationControl, OperationPhase};

    #[test]
    fn reversed_route_preserves_semantic_endpoints() {
        let edge = LayeredRelationEdge::new("source", "target", 0, 0).with_reversed_route(true);

        assert_eq!(edge.source_id(), "source");
        assert_eq!(edge.target_id(), "target");
        assert_eq!(edge.route_source_id(), "target");
        assert_eq!(edge.route_target_id(), "source");
    }

    #[test]
    fn score_ordered_layered_group_candidate_skips_duplicate_orders() {
        let boxes = [
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let candidate = vec![vec![&boxes[0]], vec![&boxes[1]]];
        let edges = vec![LayeredRelationEdge::new("a", "b", 0, 0)];
        let levels = HashMap::from([("a".to_string(), 0), ("b".to_string(), 1)]);
        let mut seen = HashSet::new();
        let mut best = None;
        let mut resources = ResourceContext::new(AsciiResourcePolicy::default());
        let mut checkpoints = RelationResourceCheckpointCursor::new();

        let first = score_ordered_layered_group_candidate(
            &candidate,
            &edges,
            &levels,
            &mut seen,
            &mut best,
            &mut resources,
            &mut checkpoints,
        )
        .expect("candidate scoring should fit");
        let duplicate = score_ordered_layered_group_candidate(
            &candidate,
            &edges,
            &levels,
            &mut seen,
            &mut best,
            &mut resources,
            &mut checkpoints,
        )
        .expect("duplicate scoring should fit");

        assert!(first.is_some());
        assert!(duplicate.is_none());
        assert_eq!(seen.len(), 1);
    }

    #[test]
    fn relation_component_indexing_obeys_conservative_work_admission() {
        let boxes = [
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
            RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
            RelationGraphBox::new("d".to_string(), vec!["D".to_string()], 1),
            RelationGraphBox::new("isolated".to_string(), vec!["I".to_string()], 1),
        ];
        let edges = vec![
            LayeredRelationEdge::new("a", "b", 0, 0),
            LayeredRelationEdge::new("c", "d", 0, 0),
        ];
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut measured = ResourceContext::new(unbounded);

        let components = relation_components(&boxes, &edges, &mut measured)
            .expect("indexed components should materialize");
        let admitted_work = measured.layout_work_used();
        assert_eq!(
            admitted_work,
            relation_component_admission_work(boxes.len(), edges.len(), &measured)
                .expect("component work formula should fit")
        );
        assert_eq!(components.len(), 3);

        let admitted_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, admitted_work)
            .expect("admitted component work limit should be valid");
        let mut admitted = ResourceContext::new(admitted_policy);
        relation_components(&boxes, &edges, &mut admitted)
            .expect("admitted component work limit should succeed");
        assert_eq!(admitted.layout_work_used(), admitted_work);

        let below_policy = unbounded
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                admitted_work
                    .checked_sub(1)
                    .expect("component work is non-zero"),
            )
            .expect("below-admission component work limit should be valid");
        let mut below = ResourceContext::new(below_policy);
        let error = relation_components(&boxes, &edges, &mut below)
            .expect_err("below-admission component work must fail before materialization");
        assert!(matches!(
            error,
            LayeredRelationPlanningError::Resource(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
        ));
        assert_eq!(below.layout_work_used(), 0);
    }

    #[test]
    fn layered_group_clone_admits_copy_before_materialization() {
        let boxes = (0..96)
            .map(|index| RelationGraphBox::new(format!("n{index}"), vec![format!("N{index}")], 1))
            .collect::<Vec<_>>();
        let groups = vec![boxes.iter().collect::<Vec<_>>()];
        let admitted_work = groups
            .len()
            .checked_mul(2)
            .and_then(|work| work.checked_add(boxes.len()))
            .expect("test work should fit");
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, admitted_work - 1)
            .expect("below-admission work limit should be valid");
        let resources = ResourceContext::new(policy);
        let mut checkpoints = RelationResourceCheckpointCursor::new();

        let error = try_clone_level_groups(&groups, &resources, &mut checkpoints)
            .expect_err("copy work must be admitted before the clone is allocated");

        assert!(matches!(
            error,
            LayeredRelationPlanningError::Resource(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
        ));
        assert_eq!(resources.layout_work_used(), groups.len());
    }

    #[test]
    fn layered_gap_height_prepass_obeys_exact_work_admission() {
        const GAP_COUNT: usize = 3;
        const EDGE_COUNT: usize = 4;
        const CROSSED_GAP_VISITS: usize = 8;
        const EXPECTED_WORK: usize = EDGE_COUNT + GAP_COUNT + EDGE_COUNT + CROSSED_GAP_VISITS;

        let levels = HashMap::from([
            ("a".to_string(), 0),
            ("b".to_string(), 1),
            ("c".to_string(), 2),
            ("d".to_string(), 3),
        ]);
        let edges = vec![
            LayeredRelationEdge::new("a", "d", 0, 2),
            LayeredRelationEdge::new("b", "c", 0, 4),
            LayeredRelationEdge::new("d", "a", 0, 1),
            LayeredRelationEdge::new("a", "b", 0, 0),
        ];
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXPECTED_WORK)
            .expect("exact gap-prepass work limit should be valid");
        let exact = ResourceContext::new(exact_policy);
        let gap_heights =
            precompute_layered_relation_gap_heights(&edges, &levels, GAP_COUNT, &exact)
                .expect("exact gap-prepass work should succeed");
        assert_eq!(gap_heights, vec![5, 7, 5]);
        assert_eq!(exact.layout_work_used(), EXPECTED_WORK);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXPECTED_WORK - 1)
            .expect("below gap-prepass work limit should be valid");
        let below = ResourceContext::new(below_policy);
        let error = precompute_layered_relation_gap_heights(&edges, &levels, GAP_COUNT, &below)
            .expect_err("below gap-prepass work must fail before allocation");
        assert!(matches!(
            error,
            LayeredRelationPlanningError::Resource(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == EXPECTED_WORK
                    && details.max == EXPECTED_WORK - 1
        ));
        assert_eq!(below.layout_work_used(), 0);
    }

    #[test]
    fn initial_layered_grouping_observes_cancellation_inside_level_scan() {
        let boxes = (0..96)
            .map(|index| RelationGraphBox::new(format!("n{index}"), vec![format!("N{index}")], 1))
            .collect::<Vec<_>>();
        let box_refs = boxes.iter().collect::<Vec<_>>();
        let levels = boxes
            .iter()
            .enumerate()
            .map(|(index, relation_box)| (relation_box.id().to_string(), index))
            .collect::<HashMap<_, _>>();
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        let ledger = ResourceContext::new(policy);
        let resources = ledger.controlled(control, OperationPhase::Layout);
        let mut checkpoints = RelationResourceCheckpointCursor::new();

        let error = initial_layered_groups(&box_refs, &levels, &resources, &mut checkpoints)
            .expect_err("large initial grouping should observe scheduled cancellation");

        assert!(matches!(
            error,
            LayeredRelationPlanningError::Resource(AsciiError::Cancelled(cancelled))
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
    }

    #[test]
    fn crossing_pair_scan_observes_cancellation_inside_the_inner_loop() {
        let boxes = [
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let level_groups = vec![vec![&boxes[0]], vec![&boxes[1]]];
        let levels = HashMap::from([("a".to_string(), 0), ("b".to_string(), 1)]);
        let edges = (0..20)
            .map(|_| LayeredRelationEdge::new("a", "b", 0, 0))
            .collect::<Vec<_>>();
        let policy = AsciiResourcePolicy::default();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        let ledger = ResourceContext::new(policy);
        let mut resources = ledger.controlled(control, OperationPhase::Layout);
        let mut checkpoints = RelationResourceCheckpointCursor::new();

        let error = crossing_layered_relation_count(
            &edges,
            &levels,
            &level_groups,
            &mut resources,
            &mut checkpoints,
        )
        .expect_err("pair scan should observe scheduled cancellation");

        assert!(matches!(
            error,
            LayeredRelationPlanningError::Resource(AsciiError::Cancelled(cancelled))
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
    }

    #[test]
    fn layered_sweep_observes_cancellation_inside_edge_scans() {
        let boxes = (0..72)
            .map(|index| RelationGraphBox::new(format!("n{index}"), vec![format!("N{index}")], 1))
            .collect::<Vec<_>>();
        let mut levels = HashMap::new();
        levels.insert("n0".to_string(), 0);
        let mut second_level = Vec::new();
        second_level
            .try_reserve_exact(boxes.len() - 1)
            .expect("test level allocation should fit");
        let mut edges = Vec::new();
        edges
            .try_reserve_exact(boxes.len() - 1)
            .expect("test edge allocation should fit");
        for relation_box in boxes.iter().skip(1) {
            levels.insert(relation_box.id().to_string(), 1);
            second_level.push(relation_box);
            edges.push(LayeredRelationEdge::new("n0", relation_box.id(), 0, 0));
        }
        let groups = vec![vec![&boxes[0]], second_level];
        let policy = AsciiResourcePolicy::default();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        let ledger = ResourceContext::new(policy);
        let mut resources = ledger.controlled(control, OperationPhase::Layout);
        let mut checkpoints = RelationResourceCheckpointCursor::new();

        let error = apply_layered_relation_sweep(
            groups,
            LayeredRelationSweep::Downward,
            &edges,
            &levels,
            &mut resources,
            &mut checkpoints,
        )
        .expect_err("layered sweep should observe scheduled cancellation");

        assert!(matches!(
            error,
            LayeredRelationPlanningError::Resource(AsciiError::Cancelled(cancelled))
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
    }
}
