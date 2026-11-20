//! ER-owned containment and endpoint admission over the shared opaque-box renderer.

use super::{
    ER_LEVEL_HORIZONTAL_GAP, ErCharset, ErRelationComponentAdapter, ErRelationLayout,
    ErRenderContext, er_relationship_summary_row, grid_overflow, layout_allocation_failed,
    prepare_er_relation_labels, render_entity_box, render_er_document_lines,
    render_horizontal_er_component_lines,
};
use crate::color::AsciiColorRole;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::relation_graph::{
    self, RelationDirection, RelationGraphBox, RelationGraphBoxStyle, RelationGraphLabel,
    RelationGraphLabelPlan, RelationGraphLine,
};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::DeferredTextRegistry;
use crate::{AsciiError, Result};
use merman_model::families::er::{ErDiagramModel, ErEntity, ErSubgraph};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ErEndpointKind {
    Entity,
    Group,
}

impl ErEndpointKind {
    pub(super) const fn summary_prefix(self) -> &'static str {
        match self {
            Self::Entity => "entity(bytes=",
            Self::Group => "group(bytes=",
        }
    }

    const fn terminal_prefix(self, source: bool) -> &'static str {
        match (self, source) {
            (Self::Entity, true) => "source-entity(bytes=",
            (Self::Entity, false) => "target-entity(bytes=",
            (Self::Group, true) => "source-group(bytes=",
            (Self::Group, false) => "target-group(bytes=",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endpoint {
    Entity(usize),
    Group(usize),
}

impl Endpoint {
    const fn kind(self) -> ErEndpointKind {
        match self {
            Self::Entity(_) => ErEndpointKind::Entity,
            Self::Group(_) => ErEndpointKind::Group,
        }
    }
}

struct Scope {
    nodes: Vec<Endpoint>,
    relations: Vec<usize>,
    direction: RelationDirection,
}

struct GroupPlan<'a> {
    model: &'a ErDiagramModel,
    entities: Vec<(&'a str, &'a ErEntity)>,
    entity_parents: Vec<Option<usize>>,
    group_parents: Vec<Option<usize>>,
    scopes: Vec<Scope>,
    postorder: Vec<usize>,
    relation_endpoints: Vec<[Endpoint; 2]>,
}

