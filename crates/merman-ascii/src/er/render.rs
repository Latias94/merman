use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::relation_graph::{
    LayeredRelationEdge, LayeredRelationError, LayeredRelationRouteStyle, LayeredRelationScene,
    RelationGraphLabel, RelationGraphLine, RelationLineChars, RelationOverlay,
    RelationParallelPlan, RelationStackPlan,
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
    let entity_labels = model
        .entities
        .values()
        .map(|entity| (entity.id.clone(), entity_display_label(entity).to_string()))
        .collect::<HashMap<_, _>>();

    if model.relationships.is_empty() {
        return Ok(relation_graph::render_stacked_boxes_with_options(
            &boxes, options,
        ));
    }

    render_er_components(
        &boxes,
        &model.relationships,
        &entity_labels,
        options,
        charset,
    )
}

fn render_er_component(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    entity_labels: &HashMap<String, String>,
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

    render_layered_relationships(boxes, relationships, entity_labels, options, charset)
}

fn render_er_components(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    entity_labels: &HashMap<String, String>,
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
        return render_er_component(boxes, relationships, entity_labels, options, charset);
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
            entity_labels,
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
    let mut sections = vec![vec![entity_display_label(entity).to_string()]];

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
    let label = RelationGraphLabel::new(&relationship.role_a);
    let label_half_width = label
        .as_ref()
        .map(RelationGraphLabel::half_width)
        .unwrap_or(0);
    let plan = RelationStackPlan::from_centered_rows(
        top,
        bottom,
        &[
            display_width(top_cardinality) / 2,
            display_width(bottom_cardinality) / 2,
            label_half_width,
        ],
        |center| {
            er_relationship_rows(
                top_cardinality,
                bottom_cardinality,
                line,
                label.as_ref(),
                center,
            )
        },
    );

    Ok(plan.render_with_options(options))
}

