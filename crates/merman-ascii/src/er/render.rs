use crate::color::AsciiColorRole;
use crate::operation::AsciiExecution;
use crate::options::{AsciiCharset, AsciiRenderOptions, TerminalWidthProfile};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::relation_graph::{
    HorizontalRelationEndpoint, HorizontalRelationMarker, HorizontalRelationStyle,
    LayeredRelationEdge, LayeredRelationError, LayeredRelationRouteStyle, PhysicalPortSide,
    RelationDirection, RelationGraphBoxStyle, RelationGraphLabel, RelationGraphLabelBatchPlan,
    RelationGraphLabelPlan, RelationGraphLine, RelationGraphSummaryRow, RelationLineChars,
    RelationOverlay, RelationParallelPlan, RelationRegionPlan, RelationSelfLoopMetrics,
    RelationStackPlan, RelationSummaryPaintPlan,
};
use crate::resource::{AsciiResourceLimitPhase, LogicalExtent, ResourceContext};
use crate::safe_text::{
    ComposedTextPlan, DeferredTextLine, DeferredTextPart, DeferredTextRegistry, charge_text_layout,
    terminal_char_display_width, terminal_single_line_text_requires_normalization,
    terminal_text_requires_normalization,
};
#[cfg(test)]
use crate::text::StyledLine;
use crate::text::display_width_with_profile;
use crate::{AsciiError, Result};
use merman_core::diagrams::er::{
    ErAttributeRenderModel, ErDiagramRenderModel, ErEntityRenderModel, ErRelationshipRenderModel,
};
use std::collections::{HashMap, hash_map::Entry};

const ER_LEVEL_HORIZONTAL_GAP: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ErCharset {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    vertical: char,
    separator_left: char,
    separator_right: char,
    solid_horizontal_relation: char,
    solid_relation: char,
    dotted_horizontal_relation: char,
    dotted_relation: char,
    relation_junction: char,
    md_parent: &'static str,
}

impl ErCharset {
    fn for_options(options: &AsciiRenderOptions) -> Self {
        match options.structural_charset() {
            AsciiCharset::Ascii => Self {
                top_left: '+',
                top_right: '+',
                bottom_left: '+',
                bottom_right: '+',
                horizontal: '-',
                vertical: '|',
                separator_left: '+',
                separator_right: '+',
                solid_horizontal_relation: '-',
                solid_relation: '|',
                dotted_horizontal_relation: '.',
                dotted_relation: ':',
                relation_junction: '+',
                md_parent: "<>",
            },
            AsciiCharset::Unicode => Self {
                top_left: '┌',
                top_right: '┐',
                bottom_left: '└',
                bottom_right: '┘',
                horizontal: '─',
                vertical: '│',
                separator_left: '├',
                separator_right: '┤',
                solid_horizontal_relation: '─',
                solid_relation: '│',
                dotted_horizontal_relation: '╌',
                dotted_relation: '┆',
                relation_junction: '┼',
                md_parent: "◆",
            },
        }
    }
}

type RenderedEntityBox = RelationGraphBox;

struct ErRelationComponentAdapter {
    charset: ErCharset,
    width_profile: TerminalWidthProfile,
    transform: relation_graph::DirectionTransform,
}

struct ErRenderContext<'render> {
    options: &'render AsciiRenderOptions,
    charset: ErCharset,
    direction: RelationDirection,
}

struct ErRelationLayout<'a> {
    transform: relation_graph::DirectionTransform,
    // Canonical semantic endpoints and authored identity/cardinality. Direction changes only
    // their physical projection; these fields are never swapped for BT or RL.
    relationship: &'a ErRelationshipRenderModel,
    top_id: &'a str,
    bottom_id: &'a str,
    top_identity: &'a str,
    bottom_identity: &'a str,
    top_cardinality: &'a str,
    bottom_cardinality: &'a str,
    label: Option<RelationGraphLabel>,
}

impl<'a> ErRelationLayout<'a> {
    fn physical_ids(&self) -> (&'a str, &'a str) {
        self.transform
            .physical_vertical_pair(self.top_id, self.bottom_id)
    }

    fn physical_cardinalities(&self) -> (&'a str, &'a str) {
        self.transform
            .physical_vertical_pair(self.top_cardinality, self.bottom_cardinality)
    }
}

pub(crate) fn render_er_diagram_with_execution(
    model: &ErDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Semantic)?;
    render_er_diagram_impl(model, options, execution)
}

fn render_er_diagram_impl(
    model: &ErDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let base_resources = ResourceContext::new(*execution.resources());
    let mut resources =
        execution.resource_context(&base_resources, merman_core::OperationPhase::Semantic);
    resources.charge_layout_work(model.direction.len().max(1))?;
    let direction =
        RelationDirection::try_from_model(&model.direction, "er", "unknown ER diagram directions");
    resources.checkpoint()?;
    let direction = direction?;
    if model.entities.is_empty() {
        if !model.relationships.is_empty() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "er",
                feature: "relationships with missing endpoint entities",
            });
        }
        return Ok(String::new());
    }

    execution.checkpoint(merman_core::OperationPhase::Semantic)?;
    preflight_er_text(model, &mut resources)?;
    charge_er_model_work(model, &mut resources)?;
    let authored_entity_identities = index_unique_er_entity_identities(model, &resources)?;
    let charset = ErCharset::for_options(options);
    resources = execution.resource_context(&resources, merman_core::OperationPhase::Layout);
    let mut deferred_text = DeferredTextRegistry::new();
    let mut boxes = Vec::new();
    boxes
        .try_reserve_exact(model.entities.len())
        .map_err(|_| layout_allocation_failed())?;
    for (authored_identity, entity) in &model.entities {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
        boxes.push(render_entity_box(
            authored_identity,
            entity,
            options,
            charset,
            &mut deferred_text,
            &mut resources,
        )?);
    }
    if model.relationships.is_empty() {
        if direction.is_horizontal() {
            let lines = relation_graph::render_horizontal_box_strip_lines(
                &boxes,
                direction.transform(),
                ER_LEVEL_HORIZONTAL_GAP,
                options.terminal_width_profile,
                &resources,
            )?;
            return render_er_document_lines(
                lines,
                options,
                &mut resources,
                &deferred_text,
                execution,
            );
        }
        if direction.is_reversed() {
            let lines = relation_graph::stacked_box_lines_ordered(
                &boxes,
                options.terminal_width_profile,
                true,
                &mut resources,
            )?;
            return render_er_document_lines(
                lines,
                options,
                &mut resources,
                &deferred_text,
                execution,
            );
        }
        return relation_graph::render_stacked_boxes_with_deferred_options_with_execution(
            &boxes,
            options,
            &mut resources,
            &deferred_text,
            execution,
        );
    }

    // Preserve the established relationship-path work receipt at its original layout-phase
    // admission point even though semantic validation now owns and returns the reusable index.
    resources.charge_layout_work(model.entities.len())?;
    let context = ErRenderContext {
        options,
        charset,
        direction,
    };
    let rendered = render_er_components(
        &boxes,
        &model.relationships,
        &authored_entity_identities,
        &context,
        &mut resources,
        &mut deferred_text,
        execution,
    )?;
    execution.checkpoint(merman_core::OperationPhase::Emit)?;
    Ok(rendered)
}

fn index_unique_er_entity_identities<'model>(
    model: &'model ErDiagramRenderModel,
    resources: &ResourceContext,
) -> Result<HashMap<&'model str, &'model str>> {
    resources.charge_layout_work(model.entities.len())?;
    let mut identities = HashMap::new();
    identities
        .try_reserve(model.entities.len())
        .map_err(|_| layout_allocation_failed())?;
    for (authored_identity, entity) in &model.entities {
        match identities.entry(entity.id.as_str()) {
            Entry::Vacant(entry) => {
                entry.insert(authored_identity.as_str());
            }
            Entry::Occupied(_) => {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "er",
                    feature: "duplicate rendered ER entity ids",
                });
            }
        }
    }
    Ok(identities)
}

fn authored_er_entity_identity<'model>(
    rendered_id: &str,
    identities: &HashMap<&'model str, &'model str>,
) -> Result<&'model str> {
    identities
        .get(rendered_id)
        .copied()
        .ok_or(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "relationships with missing endpoint entities",
        })
}

fn render_er_components<'model>(
    boxes: &[RenderedEntityBox],
    relationships: &'model [ErRelationshipRenderModel],
    authored_entity_identities: &HashMap<&'model str, &'model str>,
    context: &ErRenderContext<'_>,
    resources: &mut ResourceContext,
    deferred_text: &mut DeferredTextRegistry<'model>,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let labels =
        prepare_er_relation_labels(relationships, context.options, deferred_text, resources)?;
    let mut layouts = Vec::new();
    layouts
        .try_reserve_exact(relationships.len())
        .map_err(|_| layout_allocation_failed())?;
    for (relationship, label) in relationships.iter().zip(labels) {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
        let layout = ErRelationLayout {
            transform: context.direction.transform(),
            relationship,
            top_id: relationship.entity_a.as_str(),
            bottom_id: relationship.entity_b.as_str(),
            top_identity: authored_er_entity_identity(
                &relationship.entity_a,
                authored_entity_identities,
            )?,
            bottom_identity: authored_er_entity_identity(
                &relationship.entity_b,
                authored_entity_identities,
            )?,
            top_cardinality: relationship.rel_spec.card_b.as_str(),
            bottom_cardinality: relationship.rel_spec.card_a.as_str(),
            label,
        };
        layouts.push(layout);
    }
    let adapter = ErRelationComponentAdapter {
        charset: context.charset,
        width_profile: context.options.terminal_width_profile,
        transform: context.direction.transform(),
    };
    if context.direction.is_horizontal() {
        let lines = render_horizontal_er_component_lines(
            boxes,
            &layouts,
            context,
            &adapter,
            resources,
            deferred_text,
            execution,
        )?;
        return render_er_document_lines(
            lines,
            context.options,
            resources,
            deferred_text,
            execution,
        );
    }
    relation_graph::render_relation_components_with_deferred_with_execution(
        boxes,
        &layouts,
        context.options,
        resources,
        &adapter,
        deferred_text,
        execution,
    )
}

