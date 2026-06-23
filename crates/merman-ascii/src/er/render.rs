use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::relation_graph::{
    LayeredRelationEdge, LayeredRelationError, LayeredRelationRoutePlan, RelationGraphLine,
    RelationLineChars, RelationOverlay, RelationParallelPlan, RelationStackPlan,
};
use crate::text::display_width;
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
}

impl ErCharset {
    fn for_options(options: &AsciiRenderOptions) -> Self {
        match options.charset {
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
            },
        }
    }
}

type RenderedEntityBox = RelationGraphBox;

pub(crate) fn render_er_diagram(
    model: &ErDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    if model.entities.is_empty() {
        return Ok(String::new());
    }

    let charset = ErCharset::for_options(options);
    let boxes = model
        .entities
        .values()
        .map(|entity| render_entity_box(entity, options, charset))
        .collect::<Vec<_>>();

    if model.relationships.is_empty() {
        return Ok(relation_graph::render_stacked_boxes_with_options(
            &boxes, options,
        ));
    }

    render_er_components(&boxes, &model.relationships, options, charset)
}

fn render_er_component(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    options: &AsciiRenderOptions,
    charset: ErCharset,
) -> Result<String> {
    if relationships.is_empty() {
        return Ok(relation_graph::render_stacked_boxes_with_options(
            boxes, options,
        ));
    }
    if is_same_endpoint_parallel_relationship(relationships) {
        return render_parallel_vertical_relationships(boxes, relationships, options, charset);
    }
    if relationships.len() == 1 {
        let relationship = &relationships[0];
        let top = find_box(boxes, &relationship.entity_a)?;
        let bottom = find_box(boxes, &relationship.entity_b)?;

        return render_vertical_relationship(top, bottom, relationship, options, charset);
    }

    render_layered_relationships(boxes, relationships, options, charset)
}

fn render_er_components(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    options: &AsciiRenderOptions,
    charset: ErCharset,
) -> Result<String> {
    let edges = relationships
        .iter()
        .map(er_layered_edge)
        .collect::<Vec<_>>();
    let components =
        relation_graph::relation_components(boxes, &edges).map_err(er_layered_error)?;
    if components.len() == 1 {
        return render_er_component(boxes, relationships, options, charset);
    }

    let mut rendered = Vec::new();
    for component in components {
        let component_boxes = component
            .boxes()
            .iter()
            .map(|relation_box| (*relation_box).clone())
            .collect::<Vec<_>>();
        let component_relationships = component
            .edge_indices()
            .iter()
            .map(|index| relationships[*index].clone())
            .collect::<Vec<_>>();
        rendered.push(render_er_component(
            &component_boxes,
            &component_relationships,
            options,
            charset,
        )?);
    }

    Ok(rendered.join("\n"))
}

fn render_entity_box(
    entity: &ErEntityRenderModel,
    options: &AsciiRenderOptions,
    charset: ErCharset,
) -> RenderedEntityBox {
    let sections = entity_sections(entity);
    let content_width = content_width(&sections, options.box_border_padding);
    let mut out = Vec::new();

    out.push(border_line(
        charset.top_left,
        charset.top_right,
        charset.horizontal,
        content_width,
    ));
    for (section_index, section) in sections.iter().enumerate() {
        if section_index > 0 {
            out.push(border_line(
                charset.separator_left,
                charset.separator_right,
                charset.horizontal,
                content_width,
            ));
        }
        for line in section {
            out.push(content_line(
                line,
                content_width,
                options.box_border_padding,
                charset,
            ));
        }
    }
    out.push(border_line(
        charset.bottom_left,
        charset.bottom_right,
        charset.horizontal,
        content_width,
    ));

    RenderedEntityBox::new_with_lines(entity.id.clone(), out, content_width + 2)
}

fn entity_sections(entity: &ErEntityRenderModel) -> Vec<Vec<String>> {
    let label = if entity.alias.is_empty() {
        entity.label.clone()
    } else {
        entity.alias.clone()
    };
    let mut sections = vec![vec![label]];

    let attributes = entity
        .attributes
        .iter()
        .map(attribute_text)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if !attributes.is_empty() {
        sections.push(attributes);
    }

    sections
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
        text.push_str(&attribute.keys.join(","));
    }
    if !attribute.comment.is_empty() {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&attribute.comment);
    }
    text
}

fn content_width(sections: &[Vec<String>], padding: usize) -> usize {
    let max_line_width = sections
        .iter()
        .flat_map(|section| section.iter())
        .map(|line| display_width(line))
        .max()
        .unwrap_or(0)
        .max(1);
    max_line_width + padding.saturating_mul(2)
}

fn border_line(
    left: char,
    right: char,
    horizontal: char,
    content_width: usize,
) -> RelationGraphLine {
    RelationGraphLine::box_border(
        left,
        right,
        horizontal,
        content_width,
        AsciiColorRole::NodeBorder,
    )
}

