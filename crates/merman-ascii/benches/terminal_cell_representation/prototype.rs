use crate::allocator::{AllocationMetrics, CountingSystemAllocator};
use crate::terminal::CanvasStyle;
use std::collections::{HashMap, HashSet};
use std::hint::black_box;
use std::mem::{align_of, size_of};
use std::sync::Arc;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const LOGICAL_GRAPHEMES: usize = 1_024;
const ASCII_PATTERN: &[&str] = &[
    "A", "s", "c", "i", "i", "-", "0", "1", "2", "3", "[", "]", "(", ")", "<", ">",
];
const CJK_PATTERN: &[&str] = &["中", "文", "终", "端", "图", "表", "节", "点"];
const EMOJI_PATTERN: &[&str] = &["e\u{301}", "👩‍💻", "👍🏽", "✈️", "🇺🇸"];

#[derive(Debug, Clone, Copy)]
struct PreparedScalar {
    ch: char,
    width: usize,
}

#[derive(Debug, Clone)]
struct PreparedGrapheme {
    text: String,
    width: usize,
    scalar: Option<char>,
    scalars: Vec<PreparedScalar>,
}

#[derive(Debug, Clone)]
pub(crate) struct Workload {
    name: &'static str,
    graphemes: Vec<PreparedGrapheme>,
    output_bytes: usize,
    scalar_cells: usize,
    grapheme_cells: usize,
    complex_occurrences: usize,
    distinct_complex: usize,
    complex_bytes: usize,
}

impl Workload {
    pub(crate) fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn logical_graphemes(&self) -> usize {
        self.graphemes.len()
    }

    fn from_texts(name: &'static str, texts: impl IntoIterator<Item = String>) -> Self {
        let mut graphemes = Vec::with_capacity(LOGICAL_GRAPHEMES);
        let mut output_bytes = 0;
        let mut scalar_cells = 0;
        let mut grapheme_cells = 0;
        let mut complex_occurrences = 0;
        let mut distinct_complex = HashSet::new();
        let mut complex_bytes = 0;

        for text in texts {
            let mut chars = text.chars();
            let first = chars.next().expect("prototype patterns are non-empty");
            let scalar = chars.next().is_none().then_some(first);
            let scalars = text
                .chars()
                .map(|ch| PreparedScalar {
                    ch,
                    width: UnicodeWidthChar::width(ch).unwrap_or(0).max(1),
                })
                .collect::<Vec<_>>();
            let width = UnicodeWidthStr::width(text.as_str());
            assert!(width > 0, "prototype patterns must have positive width");

            output_bytes += text.len();
            scalar_cells += scalars.iter().map(|scalar| scalar.width).sum::<usize>();
            grapheme_cells += width;
            if scalar.is_none() {
                complex_occurrences += 1;
                complex_bytes += text.len();
                distinct_complex.insert(text.clone());
            }
            graphemes.push(PreparedGrapheme {
                text,
                width,
                scalar,
                scalars,
            });
        }

        Self {
            name,
            graphemes,
            output_bytes,
            scalar_cells,
            grapheme_cells,
            complex_occurrences,
            distinct_complex: distinct_complex.len(),
            complex_bytes,
        }
    }

    fn from_pattern(name: &'static str, pattern: &'static [&'static str]) -> Self {
        Self::from_texts(
            name,
            (0..LOGICAL_GRAPHEMES).map(|index| pattern[index % pattern.len()].to_string()),
        )
    }

    fn unique_combining() -> Self {
        Self::from_texts(
            "complex_unique",
            (0..LOGICAL_GRAPHEMES).map(|index| {
                let high = char::from_u32(0x0300 + ((index / 32) as u32))
                    .expect("generated combining mark is valid");
                let low = char::from_u32(0x0300 + ((index % 32) as u32))
                    .expect("generated combining mark is valid");
                format!("a{high}{low}")
            }),
        )
    }

    fn expected_text(&self) -> String {
        let mut output = String::with_capacity(self.output_bytes);
        for grapheme in &self.graphemes {
            output.push_str(&grapheme.text);
        }
        output
    }

    fn expected_mirror_text(&self) -> String {
        let mut output = String::with_capacity(self.output_bytes);
        for grapheme in self.graphemes.iter().rev() {
            output.push_str(&grapheme.text);
        }
        output
    }
}

pub(crate) fn workloads() -> Vec<Workload> {
    vec![
        Workload::from_pattern("ascii", ASCII_PATTERN),
        Workload::from_pattern("cjk", CJK_PATTERN),
        Workload::from_pattern("emoji_repeated", EMOJI_PATTERN),
        Workload::unique_combining(),
    ]
}

pub(crate) trait PrototypeSurface: Clone + Sized + 'static {
    const NAME: &'static str;