fn prepare_er_relation_labels<'model>(
    relationships: &'model [ErRelationshipRenderModel],
    options: &AsciiRenderOptions,
    deferred_text: &mut DeferredTextRegistry<'model>,
    resources: &ResourceContext,
) -> Result<Vec<Option<RelationGraphLabel>>> {
    resources.transaction(|resources| {
        let mut plans = Vec::new();
        plans
            .try_reserve_exact(relationships.len())
            .map_err(|_| layout_allocation_failed())?;
        for relationship in relationships {
            plans.push(RelationGraphLabelPlan::try_new(
                &relationship.role_a,
                options.terminal_width_profile,
                deferred_text,
                resources,
            )?);
        }
        RelationGraphLabelBatchPlan::try_new(plans, resources)?.materialize(
            options.color_mode == crate::color::AsciiColorMode::Html,
            deferred_text,
            resources,
        )
    })
}

fn render_entity_box<'a>(
    authored_identity: &'a str,
    entity: &'a ErEntityRenderModel,
    options: &AsciiRenderOptions,
    charset: ErCharset,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &mut ResourceContext,
) -> Result<RenderedEntityBox> {
    let sections = entity_sections(
        authored_identity,
        entity,
        options.terminal_width_profile,
        deferred_text,
        resources,
    )?;
    render_box_sections(entity.id.clone(), sections, options, charset, resources)
}

fn render_box_sections(
    id: String,
    sections: Vec<Vec<DeferredTextLine>>,
    options: &AsciiRenderOptions,
    charset: ErCharset,
    resources: &mut ResourceContext,
) -> Result<RenderedEntityBox> {
    let style = RelationGraphBoxStyle {
        top_left: charset.top_left,
        top_right: charset.top_right,
        bottom_left: charset.bottom_left,
        bottom_right: charset.bottom_right,
        horizontal: charset.horizontal,
        vertical: charset.vertical,
        separator_left: charset.separator_left,
        separator_right: charset.separator_right,
        border_role: AsciiColorRole::NodeBorder,
        text_role: AsciiColorRole::Text,
    };
    relation_graph::RelationGraphBox::from_deferred_sections(
        id,
        &sections,
        options.box_border_padding,
        style,
        options.terminal_width_profile,
        resources,
    )
}

fn entity_sections<'a>(
    authored_identity: &'a str,
    entity: &'a ErEntityRenderModel,
    width_profile: TerminalWidthProfile,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &ResourceContext,
) -> Result<Vec<Vec<DeferredTextLine>>> {
    let mut header = Vec::new();
    let (display_owner, display_label) = entity_display(entity);
    let disclose_display =
        terminal_single_line_text_requires_normalization(display_label, resources)?;
    let disclose_identity = display_label != authored_identity
        || terminal_text_requires_normalization(authored_identity, resources)?;
    header
        .try_reserve_exact(1 + usize::from(disclose_display) + usize::from(disclose_identity))
        .map_err(|_| layout_allocation_failed())?;
    let header_plan = ComposedTextPlan::try_new(resources, 1, |push| push(display_label))?;
    header.push(deferred_text.try_register(header_plan, width_profile, resources)?);
    if disclose_display {
        header.push(deferred_text.try_register_framed_value(
            display_owner,
            display_label,
            width_profile,
            resources,
        )?);
    }
    if disclose_identity {
        header.push(deferred_text.try_register_framed_value(
            "id(bytes=",
            authored_identity,
            width_profile,
            resources,
        )?);
    }
    let mut sections = Vec::new();
    sections
        .try_reserve_exact(2)
        .map_err(|_| layout_allocation_failed())?;
    sections.push(header);

    let mut attributes = Vec::new();
    attributes
        .try_reserve_exact(entity.attributes.len())
        .map_err(|_| layout_allocation_failed())?;
    for attribute in &entity.attributes {
        let line = register_er_attribute_line(attribute, width_profile, deferred_text, resources)?;
        if line.width() > 0 {
            attributes.push(line);
        }
    }
    if !attributes.is_empty() {
        sections.push(attributes);
    }

    Ok(sections)
}

fn entity_display(entity: &ErEntityRenderModel) -> (&'static str, &str) {
    if entity.alias.is_empty() {
        ("label(bytes=", &entity.label)
    } else {
        ("alias(bytes=", &entity.alias)
    }
}

fn entity_display_label(entity: &ErEntityRenderModel) -> &str {
    entity_display(entity).1
}

#[cfg(test)]
fn attribute_text_with_probe(
    attribute: &ErAttributeRenderModel,
    resources: &mut ResourceContext,
    before_materialize: impl FnOnce(),
) -> Result<String> {
    resources.transaction(move |resources| {
        let options = AsciiRenderOptions::ascii();
        let mut deferred = DeferredTextRegistry::new();
        let line = register_er_attribute_line(
            attribute,
            options.terminal_width_profile,
            &mut deferred,
            resources,
        )?;
        let mut styled = StyledLine::with_resources(options.terminal_width_profile, resources);
        styled.try_push_deferred_text(&line, AsciiColorRole::Text)?;
        let lines = [RelationGraphLine::from_styled(styled)];
        let mut emit_resources = resources.clone();
        relation_graph::render_lines_with_deferred_probe(
            &lines,
            &options,
            &mut emit_resources,
            &deferred,
            before_materialize,
        )
    })
}

fn register_er_attribute_line<'a>(
    attribute: &'a ErAttributeRenderModel,
    width_profile: TerminalWidthProfile,
    deferred: &mut DeferredTextRegistry<'a>,
    resources: &ResourceContext,
) -> Result<DeferredTextLine> {
    resources.transaction(|resources| {
        let ty = deferred.try_register_framed_value(
            "type(bytes=",
            &attribute.ty,
            width_profile,
            resources,
        )?;
        let name = deferred.try_register_framed_value(
            " name(bytes=",
            &attribute.name,
            width_profile,
            resources,
        )?;
        let keys = register_er_framed_keys(&attribute.keys, width_profile, deferred, resources)?;
        let comment = deferred.try_register_framed_value(
            " comment(bytes=",
            &attribute.comment,
            width_profile,
            resources,
        )?;
        DeferredTextLine::try_concat(&[&ty, &name, &keys, &comment], resources)
    })
}

fn register_er_framed_keys<'a>(
    keys: &'a [String],
    width_profile: TerminalWidthProfile,
    deferred: &mut DeferredTextRegistry<'a>,
    resources: &ResourceContext,
) -> Result<DeferredTextLine> {
    resources.transaction(|resources| {
        let scratch_work = keys.len().max(1);
        resources.check_usage(scratch_work, 0)?;

        let mut key_lines = Vec::new();
        key_lines
            .try_reserve_exact(keys.len())
            .map_err(|_| layout_allocation_failed())?;
        for key in keys {
            key_lines.push(deferred.try_register(
                ComposedTextPlan::try_new(resources, 1, |push| push(key))?,
                width_profile,
                resources,
            )?);
        }

        let line =
            deferred.try_register_parts(width_profile, resources, keys.len().max(1), |push| {
                push(DeferredTextPart::Static(" keys=["))?;
                for (index, (key, key_line)) in keys.iter().zip(&key_lines).enumerate() {
                    let prefix = if index == 0 { "bytes=" } else { ", bytes=" };
                    push(DeferredTextPart::Static(prefix))?;
                    push(DeferredTextPart::Decimal(key.len()))?;
                    push(DeferredTextPart::Static(" "))?;
                    push(DeferredTextPart::QuotedLine(key_line))?;
                }
                push(DeferredTextPart::Static("]"))
            })?;
        resources.charge_layout_work(scratch_work)?;
        Ok(line)
    })
}

fn render_er_document_lines(
    lines: Vec<RelationGraphLine>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    deferred_text: &DeferredTextRegistry<'_>,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    relation_graph::render_lines_with_deferred_options_with_execution(
        &lines,
        options,
        resources,
        deferred_text,
        execution,
    )
}

fn render_horizontal_er_component_lines<'model>(
    boxes: &[RenderedEntityBox],
    layouts: &[ErRelationLayout<'model>],
    context: &ErRenderContext<'_>,
    adapter: &ErRelationComponentAdapter,
    resources: &mut ResourceContext,
    deferred_text: &mut DeferredTextRegistry<'model>,
    execution: AsciiExecution<'_>,
) -> Result<Vec<RelationGraphLine>> {
    relation_graph::render_horizontal_relation_components_with_execution(
        boxes,
        layouts,
        context.direction.transform(),
        context.options,
        resources,
        adapter,
        deferred_text,
        execution,
    )
}