impl<'a> GroupPlan<'a> {
    fn new(
        model: &'a ErDiagramModel,
        direction: RelationDirection,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let count = model.subgraphs.len();
        let scope_count = resources.checked_work_add(count, 1)?;
        resources.charge_layout_work(resources.checked_work_add(
            resources.checked_work_mul(scope_count, 6)?,
            resources.checked_work_mul(model.entities.len(), 3)?,
        )?)?;
        let mut groups = HashMap::new();
        groups
            .try_reserve(count)
            .map_err(|_| layout_allocation_failed())?;
        let mut scopes = Vec::new();
        scopes
            .try_reserve_exact(scope_count)
            .map_err(|_| layout_allocation_failed())?;
        for (index, group) in model.subgraphs.iter().enumerate() {
            resources.checkpoint()?;
            if group.id.is_empty() {
                return Err(unsupported("empty ER group ids"));
            }
            if groups.insert(group.id.as_str(), index).is_some() {
                return Err(unsupported("duplicate ER group ids"));
            }
            // Mermaid leaves this unspecified. ASCII uses TB for an unspecified local scope;
            // the containing scope still controls placement of the resulting group box.
            let direction = group
                .dir
                .as_deref()
                .map_or(Ok(RelationDirection::TopDown), |raw| {
                    RelationDirection::try_from_model(raw, "er", "unknown ER group directions")
                })?;
            let mut nodes = Vec::new();
            nodes
                .try_reserve_exact(group.nodes.len())
                .map_err(|_| layout_allocation_failed())?;
            scopes.push(Scope {
                nodes,
                relations: Vec::new(),
                direction,
            });
        }
        scopes.push(Scope {
            nodes: Vec::new(),
            relations: Vec::new(),
            direction,
        });

        let capacity = resources.checked_work_add(count, model.entities.len())?;
        let mut members = HashMap::new();
        let mut routes = HashMap::new();
        members
            .try_reserve(capacity)
            .map_err(|_| layout_allocation_failed())?;
        routes
            .try_reserve(capacity)
            .map_err(|_| layout_allocation_failed())?;
        for (id, index) in &groups {
            resources.checkpoint()?;
            members.insert(*id, Endpoint::Group(*index));
            routes.insert(*id, Endpoint::Group(*index));
        }
        let mut entities = Vec::new();
        entities
            .try_reserve_exact(model.entities.len())
            .map_err(|_| layout_allocation_failed())?;
        let mut shadows = HashSet::new();
        shadows
            .try_reserve(model.entities.len())
            .map_err(|_| layout_allocation_failed())?;
        for (authored, entity) in &model.entities {
            resources.checkpoint()?;
            // Mermaid suppresses by authored map key, not the entity's generated route ID.
            if groups.contains_key(authored.as_str()) {
                shadows.insert(entity.id.as_str());
                continue;
            }
            let endpoint = Endpoint::Entity(entities.len());
            if let Some(previous) = routes.insert(entity.id.as_str(), endpoint) {
                return Err(unsupported(match previous {
                    Endpoint::Entity(_) => "duplicate rendered ER entity ids",
                    Endpoint::Group(_) => "ambiguous rendered ER endpoint ids",
                }));
            }
            members.insert(authored.as_str(), endpoint);
            entities.push((authored.as_str(), entity));
        }
        for id in &shadows {
            resources.checkpoint()?;
            if routes.contains_key(id) {
                return Err(unsupported("ambiguous rendered ER endpoint ids"));
            }
        }
        let mut plan = Self {
            model,
            entity_parents: filled(entities.len(), None)?,
            entities,
            group_parents: filled(count, None)?,
            scopes,
            postorder: Vec::new(),
            relation_endpoints: Vec::new(),
        };
        for (index, group) in model.subgraphs.iter().enumerate() {
            for member in &group.nodes {
                resources.charge_layout_work(member.len().max(1))?;
                let endpoint = *members
                    .get(member.as_str())
                    .ok_or_else(|| unsupported("ER groups with missing members"))?;
                let parent = match endpoint {
                    Endpoint::Entity(index) => &mut plan.entity_parents[index],
                    Endpoint::Group(index) => &mut plan.group_parents[index],
                };
                if let Some(previous) = parent.replace(index) {
                    return Err(unsupported(if previous == index {
                        "duplicate ER group members"
                    } else {
                        "ER members with multiple group owners"
                    }));
                }
                plan.scopes[index].nodes.push(endpoint);
            }
        }
        plan.scopes[count]
            .nodes
            .try_reserve_exact(capacity)
            .map_err(|_| layout_allocation_failed())?;
        for (index, parent) in plan.group_parents.iter().enumerate() {
            resources.checkpoint()?;
            if parent.is_none() {
                plan.scopes[count].nodes.push(Endpoint::Group(index));
            }
        }
        for (index, parent) in plan.entity_parents.iter().enumerate() {
            resources.checkpoint()?;
            if parent.is_none() {
                plan.scopes[count].nodes.push(Endpoint::Entity(index));
            }
        }
        // Leaf elimination gives a bounded iterative postorder, including disconnected cycles.
        let mut remaining = filled(count, 0usize)?;
        for parent in plan.group_parents.iter().flatten() {
            resources.checkpoint()?;
            remaining[*parent] += 1;
        }
        let mut ready = Vec::new();
        ready
            .try_reserve_exact(count)
            .map_err(|_| layout_allocation_failed())?;
        plan.postorder
            .try_reserve_exact(count)
            .map_err(|_| layout_allocation_failed())?;
        for (index, children) in remaining.iter().enumerate() {
            resources.checkpoint()?;
            if *children == 0 {
                ready.push(index);
            }
        }
        while let Some(index) = ready.pop() {
            resources.charge_layout_work(1)?;
            plan.postorder.push(index);
            if let Some(parent) = plan.group_parents[index] {
                remaining[parent] -= 1;
                if remaining[parent] == 0 {
                    ready.push(parent);
                }
            }
        }
        if plan.postorder.len() != count {
            return Err(unsupported("cyclic ER group membership"));
        }
        let mut depths = filled(scope_count, 0usize)?;
        for index in plan.postorder.iter().rev().copied() {
            let depth = resources.checked_work_add(depths[plan.parent_scope(index)], 1)?;
            resources.check(AsciiResourceLimitId::MaxNestingDepth, depth)?;
            depths[index] = depth;
        }
        plan.relation_endpoints
            .try_reserve_exact(model.relationships.len())
            .map_err(|_| layout_allocation_failed())?;
        for (index, relationship) in model.relationships.iter().enumerate() {
            let mut endpoints = [Endpoint::Group(count); 2];
            for (slot, id) in endpoints.iter_mut().zip([
                relationship.entity_a.as_str(),
                relationship.entity_b.as_str(),
            ]) {
                resources.charge_layout_work(id.len().max(1))?;
                *slot = *routes.get(id).ok_or_else(|| {
                    unsupported(if shadows.contains(id) {
                        "relationships with shadowed ER entity endpoints"
                    } else {
                        "relationships with missing ER endpoints"
                    })
                })?;
            }
            let mut left = plan.owner(endpoints[0]);
            let mut right = plan.owner(endpoints[1]);
            while left != right {
                resources.charge_layout_work(1)?;
                if depths[left] >= depths[right] {
                    left = plan.parent_scope(left);
                } else {
                    right = plan.parent_scope(right);
                }
            }
            plan.scopes[left]
                .relations
                .try_reserve(1)
                .map_err(|_| layout_allocation_failed())?;
            plan.scopes[left].relations.push(index);
            plan.relation_endpoints.push(endpoints);
        }
        Ok(plan)
    }

