use crate::canvas::Canvas;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::relation_graph::{LayeredRelationEdge, LayeredRelationError};
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
        return Ok(relation_graph::render_stacked_boxes(&boxes));
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
        return Ok(relation_graph::render_stacked_boxes(boxes));
    }
    if is_same_endpoint_parallel_relationship(relationships) {
        return render_parallel_vertical_relationships(boxes, relationships, charset);
    }
    if relationships.len() == 1 {
        let relationship = &relationships[0];
        let top = find_box(boxes, &relationship.entity_a)?;
        let bottom = find_box(boxes, &relationship.entity_b)?;

        return render_vertical_relationship(top, bottom, relationship, charset);
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

    RenderedEntityBox::new(entity.id.clone(), out, content_width + 2)
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

fn border_line(left: char, right: char, horizontal: char, content_width: usize) -> String {
    let mut line = String::new();
    line.push(left);
    line.extend(std::iter::repeat_n(horizontal, content_width));
    line.push(right);
    line
}

fn content_line(text: &str, content_width: usize, padding: usize, charset: ErCharset) -> String {
    let text_width = display_width(text);
    let trailing = content_width.saturating_sub(padding + text_width);

    let mut line = String::new();
    line.push(charset.vertical);
    line.extend(std::iter::repeat_n(' ', padding));
    line.push_str(text);
    line.extend(std::iter::repeat_n(' ', trailing));
    line.push(charset.vertical);
    line
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
    let center = relation_graph::vertical_center(
        top,
        bottom,
        &[
            display_width(top_cardinality) / 2,
            display_width(bottom_cardinality) / 2,
            label_half_width,
        ],
    );

    let mut relation_lines = Vec::new();
    relation_lines.push(relation_graph::centered_text_line(top_cardinality, center));
    if !label.is_empty() {
        relation_lines.push(relation_graph::centered_text_line(label, center));
    }
    relation_lines.push(relation_graph::marker_line(line, center));
    relation_lines.push(relation_graph::centered_text_line(
        bottom_cardinality,
        center,
    ));

    Ok(relation_graph::render_vertical_stack(
        top,
        bottom,
        center,
        relation_lines,
    ))
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
    charset: ErCharset,
) -> Result<String> {
    let first = &relationships[0];
    let top = find_box(boxes, &first.entity_a)?;
    let bottom = find_box(boxes, &first.entity_b)?;
    let lanes = relationships
        .iter()
        .map(|relationship| parallel_er_lane_rows(relationship, charset))
        .collect::<Result<Vec<_>>>()?;

    Ok(relation_graph::render_parallel_vertical_stack(
        top, bottom, &lanes, 2,
    ))
}

fn parallel_er_lane_rows(
    relationship: &ErRelationshipRenderModel,
    charset: ErCharset,
) -> Result<Vec<String>> {
    let top_cardinality = cardinality_marker(&relationship.rel_spec.card_b)?;
    let bottom_cardinality = cardinality_marker(&relationship.rel_spec.card_a)?;
    let line = relationship_line(&relationship.rel_spec.rel_type, charset)?;
    Ok(vec![
        top_cardinality.to_string(),
        relationship.role_a.trim().to_string(),
        line.to_string(),
        bottom_cardinality.to_string(),
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
    let mut draw_order = relationships
        .iter()
        .zip(parallel_lane_offsets(relationships))
        .enumerate()
        .collect::<Vec<_>>();
    draw_order.sort_by_key(|(index, (_, lane_offset))| (lane_offset.unsigned_abs(), *index));
    for (_, (relationship, lane_offset)) in draw_order {
        draw_layered_relationship(
            &mut canvas,
            &placed_by_id,
            relationship,
            lane_offset,
            charset,
        )?;
    }

    Ok(finish_trimmed_canvas(&canvas, width, height))
}

fn parallel_lane_offsets(relationships: &[ErRelationshipRenderModel]) -> Vec<isize> {
    let mut counts = HashMap::<(&str, &str), usize>::new();
    for relationship in relationships {
        *counts
            .entry((
                relationship.entity_a.as_str(),
                relationship.entity_b.as_str(),
            ))
            .or_insert(0) += 1;
    }

    let mut seen = HashMap::<(&str, &str), usize>::new();
    relationships
        .iter()
        .map(|relationship| {
            let key = (
                relationship.entity_a.as_str(),
                relationship.entity_b.as_str(),
            );
            let index = seen.entry(key).or_insert(0);
            let offset = relation_graph::parallel_lane_offset(*index, counts[&key]);
            *index += 1;
            offset
        })
        .collect()
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
        LayeredRelationError::SpanningLevels => "ER relationships spanning multiple layout levels",
        LayeredRelationError::Crossing => "crossing ER relationship layouts",
    };
    AsciiError::UnsupportedFeature {
        diagram_type: "er",
        feature,
    }
}

fn draw_layered_relationship(
    canvas: &mut Canvas,
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

    let from_x = relation_graph::offset_center(top.center_x(), lane_offset);
    let from_y = top.bottom();
    let to_x = relation_graph::offset_center(bottom.center_x(), lane_offset);
    let to_y = bottom.y();
    if to_y <= from_y + 2 {
        return Ok(());
    }

    let route_y = to_y - 2;

    for y in (from_y + 2)..=route_y {
        put_relation_char(canvas, from_x, y, vertical, charset);
    }
    if from_x != to_x {
        let left = from_x.min(to_x);
        let right = from_x.max(to_x);
        for x in left..=right {
            put_relation_char(canvas, x, route_y, horizontal, charset);
        }
    }
    for y in route_y..(to_y - 1) {
        put_relation_char(canvas, to_x, y, vertical, charset);
    }

    write_centered_relation_text(canvas, from_x, from_y + 1, top_cardinality);
    if !label.is_empty() {
        let label_y = (from_y + 2).min(route_y);
        write_centered_relation_text(canvas, (from_x + to_x) / 2, label_y, label);
    }
    write_centered_relation_text(canvas, to_x, to_y - 1, bottom_cardinality);

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

fn put_relation_char(canvas: &mut Canvas, x: usize, y: usize, ch: char, charset: ErCharset) {
    let next = match canvas.get(x, y) {
        Some(existing) if existing == ' ' || existing == ch => ch,
        Some(existing)
            if is_relation_line_char(existing, charset) && is_relation_line_char(ch, charset) =>
        {
            charset.relation_junction
        }
        _ => ch,
    };
    canvas.set(x, y, next);
}

fn is_relation_line_char(ch: char, charset: ErCharset) -> bool {
    matches!(
        ch,
        c if c == charset.solid_horizontal_relation
            || c == charset.solid_relation
            || c == charset.dotted_horizontal_relation
            || c == charset.dotted_relation
            || c == charset.relation_junction
    )
}

fn write_centered_relation_text(canvas: &mut Canvas, center_x: usize, y: usize, text: &str) {
    let text_half_width = display_width(text) / 2;
    canvas.write_text(center_x.saturating_sub(text_half_width), y, text);
}

fn finish_trimmed_canvas(canvas: &Canvas, width: usize, height: usize) -> String {
    let mut rendered = String::new();
    for y in 0..height {
        let mut line = (0..width)
            .map(|x| canvas.get(x, y).unwrap_or(' '))
            .collect::<String>();
        while line.ends_with(' ') {
            line.pop();
        }
        rendered.push_str(&line);
        rendered.push('\n');
    }
    rendered
}
