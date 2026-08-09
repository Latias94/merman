use crate::color::AsciiColorRole;
use crate::options::{AsciiCharset, AsciiRenderOptions, TerminalWidthProfile};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::relation_graph::{
    HorizontalRelationEndpoint, HorizontalRelationMarker, HorizontalRelationStyle,
    LayeredRelationEdge, LayeredRelationError, LayeredRelationRouteStyle, RelationGraphBoxStyle,
    RelationGraphHorizontalDirection, RelationGraphLabel, RelationGraphLine,
    RelationGraphSummaryRow, RelationLineChars, RelationOverlay, RelationParallelPlan,
    RelationPortSide, RelationRegionPlan, RelationSelfLoopMetrics, RelationStackPlan,
    RelationSummaryPaintPlan,
};
#[cfg(test)]
use crate::resource::AsciiResourceLimitId;
use crate::resource::{AsciiResourceLimitPhase, LogicalExtent, ResourceContext};
use crate::safe_text::{charge_text_layout, terminal_char_display_width};
use crate::text::display_width_with_profile;
use crate::{AsciiError, Result};
use merman_core::diagrams::er::{
    ErAttributeRenderModel, ErDiagramRenderModel, ErEntityRenderModel, ErRelationshipRenderModel,
};
use std::collections::HashMap;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErDirection {
    TopDown,
    BottomUp,
    LeftRight,
    RightLeft,
}

impl ErDirection {
    fn try_from_model(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "" | "TB" | "TD" => Ok(Self::TopDown),
            "BT" => Ok(Self::BottomUp),
            "LR" => Ok(Self::LeftRight),
            "RL" => Ok(Self::RightLeft),
            _ => Err(AsciiError::UnsupportedFeature {
                diagram_type: "er",
                feature: "unknown ER diagram directions",
            }),
        }
    }

    fn is_horizontal(self) -> bool {
        matches!(self, Self::LeftRight | Self::RightLeft)
    }

    fn is_reversed(self) -> bool {
        matches!(self, Self::BottomUp | Self::RightLeft)
    }

    fn horizontal_direction(self) -> RelationGraphHorizontalDirection {
        match self {
            Self::RightLeft => RelationGraphHorizontalDirection::RightLeft,
            Self::TopDown | Self::BottomUp | Self::LeftRight => {
                RelationGraphHorizontalDirection::LeftRight
            }
        }
    }
}

struct ErRelationComponentAdapter<'a> {
    charset: ErCharset,
    entity_labels: &'a HashMap<String, String>,
    width_profile: TerminalWidthProfile,
    direction: ErDirection,
}

struct ErRelationLayout<'a> {
    relationship: &'a ErRelationshipRenderModel,
    top_id: &'a str,
    bottom_id: &'a str,
    top_cardinality: &'a str,
    bottom_cardinality: &'a str,
    label: Option<RelationGraphLabel>,
}

impl ErRelationLayout<'_> {
    fn apply_direction(&mut self, direction: ErDirection) {
        if direction != ErDirection::BottomUp {
            return;
        }
        std::mem::swap(&mut self.top_id, &mut self.bottom_id);
        std::mem::swap(&mut self.top_cardinality, &mut self.bottom_cardinality);
    }
}

pub(crate) fn render_er_diagram(
    model: &ErDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    if model.entities.is_empty() {
        if !model.relationships.is_empty() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "er",
                feature: "relationships with missing endpoint entities",
            });
        }
        return Ok(String::new());
    }

    let mut resources = ResourceContext::new(options.resources);
    preflight_er_text(model, &mut resources)?;
    charge_er_model_work(model, &mut resources)?;
    let charset = ErCharset::for_options(options);
    let direction = ErDirection::try_from_model(&model.direction)?;
    let mut boxes = Vec::new();
    boxes
        .try_reserve_exact(model.entities.len())
        .map_err(|_| layout_allocation_failed())?;
    for entity in model.entities.values() {
        boxes.push(render_entity_box(entity, options, charset, &mut resources)?);
    }
    if model.relationships.is_empty() {
        if direction.is_horizontal() {
            let lines = relation_graph::render_horizontal_box_strip_lines(
                &boxes,
                direction.horizontal_direction(),
                ER_LEVEL_HORIZONTAL_GAP,
                options.terminal_width_profile,
                &resources,
            )?;
            return render_er_document_lines(lines, options, &mut resources);
        }
        if direction.is_reversed() {
            let lines = relation_graph::stacked_box_lines_ordered(
                &boxes,
                options.terminal_width_profile,
                true,
                &mut resources,
            )?;
            return render_er_document_lines(lines, options, &mut resources);
        }
        return relation_graph::render_stacked_boxes_with_options(&boxes, options, &mut resources);
    }

    let mut entity_labels = HashMap::new();
    entity_labels
        .try_reserve(model.entities.len())
        .map_err(|_| layout_allocation_failed())?;
    entity_labels.extend(
        model
            .entities
            .values()
            .map(|entity| (entity.id.clone(), entity_display_label(entity).to_string())),
    );

    render_er_components(
        &boxes,
        &model.relationships,
        &entity_labels,
        options,
        charset,
        direction,
        &mut resources,
    )
}

