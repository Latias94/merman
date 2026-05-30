use crate::AsciiError;
use crate::Result;
use crate::canvas::Canvas;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::text::display_width;
use merman_core::models::class_diagram::{ClassDiagram, ClassMember, ClassNode, ClassRelation};
use std::collections::{HashMap, HashSet, VecDeque};

const CLASS_LEVEL_HORIZONTAL_GAP: usize = 4;

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
    solid_horizontal_relation: char,
    solid_vertical_relation: char,
    dotted_horizontal_relation: char,
    dotted_vertical_relation: char,
    relation_junction: char,
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
                solid_horizontal_relation: '-',
                solid_vertical_relation: '|',
                dotted_horizontal_relation: '.',
                dotted_vertical_relation: ':',
                relation_junction: '+',
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
                solid_horizontal_relation: '─',
                solid_vertical_relation: '│',
                dotted_horizontal_relation: '╌',
                dotted_vertical_relation: '┆',
                relation_junction: '┼',
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

type RenderedClassBox = RelationGraphBox;

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
        return Ok(relation_graph::render_stacked_boxes(&boxes));
    }

    let layouts = model
        .relations
        .iter()
        .map(|relation| relation_layout(model, relation))
        .collect::<Result<Vec<_>>>()?;

    if layouts.len() == 1 && model.classes.len() != 2 {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "class relationship layouts with unrelated classes",
        });
    }

    if layouts.len() == 1 {
        let layout = layouts[0];
        let top = find_box(&boxes, layout.top_id)?;
        let bottom = find_box(&boxes, layout.bottom_id)?;

        return Ok(render_vertical_relation(top, bottom, layout, charset));
    }

    render_layered_relations(&boxes, &layouts, options, charset)
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
    RenderedClassBox::new(class.id.clone(), out, width)
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
    relation_graph::find_box(boxes, id).ok_or(AsciiError::UnsupportedFeature {
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
    let center = relation_graph::vertical_center(top, bottom, &[label_half_width]);
    let mut relation_lines = Vec::new();

    match layout.marker_side {
        MarkerSide::Top => {
            relation_lines.push(relation_graph::marker_line(
                marker_char(layout.marker, MarkerSide::Top, charset),
                center,
            ));
            if let Some(label) = layout.label {
                relation_lines.push(relation_graph::centered_text_line(label, center));
            }
            relation_lines.push(relation_graph::marker_line(
                line_char(layout.line, charset),
                center,
            ));
        }
        MarkerSide::Bottom => {
            relation_lines.push(relation_graph::marker_line(
                line_char(layout.line, charset),
                center,
            ));
            if let Some(label) = layout.label {
                relation_lines.push(relation_graph::centered_text_line(label, center));
            }
            relation_lines.push(relation_graph::marker_line(
                marker_char(layout.marker, MarkerSide::Bottom, charset),
                center,
            ));
        }
    }

    relation_graph::render_vertical_stack(top, bottom, center, relation_lines)
}

#[derive(Debug, Clone, Copy)]
struct PlacedClassBox<'a> {
    id: &'a str,
    class_box: &'a RenderedClassBox,
    x: usize,
    y: usize,
}

impl PlacedClassBox<'_> {
    fn width(&self) -> usize {
        self.class_box.width()
    }

    fn height(&self) -> usize {
        self.class_box.height()
    }

    fn center_x(&self) -> usize {
        self.x + self.width() / 2
    }

    fn bottom(&self) -> usize {
        self.y + self.height().saturating_sub(1)
    }
}

fn render_layered_relations(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'_>],
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> Result<String> {
    let levels = class_relation_levels(boxes, layouts)?;
    let placed = place_class_boxes(boxes, layouts, &levels);
    let width = placed
        .iter()
        .map(|class_box| class_box.x + class_box.width())
        .max()
        .unwrap_or(0);
    let height = placed
        .iter()
        .map(|class_box| class_box.y + class_box.height())
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
            .class_box
            .draw_at(&mut canvas, placed_box.x, placed_box.y);
    }

    let placed_by_id = placed
        .iter()
        .map(|placed_box| (placed_box.id, placed_box))
        .collect::<HashMap<_, _>>();
    for layout in layouts {
        draw_layered_relation(&mut canvas, &placed_by_id, layout, charset);
    }

    Ok(finish_trimmed_canvas(&canvas, width, height))
}

