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
    arrow_up: char,
    vertical_relation: char,
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
                arrow_up: '^',
                vertical_relation: '|',
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
                arrow_up: '▲',
                vertical_relation: '│',
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
    let (parent_id, child_id) = extension_relation(model, relation)?;
    let parent = find_box(&boxes, parent_id)?;
    let child = find_box(&boxes, child_id)?;

    Ok(render_vertical_extension(parent, child, charset))
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

fn extension_relation<'a>(
    model: &'a ClassDiagram,
    relation: &'a ClassRelation,
) -> Result<(&'a str, &'a str)> {
    if relation.relation.line_type != model.constants.line_type.line {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "non-solid class relationships",
        });
    }

    if !relation.title.trim().is_empty()
        || !relation_end_label_is_absent(&relation.relation_title_1)
        || !relation_end_label_is_absent(&relation.relation_title_2)
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "relationship labels",
        });
    }

    let extension = model.constants.relation_type.extension;
    let none = model.constants.relation_type.none;
    match (relation.relation.type1, relation.relation.type2) {
        (left, right) if left == extension && right == none => {
            Ok((relation.id1.as_str(), relation.id2.as_str()))
        }
        (left, right) if left == none && right == extension => {
            Ok((relation.id2.as_str(), relation.id1.as_str()))
        }
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "class relationship types other than extension",
        }),
    }
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

fn render_vertical_extension(
    parent: &RenderedClassBox,
    child: &RenderedClassBox,
    charset: ClassCharset,
) -> String {
    let center = (parent.width / 2).max(child.width / 2);
    let mut lines = Vec::new();

    lines.extend(align_box(parent, center));
    lines.push(marker_line(charset.arrow_up, center));
    lines.push(marker_line(charset.vertical_relation, center));
    lines.extend(align_box(child, center));

    let mut rendered = lines.join("\n");
    rendered.push('\n');
    rendered
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