fn er_relationship_rows(
    top_cardinality: &str,
    bottom_cardinality: &str,
    line: char,
    label: Option<&RelationGraphLabel>,
    center: usize,
) -> Vec<RelationGraphLine> {
    let mut relation_lines = Vec::new();
    relation_lines.push(relation_graph::centered_text_line_with_role(
        top_cardinality,
        center,
        AsciiColorRole::EdgeArrow,
    ));
    if let Some(label) = label {
        relation_lines.extend(relation_graph::centered_label_lines_with_role(
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
    let label_lines = RelationGraphLabel::new(&relationship.role_a)
        .map(|label| relation_graph::label_lines_with_role(&label, AsciiColorRole::EdgeLabel))
        .unwrap_or_else(|| {
            vec![RelationGraphLine::with_role(
                String::new(),
                AsciiColorRole::EdgeLabel,
            )]
        });
    let mut rows = vec![RelationGraphLine::with_role(
        top_cardinality.to_string(),
        AsciiColorRole::EdgeArrow,
    )];
    rows.extend(label_lines);
    rows.push(RelationGraphLine::with_role(
        line.to_string(),
        AsciiColorRole::EdgeLine,
    ));
    rows.push(RelationGraphLine::with_role(
        bottom_cardinality.to_string(),
        AsciiColorRole::EdgeArrow,
    ));
    Ok(rows)
}

fn render_layered_relationships(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    entity_labels: &HashMap<String, String>,
    options: &AsciiRenderOptions,
    charset: ErCharset,
) -> Result<String> {
    let edges = relationships
        .iter()
        .map(er_layered_edge)
        .collect::<Vec<_>>();
    let scene = match LayeredRelationScene::new(boxes, edges, ER_LEVEL_HORIZONTAL_GAP) {
        Ok(scene) => scene,
        Err(LayeredRelationError::Crossing) => {
            return render_dense_relationship_fallback(
                boxes,
                relationships,
                entity_labels,
                options,
                charset,
            );
        }
        Err(error) => return Err(er_layered_error(error)),
    };
    if scene.cell_count() > options.max_grid_cells {
        return Err(AsciiError::RenderLimitExceeded {
            actual: scene.cell_count(),
            limit: options.max_grid_cells,
        });
    }

    let mut canvas = scene.canvas_with_boxes();
    for (edge_index, lane_offset) in scene.draw_order().iter().copied() {
        let relationship = &relationships[edge_index];
        draw_layered_relationship(
            &scene,
            &mut canvas,
            edge_index,
            relationship,
            lane_offset,
            charset,
        )?;
    }

    Ok(canvas.finish_trimmed_with_options(options))
}

fn render_dense_relationship_fallback(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    entity_labels: &HashMap<String, String>,
    options: &AsciiRenderOptions,
    charset: ErCharset,
) -> Result<String> {
    let mut summaries = Vec::with_capacity(relationships.len());
    for relationship in relationships {
        summaries.push(er_relationship_summary(
            relationship,
            entity_labels,
            charset,
        )?);
    }

    Ok(relation_graph::render_stacked_boxes_with_section(
        boxes,
        "relations:",
        &summaries,
        options,
    ))
}

fn er_relationship_summary(
    relationship: &ErRelationshipRenderModel,
    entity_labels: &HashMap<String, String>,
    charset: ErCharset,
) -> Result<String> {
    let left_cardinality = cardinality_marker(&relationship.rel_spec.card_b)?;
    let right_cardinality = cardinality_marker(&relationship.rel_spec.card_a)?;
    let relation = er_relationship_summary_line(&relationship.rel_spec.rel_type, charset)?;
    let label = RelationGraphLabel::new(&relationship.role_a)
        .map(|label| format!(" : {}", label.lines().join(" / ")))
        .unwrap_or_default();

    Ok(format!(
        "{} {}{}{} {}{}",
        relationship_label(entity_labels, &relationship.entity_a),
        left_cardinality,
        relation,
        right_cardinality,
        relationship_label(entity_labels, &relationship.entity_b),
        label
    ))
}

fn relationship_label<'a>(entity_labels: &'a HashMap<String, String>, id: &'a str) -> &'a str {
    entity_labels.get(id).map(String::as_str).unwrap_or(id)
}

fn er_layered_edge(relationship: &ErRelationshipRenderModel) -> LayeredRelationEdge<'_> {
    let label = RelationGraphLabel::new(&relationship.role_a);
    LayeredRelationEdge::new(
        &relationship.entity_a,
        &relationship.entity_b,
        label
            .as_ref()
            .map(RelationGraphLabel::half_width)
            .unwrap_or(0),
        label
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

fn draw_layered_relationship(
    scene: &LayeredRelationScene<'_, '_>,
    canvas: &mut Canvas,
    edge_index: usize,
    relationship: &ErRelationshipRenderModel,
    lane_offset: isize,
    charset: ErCharset,
) -> Result<()> {
    let top_cardinality = cardinality_marker(&relationship.rel_spec.card_b)?;
    let bottom_cardinality = cardinality_marker(&relationship.rel_spec.card_a)?;
    let vertical = relationship_line(&relationship.rel_spec.rel_type, charset)?;
    let horizontal = relationship_horizontal_line(&relationship.rel_spec.rel_type, charset)?;
    let label = RelationGraphLabel::new(&relationship.role_a);
    let relation_chars = relation_line_chars(charset);
    let style = LayeredRelationRouteStyle::new(
        vertical,
        horizontal,
        relation_chars,
        relation_graph::LayeredRelationRouteProfile::er(),
    );

    scene.draw_edge(canvas, edge_index, lane_offset, style, |geometry| {
        let mut overlays = Vec::new();
        overlays.push(RelationOverlay::text(
            geometry.from_x(),
            geometry.source_marker_y(),
            top_cardinality.to_string(),
            AsciiColorRole::EdgeArrow,
        ));
        if let Some(label) = label.as_ref() {
            overlays.push(RelationOverlay::label(
                (geometry.from_x() + geometry.to_x()) / 2,
                geometry.label_y_after_source(),
                label.clone(),
                AsciiColorRole::EdgeLabel,
            ));
        }
        overlays.push(RelationOverlay::text(
            geometry.to_x(),
            geometry.target_marker_y(),
            bottom_cardinality.to_string(),
            AsciiColorRole::EdgeArrow,
        ));
        overlays
    });

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
