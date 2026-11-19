use super::layout::{Label, Scene};
use super::plan::{ArrowKind, EventKind};
use super::reserve;
use crate::canvas::Canvas;
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use crate::text::display_width_with_profile;
use crate::{AsciiCharset, AsciiColorRole, AsciiRenderOptions, Result};

struct Chars {
    h: char,
    v: char,
    tl: char,
    tr: char,
    bl: char,
    br: char,
    active: char,
    right: char,
    left: char,
}

impl Chars {
    fn new(options: &AsciiRenderOptions) -> Self {
        match options.structural_charset() {
            AsciiCharset::Ascii => Self {
                h: '-',
                v: '|',
                tl: '+',
                tr: '+',
                bl: '+',
                br: '+',
                active: '#',
                right: '>',
                left: '<',
            },
            AsciiCharset::Unicode => Self {
                h: '─',
                v: '│',
                tl: '┌',
                tr: '┐',
                bl: '└',
                br: '┘',
                active: '┃',
                right: '▶',
                left: '◀',
            },
        }
    }
}

pub(super) fn render(
    scene: &Scene<'_>,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let mut canvas = Canvas::try_with_resources_and_execution(
        scene.width,
        scene.height,
        options.terminal_width_profile,
        resources,
        execution,
    )?;
    let chars = Chars::new(options);
    for participant in &scene.participants {
        let start =
            resources.checked_grid_add(participant.top, participant.label.rows.len() + 2)?;
        if start < scene.height {
            vertical(
                &mut canvas,
                participant.center,
                start,
                scene.height - 1,
                chars.v,
                resources,
            )?;
        }
    }
    let mut activity = Vec::new();
    reserve(&mut activity, scene.participants.len())?;
    activity.resize(scene.participants.len(), (0usize, 0usize));
    for change in &scene.activations {
        resources.charge_layout_work(1)?;
        let (count, previous) = &mut activity[change.participant];
        if *count > 0 {
            vertical(
                &mut canvas,
                scene.participants[change.participant].center,
                *previous,
                change.row,
                chars.active,
                resources,
            )?;
        }
        if change.opening {
            *count = resources.checked_work_add(*count, 1)?;
        } else {
            *count = count.saturating_sub(1);
        }
        *previous = change.row;
    }
    for frame in &scene.frames {
        let inset = resources.checked_grid_mul(frame.depth, 2)?;
        rectangle(
            &mut canvas,
            inset,
            scene.width - inset - 1,
            frame.top,
            frame.bottom,
            &chars,
            resources,
        )?;
    }
    if let Some(title) = &scene.title {
        paint_label(&mut canvas, 0, 0, title, options, resources)?;
    }
    for group in &scene.groups {
        rectangle(
            &mut canvas,
            group.left,
            group.right,
            group.top,
            group.top + group.label.rows.len() + 1,
            &chars,
            resources,
        )?;
        paint_label(
            &mut canvas,
            group.left + 1,
            group.top + 1,
            &group.label,
            options,
            resources,
        )?;
    }
    for participant in &scene.participants {
        let left = participant.center - participant.half;
        let right = participant.center + participant.half;
        let bottom = participant.top + participant.label.rows.len() + 1;
        rectangle(
            &mut canvas,
            left,
            right,
            participant.top,
            bottom,
            &chars,
            resources,
        )?;
        paint_label(
            &mut canvas,
            left + 1,
            participant.top + 1,
            &participant.label,
            options,
            resources,
        )?;
    }
    for placed in &scene.events {
        resources.charge_layout_work(1)?;
        match placed.event.kind {
            EventKind::Arrow { from, to, kind, .. } => {
                let a = scene.participants[from].center;
                let b = scene.participants[to].center;
                let rightward = b >= a;
                let tip = if matches!(kind, ArrowKind::Create) {
                    if rightward {
                        b - scene.participants[to].half
                    } else {
                        b + scene.participants[to].half
                    }
                } else {
                    b
                };
                let x = a.min(tip) + 2;
                paint_label(
                    &mut canvas,
                    x,
                    placed.top,
                    &placed.label,
                    options,
                    resources,
                )?;
                arrow(
                    &mut canvas,
                    a,
                    tip,
                    placed.arrow_y,
                    if a == b { placed.label.width + 4 } else { 0 },
                    kind,
                    &chars,
                    resources,
                )?;
            }
            EventKind::AssignmentReturn { from, to, .. } => {
                let a = scene.participants[from].center;
                let b = scene.participants[to].center;
                paint_label(
                    &mut canvas,
                    a.min(b) + 2,
                    placed.top,
                    &placed.label,
                    options,
                    resources,
                )?;
                arrow(
                    &mut canvas,
                    a,
                    b,
                    placed.arrow_y,
                    0,
                    ArrowKind::Return,
                    &chars,
                    resources,
                )?;
            }
            EventKind::EndCall { target } => {
                paint_label(
                    &mut canvas,
                    scene.participants[target].center + 2,
                    placed.top,
                    &placed.label,
                    options,
                    resources,
                )?;
            }
            EventKind::Open { depth, .. } | EventKind::Close { depth } => {
                paint_label(
                    &mut canvas,
                    depth * 2 + 2,
                    placed.top,
                    &placed.label,
                    options,
                    resources,
                )?;
            }
            EventKind::Section { depth, .. } | EventKind::ParallelItem { depth, .. } => {
                let inset = depth.saturating_sub(1) * 2;
                horizontal(
                    &mut canvas,
                    inset + 1,
                    scene.width - inset - 2,
                    placed.top,
                    '-',
                    resources,
                )?;
                paint_label(
                    &mut canvas,
                    depth * 2 + 2,
                    placed.top + 1,
                    &placed.label,
                    options,
                    resources,
                )?;
            }
            EventKind::Reference { left, right, .. } => {
                let left = scene.participants[left].center - scene.participants[left].half - 1;
                let right = scene.participants[right].center + scene.participants[right].half + 1;
                rectangle(
                    &mut canvas,
                    left,
                    right,
                    placed.top,
                    placed.top + placed.label.rows.len() + 1,
                    &chars,
                    resources,
                )?;
                paint_label(
                    &mut canvas,
                    left + 1,
                    placed.top + 1,
                    &placed.label,
                    options,
                    resources,
                )?;
            }
            EventKind::Divider { .. } => {
                horizontal(&mut canvas, 0, scene.width - 1, placed.top, '=', resources)?;
                paint_label(
                    &mut canvas,
                    2,
                    placed.top + 1,
                    &placed.label,
                    options,
                    resources,
                )?;
                horizontal(
                    &mut canvas,
                    0,
                    scene.width - 1,
                    placed.top + placed.label.rows.len() + 1,
                    '=',
                    resources,
                )?;
            }
        }
    }
    canvas.finish_with_options_with_execution(options, execution)
}