fn render_er_components(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    entity_labels: &HashMap<String, String>,
    options: &AsciiRenderOptions,
    charset: ErCharset,
    direction: ErDirection,
    resources: &mut ResourceContext,
) -> Result<String> {
    let mut layouts = Vec::new();
    layouts
        .try_reserve_exact(relationships.len())
        .map_err(|_| layout_allocation_failed())?;
    for relationship in relationships {
        let mut layout = ErRelationLayout {
            relationship,
            top_id: relationship.entity_a.as_str(),
            bottom_id: relationship.entity_b.as_str(),
            top_cardinality: relationship.rel_spec.card_b.as_str(),
            bottom_cardinality: relationship.rel_spec.card_a.as_str(),
            label: RelationGraphLabel::try_new(
                &relationship.role_a,
                options.terminal_width_profile,
                resources,
            )?,
        };
        layout.apply_direction(direction);
        layouts.push(layout);
    }
    let adapter = ErRelationComponentAdapter {
        charset,
        entity_labels,
        width_profile: options.terminal_width_profile,
        direction,
    };
    if direction.is_horizontal() {
        let lines = render_horizontal_er_component_lines(
            boxes, &layouts, direction, options, &adapter, resources,
        )?;
        return render_er_document_lines(lines, options, resources);
    }
    relation_graph::render_relation_components(boxes, &layouts, options, resources, &adapter)
}

fn render_entity_box(
    entity: &ErEntityRenderModel,
    options: &AsciiRenderOptions,
    charset: ErCharset,
    resources: &mut ResourceContext,
) -> Result<RenderedEntityBox> {
    let sections = entity_sections(entity)?;
    render_box_sections(entity.id.clone(), sections, options, charset, resources)
}

fn render_box_sections(
    id: String,
    sections: Vec<Vec<String>>,
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
    relation_graph::RelationGraphBox::from_sections(
        id,
        &sections,
        options.box_border_padding,
        style,
        options.terminal_width_profile,
        resources,
    )
}

fn entity_sections(entity: &ErEntityRenderModel) -> Result<Vec<Vec<String>>> {
    let mut header = Vec::new();
    header
        .try_reserve_exact(1)
        .map_err(|_| layout_allocation_failed())?;
    header.push(entity_display_label(entity).to_string());
    let mut sections = Vec::new();
    sections
        .try_reserve_exact(2)
        .map_err(|_| layout_allocation_failed())?;
    sections.push(header);

    let mut attributes = Vec::new();
    attributes
        .try_reserve_exact(entity.attributes.len())
        .map_err(|_| layout_allocation_failed())?;
    attributes.extend(
        entity
            .attributes
            .iter()
            .map(attribute_text)
            .filter(|line| !line.is_empty()),
    );
    if !attributes.is_empty() {
        sections.push(attributes);
    }

    Ok(sections)
}

fn entity_display_label(entity: &ErEntityRenderModel) -> &str {
    if entity.alias.is_empty() {
        &entity.label
    } else {
        &entity.alias
    }
}