fn plan_vertical_relationship<'plan>(
    top: &'plan RenderedEntityBox,
    bottom: &'plan RenderedEntityBox,
    layout: &'plan ErRelationLayout<'_>,
    charset: ErCharset,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<RelationRegionPlan<'plan>> {
    let relationship = layout.relationship;
    let (top_cardinality, bottom_cardinality) = layout.physical_cardinalities();
    let top_cardinality = cardinality_marker(top_cardinality, charset)?;
    let bottom_cardinality = cardinality_marker(bottom_cardinality, charset)?;
    let line = relationship_line(&relationship.rel_spec.rel_type, charset)?;
    let label_half_width = layout
        .label
        .as_ref()
        .map(RelationGraphLabel::half_width)
        .unwrap_or(0);
    let plan = RelationStackPlan::try_new(
        top,
        bottom,
        &[
            display_width_with_profile(top_cardinality, width_profile) / 2,
            display_width_with_profile(bottom_cardinality, width_profile) / 2,
            label_half_width,
        ],
        resources,
        |center, resources| {
            er_relationship_extent(
                top_cardinality,
                bottom_cardinality,
                line,
                layout.label.as_ref(),
                center,
                width_profile,
                resources,
            )
        },
    )?;

    Ok(RelationRegionPlan::Vertical {
        plan,
        rows: Box::new(move |center, resources| {
            er_relationship_rows(
                top_cardinality,
                bottom_cardinality,
                line,
                layout.label.as_ref(),
                center,
                width_profile,
                resources,
            )
        }),
    })
}

fn er_relationship_extent(
    top_cardinality: &str,
    bottom_cardinality: &str,
    line: char,
    label: Option<&RelationGraphLabel>,
    center: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let label = label
        .map(|label| (label.width(), label.line_count()))
        .unwrap_or((0, 0));
    let centered = relation_graph::centered_row_blocks_extent(
        center,
        [
            (
                display_width_with_profile(top_cardinality, width_profile),
                1,
            ),
            label,
            (
                display_width_with_profile(bottom_cardinality, width_profile),
                1,
            ),
        ],
        resources,
    )?;
    let width = centered
        .width()
        .max(resources.checked_grid_add(center, terminal_char_display_width(line, width_profile))?);
    let height = resources.checked_grid_add(centered.height(), 1)?;
    resources.grid_extent(width, height)
}

fn er_relationship_rows(
    top_cardinality: &str,
    bottom_cardinality: &str,
    line: char,
    label: Option<&RelationGraphLabel>,
    center: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let mut relation_lines = Vec::new();
    let row_count = label
        .map(RelationGraphLabel::line_count)
        .unwrap_or(0)
        .checked_add(3)
        .ok_or_else(|| grid_overflow(resources))?;
    relation_lines
        .try_reserve_exact(row_count)
        .map_err(|_| layout_allocation_failed())?;
    relation_lines.push(relation_graph::centered_text_line_with_role(
        top_cardinality,
        center,
        AsciiColorRole::EdgeArrow,
        width_profile,
        resources,
    )?);
    if let Some(label) = label {
        relation_lines.extend(relation_graph::centered_label_lines_with_role(
            label,
            center,
            AsciiColorRole::EdgeLabel,
            resources,
        )?);
    }
    relation_lines.push(relation_graph::marker_line_with_role(
        line,
        center,
        AsciiColorRole::EdgeLine,
        width_profile,
        resources,
    )?);
    relation_lines.push(relation_graph::centered_text_line_with_role(
        bottom_cardinality,
        center,
        AsciiColorRole::EdgeArrow,
        width_profile,
        resources,
    )?);
    Ok(relation_lines)
}

fn plan_parallel_vertical_relationships<'plan, 'model>(
    boxes: Vec<&'plan RenderedEntityBox>,
    layouts: Vec<&'plan ErRelationLayout<'model>>,
    options: &AsciiRenderOptions,
    charset: ErCharset,
    resources: &mut ResourceContext,
    deferred: &mut DeferredTextRegistry<'model>,
) -> Result<RelationRegionPlan<'plan>> {
    let width_profile = options.terminal_width_profile;
    let first = layouts
        .first()
        .copied()
        .ok_or(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "empty parallel ER relationship layout",
        })?;
    let (top_id, bottom_id) = first.physical_ids();
    let top =
        relation_graph::find_box_ref(&boxes, top_id).ok_or(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "relationships with missing endpoint entities",
        })?;
    let bottom =
        relation_graph::find_box_ref(&boxes, bottom_id).ok_or(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "relationships with missing endpoint entities",
        })?;
    let mut lane_extents = Vec::new();
    lane_extents
        .try_reserve_exact(layouts.len())
        .map_err(|_| layout_allocation_failed())?;
    for layout in &layouts {
        lane_extents.push(parallel_er_lane_extent(
            layout,
            charset,
            width_profile,
            resources,
        )?);
    }
    let plan = RelationParallelPlan::new(top, bottom, lane_extents, 2, resources)?;

    if !plan.ports_fit(resources)? {
        let reason = relation_graph::LayeredRelationSummaryReason::RouteCollision;
        let mut rows = Vec::new();
        rows.try_reserve_exact(layouts.len())
            .map_err(|_| layout_allocation_failed())?;
        for layout in layouts {
            rows.push(er_relationship_summary_row(
                layout,
                charset,
                width_profile,
                resources,
                deferred,
            )?);
        }
        return Ok(RelationRegionPlan::Summary(
            RelationSummaryPaintPlan::stacked(
                boxes,
                first.transform,
                rows,
                Some(reason),
                options,
                resources,
            )?,
        ));
    }

    Ok(RelationRegionPlan::Parallel {
        plan,
        lanes: Box::new(move |resources| {
            let mut lanes = Vec::new();
            lanes
                .try_reserve_exact(layouts.len())
                .map_err(|_| layout_allocation_failed())?;
            for layout in layouts {
                lanes.push(parallel_er_lane_rows(
                    layout,
                    charset,
                    width_profile,
                    resources,
                )?);
            }
            Ok(lanes)
        }),
    })
}

fn parallel_er_lane_extent(
    layout: &ErRelationLayout<'_>,
    charset: ErCharset,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let (top_cardinality, bottom_cardinality) = layout.physical_cardinalities();
    let top_cardinality = cardinality_marker(top_cardinality, charset)?;
    let bottom_cardinality = cardinality_marker(bottom_cardinality, charset)?;
    let relation = relationship_line(&layout.relationship.rel_spec.rel_type, charset)?;
    let label_height = layout
        .label
        .as_ref()
        .map(RelationGraphLabel::line_count)
        .unwrap_or(1);
    let height = resources.checked_grid_add(label_height, 3)?;
    let relation_width = {
        let mut encoded = [0; 4];
        display_width_with_profile(relation.encode_utf8(&mut encoded), width_profile)
    };
    let width = [
        display_width_with_profile(top_cardinality, width_profile),
        display_width_with_profile(bottom_cardinality, width_profile),
        relation_width,
        layout
            .label
            .as_ref()
            .map(RelationGraphLabel::width)
            .unwrap_or(0),
    ]
    .into_iter()
    .max()
    .unwrap_or(1)
    .max(1);
    resources.grid_extent(width, height)
}

fn parallel_er_lane_rows(
    layout: &ErRelationLayout<'_>,
    charset: ErCharset,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let relationship = layout.relationship;
    let (top_cardinality, bottom_cardinality) = layout.physical_cardinalities();
    let top_cardinality = cardinality_marker(top_cardinality, charset)?;
    let bottom_cardinality = cardinality_marker(bottom_cardinality, charset)?;
    let line = relationship_line(&relationship.rel_spec.rel_type, charset)?;
    let label_lines = match layout.label.as_ref() {
        Some(label) => {
            relation_graph::label_lines_with_role(label, AsciiColorRole::EdgeLabel, resources)?
        }
        None => vec![RelationGraphLine::try_with_role(
            "",
            AsciiColorRole::EdgeLabel,
            width_profile,
            resources,
        )?],
    };
    let row_count = label_lines
        .len()
        .checked_add(3)
        .ok_or_else(|| grid_overflow(resources))?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(row_count)
        .map_err(|_| layout_allocation_failed())?;
    rows.push(RelationGraphLine::try_with_role(
        top_cardinality,
        AsciiColorRole::EdgeArrow,
        width_profile,
        resources,
    )?);
    rows.extend(label_lines);
    let line = line.to_string();
    rows.push(RelationGraphLine::try_with_role(
        &line,
        AsciiColorRole::EdgeLine,
        width_profile,
        resources,
    )?);
    rows.push(RelationGraphLine::try_with_role(
        bottom_cardinality,
        AsciiColorRole::EdgeArrow,
        width_profile,
        resources,
    )?);
    Ok(rows)
}

