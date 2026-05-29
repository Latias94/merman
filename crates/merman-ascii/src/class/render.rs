use crate::AsciiError;
use crate::Result;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::text::display_width;
use merman_core::models::class_diagram::{ClassDiagram, ClassMember, ClassNode, ClassRelation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClassCharset {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    vertical: char,
    separator_left: char,
    separator_right: char,
    solid_vertical_relation: char,
    dotted_vertical_relation: char,
    extension_up: char,
    extension_down: char,
    arrow_up: char,
    arrow_down: char,
    aggregation: char,
    composition: char,
}

impl ClassCharset {
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
                solid_vertical_relation: '|',
                dotted_vertical_relation: ':',
                extension_up: '^',
                extension_down: 'v',
                arrow_up: '^',
                arrow_down: 'v',
                aggregation: 'o',
                composition: '*',
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
                solid_vertical_relation: '│',
                dotted_vertical_relation: '┆',
                extension_up: '△',
                extension_down: '▽',
                arrow_up: '▲',
                arrow_down: '▼',
                aggregation: '◇',
                composition: '◆',
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedClassBox {
    id: String,
    lines: Vec<String>,
    width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelationMarker {
    Extension,
    Dependency,
    Aggregation,
    Composition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkerSide {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelationLine {
    Solid,
    Dotted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RelationLayout<'a> {
    top_id: &'a str,
    bottom_id: &'a str,
    marker: RelationMarker,
    marker_side: MarkerSide,
    line: RelationLine,
    label: Option<&'a str>,
}

pub(crate) fn render_class_diagram(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
) -> Result<String> {
    if model.classes.is_empty() {
        return Ok(String::new());
    }

    let charset = ClassCharset::for_options(options);
    let boxes = model
        .classes
        .values()
        .map(|class| render_class_box(class, options, charset))
        .collect::<Vec<_>>();

    if model.relations.is_empty() {
        return Ok(render_stacked_boxes(&boxes));
    }

    if model.relations.len() != 1 {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "multiple class relationships",
        });
    }
    if model.classes.len() != 2 {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "class relationship layouts with unrelated classes",
        });
    }

    let relation = &model.relations[0];
    let layout = relation_layout(model, relation)?;
    let top = find_box(&boxes, layout.top_id)?;
    let bottom = find_box(&boxes, layout.bottom_id)?;

    Ok(render_vertical_relation(top, bottom, layout, charset))
}

fn render_class_box(
    class: &ClassNode,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> RenderedClassBox {
    let sections = class_sections(class);
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

    let width = content_width + 2;
    RenderedClassBox {
        id: class.id.clone(),
        lines: out,
        width,
    }
}

fn class_sections(class: &ClassNode) -> Vec<Vec<String>> {
    let mut header = class
        .annotations
        .iter()
        .map(|annotation| format!("<<{annotation}>>"))
        .collect::<Vec<_>>();
    header.push(class.label.clone());

    let mut sections = vec![header];

    let members = class
        .members
        .iter()
        .map(member_text)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if !members.is_empty() {
        sections.push(members);
    }

    let methods = class
        .methods
        .iter()
        .map(member_text)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if !methods.is_empty() {
        sections.push(methods);
    }

    sections
}

fn member_text(member: &ClassMember) -> String {
    if !member.display_text.is_empty() {
        return member.display_text.clone();
    }
    member.id.clone()
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

fn content_line(text: &str, content_width: usize, padding: usize, charset: ClassCharset) -> String {
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

fn render_stacked_boxes(boxes: &[RenderedClassBox]) -> String {
    boxes.iter().map(render_box).collect::<Vec<_>>().join("\n")
}

fn render_box(class_box: &RenderedClassBox) -> String {
    let mut rendered = class_box.lines.join("\n");
    rendered.push('\n');
    rendered
}

fn relation_layout<'a>(
    model: &'a ClassDiagram,
    relation: &'a ClassRelation,
) -> Result<RelationLayout<'a>> {
    let line = if relation.relation.line_type == model.constants.line_type.line {
        RelationLine::Solid
    } else if relation.relation.line_type == model.constants.line_type.dotted_line {
        RelationLine::Dotted
    } else {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "unknown class relationship line types",
        });
    };

    if !relation_end_label_is_absent(&relation.relation_title_1)
        || !relation_end_label_is_absent(&relation.relation_title_2)
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "relationship endpoint labels",
        });
    }

    let left_marker = marker_for_relation_type(model, relation.relation.type1)?;
    let right_marker = marker_for_relation_type(model, relation.relation.type2)?;
    let none = model.constants.relation_type.none;

    let (marker, marker_side) = match (left_marker, right_marker) {
        (Some(marker), None) if relation.relation.type2 == none => (marker, MarkerSide::Top),
        (None, Some(marker)) if relation.relation.type1 == none => (marker, MarkerSide::Bottom),
        _ => {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "class relationships with multiple or missing markers",
            });
        }
    };

    let title = relation.title.trim();
    let label = (!title.is_empty()).then_some(title);

    if marker == RelationMarker::Extension {
        return Ok(match marker_side {
            MarkerSide::Top => RelationLayout {
                top_id: relation.id1.as_str(),
                bottom_id: relation.id2.as_str(),
                marker,
                marker_side: MarkerSide::Top,
                line,
                label,
            },
            MarkerSide::Bottom => RelationLayout {
                top_id: relation.id2.as_str(),
                bottom_id: relation.id1.as_str(),
                marker,
                marker_side: MarkerSide::Top,
                line,
                label,
            },
        });
    }

    Ok(RelationLayout {
        top_id: relation.id1.as_str(),
        bottom_id: relation.id2.as_str(),
        marker,
        marker_side,
        line,
        label,
    })
}

