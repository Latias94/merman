use super::plan::{ArrowKind, Event, EventKind, Plan};
use super::{allocation_failed, push, reserve};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::try_plan_normalized_label_lines;
use crate::{AsciiRenderOptions, Result};
use merman_model::families::zenuml::{ZenumlDiagramModel, ZenumlStatementKind};

const CAPTION_WIDTH: usize = 32;
const PARTICIPANT_WIDTH: usize = 16;

pub(super) struct Label {
    pub(super) rows: Vec<String>,
    pub(super) width: usize,
}

impl Label {
    fn height(&self) -> usize {
        self.rows.len()
    }
}

pub(super) struct Participant {
    pub(super) label: Label,
    pub(super) center: usize,
    pub(super) half: usize,
    pub(super) top: usize,
}

pub(super) struct Group {
    pub(super) label: Label,
    pub(super) left: usize,
    pub(super) right: usize,
    pub(super) top: usize,
}

pub(super) struct PlacedEvent<'a> {
    pub(super) event: Event<'a>,
    pub(super) label: Label,
    pub(super) top: usize,
    pub(super) arrow_y: usize,
}

pub(super) struct Frame {
    pub(super) depth: usize,
    pub(super) top: usize,
    pub(super) bottom: usize,
}

pub(super) struct Activation {
    pub(super) participant: usize,
    pub(super) row: usize,
    pub(super) opening: bool,
}

pub(super) struct Scene<'a> {
    pub(super) title: Option<Label>,
    pub(super) participants: Vec<Participant>,
    pub(super) groups: Vec<Group>,
    pub(super) events: Vec<PlacedEvent<'a>>,
    pub(super) frames: Vec<Frame>,
    pub(super) activations: Vec<Activation>,
    pub(super) width: usize,
    pub(super) height: usize,
}

