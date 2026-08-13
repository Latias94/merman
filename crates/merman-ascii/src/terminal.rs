use crate::color::{AsciiColorRole, AsciiColorTheme, AsciiRgb};
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, AsciiResourcePolicy};
use std::collections::{HashMap, hash_map::Entry};
use std::sync::Arc;

// This maintenance cadence bounds unreachable overwrite history even for the explicit unbounded
// profile. It is not a seventh user-configurable resource limit.
const STALE_GLYPH_COMPACTION_THRESHOLD: usize = 64;

// Conservative full-cell pass counts charged to max layout work before each bounded operation.
const CELL_COMPACTION_WORK_PASSES: usize = 3;
const SURFACE_COMPACTION_WORK_PASSES: usize = 1 + CELL_COMPACTION_WORK_PASSES;
const SURFACE_APPEND_WORK_PASSES: usize = 2;
const OVERWRITE_COMPACTION_WORK_PASSES: usize = 8;
#[cfg(test)]
const CELL_MIRROR_WORK_PASSES: usize = 2;
#[cfg(test)]
const SURFACE_MIRROR_WORK_PASSES: usize = CELL_MIRROR_WORK_PASSES + CELL_COMPACTION_WORK_PASSES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CanvasColor {
    Role(AsciiColorRole),
    Direct(AsciiRgb),
}