    fn root(&self) -> usize {
        self.model.subgraphs.len()
    }

    fn parent_scope(&self, group: usize) -> usize {
        self.group_parents[group].unwrap_or(self.root())
    }

    fn owner(&self, endpoint: Endpoint) -> usize {
        match endpoint {
            Endpoint::Entity(index) => self.entity_parents[index].unwrap_or(self.root()),
            Endpoint::Group(index) => self.parent_scope(index),
        }
    }

    fn identity(&self, endpoint: Endpoint) -> &'a str {
        match endpoint {
            Endpoint::Entity(index) => self.entities[index].0,
            Endpoint::Group(index) => self.model.subgraphs[index].id.as_str(),
        }
    }

    fn route_id(&self, endpoint: Endpoint) -> &'a str {
        match endpoint {
            Endpoint::Entity(index) => self.entities[index].1.id.as_str(),
            Endpoint::Group(_) => self.identity(endpoint),
        }
    }

    fn route_in_scope(
        &self,
        mut endpoint: Endpoint,
        scope: usize,
        resources: &ResourceContext,
    ) -> Result<Endpoint> {
        loop {
            resources.charge_layout_work(1)?;
            let owner = self.owner(endpoint);
            if owner == scope {
                return Ok(endpoint);
            }
            if owner == self.root() {
                return Err(unsupported("inconsistent ER group relation scopes"));
            }
            endpoint = Endpoint::Group(owner);
        }
    }
}

