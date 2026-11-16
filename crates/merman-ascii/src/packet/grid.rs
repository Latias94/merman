//! Fixed bit coordinates. Authored labels never participate in field geometry.

use super::{PacketSettings, ResourceContext};
use crate::operation::AsciiExecution;
use crate::safe_text::{
    BudgetedTextDocument, BudgetedTextLine, terminal_single_line_text_requires_normalization,
};
use crate::text::display_width_with_profile;
use crate::{AsciiCharset, AsciiError, AsciiRenderOptions, AsciiResourceLimitId, Result};
use merman_model::OperationPhase;
use merman_model::families::packet::{PacketBlock, PacketDiagramModel};

const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
const CELLS_PER_BIT: usize = 2;

/// Numeric DB values outside this domain remain exact records, never allocation counts.
pub(super) fn last_bit(
    model: &PacketDiagramModel,
    settings: PacketSettings,
    resources: &ResourceContext,
) -> Result<Option<u64>> {
    resources.checkpoint()?;
    if settings.bits_per_row.is_none() {
        return Ok(None);
    }
    let mut last = None;
    for row in &model.packet {
        resources.charge_layout_work(1)?;
        for block in row {
            resources.charge_layout_work(1)?;
            if !coordinate(block.start)
                || !coordinate(block.end)
                || block.end < block.start
                || !block.bits.is_finite()
                || block.bits < 0.0
                || block.bits.fract() != 0.0
            {
                return Ok(None);
            }
            let start = block.start as u64;
            if last.is_some_and(|last| start <= last) {
                return Ok(None);
            }
            last = Some(block.end as u64);
        }
    }
    Ok(last)
}

fn coordinate(value: f64) -> bool {
    (0.0..=MAX_SAFE_INTEGER).contains(&value) && value.fract() == 0.0
}

struct Span<'a> {
    start: u64,
    end: u64,
    block: Option<&'a PacketBlock>,
}

impl Span<'_> {
    fn interior_width(&self) -> usize {
        (self.end - self.start + 1) as usize * CELLS_PER_BIT - 1
    }
}

