use crate::color::{AsciiColorRole, AsciiColorTheme, AsciiRgb};
use std::sync::Arc;

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
    fn text(self, arena: &str) -> &str {
        let start = self.start as usize;
        let end = start + self.len as usize;
        &arena[start..end]
    }

    fn shifted(self, byte_offset: u32) -> Self {
        Self {
            start: self
                .start
                .checked_add(byte_offset)
                .expect("terminal glyph arena byte offsets fit u32"),
            len: self.len,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GlyphId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GlyphRemap {
    source_len: u32,
    target_base: u32,
}

impl GlyphRemap {
    fn empty() -> Self {
        Self {
            source_len: 0,
            target_base: 0,
        }
    }

    fn map(self, source: GlyphId) -> GlyphId {
        assert!(
            source.0 < self.source_len,
            "arena glyph id must belong to the imported source"
        );
        GlyphId(
            self.target_base
                .checked_add(source.0)
                .expect("terminal glyph arena entry ids fit u32"),
        )
    }
}

/// Append-only UTF-8 storage for complex grapheme clusters owned by one terminal surface.
///
/// Scalar graphemes stay entirely inside the cell token. Clones share the UTF-8 backing until a
/// later append, while compact ranges remain local so cross-surface composition must import and
/// remap ids explicitly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GlyphArena {
    text: Option<Arc<String>>,
    entries: Vec<GlyphSlice>,
}

impl GlyphArena {
    pub(crate) fn import_all(&mut self, source: &Self) -> GlyphRemap {
        if source.entries.is_empty() {
            return GlyphRemap::empty();
        }

        let source_len =
            u32::try_from(source.entries.len()).expect("terminal glyph arena entry count fits u32");
        let target_base =
            u32::try_from(self.entries.len()).expect("terminal glyph arena entry count fits u32");

        if self.entries.is_empty() && self.text.is_none() {
            self.text = source.text.clone();
            self.entries = source.entries.clone();
            return GlyphRemap {
                source_len,
                target_base,
            };
        }

        let source_text = source.backing_text();
        let target_text = self.text.get_or_insert_with(|| Arc::new(String::new()));
        let byte_offset =
            u32::try_from(target_text.len()).expect("terminal glyph arena byte length fits u32");
        Arc::make_mut(target_text).push_str(source_text);
        self.entries.reserve(source.entries.len());
        self.entries.extend(
            source
                .entries
                .iter()
                .copied()
                .map(|entry| entry.shifted(byte_offset)),
        );

        GlyphRemap {
            source_len,
            target_base,
        }
    }

    fn store(&mut self, grapheme: &str) -> TerminalGlyph {
        // Scalar inspection selects storage only; layout width was resolved by the grapheme API.
        let mut chars = grapheme.chars();
        if let Some(ch) = chars.next()
            && chars.next().is_none()
        {
            return TerminalGlyph::Scalar(ch);
        }

        let target_text = self.text.get_or_insert_with(|| Arc::new(String::new()));
        let start =
            u32::try_from(target_text.len()).expect("terminal glyph arena byte length fits u32");
        let len = u32::try_from(grapheme.len()).expect("terminal grapheme byte length fits u32");
        Arc::make_mut(target_text).push_str(grapheme);
        let id = GlyphId(
            u32::try_from(self.entries.len()).expect("terminal glyph arena entry count fits u32"),
        );
        self.entries.push(GlyphSlice { start, len });
        TerminalGlyph::Arena(id)
    }

    fn get(&self, id: GlyphId) -> &str {
        self.entries[id.0 as usize].text(self.backing_text())
    }

    fn backing_text(&self) -> &str {
        self.text.as_deref().map(String::as_str).unwrap_or("")
    }

    #[cfg(test)]
    pub(crate) fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalGlyph {
    Scalar(char),
    Arena(GlyphId),
    Continuation(u32),
}

impl TerminalGlyph {
    fn continuation(owner_back: usize) -> Self {
        let owner_back =
            u32::try_from(owner_back).expect("terminal continuation owner offset fits u32");
        assert!(owner_back > 0, "continuation owner offset must be positive");
        Self::Continuation(owner_back)
    }

    fn remap(self, remap: GlyphRemap) -> Self {
        match self {
            Self::Arena(id) => Self::Arena(remap.map(id)),
            _ => self,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalCellText<'a> {
    Scalar(char),
    Grapheme(&'a str),
}

impl TerminalCellText<'_> {
    pub(crate) fn push_to(self, output: &mut String) {
        match self {
            Self::Scalar(ch) => output.push(ch),
            Self::Grapheme(grapheme) => output.push_str(grapheme),
        }
    }
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
            glyph: TerminalGlyph::Scalar(ch),
            style,
        }
    }

    fn with_glyph_style(glyph: TerminalGlyph, style: CanvasStyle) -> Self {
        Self { glyph, style }
    }

    fn continuation_with_owner_back(owner_back: usize) -> Self {
        Self {
            glyph: TerminalGlyph::continuation(owner_back),
            style: CanvasStyle::default(),
        }
    }

    pub(crate) fn output_char(self) -> Option<char> {
        match self.glyph {
            TerminalGlyph::Scalar(ch) => Some(ch),
            TerminalGlyph::Arena(_) | TerminalGlyph::Continuation(_) => None,
        }
    }

    pub(crate) fn output_text(self, arena: &GlyphArena) -> Option<TerminalCellText<'_>> {
        match self.glyph {
            TerminalGlyph::Scalar(ch) => Some(TerminalCellText::Scalar(ch)),
            TerminalGlyph::Arena(id) => Some(TerminalCellText::Grapheme(arena.get(id))),
            TerminalGlyph::Continuation(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn color(self) -> Option<CanvasColor> {
        (!self.is_continuation())
            .then_some(self.style.foreground)
            .flatten()
    }

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
            TerminalGlyph::Scalar(_) | TerminalGlyph::Arena(_) => None,
        }
    }

    pub(crate) fn is_trimmable_blank(self, preserve_color: bool) -> bool {
        matches!(self.glyph, TerminalGlyph::Scalar(' '))
            && (!preserve_color || self.style.is_plain())
    }

    pub(crate) fn remap_arena(self, remap: GlyphRemap) -> Self {
        Self {
            glyph: self.glyph.remap(remap),
            style: self.style,
        }
    }
}

pub(crate) fn push_primary_grapheme_style(
    cells: &mut Vec<TerminalCell>,
    arena: &mut GlyphArena,
    grapheme: &str,
    width: usize,
    style: CanvasStyle,
) {
    if width == 0 {
        return;
    }
    let glyph = arena.store(grapheme);
    push_terminal_glyph(cells, glyph, width, style);
}

pub(crate) fn write_primary_grapheme_style(
    cells: &mut [TerminalCell],
    arena: &mut GlyphArena,
    index: usize,
    grapheme: &str,
    width: usize,
    style: CanvasStyle,
) -> bool {
    if !can_write(cells, index, width) {
        return false;
    }
    let glyph = arena.store(grapheme);
    write_terminal_glyph(cells, index, glyph, width, style)
}

pub(crate) fn write_primary_cell_from_cell(
    cells: &mut [TerminalCell],
    index: usize,
    cell: TerminalCell,
    width: usize,
    remap: GlyphRemap,
) -> bool {
    if cell.is_continuation() {
        return false;
    }
    write_terminal_glyph(cells, index, cell.glyph.remap(remap), width, cell.style)
}

pub(crate) fn primary_width(cells: &[TerminalCell], index: usize) -> usize {
    if index >= cells.len() || cells[index].is_continuation() {
        return 0;
    }

    let mut width = 1;
    while cells
        .get(index + width)
        .is_some_and(|cell| cell.owner_back() == Some(width))
    {
        width += 1;
    }
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

pub(crate) fn mirror_cells(cells: &[TerminalCell]) -> Vec<TerminalCell> {
    let mut mirrored = vec![TerminalCell::blank(); cells.len()];
    let mut index = 0;
    while index < cells.len() {
        if cells[index].is_continuation() {
            index += 1;
            continue;
        }

        let width = primary_width(cells, index).max(1);
        let target = cells.len() - index - width;
        mirrored[target] = cells[index];
        for owner_back in 1..width {
            mirrored[target + owner_back] = TerminalCell::continuation_with_owner_back(owner_back);
        }
        index += width;
    }
    mirrored
}

fn push_terminal_glyph(
    cells: &mut Vec<TerminalCell>,
    glyph: TerminalGlyph,
    width: usize,
    style: CanvasStyle,
) {
    if width == 0 {
        return;
    }
    cells.push(TerminalCell::with_glyph_style(glyph, style));
    for owner_back in 1..width {
        cells.push(TerminalCell::continuation_with_owner_back(owner_back));
    }
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
) -> bool {
    if !can_write(cells, index, width) {
        return false;
    }

    let end = index + width;
    for position in index..end {
        clear_owner_at(cells, position);
    }

    cells[index] = TerminalCell::with_glyph_style(glyph, style);
    for owner_back in 1..width {
        cells[index + owner_back] = TerminalCell::continuation_with_owner_back(owner_back);
    }
    true
}

fn clear_owner_at(cells: &mut [TerminalCell], index: usize) {
    let Some(owner) = owner_index(cells, index) else {
        return;
    };
    let width = primary_width(cells, owner).max(1);
    let end = owner.saturating_add(width).min(cells.len());
    cells[owner..end].fill(TerminalCell::blank());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn typed_glyph_keeps_the_complete_cell_at_the_prototype_gate_size() {
        assert_eq!(size_of::<TerminalGlyph>(), 8);
        assert_eq!(size_of::<TerminalCell>(), 40);
    }

    #[test]
    fn overwriting_a_continuation_clears_its_complete_owner() {
        let mut cells = vec![TerminalCell::blank(); 4];
        let mut arena = GlyphArena::default();
        assert!(write_primary_grapheme_style(
            &mut cells,
            &mut arena,
            0,
            "中",
            2,
            CanvasStyle::default()
        ));

        assert!(write_primary_grapheme_style(
            &mut cells,
            &mut arena,
            1,
            "X",
            1,
            CanvasStyle::default()
        ));

        assert_eq!(cells[0].output_char(), Some(' '));
        assert_eq!(cells[1].output_char(), Some('X'));
        assert!(!cells[1].is_continuation());
    }

    #[test]
    fn rejected_wide_write_is_atomic() {
        let mut cells = vec![TerminalCell::with_style('a', CanvasStyle::default()); 2];
        cells[1] = TerminalCell::with_style('b', CanvasStyle::default());
        let mut arena = GlyphArena::default();

        assert!(!write_primary_grapheme_style(
            &mut cells,
            &mut arena,
            1,
            "中",
            2,
            CanvasStyle::default()
        ));
        assert_eq!(cells[0].output_char(), Some('a'));
        assert_eq!(cells[1].output_char(), Some('b'));
    }

    #[test]
    fn arena_import_remaps_local_ids_without_changing_text() {
        let mut source = GlyphArena::default();
        let mut source_cells = Vec::new();
        push_primary_grapheme_style(
            &mut source_cells,
            &mut source,
            "e\u{301}",
            1,
            CanvasStyle::default(),
        );

        let mut target = GlyphArena::default();
        let mut prefix = Vec::new();
        push_primary_grapheme_style(
            &mut prefix,
            &mut target,
            "a\u{308}",
            1,
            CanvasStyle::default(),
        );
        let remap = target.import_all(&source);
        let remapped = source_cells[0].remap_arena(remap);

        assert_eq!(target.entry_count(), 2);
        assert_eq!(
            remapped.output_text(&target),
            Some(TerminalCellText::Grapheme("e\u{301}"))
        );
    }
}