#[expect(
    clippy::too_many_arguments,
    reason = "One planned arrow has explicit terminal coordinates and style"
)]
fn arrow(
    canvas: &mut Canvas,
    from: usize,
    to: usize,
    y: usize,
    self_width: usize,
    kind: ArrowKind,
    chars: &Chars,
    resources: &ResourceContext,
) -> Result<()> {
    let stroke = if matches!(kind, ArrowKind::Return) {
        '.'
    } else {
        chars.h
    };
    if from == to {
        let right = resources.checked_grid_add(from, self_width)?;
        horizontal(canvas, from + 1, right, y, stroke, resources)?;
        vertical(canvas, right, y + 1, y + 2, chars.v, resources)?;
        horizontal(canvas, from + 1, right, y + 2, stroke, resources)?;
        canvas.try_set(right, y, chars.tr)?;
        canvas.try_set(right, y + 2, chars.br)?;
        canvas.try_set(
            from,
            y + 2,
            if matches!(kind, ArrowKind::Async) {
                '<'
            } else {
                chars.left
            },
        )?;
    } else {
        let left = from.min(to);
        let right = from.max(to);
        horizontal(canvas, left + 1, right - 1, y, stroke, resources)?;
        canvas.try_set(
            to,
            y,
            if matches!(kind, ArrowKind::Async) {
                if to > from { '>' } else { '<' }
            } else if to > from {
                chars.right
            } else {
                chars.left
            },
        )?;
    }
    Ok(())
}

fn paint_label(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    label: &Label,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> Result<()> {
    for (index, row) in label.rows.iter().enumerate() {
        resources.charge_layout_work(row.len().max(1))?;
        let end = resources.checked_grid_add(
            x,
            display_width_with_profile(row, options.terminal_width_profile),
        )?;
        if end > x {
            horizontal(canvas, x, end - 1, y + index, ' ', resources)?;
        }
        canvas.write_text_role(x, y + index, row, AsciiColorRole::Text)?;
    }
    Ok(())
}

fn rectangle(
    canvas: &mut Canvas,
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
    chars: &Chars,
    resources: &ResourceContext,
) -> Result<()> {
    horizontal(canvas, left, right, top, chars.h, resources)?;
    horizontal(canvas, left, right, bottom, chars.h, resources)?;
    vertical(canvas, left, top, bottom, chars.v, resources)?;
    vertical(canvas, right, top, bottom, chars.v, resources)?;
    canvas.try_set(left, top, chars.tl)?;
    canvas.try_set(right, top, chars.tr)?;
    canvas.try_set(left, bottom, chars.bl)?;
    canvas.try_set(right, bottom, chars.br)?;
    Ok(())
}

fn horizontal(
    canvas: &mut Canvas,
    left: usize,
    right: usize,
    y: usize,
    glyph: char,
    resources: &ResourceContext,
) -> Result<()> {
    if left > right {
        return Ok(());
    }
    resources.charge_layout_work(right - left + 1)?;
    for x in left..=right {
        if x % 64 == 0 {
            resources.checkpoint()?;
        }
        canvas.try_set(x, y, glyph)?;
    }
    Ok(())
}

fn vertical(
    canvas: &mut Canvas,
    x: usize,
    top: usize,
    bottom: usize,
    glyph: char,
    resources: &ResourceContext,
) -> Result<()> {
    if top > bottom {
        return Ok(());
    }
    resources.charge_layout_work(bottom - top + 1)?;
    for y in top..=bottom {
        if y % 64 == 0 {
            resources.checkpoint()?;
        }
        canvas.try_set(x, y, glyph)?;
    }
    Ok(())
}