pub(super) fn write_grid(
    model: &PacketDiagramModel,
    last_bit: u64,
    settings: PacketSettings,
    options: &AsciiRenderOptions,
    document: &mut BudgetedTextDocument,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    let bits_per_row = settings.bits_per_row.expect("admitted spatial settings");
    let resources = document.resources_mut();
    let rows = usize::try_from(last_bit / bits_per_row as u64 + 1)
        .map_err(|_| resources.overflow(AsciiResourceLimitId::MaxGridCells))?;
    let width = bits_per_row * CELLS_PER_BIT + 1;
    let height = resources.checked_grid_mul(rows, 4 + usize::from(settings.show_bits))?;
    resources.grid_extent(width, height)?;
    resources.charge_layout_work_product(rows, bits_per_row)?;

    let mut plan: Vec<Vec<Span<'_>>> = Vec::new();
    plan.try_reserve_exact(rows)
        .map_err(|_| AsciiError::allocation_failed("ascii_packet_grid"))?;
    let mut blocks = model.packet.iter().flatten().peekable();
    for row in 0..rows {
        execution.checkpoint_loop(OperationPhase::Layout, row)?;
        let mut spans = Vec::new();
        let mut cursor = row as u64 * bits_per_row as u64;
        let row_end = cursor + bits_per_row as u64;
        while cursor < row_end {
            let (end, block) = match blocks.peek() {
                Some(block) if cursor >= block.start as u64 => {
                    ((block.end as u64 + 1).min(row_end), Some(*block))
                }
                next => (
                    next.map_or(row_end, |block| block.start as u64)
                        .min(row_end),
                    None,
                ),
            };
            spans
                .try_reserve(1)
                .map_err(|_| AsciiError::allocation_failed("ascii_packet_grid"))?;
            spans.push(Span {
                start: cursor,
                end: end - 1,
                block,
            });
            if block.is_some_and(|block| end > block.end as u64) {
                blocks.next();
            }
            cursor = end;
        }
        if settings.descending {
            spans.reverse();
        }
        plan.push(spans);
    }

    execution.rebind_resource_context(document.resources_mut(), OperationPhase::Emit);
    let text_width = execution.max_width().unwrap_or(80);
    document.push_wrapped_prefixed_line_with("", "", text_width, |line| {
        line.write_fmt(format_args!(
            "Packet: {bits_per_row} bits/row, {CELLS_PER_BIT} cells/bit."
        ))
    })?;
    let borders = Borders::for_charset(options.structural_charset());
    let mut has_marker = false;
    let mut has_gap = false;
    for (index, row) in plan.iter().enumerate() {
        execution.checkpoint_loop(OperationPhase::Emit, index)?;
        document.push_line_with(|line| line.write_fmt(format_args!("word {}:", index + 1)))?;
        if settings.show_bits {
            document.push_line_with(|line| {
                line.push_str(" ")?;
                for span in row {
                    let annotation = match span.block {
                        Some(_) if span.start == span.end => span.start.to_string(),
                        Some(_) if settings.descending => format!("{}..{}", span.end, span.start),
                        Some(_) => format!("{}..{}", span.start, span.end),
                        None => String::new(),
                    };
                    let annotation = if annotation.len() <= span.interior_width() {
                        annotation.as_str()
                    } else {
                        has_marker = true;
                        "*"
                    };
                    centered(line, annotation, annotation.len(), span.interior_width())?;
                    line.push_str(" ")?;
                }
                Ok(())
            })?;
        }
        border(document, row, borders.top, borders.horizontal)?;
        let mut labels = Vec::new();
        labels
            .try_reserve_exact(row.len())
            .map_err(|_| AsciiError::allocation_failed("ascii_packet_labels"))?;
        for span in row {
            let label = match span.block {
                Some(block) => inline_label(
                    &block.label,
                    span.interior_width(),
                    options,
                    document.resources_mut(),
                )?,
                None => {
                    has_gap = true;
                    "."
                }
            };
            has_marker |= span.block.is_some() && label == "*";
            labels.push((
                label,
                display_width_with_profile(label, options.terminal_width_profile),
            ));
        }
        document.push_line_with(|line| {
            line.push_str(borders.vertical)?;
            for (span, &(label, label_width)) in row.iter().zip(&labels) {
                if span.block.is_none() {
                    repeat(line, ".", span.interior_width())?;
                } else {
                    centered(line, label, label_width, span.interior_width())?;
                }
                line.push_str(borders.vertical)?;
            }
            Ok(())
        })?;
        border(document, row, borders.bottom, borders.horizontal)?;
    }
    if has_marker || has_gap {
        document.push_wrapped_prefixed_line_with("", "", text_width, |line| {
            if has_marker {
                line.push_str("* label/range in field records.")?;
            }
            if has_gap {
                if has_marker {
                    line.push_str(" ")?;
                }
                line.push_str(". unassigned bits.")?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn inline_label<'a>(
    label: &'a str,
    available: usize,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> Result<&'a str> {
    if terminal_single_line_text_requires_normalization(label, resources)?
        || label.contains(['|', '│', '┌', '┬', '┐', '└', '┴', '┘', '├', '┼', '┤'])
        || display_width_with_profile(label, options.terminal_width_profile) > available
    {
        Ok("*")
    } else {
        Ok(label)
    }
}

fn centered(
    line: &mut BudgetedTextLine<'_>,
    text: &str,
    width: usize,
    available: usize,
) -> Result<()> {
    let padding = available - width;
    repeat(line, " ", padding / 2)?;
    line.push_str(text)?;
    repeat(line, " ", padding - padding / 2)
}

fn repeat(line: &mut BudgetedTextLine<'_>, symbol: &str, count: usize) -> Result<()> {
    for _ in 0..count {
        line.push_str(symbol)?;
    }
    Ok(())
}

fn border(
    document: &mut BudgetedTextDocument,
    row: &[Span<'_>],
    (left, joint, right): (&str, &str, &str),
    horizontal: &str,
) -> Result<()> {
    document.push_line_with(|line| {
        line.push_str(left)?;
        for (index, span) in row.iter().enumerate() {
            repeat(line, horizontal, span.interior_width())?;
            line.push_str(if index + 1 == row.len() { right } else { joint })?;
        }
        Ok(())
    })
}

struct Borders {
    top: (&'static str, &'static str, &'static str),
    bottom: (&'static str, &'static str, &'static str),
    horizontal: &'static str,
    vertical: &'static str,
}

impl Borders {
    fn for_charset(charset: AsciiCharset) -> Self {
        match charset {
            AsciiCharset::Ascii => Self {
                top: ("+", "+", "+"),
                bottom: ("+", "+", "+"),
                horizontal: "-",
                vertical: "|",
            },
            AsciiCharset::Unicode => Self {
                top: ("┌", "┬", "┐"),
                bottom: ("└", "┴", "┘"),
                horizontal: "─",
                vertical: "│",
            },
        }
    }
}