    fn paint(workload: &Workload) -> Self;
    fn finalize(&self) -> String;
    fn mirror(self) -> Self;
    fn compose(&self) -> Self;
    fn cell_count(&self) -> usize;
    fn arena_entries(&self) -> usize;
}

#[derive(Debug, Clone)]
pub(crate) struct CurrentScalarSurface {
    cells: Vec<LegacyScalarCell>,
    output_bytes: usize,
}

/// Frozen pre-U2 scalar cell. Keep this benchmark baseline independent from production refactors.
#[derive(Debug, Clone, Copy)]
struct LegacyScalarCell {
    ch: char,
    style: CanvasStyle,
    continuation: bool,
}

impl LegacyScalarCell {
    fn blank() -> Self {
        Self::with_style(' ', CanvasStyle::default())
    }

    fn with_style(ch: char, style: CanvasStyle) -> Self {
        Self {
            ch,
            style,
            continuation: false,
        }
    }

    fn continuation() -> Self {
        Self {
            ch: ' ',
            style: CanvasStyle::default(),
            continuation: true,
        }
    }

    fn output_char_with_style(self) -> Option<(char, CanvasStyle)> {
        (!self.continuation).then_some((self.ch, self.style))
    }

    fn is_continuation(self) -> bool {
        self.continuation
    }
}

impl PrototypeSurface for CurrentScalarSurface {
    const NAME: &'static str = "current_scalar";

    fn paint(workload: &Workload) -> Self {
        let mut cells = Vec::with_capacity(workload.scalar_cells);
        let style = CanvasStyle::default();
        for grapheme in &workload.graphemes {
            for scalar in &grapheme.scalars {
                cells.push(LegacyScalarCell::with_style(scalar.ch, style));
                for _ in 1..scalar.width {
                    cells.push(LegacyScalarCell::continuation());
                }
            }
        }
        Self {
            cells,
            output_bytes: workload.output_bytes,
        }
    }

    fn finalize(&self) -> String {
        let mut output = String::with_capacity(self.output_bytes);
        for cell in &self.cells {
            if let Some((ch, _style)) = cell.output_char_with_style() {
                output.push(ch);
            }
        }
        output
    }

    fn mirror(self) -> Self {
        let mut mirrored = vec![LegacyScalarCell::blank(); self.cells.len()];
        let mut index = 0;
        while index < self.cells.len() {
            if self.cells[index].is_continuation() {
                index += 1;
                continue;
            }
            let width = current_primary_width(&self.cells, index);
            let target = self.cells.len() - index - width;
            mirrored[target] = self.cells[index];
            for offset in 1..width {
                mirrored[target + offset] = LegacyScalarCell::continuation();
            }
            index += width;
        }
        Self {
            cells: mirrored,
            output_bytes: self.output_bytes,
        }
    }

    fn compose(&self) -> Self {
        let mut cells = Vec::with_capacity(self.cells.len());
        cells.extend_from_slice(&self.cells);
        Self {
            cells,
            output_bytes: self.output_bytes,
        }
    }

    fn cell_count(&self) -> usize {
        self.cells.len()
    }

    fn arena_entries(&self) -> usize {
        0
    }
}