pub(super) fn render_grouped_er_diagram(
    model: &ErDiagramModel,
    options: &AsciiRenderOptions,
    direction: RelationDirection,
    resources: ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let plan = GroupPlan::new(model, direction, &resources)?;
    let mut resources =
        execution.resource_context(&resources, merman_model::OperationPhase::Layout);
    let mut deferred = DeferredTextRegistry::new();
    let labels =
        prepare_er_relation_labels(&model.relationships, options, &mut deferred, &resources)?;
    let renderer = GroupRenderer {
        plan,
        options,
        labels,
        charset: ErCharset::for_options(options),
        execution,
    };
    let mut rendered = filled(model.subgraphs.len(), None)?;
    for index in renderer.plan.postorder.iter().copied() {
        resources.checkpoint()?;
        let contents = renderer.scope_lines(index, &mut rendered, &mut deferred, &mut resources)?;
        rendered[index] = Some(render_group_frame(
            &model.subgraphs[index],
            contents,
            options,
            renderer.charset,
            &mut deferred,
            &mut resources,
        )?);
    }
    let lines = renderer.scope_lines(
        renderer.plan.root(),
        &mut rendered,
        &mut deferred,
        &mut resources,
    )?;
    render_er_document_lines(lines, options, &mut resources, &deferred, execution)
}

struct GroupRenderer<'model, 'render> {
    plan: GroupPlan<'model>,
    options: &'render AsciiRenderOptions,
    labels: Vec<Option<RelationGraphLabel>>,
    charset: ErCharset,
    execution: AsciiExecution<'render>,
}