fn content_line(
    text: &str,
    content_width: usize,
    padding: usize,
    charset: ErCharset,
) -> RelationGraphLine {
    RelationGraphLine::box_content(
        text,
        content_width,
        padding,
        charset.vertical,
        AsciiColorRole::NodeBorder,
        AsciiColorRole::Text,
    )
}

fn find_box<'a>(boxes: &'a [RenderedEntityBox], id: &str) -> Result<&'a RenderedEntityBox> {
    relation_graph::find_box(boxes, id).ok_or(AsciiError::UnsupportedFeature {
        diagram_type: "er",
        feature: "relationships with missing endpoint entities",
    })
}

fn render_vertical_relationship(
    top: &RenderedEntityBox,
    bottom: &RenderedEntityBox,
    relationship: &ErRelationshipRenderModel,
    options: &AsciiRenderOptions,
    charset: ErCharset,
) -> Result<String> {
    let top_cardinality = cardinality_marker(&relationship.rel_spec.card_b)?;
    let bottom_cardinality = cardinality_marker(&relationship.rel_spec.card_a)?;
    let line = relationship_line(&relationship.rel_spec.rel_type, charset)?;
    let label = relationship.role_a.trim();
    let label_half_width = if label.is_empty() {
        0
    } else {
        display_width(label) / 2
    };
    let plan = RelationStackPlan::from_centered_rows(
        top,
        bottom,
        &[
            display_width(top_cardinality) / 2,
            display_width(bottom_cardinality) / 2,
            label_half_width,
        ],
        |center| er_relationship_rows(top_cardinality, bottom_cardinality, line, label, center),
    );

    Ok(plan.render_with_options(options))
}

fn er_relationship_rows(
    top_cardinality: &str,
    bottom_cardinality: &str,
    line: char,
    label: &str,
    center: usize,
) -> Vec<RelationGraphLine> {
    let mut relation_lines = Vec::new();
    relation_lines.push(relation_graph::centered_text_line_with_role(
        top_cardinality,
        center,
        AsciiColorRole::EdgeArrow,
    ));
    if !label.is_empty() {
        relation_lines.push(relation_graph::centered_text_line_with_role(
            label,
            center,
            AsciiColorRole::EdgeLabel,
        ));
    }
    relation_lines.push(relation_graph::marker_line_with_role(
        line,
        center,
        AsciiColorRole::EdgeLine,
    ));
    relation_lines.push(relation_graph::centered_text_line_with_role(
        bottom_cardinality,
        center,
        AsciiColorRole::EdgeArrow,
    ));
    relation_lines
}

type PlacedEntityBox<'a> = relation_graph::PlacedRelationGraphBox<'a>;

fn is_same_endpoint_parallel_relationship(relationships: &[ErRelationshipRenderModel]) -> bool {
    let Some(first) = relationships.first() else {
        return false;
    };
    relationships.len() > 1
        && relationships.iter().all(|relationship| {
            relationship.entity_a == first.entity_a && relationship.entity_b == first.entity_b
        })
}

fn render_parallel_vertical_relationships(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    options: &AsciiRenderOptions,
    charset: ErCharset,
) -> Result<String> {
    let first = &relationships[0];
    let top = find_box(boxes, &first.entity_a)?;
    let bottom = find_box(boxes, &first.entity_b)?;
    let lanes = relationships
        .iter()
        .map(|relationship| parallel_er_lane_rows(relationship, charset))
        .collect::<Result<Vec<_>>>()?;
    let plan = RelationParallelPlan::new(top, bottom, lanes, 2);

    Ok(plan.render_with_options(options))
}

fn parallel_er_lane_rows(
    relationship: &ErRelationshipRenderModel,
    charset: ErCharset,
) -> Result<Vec<RelationGraphLine>> {
    let top_cardinality = cardinality_marker(&relationship.rel_spec.card_b)?;
    let bottom_cardinality = cardinality_marker(&relationship.rel_spec.card_a)?;
    let line = relationship_line(&relationship.rel_spec.rel_type, charset)?;
    Ok(vec![
        RelationGraphLine::with_role(top_cardinality.to_string(), AsciiColorRole::EdgeArrow),
        RelationGraphLine::with_role(
            relationship.role_a.trim().to_string(),
            AsciiColorRole::EdgeLabel,
        ),
        RelationGraphLine::with_role(line.to_string(), AsciiColorRole::EdgeLine),
        RelationGraphLine::with_role(bottom_cardinality.to_string(), AsciiColorRole::EdgeArrow),
    ])
}

