use crate::AsciiError;
use crate::Result;
use crate::canvas::Canvas;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::relation_graph::{LayeredRelationEdge, LayeredRelationError};
use crate::text::display_width;
use merman_core::models::class_diagram::{ClassDiagram, ClassMember, ClassNode, ClassRelation};
use std::collections::HashMap;

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

    render_class_components(&boxes, &layouts, options, charset)
}

fn render_class_component(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'_>],
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> Result<String> {
    if layouts.is_empty() {
        return Ok(relation_graph::render_stacked_boxes(boxes));
    }
    if is_same_endpoint_parallel_layout(layouts) {
        return render_parallel_vertical_relations(boxes, layouts, charset);
    }
    if layouts.len() == 1 {
        let layout = layouts[0];
        let top = find_box(boxes, layout.top_id)?;
        let bottom = find_box(boxes, layout.bottom_id)?;

        return Ok(render_vertical_relation(top, bottom, layout, charset));
    }

    render_layered_relations(boxes, layouts, options, charset)
}

fn render_class_components(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'_>],
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> Result<String> {
    let edges = layouts.iter().map(class_layered_edge).collect::<Vec<_>>();
    let components =
        relation_graph::relation_components(boxes, &edges).map_err(class_layered_error)?;
    if components.len() == 1 {
        return render_class_component(boxes, layouts, options, charset);
    }

    let mut rendered = Vec::new();
    for component in components {
        let component_boxes = component
            .boxes()
            .iter()
            .map(|relation_box| (*relation_box).clone())
            .collect::<Vec<_>>();
        let component_layouts = component
            .edge_indices()
            .iter()
            .map(|index| layouts[*index])
            .collect::<Vec<_>>();
        rendered.push(render_class_component(
            &component_boxes,
            &component_layouts,
            options,
            charset,
        )?);
    }

    Ok(rendered.join("\n"))
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

type PlacedClassBox<'a> = relation_graph::PlacedRelationGraphBox<'a>;

fn is_same_endpoint_parallel_layout(layouts: &[RelationLayout<'_>]) -> bool {
    let Some(first) = layouts.first() else {
        return false;
    };
    layouts.len() > 1
        && layouts
            .iter()
            .all(|layout| layout.top_id == first.top_id && layout.bottom_id == first.bottom_id)
}

fn render_parallel_vertical_relations(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'_>],
    charset: ClassCharset,
) -> Result<String> {
    let first = layouts[0];
    let top = find_box(boxes, first.top_id)?;
    let bottom = find_box(boxes, first.bottom_id)?;
    let lanes = layouts
        .iter()
        .map(|layout| parallel_class_lane_rows(*layout, charset))
        .collect::<Vec<_>>();

    Ok(relation_graph::render_parallel_vertical_stack(
        top, bottom, &lanes, 2,
    ))
}

fn parallel_class_lane_rows(layout: RelationLayout<'_>, charset: ClassCharset) -> Vec<String> {
    let marker = marker_char(layout.marker, layout.marker_side, charset).to_string();
    let line = line_char(layout.line, charset).to_string();
    let label = layout.label.unwrap_or("").to_string();
    match layout.marker_side {
        MarkerSide::Top => vec![marker, label, line],
        MarkerSide::Bottom => vec![line, label, marker],
    }
}

fn render_layered_relations(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'_>],
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> Result<String> {
    let edges = layouts.iter().map(class_layered_edge).collect::<Vec<_>>();
    let plan =
        relation_graph::plan_layered_relation_boxes(boxes, &edges, CLASS_LEVEL_HORIZONTAL_GAP)
            .map_err(class_layered_error)?;
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
    let mut draw_order = layouts
        .iter()
        .zip(parallel_lane_offsets(layouts))
        .enumerate()
        .collect::<Vec<_>>();
    draw_order.sort_by_key(|(index, (_, lane_offset))| (lane_offset.unsigned_abs(), *index));
    for (_, (layout, lane_offset)) in draw_order {
        draw_layered_relation(
            &mut canvas,
            plan.placed_boxes(),
            &placed_by_id,
            layout,
            lane_offset,
            charset,
        );
    }

    Ok(finish_trimmed_canvas(&canvas, width, height))
}

fn parallel_lane_offsets(layouts: &[RelationLayout<'_>]) -> Vec<isize> {
    let mut counts = HashMap::<(&str, &str), usize>::new();
    for layout in layouts {
        *counts.entry((layout.top_id, layout.bottom_id)).or_insert(0) += 1;
    }

    let mut seen = HashMap::<(&str, &str), usize>::new();
    layouts
        .iter()
        .map(|layout| {
            let key = (layout.top_id, layout.bottom_id);
            let index = seen.entry(key).or_insert(0);
            let offset = relation_graph::parallel_lane_offset(*index, counts[&key]);
            *index += 1;
            offset
        })
        .collect()
}

fn class_layered_edge<'a>(layout: &RelationLayout<'a>) -> LayeredRelationEdge<'a> {
    LayeredRelationEdge::new(
        layout.top_id,
        layout.bottom_id,
        layout.label.is_some(),
        layout
            .label
            .map(|label| display_width(label) / 2)
            .unwrap_or(0),
    )
}

fn class_layered_error(error: LayeredRelationError) -> AsciiError {
    let feature = match error {
        LayeredRelationError::MissingEndpoint => "relationships with missing endpoint classes",
        LayeredRelationError::UnrelatedBoxes => "class relationship layouts with unrelated classes",
        LayeredRelationError::Cyclic => "cyclic class relationship layouts",
        LayeredRelationError::Crossing => "crossing class relationship layouts",
    };
    AsciiError::UnsupportedFeature {
        diagram_type: "class",
        feature,
    }
}

fn draw_layered_relation(
    canvas: &mut Canvas,
    placed_boxes: &[PlacedClassBox<'_>],
    placed_by_id: &HashMap<&str, &PlacedClassBox<'_>>,
    layout: &RelationLayout<'_>,
    lane_offset: isize,
    charset: ClassCharset,
) {
    let Some(top) = placed_by_id.get(layout.top_id) else {
        return;
    };
    let Some(bottom) = placed_by_id.get(layout.bottom_id) else {
        return;
    };
    let lane_offset = if spans_intermediate_box(placed_boxes, top, bottom) {
        lane_offset + relation_graph::spanning_lane_offset(top.width(), bottom.width())
    } else {
        lane_offset
    };
    let from_x = relation_graph::offset_center(top.center_x(), lane_offset);
    let from_y = top.bottom();
    let to_x = relation_graph::offset_center(bottom.center_x(), lane_offset);
    let to_y = bottom.y();
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

fn spans_intermediate_box(
    placed_boxes: &[PlacedClassBox<'_>],
    top: &PlacedClassBox<'_>,
    bottom: &PlacedClassBox<'_>,
) -> bool {
    placed_boxes
        .iter()
        .any(|placed_box| placed_box.y() > top.y() && placed_box.y() < bottom.y())
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
