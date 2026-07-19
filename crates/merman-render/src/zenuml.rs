//! Headless ZenUML geometry derived from the selected ZenUML Core SVG pipeline.

use crate::Result;
use crate::model::Bounds;
use crate::text::{TextMeasurer, TextStyle};
use merman_core::diagrams::zenuml::{
    ZenumlDiagramRenderModel, ZenumlFragmentKind, ZenumlMessageStyle, ZenumlStatement,
    ZenumlStatementKind,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const MARGIN: f64 = 20.0;
const MIN_PARTICIPANT_WIDTH: f64 = 80.0;
const PARTICIPANT_MAX_WIDTH: f64 = 250.0;
const PARTICIPANT_VISUAL_HEIGHT: f64 = 40.0;
const PARTICIPANT_TOP: f64 = 28.0;
const PARTICIPANT_BOX_PADDING: f64 = 16.0;
const PARTICIPANT_ICON_ROW_WIDTH: f64 = 28.0;
const PARTICIPANT_EMOJI_WIDTH: f64 = 20.0;
const ARROW_HEAD_WIDTH: f64 = 10.0;
const OCCURRENCE_WIDTH: f64 = 15.0;
const OCCURRENCE_BAR_SIDE_WIDTH: f64 = (OCCURRENCE_WIDTH - 1.0) / 2.0;
const ROOT_BLOCK_TOP: f64 = 56.0;
const STATEMENT_MARGIN: f64 = 16.0;
const COMMENT_LINE_HEIGHT: f64 = 20.0;
const MESSAGE_HEIGHT: f64 = 16.0;
const SELF_SYNC_MESSAGE_HEIGHT: f64 = 30.0;
const SELF_ASYNC_MESSAGE_HEIGHT: f64 = 44.0;
const CREATION_MESSAGE_HEIGHT: f64 = 40.0;
const OCCURRENCE_EMPTY_HEIGHT: f64 = 24.0;
const OCCURRENCE_BORDER_BOTTOM: f64 = 2.0;
const ASSIGNMENT_RETURN_HEIGHT: f64 = 12.0;
const FRAGMENT_HEADER_HEIGHT: f64 = 25.0;
const FRAGMENT_BORDER_WIDTH: f64 = 1.0;
const FRAGMENT_BRANCH_LABEL_HEIGHT: f64 = 20.0;
const FRAGMENT_BRANCH_MARGIN: f64 = 8.0;
const FRAGMENT_PADDING_BOTTOM: f64 = 10.0;
const FRAGMENT_PADDING_X: f64 = 10.0;
const FRAGMENT_MIN_WIDTH: f64 = 100.0;
const PAR_CHILD_SEPARATOR: f64 = 1.0;
const DIVIDER_HEIGHT: f64 = 40.0;
const SVG_CONTENT_BOTTOM_SPACE: f64 = 13.0;
const RETURN_BOTTOM_SPACE: f64 = 46.0;
const MESSAGE_LABEL_PADDING: f64 = 10.0;
const DEFAULT_STARTER: &str = "_STARTER_";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlDiagramLayout {
    pub width: f64,
    pub height: f64,
    pub frame_border_left: f64,
    pub frame_border_right: f64,
    pub participants: Vec<ZenumlParticipantLayout>,
    pub lifelines: Vec<ZenumlLifelineLayout>,
    pub messages: Vec<ZenumlMessageLayout>,
    pub occurrences: Vec<ZenumlOccurrenceLayout>,
    pub fragments: Vec<ZenumlFragmentLayout>,
    pub dividers: Vec<ZenumlDividerLayout>,
    pub comments: Vec<ZenumlCommentLayout>,
    pub groups: Vec<ZenumlGroupLayout>,
    pub bounds: Bounds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlParticipantLayout {
    pub name: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub is_starter: bool,
    pub show_bottom: bool,
    pub participant_type: Option<String>,
    pub stereotype: Option<String>,
    pub color: Option<String>,
    pub emoji: Option<String>,
    pub group_id: Option<String>,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlLifelineLayout {
    pub participant_name: String,
    pub x: f64,
    pub top_y: f64,
    pub bottom_y: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ZenumlLayoutMessageKind {
    Synchronous,
    Asynchronous,
    Creation,
    Return,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlMessageLayout {
    pub statement_id: String,
    pub number: String,
    pub from: String,
    pub to: String,
    pub from_x: f64,
    pub to_x: f64,
    pub y: f64,
    pub label: String,
    pub kind: ZenumlLayoutMessageKind,
    pub is_self: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlOccurrenceLayout {
    pub statement_id: String,
    pub participant_name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlFragmentLayout {
    pub statement_id: String,
    pub kind: ZenumlFragmentKind,
    pub label: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub header_y: f64,
    pub section_y: Vec<f64>,
    pub section_labels: Vec<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlDividerLayout {
    pub statement_id: String,
    pub y: f64,
    pub width: f64,
    pub label: String,
    pub label_width: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlCommentLayout {
    pub statement_id: String,
    pub x: f64,
    pub y: f64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZenumlGroupLayout {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn layout_zenuml_diagram_typed(
    model: &ZenumlDiagramRenderModel,
    measurer: &dyn TextMeasurer,
) -> Result<ZenumlDiagramLayout> {
    let participant_style = TextStyle {
        font_family: Some("Helvetica, Verdana, serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let stereotype_style = TextStyle {
        font_size: 14.0,
        ..participant_style.clone()
    };
    let message_style = TextStyle {
        font_family: participant_style.font_family.clone(),
        font_size: 14.0,
        font_weight: None,
        font_style: None,
    };

    let mut participants = Vec::with_capacity(model.participants.len());
    let mut internal_widths = Vec::with_capacity(model.participants.len());
    for participant in &model.participants {
        let label = if participant.name == "_STARTER_" {
            String::new()
        } else {
            participant.display_name().to_string()
        };
        let label_width = measurer.measure(&label, &participant_style).width;
        let icon_width = participant
            .participant_type
            .as_ref()
            .map_or(0.0, |_| PARTICIPANT_ICON_ROW_WIDTH);
        let emoji_width = participant
            .emoji
            .as_ref()
            .map_or(0.0, |_| PARTICIPANT_EMOJI_WIDTH);
        let stereotype_width = participant.stereotype.as_ref().map_or(0.0, |stereotype| {
            measurer
                .measure(&format!("«{stereotype}»"), &stereotype_style)
                .width
                + 8.0
        });
        let visual_width = (label_width + PARTICIPANT_BOX_PADDING + icon_width + emoji_width)
            .max(stereotype_width)
            .clamp(MIN_PARTICIPANT_WIDTH, PARTICIPANT_MAX_WIDTH);
        let internal_width = (label_width
            + participant.participant_type.as_ref().map_or(0.0, |_| 40.0)
            + participant.emoji.as_ref().map_or(0.0, |_| 24.0))
        .max(MIN_PARTICIPANT_WIDTH)
            + MARGIN;
        internal_widths.push(internal_width);
        participants.push(ZenumlParticipantLayout {
            name: participant.name.clone(),
            label,
            x: 0.0,
            y: PARTICIPANT_TOP,
            width: visual_width,
            height: PARTICIPANT_VISUAL_HEIGHT,
            is_starter: participant.name == "_STARTER_",
            show_bottom: participant.name != "_STARTER_",
            participant_type: participant.participant_type.clone(),
            stereotype: participant.stereotype.clone(),
            color: participant.color.clone(),
            emoji: participant.emoji.clone(),
            group_id: participant.group_id.clone(),
            created: false,
        });
    }

    let mut x = internal_widths.first().copied().unwrap_or(0.0) / 2.0;
    for (index, participant) in participants.iter_mut().enumerate() {
        if index > 0 {
            x += internal_widths[index - 1] / 2.0 + internal_widths[index] / 2.0;
        }
        participant.x = x;
    }
    apply_message_width_constraints(model, &mut participants, measurer, &message_style);
    let positions: HashMap<String, f64> = participants
        .iter()
        .map(|participant| (participant.name.clone(), participant.x))
        .collect();

    let participant_half_widths: HashMap<String, f64> = participants
        .iter()
        .zip(internal_widths.iter())
        .map(|(participant, width)| (participant.name.clone(), *width / 2.0))
        .collect();
    let mut builder = VerticalLayoutBuilder {
        positions: &positions,
        participant_half_widths: &participant_half_widths,
        measurer,
        message_style: &message_style,
        cursor_y: ROOT_BLOCK_TOP,
        messages: Vec::new(),
        occurrences: Vec::new(),
        fragments: Vec::new(),
        dividers: Vec::new(),
        comments: Vec::new(),
        creation_y: HashMap::new(),
        active_occurrences: HashMap::new(),
        max_fragment_depth: 0,
    };
    builder.cursor_y = builder.layout_block(
        &model.statements,
        ROOT_BLOCK_TOP,
        BlockLayoutContext::root(),
    );

    for participant in &mut participants {
        if let Some(y) = builder.creation_y.get(&participant.name).copied() {
            participant.y = y;
            participant.created = true;
            participant.show_bottom = false;
        }
    }

    let participant_width = participants.last().map_or(0.0, |participant| {
        participant.x
            + participant_half_widths
                .get(&participant.name)
                .copied()
                .unwrap_or_default()
    });
    let mut content_width = (participant_width
        + self_message_extra_width(
            model,
            &positions,
            &participant_half_widths,
            measurer,
            &message_style,
        ))
    .max(FRAGMENT_MIN_WIDTH);
    for message in &builder.messages {
        if message.is_self {
            continue;
        }
        let label_width = measurer.measure(&message.label, &message_style).width;
        content_width = content_width
            .max((message.from_x + message.to_x) / 2.0 + label_width / 2.0 + MESSAGE_LABEL_PADDING);
    }
    for divider in &mut builder.dividers {
        divider.width = content_width;
    }
    for fragment in &mut builder.fragments {
        if fragment.width <= 0.0 {
            fragment.width = content_width;
        }
    }
    let max_occurrence_bottom = builder
        .occurrences
        .iter()
        .map(|occurrence| occurrence.y + occurrence.height + SVG_CONTENT_BOTTOM_SPACE)
        .max_by(f64::total_cmp)
        .unwrap_or_default();
    let max_fragment_bottom = builder
        .fragments
        .iter()
        .map(|fragment| fragment.y + fragment.height + SVG_CONTENT_BOTTOM_SPACE)
        .max_by(f64::total_cmp)
        .unwrap_or_default();
    let max_message_bottom = builder
        .messages
        .iter()
        .map(|message| {
            if message.kind == ZenumlLayoutMessageKind::Return {
                message.y + RETURN_BOTTOM_SPACE
            } else if message.is_self {
                message.y
                    + match message.kind {
                        ZenumlLayoutMessageKind::Asynchronous => SELF_ASYNC_MESSAGE_HEIGHT,
                        _ => SELF_SYNC_MESSAGE_HEIGHT,
                    }
                    + SVG_CONTENT_BOTTOM_SPACE
            } else if message.kind == ZenumlLayoutMessageKind::Creation {
                message.y + PARTICIPANT_VISUAL_HEIGHT + SVG_CONTENT_BOTTOM_SPACE
            } else {
                message.y + SVG_CONTENT_BOTTOM_SPACE
            }
        })
        .max_by(f64::total_cmp)
        .unwrap_or_default();
    let height = (builder.cursor_y + 28.0)
        .max(max_occurrence_bottom)
        .max(max_fragment_bottom)
        .max(max_message_bottom);
    let lifelines = participants
        .iter()
        .map(|participant| ZenumlLifelineLayout {
            participant_name: participant.name.clone(),
            x: participant.x,
            top_y: participant.y + participant.height,
            bottom_y: height + PARTICIPANT_VISUAL_HEIGHT - 28.0,
        })
        .collect();
    let groups = layout_groups(&participants, height);
    let frame_border = builder.max_fragment_depth as f64 * FRAGMENT_PADDING_X;
    let bounds = Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: content_width,
        max_y: height,
    };
    Ok(ZenumlDiagramLayout {
        width: content_width,
        height,
        frame_border_left: frame_border,
        frame_border_right: frame_border,
        participants,
        lifelines,
        messages: builder.messages,
        occurrences: builder.occurrences,
        fragments: builder.fragments,
        dividers: builder.dividers,
        comments: builder.comments,
        groups,
        bounds,
    })
}

fn apply_message_width_constraints(
    model: &ZenumlDiagramRenderModel,
    participants: &mut [ZenumlParticipantLayout],
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) {
    let index: HashMap<String, usize> = participants
        .iter()
        .enumerate()
        .map(|(index, participant)| (participant.name.clone(), index))
        .collect();
    let mut constraints = Vec::new();
    collect_message_constraints(&model.statements, &mut constraints);
    for (from, to, label) in constraints {
        let (Some(&from_index), Some(&to_index)) = (index.get(from), index.get(to)) else {
            continue;
        };
        if from_index == to_index {
            continue;
        }
        let (left, right) = if from_index < to_index {
            (from_index, to_index)
        } else {
            (to_index, from_index)
        };
        let required = measurer.measure(label, style).width + ARROW_HEAD_WIDTH + OCCURRENCE_WIDTH;
        let actual = participants[right].x - participants[left].x;
        if required > actual {
            let delta = required - actual;
            for participant in &mut participants[right..] {
                participant.x += delta;
            }
        }
    }
}

fn collect_message_constraints<'a>(
    statements: &'a [ZenumlStatement],
    out: &mut Vec<(&'a str, &'a str, &'a str)>,
) {
    for statement in statements {
        match &statement.kind {
            ZenumlStatementKind::Message {
                resolved_from,
                resolved_to,
                label,
                style,
                body,
                ..
            } => {
                let (from, to) = message_render_endpoints(
                    *style,
                    resolved_from.as_deref(),
                    resolved_to.as_deref(),
                );
                out.push((from, to, label));
                collect_message_constraints(body, out);
            }
            ZenumlStatementKind::Creation {
                resolved_from,
                resolved_to,
                label,
                body,
                ..
            } => {
                let from = resolved_from.as_deref().unwrap_or(DEFAULT_STARTER);
                out.push((from, resolved_to, label));
                collect_message_constraints(body, out);
            }
            ZenumlStatementKind::Return {
                resolved_from,
                resolved_to,
                label,
                ..
            } => out.push((
                resolved_from.as_deref().unwrap_or(DEFAULT_STARTER),
                resolved_to.as_deref().unwrap_or(DEFAULT_STARTER),
                label,
            )),
            ZenumlStatementKind::Fragment { sections, .. } => {
                for section in sections {
                    collect_message_constraints(&section.statements, out);
                }
            }
            ZenumlStatementKind::Reference { .. } | ZenumlStatementKind::Divider { .. } => {}
        }
    }
}

fn message_render_endpoints<'a>(
    style: ZenumlMessageStyle,
    from: Option<&'a str>,
    to: Option<&'a str>,
) -> (&'a str, &'a str) {
    let from = from.unwrap_or(DEFAULT_STARTER);
    let to = match style {
        ZenumlMessageStyle::Synchronous => to.unwrap_or(DEFAULT_STARTER),
        ZenumlMessageStyle::Asynchronous => to.unwrap_or(from),
    };
    (from, to)
}

fn self_message_extra_width(
    model: &ZenumlDiagramRenderModel,
    positions: &HashMap<String, f64>,
    half_widths: &HashMap<String, f64>,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) -> f64 {
    let Some((right_name, right_position)) = positions
        .iter()
        .max_by(|left, right| left.1.total_cmp(right.1))
    else {
        return 0.0;
    };
    let right_half = half_widths.get(right_name).copied().unwrap_or_default();
    let mut constraints = Vec::new();
    collect_message_constraints(&model.statements, &mut constraints);
    constraints
        .into_iter()
        .filter(|(from, to, _)| from == to)
        .filter_map(|(from, _, label)| {
            positions.get(from).map(|from_position| {
                measurer.measure(label, style).width - (right_position - from_position) - right_half
            })
        })
        .fold(0.0, f64::max)
}

struct VerticalLayoutBuilder<'a> {
    positions: &'a HashMap<String, f64>,
    participant_half_widths: &'a HashMap<String, f64>,
    measurer: &'a dyn TextMeasurer,
    message_style: &'a TextStyle,
    cursor_y: f64,
    messages: Vec<ZenumlMessageLayout>,
    occurrences: Vec<ZenumlOccurrenceLayout>,
    fragments: Vec<ZenumlFragmentLayout>,
    dividers: Vec<ZenumlDividerLayout>,
    comments: Vec<ZenumlCommentLayout>,
    creation_y: HashMap<String, f64>,
    active_occurrences: HashMap<String, usize>,
    max_fragment_depth: usize,
}

#[derive(Clone, Copy)]
struct BlockLayoutContext<'a> {
    parent_kind: Option<ZenumlFragmentKind>,
    inside_occurrence: bool,
    fragment_depth: usize,
    parent_number: &'a str,
    index_offset: usize,
}

impl BlockLayoutContext<'static> {
    const fn root() -> Self {
        Self {
            parent_kind: None,
            inside_occurrence: false,
            fragment_depth: 0,
            parent_number: "",
            index_offset: 0,
        }
    }
}

impl VerticalLayoutBuilder<'_> {
    fn layout_block(
        &mut self,
        statements: &[ZenumlStatement],
        start_top: f64,
        context: BlockLayoutContext<'_>,
    ) -> f64 {
        if statements.is_empty() {
            return start_top;
        }
        let mut cursor = start_top + STATEMENT_MARGIN;
        for (index, statement) in statements.iter().enumerate() {
            if context.parent_kind == Some(ZenumlFragmentKind::Parallel) && index != 0 {
                cursor += PAR_CHILD_SEPARATOR;
            }
            let ordinal = context.index_offset + index + 1;
            let number = if context.parent_number.is_empty() {
                ordinal.to_string()
            } else {
                format!("{}.{ordinal}", context.parent_number)
            };
            cursor = self.layout_statement(
                statement,
                &number,
                cursor,
                context.inside_occurrence,
                index + 1 == statements.len(),
                context.fragment_depth,
            ) + STATEMENT_MARGIN;
        }
        cursor
    }

    fn layout_statement(
        &mut self,
        statement: &ZenumlStatement,
        number: &str,
        top: f64,
        inside_occurrence: bool,
        is_last: bool,
        fragment_depth: usize,
    ) -> f64 {
        let comment_height = statement.comment.as_deref().map_or(0.0, |comment| {
            comment.lines().count() as f64 * COMMENT_LINE_HEIGHT
        });
        let comment_index = statement.comment.as_ref().map(|comment| {
            let index = self.comments.len();
            self.comments.push(ZenumlCommentLayout {
                statement_id: statement.id.clone(),
                x: self.statement_origin_x(statement),
                y: top + 15.0,
                text: comment.clone(),
            });
            index
        });
        let content_top = top + comment_height;

        match &statement.kind {
            ZenumlStatementKind::Message {
                resolved_from,
                resolved_to,
                label,
                assignment,
                style,
                body,
                ..
            } => {
                let (from, to) = message_render_endpoints(
                    *style,
                    resolved_from.as_deref(),
                    resolved_to.as_deref(),
                );
                self.layout_message(
                    statement,
                    number,
                    from,
                    to,
                    label,
                    assignment.as_deref(),
                    *style,
                    body,
                    content_top,
                    fragment_depth,
                )
            }
            ZenumlStatementKind::Creation {
                resolved_from,
                resolved_to,
                label,
                assignment,
                body,
                ..
            } => self.layout_creation(
                statement,
                number,
                resolved_from.as_deref().unwrap_or(DEFAULT_STARTER),
                resolved_to,
                label,
                assignment.as_deref(),
                body,
                content_top,
                fragment_depth,
            ),
            ZenumlStatementKind::Return {
                resolved_from,
                resolved_to,
                label,
                ..
            } => {
                let from = resolved_from.as_deref().unwrap_or(DEFAULT_STARTER);
                let to = resolved_to.as_deref().unwrap_or(DEFAULT_STARTER);
                let is_self = from == to;
                let collapsed = !is_self && inside_occurrence && is_last;
                let (from_x, to_x) =
                    self.return_endpoints(from, to, self.active_depth(from), self.active_depth(to));
                let y = if is_self {
                    content_top + 20.0
                } else if collapsed {
                    content_top + 16.5
                } else {
                    content_top + 15.5
                };
                self.messages.push(ZenumlMessageLayout {
                    statement_id: statement.id.clone(),
                    number: number.to_string(),
                    from: from.to_string(),
                    to: to.to_string(),
                    from_x,
                    to_x,
                    y,
                    label: label.clone(),
                    kind: ZenumlLayoutMessageKind::Return,
                    is_self,
                });
                // ZenUML Core's vertical VM collapses a final non-self return inside an
                // occurrence, then its SVG pipeline restores the missing 16px as return debt.
                content_top + if is_self { 20.0 } else { MESSAGE_HEIGHT }
            }
            ZenumlStatementKind::Fragment {
                fragment_kind,
                label,
                sections,
            } => {
                self.max_fragment_depth = self.max_fragment_depth.max(fragment_depth + 1);
                let mut cursor = content_top + FRAGMENT_BORDER_WIDTH + FRAGMENT_HEADER_HEIGHT;
                let mut section_y = Vec::with_capacity(sections.len());
                let mut section_labels = Vec::with_capacity(sections.len());
                let mut names = HashSet::new();
                let mut section_offset = 0;

                for (index, section) in sections.iter().enumerate() {
                    collect_participant_names(&section.statements, &mut names);
                    section_labels.push(section.label.clone());
                    match fragment_kind {
                        ZenumlFragmentKind::Alternative => {
                            if index == 0 {
                                section_y.push(cursor);
                                cursor += FRAGMENT_BRANCH_LABEL_HEIGHT;
                            } else {
                                section_y.push(cursor);
                                cursor += FRAGMENT_BRANCH_LABEL_HEIGHT
                                    + FRAGMENT_BRANCH_MARGIN
                                    + FRAGMENT_BORDER_WIDTH;
                            }
                        }
                        ZenumlFragmentKind::TryCatchFinally => {
                            section_y.push(cursor);
                            if index > 0 {
                                cursor += FRAGMENT_BRANCH_LABEL_HEIGHT
                                    + FRAGMENT_BRANCH_MARGIN
                                    + FRAGMENT_BORDER_WIDTH;
                            }
                        }
                        _ => {
                            section_y.push(cursor);
                            if section.label.is_some() {
                                cursor += FRAGMENT_BRANCH_LABEL_HEIGHT;
                            }
                        }
                    }
                    let parent_kind = (*fragment_kind == ZenumlFragmentKind::Parallel)
                        .then_some(ZenumlFragmentKind::Parallel);
                    cursor = self.layout_block(
                        &section.statements,
                        cursor,
                        BlockLayoutContext {
                            parent_kind,
                            inside_occurrence,
                            fragment_depth: fragment_depth + 1,
                            parent_number: number,
                            index_offset: section_offset,
                        },
                    );
                    section_offset += section.statements.len();
                }
                cursor += FRAGMENT_PADDING_BOTTOM + FRAGMENT_BORDER_WIDTH;
                let (x, width) = self.fragment_horizontal_bounds(&names, fragment_depth);
                if let Some(index) = comment_index {
                    self.comments[index].x = x + 1.0;
                }
                self.fragments.push(ZenumlFragmentLayout {
                    statement_id: statement.id.clone(),
                    kind: *fragment_kind,
                    label: label.clone(),
                    x,
                    y: top,
                    width,
                    height: cursor - top,
                    header_y: top + FRAGMENT_BORDER_WIDTH + comment_height,
                    section_y,
                    section_labels,
                });
                cursor
            }
            ZenumlStatementKind::Reference {
                participants,
                label,
            } => {
                self.max_fragment_depth = self.max_fragment_depth.max(fragment_depth + 1);
                let names = participants.iter().cloned().collect::<HashSet<_>>();
                let (x, width) = self.fragment_horizontal_bounds(&names, fragment_depth);
                if let Some(index) = comment_index {
                    self.comments[index].x = x + 1.0;
                }
                let end = content_top + FRAGMENT_HEADER_HEIGHT + FRAGMENT_PADDING_BOTTOM;
                self.fragments.push(ZenumlFragmentLayout {
                    statement_id: statement.id.clone(),
                    kind: ZenumlFragmentKind::Section,
                    label: Some(format!("ref {label}")),
                    x,
                    y: top,
                    width,
                    height: end - top,
                    header_y: top + comment_height,
                    section_y: Vec::new(),
                    section_labels: Vec::new(),
                });
                end
            }
            ZenumlStatementKind::Divider { label } => {
                self.dividers.push(ZenumlDividerLayout {
                    statement_id: statement.id.clone(),
                    y: content_top + DIVIDER_HEIGHT / 2.0,
                    width: 0.0,
                    label: label.clone(),
                    label_width: self.measurer.measure(label, self.message_style).width,
                });
                content_top + DIVIDER_HEIGHT
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_message(
        &mut self,
        statement: &ZenumlStatement,
        number: &str,
        from: &str,
        to: &str,
        label: &str,
        assignment: Option<&str>,
        style: ZenumlMessageStyle,
        body: &[ZenumlStatement],
        content_top: f64,
        fragment_depth: usize,
    ) -> f64 {
        let is_self = from == to;
        let message_height = match (style, is_self) {
            (ZenumlMessageStyle::Synchronous, true) => SELF_SYNC_MESSAGE_HEIGHT,
            (ZenumlMessageStyle::Asynchronous, true) => SELF_ASYNC_MESSAGE_HEIGHT,
            _ => MESSAGE_HEIGHT,
        };
        let (from_x, to_x, target_depth) = self.message_endpoints(from, to, style);
        let y = if is_self {
            content_top
        } else {
            content_top + message_height - 0.5
        };
        self.messages.push(ZenumlMessageLayout {
            statement_id: statement.id.clone(),
            number: number.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            from_x,
            to_x,
            y,
            label: label.to_string(),
            kind: match style {
                ZenumlMessageStyle::Synchronous => ZenumlLayoutMessageKind::Synchronous,
                ZenumlMessageStyle::Asynchronous => ZenumlLayoutMessageKind::Asynchronous,
            },
            is_self,
        });

        let mut cursor = content_top + message_height;
        if style != ZenumlMessageStyle::Synchronous {
            return cursor;
        }

        let occurrence_start = content_top + message_height - 2.0;
        self.enter_occurrence(to);
        cursor = if body.is_empty() {
            cursor + 22.0
        } else {
            self.layout_block(
                body,
                cursor,
                BlockLayoutContext {
                    parent_kind: None,
                    inside_occurrence: true,
                    fragment_depth,
                    parent_number: number,
                    index_offset: 0,
                },
            ) + OCCURRENCE_BORDER_BOTTOM
        };
        self.leave_occurrence(to);

        if let Some(assignment) = assignment.filter(|_| !is_self) {
            cursor += ASSIGNMENT_RETURN_HEIGHT;
            let (return_from_x, return_to_x) =
                self.return_endpoints(to, from, target_depth + 1, self.active_depth(from));
            self.messages.push(ZenumlMessageLayout {
                statement_id: format!("{}-assignment-return", statement.id),
                number: format!("{number}.{}", body.len() + 1),
                from: to.to_string(),
                to: from.to_string(),
                from_x: return_from_x,
                to_x: return_to_x,
                y: cursor - OCCURRENCE_BORDER_BOTTOM,
                label: assignment.to_string(),
                kind: ZenumlLayoutMessageKind::Return,
                is_self: false,
            });
        }
        self.occurrences.push(ZenumlOccurrenceLayout {
            statement_id: statement.id.clone(),
            participant_name: to.to_string(),
            x: self.position(to) - OCCURRENCE_BAR_SIDE_WIDTH
                + target_depth as f64 * OCCURRENCE_BAR_SIDE_WIDTH,
            y: occurrence_start,
            width: OCCURRENCE_WIDTH,
            height: (cursor - occurrence_start).max(OCCURRENCE_EMPTY_HEIGHT),
        });
        cursor
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_creation(
        &mut self,
        statement: &ZenumlStatement,
        number: &str,
        from: &str,
        to: &str,
        label: &str,
        assignment: Option<&str>,
        body: &[ZenumlStatement],
        content_top: f64,
        fragment_depth: usize,
    ) -> f64 {
        let target_depth = self.active_depth(to);
        let from_x = self.creation_sender_x(from, to);
        let to_x = self.position(to);
        self.creation_y.entry(to.to_string()).or_insert(content_top);
        self.messages.push(ZenumlMessageLayout {
            statement_id: statement.id.clone(),
            number: number.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            from_x,
            to_x,
            y: content_top + CREATION_MESSAGE_HEIGHT / 2.0,
            label: label.to_string(),
            kind: ZenumlLayoutMessageKind::Creation,
            is_self: false,
        });

        let occurrence_start = content_top + CREATION_MESSAGE_HEIGHT - 2.0;
        let mut cursor = content_top + CREATION_MESSAGE_HEIGHT;
        self.enter_occurrence(to);
        cursor = if body.is_empty() {
            cursor + 22.0
        } else {
            self.layout_block(
                body,
                cursor,
                BlockLayoutContext {
                    parent_kind: None,
                    inside_occurrence: true,
                    fragment_depth,
                    parent_number: number,
                    index_offset: 0,
                },
            ) + OCCURRENCE_BORDER_BOTTOM
        };
        self.leave_occurrence(to);
        if let Some(assignment) = assignment {
            cursor += ASSIGNMENT_RETURN_HEIGHT;
            let (return_from_x, return_to_x) =
                self.return_endpoints(to, from, target_depth + 1, self.active_depth(from));
            self.messages.push(ZenumlMessageLayout {
                statement_id: format!("{}-assignment-return", statement.id),
                number: format!("{number}.{}", body.len() + 1),
                from: to.to_string(),
                to: from.to_string(),
                from_x: return_from_x,
                to_x: return_to_x,
                y: cursor - OCCURRENCE_BORDER_BOTTOM,
                label: assignment.to_string(),
                kind: ZenumlLayoutMessageKind::Return,
                is_self: false,
            });
        }
        self.occurrences.push(ZenumlOccurrenceLayout {
            statement_id: statement.id.clone(),
            participant_name: to.to_string(),
            x: to_x - OCCURRENCE_BAR_SIDE_WIDTH + target_depth as f64 * OCCURRENCE_BAR_SIDE_WIDTH,
            y: occurrence_start,
            width: OCCURRENCE_WIDTH,
            height: (cursor - occurrence_start).max(OCCURRENCE_EMPTY_HEIGHT),
        });
        cursor
    }

    fn position(&self, participant: &str) -> f64 {
        self.positions.get(participant).copied().unwrap_or(0.0)
    }

    fn statement_origin_x(&self, statement: &ZenumlStatement) -> f64 {
        match &statement.kind {
            ZenumlStatementKind::Message { resolved_from, .. }
            | ZenumlStatementKind::Creation { resolved_from, .. }
            | ZenumlStatementKind::Return { resolved_from, .. } => {
                self.position(resolved_from.as_deref().unwrap_or(DEFAULT_STARTER))
            }
            _ => 1.0,
        }
    }

    fn fragment_horizontal_bounds(&self, names: &HashSet<String>, depth: usize) -> (f64, f64) {
        let mut participant_names = names
            .iter()
            .filter(|name| self.positions.contains_key(*name))
            .map(String::as_str)
            .collect::<Vec<_>>();
        if participant_names.is_empty() {
            return (0.0, 0.0);
        }
        participant_names
            .sort_by(|left, right| self.position(left).total_cmp(&self.position(right)));
        let left_name = participant_names[0];
        let right_name = participant_names[participant_names.len() - 1];
        let left = self.position(left_name) - self.half_width(left_name);
        let right = self.position(right_name) + self.half_width(right_name);
        let border = depth as f64 * FRAGMENT_PADDING_X;
        (
            left - border,
            (right - left + 2.0 * border).max(FRAGMENT_MIN_WIDTH),
        )
    }

    fn half_width(&self, participant: &str) -> f64 {
        self.participant_half_widths
            .get(participant)
            .copied()
            .unwrap_or(MIN_PARTICIPANT_WIDTH / 2.0)
    }

    fn active_depth(&self, participant: &str) -> usize {
        self.active_occurrences
            .get(participant)
            .copied()
            .unwrap_or(0)
    }

    fn enter_occurrence(&mut self, participant: &str) {
        *self
            .active_occurrences
            .entry(participant.to_string())
            .or_default() += 1;
    }

    fn leave_occurrence(&mut self, participant: &str) {
        if let Some(depth) = self.active_occurrences.get_mut(participant) {
            *depth = depth.saturating_sub(1);
            if *depth == 0 {
                self.active_occurrences.remove(participant);
            }
        }
    }

    fn message_endpoints(
        &self,
        from: &str,
        to: &str,
        style: ZenumlMessageStyle,
    ) -> (f64, f64, usize) {
        let raw_from = self.position(from);
        let raw_to = self.position(to);
        let sender_depth = self.active_depth(from);
        let target_depth = self.active_depth(to);
        if from == to {
            return (
                raw_from + sender_depth as f64 * OCCURRENCE_BAR_SIDE_WIDTH,
                raw_to,
                target_depth,
            );
        }
        let left_to_right = raw_from < raw_to;
        let from_x = if sender_depth == 0 {
            raw_from
        } else if left_to_right {
            raw_from + sender_depth as f64 * OCCURRENCE_BAR_SIDE_WIDTH
        } else {
            raw_from - OCCURRENCE_BAR_SIDE_WIDTH
                + sender_depth.saturating_sub(1) as f64 * OCCURRENCE_BAR_SIDE_WIDTH
        };
        let to_x = match style {
            ZenumlMessageStyle::Synchronous => {
                if left_to_right {
                    raw_to - OCCURRENCE_BAR_SIDE_WIDTH
                        + target_depth as f64 * OCCURRENCE_BAR_SIDE_WIDTH
                } else {
                    raw_to
                        + OCCURRENCE_BAR_SIDE_WIDTH
                        + target_depth as f64 * OCCURRENCE_BAR_SIDE_WIDTH
                }
            }
            ZenumlMessageStyle::Asynchronous if target_depth > 0 => {
                if left_to_right {
                    raw_to - target_depth as f64 * OCCURRENCE_BAR_SIDE_WIDTH
                } else {
                    raw_to + target_depth as f64 * OCCURRENCE_BAR_SIDE_WIDTH
                }
            }
            ZenumlMessageStyle::Asynchronous => raw_to,
        };
        (from_x, to_x, target_depth)
    }

    fn creation_sender_x(&self, from: &str, to: &str) -> f64 {
        let raw_from = self.position(from);
        let raw_to = self.position(to);
        let depth = self.active_depth(from) as f64;
        if raw_from < raw_to {
            raw_from + depth * OCCURRENCE_BAR_SIDE_WIDTH
        } else {
            raw_from - depth * OCCURRENCE_BAR_SIDE_WIDTH
        }
    }

    fn return_endpoints(
        &self,
        from: &str,
        to: &str,
        from_layers: usize,
        to_layers: usize,
    ) -> (f64, f64) {
        let raw_from = self.position(from);
        let raw_to = self.position(to);
        let reverse = raw_to < raw_from;
        let from_x = if reverse {
            if from_layers == 0 {
                raw_from
            } else {
                raw_from + from_layers.saturating_sub(1) as f64 * OCCURRENCE_BAR_SIDE_WIDTH
                    - OCCURRENCE_BAR_SIDE_WIDTH
            }
        } else {
            raw_from + from_layers as f64 * OCCURRENCE_BAR_SIDE_WIDTH + 1.0
        };
        let to_x = if reverse {
            raw_to + to_layers as f64 * OCCURRENCE_BAR_SIDE_WIDTH + 1.0
        } else if to_layers == 0 {
            raw_to
        } else {
            raw_to + to_layers.saturating_sub(1) as f64 * OCCURRENCE_BAR_SIDE_WIDTH
                - OCCURRENCE_BAR_SIDE_WIDTH
        };
        (from_x, to_x)
    }
}

fn collect_participant_names(statements: &[ZenumlStatement], names: &mut HashSet<String>) {
    for statement in statements {
        match &statement.kind {
            ZenumlStatementKind::Message {
                resolved_from,
                resolved_to,
                style,
                body,
                ..
            } => {
                let (from, to) = message_render_endpoints(
                    *style,
                    resolved_from.as_deref(),
                    resolved_to.as_deref(),
                );
                names.insert(from.to_string());
                names.insert(to.to_string());
                collect_participant_names(body, names);
            }
            ZenumlStatementKind::Creation {
                resolved_from,
                resolved_to,
                body,
                ..
            } => {
                names.insert(
                    resolved_from
                        .as_deref()
                        .unwrap_or(DEFAULT_STARTER)
                        .to_string(),
                );
                names.insert(resolved_to.clone());
                collect_participant_names(body, names);
            }
            ZenumlStatementKind::Return {
                resolved_from,
                resolved_to,
                ..
            } => {
                names.insert(
                    resolved_from
                        .as_deref()
                        .unwrap_or(DEFAULT_STARTER)
                        .to_string(),
                );
                names.insert(
                    resolved_to
                        .as_deref()
                        .unwrap_or(DEFAULT_STARTER)
                        .to_string(),
                );
            }
            ZenumlStatementKind::Fragment { sections, .. } => {
                for section in sections {
                    collect_participant_names(&section.statements, names);
                }
            }
            ZenumlStatementKind::Reference { participants, .. } => {
                names.extend(participants.iter().cloned());
            }
            ZenumlStatementKind::Divider { .. } => {}
        }
    }
}

fn layout_groups(participants: &[ZenumlParticipantLayout], height: f64) -> Vec<ZenumlGroupLayout> {
    let mut grouped: HashMap<&str, Vec<&ZenumlParticipantLayout>> = HashMap::new();
    for participant in participants {
        if let Some(group_id) = participant.group_id.as_deref() {
            grouped.entry(group_id).or_default().push(participant);
        }
    }
    grouped
        .into_iter()
        .filter_map(|(name, members)| {
            let left = members
                .iter()
                .map(|participant| participant.x - participant.width / 2.0)
                .min_by(f64::total_cmp)?;
            let right = members
                .iter()
                .map(|participant| participant.x + participant.width / 2.0)
                .max_by(f64::total_cmp)?;
            Some(ZenumlGroupLayout {
                name: name.to_string(),
                x: left - 2.0,
                y: PARTICIPANT_TOP - 18.5,
                width: right - left + 4.0,
                height: height - PARTICIPANT_TOP + 31.5,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::DeterministicTextMeasurer;
    use merman_core::{Engine, ParseOptions, RenderSemanticModel};

    fn layout(source: &str) -> ZenumlDiagramLayout {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Zenuml(model) = parsed.model else {
            panic!("expected ZenUML model");
        };
        layout_zenuml_diagram_typed(&model, &DeterministicTextMeasurer::default()).unwrap()
    }

    #[test]
    fn advanced_topology_keeps_nested_owners_and_fragments() {
        let source = "zenuml\n@Starter(Client)\nA.one() {\n  if(ok) {\n    B.two()\n  }\n}\n";
        let layout = layout(source);
        assert_eq!(layout.messages.len(), 2);
        assert_eq!(layout.fragments.len(), 1);
        assert_eq!(layout.occurrences.len(), 2);
    }

    #[test]
    fn root_block_and_empty_occurrence_follow_the_oracle_vertical_vm() {
        let layout = layout("zenuml\nA.call()\n");
        let message = &layout.messages[0];
        let occurrence = &layout.occurrences[0];

        // VerticalCoordinates starts the root block at 56px; BlockVM then applies a 16px
        // statement margin before the 16px non-self message.
        assert_eq!(message.y, 87.5);
        assert_eq!(occurrence.y, 86.0);
        assert_eq!(occurrence.height, OCCURRENCE_EMPTY_HEIGHT);
        assert_eq!(layout.height, 154.0);
    }

    #[test]
    fn final_return_debt_expands_its_parent_occurrence() {
        let without_return = layout("zenuml\nA.call() {\n  B.work()\n}\n");
        let with_return = layout("zenuml\nA.call() {\n  B.work()\n  return done\n}\n");
        let outer_without = without_return
            .occurrences
            .iter()
            .find(|occurrence| occurrence.statement_id == "zenuml-statement-1")
            .unwrap();
        let outer_with = with_return
            .occurrences
            .iter()
            .find(|occurrence| occurrence.statement_id == "zenuml-statement-1")
            .unwrap();

        assert!(
            with_return.height > without_return.height,
            "return debt must increase rendered height: without={without_return:?}, with={with_return:?}"
        );
        assert!(outer_with.height >= outer_without.height);
        assert!(
            with_return
                .messages
                .iter()
                .any(|message| message.kind == ZenumlLayoutMessageKind::Return)
        );
    }

    #[test]
    fn divider_background_uses_the_operation_text_measurer() {
        let source = "zenuml\n== Wide label ==\n";
        let layout = layout(source);
        let divider = &layout.dividers[0];
        let expected = DeterministicTextMeasurer::default().measure(
            &divider.label,
            &TextStyle {
                font_family: Some("Helvetica, Verdana, serif".to_string()),
                font_size: 14.0,
                font_weight: None,
                font_style: None,
            },
        );
        assert_eq!(divider.label_width, expected.width);
    }

    #[test]
    fn endpoint_fallbacks_are_statement_kind_specific() {
        // Both selected @zenuml/core 3.47.8 and candidate 3.50.1 keep missing-target
        // `_STARTER_` coordinates without adding it to OrderedParticipants.
        let synchronous = layout("zenuml\n@Starter(A)\nmethod()\n");
        assert_eq!(
            synchronous
                .participants
                .iter()
                .map(|participant| participant.name.as_str())
                .collect::<Vec<_>>(),
            ["A"]
        );
        assert_eq!(synchronous.messages[0].from, "A");
        assert_eq!(synchronous.messages[0].to, DEFAULT_STARTER);
        assert_eq!(synchronous.messages[0].to_x, 7.0);

        let asynchronous = layout("zenuml\nA ->\n");
        assert_eq!(
            asynchronous
                .participants
                .iter()
                .map(|participant| participant.name.as_str())
                .collect::<Vec<_>>(),
            ["A"]
        );
        assert_eq!(asynchronous.messages[0].from, "A");
        assert_eq!(asynchronous.messages[0].to, "A");

        let returned = layout("zenuml\nA -->\n");
        assert_eq!(
            returned
                .participants
                .iter()
                .map(|participant| participant.name.as_str())
                .collect::<Vec<_>>(),
            ["A"]
        );
        assert_eq!(returned.messages[0].from, "A");
        assert_eq!(returned.messages[0].to, DEFAULT_STARTER);
        assert_eq!(returned.messages[0].to_x, 1.0);
    }

    #[test]
    fn renderer_numbers_fragment_sections_with_one_cumulative_offset() {
        let layout = layout("zenuml\nif(x) { A.m() } else if(y) { B.m() } else { C.m() }\n");
        assert_eq!(
            layout
                .messages
                .iter()
                .map(|message| message.number.as_str())
                .collect::<Vec<_>>(),
            ["1.1", "1.2", "1.3"]
        );
    }

    #[test]
    fn source_width_is_not_a_native_svg_geometry_input() {
        let without_width = layout("zenuml\n@Actor A\n@Boundary B\nA->B.m()\n");
        let with_width = layout("zenuml\n@Actor A 400\n@Boundary B 1\nA->B.m()\n");
        let geometry = |layout: &ZenumlDiagramLayout| {
            layout
                .participants
                .iter()
                .map(|participant| (participant.name.clone(), participant.x, participant.width))
                .collect::<Vec<_>>()
        };
        assert_eq!(geometry(&without_width), geometry(&with_width));
    }
}