fn er_relationship_summary_row<'model>(
    layout: &ErRelationLayout<'model>,
    charset: ErCharset,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    deferred: &mut DeferredTextRegistry<'model>,
) -> Result<RelationGraphSummaryRow> {
    let relationship = layout.relationship;
    // Summary rows retain semantic source/target identity. Direction changes physical placement
    // and port selection only; it must not relabel the relationship terminals.
    let left_cardinality =
        horizontal_cardinality_marker(layout.top_cardinality, PhysicalPortSide::Right, charset)?;
    let right_cardinality =
        horizontal_cardinality_marker(layout.bottom_cardinality, PhysicalPortSide::Left, charset)?;
    let relation = er_relationship_summary_line(&relationship.rel_spec.rel_type, charset)?;
    let source = deferred.try_register_framed_value(
        "id(bytes=",
        layout.top_identity,
        width_profile,
        resources,
    )?;
    let connector = deferred.try_register(
        ComposedTextPlan::try_new(resources, 3, |push| {
            push(left_cardinality)?;
            push(relation)?;
            push(right_cardinality)
        })?,
        width_profile,
        resources,
    )?;
    let target = deferred.try_register_framed_value(
        "id(bytes=",
        layout.bottom_identity,
        width_profile,
        resources,
    )?;
    let label = layout
        .label
        .as_ref()
        .map(RelationGraphLabel::shared_lines)
        .unwrap_or_else(|| std::rc::Rc::new(Vec::new()));
    Ok(RelationGraphSummaryRow::new(
        source, connector, target, label,
    ))
}

fn er_relationship_summary_row_for_reason<'model>(
    layout: &ErRelationLayout<'model>,
    charset: ErCharset,
    reason: relation_graph::LayeredRelationSummaryReason,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    deferred: &mut DeferredTextRegistry<'model>,
) -> Result<RelationGraphSummaryRow> {
    match reason {
        relation_graph::LayeredRelationSummaryReason::Crossing
        | relation_graph::LayeredRelationSummaryReason::RouteCollision
        | relation_graph::LayeredRelationSummaryReason::OverlayCollision => {
            er_relationship_summary_row(layout, charset, width_profile, resources, deferred)
        }
    }
}

fn er_layered_edge(layout: &ErRelationLayout<'_>) -> LayeredRelationEdge {
    LayeredRelationEdge::new(
        layout.top_id,
        layout.bottom_id,
        layout
            .label
            .as_ref()
            .map(RelationGraphLabel::width)
            .unwrap_or(0)
            .max(1),
        layout
            .label
            .as_ref()
            .map(RelationGraphLabel::line_count)
            .unwrap_or(0),
    )
    .with_reversed_route(layout.transform.reverses_vertical_order())
}

fn er_layered_error(error: LayeredRelationError) -> AsciiError {
    let feature = match error {
        LayeredRelationError::MissingEndpoint => "relationships with missing endpoint entities",
        LayeredRelationError::UnrelatedBoxes => "ER relationship layouts with unrelated entities",
        LayeredRelationError::Crossing => "crossing ER relationship layouts",
    };
    AsciiError::UnsupportedFeature {
        diagram_type: "er",
        feature,
    }
}

impl<'model> relation_graph::RelationComponentAdapter<'model, ErRelationLayout<'model>>
    for ErRelationComponentAdapter
{
    fn direction_transform(&self) -> relation_graph::DirectionTransform {
        self.transform
    }

    fn build_edges(&self, layout: &ErRelationLayout<'_>) -> LayeredRelationEdge {
        er_layered_edge(layout)
    }

    fn is_self_relation(&self, layout: &ErRelationLayout<'_>) -> bool {
        layout.top_id == layout.bottom_id
    }

    fn self_loop_metrics(
        &self,
        layout: &ErRelationLayout<'_>,
        resources: &ResourceContext,
    ) -> Result<RelationSelfLoopMetrics> {
        self_loop_metrics_for_er_relationship(
            layout,
            self.charset,
            self.width_profile,
            self.transform.is_horizontal(),
            resources,
        )
    }

    fn self_loop_rows(
        &self,
        layout: &ErRelationLayout<'_>,
        resources: &ResourceContext,
    ) -> Result<relation_graph::RelationSelfLoopRows> {
        self_loop_rows_for_er_relationship(
            layout,
            self.charset,
            self.width_profile,
            self.transform.is_horizontal(),
            resources,
        )
    }

    fn horizontal_relation_style(
        &self,
        layout: &ErRelationLayout<'_>,
        source_side: PhysicalPortSide,
        target_side: PhysicalPortSide,
        _resources: &ResourceContext,
    ) -> Result<HorizontalRelationStyle> {
        let (top_cardinality, bottom_cardinality) = layout.physical_cardinalities();
        let source_marker = HorizontalRelationMarker::new(
            horizontal_cardinality_marker(top_cardinality, source_side, self.charset)?,
            AsciiColorRole::EdgeArrow,
            self.width_profile,
        );
        let target_marker = HorizontalRelationMarker::new(
            horizontal_cardinality_marker(bottom_cardinality, target_side, self.charset)?,
            AsciiColorRole::EdgeArrow,
            self.width_profile,
        );
        Ok(HorizontalRelationStyle::new(
            HorizontalRelationEndpoint::new(Some(source_marker), None),
            HorizontalRelationEndpoint::new(Some(target_marker), None),
            layout.label.clone(),
            relationship_horizontal_line(&layout.relationship.rel_spec.rel_type, self.charset)?,
            relationship_line(&layout.relationship.rel_spec.rel_type, self.charset)?,
            relation_line_chars(self.charset),
        ))
    }

    fn layered_horizontal_gap(&self) -> usize {
        ER_LEVEL_HORIZONTAL_GAP
    }

    fn layered_route_style(
        &self,
        layout: &ErRelationLayout<'_>,
    ) -> Result<LayeredRelationRouteStyle> {
        let relationship = layout.relationship;
        let vertical = relationship_line(&relationship.rel_spec.rel_type, self.charset)?;
        let horizontal =
            relationship_horizontal_line(&relationship.rel_spec.rel_type, self.charset)?;
        let relation_chars = relation_line_chars(self.charset);
        Ok(LayeredRelationRouteStyle::new(
            vertical,
            horizontal,
            relation_chars,
            relation_graph::LayeredRelationRouteProfile::er(),
        ))
    }

    fn layered_relation_overlays(
        &self,
        layout: &ErRelationLayout<'_>,
        geometry: &relation_graph::LayeredRelationRouteGeometry,
        resources: &ResourceContext,
    ) -> Result<Vec<RelationOverlay>> {
        resources.charge_layout_work(3)?;
        let (top_cardinality, bottom_cardinality) = layout.physical_cardinalities();
        let top_cardinality = cardinality_marker(top_cardinality, self.charset)?;
        let bottom_cardinality = cardinality_marker(bottom_cardinality, self.charset)?;

        let mut overlays = Vec::new();
        overlays
            .try_reserve_exact(3)
            .map_err(|_| layout_allocation_failed())?;
        overlays.push(RelationOverlay::text(
            geometry.source_x(),
            geometry.source_marker_y(),
            top_cardinality.to_string(),
            AsciiColorRole::EdgeArrow,
            self.width_profile,
        ));
        if let Some(label) = layout.label.as_ref() {
            let (center_x, y) = geometry.relation_label_anchor(label.line_count(), resources)?;
            overlays.push(RelationOverlay::label(
                center_x,
                y,
                label.clone(),
                AsciiColorRole::EdgeLabel,
            ));
        }
        overlays.push(RelationOverlay::text(
            geometry.target_x(),
            geometry.target_marker_y(),
            bottom_cardinality.to_string(),
            AsciiColorRole::EdgeArrow,
            self.width_profile,
        ));
        Ok(overlays)
    }

    fn plan_vertical_region<'plan>(
        &self,
        boxes: &[&'plan RenderedEntityBox],
        layout: &'plan ErRelationLayout<'model>,
        resources: &mut ResourceContext,
    ) -> Result<RelationRegionPlan<'plan>> {
        let (top_id, bottom_id) = layout.physical_ids();
        let top =
            relation_graph::find_box_ref(boxes, top_id).ok_or(AsciiError::UnsupportedFeature {
                diagram_type: "er",
                feature: "relationships with missing endpoint entities",
            })?;
        let bottom = relation_graph::find_box_ref(boxes, bottom_id).ok_or(
            AsciiError::UnsupportedFeature {
                diagram_type: "er",
                feature: "relationships with missing endpoint entities",
            },
        )?;
        plan_vertical_relationship(
            top,
            bottom,
            layout,
            self.charset,
            self.width_profile,
            resources,
        )
    }

    fn plan_parallel_region<'plan>(
        &self,
        boxes: Vec<&'plan RenderedEntityBox>,
        layouts: Vec<&'plan ErRelationLayout<'model>>,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
        deferred: &mut DeferredTextRegistry<'model>,
    ) -> Result<RelationRegionPlan<'plan>> {
        plan_parallel_vertical_relationships(
            boxes,
            layouts,
            options,
            self.charset,
            resources,
            deferred,
        )
    }

    fn build_summary_row(
        &self,
        layout: &ErRelationLayout<'model>,
        reason: relation_graph::LayeredRelationSummaryReason,
        resources: &ResourceContext,
        deferred: &mut DeferredTextRegistry<'model>,
    ) -> Result<RelationGraphSummaryRow> {
        er_relationship_summary_row_for_reason(
            layout,
            self.charset,
            reason,
            self.width_profile,
            resources,
            deferred,
        )
    }

    fn layered_error(&self, error: LayeredRelationError) -> AsciiError {
        er_layered_error(error)
    }
}