fn attribute_text(attribute: &ErAttributeRenderModel) -> String {
    let mut parts = Vec::new();
    if !attribute.ty.is_empty() {
        parts.push(attribute.ty.as_str());
    }
    if !attribute.name.is_empty() {
        parts.push(attribute.name.as_str());
    }
    let mut text = parts.join(" ");
    if !attribute.keys.is_empty() {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str("[keys: ");
        text.push_str(&attribute.keys.join(","));
        text.push(']');
    }
    if !attribute.comment.is_empty() {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str("[comment: ");
        text.push_str(&attribute.comment);
        text.push(']');
    }
    text
}

fn render_er_document_lines(
    lines: Vec<RelationGraphLine>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    relation_graph::render_lines_with_options(&lines, options, resources)
}

fn render_horizontal_er_component_lines(
    boxes: &[RenderedEntityBox],
    layouts: &[ErRelationLayout<'_>],
    direction: ErDirection,
    options: &AsciiRenderOptions,
    adapter: &ErRelationComponentAdapter<'_>,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    relation_graph::render_horizontal_relation_components(
        boxes,
        layouts,
        direction.horizontal_direction(),
        options,
        resources,
        adapter,
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
    let top_cardinality = cardinality_marker(layout.top_cardinality, charset)?;
    let bottom_cardinality = cardinality_marker(layout.bottom_cardinality, charset)?;
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

fn plan_parallel_vertical_relationships<'plan>(
    boxes: Vec<&'plan RenderedEntityBox>,
    layouts: Vec<&'plan ErRelationLayout<'_>>,
    options: &AsciiRenderOptions,
    charset: ErCharset,
    width_profile: TerminalWidthProfile,
    entity_labels: &HashMap<String, String>,
    resources: &mut ResourceContext,
) -> Result<RelationRegionPlan<'plan>> {
    let first = layouts
        .first()
        .copied()
        .ok_or(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "empty parallel ER relationship layout",
        })?;
    let top = relation_graph::find_box_ref(&boxes, first.top_id).ok_or(
        AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "relationships with missing endpoint entities",
        },
    )?;
    let bottom = relation_graph::find_box_ref(&boxes, first.bottom_id).ok_or(
        AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "relationships with missing endpoint entities",
        },
    )?;
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
            rows.push(er_relationship_summary_row(layout, entity_labels, charset)?);
        }
        return Ok(RelationRegionPlan::Summary(
            RelationSummaryPaintPlan::stacked(boxes, rows, Some(reason), options, resources)?,
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
    let top_cardinality = cardinality_marker(layout.top_cardinality, charset)?;
    let bottom_cardinality = cardinality_marker(layout.bottom_cardinality, charset)?;
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
    let top_cardinality = cardinality_marker(layout.top_cardinality, charset)?;
    let bottom_cardinality = cardinality_marker(layout.bottom_cardinality, charset)?;
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

fn er_relationship_summary_row(
    layout: &ErRelationLayout<'_>,
    entity_labels: &HashMap<String, String>,
    charset: ErCharset,
) -> Result<RelationGraphSummaryRow> {
    let relationship = layout.relationship;
    // Summary rows use Mermaid's horizontal, left-to-right token orientation;
    // the routed renderer passes the physical port side explicitly, so mirror
    // that mapping here instead of reusing vertical marker glyphs.
    let left_cardinality =
        horizontal_cardinality_marker(layout.top_cardinality, RelationPortSide::Right, charset)?;
    let right_cardinality =
        horizontal_cardinality_marker(layout.bottom_cardinality, RelationPortSide::Left, charset)?;
    let relation = er_relationship_summary_line(&relationship.rel_spec.rel_type, charset)?;
    Ok(RelationGraphSummaryRow::new(
        relationship_label(entity_labels, layout.top_id),
        format!("{left_cardinality}{relation}{right_cardinality}"),
        relationship_label(entity_labels, layout.bottom_id),
    )
    .with_label(layout.label.as_ref()))
}

fn er_relationship_summary_row_for_reason(
    layout: &ErRelationLayout<'_>,
    entity_labels: &HashMap<String, String>,
    charset: ErCharset,
    reason: relation_graph::LayeredRelationSummaryReason,
) -> Result<RelationGraphSummaryRow> {
    match reason {
        relation_graph::LayeredRelationSummaryReason::Crossing
        | relation_graph::LayeredRelationSummaryReason::RouteCollision
        | relation_graph::LayeredRelationSummaryReason::OverlayCollision => {
            er_relationship_summary_row(layout, entity_labels, charset)
        }
    }
}

fn relationship_label<'a>(entity_labels: &'a HashMap<String, String>, id: &'a str) -> &'a str {
    entity_labels.get(id).map(String::as_str).unwrap_or(id)
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

impl<'adapter, 'relation> relation_graph::RelationComponentAdapter<ErRelationLayout<'relation>>
    for ErRelationComponentAdapter<'adapter>
{
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
            self.direction.is_horizontal(),
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
            self.direction.is_horizontal(),
            resources,
        )
    }

    fn horizontal_relation_style(
        &self,
        layout: &ErRelationLayout<'_>,
        source_side: RelationPortSide,
        target_side: RelationPortSide,
        _resources: &ResourceContext,
    ) -> Result<HorizontalRelationStyle> {
        let source_marker = HorizontalRelationMarker::new(
            horizontal_cardinality_marker(layout.top_cardinality, source_side, self.charset)?,
            AsciiColorRole::EdgeArrow,
            self.width_profile,
        );
        let target_marker = HorizontalRelationMarker::new(
            horizontal_cardinality_marker(layout.bottom_cardinality, target_side, self.charset)?,
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
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationOverlay>> {
        resources.charge_layout_work(3)?;
        let top_cardinality = cardinality_marker(layout.top_cardinality, self.charset)?;
        let bottom_cardinality = cardinality_marker(layout.bottom_cardinality, self.charset)?;

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
            let center_x =
                resources.checked_grid_add(geometry.source_x(), geometry.target_x())? / 2;
            overlays.push(RelationOverlay::label(
                center_x,
                geometry.label_y_after_source(),
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
        layout: &'plan ErRelationLayout<'relation>,
        resources: &mut ResourceContext,
    ) -> Result<RelationRegionPlan<'plan>> {
        let top = relation_graph::find_box_ref(boxes, layout.top_id).ok_or(
            AsciiError::UnsupportedFeature {
                diagram_type: "er",
                feature: "relationships with missing endpoint entities",
            },
        )?;
        let bottom = relation_graph::find_box_ref(boxes, layout.bottom_id).ok_or(
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
        layouts: Vec<&'plan ErRelationLayout<'relation>>,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
    ) -> Result<RelationRegionPlan<'plan>> {
        plan_parallel_vertical_relationships(
            boxes,
            layouts,
            options,
            self.charset,
            self.width_profile,
            self.entity_labels,
            resources,
        )
    }

    fn build_summary_row(
        &self,
        layout: &ErRelationLayout<'_>,
        reason: relation_graph::LayeredRelationSummaryReason,
    ) -> Result<RelationGraphSummaryRow> {
        er_relationship_summary_row_for_reason(layout, self.entity_labels, self.charset, reason)
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
    if horizontal {
        Ok((
            horizontal_cardinality_marker(
                layout.top_cardinality,
                RelationPortSide::Right,
                charset,
            )?,
            horizontal_cardinality_marker(
                layout.bottom_cardinality,
                RelationPortSide::Left,
                charset,
            )?,
        ))
    } else {
        Ok((
            cardinality_marker(layout.top_cardinality, charset)?,
            cardinality_marker(layout.bottom_cardinality, charset)?,
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
    side: RelationPortSide,
    charset: ErCharset,
) -> Result<&'static str> {
    match (cardinality, side) {
        ("ONLY_ONE", _) => Ok("||"),
        ("ZERO_OR_ONE", RelationPortSide::Right) => Ok("|o"),
        ("ZERO_OR_ONE", RelationPortSide::Left) => Ok("o|"),
        ("ONE_OR_MORE", RelationPortSide::Right) => Ok("}|"),
        ("ONE_OR_MORE", RelationPortSide::Left) => Ok("|{"),
        ("ZERO_OR_MORE", RelationPortSide::Right) => Ok("}o"),
        ("ZERO_OR_MORE", RelationPortSide::Left) => Ok("o{"),
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
    use crate::resource::AsciiResourcePolicy;

    fn resources_with_limit(id: AsciiResourceLimitId, max: usize) -> ResourceContext {
        let policy = AsciiResourcePolicy::default()
            .with_limit(id, max)
            .expect("resource test limit should be valid");
        ResourceContext::new(policy)
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
                RelationGraphHorizontalDirection::LeftRight,
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
