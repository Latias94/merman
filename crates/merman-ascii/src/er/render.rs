use crate::canvas::Canvas;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::text::display_width;
use crate::{AsciiError, Result};
use merman_core::diagrams::er::{
    ErAttributeRenderModel, ErDiagramRenderModel, ErEntityRenderModel, ErRelationshipRenderModel,
};
use std::collections::{HashMap, HashSet, VecDeque};

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

    if model.relationships.len() == 1 && model.entities.len() != 2 {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "ER relationship layouts with unrelated entities",
        });
    }

    if model.relationships.len() == 1 {
        let relationship = &model.relationships[0];
        let top = find_box(&boxes, &relationship.entity_a)?;
        let bottom = find_box(&boxes, &relationship.entity_b)?;

        return render_vertical_relationship(top, bottom, relationship, charset);
    }

    render_layered_relationships(&boxes, &model.relationships, options, charset)
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

#[derive(Debug, Clone, Copy)]
struct PlacedEntityBox<'a> {
    id: &'a str,
    entity_box: &'a RenderedEntityBox,
    x: usize,
    y: usize,
}

impl PlacedEntityBox<'_> {
    fn width(&self) -> usize {
        self.entity_box.width()
    }

    fn height(&self) -> usize {
        self.entity_box.height()
    }

    fn center_x(&self) -> usize {
        self.x + self.width() / 2
    }

    fn bottom(&self) -> usize {
        self.y + self.height().saturating_sub(1)
    }
}

fn render_layered_relationships(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    options: &AsciiRenderOptions,
    charset: ErCharset,
) -> Result<String> {
    let levels = er_relationship_levels(boxes, relationships)?;
    let placed = place_entity_boxes(boxes, relationships, &levels);
    let width = placed
        .iter()
        .map(|entity_box| entity_box.x + entity_box.width())
        .max()
        .unwrap_or(0);
    let height = placed
        .iter()
        .map(|entity_box| entity_box.y + entity_box.height())
        .max()
        .unwrap_or(0);
    let actual_cells = width.saturating_mul(height);
    if actual_cells > options.max_grid_cells {
        return Err(AsciiError::RenderLimitExceeded {
            actual: actual_cells,
            limit: options.max_grid_cells,
        });
    }

    let mut canvas = Canvas::new(width, height);
    for placed_box in &placed {
        placed_box
            .entity_box
            .draw_at(&mut canvas, placed_box.x, placed_box.y);
    }

    let placed_by_id = placed
        .iter()
        .map(|placed_box| (placed_box.id, placed_box))
        .collect::<HashMap<_, _>>();
    for relationship in relationships {
        draw_layered_relationship(&mut canvas, &placed_by_id, relationship, charset)?;
    }

    Ok(finish_trimmed_canvas(&canvas, width, height))
}

fn er_relationship_levels(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
) -> Result<HashMap<String, usize>> {
    let mut incident = HashSet::new();
    let mut incoming_count = boxes
        .iter()
        .map(|entity_box| (entity_box.id().to_string(), 0usize))
        .collect::<HashMap<_, _>>();
    let mut outgoing = HashMap::<String, Vec<String>>::new();
    let mut relationship_pairs = HashSet::new();

    for relationship in relationships {
        find_box(boxes, &relationship.entity_a)?;
        find_box(boxes, &relationship.entity_b)?;

        if !relationship_pairs
            .insert((relationship.entity_a.clone(), relationship.entity_b.clone()))
        {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "er",
                feature: "parallel ER relationship layouts",
            });
        }

        incident.insert(relationship.entity_a.clone());
        incident.insert(relationship.entity_b.clone());
        *incoming_count
            .entry(relationship.entity_b.clone())
            .or_insert(0) += 1;
        outgoing
            .entry(relationship.entity_a.clone())
            .or_default()
            .push(relationship.entity_b.clone());
    }

    if incident.len() != boxes.len() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "ER relationship layouts with unrelated entities",
        });
    }

    let mut levels = HashMap::<String, usize>::new();
    let mut queue = boxes
        .iter()
        .filter(|entity_box| incoming_count.get(entity_box.id()).copied().unwrap_or(0) == 0)
        .map(|entity_box| entity_box.id().to_string())
        .collect::<VecDeque<_>>();

    if queue.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "cyclic ER relationship layouts",
        });
    }

    for id in &queue {
        levels.insert(id.clone(), 0);
    }

    let level_cap = boxes.len().saturating_sub(1);
    while let Some(id) = queue.pop_front() {
        let current_level = levels.get(&id).copied().unwrap_or(0);
        let Some(children) = outgoing.get(&id) else {
            continue;
        };
        for child_id in children {
            let next_level = current_level + 1;
            if next_level > level_cap {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "er",
                    feature: "cyclic ER relationship layouts",
                });
            }
            let should_update = match levels.get(child_id) {
                Some(existing_level) => *existing_level < next_level,
                None => true,
            };
            if should_update {
                levels.insert(child_id.clone(), next_level);
                queue.push_back(child_id.clone());
            }
        }
    }

    if levels.len() != boxes.len() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "cyclic ER relationship layouts",
        });
    }

    for relationship in relationships {
        let top_level = levels.get(&relationship.entity_a).copied().unwrap_or(0);
        let bottom_level = levels.get(&relationship.entity_b).copied().unwrap_or(0);
        if bottom_level <= top_level {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "er",
                feature: "cyclic ER relationship layouts",
            });
        }
        if bottom_level != top_level + 1 {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "er",
                feature: "ER relationships spanning multiple layout levels",
            });
        }
    }

    reject_crossing_er_relationships(boxes, relationships, &levels)?;

    Ok(levels)
}

