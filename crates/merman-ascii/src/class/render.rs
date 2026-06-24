use crate::AsciiError;
use crate::Result;
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::relation_graph::{
    LayeredRelationEdge, LayeredRelationError, LayeredRelationRouteStyle, LayeredRelationScene,
    RelationGraphLabel, RelationGraphLine, RelationGraphSummaryRow, RelationLineChars,
    RelationOverlay, RelationParallelPlan, RelationStackPlan,
};
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct RelationLayout<'a> {
    top_id: &'a str,
    bottom_id: &'a str,
    marker: RelationMarker,
    marker_side: MarkerSide,
    line: RelationLine,
    label: Option<RelationGraphLabel>,
}

pub(crate) fn render_class_diagram(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
) -> Result<String> {
    if model.classes.is_empty() {
        return Ok(String::new());
    }

    let charset = ClassCharset::for_options(options);
    let namespace_facade_aliases = namespace_facade_aliases(model);
    let boxes = model
        .classes
        .values()
        .filter(|class| !namespace_facade_aliases.contains_key(class.id.as_str()))
        .map(|class| render_class_box(class, options, charset))
        .collect::<Vec<_>>();

    if model.relations.is_empty() {
        return Ok(relation_graph::render_stacked_boxes_with_options(
            &boxes, options,
        ));
    }

    let layouts = model
        .relations
        .iter()
        .map(|relation| relation_layout(model, relation, &namespace_facade_aliases))
        .collect::<Result<Vec<_>>>()?;

    render_class_components(&boxes, &layouts, options, charset)
}

fn namespace_facade_aliases(model: &ClassDiagram) -> HashMap<String, String> {
    let mut aliases = HashMap::new();
    for class in model.classes.values() {
        let Some(local_id) = namespace_facade_local_id(model, class) else {
            continue;
        };
        aliases.insert(class.id.clone(), local_id.to_string());
    }
    aliases
}

fn namespace_facade_local_id<'a>(model: &'a ClassDiagram, class: &'a ClassNode) -> Option<&'a str> {
    if class
        .parent
        .as_deref()
        .map(str::trim)
        .is_some_and(|parent| !parent.is_empty())
        || !class.annotations.is_empty()
        || !class.members.is_empty()
        || !class.methods.is_empty()
    {
        return None;
    }

    model
        .namespaces
        .values()
        .filter_map(|namespace| {
            let remainder = class
                .id
                .strip_prefix(namespace.id.as_str())?
                .strip_prefix('.')?;
            namespace
                .class_ids
                .iter()
                .any(|id| id == remainder)
                .then_some((namespace.id.len(), remainder))
        })
        .max_by_key(|(namespace_len, _)| *namespace_len)
        .and_then(|(_, local_id)| model.classes.contains_key(local_id).then_some(local_id))
}

fn render_class_component(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'_>],
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> Result<String> {
    if layouts.is_empty() {
        return Ok(relation_graph::render_stacked_boxes_with_options(
            boxes, options,
        ));
    }
    if is_same_endpoint_parallel_layout(layouts) {
        return render_parallel_vertical_relations(boxes, layouts, options, charset);
    }
    if layouts.len() == 1 {
        let layout = &layouts[0];
        let top = find_box(boxes, layout.top_id)?;
        let bottom = find_box(boxes, layout.bottom_id)?;

        return Ok(render_vertical_relation(
            top, bottom, layout, options, charset,
        ));
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
            .map(|index| layouts[*index].clone())
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
    RenderedClassBox::new_with_lines(class.id.clone(), out, width)
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
    charset: ClassCharset,
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

fn relation_layout<'a>(
    model: &'a ClassDiagram,
    relation: &'a ClassRelation,
    namespace_facade_aliases: &'a HashMap<String, String>,
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

    let label = RelationGraphLabel::new(&relation.title);

    if marker == RelationMarker::Extension {
        return Ok(match marker_side {
            MarkerSide::Top => RelationLayout {
                top_id: relation_endpoint_id(namespace_facade_aliases, relation.id1.as_str()),
                bottom_id: relation_endpoint_id(namespace_facade_aliases, relation.id2.as_str()),
                marker,
                marker_side: MarkerSide::Top,
                line,
                label,
            },
            MarkerSide::Bottom => RelationLayout {
                top_id: relation_endpoint_id(namespace_facade_aliases, relation.id2.as_str()),
                bottom_id: relation_endpoint_id(namespace_facade_aliases, relation.id1.as_str()),
                marker,
                marker_side: MarkerSide::Top,
                line,
                label,
            },
        });
    }

    Ok(RelationLayout {
        top_id: relation_endpoint_id(namespace_facade_aliases, relation.id1.as_str()),
        bottom_id: relation_endpoint_id(namespace_facade_aliases, relation.id2.as_str()),
        marker,
        marker_side,
        line,
        label,
    })
}

fn relation_endpoint_id<'a>(
    namespace_facade_aliases: &'a HashMap<String, String>,
    id: &'a str,
) -> &'a str {
    namespace_facade_aliases
        .get(id)
        .map(String::as_str)
        .unwrap_or(id)
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
    layout: &RelationLayout<'_>,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> String {
    let label_half_width = layout
        .label
        .as_ref()
        .map(RelationGraphLabel::half_width)
        .unwrap_or(0);
    let plan = RelationStackPlan::from_centered_rows(top, bottom, &[label_half_width], |center| {
        class_relation_rows(layout, center, charset)
    });

    plan.render_with_options(options)
}