fn current_primary_width(cells: &[LegacyScalarCell], index: usize) -> usize {
    let mut width = 1;
    while cells
        .get(index + width)
        .is_some_and(|cell| cell.is_continuation())
    {
        width += 1;
    }
    width
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackedGlyph {
    Scalar(char, u8),
    Arena(u32, u8),
    Continuation(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackedGlyphView {
    Scalar(char, u8),
    Arena(u32, u8),
    Continuation(u32),
}

impl PackedGlyph {
    fn scalar(ch: char) -> Self {
        Self::Scalar(ch, 1)
    }

    fn arena(id: u32) -> Self {
        Self::Arena(id, 1)
    }

    fn with_primary_width(self, width: usize) -> Self {
        let width = u8::try_from(width).expect("terminal grapheme width fits u8");
        assert!(width > 0);
        match self {
            Self::Scalar(ch, _) => Self::Scalar(ch, width),
            Self::Arena(id, _) => Self::Arena(id, width),
            Self::Continuation(_) => panic!("continuations cannot own a primary width"),
        }
    }

    fn primary_width(self) -> Option<usize> {
        match self {
            Self::Scalar(_, width) | Self::Arena(_, width) => Some(width as usize),
            Self::Continuation(_) => None,
        }
    }

    fn continuation(owner_back: usize) -> Self {
        let owner_back =
            u32::try_from(owner_back).expect("terminal width fits packed owner offset");
        assert!(owner_back > 0);
        Self::Continuation(owner_back)
    }

    fn view(self) -> PackedGlyphView {
        match self {
            Self::Scalar(ch, width) => PackedGlyphView::Scalar(ch, width),
            Self::Arena(id, width) => PackedGlyphView::Arena(id, width),
            Self::Continuation(owner_back) => PackedGlyphView::Continuation(owner_back),
        }
    }

    fn remap_arena(self, remap: &[u32]) -> Self {
        match self.view() {
            PackedGlyphView::Arena(id, width) => Self::Arena(remap[id as usize], width),
            _ => self,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PackedCell {
    glyph: PackedGlyph,
    style: CanvasStyle,
}

impl PackedCell {
    fn primary(glyph: PackedGlyph, style: CanvasStyle) -> Self {
        Self { glyph, style }
    }

    fn continuation(owner_back: usize) -> Self {
        Self {
            glyph: PackedGlyph::continuation(owner_back),
            style: CanvasStyle::default(),
        }
    }

    fn blank() -> Self {
        Self::primary(PackedGlyph::scalar(' '), CanvasStyle::default())
    }

    fn remap_arena(self, remap: &[u32]) -> Self {
        Self {
            glyph: self.glyph.remap_arena(remap),
            style: self.style,
        }
    }
}

fn push_packed(cells: &mut Vec<PackedCell>, glyph: PackedGlyph, width: usize) {
    cells.push(PackedCell::primary(
        glyph.with_primary_width(width),
        CanvasStyle::default(),
    ));
    for owner_back in 1..width {
        cells.push(PackedCell::continuation(owner_back));
    }
}

fn packed_primary_width(cells: &[PackedCell], index: usize) -> usize {
    let Some(width) = cells.get(index).and_then(|cell| cell.glyph.primary_width()) else {
        return 0;
    };
    debug_assert!((1..width).all(
        |offset| cells.get(index + offset).is_some_and(|cell| matches!(
            cell.glyph.view(),
            PackedGlyphView::Continuation(owner_back) if owner_back as usize == offset
        ))
    ));
    width
}

fn mirror_packed_cells(cells: &[PackedCell]) -> Vec<PackedCell> {
    let mut mirrored = vec![PackedCell::blank(); cells.len()];
    let mut index = 0;
    while index < cells.len() {
        if matches!(cells[index].glyph.view(), PackedGlyphView::Continuation(_)) {
            index += 1;
            continue;
        }
        let width = packed_primary_width(cells, index);
        let target = cells.len() - index - width;
        mirrored[target] = cells[index];
        for owner_back in 1..width {
            mirrored[target + owner_back] = PackedCell::continuation(owner_back);
        }
        index += width;
    }
    mirrored
}

fn finalize_packed(cells: &[PackedCell], arena: &[Arc<str>], output_bytes: usize) -> String {
    let mut output = String::with_capacity(output_bytes);
    let mut index = 0usize;
    while let Some(cell) = cells.get(index) {
        match cell.glyph {
            PackedGlyph::Scalar(ch, width) => {
                output.push(ch);
                index += width as usize;
            }
            PackedGlyph::Arena(id, width) => {
                output.push_str(&arena[id as usize]);
                index += width as usize;
            }
            PackedGlyph::Continuation(_) => {
                unreachable!("primary-run iteration skips continuations")
            }
        }
    }
    output
}

#[derive(Debug, Clone, Copy)]
struct GlyphSlice {
    start: u32,
    len: u32,
}

impl GlyphSlice {
    fn get(self, text: &str) -> &str {
        let start = self.start as usize;
        let end = start + self.len as usize;
        &text[start..end]
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CompactArenaSurface {
    cells: Vec<PackedCell>,
    text: Option<Arc<String>>,
    entries: Vec<GlyphSlice>,
    output_bytes: usize,
}

impl PrototypeSurface for CompactArenaSurface {
    const NAME: &'static str = "compact_arena";

    fn paint(workload: &Workload) -> Self {
        let mut cells = Vec::with_capacity(workload.grapheme_cells);
        let mut text = String::with_capacity(workload.complex_bytes);
        let mut entries = Vec::with_capacity(workload.complex_occurrences);
        for grapheme in &workload.graphemes {
            let glyph = match grapheme.scalar {
                Some(ch) => PackedGlyph::scalar(ch),
                None => {
                    let id = u32::try_from(entries.len()).expect("prototype arena fits u32");
                    let start = u32::try_from(text.len()).expect("prototype text arena fits u32");
                    let len =
                        u32::try_from(grapheme.text.len()).expect("prototype grapheme fits u32");
                    text.push_str(&grapheme.text);
                    entries.push(GlyphSlice { start, len });
                    PackedGlyph::arena(id)
                }
            };
            push_packed(&mut cells, glyph, grapheme.width);
        }
        Self {
            cells,
            text: (!entries.is_empty()).then(|| Arc::new(text)),
            entries,
            output_bytes: workload.output_bytes,
        }
    }

    fn finalize(&self) -> String {
        let text = self.text.as_deref().map(String::as_str).unwrap_or("");
        let mut output = String::with_capacity(self.output_bytes);
        let mut index = 0usize;
        while let Some(cell) = self.cells.get(index) {
            match cell.glyph {
                PackedGlyph::Scalar(ch, width) => {
                    output.push(ch);
                    index += width as usize;
                }
                PackedGlyph::Arena(id, width) => {
                    output.push_str(self.entries[id as usize].get(text));
                    index += width as usize;
                }
                PackedGlyph::Continuation(_) => {
                    unreachable!("primary-run iteration skips continuations")
                }
            }
        }
        output
    }

    fn mirror(self) -> Self {
        Self {
            cells: mirror_packed_cells(&self.cells),
            text: self.text,
            entries: self.entries,
            output_bytes: self.output_bytes,
        }
    }

    fn compose(&self) -> Self {
        if self.entries.is_empty() {
            return Self {
                cells: self.cells.clone(),
                text: None,
                entries: Vec::new(),
                output_bytes: self.output_bytes,
            };
        }
        let mut entries = Vec::with_capacity(self.entries.len() + 1);
        entries.push(GlyphSlice { start: 0, len: 0 });
        let mut remap = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            let id = u32::try_from(entries.len()).expect("prototype arena fits u32");
            entries.push(*entry);
            remap.push(id);
        }
        let cells = self
            .cells
            .iter()
            .map(|cell| cell.remap_arena(&remap))
            .collect();
        Self {
            cells,
            text: self.text.clone(),
            entries,
            output_bytes: self.output_bytes,
        }
    }

    fn cell_count(&self) -> usize {
        self.cells.len()
    }

    fn arena_entries(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Debug)]
pub(crate) struct CompactInternedSurface {
    cells: Vec<PackedCell>,
    arena: Vec<Arc<str>>,
    ids: Option<HashMap<Arc<str>, u32>>,
    output_bytes: usize,
}

impl Clone for CompactInternedSurface {
    fn clone(&self) -> Self {
        Self {
            cells: self.cells.clone(),
            arena: self.arena.clone(),
            ids: None,
            output_bytes: self.output_bytes,
        }
    }
}

impl CompactInternedSurface {
    fn empty(cell_capacity: usize, arena_capacity: usize) -> Self {
        Self {
            cells: Vec::with_capacity(cell_capacity),
            arena: Vec::with_capacity(arena_capacity),
            ids: Some(HashMap::with_capacity(arena_capacity)),
            output_bytes: 0,
        }
    }

    fn ensure_ids(&mut self) -> &mut HashMap<Arc<str>, u32> {
        self.ids.get_or_insert_with(|| {
            self.arena
                .iter()
                .enumerate()
                .map(|(id, glyph)| {
                    (
                        Arc::clone(glyph),
                        u32::try_from(id).expect("prototype arena fits u32"),
                    )
                })
                .collect()
        })
    }

    fn intern_text(&mut self, text: &str) -> u32 {
        if let Some(id) = self.ensure_ids().get(text) {
            return *id;
        }
        let id = u32::try_from(self.arena.len()).expect("prototype arena fits u32");
        let glyph = Arc::<str>::from(text);
        self.arena.push(Arc::clone(&glyph));
        self.ensure_ids().insert(glyph, id);
        id
    }

    fn import_glyph(&mut self, glyph: &Arc<str>) -> u32 {
        if let Some(id) = self.ensure_ids().get(glyph.as_ref()) {
            return *id;
        }
        let id = u32::try_from(self.arena.len()).expect("prototype arena fits u32");
        self.arena.push(Arc::clone(glyph));
        self.ensure_ids().insert(Arc::clone(glyph), id);
        id
    }
}

impl PrototypeSurface for CompactInternedSurface {
    const NAME: &'static str = "compact_interned";

    fn paint(workload: &Workload) -> Self {
        let mut surface = Self::empty(workload.grapheme_cells, workload.distinct_complex);
        surface.output_bytes = workload.output_bytes;
        for grapheme in &workload.graphemes {
            let glyph = match grapheme.scalar {
                Some(ch) => PackedGlyph::scalar(ch),
                None => PackedGlyph::arena(surface.intern_text(&grapheme.text)),
            };
            push_packed(&mut surface.cells, glyph, grapheme.width);
        }
        surface
    }

    fn finalize(&self) -> String {
        finalize_packed(&self.cells, &self.arena, self.output_bytes)
    }

    fn mirror(self) -> Self {
        Self {
            cells: mirror_packed_cells(&self.cells),
            arena: self.arena,
            ids: None,
            output_bytes: self.output_bytes,
        }
    }

    fn compose(&self) -> Self {
        if self.arena.is_empty() {
            return Self {
                cells: self.cells.clone(),
                arena: Vec::new(),
                ids: None,
                output_bytes: self.output_bytes,
            };
        }
        let mut target = Self::empty(self.cells.len(), self.arena.len());
        target.output_bytes = self.output_bytes;
        let mut remap = Vec::with_capacity(self.arena.len());
        for glyph in &self.arena {
            remap.push(target.import_glyph(glyph));
        }
        target
            .cells
            .extend(self.cells.iter().map(|cell| cell.remap_arena(&remap)));
        target
    }

    fn cell_count(&self) -> usize {
        self.cells.len()
    }

    fn arena_entries(&self) -> usize {
        self.arena.len()
    }
}

pub(crate) fn verify_semantics(workloads: &[Workload]) {
    for workload in workloads {
        let expected = workload.expected_text();
        let mirrored = workload.expected_mirror_text();
        let scalar = CurrentScalarSurface::paint(workload);
        let arena = CompactArenaSurface::paint(workload);
        let interned = CompactInternedSurface::paint(workload);

        assert_eq!(scalar.finalize(), expected);
        assert_eq!(arena.finalize(), expected);
        assert_eq!(interned.finalize(), expected);
        assert_eq!(scalar.clone().finalize(), expected);
        assert_eq!(arena.clone().finalize(), expected);
        assert_eq!(interned.clone().finalize(), expected);
        assert_eq!(scalar.compose().finalize(), expected);
        assert_eq!(arena.compose().finalize(), expected);
        assert_eq!(interned.compose().finalize(), expected);
        assert_eq!(arena.clone().mirror().finalize(), mirrored);
        assert_eq!(interned.clone().mirror().finalize(), mirrored);

        if workload.complex_occurrences == 0 {
            assert_eq!(scalar.clone().mirror().finalize(), mirrored);
        } else {
            assert_ne!(scalar.clone().mirror().finalize(), mirrored);
        }

        assert_eq!(scalar.cell_count(), workload.scalar_cells);
        assert_eq!(arena.cell_count(), workload.grapheme_cells);
        assert_eq!(interned.cell_count(), workload.grapheme_cells);
        assert_eq!(arena.arena_entries(), workload.complex_occurrences);
        assert_eq!(interned.arena_entries(), workload.distinct_complex);
    }
}

fn measure_allocations<T>(
    allocator: &CountingSystemAllocator,
    representation: &'static str,
    workload: &'static str,
    operation: &'static str,
    action: impl FnOnce() -> T,
) {
    let snapshot = allocator.begin_measurement();
    let result = action();
    black_box(&result);
    let metrics = allocator.finish_measurement(snapshot);
    print_allocation_metrics(representation, workload, operation, &metrics);
    drop(result);
}

fn print_allocation_metrics(
    representation: &str,
    workload: &str,
    operation: &str,
    metrics: &AllocationMetrics,
) {
    assert!(!metrics.counter_overflowed, "allocator counter overflowed");
    assert!(
        !metrics.counter_underflowed,
        "allocator counter underflowed"
    );
    let retained_growth = metrics
        .live_bytes_after
        .saturating_sub(metrics.snapshot_live_bytes);
    println!(
        "allocation representation={representation} workload={workload} operation={operation} count={} bytes={} peak_growth={} retained_growth={} peak_live={}",
        metrics.allocation_count,
        metrics.allocated_bytes,
        metrics.peak_growth_bytes,
        retained_growth,
        metrics.peak_live_bytes,
    );
}

fn report_surface<S: PrototypeSurface>(allocator: &CountingSystemAllocator, workload: &Workload) {
    measure_allocations(allocator, S::NAME, workload.name, "paint", || {
        S::paint(workload)
    });

    let source = S::paint(workload);
    measure_allocations(allocator, S::NAME, workload.name, "clone", || {
        source.clone()
    });
    measure_allocations(allocator, S::NAME, workload.name, "finalize", || {
        source.finalize()
    });
    measure_allocations(allocator, S::NAME, workload.name, "compose", || {
        source.compose()
    });

    let mirror_source = S::paint(workload);
    measure_allocations(allocator, S::NAME, workload.name, "mirror", || {
        mirror_source.mirror()
    });
}

pub(crate) fn print_structural_and_allocation_report(allocator: &CountingSystemAllocator) {
    let workloads = workloads();
    verify_semantics(&workloads);

    println!(
        "size type=current_terminal_cell bytes={} align={}",
        size_of::<LegacyScalarCell>(),
        align_of::<LegacyScalarCell>()
    );
    println!(
        "size type=canvas_style bytes={} align={}",
        size_of::<CanvasStyle>(),
        align_of::<CanvasStyle>()
    );
    println!(
        "size type=glyph_token bytes={} align={}",
        size_of::<PackedGlyph>(),
        align_of::<PackedGlyph>()
    );
    println!(
        "size type=packed_cell bytes={} align={}",
        size_of::<PackedCell>(),
        align_of::<PackedCell>()
    );
    println!(
        "size type=arc_str bytes={} align={}",
        size_of::<Arc<str>>(),
        align_of::<Arc<str>>()
    );
    println!(
        "size type=glyph_slice bytes={} align={}",
        size_of::<GlyphSlice>(),
        align_of::<GlyphSlice>()
    );
    println!(
        "size type=hashmap_header bytes={} align={}",
        size_of::<HashMap<Arc<str>, u32>>(),
        align_of::<HashMap<Arc<str>, u32>>()
    );

    for workload in &workloads {
        let scalar = CurrentScalarSurface::paint(workload);
        let arena = CompactArenaSurface::paint(workload);
        let interned = CompactInternedSurface::paint(workload);
        let scalar_mirror_equivalent = workload.complex_occurrences == 0;
        println!(
            "workload name={} graphemes={} output_bytes={} scalar_cells={} grapheme_cells={} complex_occurrences={} distinct_complex={} scalar_mirror_equivalent={scalar_mirror_equivalent}",
            workload.name,
            workload.logical_graphemes(),
            workload.output_bytes,
            scalar.cell_count(),
            arena.cell_count(),
            workload.complex_occurrences,
            interned.arena_entries(),
        );

        report_surface::<CurrentScalarSurface>(allocator, workload);
        report_surface::<CompactArenaSurface>(allocator, workload);
        report_surface::<CompactInternedSurface>(allocator, workload);
    }
}