fn marker_for_relation_type(
    model: &ClassDiagram,
    relation_type: i32,
) -> Result<Option<RelationMarker>> {
    let constants = &model.constants.relation_type;
    if relation_type == constants.none {
        return Ok(None);
    }
    if relation_type == constants.extension {
        return Ok(Some(RelationMarker::Extension));
    }
    if relation_type == constants.dependency {
        return Ok(Some(RelationMarker::Dependency));
    }
    if relation_type == constants.aggregation {
        return Ok(Some(RelationMarker::Aggregation));
    }
    if relation_type == constants.composition {
        return Ok(Some(RelationMarker::Composition));
    }

    Err(AsciiError::UnsupportedFeature {
        diagram_type: "class",
        feature: "class relationship types other than extension, dependency, aggregation, or composition",
    })
}

fn relation_end_label_is_absent(label: &str) -> bool {
    let trimmed = label.trim();
    trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none")
}

fn find_box<'a>(boxes: &'a [RenderedClassBox], id: &str) -> Result<&'a RenderedClassBox> {
    boxes
        .iter()
        .find(|class_box| class_box.id == id)
        .ok_or(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "relationships with missing endpoint classes",
        })
}

fn render_vertical_relation(
    top: &RenderedClassBox,
    bottom: &RenderedClassBox,
    layout: RelationLayout<'_>,
    charset: ClassCharset,
) -> String {
    let label_half_width = layout
        .label
        .map(|label| display_width(label) / 2)
        .unwrap_or(0);
    let center = (top.width / 2).max(bottom.width / 2).max(label_half_width);
    let mut lines = Vec::new();

    lines.extend(align_box(top, center));
    match layout.marker_side {
        MarkerSide::Top => {
            lines.push(marker_line(
                marker_char(layout.marker, MarkerSide::Top, charset),
                center,
            ));
            if let Some(label) = layout.label {
                lines.push(centered_text_line(label, center));
            }
            lines.push(marker_line(line_char(layout.line, charset), center));
        }
        MarkerSide::Bottom => {
            lines.push(marker_line(line_char(layout.line, charset), center));
            if let Some(label) = layout.label {
                lines.push(centered_text_line(label, center));
            }
            lines.push(marker_line(
                marker_char(layout.marker, MarkerSide::Bottom, charset),
                center,
            ));
        }
    }
    lines.extend(align_box(bottom, center));

    let mut rendered = lines.join("\n");
    rendered.push('\n');
    rendered
}

fn marker_char(marker: RelationMarker, side: MarkerSide, charset: ClassCharset) -> char {
    match marker {
        RelationMarker::Extension => match side {
            MarkerSide::Top => charset.extension_up,
            MarkerSide::Bottom => charset.extension_down,
        },
        RelationMarker::Dependency => match side {
            MarkerSide::Top => charset.arrow_up,
            MarkerSide::Bottom => charset.arrow_down,
        },
        RelationMarker::Aggregation => charset.aggregation,
        RelationMarker::Composition => charset.composition,
    }
}

fn line_char(line: RelationLine, charset: ClassCharset) -> char {
    match line {
        RelationLine::Solid => charset.solid_vertical_relation,
        RelationLine::Dotted => charset.dotted_vertical_relation,
    }
}

fn align_box(class_box: &RenderedClassBox, center: usize) -> Vec<String> {
    let left_padding = center.saturating_sub(class_box.width / 2);
    let padding = " ".repeat(left_padding);
    class_box
        .lines
        .iter()
        .map(|line| format!("{padding}{line}"))
        .collect()
}

fn marker_line(marker: char, center: usize) -> String {
    let mut line = String::new();
    line.extend(std::iter::repeat_n(' ', center));
    line.push(marker);
    line
}

fn centered_text_line(text: &str, center: usize) -> String {
    let mut line = String::new();
    let half_width = display_width(text) / 2;
    line.extend(std::iter::repeat_n(' ', center.saturating_sub(half_width)));
    line.push_str(text);
    line
}