fn class_relation_rows(
    layout: &RelationLayout<'_>,
    center: usize,
    charset: ClassCharset,
) -> Vec<RelationGraphLine> {
    let mut relation_lines = Vec::new();
    match layout.marker_side {
        MarkerSide::Top => {
            relation_lines.push(relation_graph::marker_line_with_role(
                marker_char(layout.marker, MarkerSide::Top, charset),
                center,
                AsciiColorRole::EdgeArrow,
            ));
            if let Some(label) = layout.label.as_ref() {
                relation_lines.extend(relation_graph::centered_label_lines_with_role(
                    label,
                    center,
                    AsciiColorRole::EdgeLabel,
                ));
            }
            relation_lines.push(relation_graph::marker_line_with_role(
                line_char(layout.line, charset),
                center,
                AsciiColorRole::EdgeLine,
            ));
        }
        MarkerSide::Bottom => {
            relation_lines.push(relation_graph::marker_line_with_role(
                line_char(layout.line, charset),
                center,
                AsciiColorRole::EdgeLine,
            ));
            if let Some(label) = layout.label.as_ref() {
                relation_lines.extend(relation_graph::centered_label_lines_with_role(
                    label,
                    center,
                    AsciiColorRole::EdgeLabel,
                ));
            }
            relation_lines.push(relation_graph::marker_line_with_role(
                marker_char(layout.marker, MarkerSide::Bottom, charset),
                center,
                AsciiColorRole::EdgeArrow,
            ));
        }
    }
    relation_lines
}

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
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> Result<String> {
    let first = &layouts[0];
    let top = find_box(boxes, first.top_id)?;
    let bottom = find_box(boxes, first.bottom_id)?;
    let lanes = layouts
        .iter()
        .map(|layout| parallel_class_lane_rows(layout, charset))
        .collect::<Vec<_>>();
    let plan = RelationParallelPlan::new(top, bottom, lanes, 2);

    Ok(plan.render_with_options(options))
}

fn parallel_class_lane_rows(
    layout: &RelationLayout<'_>,
    charset: ClassCharset,
) -> Vec<RelationGraphLine> {
    let marker = RelationGraphLine::with_role(
        marker_char(layout.marker, layout.marker_side, charset).to_string(),
        AsciiColorRole::EdgeArrow,
    );
    let line = RelationGraphLine::with_role(
        line_char(layout.line, charset).to_string(),
        AsciiColorRole::EdgeLine,
    );
    let label_lines = layout
        .label
        .as_ref()
        .map(|label| relation_graph::label_lines_with_role(label, AsciiColorRole::EdgeLabel))
        .unwrap_or_else(|| {
            vec![RelationGraphLine::with_role(
                String::new(),
                AsciiColorRole::EdgeLabel,
            )]
        });
    match layout.marker_side {
        MarkerSide::Top => {
            let mut rows = vec![marker];
            rows.extend(label_lines);
            rows.push(line);
            rows
        }
        MarkerSide::Bottom => {
            let mut rows = vec![line];
            rows.extend(label_lines);
            rows.push(marker);
            rows
        }
    }
}