pub(super) fn prepare<'a>(
    model: &ZenumlDiagramModel,
    plan: &Plan<'a>,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> Result<Scene<'a>> {
    let count = model.participants.len();
    let mut participants = Vec::new();
    reserve(&mut participants, count)?;
    for (index, participant) in model.participants.iter().enumerate() {
        let key = format!("P{} ", index + 1);
        let name = if participant.name == "_STARTER_" && participant.is_starter {
            "starter"
        } else {
            participant.display_name()
        };
        let label = label(&[&key, name], PARTICIPANT_WIDTH, options, resources)?;
        let half = resources.checked_grid_add(label.width, 3)? / 2;
        participants.push(Participant {
            label,
            center: 0,
            half,
            top: 0,
        });
    }
    let mut groups = Vec::new();
    reserve(&mut groups, plan.groups.len())?;
    for group in &plan.groups {
        let key = format!("G{} ", group.index + 1);
        let group_label = label(
            &[
                &key,
                model.groups[group.index].id.as_deref().unwrap_or("group"),
            ],
            20,
            options,
            resources,
        )?;
        if group.left == group.right {
            participants[group.left].half = participants[group.left]
                .half
                .max(resources.checked_grid_add(group_label.width, 3)? / 2);
        }
        groups.push(Group {
            label: group_label,
            left: group.left,
            right: group.right,
            top: 0,
        });
    }
    let title = model
        .title
        .as_deref()
        .map(|title| label(&[title], 64, options, resources))
        .transpose()?;
    let mut events = Vec::new();
    reserve(&mut events, plan.events.len())?;
    for &event in &plan.events {
        let caption = caption(event, options, resources)?;
        if let EventKind::Reference { left, right, .. } = event.kind
            && left == right
        {
            participants[left].half = participants[left]
                .half
                .max(resources.checked_grid_add(caption.width, 3)? / 2);
        }
        events.push(PlacedEvent {
            event,
            label: caption,
            top: 0,
            arrow_y: 0,
        });
    }
    let margin = resources.checked_grid_add(resources.checked_grid_mul(plan.frame_depth, 2)?, 2)?;
    let mut constraints: Vec<Vec<(usize, usize)>> = Vec::new();
    reserve(&mut constraints, count)?;
    constraints.resize_with(count, Vec::new);
    for index in 1..count {
        let gap = resources.checked_grid_add(
            resources.checked_grid_add(participants[index - 1].half, participants[index].half)?,
            5,
        )?;
        push(&mut constraints[index], (index - 1, gap))?;
    }
    let mut tail = resources.checked_grid_add(participants[count - 1].half, 2)?;
    let mut minimum_width = title.as_ref().map_or(0, |title| title.width);
    for placed in &events {
        resources.charge_layout_work(1)?;
        let pair = match placed.event.kind {
            EventKind::Arrow { from, to, kind, .. } => {
                let mut needed = resources.checked_grid_add(placed.label.width, 4)?;
                if matches!(kind, ArrowKind::Create) {
                    needed = resources.checked_grid_add(needed, participants[to].half)?;
                }
                Some((from, to, needed))
            }
            EventKind::AssignmentReturn { from, to, .. } => {
                Some((from, to, resources.checked_grid_add(placed.label.width, 4)?))
            }
            EventKind::EndCall { target } => Some((
                target,
                target,
                resources.checked_grid_add(placed.label.width, 4)?,
            )),
            EventKind::Reference { left, right, .. } => {
                let needed = resources
                    .checked_grid_add(placed.label.width, 4)?
                    .saturating_sub(participants[left].half)
                    .saturating_sub(participants[right].half);
                if left != right {
                    push(&mut constraints[right], (left, needed))?;
                }
                None
            }
            _ => {
                minimum_width = minimum_width.max(resources.checked_grid_add(
                    placed.label.width,
                    resources.checked_grid_add(resources.checked_grid_mul(margin, 2)?, 4)?,
                )?);
                None
            }
        };
        if let Some((from, to, needed)) = pair {
            if from == to {
                if from + 1 < count {
                    push(
                        &mut constraints[from + 1],
                        (
                            from,
                            resources.checked_grid_add(needed, participants[from + 1].half)?,
                        ),
                    )?;
                } else {
                    tail = tail.max(needed);
                }
            } else {
                push(&mut constraints[from.max(to)], (from.min(to), needed))?;
            }
        }
    }
    for (index, group) in plan.groups.iter().enumerate() {
        if group.left != group.right {
            let needed = resources
                .checked_grid_add(groups[index].label.width, 4)?
                .saturating_sub(participants[group.left].half)
                .saturating_sub(participants[group.right].half);
            push(&mut constraints[group.right], (group.left, needed))?;
        }
    }
    participants[0].center = resources.checked_grid_add(margin, participants[0].half)?;
    for index in 1..count {
        let mut center = 0;
        for &(left, gap) in &constraints[index] {
            resources.charge_layout_work(1)?;
            center = center.max(resources.checked_grid_add(participants[left].center, gap)?);
        }
        participants[index].center = center;
    }
    let width = minimum_width.max(resources.checked_grid_add(
        resources.checked_grid_add(participants[count - 1].center, tail)?,
        resources.checked_grid_add(margin, 1)?,
    )?);
    let title_height = title.as_ref().map_or(0, |title| title.height() + 1);
    let mut group_height = 0;
    for group in &mut groups {
        group.left = participants[group.left].center - participants[group.left].half - 1;
        group.right = participants[group.right].center + participants[group.right].half + 1;
        group.top = title_height;
        group_height = group_height.max(resources.checked_grid_add(group.label.height(), 3)?);
    }
    let header_top = resources.checked_grid_add(title_height, group_height)?;
    let mut next_y = header_top;
    for (index, participant) in participants.iter_mut().enumerate() {
        participant.top = if plan.created_at[index].is_some() {
            usize::MAX
        } else {
            header_top
        };
        next_y = next_y.max(resources.checked_grid_add(
            header_top,
            resources.checked_grid_add(participant.label.height(), 3)?,
        )?);
    }
    let mut open_frames = Vec::new();
    let mut frames = Vec::new();
    let mut activations = Vec::new();
    for placed in &mut events {
        resources.charge_layout_work(1)?;
        placed.top = next_y;
        let height = placed.label.height();
        let advance = match placed.event.kind {
            EventKind::Arrow {
                from,
                to,
                kind,
                call,
                ..
            } => {
                placed.arrow_y = resources.checked_grid_add(next_y, height)?;
                if matches!(kind, ArrowKind::Create) {
                    placed.arrow_y = resources.checked_grid_add(placed.arrow_y, 1)?;
                    participants[to].top = placed.arrow_y - 1;
                }
                if call {
                    push(
                        &mut activations,
                        Activation {
                            participant: to,
                            row: resources.checked_grid_add(placed.arrow_y, 1)?,
                            opening: true,
                        },
                    )?;
                }
                if matches!(kind, ArrowKind::Create) {
                    resources.checked_grid_add(
                        height,
                        resources.checked_grid_add(participants[to].label.height(), 4)?,
                    )?
                } else {
                    resources.checked_grid_add(height, if from == to { 5 } else { 2 })?
                }
            }
            EventKind::AssignmentReturn { .. } => {
                placed.arrow_y = resources.checked_grid_add(next_y, height)?;
                resources.checked_grid_add(height, 2)?
            }
            EventKind::Open { depth, .. } => {
                push(&mut open_frames, (depth, next_y))?;
                resources.checked_grid_add(height, 2)?
            }
            EventKind::Close { depth } => {
                let (_, top) = open_frames.pop().ok_or_else(invalid_scene)?;
                push(
                    &mut frames,
                    Frame {
                        depth,
                        top,
                        bottom: next_y,
                    },
                )?;
                2
            }
            EventKind::EndCall { target } => {
                push(
                    &mut activations,
                    Activation {
                        participant: target,
                        row: next_y,
                        opening: false,
                    },
                )?;
                resources.checked_grid_add(height, 1)?
            }
            EventKind::Reference { .. } | EventKind::Divider { .. } => {
                resources.checked_grid_add(height, 3)?
            }
            EventKind::Section { .. } | EventKind::ParallelItem { .. } => {
                resources.checked_grid_add(height, 2)?
            }
        };
        next_y = resources.checked_grid_add(next_y, advance)?;
    }
    let height = resources.checked_grid_add(next_y, 1)?;
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(resources.checked_work_mul(extent.cells(), 2)?)?;
    Ok(Scene {
        title,
        participants,
        groups,
        events,
        frames,
        activations,
        width,
        height,
    })
}