impl<'model> GroupRenderer<'model, '_> {
    fn scope_lines(
        &self,
        scope: usize,
        rendered: &mut [Option<RelationGraphBox>],
        deferred: &mut DeferredTextRegistry<'model>,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>> {
        let scope_plan = &self.plan.scopes[scope];
        let mut boxes = Vec::new();
        boxes
            .try_reserve_exact(scope_plan.nodes.len())
            .map_err(|_| layout_allocation_failed())?;
        for endpoint in &scope_plan.nodes {
            resources.charge_layout_work(1)?;
            boxes.push(match *endpoint {
                Endpoint::Entity(index) => {
                    let (authored, entity) = self.plan.entities[index];
                    render_entity_box(
                        authored,
                        entity,
                        self.options,
                        self.charset,
                        deferred,
                        resources,
                    )?
                }
                Endpoint::Group(index) => rendered[index]
                    .take()
                    .ok_or_else(|| unsupported("inconsistent ER group rendering order"))?,
            });
        }
        let mut layouts = Vec::new();
        let mut summaries = Vec::new();
        layouts
            .try_reserve_exact(scope_plan.relations.len())
            .map_err(|_| layout_allocation_failed())?;
        summaries
            .try_reserve_exact(scope_plan.relations.len())
            .map_err(|_| layout_allocation_failed())?;
        for index in scope_plan.relations.iter().copied() {
            let endpoints = self.plan.relation_endpoints[index];
            let routes = [
                self.plan.route_in_scope(endpoints[0], scope, resources)?,
                self.plan.route_in_scope(endpoints[1], scope, resources)?,
            ];
            let relationship = &self.plan.model.relationships[index];
            let mut layout = ErRelationLayout {
                transform: scope_plan.direction.transform(),
                relationship,
                top_id: self.plan.route_id(routes[0]),
                bottom_id: self.plan.route_id(routes[1]),
                top_identity: self.plan.identity(endpoints[0]),
                bottom_identity: self.plan.identity(endpoints[1]),
                endpoint_kinds: Some([endpoints[0].kind(), endpoints[1].kind()]),
                top_cardinality: relationship.rel_spec.card_b.as_str(),
                bottom_cardinality: relationship.rel_spec.card_a.as_str(),
                label: self.labels[index].clone(),
            };
            if routes[0] == routes[1] && endpoints[0] != endpoints[1] {
                // A group and its member are distinct semantic ends, never a synthetic self-loop.
                summaries.push(er_relationship_summary_row(
                    &layout,
                    self.charset,
                    self.options.terminal_width_profile,
                    resources,
                    deferred,
                )?);
            } else {
                self.disclose_facades(&mut layout, endpoints, routes, deferred, resources)?;
                layouts.push(layout);
            }
        }
        let direction = scope_plan.direction;
        let context = ErRenderContext {
            options: self.options,
            charset: self.charset,
            direction,
        };
        let adapter = ErRelationComponentAdapter {
            charset: self.charset,
            width_profile: self.options.terminal_width_profile,
            transform: direction.transform(),
        };
        let mut lines = if layouts.is_empty() {
            if direction.is_horizontal() {
                relation_graph::render_horizontal_box_strip_lines(
                    &boxes,
                    direction.transform(),
                    ER_LEVEL_HORIZONTAL_GAP,
                    self.options.terminal_width_profile,
                    resources,
                )?
            } else {
                relation_graph::stacked_box_lines_ordered(
                    &boxes,
                    self.options.terminal_width_profile,
                    direction.is_reversed(),
                    resources,
                )?
            }
        } else if direction.is_horizontal() {
            render_horizontal_er_component_lines(
                &boxes,
                &layouts,
                &context,
                &adapter,
                resources,
                deferred,
                self.execution,
            )?
        } else {
            relation_graph::render_relation_component_lines_with_execution(
                &boxes,
                &layouts,
                self.options,
                resources,
                &adapter,
                deferred,
                self.execution,
            )?
        };
        if !summaries.is_empty() {
            let rows = relation_graph::relation_summary_lines_for_rows(
                &summaries,
                None,
                self.options,
                resources,
            )?;
            let additional = resources.checked_work_add(rows.len(), 2)?;
            lines
                .try_reserve(additional)
                .map_err(|_| layout_allocation_failed())?;
            lines.push(RelationGraphLine::try_blank(
                0,
                self.options.terminal_width_profile,
                resources,
            )?);
            lines.push(RelationGraphLine::try_with_role(
                "relations:",
                AsciiColorRole::EdgeLabel,
                self.options.terminal_width_profile,
                resources,
            )?);
            lines.extend(rows);
        }
        Ok(lines)
    }

    fn disclose_facades(
        &self,
        layout: &mut ErRelationLayout<'model>,
        endpoints: [Endpoint; 2],
        routes: [Endpoint; 2],
        deferred: &mut DeferredTextRegistry<'model>,
        resources: &ResourceContext,
    ) -> Result<()> {
        if endpoints == routes {
            return Ok(());
        }
        let mut lines = Vec::new();
        lines
            .try_reserve_exact(5)
            .map_err(|_| layout_allocation_failed())?;
        if !layout.relationship.role_a.is_empty() {
            // Quote the authored role so it cannot impersonate the terminal disclosure records.
            lines.push(deferred.try_register_framed_value(
                "role(bytes=",
                &layout.relationship.role_a,
                self.options.terminal_width_profile,
                resources,
            )?);
        }
        for index in 0..2 {
            if endpoints[index] == routes[index] {
                continue;
            }
            lines.push(deferred.try_register_framed_value(
                endpoints[index].kind().terminal_prefix(index == 0),
                self.plan.identity(endpoints[index]),
                self.options.terminal_width_profile,
                resources,
            )?);
            lines.push(deferred.try_register_framed_value(
                if index == 0 {
                    "source-via-group(bytes="
                } else {
                    "target-via-group(bytes="
                },
                self.plan.identity(routes[index]),
                self.options.terminal_width_profile,
                resources,
            )?);
        }
        layout.label = Some(RelationGraphLabel::try_from_lines(
            lines,
            self.options.terminal_width_profile,
            resources,
        )?);
        Ok(())
    }
}