fn er_self_loop_cardinality_markers(
    layout: &ErRelationLayout<'_>,
    charset: ErCharset,
    horizontal: bool,
) -> Result<(&'static str, &'static str)> {
    let (top_cardinality, bottom_cardinality) = layout.physical_cardinalities();
    if horizontal {
        Ok((
            horizontal_cardinality_marker(top_cardinality, PhysicalPortSide::Right, charset)?,
            horizontal_cardinality_marker(bottom_cardinality, PhysicalPortSide::Left, charset)?,
        ))
    } else {
        Ok((
            cardinality_marker(top_cardinality, charset)?,
            cardinality_marker(bottom_cardinality, charset)?,
        ))
    }
}

fn self_loop_metrics_for_er_relationship(
    layout: &ErRelationLayout<'_>,
    charset: ErCharset,
    width_profile: TerminalWidthProfile,
    horizontal: bool,
    _resources: &ResourceContext,
) -> Result<RelationSelfLoopMetrics> {
    let relationship = layout.relationship;
    let (top_cardinality, bottom_cardinality) =
        er_self_loop_cardinality_markers(layout, charset, horizontal)?;
    Ok(RelationSelfLoopMetrics::new(
        display_width_with_profile(top_cardinality, width_profile),
        layout
            .label
            .as_ref()
            .map(RelationGraphLabel::width)
            .unwrap_or(0),
        layout
            .label
            .as_ref()
            .map(RelationGraphLabel::line_count)
            .unwrap_or(0),
        display_width_with_profile(bottom_cardinality, width_profile),
        Some(display_width_with_profile(top_cardinality, width_profile)),
        relationship_horizontal_line(&relationship.rel_spec.rel_type, charset)?,
        relationship_line(&relationship.rel_spec.rel_type, charset)?,
    ))
}

fn self_loop_rows_for_er_relationship(
    layout: &ErRelationLayout<'_>,
    charset: ErCharset,
    width_profile: TerminalWidthProfile,
    horizontal: bool,
    resources: &ResourceContext,
) -> Result<relation_graph::RelationSelfLoopRows> {
    let relationship = layout.relationship;
    let (top_cardinality, bottom_cardinality) =
        er_self_loop_cardinality_markers(layout, charset, horizontal)?;
    let top_marker = RelationGraphLine::try_with_role(
        top_cardinality,
        AsciiColorRole::EdgeArrow,
        width_profile,
        resources,
    )?;
    let bottom_marker = RelationGraphLine::try_with_role(
        bottom_cardinality,
        AsciiColorRole::EdgeArrow,
        width_profile,
        resources,
    )?;
    let label_lines = match layout.label.as_ref() {
        Some(label) => {
            relation_graph::label_lines_with_role(label, AsciiColorRole::EdgeLabel, resources)?
        }
        None => Vec::new(),
    };

    Ok(relation_graph::RelationSelfLoopRows::new(
        top_marker,
        label_lines,
        bottom_marker,
        relationship_horizontal_line(&relationship.rel_spec.rel_type, charset)?,
        relationship_line(&relationship.rel_spec.rel_type, charset)?,
    )
    .with_tail_prefix(RelationGraphLine::try_with_role(
        top_cardinality,
        AsciiColorRole::EdgeArrow,
        width_profile,
        resources,
    )?))
}

fn cardinality_marker(cardinality: &str, charset: ErCharset) -> Result<&'static str> {
    match cardinality {
        "ONLY_ONE" => Ok("||"),
        "ZERO_OR_ONE" => Ok("o|"),
        "ONE_OR_MORE" => Ok("|{"),
        "ZERO_OR_MORE" => Ok("o{"),
        "MD_PARENT" => Ok(charset.md_parent),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "unknown ER cardinality markers",
        }),
    }
}

fn horizontal_cardinality_marker(
    cardinality: &str,
    side: PhysicalPortSide,
    charset: ErCharset,
) -> Result<&'static str> {
    match (cardinality, side) {
        ("ONLY_ONE", _) => Ok("||"),
        ("ZERO_OR_ONE", PhysicalPortSide::Right) => Ok("|o"),
        ("ZERO_OR_ONE", PhysicalPortSide::Left) => Ok("o|"),
        ("ONE_OR_MORE", PhysicalPortSide::Right) => Ok("}|"),
        ("ONE_OR_MORE", PhysicalPortSide::Left) => Ok("|{"),
        ("ZERO_OR_MORE", PhysicalPortSide::Right) => Ok("}o"),
        ("ZERO_OR_MORE", PhysicalPortSide::Left) => Ok("o{"),
        ("MD_PARENT", _) => Ok(charset.md_parent),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "unknown ER cardinality markers",
        }),
    }
}

fn relationship_horizontal_line(rel_type: &str, charset: ErCharset) -> Result<char> {
    match rel_type {
        "IDENTIFYING" | "" => Ok(charset.solid_horizontal_relation),
        "NON_IDENTIFYING" => Ok(charset.dotted_horizontal_relation),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "unknown ER relationship identification types",
        }),
    }
}

fn er_relationship_summary_line(rel_type: &str, charset: ErCharset) -> Result<&'static str> {
    match relationship_horizontal_line(rel_type, charset)? {
        '-' | '─' => Ok("--"),
        '.' | '╌' => Ok(".."),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "unknown ER relationship identification types",
        }),
    }
}

fn relationship_line(rel_type: &str, charset: ErCharset) -> Result<char> {
    match rel_type {
        "IDENTIFYING" | "" => Ok(charset.solid_relation),
        "NON_IDENTIFYING" => Ok(charset.dotted_relation),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "unknown ER relationship identification types",
        }),
    }
}

fn relation_line_chars(charset: ErCharset) -> RelationLineChars {
    RelationLineChars::new(
        [
            charset.solid_horizontal_relation,
            charset.solid_relation,
            charset.dotted_horizontal_relation,
            charset.dotted_relation,
        ],
        charset.relation_junction,
    )
}

fn charge_er_model_work(
    model: &ErDiagramRenderModel,
    resources: &mut ResourceContext,
) -> Result<()> {
    let attribute_count = model.entities.values().try_fold(0usize, |total, entity| {
        total.checked_add(entity.attributes.len())
    });
    let item_count = model
        .entities
        .len()
        .checked_add(model.relationships.len())
        .and_then(|value| value.checked_add(attribute_count?))
        .ok_or_else(|| work_overflow(resources))?;
    resources.charge_layout_work(item_count.max(1))
}

fn preflight_er_text(model: &ErDiagramRenderModel, resources: &mut ResourceContext) -> Result<()> {
    for entity in model.entities.values() {
        charge_text_layout(resources, &entity.id)?;
        charge_text_layout(resources, entity_display_label(entity))?;
        for attribute in &entity.attributes {
            charge_text_layout(resources, &attribute.ty)?;
            charge_text_layout(resources, &attribute.name)?;
            for key in &attribute.keys {
                charge_text_layout(resources, key)?;
            }
            charge_text_layout(resources, &attribute.comment)?;
        }
    }

    for relationship in &model.relationships {
        charge_text_layout(resources, &relationship.entity_a)?;
        charge_text_layout(resources, &relationship.role_a)?;
        charge_text_layout(resources, &relationship.entity_b)?;
        charge_text_layout(resources, &relationship.rel_spec.card_a)?;
        charge_text_layout(resources, &relationship.rel_spec.card_b)?;
        charge_text_layout(resources, &relationship.rel_spec.rel_type)?;
    }

    Ok(())
}

fn work_overflow(resources: &ResourceContext) -> AsciiError {
    resources.work_overflow()
}

