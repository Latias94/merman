use super::composed::{ComposedTextPlan, DeferredTextPiece};
use super::framing::{QuotedTerminalTextEvent, visit_quoted_terminal_text_with};
use super::label::{DeferredLabelPiece, try_plan_normalized_label_lines};
use crate::Result;
use crate::error::AsciiError;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::terminal::DeferredTextId;
use crate::text::display_width_with_profile;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeferredTextGlyph {
    id: DeferredTextId,
    width: usize,
}

impl DeferredTextGlyph {
    pub(crate) const fn id(self) -> DeferredTextId {
        self.id
    }

    pub(crate) const fn width(self) -> usize {
        self.width
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeferredTextLine {
    glyphs: Vec<DeferredTextGlyph>,
    width: usize,
    plain_bytes: usize,
    html_bytes: usize,
}

impl DeferredTextLine {
    pub(crate) fn glyphs(&self) -> &[DeferredTextGlyph] {
        &self.glyphs
    }

    pub(crate) const fn width(&self) -> usize {
        self.width
    }

    pub(crate) const fn plain_bytes(&self) -> usize {
        self.plain_bytes
    }

    pub(crate) fn try_concat(lines: &[&Self], resources: &ResourceContext) -> Result<Self> {
        resources.transaction(|resources| {
            let (glyph_count, width, plain_bytes, html_bytes) = lines.iter().copied().try_fold(
                (0usize, 0usize, 0usize, 0usize),
                |(glyph_count, width, plain_bytes, html_bytes), line| {
                    Ok::<_, AsciiError>((
                        glyph_count
                            .checked_add(line.glyphs.len())
                            .ok_or_else(layout_allocation_failed)?,
                        resources.checked_grid_add(width, line.width)?,
                        plain_bytes
                            .checked_add(line.plain_bytes)
                            .ok_or_else(|| output_overflow(resources))?,
                        html_bytes
                            .checked_add(line.html_bytes)
                            .ok_or_else(|| output_overflow(resources))?,
                    ))
                },
            )?;
            resources.check_usage(glyph_count.max(1), 0)?;

            let mut glyphs = Vec::new();
            glyphs
                .try_reserve_exact(glyph_count)
                .map_err(|_| layout_allocation_failed())?;
            for line in lines.iter().copied() {
                glyphs.extend_from_slice(&line.glyphs);
            }
            resources.charge_layout_work(glyph_count.max(1))?;
            Ok(Self {
                glyphs,
                width,
                plain_bytes,
                html_bytes,
            })
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum DeferredTextPart<'a> {
    Static(&'static str),
    Decimal(usize),
    QuotedLine(&'a DeferredTextLine),
}

#[derive(Debug, Clone, Copy)]
enum DeferredTextEntry<'a> {
    Composed {
        plan_index: usize,
        piece: DeferredTextPiece,
        width_profile: TerminalWidthProfile,
    },
    Label {
        piece: DeferredLabelPiece<'a>,
    },
    Static {
        text: &'static str,
        replay_work_units: usize,
        plain_bytes: usize,
        html_bytes: usize,
    },
    Decimal {
        value: usize,
        replay_work_units: usize,
        plain_bytes: usize,
    },
    QuotedBorrowed {
        text: &'a str,
        replay_work_units: usize,
        plain_bytes: usize,
        html_bytes: usize,
    },
    QuotedLine {
        line_index: usize,
        replay_work_units: usize,
        plain_bytes: usize,
        html_bytes: usize,
    },
}

#[derive(Debug, Default)]
pub(crate) struct DeferredTextRegistry<'a> {
    plans: Vec<ComposedTextPlan<'a>>,
    entries: Vec<DeferredTextEntry<'a>>,
    quoted_lines: Vec<Vec<DeferredTextId>>,
}

impl<'a> DeferredTextRegistry<'a> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn try_register(
        &mut self,
        plan: ComposedTextPlan<'a>,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<DeferredTextLine> {
        resources.transaction(|resources| {
            self.try_register_transactional(plan, width_profile, resources)
        })
    }

    fn try_register_transactional(
        &mut self,
        plan: ComposedTextPlan<'a>,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<DeferredTextLine> {
        let (pieces, metrics) = plan.try_deferred_pieces(width_profile, resources)?;
        let (plain_bytes, html_bytes) =
            pieces
                .iter()
                .try_fold((0usize, 0usize), |(plain_bytes, html_bytes), piece| {
                    Ok::<_, AsciiError>((
                        plain_bytes
                            .checked_add(piece.encoded_bytes(false))
                            .ok_or_else(|| {
                                resources
                                    .overflow(crate::resource::AsciiResourceLimitId::MaxOutputBytes)
                            })?,
                        html_bytes
                            .checked_add(piece.encoded_bytes(true))
                            .ok_or_else(|| {
                                resources
                                    .overflow(crate::resource::AsciiResourceLimitId::MaxOutputBytes)
                            })?,
                    ))
                })?;
        let plan_index = self.plans.len();
        let final_plan_count = plan_index
            .checked_add(1)
            .ok_or_else(layout_allocation_failed)?;
        let final_entry_count = self
            .entries
            .len()
            .checked_add(pieces.len())
            .ok_or_else(layout_allocation_failed)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(pieces.len())
            .map_err(|_| layout_allocation_failed())?;
        let mut glyphs = Vec::new();
        glyphs
            .try_reserve_exact(pieces.len())
            .map_err(|_| layout_allocation_failed())?;
        for piece in pieces {
            let entry_index = self
                .entries
                .len()
                .checked_add(entries.len())
                .ok_or_else(layout_allocation_failed)?;
            let id = DeferredTextId::try_from_index(entry_index)?;
            glyphs.push(DeferredTextGlyph {
                id,
                width: piece.display_width(),
            });
            entries.push(DeferredTextEntry::Composed {
                plan_index,
                piece,
                width_profile,
            });
        }
        self.plans
            .try_reserve(final_plan_count - self.plans.len())
            .map_err(|_| layout_allocation_failed())?;
        self.entries
            .try_reserve(final_entry_count - self.entries.len())
            .map_err(|_| layout_allocation_failed())?;
        self.entries.extend(entries);
        self.plans.push(plan);
        Ok(DeferredTextLine {
            glyphs,
            width: metrics.display_width(),
            plain_bytes,
            html_bytes,
        })
    }

    pub(crate) fn try_register_label_lines(
        &mut self,
        raw: &'a str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Option<Vec<DeferredTextLine>>> {
        resources.transaction(|resources| {
            self.try_register_label_lines_transactional(raw, width_profile, resources)
        })
    }

    fn try_register_label_lines_transactional(
        &mut self,
        raw: &'a str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Option<Vec<DeferredTextLine>>> {
        let Some(plan) =
            try_plan_normalized_label_lines(raw, width_profile, true, None, resources)?
        else {
            return Ok(None);
        };
        let rows = plan.try_deferred_rows(raw, resources)?;
        let entry_count = rows.iter().try_fold(0usize, |count, row| {
            count
                .checked_add(row.pieces().len())
                .ok_or_else(layout_allocation_failed)
        })?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(entry_count)
            .map_err(|_| layout_allocation_failed())?;
        let mut lines = Vec::new();
        lines
            .try_reserve_exact(rows.len())
            .map_err(|_| layout_allocation_failed())?;
        for row in rows {
            let (plain_bytes, html_bytes) = row.pieces().iter().try_fold(
                (0usize, 0usize),
                |(plain_bytes, html_bytes), piece| {
                    Ok::<_, AsciiError>((
                        plain_bytes
                            .checked_add(piece.encoded_bytes(false))
                            .ok_or_else(|| {
                                resources
                                    .overflow(crate::resource::AsciiResourceLimitId::MaxOutputBytes)
                            })?,
                        html_bytes
                            .checked_add(piece.encoded_bytes(true))
                            .ok_or_else(|| {
                                resources
                                    .overflow(crate::resource::AsciiResourceLimitId::MaxOutputBytes)
                            })?,
                    ))
                },
            )?;
            let mut glyphs = Vec::new();
            glyphs
                .try_reserve_exact(row.pieces().len())
                .map_err(|_| layout_allocation_failed())?;
            for &piece in row.pieces() {
                let entry_index = self
                    .entries
                    .len()
                    .checked_add(entries.len())
                    .ok_or_else(layout_allocation_failed)?;
                glyphs.push(DeferredTextGlyph {
                    id: DeferredTextId::try_from_index(entry_index)?,
                    width: piece.display_width(),
                });
                entries.push(DeferredTextEntry::Label { piece });
            }
            lines.push(DeferredTextLine {
                glyphs,
                width: row.width(),
                plain_bytes,
                html_bytes,
            });
        }
        self.entries
            .try_reserve(entry_count)
            .map_err(|_| layout_allocation_failed())?;
        self.entries.extend(entries);
        Ok(Some(lines))
    }

    pub(crate) fn try_register_quoted_text(
        &mut self,
        text: &'a str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<DeferredTextLine> {
        resources.transaction(|resources| {
            let metrics = quoted_text_metrics(text, width_profile, resources)?;
            resources.check_usage(metrics.planning_work_units, 0)?;
            let entry_index = self.entries.len();
            self.entries
                .try_reserve(1)
                .map_err(|_| layout_allocation_failed())?;
            let mut glyphs = Vec::new();
            glyphs
                .try_reserve_exact(1)
                .map_err(|_| layout_allocation_failed())?;
            glyphs.push(DeferredTextGlyph {
                id: DeferredTextId::try_from_index(entry_index)?,
                width: metrics.width,
            });
            resources.charge_layout_work(metrics.planning_work_units)?;
            self.entries.push(DeferredTextEntry::QuotedBorrowed {
                text,
                replay_work_units: metrics.replay_work_units,
                plain_bytes: metrics.plain_bytes,
                html_bytes: metrics.html_bytes,
            });
            Ok(DeferredTextLine {
                glyphs,
                width: metrics.width,
                plain_bytes: metrics.plain_bytes,
                html_bytes: metrics.html_bytes,
            })
        })
    }

    pub(crate) fn try_register_parts<'part>(
        &mut self,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
        producer_work_per_pass: usize,
        produce: impl Fn(&mut dyn FnMut(DeferredTextPart<'part>) -> Result<()>) -> Result<()>,
    ) -> Result<DeferredTextLine> {
        resources.transaction(|resources| {
            let mut part_count = 0usize;
            let mut total_work = 0usize;
            let mut total_width = 0usize;
            let mut total_plain_bytes = 0usize;
            let mut total_html_bytes = 0usize;
            let mut quoted_line_count = 0usize;
            produce(&mut |part| {
                let metrics = self.deferred_part_metrics(part, width_profile, resources)?;
                part_count = part_count
                    .checked_add(1)
                    .ok_or_else(layout_allocation_failed)?;
                total_work = resources.checked_work_add(total_work, metrics.planning_work_units)?;
                total_width = resources.checked_grid_add(total_width, metrics.width)?;
                total_plain_bytes = total_plain_bytes
                    .checked_add(metrics.plain_bytes)
                    .ok_or_else(|| {
                        resources.overflow(crate::resource::AsciiResourceLimitId::MaxOutputBytes)
                    })?;
                total_html_bytes = total_html_bytes
                    .checked_add(metrics.html_bytes)
                    .ok_or_else(|| {
                        resources.overflow(crate::resource::AsciiResourceLimitId::MaxOutputBytes)
                    })?;
                if matches!(part, DeferredTextPart::QuotedLine(_)) {
                    quoted_line_count = quoted_line_count
                        .checked_add(1)
                        .ok_or_else(layout_allocation_failed)?;
                }
                Ok(())
            })?;
            total_work = resources.checked_work_add(
                resources.checked_work_mul(total_work, 2)?,
                resources.checked_work_mul(producer_work_per_pass.max(1), 2)?,
            )?;
            resources.check_usage(total_work, 0)?;

            let final_entry_count = self
                .entries
                .len()
                .checked_add(part_count)
                .ok_or_else(layout_allocation_failed)?;
            let final_quoted_line_count = self
                .quoted_lines
                .len()
                .checked_add(quoted_line_count)
                .ok_or_else(layout_allocation_failed)?;
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(part_count)
                .map_err(|_| layout_allocation_failed())?;
            let mut glyphs = Vec::new();
            glyphs
                .try_reserve_exact(part_count)
                .map_err(|_| layout_allocation_failed())?;
            let mut quoted_lines = Vec::new();
            quoted_lines
                .try_reserve_exact(quoted_line_count)
                .map_err(|_| layout_allocation_failed())?;

            produce(&mut |part| {
                let metrics = self.deferred_part_metrics(part, width_profile, resources)?;
                let entry_index = self
                    .entries
                    .len()
                    .checked_add(entries.len())
                    .ok_or_else(layout_allocation_failed)?;
                glyphs.push(DeferredTextGlyph {
                    id: DeferredTextId::try_from_index(entry_index)?,
                    width: metrics.width,
                });
                entries.push(match part {
                    DeferredTextPart::Static(text) => DeferredTextEntry::Static {
                        text,
                        replay_work_units: metrics.replay_work_units,
                        plain_bytes: metrics.plain_bytes,
                        html_bytes: metrics.html_bytes,
                    },
                    DeferredTextPart::Decimal(value) => DeferredTextEntry::Decimal {
                        value,
                        replay_work_units: metrics.replay_work_units,
                        plain_bytes: metrics.plain_bytes,
                    },
                    DeferredTextPart::QuotedLine(line) => {
                        let line_index = self
                            .quoted_lines
                            .len()
                            .checked_add(quoted_lines.len())
                            .ok_or_else(layout_allocation_failed)?;
                        let mut ids = Vec::new();
                        ids.try_reserve_exact(line.glyphs.len())
                            .map_err(|_| layout_allocation_failed())?;
                        ids.extend(line.glyphs.iter().map(|glyph| glyph.id));
                        quoted_lines.push(ids);
                        DeferredTextEntry::QuotedLine {
                            line_index,
                            replay_work_units: metrics.replay_work_units,
                            plain_bytes: metrics.plain_bytes,
                            html_bytes: metrics.html_bytes,
                        }
                    }
                });
                Ok(())
            })?;
            if entries.len() != part_count || quoted_lines.len() != quoted_line_count {
                return Err(layout_allocation_failed());
            }

            self.entries
                .try_reserve(final_entry_count - self.entries.len())
                .map_err(|_| layout_allocation_failed())?;
            self.quoted_lines
                .try_reserve(final_quoted_line_count - self.quoted_lines.len())
                .map_err(|_| layout_allocation_failed())?;
            resources.charge_usage(total_work, 0)?;
            self.entries.extend(entries);
            self.quoted_lines.extend(quoted_lines);
            Ok(DeferredTextLine {
                glyphs,
                width: total_width,
                plain_bytes: total_plain_bytes,
                html_bytes: total_html_bytes,
            })
        })
    }

    pub(crate) fn replay_work_units(&self, id: DeferredTextId) -> Result<usize> {
        self.entries
            .get(id.index())
            .map(|entry| match entry {
                DeferredTextEntry::Composed { piece, .. } => piece.replay_work_units(),
                DeferredTextEntry::Label { piece } => piece.replay_work_units(),
                DeferredTextEntry::Static {
                    replay_work_units, ..
                }
                | DeferredTextEntry::Decimal {
                    replay_work_units, ..
                }
                | DeferredTextEntry::QuotedBorrowed {
                    replay_work_units, ..
                }
                | DeferredTextEntry::QuotedLine {
                    replay_work_units, ..
                } => *replay_work_units,
            })
            .ok_or_else(invalid_deferred_text_id)
    }

    pub(crate) fn encoded_bytes(&self, id: DeferredTextId, html: bool) -> Result<usize> {
        self.entries
            .get(id.index())
            .map(|entry| match entry {
                DeferredTextEntry::Composed { piece, .. } => piece.encoded_bytes(html),
                DeferredTextEntry::Label { piece } => piece.encoded_bytes(html),
                DeferredTextEntry::Static {
                    plain_bytes,
                    html_bytes,
                    ..
                }
                | DeferredTextEntry::QuotedBorrowed {
                    plain_bytes,
                    html_bytes,
                    ..
                }
                | DeferredTextEntry::QuotedLine {
                    plain_bytes,
                    html_bytes,
                    ..
                } => {
                    if html {
                        *html_bytes
                    } else {
                        *plain_bytes
                    }
                }
                DeferredTextEntry::Decimal { plain_bytes, .. } => *plain_bytes,
            })
            .ok_or_else(invalid_deferred_text_id)
    }

    pub(crate) fn try_visit(
        &self,
        id: DeferredTextId,
        visit: &mut dyn FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        let entry = self
            .entries
            .get(id.index())
            .copied()
            .ok_or_else(invalid_deferred_text_id)?;
        match entry {
            DeferredTextEntry::Composed {
                plan_index,
                piece,
                width_profile,
            } => self
                .plans
                .get(plan_index)
                .ok_or_else(invalid_deferred_text_id)?
                .try_visit_deferred_piece(width_profile, piece, visit),
            DeferredTextEntry::Label { piece } => piece.try_visit(visit),
            DeferredTextEntry::Static { text, .. } => visit(text),
            DeferredTextEntry::Decimal { value, .. } => visit_decimal(value, visit),
            DeferredTextEntry::QuotedBorrowed { text, .. } => visit_quoted_output(text, visit),
            DeferredTextEntry::QuotedLine { line_index, .. } => {
                let ids = self
                    .quoted_lines
                    .get(line_index)
                    .ok_or_else(invalid_deferred_text_id)?;
                self.try_visit_quoted_line(ids, visit)
            }
        }
    }

    fn quoted_line_metrics(
        &self,
        line: &DeferredTextLine,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<DeferredPartMetrics> {
        let mut width = 0usize;
        let mut source_work = 0usize;
        let mut fragment_count = 0usize;
        let mut plain_bytes = 0usize;
        let mut html_bytes = 0usize;
        for glyph in &line.glyphs {
            source_work =
                resources.checked_work_add(source_work, self.replay_work_units(glyph.id)?)?;
        }
        self.try_visit_quoted_glyphs(line.glyphs.iter().map(|glyph| glyph.id), &mut |fragment| {
            fragment_count = resources.checked_work_add(fragment_count, 1)?;
            plain_bytes = plain_bytes.checked_add(fragment.len()).ok_or_else(|| {
                resources.overflow(crate::resource::AsciiResourceLimitId::MaxOutputBytes)
            })?;
            html_bytes = html_bytes
                .checked_add(encoded_html_bytes(resources, fragment)?)
                .ok_or_else(|| {
                    resources.overflow(crate::resource::AsciiResourceLimitId::MaxOutputBytes)
                })?;
            width = resources
                .checked_grid_add(width, display_width_with_profile(fragment, width_profile))?;
            Ok(())
        })?;
        let replay_work_units = resources.checked_work_add(
            source_work,
            resources.checked_work_add(plain_bytes.max(1), fragment_count.max(1))?,
        )?;
        Ok(DeferredPartMetrics {
            width,
            planning_work_units: replay_work_units,
            replay_work_units,
            plain_bytes,
            html_bytes,
        })
    }

    fn try_visit_quoted_line(
        &self,
        ids: &[DeferredTextId],
        visit: &mut dyn FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        self.try_visit_quoted_glyphs(ids.iter().copied(), visit)
    }

    fn try_visit_quoted_glyphs(
        &self,
        ids: impl IntoIterator<Item = DeferredTextId>,
        visit: &mut dyn FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        visit("\"")?;
        for id in ids {
            self.try_visit(id, &mut |fragment| visit_quoted_body(fragment, visit))?;
        }
        visit("\"")
    }

    fn deferred_part_metrics(
        &self,
        part: DeferredTextPart<'_>,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<DeferredPartMetrics> {
        match part {
            DeferredTextPart::Static(text) => text_metrics(text, width_profile, resources),
            DeferredTextPart::Decimal(value) => {
                let bytes = decimal_len(value);
                Ok(DeferredPartMetrics {
                    width: bytes,
                    planning_work_units: bytes.max(1),
                    replay_work_units: bytes.max(1),
                    plain_bytes: bytes,
                    html_bytes: bytes,
                })
            }
            DeferredTextPart::QuotedLine(line) => {
                self.quoted_line_metrics(line, width_profile, resources)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DeferredPartMetrics {
    width: usize,
    planning_work_units: usize,
    replay_work_units: usize,
    plain_bytes: usize,
    html_bytes: usize,
}

fn text_metrics(
    text: &str,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<DeferredPartMetrics> {
    let bytes = text.len();
    Ok(DeferredPartMetrics {
        width: display_width_with_profile(text, width_profile),
        planning_work_units: bytes.max(1),
        replay_work_units: bytes.max(1),
        plain_bytes: bytes,
        html_bytes: encoded_html_bytes(resources, text)?,
    })
}

fn quoted_text_metrics(
    text: &str,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<DeferredPartMetrics> {
    let mut width = 0usize;
    let mut source_work = 0usize;
    let mut fragment_count = 0usize;
    let mut plain_bytes = 0usize;
    let mut html_bytes = 0usize;
    visit_quoted_terminal_text_with(text, |event| match event {
        QuotedTerminalTextEvent::SourceGrapheme(grapheme) => {
            resources.check_grapheme_bytes(grapheme.len())?;
            source_work = resources.checked_work_add(source_work, 1)?;
            Ok(())
        }
        QuotedTerminalTextEvent::OutputFragment(fragment) => {
            fragment_count = resources.checked_work_add(fragment_count, 1)?;
            plain_bytes = plain_bytes.checked_add(fragment.len()).ok_or_else(|| {
                resources.overflow(crate::resource::AsciiResourceLimitId::MaxOutputBytes)
            })?;
            html_bytes = html_bytes
                .checked_add(encoded_html_bytes(resources, fragment)?)
                .ok_or_else(|| {
                    resources.overflow(crate::resource::AsciiResourceLimitId::MaxOutputBytes)
                })?;
            width = resources
                .checked_grid_add(width, display_width_with_profile(fragment, width_profile))?;
            Ok(())
        }
    })?;
    let replay_work_units =
        resources.checked_work_add(plain_bytes.max(1), fragment_count.max(1))?;
    Ok(DeferredPartMetrics {
        width,
        planning_work_units: resources.checked_work_add(source_work, replay_work_units)?,
        replay_work_units,
        plain_bytes,
        html_bytes,
    })
}

fn visit_quoted_output(text: &str, visit: &mut dyn FnMut(&str) -> Result<()>) -> Result<()> {
    visit_quoted_terminal_text_with(text, |event| match event {
        QuotedTerminalTextEvent::SourceGrapheme(_) => Ok(()),
        QuotedTerminalTextEvent::OutputFragment(fragment) => visit(fragment),
    })
}

fn visit_quoted_body(text: &str, visit: &mut dyn FnMut(&str) -> Result<()>) -> Result<()> {
    for grapheme in text.graphemes(true) {
        if !grapheme
            .chars()
            .any(|ch| ch == '\\' || ch == '"' || (ch != ' ' && ch.is_whitespace()))
        {
            visit(grapheme)?;
            continue;
        }
        for ch in grapheme.chars() {
            match ch {
                '\\' => visit("\\\\")?,
                '"' => visit("\\\"")?,
                ' ' => visit(" ")?,
                '\t' => visit("\\t")?,
                '\n' => visit("\\n")?,
                '\r' => visit("\\r")?,
                ch if ch.is_whitespace() => {
                    let mut buffer = [0u8; 10];
                    visit(super::normalization::visible_escape(ch, &mut buffer))?;
                }
                ch => {
                    let mut buffer = [0u8; 4];
                    visit(ch.encode_utf8(&mut buffer))?;
                }
            }
        }
    }
    Ok(())
}

fn decimal_len(value: usize) -> usize {
    if value == 0 {
        1
    } else {
        value.ilog10() as usize + 1
    }
}

fn visit_decimal(value: usize, visit: &mut dyn FnMut(&str) -> Result<()>) -> Result<()> {
    let mut buffer = [0u8; usize::MAX.ilog10() as usize + 1];
    let mut value = value;
    let mut start = buffer.len();
    loop {
        start -= 1;
        buffer[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    let digits = std::str::from_utf8(&buffer[start..]).map_err(|_| layout_allocation_failed())?;
    visit(digits)
}

fn encoded_html_bytes(resources: &ResourceContext, value: &str) -> Result<usize> {
    let mut bytes = 0usize;
    super::encode::visit_html_escaped_text(value, |fragment| {
        bytes = bytes.checked_add(fragment.len()).ok_or_else(|| {
            resources.overflow(crate::resource::AsciiResourceLimitId::MaxOutputBytes)
        })?;
        Ok(())
    })?;
    Ok(bytes)
}

fn output_overflow(resources: &ResourceContext) -> AsciiError {
    resources.overflow(crate::resource::AsciiResourceLimitId::MaxOutputBytes)
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

fn invalid_deferred_text_id() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "terminal_text",
        feature: "deferred text registry id",
    }
}