fn render_group_frame<'a>(
    group: &'a ErSubgraph,
    contents: Vec<RelationGraphLine>,
    options: &AsciiRenderOptions,
    charset: ErCharset,
    deferred: &mut DeferredTextRegistry<'a>,
    resources: &mut ResourceContext,
) -> Result<RelationGraphBox> {
    let profile = options.terminal_width_profile;
    let title =
        RelationGraphLabelPlan::try_new_present(&group.title, profile, deferred, resources)?
            .materialize(deferred, resources)?;
    let identity =
        deferred.try_register_framed_value("group(bytes=", &group.id, profile, resources)?;
    let inner_width = contents
        .iter()
        .map(RelationGraphLine::width)
        .chain([title.width(), identity.width()])
        .max()
        .unwrap_or(0);
    let padding = options.box_border_padding;
    let content_width =
        resources.checked_grid_add(inner_width, resources.checked_grid_mul(padding, 2)?)?;
    let width = resources.checked_grid_add(content_width, 2)?;
    let height = resources.checked_grid_add(
        resources.checked_grid_add(contents.len().max(1), title.line_count())?,
        4,
    )?;
    resources.charge_layout_work(resources.grid_extent(width, height)?.cells())?;
    let style = RelationGraphBoxStyle {
        top_left: charset.top_left,
        top_right: charset.top_right,
        bottom_left: charset.bottom_left,
        bottom_right: charset.bottom_right,
        horizontal: charset.horizontal,
        vertical: charset.vertical,
        separator_left: charset.separator_left,
        separator_right: charset.separator_right,
        border_role: AsciiColorRole::GroupBorder,
        text_role: AsciiColorRole::Text,
    };
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(height)
        .map_err(|_| layout_allocation_failed())?;
    lines.push(RelationGraphLine::try_box_border(
        style.top_left,
        style.top_right,
        style.horizontal,
        content_width,
        style.border_role,
        profile,
        resources,
    )?);
    for header in title.lines().iter().chain(std::iter::once(&identity)) {
        resources.checkpoint()?;
        lines.push(RelationGraphLine::deferred_box_content(
            header,
            content_width,
            padding,
            style,
            profile,
            resources,
        )?);
    }
    lines.push(RelationGraphLine::try_box_border(
        style.separator_left,
        style.separator_right,
        style.horizontal,
        content_width,
        style.border_role,
        profile,
        resources,
    )?);
    if contents.is_empty() {
        lines.push(RelationGraphLine::try_box_border(
            style.vertical,
            style.vertical,
            ' ',
            content_width,
            style.border_role,
            profile,
            resources,
        )?);
    }
    for content in contents {
        resources.checkpoint()?;
        let trailing = content_width
            .checked_sub(resources.checked_grid_add(padding, content.width())?)
            .ok_or_else(|| grid_overflow(resources))?;
        let mut parts = Vec::new();
        parts
            .try_reserve_exact(5)
            .map_err(|_| layout_allocation_failed())?;
        parts.push(RelationGraphLine::try_role_char(
            style.vertical,
            style.border_role,
            profile,
            resources,
        )?);
        parts.push(RelationGraphLine::try_blank(padding, profile, resources)?);
        parts.push(content);
        parts.push(RelationGraphLine::try_blank(trailing, profile, resources)?);
        parts.push(RelationGraphLine::try_role_char(
            style.vertical,
            style.border_role,
            profile,
            resources,
        )?);
        lines.push(relation_graph::try_concat_relation_lines(
            parts, profile, resources,
        )?);
    }
    lines.push(RelationGraphLine::try_box_border(
        style.bottom_left,
        style.bottom_right,
        style.horizontal,
        content_width,
        style.border_role,
        profile,
        resources,
    )?);
    Ok(RelationGraphBox::new_with_lines(
        group.id.clone(),
        lines,
        width,
        profile,
    ))
}

fn filled<T: Clone>(length: usize, value: T) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| layout_allocation_failed())?;
    values.resize(length, value);
    Ok(values)
}

fn unsupported(feature: &'static str) -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "er",
        feature,
    }
}
