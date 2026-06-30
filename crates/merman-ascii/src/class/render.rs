use crate::AsciiError;
use crate::Result;
use crate::color::AsciiColorRole;
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::relation_graph::{
    LayeredRelationEdge, LayeredRelationError, LayeredRelationRouteStyle, RelationGraphBoxStyle,
    RelationGraphLabel, RelationGraphLine, RelationGraphSummaryRow, RelationLineChars,
    RelationOverlay, RelationParallelPlan, RelationStackPlan,
};
use merman_core::entities::decode_html_entities_to_unicode;
use merman_core::models::class_diagram::{
    ClassDiagram, ClassInterface, ClassMember, ClassNode, ClassNote, ClassRelation,
};
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
    lollipop: char,
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
                lollipop: 'o',
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
                lollipop: '○',
            },
        }
    }
}

type RenderedClassBox = RelationGraphBox;

struct ClassRelationComponentAdapter {
    charset: ClassCharset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelationMarker {
    Extension,
    Dependency,
    Aggregation,
    Composition,
    Lollipop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkerSide {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RelationEndpointMarker {
    marker: RelationMarker,
    side: MarkerSide,
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
    endpoint_marker: Option<RelationEndpointMarker>,
    line: RelationLine,
    label: Option<RelationGraphLabel>,
    top_endpoint_label: Option<RelationGraphLabel>,
    bottom_endpoint_label: Option<RelationGraphLabel>,
}

pub(crate) fn render_class_diagram(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let charset = ClassCharset::for_options(options);
    let namespace_facade_aliases = namespace_facade_aliases(model);
    let boxes = render_class_boxes(model, options, charset, &namespace_facade_aliases);
    if boxes.is_empty() {
        return Ok(relation_graph::render_stacked_boxes_with_options(
            &boxes, options,
        ));
    }

    let mut layouts = model
        .relations
        .iter()
        .map(|relation| relation_layout(model, relation, &namespace_facade_aliases))
        .collect::<Result<Vec<_>>>()?;
    layouts.extend(note_relation_layouts(
        model,
        &namespace_facade_aliases,
        &boxes,
    ));

    if layouts.is_empty() {
        return Ok(relation_graph::render_stacked_boxes_with_options(
            &boxes, options,
        ));
    }

    render_class_components(&boxes, &layouts, options, charset)
}

fn render_class_boxes(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    namespace_facade_aliases: &HashMap<String, String>,
) -> Vec<RenderedClassBox> {
    let mut boxes =
        Vec::with_capacity(model.classes.len() + model.interfaces.len() + model.notes.len());
    boxes.extend(
        model
            .classes
            .values()
            .filter(|class| !namespace_facade_aliases.contains_key(class.id.as_str()))
            .map(|class| render_class_box(class, options, charset)),
    );
    boxes.extend(
        model
            .interfaces
            .iter()
            .map(|interface| render_interface_box(interface, options, charset)),
    );
    boxes.extend(
        model
            .notes
            .iter()
            .map(|note| render_note_box(note, options, charset)),
    );
    boxes
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

fn render_class_components(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'_>],
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> Result<String> {
    let adapter = ClassRelationComponentAdapter { charset };
    relation_graph::render_relation_components(boxes, layouts, options, &adapter)
}

fn render_class_box(
    class: &ClassNode,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> RenderedClassBox {
    let sections = class_sections(class);
    render_box_sections(class.id.clone(), sections, options, charset)
}

fn render_interface_box(
    interface: &ClassInterface,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> RenderedClassBox {
    let sections = vec![vec![
        "<<interface>>".to_string(),
        decode_html_entities_to_unicode(&interface.label).into_owned(),
    ]];
    render_box_sections(interface.id.clone(), sections, options, charset)
}

fn render_note_box(
    note: &ClassNote,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> RenderedClassBox {
    let mut lines = vec!["note".to_string()];
    if let Some(label) = RelationGraphLabel::new(&note.text) {
        lines.extend(label.lines().iter().cloned());
    }

    render_box_sections(note.id.clone(), vec![lines], options, charset)
}

fn render_box_sections(
    id: String,
    sections: Vec<Vec<String>>,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
) -> RenderedClassBox {
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

fn class_sections(class: &ClassNode) -> Vec<Vec<String>> {
    let mut header = class
        .annotations
        .iter()
        .map(|annotation| format!("<<{annotation}>>"))
        .collect::<Vec<_>>();
    header.push(class_title(class));

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

fn class_title(class: &ClassNode) -> String {
    decode_html_entities_to_unicode(&class.text).into_owned()
}

fn member_text(member: &ClassMember) -> String {
    if !member.display_text.is_empty() {
        return member.display_text.clone();
    }
    member.id.clone()
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

    let left_marker = marker_for_relation_type(model, relation.relation.type1)?;
    let right_marker = marker_for_relation_type(model, relation.relation.type2)?;
    let none = model.constants.relation_type.none;

    let endpoint_marker = match (left_marker, right_marker) {
        (Some(marker), None) if relation.relation.type2 == none => Some(RelationEndpointMarker {
            marker,
            side: MarkerSide::Top,
        }),
        (None, Some(marker)) if relation.relation.type1 == none => Some(RelationEndpointMarker {
            marker,
            side: MarkerSide::Bottom,
        }),
        (None, None) if relation.relation.type1 == none && relation.relation.type2 == none => None,
        _ => {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "class relationships with multiple markers",
            });
        }
    };

    let label = RelationGraphLabel::new(&relation.title);
    let left_endpoint_label = relation_endpoint_label(&relation.relation_title_1);
    let right_endpoint_label = relation_endpoint_label(&relation.relation_title_2);

    if let Some(marker) =
        endpoint_marker.filter(|marker| marker.marker == RelationMarker::Extension)
    {
        return Ok(match marker.side {
            MarkerSide::Top => RelationLayout {
                top_id: relation_endpoint_id(namespace_facade_aliases, relation.id1.as_str()),
                bottom_id: relation_endpoint_id(namespace_facade_aliases, relation.id2.as_str()),
                endpoint_marker: Some(RelationEndpointMarker {
                    marker: marker.marker,
                    side: MarkerSide::Top,
                }),
                line,
                label,
                top_endpoint_label: left_endpoint_label,
                bottom_endpoint_label: right_endpoint_label,
            },
            MarkerSide::Bottom => RelationLayout {
                top_id: relation_endpoint_id(namespace_facade_aliases, relation.id2.as_str()),
                bottom_id: relation_endpoint_id(namespace_facade_aliases, relation.id1.as_str()),
                endpoint_marker: Some(RelationEndpointMarker {
                    marker: marker.marker,
                    side: MarkerSide::Top,
                }),
                line,
                label,
                top_endpoint_label: right_endpoint_label,
                bottom_endpoint_label: left_endpoint_label,
            },
        });
    }

    Ok(RelationLayout {
        top_id: relation_endpoint_id(namespace_facade_aliases, relation.id1.as_str()),
        bottom_id: relation_endpoint_id(namespace_facade_aliases, relation.id2.as_str()),
        endpoint_marker,
        line,
        label,
        top_endpoint_label: left_endpoint_label,
        bottom_endpoint_label: right_endpoint_label,
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
    if relation_type == constants.lollipop {
        return Ok(Some(RelationMarker::Lollipop));
    }

    Err(AsciiError::UnsupportedFeature {
        diagram_type: "class",
        feature: "class relationship types other than extension, dependency, aggregation, composition, or lollipop",
    })
}

fn note_relation_layouts<'a>(
    model: &'a ClassDiagram,
    namespace_facade_aliases: &'a HashMap<String, String>,
    boxes: &[RenderedClassBox],
) -> Vec<RelationLayout<'a>> {
    model
        .notes
        .iter()
        .filter_map(|note| {
            let target_id = note.class_id.as_deref()?;
            let target_id = relation_endpoint_id(namespace_facade_aliases, target_id);
            relation_graph::find_box(boxes, target_id).map(|_| RelationLayout {
                top_id: note.id.as_str(),
                bottom_id: target_id,
                endpoint_marker: None,
                line: RelationLine::Dotted,
                label: None,
                top_endpoint_label: None,
                bottom_endpoint_label: None,
            })
        })
        .collect()
}

fn relation_endpoint_label(label: &str) -> Option<RelationGraphLabel> {
    let trimmed = label.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        return None;
    }

    RelationGraphLabel::new(trimmed)
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
    let label_half_widths = [
        layout
            .label
            .as_ref()
            .map(RelationGraphLabel::half_width)
            .unwrap_or(0),
        layout
            .top_endpoint_label
            .as_ref()
            .map(RelationGraphLabel::half_width)
            .unwrap_or(0),
        layout
            .bottom_endpoint_label
            .as_ref()
            .map(RelationGraphLabel::half_width)
            .unwrap_or(0),
    ];
    let plan = RelationStackPlan::from_centered_rows(top, bottom, &label_half_widths, |center| {
        class_relation_rows(layout, center, charset)
    });

    plan.render_with_options(options)
}

fn push_centered_endpoint_label(
    relation_lines: &mut Vec<RelationGraphLine>,
    label: Option<&RelationGraphLabel>,
    center: usize,
) {
    if let Some(label) = label {
        relation_lines.extend(relation_graph::centered_label_lines_with_role(
            label,
            center,
            AsciiColorRole::EdgeLabel,
        ));
    }
}

fn class_relation_rows(
    layout: &RelationLayout<'_>,
    center: usize,
    charset: ClassCharset,
) -> Vec<RelationGraphLine> {
    let mut relation_lines = Vec::new();
    push_centered_endpoint_label(
        &mut relation_lines,
        layout.top_endpoint_label.as_ref(),
        center,
    );
    match layout.endpoint_marker {
        None => {
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
                relation_lines.push(relation_graph::marker_line_with_role(
                    line_char(layout.line, charset),
                    center,
                    AsciiColorRole::EdgeLine,
                ));
            }
        }
        Some(endpoint_marker) => match endpoint_marker.side {
            MarkerSide::Top => {
                relation_lines.push(relation_graph::marker_line_with_role(
                    marker_char(endpoint_marker.marker, MarkerSide::Top, charset),
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
                    marker_char(endpoint_marker.marker, MarkerSide::Bottom, charset),
                    center,
                    AsciiColorRole::EdgeArrow,
                ));
            }
        },
    }
    push_centered_endpoint_label(
        &mut relation_lines,
        layout.bottom_endpoint_label.as_ref(),
        center,
    );
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
    let reserve_top_endpoint_label = layouts
        .iter()
        .any(|layout| layout.top_endpoint_label.is_some());
    let reserve_bottom_endpoint_label = layouts
        .iter()
        .any(|layout| layout.bottom_endpoint_label.is_some());
    let lanes = layouts
        .iter()
        .map(|layout| {
            parallel_class_lane_rows(
                layout,
                charset,
                reserve_top_endpoint_label,
                reserve_bottom_endpoint_label,
            )
        })
        .collect::<Vec<_>>();
    let plan = RelationParallelPlan::new(top, bottom, lanes, 2);

    Ok(plan.render_with_options(options))
}

fn endpoint_label_lines_or_empty(
    label: Option<&RelationGraphLabel>,
    reserve_empty: bool,
) -> Vec<RelationGraphLine> {
    match label {
        Some(label) => relation_graph::label_lines_with_role(label, AsciiColorRole::EdgeLabel),
        None if reserve_empty => {
            vec![RelationGraphLine::with_role(
                String::new(),
                AsciiColorRole::EdgeLabel,
            )]
        }
        None => Vec::new(),
    }
}

fn central_label_lines_or_empty(label: Option<&RelationGraphLabel>) -> Vec<RelationGraphLine> {
    label
        .map(|label| relation_graph::label_lines_with_role(label, AsciiColorRole::EdgeLabel))
        .unwrap_or_else(|| {
            vec![RelationGraphLine::with_role(
                String::new(),
                AsciiColorRole::EdgeLabel,
            )]
        })
}

fn parallel_class_lane_rows(
    layout: &RelationLayout<'_>,
    charset: ClassCharset,
    reserve_top_endpoint_label: bool,
    reserve_bottom_endpoint_label: bool,
) -> Vec<RelationGraphLine> {
    let line = RelationGraphLine::with_role(
        line_char(layout.line, charset).to_string(),
        AsciiColorRole::EdgeLine,
    );
    let mut rows = endpoint_label_lines_or_empty(
        layout.top_endpoint_label.as_ref(),
        reserve_top_endpoint_label,
    );
    let label_lines = central_label_lines_or_empty(layout.label.as_ref());
    let relation_rows = match layout.endpoint_marker {
        None => {
            let mut rows = vec![line.clone()];
            rows.extend(label_lines);
            rows.push(line);
            rows
        }
        Some(endpoint_marker) => {
            let marker = RelationGraphLine::with_role(
                marker_char(endpoint_marker.marker, endpoint_marker.side, charset).to_string(),
                AsciiColorRole::EdgeArrow,
            );
            match endpoint_marker.side {
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
    };
    rows.extend(relation_rows);
    rows.extend(endpoint_label_lines_or_empty(
        layout.bottom_endpoint_label.as_ref(),
        reserve_bottom_endpoint_label,
    ));
    rows
}

fn class_relation_summary_row(layout: &RelationLayout<'_>) -> Result<RelationGraphSummaryRow> {
    Ok(RelationGraphSummaryRow::new(
        layout.top_id,
        class_relation_summary_connector(layout),
        layout.bottom_id,
    )
    .with_label(layout.label.as_ref()))
}

fn class_relation_summary_row_for_reason(
    layout: &RelationLayout<'_>,
    reason: relation_graph::LayeredRelationSummaryReason,
) -> Result<RelationGraphSummaryRow> {
    match reason {
        relation_graph::LayeredRelationSummaryReason::Crossing
        | relation_graph::LayeredRelationSummaryReason::RouteCollision
        | relation_graph::LayeredRelationSummaryReason::OverlayCollision
        | relation_graph::LayeredRelationSummaryReason::GridBudget { .. } => {
            class_relation_summary_row(layout)
        }
    }
}

fn class_relation_summary_connector(layout: &RelationLayout<'_>) -> String {
    let symbol = class_relation_summary_symbol(layout);
    let top_label = layout
        .top_endpoint_label
        .as_ref()
        .map(endpoint_label_summary_text);
    let bottom_label = layout
        .bottom_endpoint_label
        .as_ref()
        .map(endpoint_label_summary_text);

    match (top_label, bottom_label) {
        (Some(top_label), Some(bottom_label)) => {
            format!("\"{top_label}\" {symbol} \"{bottom_label}\"")
        }
        (Some(top_label), None) => format!("\"{top_label}\" {symbol}"),
        (None, Some(bottom_label)) => format!("{symbol} \"{bottom_label}\""),
        (None, None) => symbol.to_string(),
    }
}

fn endpoint_label_summary_text(label: &RelationGraphLabel) -> String {
    label.lines().join("/")
}

fn class_relation_summary_symbol(layout: &RelationLayout<'_>) -> &'static str {
    let Some(endpoint_marker) = layout.endpoint_marker else {
        return match layout.line {
            RelationLine::Solid => "--",
            RelationLine::Dotted => "..",
        };
    };

    match (endpoint_marker.marker, endpoint_marker.side, layout.line) {
        (RelationMarker::Extension, MarkerSide::Top, RelationLine::Solid) => "<|--",
        (RelationMarker::Extension, MarkerSide::Top, RelationLine::Dotted) => "<|..",
        (RelationMarker::Extension, MarkerSide::Bottom, RelationLine::Solid) => "--|>",
        (RelationMarker::Extension, MarkerSide::Bottom, RelationLine::Dotted) => "..|>",
        (RelationMarker::Dependency, MarkerSide::Top, RelationLine::Dotted) => "<..",
        (RelationMarker::Dependency, MarkerSide::Bottom, RelationLine::Dotted) => "..>",
        (RelationMarker::Dependency, MarkerSide::Top, RelationLine::Solid) => "<--",
        (RelationMarker::Dependency, MarkerSide::Bottom, RelationLine::Solid) => "-->",
        (RelationMarker::Aggregation, MarkerSide::Top, RelationLine::Solid) => "o--",
        (RelationMarker::Aggregation, MarkerSide::Top, RelationLine::Dotted) => "o..",
        (RelationMarker::Aggregation, MarkerSide::Bottom, RelationLine::Solid) => "--o",
        (RelationMarker::Aggregation, MarkerSide::Bottom, RelationLine::Dotted) => "..o",
        (RelationMarker::Composition, MarkerSide::Top, RelationLine::Solid) => "*--",
        (RelationMarker::Composition, MarkerSide::Top, RelationLine::Dotted) => "*..",
        (RelationMarker::Composition, MarkerSide::Bottom, RelationLine::Solid) => "--*",
        (RelationMarker::Composition, MarkerSide::Bottom, RelationLine::Dotted) => "..*",
        (RelationMarker::Lollipop, MarkerSide::Top, RelationLine::Solid) => "()--",
        (RelationMarker::Lollipop, MarkerSide::Top, RelationLine::Dotted) => "()..",
        (RelationMarker::Lollipop, MarkerSide::Bottom, RelationLine::Solid) => "--()",
        (RelationMarker::Lollipop, MarkerSide::Bottom, RelationLine::Dotted) => "..()",
    }
}

fn class_layered_edge(layout: &RelationLayout<'_>) -> LayeredRelationEdge {
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

impl<'a> relation_graph::RelationComponentAdapter<RelationLayout<'a>>
    for ClassRelationComponentAdapter
{
    fn build_edges(&self, layout: &RelationLayout<'a>) -> LayeredRelationEdge {
        class_layered_edge(layout)
    }

    fn is_same_endpoint_parallel(&self, layouts: &[RelationLayout<'a>]) -> bool {
        is_same_endpoint_parallel_layout(layouts)
    }

    fn is_self_relation(&self, layout: &RelationLayout<'a>) -> bool {
        layout.top_id == layout.bottom_id
    }

    fn render_self_relation(
        &self,
        relation_box: &RenderedClassBox,
        layout: &RelationLayout<'a>,
        options: &AsciiRenderOptions,
    ) -> Result<String> {
        let rows = self_loop_rows_for_class_layout(layout, self.charset);

        Ok(relation_graph::render_parallel_self_loops_with_options(
            relation_box,
            vec![rows],
            options,
        ))
    }

    fn render_self_relations(
        &self,
        relation_box: &RenderedClassBox,
        layouts: &[RelationLayout<'a>],
        options: &AsciiRenderOptions,
    ) -> Result<String> {
        let loops = layouts
            .iter()
            .map(|layout| self_loop_rows_for_class_layout(layout, self.charset))
            .collect::<Vec<_>>();

        Ok(relation_graph::render_parallel_self_loops_with_options(
            relation_box,
            loops,
            options,
        ))
    }

    fn layered_horizontal_gap(&self) -> usize {
        CLASS_LEVEL_HORIZONTAL_GAP
    }

    fn layered_route_style(
        &self,
        layout: &RelationLayout<'a>,
    ) -> Result<LayeredRelationRouteStyle> {
        let vertical = line_char(layout.line, self.charset);
        let horizontal = horizontal_line_char(layout.line, self.charset);
        let relation_chars = relation_line_chars(self.charset);
        Ok(LayeredRelationRouteStyle::new(
            vertical,
            horizontal,
            relation_chars,
            class_route_profile(layout),
        ))
    }

    fn layered_relation_overlays(
        &self,
        layout: &RelationLayout<'a>,
        geometry: &relation_graph::LayeredRelationRouteGeometry,
    ) -> Result<Vec<RelationOverlay>> {
        let mut overlays = Vec::new();
        if let Some(label) = layout.top_endpoint_label.as_ref() {
            overlays.push(RelationOverlay::label(
                geometry.source_x(),
                geometry
                    .source_marker_y()
                    .saturating_sub(label.line_count()),
                label.clone(),
                AsciiColorRole::EdgeLabel,
            ));
        }
        if let Some(label) = layout.label.as_ref() {
            overlays.push(RelationOverlay::label(
                (geometry.source_x() + geometry.target_x()) / 2,
                geometry.label_y_after_source(),
                label.clone(),
                AsciiColorRole::EdgeLabel,
            ));
        }

        if let Some(endpoint_marker) = layout.endpoint_marker {
            match endpoint_marker.side {
                MarkerSide::Top => overlays.push(RelationOverlay::glyph(
                    geometry.source_x(),
                    geometry.source_marker_y(),
                    marker_char(endpoint_marker.marker, MarkerSide::Top, self.charset),
                    AsciiColorRole::EdgeArrow,
                )),
                MarkerSide::Bottom => overlays.push(RelationOverlay::glyph(
                    geometry.target_x(),
                    geometry.target_marker_y(),
                    marker_char(endpoint_marker.marker, MarkerSide::Bottom, self.charset),
                    AsciiColorRole::EdgeArrow,
                )),
            }
        }

        if let Some(label) = layout.bottom_endpoint_label.as_ref() {
            overlays.push(RelationOverlay::label(
                geometry.target_x(),
                geometry.target_marker_y() + 1,
                label.clone(),
                AsciiColorRole::EdgeLabel,
            ));
        }

        Ok(overlays)
    }

    fn render_vertical(
        &self,
        boxes: &[RenderedClassBox],
        layout: &RelationLayout<'a>,
        options: &AsciiRenderOptions,
    ) -> Result<String> {
        let top = find_box(boxes, layout.top_id)?;
        let bottom = find_box(boxes, layout.bottom_id)?;

        Ok(render_vertical_relation(
            top,
            bottom,
            layout,
            options,
            self.charset,
        ))
    }

    fn render_parallel(
        &self,
        boxes: &[RenderedClassBox],
        layouts: &[RelationLayout<'a>],
        options: &AsciiRenderOptions,
    ) -> Result<String> {
        render_parallel_vertical_relations(boxes, layouts, options, self.charset)
    }

    fn build_summary_row(
        &self,
        layout: &RelationLayout<'a>,
        reason: relation_graph::LayeredRelationSummaryReason,
    ) -> Result<RelationGraphSummaryRow> {
        class_relation_summary_row_for_reason(layout, reason)
    }

    fn layered_error(&self, error: LayeredRelationError) -> AsciiError {
        class_layered_error(error)
    }
}

fn self_loop_rows_for_class_layout(
    layout: &RelationLayout<'_>,
    charset: ClassCharset,
) -> relation_graph::RelationSelfLoopRows {
    let top_marker = RelationGraphLine::with_role("+".to_string(), AsciiColorRole::EdgeLine);
    let bottom_marker = RelationGraphLine::with_role(
        layout
            .endpoint_marker
            .map(|marker| marker_char(marker.marker, marker.side, charset))
            .unwrap_or_else(|| line_char(layout.line, charset))
            .to_string(),
        AsciiColorRole::EdgeArrow,
    );
    let label_lines = layout
        .label
        .as_ref()
        .map(|label| relation_graph::label_lines_with_role(label, AsciiColorRole::EdgeLabel))
        .unwrap_or_default();

    relation_graph::RelationSelfLoopRows::new(
        top_marker,
        label_lines,
        bottom_marker,
        horizontal_line_char(layout.line, charset),
        line_char(layout.line, charset),
    )
}

fn class_route_profile(layout: &RelationLayout<'_>) -> relation_graph::LayeredRelationRouteProfile {
    let endpoint_label_gap = layout
        .top_endpoint_label
        .as_ref()
        .map(RelationGraphLabel::line_count)
        .unwrap_or(0)
        .max(
            layout
                .bottom_endpoint_label
                .as_ref()
                .map(RelationGraphLabel::line_count)
                .unwrap_or(0),
        );

    if endpoint_label_gap > 0 {
        relation_graph::LayeredRelationRouteProfile::class_with_endpoint_labels(endpoint_label_gap)
    } else {
        relation_graph::LayeredRelationRouteProfile::class()
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
        RelationMarker::Lollipop => charset.lollipop,
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
