use super::super::{RelationGraphBox, find_box, find_box_ref};
use super::lanes::parallel_lane_margin;
use crate::AsciiError;
use crate::canvas::Canvas;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayeredRelationEdge {
    from_id: String,
    to_id: String,
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
            label_width,
            label_line_count,
        }
    }

    pub(crate) fn source_id(&self) -> &str {
        self.from_id.as_str()
    }

    pub(crate) fn target_id(&self) -> &str {
        self.to_id.as_str()
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

pub(crate) fn relation_components<'a>(
    boxes: &'a [RelationGraphBox],
    edges: &[LayeredRelationEdge],
    resources: &mut ResourceContext,
) -> std::result::Result<Vec<RelationGraphComponent<'a>>, LayeredRelationPlanningError> {
    charge_work_product(resources, edges.len(), boxes.len().max(1))?;
    let mut incident_ids = HashSet::new();
    incident_ids
        .try_reserve(
            edges
                .len()
                .checked_mul(2)
                .ok_or_else(|| work_overflow(resources))?,
        )
        .map_err(|_| layout_allocation_failed())?;
    let mut neighbors = HashMap::<&str, Vec<&str>>::new();
    neighbors
        .try_reserve(
            edges
                .len()
                .checked_mul(2)
                .ok_or_else(|| work_overflow(resources))?,
        )
        .map_err(|_| layout_allocation_failed())?;
    for edge in edges {
        if find_box(boxes, edge.source_id()).is_none()
            || find_box(boxes, edge.target_id()).is_none()
        {
            return Err(LayeredRelationError::MissingEndpoint.into());
        }

        incident_ids.insert(edge.source_id());
        incident_ids.insert(edge.target_id());
        try_push_adjacency(&mut neighbors, edge.source_id(), edge.target_id())?;
        try_push_adjacency(&mut neighbors, edge.target_id(), edge.source_id())?;
    }

    let mut components = Vec::new();
    components
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut visited = HashSet::new();
    visited
        .try_reserve(boxes.len())
        .map_err(|_| layout_allocation_failed())?;

    for edge in edges {
        let start_id = edge.source_id();
        if visited.contains(start_id) {
            continue;
        }

        let mut component_ids = HashSet::new();
        component_ids
            .try_reserve(boxes.len())
            .map_err(|_| layout_allocation_failed())?;
        let mut queue = VecDeque::new();
        queue
            .try_reserve(boxes.len())
            .map_err(|_| layout_allocation_failed())?;
        visited.insert(start_id);
        component_ids.insert(start_id);
        queue.push_back(start_id);

        while let Some(id) = queue.pop_front() {
            resources.charge_layout_work(1)?;
            for neighbor_id in neighbors.get(id).into_iter().flatten() {
                resources.charge_layout_work(1)?;
                if visited.insert(*neighbor_id) {
                    component_ids.insert(*neighbor_id);
                    queue.push_back(*neighbor_id);
                }
            }
        }

        let mut component_boxes = Vec::new();
        component_boxes
            .try_reserve_exact(component_ids.len())
            .map_err(|_| layout_allocation_failed())?;
        component_boxes.extend(
            boxes
                .iter()
                .filter(|relation_box| component_ids.contains(relation_box.id())),
        );
        let mut component_edge_indices = Vec::new();
        component_edge_indices
            .try_reserve_exact(edges.len())
            .map_err(|_| layout_allocation_failed())?;
        component_edge_indices.extend(edges.iter().enumerate().filter_map(|(index, edge)| {
            (component_ids.contains(edge.source_id()) && component_ids.contains(edge.target_id()))
                .then_some(index)
        }));

        components.push(RelationGraphComponent {
            boxes: component_boxes,
            edge_indices: component_edge_indices,
        });
    }

    components.extend(
        boxes
            .iter()
            .filter(|relation_box| !incident_ids.contains(relation_box.id()))
            .map(|relation_box| RelationGraphComponent {
                boxes: vec![relation_box],
                edge_indices: Vec::new(),
            }),
    );

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
    charge_work_product(resources, edges.len(), boxes.len().max(1))?;
    let mut incident = HashSet::new();
    incident
        .try_reserve(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut outgoing = HashMap::<&str, Vec<&str>>::new();
    outgoing
        .try_reserve(boxes.len())
        .map_err(|_| layout_allocation_failed())?;

    for edge in edges {
        if find_box_ref(boxes, edge.source_id()).is_none()
            || find_box_ref(boxes, edge.target_id()).is_none()
        {
            return Err(LayeredRelationError::MissingEndpoint.into());
        }

        incident.insert(edge.source_id().to_string());
        incident.insert(edge.target_id().to_string());
        try_push_adjacency(&mut outgoing, edge.source_id(), edge.target_id())?;
    }

    if incident.len() != boxes.len() {
        return Err(LayeredRelationError::UnrelatedBoxes.into());
    }

    let mut levels = HashMap::<String, usize>::new();
    levels
        .try_reserve(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut queue = VecDeque::new();
    queue
        .try_reserve(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    for relation_box in boxes {
        let id = relation_box.id().to_string();
        levels.insert(id.clone(), 0);
        queue.push_back(id);
    }

    let level_cap = boxes.len().saturating_sub(1);
    while let Some(id) = queue.pop_front() {
        resources.charge_layout_work(1)?;
        let current_level = levels.get(&id).copied().unwrap_or(0);
        let Some(children) = outgoing.get(id.as_str()) else {
            continue;
        };
        for &child_id in children {
            resources.charge_layout_work(1)?;
            let next_level = current_level
                .checked_add(1)
                .ok_or_else(|| nesting_overflow(resources))?;
            resources.check_nesting_depth(next_level)?;
            if next_level > level_cap {
                continue;
            }
            let should_update = match levels.get(child_id) {
                Some(existing_level) => *existing_level < next_level,
                None => true,
            };
            if should_update {
                levels.insert(child_id.to_string(), next_level);
                queue.push_back(child_id.to_string());
            }
        }
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
    let base = initial_layered_groups(boxes, levels, resources)?;
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
        &base, edges, levels, &mut seen, &mut best, resources,
    )? {
        return Ok((level_groups, LayeredRelationLayoutKind::Standard));
    }

    for first_sweep in [LayeredRelationSweep::Downward, LayeredRelationSweep::Upward] {
        let mut groups = try_clone_level_groups(&base)?;
        for index in 0..max_sweeps {
            groups = apply_layered_relation_sweep(
                groups,
                first_sweep.alternating(index),
                edges,
                levels,
                resources,
            )?;
            if let Some(level_groups) = score_ordered_layered_group_candidate(
                &groups, edges, levels, &mut seen, &mut best, resources,
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
        if find_box_ref(boxes, edge.source_id()).is_none()
            || find_box_ref(boxes, edge.target_id()).is_none()
        {
            return Err(LayeredRelationError::MissingEndpoint.into());
        }
        if edge.source_id() == edge.target_id()
            || !pairs.insert(ordered_endpoint_pair(edge.source_id(), edge.target_id()))
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
        if edge.source_id() == first.id() {
            neighbors.push(edge.target_id());
        } else if edge.target_id() == first.id() {
            neighbors.push(edge.source_id());
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
) -> Result<Option<Vec<Vec<&'a RelationGraphBox>>>, LayeredRelationPlanningError> {
    let node_count = candidate.iter().try_fold(0usize, |total, group| {
        total
            .checked_add(group.len())
            .ok_or_else(|| work_overflow(resources))
    })?;
    resources.charge_layout_work(node_count.max(1))?;
    if !seen.insert(layered_group_candidate_key(candidate)?) {
        return Ok(None);
    }

    let crossings = crossing_layered_relation_count(edges, levels, candidate, resources)?;
    if crossings == 0 {
        return Ok(Some(try_clone_level_groups(candidate)?));
    }

    let should_replace = best
        .as_ref()
        .is_none_or(|(best_crossings, _)| crossings < *best_crossings);
    if should_replace {
        *best = Some((crossings, try_clone_level_groups(candidate)?));
    }

    Ok(None)
}

fn layered_group_candidate_key<'a>(
    level_groups: &[Vec<&'a RelationGraphBox>],
) -> Result<Vec<Vec<&'a str>>, LayeredRelationPlanningError> {
    let mut key = Vec::new();
    key.try_reserve_exact(level_groups.len())
        .map_err(|_| layout_allocation_failed())?;
    for group in level_groups {
        let mut ids = Vec::new();
        ids.try_reserve_exact(group.len())
            .map_err(|_| layout_allocation_failed())?;
        ids.extend(group.iter().map(|relation_box| relation_box.id()));
        key.push(ids);
    }
    Ok(key)
}

fn try_clone_level_groups<'a>(
    level_groups: &[Vec<&'a RelationGraphBox>],
) -> Result<Vec<Vec<&'a RelationGraphBox>>, LayeredRelationPlanningError> {
    let mut cloned = Vec::new();
    cloned
        .try_reserve_exact(level_groups.len())
        .map_err(|_| layout_allocation_failed())?;
    for group in level_groups {
        let mut cloned_group = Vec::new();
        cloned_group
            .try_reserve_exact(group.len())
            .map_err(|_| layout_allocation_failed())?;
        cloned_group.extend(group.iter().copied());
        cloned.push(cloned_group);
    }
    Ok(cloned)
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
) -> Result<Vec<Vec<&'a RelationGraphBox>>, LayeredRelationPlanningError> {
    let node_count = level_groups.iter().try_fold(0usize, |total, group| {
        total
            .checked_add(group.len())
            .ok_or_else(|| work_overflow(resources))
    })?;
    charge_work_product(resources, node_count.max(1), edges.len().max(1))?;
    match sweep {
        LayeredRelationSweep::Downward => {
            order_layered_groups_downward(level_groups, edges, levels, resources)
        }
        LayeredRelationSweep::Upward => {
            order_layered_groups_upward(level_groups, edges, levels, resources)
        }
    }
}

fn crossing_layered_relation_count(
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    level_groups: &[Vec<&RelationGraphBox>],
    resources: &mut ResourceContext,
) -> Result<usize, LayeredRelationPlanningError> {
    let pair_count = edges
        .len()
        .checked_mul(edges.len().saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| work_overflow(resources))?;
    resources.charge_layout_work(pair_count.max(1))?;
    let node_count = level_groups.iter().try_fold(0usize, |total, group| {
        total
            .checked_add(group.len())
            .ok_or_else(|| work_overflow(resources))
    })?;
    let mut order_by_id = HashMap::new();
    order_by_id
        .try_reserve(node_count)
        .map_err(|_| layout_allocation_failed())?;
    for group in level_groups {
        for (index, relation_box) in group.iter().enumerate() {
            order_by_id.insert(relation_box.id(), index);
        }
    }

    let mut crossings = 0usize;
    for (left_index, left) in edges.iter().enumerate() {
        let left_from_level = levels.get(left.source_id()).copied().unwrap_or(0);
        let left_to_level = levels.get(left.target_id()).copied().unwrap_or(0);
        for right in edges.iter().skip(left_index + 1) {
            if levels.get(right.source_id()).copied().unwrap_or(0) != left_from_level
                || levels.get(right.target_id()).copied().unwrap_or(0) != left_to_level
            {
                continue;
            }

            let left_from_order = order_by_id.get(left.source_id()).copied().unwrap_or(0);
            let left_to_order = order_by_id.get(left.target_id()).copied().unwrap_or(0);
            let right_from_order = order_by_id.get(right.source_id()).copied().unwrap_or(0);
            let right_to_order = order_by_id.get(right.target_id()).copied().unwrap_or(0);

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
) -> Result<Vec<Vec<&'a RelationGraphBox>>, LayeredRelationPlanningError> {
    let max_level = levels.values().copied().max().unwrap_or(0);
    let level_count = max_level
        .checked_add(1)
        .ok_or_else(|| nesting_overflow(resources))?;
    resources.check_nesting_depth(max_level)?;
    let mut level_groups = Vec::new();
    level_groups
        .try_reserve_exact(level_count)
        .map_err(|_| layout_allocation_failed())?;
    let mut level_sizes = Vec::<usize>::new();
    level_sizes
        .try_reserve_exact(level_count)
        .map_err(|_| layout_allocation_failed())?;
    level_sizes.resize(level_count, 0);
    for relation_box in boxes {
        if let Some(level) = levels.get(relation_box.id()).copied() {
            level_sizes[level] = level_sizes[level]
                .checked_add(1)
                .ok_or_else(|| work_overflow(resources))?;
        }
    }
    for level_size in level_sizes {
        let mut group = Vec::new();
        group
            .try_reserve_exact(level_size)
            .map_err(|_| layout_allocation_failed())?;
        level_groups.push(group);
    }
    for relation_box in boxes {
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
) -> Result<Vec<Vec<&'a RelationGraphBox>>, LayeredRelationPlanningError> {
    let max_level = level_groups.len().saturating_sub(1);
    for level in 1..=max_level {
        let (previous_levels, current_levels) = level_groups.split_at_mut(level);
        let previous_group = &previous_levels[level - 1];
        let current_group = &mut current_levels[0];
        let mut previous_order = HashMap::new();
        previous_order
            .try_reserve(previous_group.len())
            .map_err(|_| layout_allocation_failed())?;
        for (index, relation_box) in previous_group.iter().enumerate() {
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
            original_order.insert(relation_box.id(), index);
            let mut neighbor_orders = Vec::new();
            neighbor_orders
                .try_reserve_exact(edges.len())
                .map_err(|_| layout_allocation_failed())?;
            for edge in edges.iter().filter(|edge| {
                edge.target_id() == relation_box.id()
                    && levels.get(edge.source_id()).copied() == Some(level - 1)
            }) {
                if let Some(order) = previous_order.get(edge.source_id()).copied() {
                    neighbor_orders.push(order);
                }
            }
            neighbor_order.insert(
                relation_box.id(),
                barycenter_order(&neighbor_orders, resources)?,
            );
        }

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
    }

    Ok(level_groups)
}

fn order_layered_groups_upward<'a>(
    mut level_groups: Vec<Vec<&'a RelationGraphBox>>,
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    resources: &ResourceContext,
) -> Result<Vec<Vec<&'a RelationGraphBox>>, LayeredRelationPlanningError> {
    let max_level = level_groups.len().saturating_sub(1);
    for level in (0..max_level).rev() {
        let (through_current, next_levels) = level_groups.split_at_mut(level + 1);
        let current_group = &mut through_current[level];
        let next_group = &next_levels[0];
        let mut next_order = HashMap::new();
        next_order
            .try_reserve(next_group.len())
            .map_err(|_| layout_allocation_failed())?;
        for (index, relation_box) in next_group.iter().enumerate() {
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
            original_order.insert(relation_box.id(), index);
            let mut neighbor_orders = Vec::new();
            neighbor_orders
                .try_reserve_exact(edges.len())
                .map_err(|_| layout_allocation_failed())?;
            for edge in edges.iter().filter(|edge| {
                edge.source_id() == relation_box.id()
                    && levels.get(edge.target_id()).copied() == Some(level + 1)
            }) {
                if let Some(order) = next_order.get(edge.target_id()).copied() {
                    neighbor_orders.push(order);
                }
            }
            neighbor_order.insert(
                relation_box.id(),
                barycenter_order(&neighbor_orders, resources)?,
            );
        }

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
    }

    Ok(level_groups)
}

fn barycenter_order(
    neighbor_orders: &[usize],
    resources: &ResourceContext,
) -> Result<usize, LayeredRelationPlanningError> {
    if neighbor_orders.is_empty() {
        return Ok(usize::MAX);
    }

    let sum = neighbor_orders.iter().try_fold(0usize, |total, order| {
        total
            .checked_add(*order)
            .ok_or_else(|| work_overflow(resources))
    })?;
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
            .map(|edge| (edge.source_id(), edge.target_id())),
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
                Ok(resources.checked_grid_add(
                    height,
                    layered_relation_gap_height(edges, levels, level, resources)?,
                )?)
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
            y = resources.checked_grid_add(
                y,
                layered_relation_gap_height(edges, levels, level, resources)?,
            )?;
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
        let from_level = levels.get(edge.source_id()).copied().unwrap_or(0);
        let to_level = levels.get(edge.target_id()).copied().unwrap_or(0);
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

fn layered_relation_gap_height(
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    level: usize,
    resources: &ResourceContext,
) -> Result<usize, LayeredRelationPlanningError> {
    let max_label_lines = edges
        .iter()
        .filter(|edge| relation_edge_crosses_level_gap(edge, levels, level))
        .map(|edge| edge.label_line_count)
        .max()
        .unwrap_or(0);
    if max_label_lines > 0 {
        Ok(resources.checked_grid_add(max_label_lines, 3)?)
    } else {
        Ok(3)
    }
}

fn relation_edge_crosses_level_gap(
    edge: &LayeredRelationEdge,
    levels: &HashMap<String, usize>,
    level: usize,
) -> bool {
    let from_level = levels.get(edge.source_id()).copied().unwrap_or(0);
    let to_level = levels.get(edge.target_id()).copied().unwrap_or(0);
    let min_level = from_level.min(to_level);
    let max_level = from_level.max(to_level);

    min_level <= level && level < max_level
}

fn charge_work_product(
    resources: &mut ResourceContext,
    left: usize,
    right: usize,
) -> Result<(), LayeredRelationPlanningError> {
    resources.charge_layout_work_product(left, right)?;
    Ok(())
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

fn try_push_adjacency<'a>(
    adjacency: &mut HashMap<&'a str, Vec<&'a str>>,
    source: &'a str,
    target: &'a str,
) -> std::result::Result<(), LayeredRelationPlanningError> {
    let neighbors = adjacency.entry(source).or_default();
    neighbors
        .try_reserve(1)
        .map_err(|_| layout_allocation_failed())?;
    neighbors.push(target);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiResourcePolicy;

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

        let first = score_ordered_layered_group_candidate(
            &candidate,
            &edges,
            &levels,
            &mut seen,
            &mut best,
            &mut resources,
        )
        .expect("candidate scoring should fit");
        let duplicate = score_ordered_layered_group_candidate(
            &candidate,
            &edges,
            &levels,
            &mut seen,
            &mut best,
            &mut resources,
        )
        .expect("duplicate scoring should fit");

        assert!(first.is_some());
        assert!(duplicate.is_none());
        assert_eq!(seen.len(), 1);
    }
}