fn grid_overflow(resources: &ResourceContext) -> AsciiError {
    resources.grid_overflow()
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::AsciiColorMode;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;
    use merman_core::{CancelReason, OperationControl, OperationPhase};
    use std::cell::Cell;

    fn resources_with_limit(id: AsciiResourceLimitId, max: usize) -> ResourceContext {
        let policy = AsciiResourcePolicy::default()
            .with_limit(id, max)
            .expect("resource test limit should be valid");
        ResourceContext::new(policy)
    }

    fn composite_attribute() -> ErAttributeRenderModel {
        ErAttributeRenderModel {
            ty: "string".to_string(),
            name: "owner_id".to_string(),
            keys: vec!["PK".to_string(), "FK".to_string()],
            comment: "record owner".to_string(),
        }
    }

    fn unbounded_policy() -> AsciiResourcePolicy {
        AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
    }

    fn horizontal_control_model() -> ErDiagramRenderModel {
        let mut model = ErDiagramRenderModel {
            direction: "LR".to_string(),
            ..ErDiagramRenderModel::default()
        };
        for id in ["A", "B"] {
            model.entities.insert(
                id.to_string(),
                ErEntityRenderModel {
                    id: id.to_string(),
                    label: id.to_string(),
                    ..ErEntityRenderModel::default()
                },
            );
        }
        model.relationships.push(ErRelationshipRenderModel {
            entity_a: "A".to_string(),
            role_a: "owns".to_string(),
            entity_b: "B".to_string(),
            rel_spec: merman_core::diagrams::er::ErRelSpecRenderModel {
                card_a: "ONLY_ONE".to_string(),
                card_b: "ZERO_OR_MORE".to_string(),
                rel_type: "IDENTIFYING".to_string(),
            },
        });
        model
    }

    fn render_er_identity_fixture(model: &ErDiagramRenderModel) -> String {
        let policy = unbounded_policy();
        render_er_diagram_with_execution(
            model,
            &AsciiRenderOptions::ascii(),
            AsciiExecution::for_test(&policy),
        )
        .expect("ER identity fixture should render")
    }

    fn render_er_section_fixture(
        entity: &ErEntityRenderModel,
        options: &AsciiRenderOptions,
        policy: AsciiResourcePolicy,
        materialized: &Cell<bool>,
    ) -> (Result<String>, (usize, usize), (usize, usize)) {
        let mut resources = ResourceContext::new(policy);
        let mut deferred = DeferredTextRegistry::new();
        let relation_box = render_entity_box(
            &entity.id,
            entity,
            options,
            ErCharset::for_options(options),
            &mut deferred,
            &mut resources,
        )
        .expect("ER section fixture should plan before final output admission");
        let lines = relation_graph::stacked_box_lines(
            std::slice::from_ref(&relation_box),
            options.terminal_width_profile,
            &mut resources,
        )
        .expect("ER section fixture should build relation rows");
        let before = (
            resources.layout_work_used(),
            resources.document_cells_used(),
        );
        let result = relation_graph::render_lines_with_deferred_probe(
            &lines,
            options,
            &mut resources,
            &deferred,
            || materialized.set(true),
        );
        let after = (
            resources.layout_work_used(),
            resources.document_cells_used(),
        );
        (result, before, after)
    }

    fn er_summary_model() -> ErDiagramRenderModel {
        let mut model = ErDiagramRenderModel {
            direction: "TB".to_string(),
            ..ErDiagramRenderModel::default()
        };
        model.entities.insert(
            "A".to_string(),
            ErEntityRenderModel {
                id: "A".to_string(),
                label: "Source<&中".to_string(),
                ..ErEntityRenderModel::default()
            },
        );
        model.entities.insert(
            "B".to_string(),
            ErEntityRenderModel {
                id: "B".to_string(),
                label: "Target<&中".to_string(),
                ..ErEntityRenderModel::default()
            },
        );
        model.relationships = vec![
            ErRelationshipRenderModel {
                entity_a: "A".to_string(),
                role_a: "self<&中".to_string(),
                entity_b: "A".to_string(),
                rel_spec: merman_core::diagrams::er::ErRelSpecRenderModel {
                    card_a: "ONLY_ONE".to_string(),
                    card_b: "ZERO_OR_MORE".to_string(),
                    rel_type: "IDENTIFYING".to_string(),
                },
            },
            ErRelationshipRenderModel {
                entity_a: "A".to_string(),
                role_a: "linked<&中".to_string(),
                entity_b: "B".to_string(),
                rel_spec: merman_core::diagrams::er::ErRelSpecRenderModel {
                    card_a: "ZERO_OR_ONE".to_string(),
                    card_b: "ONE_OR_MORE".to_string(),
                    rel_type: "NON_IDENTIFYING".to_string(),
                },
            },
        ];
        model
    }

    fn render_er_summary_fixture(
        model: &ErDiagramRenderModel,
        options: &AsciiRenderOptions,
        policy: AsciiResourcePolicy,
        materialized: &Cell<bool>,
    ) -> (Result<String>, (usize, usize), (usize, usize)) {
        let mut resources = ResourceContext::new(policy);
        let charset = ErCharset::for_options(options);
        let direction = RelationDirection::try_from_model(
            &model.direction,
            "er",
            "unknown ER diagram directions",
        )
        .expect("ER summary direction should be valid");
        let mut deferred = DeferredTextRegistry::new();
        let mut boxes = Vec::new();
        boxes
            .try_reserve_exact(model.entities.len())
            .expect("ER summary box allocation should succeed");
        for (authored_identity, entity) in &model.entities {
            boxes.push(
                render_entity_box(
                    authored_identity,
                    entity,
                    options,
                    charset,
                    &mut deferred,
                    &mut resources,
                )
                .expect("ER summary box should plan"),
            );
        }
        let authored_entity_identities = index_unique_er_entity_identities(model, &resources)
            .expect("ER summary identities should plan");
        let mut layouts = Vec::new();
        layouts
            .try_reserve_exact(model.relationships.len())
            .expect("ER summary layout allocation should succeed");
        for relationship in &model.relationships {
            let label_plan = RelationGraphLabelPlan::try_new(
                &relationship.role_a,
                options.terminal_width_profile,
                &deferred,
                &resources,
            )
            .expect("ER summary label should plan");
            let label = label_plan
                .map(|plan| plan.materialize(&mut deferred, &resources))
                .transpose()
                .expect("ER summary label should materialize");
            layouts.push(ErRelationLayout {
                transform: direction.transform(),
                relationship,
                top_id: relationship.entity_a.as_str(),
                bottom_id: relationship.entity_b.as_str(),
                top_identity: authored_er_entity_identity(
                    &relationship.entity_a,
                    &authored_entity_identities,
                )
                .expect("ER summary source identity should exist"),
                bottom_identity: authored_er_entity_identity(
                    &relationship.entity_b,
                    &authored_entity_identities,
                )
                .expect("ER summary target identity should exist"),
                top_cardinality: relationship.rel_spec.card_b.as_str(),
                bottom_cardinality: relationship.rel_spec.card_a.as_str(),
                label,
            });
        }
        let adapter = ErRelationComponentAdapter {
            charset,
            width_profile: options.terminal_width_profile,
            transform: direction.transform(),
        };
        let lines = relation_graph::render_relation_component_lines(
            &boxes,
            &layouts,
            options,
            &mut resources,
            &adapter,
            &mut deferred,
        )
        .expect("ER summary should plan");
        let before = (
            resources.layout_work_used(),
            resources.document_cells_used(),
        );
        let result = relation_graph::render_lines_with_deferred_probe(
            &lines,
            options,
            &mut resources,
            &deferred,
            || materialized.set(true),
        );
        let after = (
            resources.layout_work_used(),
            resources.document_cells_used(),
        );
        (result, before, after)
    }

    #[test]
    fn er_role_discloses_lossy_authored_projection_in_routed_and_summary_output() {
        const AUTHORED: &str = "\u{1b}";
        const VISIBLE: &str = r"\u{1B}";
        const DISCLOSURE: &str = r#"authored(bytes=1)="\u{1B}""#;

        let routed = |value: &str| {
            let mut model = horizontal_control_model();
            model.relationships[0].role_a = value.to_string();
            render_er_identity_fixture(&model)
        };
        let authored_routed = routed(AUTHORED);
        let visible_routed = routed(VISIBLE);
        assert_ne!(authored_routed, visible_routed);
        assert!(authored_routed.contains(DISCLOSURE));
        assert!(!visible_routed.contains("authored(bytes="));

        let html_break_routed = routed("owns<br>items");
        let line_break_routed = routed("owns\nitems");
        assert_ne!(html_break_routed, line_break_routed);
        assert!(html_break_routed.contains(r#"authored(bytes=13)="owns<br>items""#));
        assert!(line_break_routed.contains(r#"authored(bytes=10)="owns\nitems""#));

        let trimmed_routed = routed(" owns ");
        let untrimmed_routed = routed("owns");
        assert_ne!(trimmed_routed, untrimmed_routed);
        assert!(trimmed_routed.contains(r#"authored(bytes=6)=" owns ""#));
        assert!(!untrimmed_routed.contains("authored(bytes="));

        let summary = |value: &str| {
            let mut model = er_summary_model();
            for relationship in &mut model.relationships {
                relationship.role_a = value.to_string();
            }
            render_er_summary_fixture(
                &model,
                &AsciiRenderOptions::ascii(),
                unbounded_policy(),
                &Cell::new(false),
            )
            .0
            .expect("ER summary identity fixture should render")
        };
        let authored_summary = summary(AUTHORED);
        let visible_summary = summary(VISIBLE);
        assert!(authored_summary.contains("relations:"));
        assert_ne!(authored_summary, visible_summary);
        assert!(authored_summary.contains(DISCLOSURE));
        assert!(!visible_summary.contains("authored(bytes="));

        let html_break_summary = summary("owns<br>items");
        let line_break_summary = summary("owns\nitems");
        assert_ne!(html_break_summary, line_break_summary);
        assert!(html_break_summary.contains(r#"authored(bytes=13)="owns<br>items""#));
        assert!(line_break_summary.contains(r#"authored(bytes=10)="owns\nitems""#));
    }

    #[test]
    fn er_relation_label_batch_admits_aggregate_limits_before_registry_materialization() {
        let mut model = er_summary_model();
        model.relationships[0].role_a = "left<&".repeat(128);
        model.relationships[1].role_a = "right<&".repeat(128);
        let options = AsciiRenderOptions::ascii();

        let measured_resources = ResourceContext::new(unbounded_policy());
        let mut measured_registry = DeferredTextRegistry::new();
        let measured = prepare_er_relation_labels(
            &model.relationships,
            &options,
            &mut measured_registry,
            &measured_resources,
        )
        .expect("the aggregate relation-label batch should materialize without limits");
        assert_eq!(measured.iter().flatten().count(), 2);
        let aggregate_bytes = measured
            .iter()
            .flatten()
            .flat_map(RelationGraphLabel::lines)
            .map(DeferredTextLine::plain_bytes)
            .sum::<usize>();
        let aggregate_document_cells = measured
            .iter()
            .flatten()
            .flat_map(RelationGraphLabel::lines)
            .map(DeferredTextLine::width)
            .sum::<usize>();
        let aggregate_grid_cells = measured
            .iter()
            .flatten()
            .map(|label| label.width() * label.line_count())
            .max()
            .unwrap_or(0);
        assert!(aggregate_bytes > model.relationships[0].role_a.len());
        assert!(aggregate_document_cells > model.relationships[0].role_a.len());

        const PRIOR_WORK: usize = 7;
        const PRIOR_DOCUMENT: usize = 11;
        for (limit, actual) in [
            (AsciiResourceLimitId::MaxGridCells, aggregate_grid_cells),
            (
                AsciiResourceLimitId::MaxDocumentCells,
                PRIOR_DOCUMENT + aggregate_document_cells,
            ),
            (AsciiResourceLimitId::MaxOutputBytes, aggregate_bytes),
        ] {
            let exact_policy = unbounded_policy()
                .with_limit(limit, actual)
                .expect("the exact aggregate relation-label limit should be valid");
            let exact_resources = ResourceContext::new(exact_policy);
            exact_resources
                .charge_usage(PRIOR_WORK, PRIOR_DOCUMENT)
                .expect("the exact fixture checkpoint should fit");
            let mut exact_registry = DeferredTextRegistry::new();
            prepare_er_relation_labels(
                &model.relationships,
                &options,
                &mut exact_registry,
                &exact_resources,
            )
            .unwrap_or_else(|error| {
                panic!("the exact aggregate {limit:?} budget should materialize: {error:?}")
            });
            assert!(exact_registry.entry_count() > 0, "limit={limit:?}");
            assert_eq!(
                exact_resources.document_cells_used(),
                PRIOR_DOCUMENT,
                "limit={limit:?}",
            );

            let below_policy = unbounded_policy()
                .with_limit(limit, actual - 1)
                .expect("the aggregate N-1 relation-label limit should be valid");
            let below_resources = ResourceContext::new(below_policy);
            below_resources
                .charge_usage(PRIOR_WORK, PRIOR_DOCUMENT)
                .expect("the N-1 fixture checkpoint should fit");
            let before = (
                below_resources.layout_work_used(),
                below_resources.document_cells_used(),
            );
            let mut below_registry = DeferredTextRegistry::new();
            let error = prepare_er_relation_labels(
                &model.relationships,
                &options,
                &mut below_registry,
                &below_resources,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == limit
                        && details.actual == actual
                        && details.max == actual - 1
            ));
            assert_eq!(below_registry.entry_count(), 0, "limit={limit:?}");
            assert_eq!(
                (
                    below_resources.layout_work_used(),
                    below_resources.document_cells_used(),
                ),
                before,
                "limit={limit:?}",
            );
        }

        let rendered = render_er_diagram_with_execution(
            &model,
            &options,
            AsciiExecution::for_test(&unbounded_policy()),
        )
        .expect("the public ER renderer should accept the multi-label fixture");
        let exact_policy = unbounded_policy()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, rendered.len())
            .expect("the public exact output limit should be valid");
        let exact = render_er_diagram_with_execution(
            &model,
            &options,
            AsciiExecution::for_test(&exact_policy),
        )
        .expect("the public ER renderer should accept the exact aggregate output budget");
        assert_eq!(exact, rendered);

        let below_policy = unbounded_policy()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, rendered.len() - 1)
            .expect("the public aggregate N-1 output limit should be valid");
        let error = render_er_diagram_with_execution(
            &model,
            &options,
            AsciiExecution::for_test(&below_policy),
        )
        .expect_err("the public ER renderer must reject aggregate output N-1");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxOutputBytes
                    && details.actual == rendered.len()
                    && details.max == rendered.len() - 1
        ));
    }

    #[test]
    fn horizontal_er_cancellation_wins_before_shared_work_ledger_mutation() {
        const WORK_LIMIT: usize = 10_000;

        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, WORK_LIMIT)
            .expect("horizontal ER work limit should be valid");
        let model = horizontal_control_model();
        let options = AsciiRenderOptions::ascii();
        let charset = ErCharset::for_options(&options);
        let direction = RelationDirection::try_from_model(
            &model.direction,
            "er",
            "unknown ER diagram directions",
        )
        .expect("horizontal ER direction should be valid");
        let mut resources = ResourceContext::new(policy);
        let mut deferred_text = DeferredTextRegistry::new();
        let mut boxes = Vec::new();
        for (authored_identity, entity) in &model.entities {
            boxes.push(
                render_entity_box(
                    authored_identity,
                    entity,
                    &options,
                    charset,
                    &mut deferred_text,
                    &mut resources,
                )
                .expect("horizontal ER entity should plan"),
            );
        }
        let authored_entity_identities = index_unique_er_entity_identities(&model, &resources)
            .expect("horizontal ER identities should plan");
        let mut layouts = Vec::new();
        for relationship in &model.relationships {
            let label_plan = RelationGraphLabelPlan::try_new(
                &relationship.role_a,
                options.terminal_width_profile,
                &deferred_text,
                &resources,
            )
            .expect("horizontal ER label should plan");
            let label = label_plan
                .map(|plan| plan.materialize(&mut deferred_text, &resources))
                .transpose()
                .expect("horizontal ER label should materialize");
            layouts.push(ErRelationLayout {
                transform: direction.transform(),
                relationship,
                top_id: relationship.entity_a.as_str(),
                bottom_id: relationship.entity_b.as_str(),
                top_identity: authored_er_entity_identity(
                    &relationship.entity_a,
                    &authored_entity_identities,
                )
                .expect("horizontal ER source identity should exist"),
                bottom_identity: authored_er_entity_identity(
                    &relationship.entity_b,
                    &authored_entity_identities,
                )
                .expect("horizontal ER target identity should exist"),
                top_cardinality: relationship.rel_spec.card_b.as_str(),
                bottom_cardinality: relationship.rel_spec.card_a.as_str(),
                label,
            });
        }
        let adapter = ErRelationComponentAdapter {
            charset,
            width_profile: options.terminal_width_profile,
            transform: direction.transform(),
        };
        let context = ErRenderContext {
            options: &options,
            charset,
            direction,
        };

        let remaining = WORK_LIMIT
            .checked_sub(resources.layout_work_used())
            .expect("fixture planning should remain below the work limit");
        resources
            .charge_layout_work(remaining)
            .expect("fixture should fill the work ledger exactly");
        let before = (
            resources.layout_work_used(),
            resources.document_cells_used(),
        );
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);

        let error = render_horizontal_er_component_lines(
            &boxes,
            &layouts,
            &context,
            &adapter,
            &mut resources,
            &mut deferred_text,
            AsciiExecution::new(&control, &policy),
        )
        .expect_err("scheduled cancellation must precede horizontal ER work exhaustion");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(
            (
                resources.layout_work_used(),
                resources.document_cells_used(),
            ),
            before,
            "cancellation before the rejected charge must not mutate the shared ledgers"
        );
    }

    #[test]
    fn er_summary_admits_exact_encoded_output_before_deferred_materialization() {
        let model = er_summary_model();
        for mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let base = AsciiRenderOptions::unicode().with_color_mode(mode);
            let base_policy = unbounded_policy();
            let measured_probe = Cell::new(false);
            let (measured, _, _) =
                render_er_summary_fixture(&model, &base, base_policy, &measured_probe);
            let expected = measured.expect("unbounded ER summary should render");
            assert!(measured_probe.get(), "mode={mode:?}");
            assert!(expected.contains("relations:"), "mode={mode:?}");
            if mode == AsciiColorMode::Html {
                assert!(expected.contains("Source&lt;&amp;中"), "mode={mode:?}");
                assert!(expected.contains("self&lt;&amp;中"), "mode={mode:?}");
                assert!(
                    expected.contains("id(bytes=1)=&quot;A&quot;"),
                    "mode={mode:?}"
                );
            } else {
                assert!(expected.contains("Source<&中"), "mode={mode:?}");
                assert!(expected.contains("self<&中"), "mode={mode:?}");
                assert!(expected.contains(r#"id(bytes=1)="A""#), "mode={mode:?}");
            }

            let exact_policy = base_policy
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len())
                .expect("exact ER summary output limit should be valid");
            let exact_probe = Cell::new(false);
            let (rendered, _, _) =
                render_er_summary_fixture(&model, &base, exact_policy, &exact_probe);
            assert_eq!(
                rendered.expect("exact ER summary should materialize"),
                expected,
                "mode={mode:?}"
            );
            assert!(exact_probe.get(), "mode={mode:?}");

            let below_policy = base_policy
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len() - 1)
                .expect("max-minus-one ER summary limit should be valid");
            let below_probe = Cell::new(false);
            let (error, before, after) =
                render_er_summary_fixture(&model, &base, below_policy, &below_probe);
            assert!(!below_probe.get(), "mode={mode:?}");
            assert_eq!(after, before, "mode={mode:?}");
            assert!(matches!(
                error.expect_err("max-minus-one ER summary must reject"),
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == AsciiResourceLimitId::MaxOutputBytes
                        && details.actual == expected.len()
                        && details.max == expected.len() - 1
            ));
        }
    }

    #[test]
    fn er_sections_admit_exact_encoded_output_before_deferred_materialization() {
        let entity = ErEntityRenderModel {
            id: "OWNER".to_string(),
            label: "Owner<&中".to_string(),
            attributes: vec![ErAttributeRenderModel {
                ty: "string".to_string(),
                name: "owner<&中".to_string(),
                keys: vec!["PK".to_string(), "FK".to_string()],
                comment: "record<&中".to_string(),
            }],
            ..ErEntityRenderModel::default()
        };

        for mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let base = AsciiRenderOptions::unicode().with_color_mode(mode);
            let base_policy = unbounded_policy();
            let measured_probe = Cell::new(false);
            let (measured, _, _) =
                render_er_section_fixture(&entity, &base, base_policy, &measured_probe);
            let expected = measured.expect("unbounded ER section should render");
            assert!(measured_probe.get(), "mode={mode:?}");
            if mode == AsciiColorMode::Html {
                assert!(expected.contains("Owner&lt;&amp;中"), "mode={mode:?}");
                assert!(expected.contains("owner&lt;&amp;中"), "mode={mode:?}");
            } else {
                assert!(expected.contains("Owner<&中"), "mode={mode:?}");
                assert!(expected.contains("owner<&中"), "mode={mode:?}");
            }

            let exact_policy = base_policy
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len())
                .expect("exact ER output limit should be valid");
            let exact_probe = Cell::new(false);
            let (rendered, _, _) =
                render_er_section_fixture(&entity, &base, exact_policy, &exact_probe);
            assert_eq!(
                rendered.expect("exact ER output should materialize"),
                expected,
                "mode={mode:?}"
            );
            assert!(exact_probe.get(), "mode={mode:?}");

            let below_policy = base_policy
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len() - 1)
                .expect("max-minus-one ER output limit should be valid");
            let below_probe = Cell::new(false);
            let (error, before, after) =
                render_er_section_fixture(&entity, &base, below_policy, &below_probe);
            assert!(!below_probe.get(), "mode={mode:?}");
            assert_eq!(after, before, "mode={mode:?}");
            assert!(
                matches!(
                    error.expect_err("max-minus-one ER output must reject"),
                    AsciiError::ResourceLimitExceeded(details)
                        if details.limit == AsciiResourceLimitId::MaxOutputBytes
                            && details.actual == expected.len()
                            && details.max == expected.len() - 1
                ),
                "mode={mode:?}"
            );
        }
    }

    #[test]
    fn er_attribute_composition_admits_exact_work_before_materializing() {
        let attribute = composite_attribute();
        let expected = concat!(
            "type(bytes=6)=\"string\" name(bytes=8)=\"owner_id\" ",
            "keys=[bytes=2 \"PK\", bytes=2 \"FK\"] ",
            "comment(bytes=12)=\"record owner\"\n",
        );
        let prior_work = 2;
        let prior_document = 3;
        let expected_document = expected.len() - 1;

        let mut measured = ResourceContext::new(unbounded_policy());
        measured
            .charge_usage(prior_work, prior_document)
            .expect("the test checkpoint should fit");
        let measured_probe = Cell::new(false);
        let rendered =
            attribute_text_with_probe(&attribute, &mut measured, || measured_probe.set(true))
                .expect("unbounded ER attribute composition should render");
        let exact_work = measured.layout_work_used();
        assert_eq!(rendered, expected);
        assert!(measured_probe.get());
        assert!(exact_work > prior_work);
        assert_eq!(
            measured.document_cells_used(),
            prior_document + expected_document
        );

        let exact_policy = unbounded_policy()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact ER attribute work limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        exact_resources
            .charge_usage(prior_work, prior_document)
            .expect("the exact checkpoint should fit");
        let exact_probe = Cell::new(false);
        let exact_rendered =
            attribute_text_with_probe(&attribute, &mut exact_resources, || exact_probe.set(true))
                .expect("exact ER attribute work should permit materialization");
        assert_eq!(exact_rendered, expected);
        assert!(exact_probe.get());
        assert_eq!(exact_resources.layout_work_used(), exact_work);
        assert_eq!(
            exact_resources.document_cells_used(),
            prior_document + expected_document
        );

        let below_policy = unbounded_policy()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one ER attribute work limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        below_resources
            .charge_usage(prior_work, prior_document)
            .expect("the below-limit checkpoint should fit");
        let below_probe = Cell::new(false);
        let error =
            attribute_text_with_probe(&attribute, &mut below_resources, || below_probe.set(true))
                .expect_err("max-minus-one ER attribute work must reject before materialization");
        assert!(!below_probe.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == exact_work
                    && details.max == exact_work - 1
        ));
        assert_eq!(below_resources.layout_work_used(), prior_work);
        assert_eq!(below_resources.document_cells_used(), prior_document);
    }

    #[test]
    fn er_attribute_composition_admits_exact_output_before_materializing() {
        let attribute = composite_attribute();
        let expected = concat!(
            "type(bytes=6)=\"string\" name(bytes=8)=\"owner_id\" ",
            "keys=[bytes=2 \"PK\", bytes=2 \"FK\"] ",
            "comment(bytes=12)=\"record owner\"\n",
        );
        let prior_work = 2;
        let prior_document = 3;
        let expected_document = expected.len() - 1;

        let exact_policy = unbounded_policy()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len())
            .expect("exact ER attribute output limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        exact_resources
            .charge_usage(prior_work, prior_document)
            .expect("the exact checkpoint should fit");
        let exact_probe = Cell::new(false);
        let rendered =
            attribute_text_with_probe(&attribute, &mut exact_resources, || exact_probe.set(true))
                .expect("exact ER attribute output should permit materialization");
        assert_eq!(rendered, expected);
        assert!(exact_probe.get());
        assert!(exact_resources.layout_work_used() > prior_work);
        assert_eq!(
            exact_resources.document_cells_used(),
            prior_document + expected_document
        );

        let below_policy = unbounded_policy()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len() - 1)
            .expect("max-minus-one ER attribute output limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        below_resources
            .charge_usage(prior_work, prior_document)
            .expect("the below-limit checkpoint should fit");
        let below_probe = Cell::new(false);
        let error =
            attribute_text_with_probe(&attribute, &mut below_resources, || below_probe.set(true))
                .expect_err("max-minus-one ER attribute output must reject before materialization");
        assert!(!below_probe.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxOutputBytes
                    && details.actual == expected.len()
                    && details.max == expected.len() - 1
        ));
        assert_eq!(below_resources.layout_work_used(), prior_work);
        assert_eq!(below_resources.document_cells_used(), prior_document);
    }

    #[test]
    fn er_attribute_frames_a_leading_combining_grapheme_at_exact_limit() {
        let attribute = ErAttributeRenderModel {
            ty: "string".to_string(),
            name: "\u{301}".to_string(),
            keys: Vec::new(),
            comment: String::new(),
        };
        let exact_policy = unbounded_policy()
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 2)
            .expect("grapheme resource limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        exact_resources
            .charge_usage(3, 5)
            .expect("test checkpoint should fit");
        let exact_probe = Cell::new(false);
        let rendered =
            attribute_text_with_probe(&attribute, &mut exact_resources, || exact_probe.set(true))
                .expect("the exact raw grapheme limit should permit framed output");
        assert!(exact_probe.get());
        assert!(rendered.contains(r#"name(bytes=2)="\u{301}""#));

        let below_policy = unbounded_policy()
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 1)
            .expect("grapheme resource limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        below_resources
            .charge_usage(3, 5)
            .expect("test checkpoint should fit");
        let materialized = Cell::new(false);
        let error =
            attribute_text_with_probe(&attribute, &mut below_resources, || materialized.set(true))
                .expect_err(
                    "one byte below the authored grapheme must reject before materialization",
                );
        assert!(!materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGraphemeBytes
                    && details.actual == 2
                    && details.max == 1
        ));
        assert_eq!(below_resources.layout_work_used(), 3);
        assert_eq!(below_resources.document_cells_used(), 5);
    }

    #[test]
    fn horizontal_er_strip_checks_grid_and_layout_work_before_allocating_rows() {
        let boxes = vec![
            RelationGraphBox::new("A".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("B".to_string(), vec!["B".to_string()], 1),
        ];

        for limit in [
            AsciiResourceLimitId::MaxGridCells,
            AsciiResourceLimitId::MaxLayoutWorkUnits,
        ] {
            let resources = resources_with_limit(limit, 5);
            let error = relation_graph::render_horizontal_box_strip_lines(
                &boxes,
                RelationDirection::LeftRight.transform(),
                ER_LEVEL_HORIZONTAL_GAP,
                TerminalWidthProfile::Unicode,
                &resources,
            )
            .expect_err("a six-cell horizontal strip must reject a five-cell/work budget");
            let AsciiError::ResourceLimitExceeded(details) = error else {
                panic!("expected a typed horizontal resource error, got {error:?}");
            };
            assert_eq!(details.limit, limit);
            assert_eq!(details.actual, 6);
            assert_eq!(details.max, 5);
        }
    }
}