fn render_layered_relationships(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    options: &AsciiRenderOptions,
    charset: ErCharset,
) -> Result<String> {
    let edges = relationships
        .iter()
        .map(er_layered_edge)
        .collect::<Vec<_>>();
    let plan = relation_graph::plan_layered_relation_boxes(boxes, &edges, ER_LEVEL_HORIZONTAL_GAP)
        .map_err(er_layered_error)?;
    let width = plan.width();
    let height = plan.height();
    let actual_cells = width.saturating_mul(height);
    if actual_cells > options.max_grid_cells {
        return Err(AsciiError::RenderLimitExceeded {
            actual: actual_cells,
            limit: options.max_grid_cells,
        });
    }

    let mut canvas = Canvas::new(width, height);
    for placed_box in plan.placed_boxes() {
        placed_box.draw_at(&mut canvas);
    }

    let placed_by_id = plan
        .placed_boxes()
        .iter()
        .map(|placed_box| (placed_box.id(), placed_box))
        .collect::<HashMap<_, _>>();
    let lane_offsets =
        relation_graph::parallel_relation_lane_offsets(relationships.iter().map(|relationship| {
            (
                relationship.entity_a.as_str(),
                relationship.entity_b.as_str(),
            )
        }));
    let mut draw_order = relationships
        .iter()
        .zip(lane_offsets)
        .enumerate()
        .collect::<Vec<_>>();
    draw_order.sort_by_key(|(index, (_, lane_offset))| (lane_offset.unsigned_abs(), *index));
    for (_, (relationship, lane_offset)) in draw_order {
        draw_layered_relationship(
            &mut canvas,
            plan.placed_boxes(),
            &placed_by_id,
            relationship,
            lane_offset,
            charset,
        )?;
    }

    Ok(canvas.finish_trimmed_with_options(options))
}

fn er_layered_edge(relationship: &ErRelationshipRenderModel) -> LayeredRelationEdge<'_> {
    let label = relationship.role_a.trim();
    LayeredRelationEdge::new(
        &relationship.entity_a,
        &relationship.entity_b,
        !label.is_empty(),
        display_width(label) / 2,
    )
}

fn er_layered_error(error: LayeredRelationError) -> AsciiError {
    let feature = match error {
        LayeredRelationError::MissingEndpoint => "relationships with missing endpoint entities",
        LayeredRelationError::UnrelatedBoxes => "ER relationship layouts with unrelated entities",
        LayeredRelationError::Cyclic => "cyclic ER relationship layouts",
        LayeredRelationError::Crossing => "crossing ER relationship layouts",
    };
    AsciiError::UnsupportedFeature {
        diagram_type: "er",
        feature,
    }
}

fn draw_layered_relationship(
    canvas: &mut Canvas,
    placed_boxes: &[PlacedEntityBox<'_>],
    placed_by_id: &HashMap<&str, &PlacedEntityBox<'_>>,
    relationship: &ErRelationshipRenderModel,
    lane_offset: isize,
    charset: ErCharset,
) -> Result<()> {
    let Some(top) = placed_by_id.get(relationship.entity_a.as_str()) else {
        return Ok(());
    };
    let Some(bottom) = placed_by_id.get(relationship.entity_b.as_str()) else {
        return Ok(());
    };
    let top_cardinality = cardinality_marker(&relationship.rel_spec.card_b)?;
    let bottom_cardinality = cardinality_marker(&relationship.rel_spec.card_a)?;
    let vertical = relationship_line(&relationship.rel_spec.rel_type, charset)?;
    let horizontal = relationship_horizontal_line(&relationship.rel_spec.rel_type, charset)?;
    let label = relationship.role_a.trim();
    let relation_chars = relation_line_chars(charset);
    let Some(geometry) = relation_graph::plan_layered_relation_route(
        placed_boxes,
        top,
        bottom,
        lane_offset,
        2,
        2,
        2,
        1,
    ) else {
        return Ok(());
    };

    let mut overlays = Vec::new();
    overlays.push(RelationOverlay::text(
        geometry.from_x(),
        geometry.from_y() + 1,
        top_cardinality.to_string(),
        AsciiColorRole::EdgeArrow,
    ));
    if !label.is_empty() {
        let label_y = geometry.from_y().saturating_add(2).min(geometry.route_y());
        overlays.push(RelationOverlay::text(
            (geometry.from_x() + geometry.to_x()) / 2,
            label_y,
            label.to_string(),
            AsciiColorRole::EdgeLabel,
        ));
    }
    overlays.push(RelationOverlay::text(
        geometry.to_x(),
        geometry.to_y().saturating_sub(1),
        bottom_cardinality.to_string(),
        AsciiColorRole::EdgeArrow,
    ));

    LayeredRelationRoutePlan::new(geometry, vertical, horizontal, relation_chars, overlays)
        .draw_at(canvas);

    Ok(())
}

fn cardinality_marker(cardinality: &str) -> Result<&'static str> {
    match cardinality {
        "ONLY_ONE" => Ok("||"),
        "ZERO_OR_ONE" => Ok("o|"),
        "ONE_OR_MORE" => Ok("|{"),
        "ZERO_OR_MORE" => Ok("o{"),
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