impl CanvasColor {
    pub(crate) fn resolve(self, theme: AsciiColorTheme) -> AsciiRgb {
        match self {
            Self::Role(role) => theme.color_for(role),
            Self::Direct(color) => color,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CanvasStyle {
    pub(crate) foreground: Option<CanvasColor>,
    pub(crate) background: Option<CanvasColor>,
}

impl CanvasStyle {
    pub(crate) fn foreground(color: CanvasColor) -> Self {
        Self {
            foreground: Some(color),
            background: None,
        }
    }

    pub(crate) fn with_foreground(mut self, color: Option<CanvasColor>) -> Self {
        self.foreground = color;
        self
    }

    pub(crate) fn is_plain(self) -> bool {
        self.foreground.is_none() && self.background.is_none()
    }

    pub(crate) fn resolve(self, theme: AsciiColorTheme) -> ResolvedCanvasStyle {
        ResolvedCanvasStyle {
            foreground: self.foreground.map(|color| color.resolve(theme)),
            background: self.background.map(|color| color.resolve(theme)),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedCanvasStyle {
    pub(crate) foreground: Option<AsciiRgb>,
    pub(crate) background: Option<AsciiRgb>,
}

impl ResolvedCanvasStyle {
    pub(crate) fn is_plain(self) -> bool {
        self.foreground.is_none() && self.background.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GlyphSlice {
    start: u32,
    len: u32,
}

impl GlyphSlice {
    fn text(self, arena: &str) -> Option<&str> {
        let start = self.start as usize;
        let end = start.checked_add(self.len as usize)?;
        arena.get(start..end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GlyphId(u32);

/// Identifies one borrowed terminal-text token retained by a render-scoped resolver.
///
/// Unlike [`GlyphId`], this id never addresses bytes owned by a terminal surface. It may be
/// copied across lines and canvases without importing an arena entry; the final document encoder
/// resolves it only after the complete output-byte count has been admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DeferredTextId(u32);

impl DeferredTextId {
    pub(crate) fn try_from_index(index: usize) -> Result<Self> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| document_allocation_failed())
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Append-only UTF-8 storage for complex grapheme clusters owned by one terminal surface.
///
/// Scalar graphemes stay entirely inside the cell token. Clones share the UTF-8 backing until a
/// later append, while compact ranges remain local so cross-surface composition must import and
/// remap ids explicitly. Retained UTF-8 bytes are charged to `max_ascii_output_bytes`; overwrite
/// paths compact live cells and retry before reporting that aggregate limit.
#[derive(Debug, Clone, Default)]
pub(crate) struct GlyphArena {
    text: Option<Arc<String>>,
    entries: Vec<GlyphSlice>,
    stale_entries_since_compaction: usize,
}

impl PartialEq for GlyphArena {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text && self.entries == other.entries
    }
}

impl Eq for GlyphArena {}

impl GlyphArena {
    fn try_import_referenced_cells_where(
        &mut self,
        source: &Self,
        cells: &[TerminalCell],
        policy: AsciiResourcePolicy,
        mut include: impl FnMut(usize, &[TerminalCell]) -> Result<bool>,
    ) -> Result<HashMap<GlyphId, GlyphId>> {
        let capacity = cells.len().min(source.entries.len());
        let mut source_to_target = HashMap::new();
        source_to_target
            .try_reserve(capacity)
            .map_err(|_| glyph_allocation_failed())?;
        let mut referenced = Vec::new();
        referenced
            .try_reserve_exact(capacity)
            .map_err(|_| glyph_allocation_failed())?;
        let target_base = self.entries.len();
        let mut referenced_bytes = 0usize;

        for (index, cell) in cells.iter().copied().enumerate() {
            let TerminalGlyph::Arena(source_id, _) = cell.glyph else {
                continue;
            };
            if !include(index, cells)? {
                continue;
            }
            let Entry::Vacant(remap_entry) = source_to_target.entry(source_id) else {
                continue;
            };
            let grapheme = source.get(source_id).ok_or_else(glyph_allocation_failed)?;
            check_grapheme(policy, grapheme)?;
            referenced_bytes = referenced_bytes
                .checked_add(grapheme.len())
                .ok_or_else(glyph_allocation_failed)?;
            let target_id = target_base
                .checked_add(referenced.len())
                .and_then(|id| u32::try_from(id).ok())
                .map(GlyphId)
                .ok_or_else(glyph_allocation_failed)?;
            remap_entry.insert(target_id);
            referenced.push(source_id);
        }

        let final_entry_count = target_base
            .checked_add(referenced.len())
            .ok_or_else(glyph_allocation_failed)?;
        u32::try_from(final_entry_count).map_err(|_| glyph_allocation_failed())?;
        let final_byte_len = self
            .backing_text()
            .len()
            .checked_add(referenced_bytes)
            .ok_or_else(glyph_allocation_failed)?;
        check_retained_glyph_bytes(policy, final_byte_len)?;
        u32::try_from(final_byte_len).map_err(|_| glyph_allocation_failed())?;

        self.entries
            .try_reserve(referenced.len())
            .map_err(|_| glyph_allocation_failed())?;
        self.try_prepare_text_append(referenced_bytes)?;
        for source_id in referenced {
            let grapheme = source.get(source_id).ok_or_else(glyph_allocation_failed)?;
            self.append_complex_prepared(grapheme)?;
        }
        Ok(source_to_target)
    }

    fn try_import_referenced_cells(
        &mut self,
        source: &Self,
        cells: &[TerminalCell],
        policy: AsciiResourcePolicy,
    ) -> Result<HashMap<GlyphId, GlyphId>> {
        self.try_import_referenced_cells_where(source, cells, policy, |_, _| Ok(true))
    }

    #[cfg(test)]
    pub(crate) fn try_remap_referenced_cells(
        &mut self,
        source: &Self,
        cells: &[TerminalCell],
        policy: AsciiResourcePolicy,
    ) -> Result<Vec<TerminalCell>> {
        check_concurrent_cell_extent(policy, cells.len(), cells.len())?;
        let mut remapped = Vec::new();
        remapped
            .try_reserve_exact(cells.len())
            .map_err(|_| glyph_allocation_failed())?;
        let source_to_target = self.try_import_referenced_cells(source, cells, policy)?;
        for cell in cells.iter().copied() {
            remapped.push(try_remap_cell(cell, &source_to_target)?);
        }
        Ok(remapped)
    }

    fn try_compact_cells_from_source(
        source: &Self,
        cells: &mut [TerminalCell],
        policy: AsciiResourcePolicy,
    ) -> Result<Self> {
        check_cell_work(policy, cells.len(), CELL_COMPACTION_WORK_PASSES)?;
        let mut arena = Self::default();
        let source_to_target = arena.try_import_referenced_cells(source, cells, policy)?;
        validate_cell_remap(cells, &source_to_target)?;
        apply_validated_cell_remap(cells, &source_to_target)?;
        Ok(arena)
    }

    pub(crate) fn try_compact_in_place(
        &mut self,
        cells: &mut [TerminalCell],
        policy: AsciiResourcePolicy,
    ) -> Result<()> {
        let compacted = Self::try_compact_cells_from_source(self, cells, policy)?;
        *self = compacted;
        Ok(())
    }

    pub(crate) fn try_compact_surface(
        source: &Self,
        cells: &[TerminalCell],
        policy: AsciiResourcePolicy,
    ) -> Result<(Vec<TerminalCell>, Self)> {
        check_concurrent_cell_extent(policy, cells.len(), cells.len())?;
        check_cell_work(policy, cells.len(), SURFACE_COMPACTION_WORK_PASSES)?;
        let mut compacted_cells = Vec::new();
        compacted_cells
            .try_reserve_exact(cells.len())
            .map_err(|_| document_allocation_failed())?;
        compacted_cells.extend_from_slice(cells);
        let arena = Self::try_compact_cells_from_source(source, &mut compacted_cells, policy)?;
        let cells = compacted_cells;
        Ok((cells, arena))
    }

    fn try_store(&mut self, grapheme: &str, policy: AsciiResourcePolicy) -> Result<TerminalGlyph> {
        check_grapheme(policy, grapheme)?;
        check_retained_glyph_bytes(policy, self.backing_text().len())?;
        // Scalar inspection selects storage only; layout width was resolved by the grapheme API.
        let mut chars = grapheme.chars();
        if let Some(ch) = chars.next()
            && chars.next().is_none()
        {
            return Ok(TerminalGlyph::Scalar(ch, 1));
        }

        let final_byte_len = self
            .backing_text()
            .len()
            .checked_add(grapheme.len())
            .ok_or_else(glyph_allocation_failed)?;
        check_retained_glyph_bytes(policy, final_byte_len)?;
        u32::try_from(final_byte_len).map_err(|_| glyph_allocation_failed())?;
        let final_entry_count = self
            .entries
            .len()
            .checked_add(1)
            .ok_or_else(glyph_allocation_failed)?;
        u32::try_from(final_entry_count).map_err(|_| glyph_allocation_failed())?;

        self.entries
            .try_reserve(1)
            .map_err(|_| glyph_allocation_failed())?;
        self.try_prepare_text_append(grapheme.len())?;
        self.append_complex_prepared(grapheme)
            .map(|id| TerminalGlyph::Arena(id, 1))
    }

    fn append_complex_prepared(&mut self, grapheme: &str) -> Result<GlyphId> {
        let start =
            u32::try_from(self.backing_text().len()).map_err(|_| glyph_allocation_failed())?;
        let len = u32::try_from(grapheme.len()).map_err(|_| glyph_allocation_failed())?;
        let id = GlyphId(u32::try_from(self.entries.len()).map_err(|_| glyph_allocation_failed())?);
        self.text_mut_prepared()?.push_str(grapheme);
        self.entries.push(GlyphSlice { start, len });
        Ok(id)
    }

    fn try_prepare_text_append(&mut self, additional: usize) -> Result<()> {
        if additional == 0 {
            return Ok(());
        }

        if let Some(text) = self.text.as_mut()
            && let Some(text) = Arc::get_mut(text)
        {
            text.try_reserve(additional)
                .map_err(|_| glyph_allocation_failed())?;
            return Ok(());
        }

        let existing = self.backing_text();
        let capacity = existing
            .len()
            .checked_add(additional)
            .ok_or_else(glyph_allocation_failed)?;
        let mut replacement = String::new();
        replacement
            .try_reserve_exact(capacity)
            .map_err(|_| glyph_allocation_failed())?;
        replacement.push_str(existing);
        self.text = Some(Arc::new(replacement));
        Ok(())
    }

    fn text_mut_prepared(&mut self) -> Result<&mut String> {
        self.text
            .as_mut()
            .and_then(Arc::get_mut)
            .ok_or_else(glyph_allocation_failed)
    }

    fn get(&self, id: GlyphId) -> Option<&str> {
        self.entries
            .get(id.0 as usize)
            .and_then(|entry| entry.text(self.backing_text()))
    }

    fn backing_text(&self) -> &str {
        self.text.as_deref().map(String::as_str).unwrap_or("")
    }

    fn stale_entries_after_overwrite(&self, count: usize) -> Option<usize> {
        self.stale_entries_since_compaction.checked_add(count)
    }

    #[cfg(test)]
    pub(crate) fn entry_count(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn retained_bytes(&self) -> usize {
        self.backing_text().len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalGlyph {
    Scalar(char, u8),
    Arena(GlyphId, u8),
    Deferred(DeferredTextId),
    Continuation(u32),
}

impl TerminalGlyph {
    fn try_with_primary_width(self, width: usize) -> Result<Self> {
        if width == 0 {
            return Err(document_allocation_failed());
        }
        match self {
            Self::Scalar(ch, _) => u8::try_from(width)
                .map(|width| Self::Scalar(ch, width))
                .map_err(|_| document_allocation_failed()),
            Self::Arena(id, _) => u8::try_from(width)
                .map(|width| Self::Arena(id, width))
                .map_err(|_| document_allocation_failed()),
            Self::Deferred(id) => Ok(Self::Deferred(id)),
            Self::Continuation(_) => Err(document_allocation_failed()),
        }
    }

    const fn primary_width(self) -> Option<usize> {
        match self {
            Self::Scalar(_, width) | Self::Arena(_, width) => Some(width as usize),
            Self::Deferred(_) | Self::Continuation(_) => None,
        }
    }

    fn try_continuation(owner_back: usize) -> Result<Self> {
        let owner_back = u32::try_from(owner_back).map_err(|_| glyph_allocation_failed())?;
        if owner_back == 0 {
            return Err(glyph_allocation_failed());
        }
        Ok(Self::Continuation(owner_back))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalCellText<'a> {
    Scalar(char),
    Grapheme(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TerminalCell {
    glyph: TerminalGlyph,
    style: CanvasStyle,
}

impl TerminalCell {
    pub(crate) fn blank() -> Self {
        Self::with_style(' ', CanvasStyle::default())
    }

    pub(crate) fn with_style(ch: char, style: CanvasStyle) -> Self {
        Self {
            glyph: TerminalGlyph::Scalar(ch, 1),
            style,
        }
    }

    fn try_with_glyph_width_style(
        glyph: TerminalGlyph,
        width: usize,
        style: CanvasStyle,
    ) -> Result<Self> {
        Ok(Self {
            glyph: glyph.try_with_primary_width(width)?,
            style,
        })
    }

    fn try_continuation_with_owner_back(owner_back: usize) -> Result<Self> {
        Ok(Self {
            glyph: TerminalGlyph::try_continuation(owner_back)?,
            style: CanvasStyle::default(),
        })
    }

    pub(crate) fn output_char(self) -> Option<char> {
        match self.glyph {
            TerminalGlyph::Scalar(ch, _) => Some(ch),
            TerminalGlyph::Arena(_, _)
            | TerminalGlyph::Deferred(_)
            | TerminalGlyph::Continuation(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn output_text(self, arena: &GlyphArena) -> Option<TerminalCellText<'_>> {
        self.try_output_text(arena).ok().flatten()
    }

    pub(crate) fn try_output_text(
        self,
        arena: &GlyphArena,
    ) -> Result<Option<TerminalCellText<'_>>> {
        match self.glyph {
            TerminalGlyph::Scalar(ch, _) => Ok(Some(TerminalCellText::Scalar(ch))),
            TerminalGlyph::Arena(id, _) => arena
                .get(id)
                .map(TerminalCellText::Grapheme)
                .map(Some)
                .ok_or_else(glyph_allocation_failed),
            TerminalGlyph::Deferred(_) => Err(deferred_text_requires_resolver()),
            TerminalGlyph::Continuation(_) => Ok(None),
        }
    }

    pub(crate) const fn deferred_text_id(self) -> Option<DeferredTextId> {
        match self.glyph {
            TerminalGlyph::Deferred(id) => Some(id),
            TerminalGlyph::Scalar(_, _)
            | TerminalGlyph::Arena(_, _)
            | TerminalGlyph::Continuation(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn color(self) -> Option<CanvasColor> {
        (!self.is_continuation())
            .then_some(self.style.foreground)
            .flatten()
    }

    #[cfg(test)]
    pub(crate) fn style(self) -> Option<CanvasStyle> {
        (!self.is_continuation() && !self.style.is_plain()).then_some(self.style)
    }

    pub(crate) fn raw_style(self) -> CanvasStyle {
        if self.is_continuation() {
            CanvasStyle::default()
        } else {
            self.style
        }
    }

    pub(crate) fn set_background(&mut self, color: CanvasColor) {
        if !self.is_continuation() {
            self.style.background = Some(color);
        }
    }

    pub(crate) fn is_continuation(self) -> bool {
        matches!(self.glyph, TerminalGlyph::Continuation(_))
    }

    pub(crate) fn owner_back(self) -> Option<usize> {
        match self.glyph {
            TerminalGlyph::Continuation(owner_back) => Some(owner_back as usize),
            TerminalGlyph::Scalar(_, _)
            | TerminalGlyph::Arena(_, _)
            | TerminalGlyph::Deferred(_) => None,
        }
    }

    pub(crate) const fn primary_width_hint(self) -> Option<usize> {
        self.glyph.primary_width()
    }

    pub(crate) fn is_trimmable_blank(self, preserve_color: bool) -> bool {
        matches!(self.glyph, TerminalGlyph::Scalar(' ', _))
            && (!preserve_color || self.style.is_plain())
    }

    fn with_arena_id(self, id: GlyphId) -> Self {
        let width = self.glyph.primary_width().unwrap_or(1) as u8;
        Self {
            glyph: TerminalGlyph::Arena(id, width),
            style: self.style,
        }
    }
}

#[cfg(test)]
fn try_push_primary_grapheme_style(
    cells: &mut Vec<TerminalCell>,
    arena: &mut GlyphArena,
    grapheme: &str,
    width: usize,
    style: CanvasStyle,
) -> Result<()> {
    try_push_primary_grapheme_style_with_policy(
        cells,
        arena,
        grapheme,
        width,
        style,
        unbounded_test_policy(),
    )
}

pub(crate) fn try_push_primary_grapheme_style_with_policy(
    cells: &mut Vec<TerminalCell>,
    arena: &mut GlyphArena,
    grapheme: &str,
    width: usize,
    style: CanvasStyle,
    policy: AsciiResourcePolicy,
) -> Result<()> {
    if width == 0 {
        return Ok(());
    }
    let final_len = cells
        .len()
        .checked_add(width)
        .ok_or_else(document_allocation_failed)?;
    check_document_cell_extent(policy, final_len)?;
    check_primary_cell_extent(policy, final_len)?;
    validate_continuation_width(width)?;
    cells
        .try_reserve(width)
        .map_err(|_| document_allocation_failed())?;
    let glyph = match arena.try_store(grapheme, policy) {
        Ok(glyph) => glyph,
        Err(error) if is_retained_glyph_budget_error(&error) => {
            arena.try_compact_in_place(cells, policy)?;
            arena.try_store(grapheme, policy)?
        }
        Err(error) => return Err(error),
    };
    push_terminal_glyph_prepared(cells, glyph, width, style)?;
    Ok(())
}

pub(crate) fn try_push_primary_deferred_style_with_policy(
    cells: &mut Vec<TerminalCell>,
    id: DeferredTextId,
    width: usize,
    style: CanvasStyle,
    policy: AsciiResourcePolicy,
) -> Result<()> {
    if width == 0 {
        return Ok(());
    }
    let final_len = cells
        .len()
        .checked_add(width)
        .ok_or_else(document_allocation_failed)?;
    check_document_cell_extent(policy, final_len)?;
    check_primary_cell_extent(policy, final_len)?;
    validate_continuation_width(width)?;
    cells
        .try_reserve(width)
        .map_err(|_| document_allocation_failed())?;
    push_terminal_glyph_prepared(cells, TerminalGlyph::Deferred(id), width, style)
}

pub(crate) fn try_write_primary_deferred_style_with_policy(
    cells: &mut [TerminalCell],
    index: usize,
    id: DeferredTextId,
    width: usize,
    style: CanvasStyle,
) -> Result<bool> {
    write_terminal_glyph(cells, index, TerminalGlyph::Deferred(id), width, style)
}

pub(crate) fn try_append_cells_from_surface(
    cells: &mut Vec<TerminalCell>,
    arena: &mut GlyphArena,
    source_cells: &[TerminalCell],
    source_arena: &GlyphArena,
    policy: AsciiResourcePolicy,
) -> Result<()> {
    let final_len = cells
        .len()
        .checked_add(source_cells.len())
        .ok_or_else(document_allocation_failed)?;
    check_document_cell_extent(policy, final_len)?;
    check_primary_cell_extent(policy, final_len)?;
    check_concurrent_cell_extent(policy, final_len, source_cells.len())?;
    check_cell_work(policy, source_cells.len(), SURFACE_APPEND_WORK_PASSES)?;
    cells
        .try_reserve(source_cells.len())
        .map_err(|_| document_allocation_failed())?;
    let source_to_target = arena.try_import_referenced_cells(source_arena, source_cells, policy)?;
    for source_cell in source_cells.iter().copied() {
        cells.push(try_remap_cell(source_cell, &source_to_target)?);
    }
    Ok(())
}

#[cfg(test)]
fn try_write_primary_grapheme_style(
    cells: &mut [TerminalCell],
    arena: &mut GlyphArena,
    index: usize,
    grapheme: &str,
    width: usize,
    style: CanvasStyle,
) -> Result<bool> {
    try_write_primary_grapheme_style_with_policy(
        cells,
        arena,
        index,
        grapheme,
        width,
        style,
        unbounded_test_policy(),
    )
}

pub(crate) fn try_write_primary_grapheme_style_with_policy(
    cells: &mut [TerminalCell],
    arena: &mut GlyphArena,
    index: usize,
    grapheme: &str,
    width: usize,
    style: CanvasStyle,
    policy: AsciiResourcePolicy,
) -> Result<bool> {
    if !can_write(cells, index, width) {
        return Ok(false);
    }
    validate_continuation_width(width)?;
    let stale_entries = overwritten_arena_entries(cells, index, width)?;
    let Some(stale_entries_after_write) = arena.stale_entries_after_overwrite(stale_entries) else {
        return try_write_grapheme_after_compaction(
            cells, arena, index, grapheme, width, style, policy,
        );
    };
    if stale_entries_after_write >= STALE_GLYPH_COMPACTION_THRESHOLD {
        return try_write_grapheme_after_compaction(
            cells, arena, index, grapheme, width, style, policy,
        );
    }
    let glyph = match arena.try_store(grapheme, policy) {
        Ok(glyph) => glyph,
        Err(error) if is_retained_glyph_budget_error(&error) => {
            return try_write_grapheme_after_compaction(
                cells, arena, index, grapheme, width, style, policy,
            );
        }
        Err(error) => return Err(error),
    };
    let wrote = write_terminal_glyph(cells, index, glyph, width, style)?;
    if wrote {
        arena.stale_entries_since_compaction = stale_entries_after_write;
    }
    Ok(wrote)
}

fn try_write_grapheme_after_compaction(
    cells: &mut [TerminalCell],
    arena: &mut GlyphArena,
    index: usize,
    grapheme: &str,
    width: usize,
    style: CanvasStyle,
    policy: AsciiResourcePolicy,
) -> Result<bool> {
    let end = index
        .checked_add(width)
        .filter(|end| *end <= cells.len())
        .ok_or_else(document_allocation_failed)?;
    check_cell_work(policy, cells.len(), OVERWRITE_COMPACTION_WORK_PASSES)?;
    let mut compacted_arena = GlyphArena::default();
    let source_to_target = compacted_arena.try_import_referenced_cells_where(
        arena,
        cells,
        policy,
        |owner, cells| cell_survives_overwrite(cells, owner, index, end),
    )?;
    let glyph = compacted_arena.try_store(grapheme, policy)?;
    validate_surviving_cell_remap(cells, index, end, &source_to_target)?;

    for position in index..end {
        clear_owner_at(cells, position);
    }
    apply_validated_cell_remap(cells, &source_to_target)?;
    *arena = compacted_arena;
    let wrote = write_terminal_glyph_to_cleared_range(cells, index, glyph, width, style)?;
    Ok(wrote)
}

fn overwritten_arena_entries(cells: &[TerminalCell], index: usize, width: usize) -> Result<usize> {
    let end = index
        .checked_add(width)
        .filter(|end| *end <= cells.len())
        .ok_or_else(document_allocation_failed)?;
    let mut last_owner = None;
    let mut count = 0usize;
    for position in index..end {
        let Some(owner) = owner_index(cells, position) else {
            continue;
        };
        if last_owner == Some(owner) {
            continue;
        }
        last_owner = Some(owner);
        if matches!(cells[owner].glyph, TerminalGlyph::Arena(_, _)) {
            count = count
                .checked_add(1)
                .ok_or_else(document_allocation_failed)?;
        }
    }
    Ok(count)
}

fn cell_survives_overwrite(
    cells: &[TerminalCell],
    owner: usize,
    overwrite_start: usize,
    overwrite_end: usize,
) -> Result<bool> {
    let owner_end = owner
        .checked_add(primary_width(cells, owner).max(1))
        .ok_or_else(document_allocation_failed)?;
    Ok(owner_end <= overwrite_start || owner >= overwrite_end)
}

fn validate_surviving_cell_remap(
    cells: &[TerminalCell],
    overwrite_start: usize,
    overwrite_end: usize,
    source_to_target: &HashMap<GlyphId, GlyphId>,
) -> Result<()> {
    for (owner, cell) in cells.iter().copied().enumerate() {
        let TerminalGlyph::Arena(source_id, _) = cell.glyph else {
            continue;
        };
        if cell_survives_overwrite(cells, owner, overwrite_start, overwrite_end)?
            && !source_to_target.contains_key(&source_id)
        {
            return Err(glyph_allocation_failed());
        }
    }
    Ok(())
}

fn validate_cell_remap(
    cells: &[TerminalCell],
    source_to_target: &HashMap<GlyphId, GlyphId>,
) -> Result<()> {
    for cell in cells.iter().copied() {
        if let TerminalGlyph::Arena(source_id, _) = cell.glyph
            && !source_to_target.contains_key(&source_id)
        {
            return Err(glyph_allocation_failed());
        }
    }
    Ok(())
}

fn apply_validated_cell_remap(
    cells: &mut [TerminalCell],
    source_to_target: &HashMap<GlyphId, GlyphId>,
) -> Result<()> {
    // The validation pass makes missing ids impossible without allocating a full staged surface.
    for cell in cells {
        let TerminalGlyph::Arena(source_id, _) = cell.glyph else {
            continue;
        };
        let target_id = source_to_target
            .get(&source_id)
            .copied()
            .ok_or_else(glyph_allocation_failed)?;
        *cell = (*cell).with_arena_id(target_id);
    }
    Ok(())
}

fn try_remap_cell(
    cell: TerminalCell,
    source_to_target: &HashMap<GlyphId, GlyphId>,
) -> Result<TerminalCell> {
    match cell.glyph {
        TerminalGlyph::Arena(source_id, _) => source_to_target
            .get(&source_id)
            .copied()
            .map(|target_id| cell.with_arena_id(target_id))
            .ok_or_else(glyph_allocation_failed),
        TerminalGlyph::Scalar(_, _)
        | TerminalGlyph::Deferred(_)
        | TerminalGlyph::Continuation(_) => Ok(cell),
    }
}

pub(crate) fn try_write_primary_cell_from_surface(
    cells: &mut [TerminalCell],
    arena: &mut GlyphArena,
    index: usize,
    source_cell: TerminalCell,
    width: usize,
    source_arena: &GlyphArena,
    policy: AsciiResourcePolicy,
) -> Result<bool> {
    if source_cell.is_continuation() {
        return Ok(false);
    }
    if let Some(id) = source_cell.deferred_text_id() {
        return write_terminal_glyph(
            cells,
            index,
            TerminalGlyph::Deferred(id),
            width,
            source_cell.raw_style(),
        );
    }
    let Some(text) = source_cell.try_output_text(source_arena)? else {
        return Ok(false);
    };
    match text {
        TerminalCellText::Scalar(ch) => {
            let mut encoded = [0; 4];
            try_write_primary_grapheme_style_with_policy(
                cells,
                arena,
                index,
                ch.encode_utf8(&mut encoded),
                width,
                source_cell.raw_style(),
                policy,
            )
        }
        TerminalCellText::Grapheme(grapheme) => try_write_primary_grapheme_style_with_policy(
            cells,
            arena,
            index,
            grapheme,
            width,
            source_cell.raw_style(),
            policy,
        ),
    }
}

pub(crate) fn primary_width(cells: &[TerminalCell], index: usize) -> usize {
    let Some(cell) = cells.get(index).copied() else {
        return 0;
    };
    let width = match cell.primary_width_hint() {
        Some(width) => width,
        None if cell.deferred_text_id().is_some() => {
            let mut width = 1usize;
            while index
                .checked_add(width)
                .and_then(|position| cells.get(position))
                .is_some_and(|cell| cell.owner_back() == Some(width))
            {
                width += 1;
            }
            width
        }
        None => return 0,
    };
    debug_assert!(
        (1..width).all(|offset| cells
            .get(index + offset)
            .is_some_and(|cell| cell.owner_back() == Some(offset))),
        "primary width hint must match its continuation ownership"
    );
    width
}

pub(crate) fn owner_index(cells: &[TerminalCell], index: usize) -> Option<usize> {
    let cell = *cells.get(index)?;
    match cell.owner_back() {
        Some(owner_back) => index.checked_sub(owner_back),
        None => Some(index),
    }
}

pub(crate) fn style_at(cells: &[TerminalCell], index: usize) -> CanvasStyle {
    owner_index(cells, index)
        .and_then(|owner| cells.get(owner))
        .copied()
        .map(TerminalCell::raw_style)
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) fn try_mirror_cells(
    cells: &[TerminalCell],
    policy: AsciiResourcePolicy,
) -> Result<Vec<TerminalCell>> {
    check_document_cell_extent(policy, cells.len())?;
    check_concurrent_cell_extent(policy, cells.len(), cells.len())?;
    check_cell_work(policy, cells.len(), CELL_MIRROR_WORK_PASSES)?;
    let mut mirrored = Vec::new();
    mirrored
        .try_reserve_exact(cells.len())
        .map_err(|_| document_allocation_failed())?;
    mirrored.resize(cells.len(), TerminalCell::blank());
    let mut index = 0;
    while index < cells.len() {
        if cells[index].is_continuation() {
            index += 1;
            continue;
        }

        let width = primary_width(cells, index).max(1);
        let target = cells
            .len()
            .checked_sub(index)
            .and_then(|remaining| remaining.checked_sub(width))
            .ok_or_else(document_allocation_failed)?;
        mirrored[target] = cells[index];
        for owner_back in 1..width {
            let position = target
                .checked_add(owner_back)
                .ok_or_else(document_allocation_failed)?;
            mirrored[position] = TerminalCell::try_continuation_with_owner_back(owner_back)?;
        }
        index = index
            .checked_add(width)
            .ok_or_else(document_allocation_failed)?;
    }
    Ok(mirrored)
}

#[cfg(test)]
pub(crate) fn try_mirror_surface(
    cells: &[TerminalCell],
    arena: &GlyphArena,
    policy: AsciiResourcePolicy,
) -> Result<(Vec<TerminalCell>, GlyphArena)> {
    check_cell_work(policy, cells.len(), SURFACE_MIRROR_WORK_PASSES)?;
    let mut mirrored = try_mirror_cells(cells, policy)?;
    let arena = GlyphArena::try_compact_cells_from_source(arena, &mut mirrored, policy)?;
    Ok((mirrored, arena))
}

fn push_terminal_glyph_prepared(
    cells: &mut Vec<TerminalCell>,
    glyph: TerminalGlyph,
    width: usize,
    style: CanvasStyle,
) -> Result<()> {
    if width == 0 {
        return Ok(());
    }
    cells.push(TerminalCell::try_with_glyph_width_style(
        glyph, width, style,
    )?);
    for owner_back in 1..width {
        cells.push(TerminalCell::try_continuation_with_owner_back(owner_back)?);
    }
    Ok(())
}

fn can_write(cells: &[TerminalCell], index: usize, width: usize) -> bool {
    width > 0
        && index
            .checked_add(width)
            .is_some_and(|end| end <= cells.len())
}

fn write_terminal_glyph(
    cells: &mut [TerminalCell],
    index: usize,
    glyph: TerminalGlyph,
    width: usize,
    style: CanvasStyle,
) -> Result<bool> {
    if !can_write(cells, index, width) {
        return Ok(false);
    }
    validate_continuation_width(width)?;

    let end = index
        .checked_add(width)
        .filter(|end| *end <= cells.len())
        .ok_or_else(document_allocation_failed)?;
    for position in index..end {
        clear_owner_at(cells, position);
    }

    cells[index] = TerminalCell::try_with_glyph_width_style(glyph, width, style)?;
    for owner_back in 1..width {
        let position = index
            .checked_add(owner_back)
            .ok_or_else(document_allocation_failed)?;
        cells[position] = TerminalCell::try_continuation_with_owner_back(owner_back)?;
    }
    Ok(true)
}

fn write_terminal_glyph_to_cleared_range(
    cells: &mut [TerminalCell],
    index: usize,
    glyph: TerminalGlyph,
    width: usize,
    style: CanvasStyle,
) -> Result<bool> {
    if !can_write(cells, index, width) {
        return Ok(false);
    }
    validate_continuation_width(width)?;

    cells[index] = TerminalCell::try_with_glyph_width_style(glyph, width, style)?;
    for owner_back in 1..width {
        let position = index
            .checked_add(owner_back)
            .ok_or_else(document_allocation_failed)?;
        cells[position] = TerminalCell::try_continuation_with_owner_back(owner_back)?;
    }
    Ok(true)
}

fn clear_owner_at(cells: &mut [TerminalCell], index: usize) {
    let Some(owner) = owner_index(cells, index) else {
        return;
    };
    let width = primary_width(cells, owner).max(1);
    let end = owner.saturating_add(width).min(cells.len());
    cells[owner..end].fill(TerminalCell::blank());
}

fn validate_continuation_width(width: usize) -> Result<()> {
    if width > 1 {
        u32::try_from(width - 1).map_err(|_| document_allocation_failed())?;
    }
    Ok(())
}

fn check_grapheme(policy: AsciiResourcePolicy, grapheme: &str) -> Result<()> {
    if grapheme.is_empty() {
        return Err(AsciiError::InvalidOption {
            field: "grapheme",
            message: "terminal grapheme clusters must not be empty",
        });
    }
    policy.check(AsciiResourceLimitId::MaxGraphemeBytes, grapheme.len())
}

fn check_document_cell_extent(policy: AsciiResourcePolicy, cells: usize) -> Result<()> {
    policy.check(AsciiResourceLimitId::MaxDocumentCells, cells)
}

fn check_retained_glyph_bytes(policy: AsciiResourcePolicy, bytes: usize) -> Result<()> {
    // Complex glyph storage is retained encoded terminal payload. Charging it to the output-byte
    // budget gives append-only arenas a finite aggregate bound without inventing another limit.
    policy.check(AsciiResourceLimitId::MaxOutputBytes, bytes)
}

fn check_concurrent_cell_extent(
    policy: AsciiResourcePolicy,
    primary_cells: usize,
    temporary_cells: usize,
) -> Result<()> {
    let concurrent_cells = primary_cells
        .checked_add(temporary_cells)
        .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxGridCells))?;
    policy.check(AsciiResourceLimitId::MaxGridCells, concurrent_cells)
}

fn check_primary_cell_extent(policy: AsciiResourcePolicy, cells: usize) -> Result<()> {
    policy.check(AsciiResourceLimitId::MaxGridCells, cells)
}

fn check_cell_work(policy: AsciiResourcePolicy, cells: usize, passes: usize) -> Result<()> {
    let work_units = cells
        .checked_mul(passes)
        .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxLayoutWorkUnits))?;
    policy.check(AsciiResourceLimitId::MaxLayoutWorkUnits, work_units)
}

pub(crate) fn is_retained_glyph_budget_error(error: &AsciiError) -> bool {
    matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == AsciiResourceLimitId::MaxOutputBytes
    )
}

fn glyph_allocation_failed() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::Grapheme.as_str(),
    }
}

fn document_allocation_failed() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::Document.as_str(),
    }
}

fn deferred_text_requires_resolver() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "terminal_text",
        feature: "deferred terminal text requires a document resolver",
    }
}

#[cfg(test)]
fn unbounded_test_policy() -> AsciiResourcePolicy {
    AsciiResourcePolicy::for_profile(
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::AsciiResourceLimitExceeded;
    use std::mem::size_of;

    #[test]
    fn typed_glyph_keeps_the_complete_cell_at_the_prototype_gate_size() {
        assert_eq!(size_of::<TerminalGlyph>(), 8);
        assert_eq!(size_of::<TerminalCell>(), 40);
    }

    #[test]
    fn deferred_glyph_preserves_width_beyond_the_inline_u8_range() {
        const WIDTH: usize = 300;

        let id = DeferredTextId::try_from_index(0).expect("test deferred id should fit");
        let mut source = Vec::new();
        try_push_primary_deferred_style_with_policy(
            &mut source,
            id,
            WIDTH,
            CanvasStyle::default(),
            unbounded_test_policy(),
        )
        .expect("wide deferred text should fit the trusted-input policy");

        assert_eq!(source.len(), WIDTH);
        assert_eq!(primary_width(&source, 0), WIDTH);
        assert_eq!(source[WIDTH - 1].owner_back(), Some(WIDTH - 1));

        let mut target = vec![TerminalCell::blank(); WIDTH];
        assert!(
            try_write_primary_cell_from_surface(
                &mut target,
                &mut GlyphArena::default(),
                0,
                source[0],
                WIDTH,
                &GlyphArena::default(),
                unbounded_test_policy(),
            )
            .expect("wide deferred surface copy should remain fallible")
        );
        assert_eq!(primary_width(&target, 0), WIDTH);
        assert_eq!(target[0].deferred_text_id(), Some(id));
    }

    #[test]
    fn overwriting_a_continuation_clears_its_complete_owner() {
        let mut cells = vec![TerminalCell::blank(); 4];
        let mut arena = GlyphArena::default();
        assert!(
            try_write_primary_grapheme_style(
                &mut cells,
                &mut arena,
                0,
                "中",
                2,
                CanvasStyle::default()
            )
            .expect("test grapheme should fit")
        );

        assert!(
            try_write_primary_grapheme_style(
                &mut cells,
                &mut arena,
                1,
                "X",
                1,
                CanvasStyle::default()
            )
            .expect("test grapheme should fit")
        );

        assert_eq!(cells[0].output_char(), Some(' '));
        assert_eq!(cells[1].output_char(), Some('X'));
        assert!(!cells[1].is_continuation());
    }

    #[test]
    fn rejected_wide_write_is_atomic() {
        let mut cells = vec![TerminalCell::with_style('a', CanvasStyle::default()); 2];
        cells[1] = TerminalCell::with_style('b', CanvasStyle::default());
        let mut arena = GlyphArena::default();

        assert!(
            !try_write_primary_grapheme_style(
                &mut cells,
                &mut arena,
                1,
                "中",
                2,
                CanvasStyle::default()
            )
            .expect("rejected write should not be a resource error")
        );
        assert_eq!(cells[0].output_char(), Some('a'));
        assert_eq!(cells[1].output_char(), Some('b'));
    }

    #[test]
    fn referenced_arena_import_remaps_local_ids_without_changing_text() {
        let mut source = GlyphArena::default();
        let mut source_cells = Vec::new();
        try_push_primary_grapheme_style(
            &mut source_cells,
            &mut source,
            "e\u{301}",
            1,
            CanvasStyle::default(),
        )
        .expect("test grapheme should fit");

        let mut target = GlyphArena::default();
        let mut prefix = Vec::new();
        try_push_primary_grapheme_style(
            &mut prefix,
            &mut target,
            "a\u{308}",
            1,
            CanvasStyle::default(),
        )
        .expect("test grapheme should fit");
        let remapped = target
            .try_remap_referenced_cells(&source, &source_cells, AsciiResourcePolicy::default())
            .expect("test arena should fit");

        assert_eq!(target.entry_count(), 2);
        assert_eq!(
            remapped[0].output_text(&target),
            Some(TerminalCellText::Grapheme("e\u{301}"))
        );
    }

    #[test]
    fn compact_surface_imports_only_referenced_complex_glyphs() {
        let mut source = GlyphArena::default();
        let mut source_cells = Vec::new();
        try_push_primary_grapheme_style(
            &mut source_cells,
            &mut source,
            "e\u{301}",
            1,
            CanvasStyle::default(),
        )
        .expect("first test grapheme should fit");
        try_push_primary_grapheme_style(
            &mut source_cells,
            &mut source,
            "a\u{308}",
            1,
            CanvasStyle::default(),
        )
        .expect("second test grapheme should fit");

        let (cells, arena) =
            GlyphArena::try_compact_surface(&source, &source_cells[..1], unbounded_test_policy())
                .expect("referenced surface should compact");

        assert_eq!(arena.entry_count(), 1);
        assert_eq!(
            cells[0].output_text(&arena),
            Some(TerminalCellText::Grapheme("e\u{301}"))
        );
    }

    #[test]
    fn referenced_arena_import_checks_each_grapheme_against_the_target_policy() {
        let mut source = GlyphArena::default();
        let mut cells = Vec::new();
        try_push_primary_grapheme_style(
            &mut cells,
            &mut source,
            "e\u{301}",
            1,
            CanvasStyle::default(),
        )
        .expect("source test grapheme should fit");
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 2)
            .expect("valid test override");

        let error = GlyphArena::default()
            .try_remap_referenced_cells(&source, &cells, policy)
            .expect_err("three-byte grapheme must exceed a two-byte policy");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxGraphemeBytes,
                actual: 3,
                max: 2,
                ..
            })
        ));
    }

    #[test]
    fn bounded_overwrites_compact_before_exceeding_the_output_byte_aggregate() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 3)
            .expect("valid test override")
            .with_limit(AsciiResourceLimitId::MaxGridCells, 1)
            .expect("valid test override");
        let mut cells = vec![TerminalCell::blank()];
        let mut arena = GlyphArena::default();

        for grapheme in ["e\u{301}", "a\u{308}", "o\u{302}"]
            .into_iter()
            .cycle()
            .take(30)
        {
            assert!(
                try_write_primary_grapheme_style_with_policy(
                    &mut cells,
                    &mut arena,
                    0,
                    grapheme,
                    1,
                    CanvasStyle::default(),
                    policy,
                )
                .expect("overwriting one active three-byte glyph should stay within the budget")
            );
            assert_eq!(arena.retained_bytes(), 3);
            assert_eq!(arena.entry_count(), 1);
            assert_eq!(
                cells[0].output_text(&arena),
                Some(TerminalCellText::Grapheme(grapheme))
            );
        }
    }

    #[test]
    fn scalar_overwrite_cannot_bypass_an_existing_retained_byte_overage() {
        let mut cells = vec![TerminalCell::blank()];
        let mut arena = GlyphArena::default();
        try_write_primary_grapheme_style_with_policy(
            &mut cells,
            &mut arena,
            0,
            "e\u{301}",
            1,
            CanvasStyle::default(),
            unbounded_test_policy(),
        )
        .expect("trusted source glyph should fit");
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 1)
            .expect("valid test override");

        assert!(
            try_write_primary_grapheme_style_with_policy(
                &mut cells,
                &mut arena,
                0,
                "x",
                1,
                CanvasStyle::default(),
                policy,
            )
            .expect("overwriting the only retained glyph should compact before storing a scalar")
        );

        assert_eq!(arena.entry_count(), 0);
        assert_eq!(arena.retained_bytes(), 0);
        assert_eq!(cells[0].output_char(), Some('x'));
    }

    #[test]
    fn overwrite_compaction_charges_its_bounded_remap_work_before_mutating() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 3)
            .expect("valid test override")
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 7)
            .expect("valid test override");
        let mut cells = vec![TerminalCell::blank()];
        let mut arena = GlyphArena::default();
        try_write_primary_grapheme_style_with_policy(
            &mut cells,
            &mut arena,
            0,
            "e\u{301}",
            1,
            CanvasStyle::default(),
            policy,
        )
        .expect("first glyph should fit without compaction");