fn class_relation_levels(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'_>],
) -> Result<HashMap<String, usize>> {
    let mut incident = HashSet::new();
    let mut incoming_count = boxes
        .iter()
        .map(|class_box| (class_box.id().to_string(), 0usize))
        .collect::<HashMap<_, _>>();
    let mut outgoing = HashMap::<String, Vec<String>>::new();
    let mut relation_pairs = HashSet::new();

    for layout in layouts {
        find_box(boxes, layout.top_id)?;
        find_box(boxes, layout.bottom_id)?;

        if !relation_pairs.insert((layout.top_id.to_string(), layout.bottom_id.to_string())) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "parallel class relationship layouts",
            });
        }

        incident.insert(layout.top_id.to_string());
        incident.insert(layout.bottom_id.to_string());
        *incoming_count
            .entry(layout.bottom_id.to_string())
            .or_insert(0) += 1;
        outgoing
            .entry(layout.top_id.to_string())
            .or_default()
            .push(layout.bottom_id.to_string());
    }

    if incident.len() != boxes.len() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "class relationship layouts with unrelated classes",
        });
    }

    let mut levels = HashMap::<String, usize>::new();
    let mut queue = boxes
        .iter()
        .filter(|class_box| incoming_count.get(class_box.id()).copied().unwrap_or(0) == 0)
        .map(|class_box| class_box.id().to_string())
        .collect::<VecDeque<_>>();

    if queue.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "cyclic class relationship layouts",
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
                    diagram_type: "class",
                    feature: "cyclic class relationship layouts",
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
            diagram_type: "class",
            feature: "cyclic class relationship layouts",
        });
    }

    for layout in layouts {
        let top_level = levels.get(layout.top_id).copied().unwrap_or(0);
        let bottom_level = levels.get(layout.bottom_id).copied().unwrap_or(0);
        if bottom_level <= top_level {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "cyclic class relationship layouts",
            });
        }
        if bottom_level != top_level + 1 {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "class relationships spanning multiple layout levels",
            });
        }
    }

    reject_crossing_class_relationships(boxes, layouts, &levels)?;

    Ok(levels)
}

fn reject_crossing_class_relationships(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'_>],
    levels: &HashMap<String, usize>,
) -> Result<()> {
    let mut order_by_id = HashMap::new();
    let max_level = levels.values().copied().max().unwrap_or(0);
    for level in 0..=max_level {
        let mut index = 0;
        for class_box in boxes {
            if levels.get(class_box.id()).copied() == Some(level) {
                order_by_id.insert(class_box.id().to_string(), index);
                index += 1;
            }
        }
    }

    for (left_index, left) in layouts.iter().enumerate() {
        let left_top_level = levels.get(left.top_id).copied().unwrap_or(0);
        let left_bottom_level = levels.get(left.bottom_id).copied().unwrap_or(0);
        for right in layouts.iter().skip(left_index + 1) {
            if levels.get(right.top_id).copied().unwrap_or(0) != left_top_level
                || levels.get(right.bottom_id).copied().unwrap_or(0) != left_bottom_level
            {
                continue;
            }

            let left_top_order = order_by_id.get(left.top_id).copied().unwrap_or(0);
            let left_bottom_order = order_by_id.get(left.bottom_id).copied().unwrap_or(0);
            let right_top_order = order_by_id.get(right.top_id).copied().unwrap_or(0);
            let right_bottom_order = order_by_id.get(right.bottom_id).copied().unwrap_or(0);

            let crosses_left_to_right =
                left_top_order < right_top_order && left_bottom_order > right_bottom_order;
            let crosses_right_to_left =
                left_top_order > right_top_order && left_bottom_order < right_bottom_order;
            if crosses_left_to_right || crosses_right_to_left {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "class",
                    feature: "crossing class relationship layouts",
                });
            }
        }
    }

    Ok(())
}