fn render_layered_relations(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'_>],
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> Result<String> {
    let edges = layouts.iter().map(class_layered_edge).collect::<Vec<_>>();
    let scene = match relation_graph::plan_layered_relation_scene(
        boxes,
        edges,
        CLASS_LEVEL_HORIZONTAL_GAP,
        options.max_grid_cells,
    )
    .map_err(class_layered_error)?
    {
        relation_graph::LayeredRelationScenePlan::Routed(scene) => scene,
        relation_graph::LayeredRelationScenePlan::Summary(reason) => {
            let _ = reason;
            return Ok(render_dense_relation_fallback(boxes, layouts, options));
        }
    };

    let mut canvas = scene.canvas_with_boxes();
    for (edge_index, lane_offset) in scene.draw_order().iter().copied() {
        let layout = &layouts[edge_index];
        draw_layered_relation(
            &scene,
            &mut canvas,
            edge_index,
            layout,
            lane_offset,
            charset,
        );
    }

    Ok(canvas.finish_trimmed_with_options(options))
}

fn render_dense_relation_fallback(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'_>],
    options: &AsciiRenderOptions,
) -> String {
    let rows = layouts
        .iter()
        .map(class_relation_summary_row)
        .collect::<Vec<_>>();
    relation_graph::render_stacked_boxes_with_relation_summary(boxes, &rows, options)
}

fn class_relation_summary_row(layout: &RelationLayout<'_>) -> RelationGraphSummaryRow {
    RelationGraphSummaryRow::new(
        layout.top_id,
        class_relation_summary_symbol(layout),
        layout.bottom_id,
    )
    .with_label(layout.label.as_ref())
}

fn class_relation_summary_symbol(layout: &RelationLayout<'_>) -> &'static str {
    match (layout.marker, layout.marker_side, layout.line) {
        (RelationMarker::Extension, MarkerSide::Top, _) => "<|--",
        (RelationMarker::Extension, MarkerSide::Bottom, _) => "--|>",
        (RelationMarker::Dependency, MarkerSide::Top, RelationLine::Dotted) => "<..",
        (RelationMarker::Dependency, MarkerSide::Bottom, RelationLine::Dotted) => "..>",
        (RelationMarker::Dependency, MarkerSide::Top, RelationLine::Solid) => "<--",
        (RelationMarker::Dependency, MarkerSide::Bottom, RelationLine::Solid) => "-->",
        (RelationMarker::Aggregation, MarkerSide::Top, _) => "o--",
        (RelationMarker::Aggregation, MarkerSide::Bottom, _) => "--o",
        (RelationMarker::Composition, MarkerSide::Top, _) => "*--",
        (RelationMarker::Composition, MarkerSide::Bottom, _) => "--*",
    }
}

fn class_layered_edge<'a>(layout: &RelationLayout<'a>) -> LayeredRelationEdge<'a> {
    let label = layout.label.as_ref();
    LayeredRelationEdge::new(
        layout.top_id,
        layout.bottom_id,
        label.map(RelationGraphLabel::half_width).unwrap_or(0),
        label.map(RelationGraphLabel::line_count).unwrap_or(0),
    )
}

fn class_layered_error(error: LayeredRelationError) -> AsciiError {
    let feature = match error {
        LayeredRelationError::MissingEndpoint => "relationships with missing endpoint classes",
        LayeredRelationError::UnrelatedBoxes => "class relationship layouts with unrelated classes",
        LayeredRelationError::Crossing => "crossing class relationship layouts",
    };
    AsciiError::UnsupportedFeature {
        diagram_type: "class",
        feature,
    }
}

fn draw_layered_relation(
    scene: &LayeredRelationScene<'_, '_>,
    canvas: &mut Canvas,
    edge_index: usize,
    layout: &RelationLayout<'_>,
    lane_offset: isize,
    charset: ClassCharset,
) {
    let vertical = line_char(layout.line, charset);
    let horizontal = horizontal_line_char(layout.line, charset);
    let relation_chars = relation_line_chars(charset);
    let style = LayeredRelationRouteStyle::new(
        vertical,
        horizontal,
        relation_chars,
        relation_graph::LayeredRelationRouteProfile::class(),
    );
    scene.draw_edge(canvas, edge_index, lane_offset, style, |geometry| {
        let mut overlays = Vec::new();
        if let Some(label) = layout.label.as_ref() {
            overlays.push(RelationOverlay::label(
                (geometry.from_x() + geometry.to_x()) / 2,
                geometry.label_y_after_source(),
                label.clone(),
                AsciiColorRole::EdgeLabel,
            ));
        }

        match layout.marker_side {
            MarkerSide::Top => overlays.push(RelationOverlay::glyph(
                geometry.from_x(),
                geometry.source_marker_y(),
                marker_char(layout.marker, MarkerSide::Top, charset),
                AsciiColorRole::EdgeArrow,
            )),
            MarkerSide::Bottom => overlays.push(RelationOverlay::glyph(
                geometry.to_x(),
                geometry.target_marker_y(),
                marker_char(layout.marker, MarkerSide::Bottom, charset),
                AsciiColorRole::EdgeArrow,
            )),
        }

        overlays
    });
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

fn relation_line_chars(charset: ClassCharset) -> RelationLineChars {
    RelationLineChars::new(
        [
            charset.solid_horizontal_relation,
            charset.solid_vertical_relation,
            charset.dotted_horizontal_relation,
            charset.dotted_vertical_relation,
        ],
        charset.relation_junction,
    )
}