fn caption(
    event: Event<'_>,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> Result<Label> {
    let key = format!("S{} ", event.key);
    match event.kind {
        EventKind::Arrow {
            statement, kind, ..
        } => {
            let (value, assignment) = match &statement.kind {
                ZenumlStatementKind::Message {
                    label, assignment, ..
                }
                | ZenumlStatementKind::Creation {
                    label, assignment, ..
                } => (label.as_str(), assignment.as_deref()),
                ZenumlStatementKind::Return { label, .. } => (label.as_str(), None),
                _ => return Err(invalid_scene()),
            };
            label(
                &[
                    &key,
                    kind.name(),
                    ": ",
                    assignment.unwrap_or(""),
                    if assignment.is_some() { " = " } else { "" },
                    value,
                ],
                CAPTION_WIDTH,
                options,
                resources,
            )
        }
        EventKind::AssignmentReturn { value, .. } => label(
            &[&key, "result: ", value],
            CAPTION_WIDTH,
            options,
            resources,
        ),
        EventKind::Open { statement, .. } => {
            let ZenumlStatementKind::Fragment {
                fragment_kind,
                label: value,
                ..
            } = &statement.kind
            else {
                return Err(invalid_scene());
            };
            label(
                &[
                    &key,
                    fragment_kind.as_str(),
                    " ",
                    value.as_deref().unwrap_or(""),
                ],
                CAPTION_WIDTH,
                options,
                resources,
            )
        }
        EventKind::Section { section, index, .. } => {
            let part = format!("branch {index}: ");
            label(
                &[
                    &key,
                    &part,
                    section.label.as_deref().unwrap_or("(unconditional)"),
                ],
                CAPTION_WIDTH,
                options,
                resources,
            )
        }
        EventKind::ParallelItem { index, .. } => {
            let part = format!("parallel item {index}");
            label(&[&key, &part], CAPTION_WIDTH, options, resources)
        }
        EventKind::Close { .. } => {
            let end = format!("end S{}", event.key);
            label(&[&end], CAPTION_WIDTH, options, resources)
        }
        EventKind::EndCall { .. } => {
            let end = format!("[end S{}]", event.key);
            label(&[&end], CAPTION_WIDTH, options, resources)
        }
        EventKind::Reference { statement, .. } => {
            let ZenumlStatementKind::Reference {
                label: value,
                participants,
            } = &statement.kind
            else {
                return Err(invalid_scene());
            };
            let selection = if participants.is_empty() {
                "context"
            } else {
                "selected"
            };
            label(
                &[&key, "ref (", selection, "): ", value],
                CAPTION_WIDTH,
                options,
                resources,
            )
        }
        EventKind::Divider { statement } => {
            let ZenumlStatementKind::Divider { label: value } = &statement.kind else {
                return Err(invalid_scene());
            };
            label(
                &[&key, value.trim().trim_matches('=').trim()],
                CAPTION_WIDTH,
                options,
                resources,
            )
        }
    }
}

fn label(
    parts: &[&str],
    wrap: usize,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> Result<Label> {
    let mut bytes = 0;
    for part in parts {
        bytes = resources.checked_work_add(bytes, part.len())?;
    }
    resources.check(AsciiResourceLimitId::MaxOutputBytes, bytes)?;
    resources.charge_layout_work(bytes.max(1))?;
    let mut text = String::new();
    text.try_reserve_exact(bytes)
        .map_err(|_| allocation_failed())?;
    for part in parts {
        text.push_str(part);
    }
    let Some(plan) = try_plan_normalized_label_lines(
        &text,
        options.terminal_width_profile,
        false,
        Some(wrap),
        resources,
    )?
    else {
        return Ok(Label {
            rows: Vec::new(),
            width: 0,
        });
    };
    let metrics = plan.metrics();
    resources.check(
        AsciiResourceLimitId::MaxOutputBytes,
        metrics.materialized_bytes,
    )?;
    resources.charge_document_cells(metrics.document_cells)?;
    let (rows, width) = plan.materialize(&text, resources)?.into_parts();
    Ok(Label { rows, width })
}

fn invalid_scene() -> crate::AsciiError {
    crate::AsciiError::UnsupportedFeature {
        diagram_type: "zenuml",
        feature: "invalid terminal event plan",
    }
}
