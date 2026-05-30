use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::text::display_width;
use crate::{AsciiError, Result};
use merman_core::diagrams::er::{
    ErAttributeRenderModel, ErDiagramRenderModel, ErEntityRenderModel, ErRelationshipRenderModel,
};

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
    solid_relation: char,
    dotted_relation: char,
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
                solid_relation: '|',
                dotted_relation: ':',
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
                solid_relation: '│',
                dotted_relation: '┆',
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

    if model.relationships.len() != 1 {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "multiple ER relationships",
        });
    }
    if model.entities.len() != 2 {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "er",
            feature: "ER relationship layouts with unrelated entities",
        });
    }

    let relationship = &model.relationships[0];
    let top = find_box(&boxes, &relationship.entity_a)?;
    let bottom = find_box(&boxes, &relationship.entity_b)?;

    render_vertical_relationship(top, bottom, relationship, charset)
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