        let error = try_write_primary_grapheme_style_with_policy(
            &mut cells,
            &mut arena,
            0,
            "a\u{308}",
            1,
            CanvasStyle::default(),
            policy,
        )
        .expect_err("eight bounded remap passes must exceed a seven-unit work budget");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxLayoutWorkUnits,
                actual: OVERWRITE_COMPACTION_WORK_PASSES,
                max: 7,
                ..
            })
        ));
        assert_eq!(arena.entry_count(), 1);
        assert_eq!(arena.retained_bytes(), 3);
        assert_eq!(
            cells[0].output_text(&arena),
            Some(TerminalCellText::Grapheme("e\u{301}"))
        );
    }

    #[test]
    fn unbounded_overwrites_periodically_compact_stale_arena_history() {
        let policy = unbounded_test_policy()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 1)
            .expect("valid test override");
        let mut cells = vec![TerminalCell::blank()];
        let mut arena = GlyphArena::default();

        for grapheme in ["e\u{301}", "a\u{308}", "o\u{302}"]
            .into_iter()
            .cycle()
            .take(STALE_GLYPH_COMPACTION_THRESHOLD * 3 + 8)
        {
            assert!(
                try_write_primary_grapheme_style_with_policy(
                    &mut cells,
                    &mut arena,
                    0,
                    grapheme,
                    1,
                    CanvasStyle::default(),
                    policy,
                )
                .expect("periodic compaction should keep trusted-input overwrites fallible")
            );
            assert!(arena.entry_count() <= STALE_GLYPH_COMPACTION_THRESHOLD);
            assert!(arena.retained_bytes() <= STALE_GLYPH_COMPACTION_THRESHOLD * 3);
            assert_eq!(
                cells[0].output_text(&arena),
                Some(TerminalCellText::Grapheme(grapheme))
            );
        }
    }

    #[test]
    fn compact_surface_charges_the_source_and_temporary_cell_extents() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 3)
            .expect("valid test override");
        let cells = vec![TerminalCell::blank(); 2];

        let error = GlyphArena::try_compact_surface(&GlyphArena::default(), &cells, policy)
            .expect_err("two concurrent two-cell surfaces must exceed a three-cell grid budget");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxGridCells,
                actual: 4,
                max: 3,
                ..
            })
        ));
    }

    #[test]
    fn mirror_charges_the_source_and_mirrored_cell_extents() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 3)
            .expect("valid test override");
        let cells = vec![TerminalCell::blank(); 2];

        let error = try_mirror_cells(&cells, policy)
            .expect_err("source and mirrored cells must share the grid budget");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxGridCells,
                actual: 4,
                max: 3,
                ..
            })
        ));
    }

    #[test]
    fn active_complex_glyphs_cannot_exceed_the_output_byte_aggregate() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 3)
            .expect("valid test override");
        let mut cells = Vec::new();
        let mut arena = GlyphArena::default();
        try_push_primary_grapheme_style_with_policy(
            &mut cells,
            &mut arena,
            "e\u{301}",
            1,
            CanvasStyle::default(),
            policy,
        )
        .expect("first active grapheme should fit exactly");

        let error = try_push_primary_grapheme_style_with_policy(
            &mut cells,
            &mut arena,
            "a\u{308}",
            1,
            CanvasStyle::default(),
            policy,
        )
        .expect_err("two active three-byte graphemes must exceed a three-byte aggregate");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxOutputBytes,
                actual: 6,
                max: 3,
                ..
            })
        ));
        assert_eq!(cells.len(), 1);
        assert_eq!(arena.retained_bytes(), 3);
        assert_eq!(arena.entry_count(), 1);
    }

    #[test]
    fn surface_write_imports_only_the_referenced_source_glyph() {
        let mut source_arena = GlyphArena::default();
        let mut source_cells = Vec::new();
        for grapheme in ["e\u{301}", "a\u{308}"] {
            try_push_primary_grapheme_style(
                &mut source_cells,
                &mut source_arena,
                grapheme,
                1,
                CanvasStyle::default(),
            )
            .expect("source test grapheme should fit");
        }
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 3)
            .expect("valid test override");
        let mut target_cells = vec![TerminalCell::blank()];
        let mut target_arena = GlyphArena::default();

        assert!(
            try_write_primary_cell_from_surface(
                &mut target_cells,
                &mut target_arena,
                0,
                source_cells[0],
                1,
                &source_arena,
                policy,
            )
            .expect("one referenced glyph should fit without importing its unused sibling")
        );

        assert_eq!(target_arena.entry_count(), 1);
        assert_eq!(target_arena.retained_bytes(), 3);
        assert_eq!(
            target_cells[0].output_text(&target_arena),
            Some(TerminalCellText::Grapheme("e\u{301}"))
        );
    }
}