fn place_class_boxes<'a>(
    boxes: &'a [RenderedClassBox],
    layouts: &[RelationLayout<'_>],
    levels: &HashMap<String, usize>,
) -> Vec<PlacedClassBox<'a>> {
    let max_level = levels.values().copied().max().unwrap_or(0);
    let mut level_groups = vec![Vec::<&RenderedClassBox>::new(); max_level + 1];
    for class_box in boxes {
        if let Some(level) = levels.get(class_box.id()).copied() {
            level_groups[level].push(class_box);
        }
    }

    let group_widths = level_groups
        .iter()
        .map(|group| {
            let boxes_width = group
                .iter()
                .map(|class_box| class_box.width())
                .sum::<usize>();
            let gaps_width =
                CLASS_LEVEL_HORIZONTAL_GAP.saturating_mul(group.len().saturating_sub(1));
            boxes_width + gaps_width
        })
        .collect::<Vec<_>>();
    let max_label_half_width = layouts
        .iter()
        .filter_map(|layout| layout.label)
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
        for class_box in group {
            placed.push(PlacedClassBox {
                id: class_box.id(),
                class_box,
                x,
                y,
            });
            x += class_box.width() + CLASS_LEVEL_HORIZONTAL_GAP;
        }

        let row_height = group
            .iter()
            .map(|class_box| class_box.height())
            .max()
            .unwrap_or(0);
        y += row_height;
        if level < max_level {
            y += relation_gap_height(layouts, levels, level);
        }
    }

    placed
}

fn relation_gap_height(
    layouts: &[RelationLayout<'_>],
    levels: &HashMap<String, usize>,
    level: usize,
) -> usize {
    let has_label = layouts.iter().any(|layout| {
        levels.get(layout.top_id).copied() == Some(level)
            && levels.get(layout.bottom_id).copied() == Some(level + 1)
            && layout.label.is_some()
    });
    if has_label { 4 } else { 3 }
}

fn draw_layered_relation(
    canvas: &mut Canvas,
    placed_by_id: &HashMap<&str, &PlacedClassBox<'_>>,
    layout: &RelationLayout<'_>,
    charset: ClassCharset,
) {
    let Some(top) = placed_by_id.get(layout.top_id) else {
        return;
    };
    let Some(bottom) = placed_by_id.get(layout.bottom_id) else {
        return;
    };
    let from_x = top.center_x();
    let from_y = top.bottom();
    let to_x = bottom.center_x();
    let to_y = bottom.y;
    if to_y <= from_y + 1 {
        return;
    }

    let route_y = to_y - 1;
    let vertical = line_char(layout.line, charset);
    let horizontal = horizontal_line_char(layout.line, charset);

    for y in (from_y + 1)..=route_y {
        put_relation_char(canvas, from_x, y, vertical, charset);
    }
    if from_x != to_x {
        let left = from_x.min(to_x);
        let right = from_x.max(to_x);
        for x in left..=right {
            put_relation_char(canvas, x, route_y, horizontal, charset);
        }
    }
    for y in route_y..to_y {
        put_relation_char(canvas, to_x, y, vertical, charset);
    }

    if let Some(label) = layout.label {
        let label_y = (from_y + 2).min(route_y);
        write_centered_relation_text(canvas, (from_x + to_x) / 2, label_y, label);
    }

    match layout.marker_side {
        MarkerSide::Top => canvas.set(
            from_x,
            from_y + 1,
            marker_char(layout.marker, MarkerSide::Top, charset),
        ),
        MarkerSide::Bottom => canvas.set(
            to_x,
            to_y - 1,
            marker_char(layout.marker, MarkerSide::Bottom, charset),
        ),
    }
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

fn horizontal_line_char(line: RelationLine, charset: ClassCharset) -> char {
    match line {
        RelationLine::Solid => charset.solid_horizontal_relation,
        RelationLine::Dotted => charset.dotted_horizontal_relation,
    }
}

fn line_char(line: RelationLine, charset: ClassCharset) -> char {
    match line {
        RelationLine::Solid => charset.solid_vertical_relation,
        RelationLine::Dotted => charset.dotted_vertical_relation,
    }
}

fn put_relation_char(canvas: &mut Canvas, x: usize, y: usize, ch: char, charset: ClassCharset) {
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

fn is_relation_line_char(ch: char, charset: ClassCharset) -> bool {
    matches!(
        ch,
        c if c == charset.solid_horizontal_relation
            || c == charset.solid_vertical_relation
            || c == charset.dotted_horizontal_relation
            || c == charset.dotted_vertical_relation
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
