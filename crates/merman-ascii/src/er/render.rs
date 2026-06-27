use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::relation_graph::{
    LayeredRelationEdge, LayeredRelationError, LayeredRelationRouteStyle, LayeredRelationScene,
    RelationGraphBoxStyle, RelationGraphLabel, RelationGraphLine, RelationGraphSummaryRow,
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

struct ErRelationComponentAdapter<'a> {
    charset: ErCharset,
    entity_labels: &'a HashMap<String, String>,
}

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

fn render_er_components(
    boxes: &[RenderedEntityBox],
    relationships: &[ErRelationshipRenderModel],
    entity_labels: &HashMap<String, String>,
    options: &AsciiRenderOptions,
    charset: ErCharset,
) -> Result<String> {
    let adapter = ErRelationComponentAdapter {
        charset,
        entity_labels,
    };
    relation_graph::render_relation_components(boxes, relationships, options, &adapter)
}

fn render_entity_box(
    entity: &ErEntityRenderModel,
    options: &AsciiRenderOptions,
    charset: ErCharset,
) -> RenderedEntityBox {
    let sections = entity_sections(entity);
    render_box_sections(entity.id.clone(), sections, options, charset)
}

fn render_box_sections(
    id: String,
    sections: Vec<Vec<String>>,
    options: &AsciiRenderOptions,
    charset: ErCharset,
) -> RenderedEntityBox {
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
    )
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

fn er_relationship_summary_row(
    relationship: &ErRelationshipRenderModel,
    entity_labels: &HashMap<String, String>,
    charset: ErCharset,
) -> Result<RelationGraphSummaryRow> {
    let left_cardinality = cardinality_marker(&relationship.rel_spec.card_b)?;
    let right_cardinality = cardinality_marker(&relationship.rel_spec.card_a)?;
    let relation = er_relationship_summary_line(&relationship.rel_spec.rel_type, charset)?;
    let label = RelationGraphLabel::new(&relationship.role_a);

    Ok(RelationGraphSummaryRow::new(
        relationship_label(entity_labels, &relationship.entity_a),
        format!("{left_cardinality}{relation}{right_cardinality}"),
        relationship_label(entity_labels, &relationship.entity_b),
    )
    .with_label(label.as_ref()))
}

fn relationship_label<'a>(entity_labels: &'a HashMap<String, String>, id: &'a str) -> &'a str {
    entity_labels.get(id).map(String::as_str).unwrap_or(id)
}

fn er_layered_edge(relationship: &ErRelationshipRenderModel) -> LayeredRelationEdge {
    let label = RelationGraphLabel::new(&relationship.role_a);
    LayeredRelationEdge::new(
        relationship.entity_a.as_str(),
        relationship.entity_b.as_str(),
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

impl<'a> relation_graph::RelationComponentAdapter<ErRelationshipRenderModel>
    for ErRelationComponentAdapter<'a>
{
    fn build_edges(&self, relationship: &ErRelationshipRenderModel) -> LayeredRelationEdge {
        er_layered_edge(relationship)
    }

    fn is_same_endpoint_parallel(&self, relationships: &[ErRelationshipRenderModel]) -> bool {
        is_same_endpoint_parallel_relationship(relationships)
    }

    fn layered_horizontal_gap(&self) -> usize {
        ER_LEVEL_HORIZONTAL_GAP
    }

    fn render_vertical(
        &self,
        boxes: &[RenderedEntityBox],
        relationship: &ErRelationshipRenderModel,
        options: &AsciiRenderOptions,
    ) -> Result<String> {
        let top = find_box(boxes, &relationship.entity_a)?;
        let bottom = find_box(boxes, &relationship.entity_b)?;

        render_vertical_relationship(top, bottom, relationship, options, self.charset)
    }

    fn render_parallel(
        &self,
        boxes: &[RenderedEntityBox],
        relationships: &[ErRelationshipRenderModel],
        options: &AsciiRenderOptions,
    ) -> Result<String> {
        render_parallel_vertical_relationships(boxes, relationships, options, self.charset)
    }

    fn build_summary_row(
        &self,
        relationship: &ErRelationshipRenderModel,
        _reason: relation_graph::LayeredRelationSummaryReason,
    ) -> Result<RelationGraphSummaryRow> {
        er_relationship_summary_row(relationship, self.entity_labels, self.charset)
    }

    fn draw_layered_edge<'boxes>(
        &self,
        scene: &relation_graph::LayeredRelationScene<'boxes>,
        canvas: &mut Canvas,
        edge_index: usize,
        relationship: &ErRelationshipRenderModel,
        lane_offset: isize,
    ) -> Result<()> {
        draw_layered_relationship(
            scene,
            canvas,
            edge_index,
            relationship,
            lane_offset,
            self.charset,
        )?;
        Ok(())
    }

    fn layered_error(&self, error: LayeredRelationError) -> AsciiError {
        er_layered_error(error)
    }
}

fn draw_layered_relationship(
    scene: &LayeredRelationScene<'_>,
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
            geometry.source_x(),
            geometry.source_marker_y(),
            top_cardinality.to_string(),
            AsciiColorRole::EdgeArrow,
        ));
        if let Some(label) = label.as_ref() {
            overlays.push(RelationOverlay::label(
                (geometry.source_x() + geometry.target_x()) / 2,
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