fn reject_crossing_er_relationships(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    levels: &HashMap<String, usize>,
) -> Result<()> {
    let mut order_by_id = HashMap::new();
    let max_level = levels.values().copied().max().unwrap_or(0);
    for level in 0..=max_level {
        let mut index = 0;
        for entity_box in boxes {
            if levels.get(entity_box.id()).copied() == Some(level) {
                order_by_id.insert(entity_box.id().to_string(), index);
                index += 1;
            }
        }
    }

    for (left_index, left) in relationships.iter().enumerate() {
        let left_top_level = levels.get(&left.entity_a).copied().unwrap_or(0);
        let left_bottom_level = levels.get(&left.entity_b).copied().unwrap_or(0);
        for right in relationships.iter().skip(left_index + 1) {
            if levels.get(&right.entity_a).copied().unwrap_or(0) != left_top_level
                || levels.get(&right.entity_b).copied().unwrap_or(0) != left_bottom_level
            {
                continue;
            }

            let left_top_order = order_by_id.get(&left.entity_a).copied().unwrap_or(0);
            let left_bottom_order = order_by_id.get(&left.entity_b).copied().unwrap_or(0);
            let right_top_order = order_by_id.get(&right.entity_a).copied().unwrap_or(0);
            let right_bottom_order = order_by_id.get(&right.entity_b).copied().unwrap_or(0);

            let crosses_left_to_right =
                left_top_order < right_top_order && left_bottom_order > right_bottom_order;
            let crosses_right_to_left =
                left_top_order > right_top_order && left_bottom_order < right_bottom_order;
            if crosses_left_to_right || crosses_right_to_left {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "er",
                    feature: "crossing ER relationship layouts",
                });
            }
        }
    }

    Ok(())
}

fn place_entity_boxes<'a>(
    boxes: &'a [RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    levels: &HashMap<String, usize>,
) -> Vec<PlacedEntityBox<'a>> {
    let max_level = levels.values().copied().max().unwrap_or(0);
    let mut level_groups = vec![Vec::<&RenderedEntityBox>::new(); max_level + 1];
    for entity_box in boxes {
        if let Some(level) = levels.get(entity_box.id()).copied() {
            level_groups[level].push(entity_box);
        }
    }

    let group_widths = level_groups
        .iter()
        .map(|group| {
            let boxes_width = group
                .iter()
                .map(|entity_box| entity_box.width())
                .sum::<usize>();
            let gaps_width = ER_LEVEL_HORIZONTAL_GAP.saturating_mul(group.len().saturating_sub(1));
            boxes_width + gaps_width
        })
        .collect::<Vec<_>>();
    let max_label_half_width = relationships
        .iter()
        .map(|relationship| relationship.role_a.trim())
        .filter(|label| !label.is_empty())
        .map(|label| display_width(label) / 2)
        .max()
        .unwrap_or(0);
    let content_width = group_widths
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .max(max_label_half_width.saturating_mul(2).saturating_add(1));
    let global_center = content_width / 2;

    let mut placed = Vec::new();
    let mut y = 0;
    for (level, group) in level_groups.iter().enumerate() {
        let group_width = group_widths[level];
        let mut x = global_center.saturating_sub(group_width / 2);
        for entity_box in group {
            placed.push(PlacedEntityBox {
                id: entity_box.id(),
                entity_box,
                x,
                y,
            });
            x += entity_box.width() + ER_LEVEL_HORIZONTAL_GAP;
        }

        let row_height = group
            .iter()
            .map(|entity_box| entity_box.height())
            .max()
            .unwrap_or(0);
        y += row_height;
        if level < max_level {
            y += er_relation_gap_height(relationships, levels, level);
        }
    }

    placed
}

fn er_relation_gap_height(
    relationships: &[ErRelationshipRenderModel],
    levels: &HashMap<String, usize>,
    level: usize,
) -> usize {
    let has_label = relationships.iter().any(|relationship| {
        levels.get(&relationship.entity_a).copied() == Some(level)
            && levels.get(&relationship.entity_b).copied() == Some(level + 1)
            && !relationship.role_a.trim().is_empty()
    });
    if has_label { 4 } else { 3 }
}

fn draw_layered_relationship(
    canvas: &mut Canvas,
    placed_by_id: &HashMap<&str, &PlacedEntityBox<'_>>,
    relationship: &ErRelationshipRenderModel,
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

    let from_x = top.center_x();
    let from_y = top.bottom();
    let to_x = bottom.center_x();
    let to_y = bottom.y;
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
