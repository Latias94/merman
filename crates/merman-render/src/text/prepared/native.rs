//! Optional built-in font shaping; contract and host response validation stay in the parent.

use super::*;
use rustybuzz::{
    Direction as BuzzDirection, Feature, Language, Script as BuzzScript, UnicodeBuffer, Variation,
};
use std::str::FromStr;
use unicode_script::{Script as UnicodeScript, UnicodeScript as _};

impl TextLayoutDirection {
    fn to_rustybuzz(self) -> Option<BuzzDirection> {
        match self {
            Self::Auto => None,
            Self::LeftToRight => Some(BuzzDirection::LeftToRight),
            Self::RightToLeft => Some(BuzzDirection::RightToLeft),
        }
    }
}

pub(super) fn parse_features(
    features: impl IntoIterator<Item = impl Into<String>>,
) -> Result<Arc<[Arc<str>]>, TextLayoutError> {
    let mut values = Vec::new();
    for feature in features {
        let feature = feature.into();
        if feature.trim().is_empty() || feature.len() > 64 {
            return Err(TextLayoutError::InvalidRequest("features"));
        }
        Feature::from_str(&feature).map_err(|_| TextLayoutError::InvalidFeature)?;
        values.push(Arc::from(feature));
    }
    if values.len() > 64 {
        return Err(TextLayoutError::LimitExceeded("features"));
    }
    Ok(values.into())
}

pub(super) fn parse_variations(
    variations: impl IntoIterator<Item = impl Into<String>>,
) -> Result<Arc<[Arc<str>]>, TextLayoutError> {
    let mut values = Vec::new();
    for variation in variations {
        let variation = variation.into();
        if variation.trim().is_empty() || variation.len() > 64 {
            return Err(TextLayoutError::InvalidRequest("variations"));
        }
        Variation::from_str(&variation).map_err(|_| TextLayoutError::InvalidVariation)?;
        values.push(Arc::from(variation));
    }
    if values.len() > 64 {
        return Err(TextLayoutError::LimitExceeded("variations"));
    }
    Ok(values.into())
}

/// The built-in, pure-Rust shaping backend used by custom font catalogs.
#[derive(Debug, Clone)]
pub struct NativeTextLayoutBackend {
    identity: TextLayoutBackendIdentity,
}

impl Default for NativeTextLayoutBackend {
    fn default() -> Self {
        Self {
            identity: TextLayoutBackendIdentity::new("merman.native-rustybuzz", "contract-1")
                .expect("static native text backend identity is valid"),
        }
    }
}

impl NativeTextLayoutBackend {
    pub fn new(identity: TextLayoutBackendIdentity) -> Self {
        Self { identity }
    }

    pub(crate) fn prepare(
        &self,
        request: &PrepareCatalogRequest,
    ) -> Result<PreparedTextLayout, TextLayoutError> {
        let response = <Self as TextLayoutBackend>::prepare_catalog(self, request)?;
        PreparedTextLayout::admit_native_backend_response(
            request,
            self.identity(),
            self.capabilities(),
            response,
        )
    }

    #[cfg(feature = "fuzzing")]
    #[doc(hidden)]
    pub(super) fn prepare_text_probe(
        &self,
        catalog_request: &PrepareCatalogRequest,
        text_request: &PrepareTextRequest,
    ) -> Result<PreparedTextFuzzProbe, TextLayoutError> {
        let prepared = self.prepare(catalog_request)?.prepare_text(text_request)?;
        Ok(PreparedTextFuzzProbe {
            metrics: prepared.metrics(),
            raw_width_px: prepared.raw_width_px(),
            bbox_width_px: prepared.bbox_width_px(),
            bbox_height_px: prepared.bbox_height_px(),
            vertical_extents: prepared.vertical_extents(),
        })
    }
}

impl TextLayoutBackend for NativeTextLayoutBackend {
    fn identity(&self) -> &TextLayoutBackendIdentity {
        &self.identity
    }

    fn capabilities(&self) -> TextLayoutCapabilities {
        TextLayoutCapabilities::native()
    }

    fn prepare_catalog(
        &self,
        request: &PrepareCatalogRequest,
    ) -> Result<PreparedTextLayoutResponse, TextLayoutError> {
        if request.catalog().faces().is_empty() {
            return Err(TextLayoutError::NoUsableFace);
        }
        if !request.font_source_policy().contains(FontSource::Embedded) {
            return Err(TextLayoutError::FontSourceUnavailable);
        }
        let session_token = TextLayoutSessionToken::derive(
            request.catalog_fingerprint,
            &self.identity,
            FontSource::Embedded,
        );
        let native = Arc::new(NativeCatalogTextMeasurer::new(
            request,
            FontSource::Embedded,
        )?);
        let face_evidence =
            TextLayoutFaceEvidence::new(native.faces.iter().map(|face| face.metadata.clone()))?;
        let session: Arc<dyn PreparedTextBackendSession> = native.clone();
        PreparedTextLayoutResponse::new(
            request.catalog_fingerprint,
            request.contract_version,
            self.identity.clone(),
            self.capabilities(),
            FontSource::Embedded,
            face_evidence,
            session_token,
            session,
        )
        .map(|response| response.with_native_session(native))
    }
}

#[derive(Clone)]
struct PreparedFace {
    data: Arc<[u8]>,
    face_index: u32,
    metadata: FontFaceMetadata,
    family_key: String,
}

pub(super) struct NativeCatalogTextMeasurer {
    faces: Vec<PreparedFace>,
    catalog: FontCatalog,
    font_source: FontSource,
    // These fields are retained only for the legacy TextMeasurer compatibility adapter. New
    // custom-font callers use PrepareTextRequest, which carries all shaping settings per label.
    direction: TextLayoutDirection,
    script: Option<BuzzScript>,
    language: Option<Language>,
    features: Vec<Feature>,
    variations: Vec<Variation>,
}

#[derive(Clone)]
struct FaceSelector {
    resolved_families: Vec<String>,
    requested_weight: u16,
    requested_style: String,
}

struct CompiledStructuredTextRequest<'a> {
    backend_request: &'a PreparedTextBackendRequest,
    features: Vec<Feature>,
    variations: Vec<Variation>,
    requested_script: Option<BuzzScript>,
    language: Option<Language>,
    wrapping: CompiledStructuredTextStyle,
    metrics: CompiledStructuredTextStyle,
}

#[derive(Clone)]
struct CompiledStructuredTextStyle {
    candidate_faces: Arc<[usize]>,
    coverage_lane: usize,
}

const STRUCTURED_COVERAGE_UNCHECKED: u32 = u32::MAX;
const STRUCTURED_COVERAGE_MISSING: u32 = u32::MAX - 1;

struct StructuredCoverageCache {
    entries: Vec<[u32; STRUCTURED_COVERAGE_CACHE_LANES]>,
}

impl StructuredCoverageCache {
    fn new(span_count: usize) -> Result<Self, TextLayoutError> {
        let entry_count = span_count
            .checked_mul(STRUCTURED_COVERAGE_CACHE_LANES)
            .ok_or(TextLayoutError::LimitExceeded("coverage_cache"))?;
        if entry_count > MAX_STRUCTURED_COVERAGE_CACHE_ENTRIES {
            return Err(TextLayoutError::LimitExceeded("coverage_cache"));
        }
        Ok(Self {
            entries: vec![
                [STRUCTURED_COVERAGE_UNCHECKED; STRUCTURED_COVERAGE_CACHE_LANES];
                span_count
            ],
        })
    }

    fn get(
        &self,
        span_index: usize,
        lane: usize,
    ) -> Result<Option<Option<usize>>, TextLayoutError> {
        let value = *self
            .entries
            .get(span_index)
            .and_then(|entry| entry.get(lane))
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        Ok(match value {
            STRUCTURED_COVERAGE_UNCHECKED => None,
            STRUCTURED_COVERAGE_MISSING => Some(None),
            face_index => Some(Some(face_index as usize)),
        })
    }

    fn insert(
        &mut self,
        span_index: usize,
        lane: usize,
        face_index: Option<usize>,
    ) -> Result<(), TextLayoutError> {
        let slot = self
            .entries
            .get_mut(span_index)
            .and_then(|entry| entry.get_mut(lane))
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        if *slot != STRUCTURED_COVERAGE_UNCHECKED {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        *slot = match face_index {
            Some(face_index) => u32::try_from(face_index)
                .ok()
                .filter(|face_index| *face_index < STRUCTURED_COVERAGE_MISSING)
                .ok_or(TextLayoutError::InvalidPreparedText)?,
            None => STRUCTURED_COVERAGE_MISSING,
        };
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct StructuredShapingWork {
    candidate_compilation_units: usize,
    coverage_input_bytes: usize,
    face_inspections: usize,
    coverage_cache_insertions: usize,
    coverage_cache_slots: usize,
    source_line_scan_bytes: usize,
    source_line_visits: usize,
    wrapped_line_emissions: usize,
    wrapping_input_bytes: usize,
    metrics_input_bytes: usize,
    wrapping_span_visits: usize,
    metrics_span_visits: usize,
    wrapping_line_ranges: usize,
    metrics_line_ranges: usize,
    glyph_visits: usize,
    cluster_visits: usize,
    wrap_boundary_visits: usize,
}

#[derive(Debug, Clone, Copy)]
enum StructuredShapingPass {
    Coverage,
    Wrapping,
    Metrics,
}

impl StructuredShapingWork {
    fn total_units(self) -> Result<usize, TextLayoutError> {
        [
            self.candidate_compilation_units,
            self.coverage_input_bytes,
            self.face_inspections,
            self.coverage_cache_insertions,
            self.coverage_cache_slots,
            self.source_line_scan_bytes,
            self.source_line_visits,
            self.wrapped_line_emissions,
            self.wrapping_input_bytes,
            self.metrics_input_bytes,
            self.wrapping_span_visits,
            self.metrics_span_visits,
            self.wrapping_line_ranges,
            self.metrics_line_ranges,
            self.glyph_visits,
            self.cluster_visits,
            self.wrap_boundary_visits,
        ]
        .into_iter()
        .try_fold(0usize, |total, units| total.checked_add(units))
        .ok_or(TextLayoutError::LimitExceeded("operation_work"))
    }
}

struct StructuredShapingBudget<'a> {
    work: StructuredShapingWork,
    work_meter: Option<&'a OperationWorkMeter>,
}

impl<'a> StructuredShapingBudget<'a> {
    fn new(work_meter: Option<&'a OperationWorkMeter>) -> Self {
        Self {
            work: StructuredShapingWork::default(),
            work_meter,
        }
    }

    fn charge(&self, units: usize) -> Result<(), TextLayoutError> {
        if let Some(work_meter) = self.work_meter {
            work_meter
                .charge(units)
                .map_err(|_| TextLayoutError::LimitExceeded("operation_work"))?;
        }
        Ok(())
    }

    fn record_candidate_compilation(&mut self, units: usize) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .candidate_compilation_units
            .checked_add(units)
            .ok_or(TextLayoutError::LimitExceeded("candidate_compilation"))?;
        self.charge(units)?;
        self.work.candidate_compilation_units = next;
        Ok(())
    }

    fn record(
        &mut self,
        pass: StructuredShapingPass,
        input_bytes: usize,
    ) -> Result<(), TextLayoutError> {
        let (current, limit) = match pass {
            StructuredShapingPass::Coverage => (
                self.work.coverage_input_bytes,
                MAX_STRUCTURED_COVERAGE_INPUT_BYTES,
            ),
            StructuredShapingPass::Wrapping => {
                (self.work.wrapping_input_bytes, MAX_TEXT_PROJECTION_BYTES)
            }
            StructuredShapingPass::Metrics => {
                (self.work.metrics_input_bytes, MAX_TEXT_PROJECTION_BYTES)
            }
        };
        let next = current
            .checked_add(input_bytes)
            .ok_or(TextLayoutError::LimitExceeded("shaping_work"))?;
        if next > limit {
            return Err(TextLayoutError::LimitExceeded("shaping_work"));
        }
        self.charge(input_bytes)?;
        match pass {
            StructuredShapingPass::Coverage => self.work.coverage_input_bytes = next,
            StructuredShapingPass::Wrapping => self.work.wrapping_input_bytes = next,
            StructuredShapingPass::Metrics => self.work.metrics_input_bytes = next,
        }
        Ok(())
    }

    fn record_span_visit(&mut self, pass: StructuredShapingPass) -> Result<(), TextLayoutError> {
        let current = match pass {
            StructuredShapingPass::Wrapping => self.work.wrapping_span_visits,
            StructuredShapingPass::Metrics => self.work.metrics_span_visits,
            StructuredShapingPass::Coverage => return Ok(()),
        };
        let next = current
            .checked_add(1)
            .ok_or(TextLayoutError::LimitExceeded("span_work"))?;
        if next > MAX_STRUCTURED_SPAN_VISITS {
            return Err(TextLayoutError::LimitExceeded("span_work"));
        }
        self.charge(1)?;
        match pass {
            StructuredShapingPass::Wrapping => self.work.wrapping_span_visits = next,
            StructuredShapingPass::Metrics => self.work.metrics_span_visits = next,
            StructuredShapingPass::Coverage => {}
        }
        Ok(())
    }

    fn record_face_inspection(&mut self) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .face_inspections
            .checked_add(1)
            .ok_or(TextLayoutError::LimitExceeded("coverage_work"))?;
        if next > MAX_STRUCTURED_FACE_INSPECTIONS {
            return Err(TextLayoutError::LimitExceeded("coverage_work"));
        }
        self.charge(1)?;
        self.work.face_inspections = next;
        Ok(())
    }

    fn record_coverage_cache_insertion(&mut self) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .coverage_cache_insertions
            .checked_add(1)
            .ok_or(TextLayoutError::LimitExceeded("coverage_cache"))?;
        if next > MAX_STRUCTURED_COVERAGE_CACHE_ENTRIES {
            return Err(TextLayoutError::LimitExceeded("coverage_cache"));
        }
        self.charge(1)?;
        self.work.coverage_cache_insertions = next;
        Ok(())
    }

    fn record_coverage_cache_slots(&mut self, slots: usize) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .coverage_cache_slots
            .checked_add(slots)
            .ok_or(TextLayoutError::LimitExceeded("coverage_cache"))?;
        if next > MAX_STRUCTURED_COVERAGE_CACHE_ENTRIES {
            return Err(TextLayoutError::LimitExceeded("coverage_cache"));
        }
        self.charge(slots)?;
        self.work.coverage_cache_slots = next;
        Ok(())
    }

    fn record_source_line_scan(&mut self, bytes: usize) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .source_line_scan_bytes
            .checked_add(bytes)
            .ok_or(TextLayoutError::LimitExceeded("line_work"))?;
        if next > MAX_TEXT_PROJECTION_BYTES {
            return Err(TextLayoutError::LimitExceeded("line_work"));
        }
        self.charge(bytes)?;
        self.work.source_line_scan_bytes = next;
        Ok(())
    }

    fn record_source_line_visit(&mut self) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .source_line_visits
            .checked_add(1)
            .ok_or(TextLayoutError::LimitExceeded("line_work"))?;
        if next > MAX_PREPARED_TEXT_LINES {
            return Err(TextLayoutError::LimitExceeded("lines"));
        }
        self.charge(1)?;
        self.work.source_line_visits = next;
        Ok(())
    }

    fn record_wrapped_line_emission(&mut self) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .wrapped_line_emissions
            .checked_add(1)
            .ok_or(TextLayoutError::LimitExceeded("line_work"))?;
        if next > MAX_PREPARED_TEXT_LINES {
            return Err(TextLayoutError::LimitExceeded("lines"));
        }
        self.charge(1)?;
        self.work.wrapped_line_emissions = next;
        Ok(())
    }

    fn record_glyph_visits(&mut self, visits: usize) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .glyph_visits
            .checked_add(visits)
            .ok_or(TextLayoutError::LimitExceeded("glyph_work"))?;
        if next > MAX_STRUCTURED_GLYPH_VISITS {
            return Err(TextLayoutError::LimitExceeded("glyph_work"));
        }
        self.charge(visits)?;
        self.work.glyph_visits = next;
        Ok(())
    }

    fn record_cluster_visits(&mut self, visits: usize) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .cluster_visits
            .checked_add(visits)
            .ok_or(TextLayoutError::LimitExceeded("cluster_work"))?;
        if next > MAX_STRUCTURED_CLUSTER_VISITS {
            return Err(TextLayoutError::LimitExceeded("cluster_work"));
        }
        self.charge(visits)?;
        self.work.cluster_visits = next;
        Ok(())
    }

    fn record_wrap_boundary_visits(&mut self, visits: usize) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .wrap_boundary_visits
            .checked_add(visits)
            .ok_or(TextLayoutError::LimitExceeded("wrap_work"))?;
        if next > MAX_STRUCTURED_WRAP_BOUNDARY_VISITS {
            return Err(TextLayoutError::LimitExceeded("wrap_work"));
        }
        self.charge(visits)?;
        self.work.wrap_boundary_visits = next;
        Ok(())
    }

    fn record_line_range(&mut self, pass: StructuredShapingPass) -> Result<(), TextLayoutError> {
        let current = match pass {
            StructuredShapingPass::Wrapping => self.work.wrapping_line_ranges,
            StructuredShapingPass::Metrics => self.work.metrics_line_ranges,
            StructuredShapingPass::Coverage => return Ok(()),
        };
        let next = current
            .checked_add(1)
            .ok_or(TextLayoutError::LimitExceeded("span_work"))?;
        if next > MAX_PREPARED_TEXT_LINES {
            return Err(TextLayoutError::LimitExceeded("span_work"));
        }
        self.charge(1)?;
        match pass {
            StructuredShapingPass::Wrapping => self.work.wrapping_line_ranges = next,
            StructuredShapingPass::Metrics => self.work.metrics_line_ranges = next,
            StructuredShapingPass::Coverage => {}
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct ProjectionSpanCursor {
    next_index: usize,
    previous_line_end: usize,
}

impl ProjectionSpanCursor {
    fn range_for_line(
        &mut self,
        projection: &TextProjection,
        line_range: TextByteRange,
        pass: StructuredShapingPass,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<std::ops::Range<usize>, TextLayoutError> {
        budget.record_line_range(pass)?;
        let start = line_range.start();
        let end = line_range.end();
        if start > end || start < self.previous_line_end || end > projection.visible().len() {
            return Err(TextLayoutError::InvalidPreparedText);
        }

        let spans = projection.spans();
        while let Some(span) = spans.get(self.next_index) {
            budget.record_span_visit(pass)?;
            if span.visible().end() <= start {
                self.next_index = self.next_index.saturating_add(1);
                continue;
            }
            break;
        }

        let first = self.next_index;
        let mut atom_cursor = start;
        while let Some(span) = spans.get(self.next_index) {
            budget.record_span_visit(pass)?;
            let visible = span.visible();
            if visible.start() >= end {
                break;
            }
            if visible.start() < start
                || visible.end() > end
                || visible.start() != atom_cursor
                || visible.end() < visible.start()
            {
                return Err(TextLayoutError::InvalidPreparedText);
            }
            atom_cursor = visible.end();
            self.next_index = self.next_index.saturating_add(1);
        }
        if atom_cursor != end {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        self.previous_line_end = end;
        Ok(first..self.next_index)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ShapedLineMetrics {
    advance: f64,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
    has_bounds: bool,
}

struct ShapedStructuredLine {
    metrics: ShapedLineMetrics,
    clusters: Vec<ShapedClusterMetric>,
    span_range: std::ops::Range<usize>,
}

struct ShapedStructuredRun {
    metrics: ShapedLineMetrics,
    clusters: Vec<ShapedClusterMetric>,
}

#[derive(Debug, Clone, Copy)]
struct ShapedClusterMetric {
    range: TextByteRange,
    advance: f64,
    min_x: f64,
    max_x: f64,
    has_bounds: bool,
}

impl ShapedClusterMetric {
    fn include_glyph(&mut self, advance: f64, bounds: Option<(f64, f64)>) {
        self.advance += advance;
        let Some((min_x, max_x)) = bounds else {
            return;
        };
        if self.has_bounds {
            self.min_x = self.min_x.min(min_x);
            self.max_x = self.max_x.max(max_x);
        } else {
            self.min_x = min_x;
            self.max_x = max_x;
            self.has_bounds = true;
        }
    }

    fn offset(mut self, byte_offset: usize, x_offset: f64) -> Self {
        self.range = TextByteRange::new(
            self.range.start().saturating_add(byte_offset),
            self.range.end().saturating_add(byte_offset),
        );
        if self.has_bounds {
            self.min_x += x_offset;
            self.max_x += x_offset;
        }
        self
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct WrapMeasure {
    advance: f64,
    min_x: f64,
    max_x: f64,
    grapheme_count: usize,
    ascii_space_count: usize,
    has_bounds: bool,
}

impl WrapMeasure {
    fn include(&mut self, other: Self) {
        self.advance += other.advance;
        self.grapheme_count = self.grapheme_count.saturating_add(other.grapheme_count);
        self.ascii_space_count = self
            .ascii_space_count
            .saturating_add(other.ascii_space_count);
        if !other.has_bounds {
            return;
        }
        if self.has_bounds {
            self.min_x = self.min_x.min(other.min_x);
            self.max_x = self.max_x.max(other.max_x);
        } else {
            self.min_x = other.min_x;
            self.max_x = other.max_x;
            self.has_bounds = true;
        }
    }

    fn width(self, typography: &ThemeTextStyle) -> f64 {
        let letter_spacing = f64::from(typography.letter_spacing_px())
            * self.grapheme_count.saturating_sub(1) as f64;
        let word_spacing = f64::from(typography.word_spacing_px()) * self.ascii_space_count as f64;
        let advance = (self.advance + letter_spacing + word_spacing).abs();
        let ink = if self.has_bounds {
            (self.max_x - self.min_x).max(0.0)
        } else {
            0.0
        };
        advance.max(ink)
    }
}

#[derive(Debug, Clone, Copy)]
struct WrapAtomMetric {
    range: TextByteRange,
    measure: WrapMeasure,
    whitespace: bool,
}

#[derive(Debug, Clone)]
struct ShapedWrapLine {
    atoms: Vec<WrapAtomMetric>,
    measure: WrapMeasure,
}

#[derive(Debug, Clone, Copy)]
struct WrapSegment {
    atom_start: usize,
    atom_end: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct WrapLineState {
    atom_start: Option<usize>,
    atom_end: usize,
    content_end: usize,
    content: WrapMeasure,
    trailing: WrapMeasure,
}

impl ShapedLineMetrics {
    fn include_run(&mut self, run: Self, offset_x: f64) {
        self.advance += run.advance;
        if !run.has_bounds {
            return;
        }
        let min_x = run.min_x + offset_x;
        let max_x = run.max_x + offset_x;
        if !self.has_bounds {
            self.min_x = min_x;
            self.max_x = max_x;
            self.min_y = run.min_y;
            self.max_y = run.max_y;
            self.has_bounds = true;
        } else {
            self.min_x = self.min_x.min(min_x);
            self.max_x = self.max_x.max(max_x);
            self.min_y = self.min_y.min(run.min_y);
            self.max_y = self.max_y.max(run.max_y);
        }
    }

    fn bbox_width(self) -> f64 {
        if self.has_bounds {
            (self.max_x - self.min_x).max(0.0)
        } else {
            0.0
        }
    }

    fn bbox_height(self) -> f64 {
        if self.has_bounds {
            (self.max_y - self.min_y).max(0.0)
        } else {
            0.0
        }
    }

    fn vertical_extents(self) -> Result<PreparedTextVerticalExtents, TextLayoutError> {
        if self.has_bounds {
            // Font coordinates are y-up around the alphabetic baseline. SVG uses y-down, so the
            // upper outline edge becomes `-max_y` and the lower edge becomes `-min_y`.
            PreparedTextVerticalExtents::new(-self.max_y, -self.min_y)
        } else {
            // Empty, whitespace-only, and default-ignorable lines have no ink. Retaining their
            // baseline as a zero-height point lets multiline aggregation preserve line advance.
            PreparedTextVerticalExtents::new(0.0, 0.0)
        }
    }

    fn bbox_x(self) -> (f64, f64) {
        if self.has_bounds {
            ((-self.min_x).max(0.0), self.max_x.max(0.0))
        } else {
            (0.0, 0.0)
        }
    }

    fn layout_width(self) -> f64 {
        self.advance.abs().max(self.bbox_width())
    }
}

impl WrapLineState {
    const fn is_empty(self) -> bool {
        self.atom_start.is_none()
    }

    fn has_visible_content(self) -> bool {
        self.atom_start
            .is_some_and(|start| self.content_end > start)
    }

    fn append_atom(&mut self, atom_index: usize, atom: WrapAtomMetric) {
        self.atom_start.get_or_insert(atom_index);
        self.atom_end = atom_index.saturating_add(1);
        if atom.whitespace {
            self.trailing.include(atom.measure);
        } else {
            self.content.include(self.trailing);
            self.trailing = WrapMeasure::default();
            self.content.include(atom.measure);
            self.content_end = self.atom_end;
        }
    }

    fn append_segment(&mut self, segment: WrapSegment, atoms: &[WrapAtomMetric]) {
        for (atom_index, atom) in atoms
            .iter()
            .copied()
            .enumerate()
            .take(segment.atom_end)
            .skip(segment.atom_start)
        {
            self.append_atom(atom_index, atom);
        }
    }

    fn width(self, typography: &ThemeTextStyle) -> f64 {
        self.content.width(typography)
    }

    fn trimmed_range(self, atoms: &[WrapAtomMetric]) -> Result<TextByteRange, TextLayoutError> {
        let Some(start) = self.atom_start else {
            return Ok(TextByteRange::new(0, 0));
        };
        let local_start = atoms
            .get(start)
            .ok_or(TextLayoutError::InvalidPreparedText)?
            .range
            .start();
        let local_end = if self.has_visible_content() {
            atoms
                .get(self.content_end.saturating_sub(1))
                .ok_or(TextLayoutError::InvalidPreparedText)?
                .range
                .end()
        } else {
            local_start
        };
        Ok(TextByteRange::new(local_start, local_end))
    }
}

fn coalesce_wrap_atoms(
    text: &str,
    visible_start: usize,
    projection: &TextProjection,
    span_range: std::ops::Range<usize>,
    clusters: &[ShapedClusterMetric],
    budget: &mut StructuredShapingBudget<'_>,
) -> Result<Vec<WrapAtomMetric>, TextLayoutError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let mut grapheme_boundaries = text
        .grapheme_indices(true)
        .map(|(offset, _)| offset)
        .skip(1)
        .chain(std::iter::once(text.len()))
        .peekable();
    let mut projection_boundaries = projection.spans()[span_range]
        .iter()
        .filter(|span| span.visible().start() < span.visible().end())
        .map(|span| span.visible().end())
        .peekable();
    let mut atoms = Vec::new();
    let mut current_start = 0;
    let mut current_end = 0;
    let mut current_grapheme_count: usize = 0;
    let mut current_measure = WrapMeasure::default();
    for cluster in clusters {
        budget.record_cluster_visits(1)?;
        if cluster.range.start() != current_end
            || cluster.range.end() <= cluster.range.start()
            || cluster.range.end() > text.len()
        {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        current_end = cluster.range.end();
        current_measure.include(WrapMeasure {
            advance: cluster.advance,
            min_x: cluster.min_x,
            max_x: cluster.max_x,
            grapheme_count: 0,
            ascii_space_count: 0,
            has_bounds: cluster.has_bounds,
        });
        let global_end = visible_start
            .checked_add(current_end)
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        let mut grapheme_boundary = false;
        while grapheme_boundaries
            .peek()
            .is_some_and(|boundary| *boundary <= current_end)
        {
            let boundary = grapheme_boundaries
                .next()
                .expect("peeked grapheme boundary is present");
            budget.record_wrap_boundary_visits(1)?;
            current_grapheme_count = current_grapheme_count.saturating_add(1);
            if boundary == current_end {
                grapheme_boundary = true;
                break;
            }
        }
        while projection_boundaries
            .peek()
            .is_some_and(|boundary| *boundary < global_end)
        {
            projection_boundaries.next();
            budget.record_wrap_boundary_visits(1)?;
        }
        let projection_boundary = projection_boundaries
            .peek()
            .is_some_and(|boundary| *boundary == global_end);
        if projection_boundary {
            projection_boundaries.next();
            budget.record_wrap_boundary_visits(1)?;
        }
        if !grapheme_boundary || !projection_boundary {
            continue;
        }

        let atom_text = text
            .get(current_start..current_end)
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        budget.record_wrap_boundary_visits(atom_text.len())?;
        current_measure.grapheme_count = current_grapheme_count;
        current_measure.ascii_space_count = atom_text.bytes().filter(|byte| *byte == b' ').count();
        if !current_measure.advance.is_finite()
            || current_measure.advance.abs() > MAX_PREPARED_TEXT_GEOMETRY_PX
            || (current_measure.has_bounds
                && (!current_measure.min_x.is_finite()
                    || !current_measure.max_x.is_finite()
                    || current_measure.min_x.abs() > MAX_PREPARED_TEXT_GEOMETRY_PX
                    || current_measure.max_x.abs() > MAX_PREPARED_TEXT_GEOMETRY_PX))
        {
            return Err(TextLayoutError::LimitExceeded("geometry"));
        }
        atoms.push(WrapAtomMetric {
            range: TextByteRange::new(current_start, current_end),
            measure: current_measure,
            whitespace: atom_text.chars().all(char::is_whitespace),
        });
        current_start = current_end;
        current_grapheme_count = 0;
        current_measure = WrapMeasure::default();
    }
    if current_end != text.len() || current_start != text.len() {
        return Err(TextLayoutError::InvalidPreparedText);
    }
    Ok(atoms)
}

fn svg_wrap_segment_ranges(text: &str) -> Vec<TextByteRange> {
    let mut ranges = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let end = if text.as_bytes()[cursor] == b' ' {
            cursor + 1
        } else {
            text.as_bytes()[cursor..]
                .iter()
                .position(|byte| *byte == b' ')
                .map(|offset| cursor + offset)
                .unwrap_or(text.len())
        };
        ranges.push(TextByteRange::new(cursor, end));
        cursor = end;
    }
    ranges
}

fn html_wrap_segment_ranges(text: &str) -> Vec<TextByteRange> {
    let mut cursor: usize = 0;
    crate::text::line_break::html_break_spaces_segments(text)
        .into_iter()
        .map(|segment| {
            let start = cursor;
            cursor = cursor.saturating_add(segment.len());
            TextByteRange::new(start, cursor)
        })
        .collect()
}

fn align_wrap_segments(
    atoms: &[WrapAtomMetric],
    ranges: &[TextByteRange],
    budget: &mut StructuredShapingBudget<'_>,
) -> Result<Vec<WrapSegment>, TextLayoutError> {
    let mut segments = Vec::new();
    let mut atom_cursor = 0;
    let mut previous_end = 0;
    for range in ranges {
        budget.record_wrap_boundary_visits(1)?;
        if range.start() < previous_end || range.end() < range.start() {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        previous_end = range.end();
        while atom_cursor < atoms.len() && atoms[atom_cursor].range.end() <= range.start() {
            budget.record_wrap_boundary_visits(1)?;
            atom_cursor += 1;
        }
        let atom_start = atom_cursor;
        while atom_cursor < atoms.len() && atoms[atom_cursor].range.start() < range.end() {
            budget.record_wrap_boundary_visits(1)?;
            atom_cursor += 1;
        }
        if atom_start < atom_cursor {
            segments.push(WrapSegment {
                atom_start,
                atom_end: atom_cursor,
            });
        }
    }
    if atom_cursor != atoms.len() {
        return Err(TextLayoutError::InvalidPreparedText);
    }
    Ok(segments)
}

fn segment_is_whitespace(
    segment: WrapSegment,
    atoms: &[WrapAtomMetric],
    budget: &mut StructuredShapingBudget<'_>,
) -> Result<bool, TextLayoutError> {
    budget.record_wrap_boundary_visits(segment.atom_end.saturating_sub(segment.atom_start))?;
    Ok(atoms[segment.atom_start..segment.atom_end]
        .iter()
        .all(|atom| atom.whitespace))
}

fn push_wrapped_state(
    wrapped: &mut Vec<ProjectedVisibleLine>,
    source: ProjectedVisibleLine,
    state: WrapLineState,
    atoms: &[WrapAtomMetric],
    budget: &mut StructuredShapingBudget<'_>,
) -> Result<(), TextLayoutError> {
    let line = ProjectedVisibleLine::from_local_range(source, state.trimmed_range(atoms)?)?;
    push_projected_visible_line(wrapped, line, budget)?;
    Ok(())
}

fn push_projected_visible_line(
    wrapped: &mut Vec<ProjectedVisibleLine>,
    line: ProjectedVisibleLine,
    budget: &mut StructuredShapingBudget<'_>,
) -> Result<(), TextLayoutError> {
    budget.record_wrapped_line_emission()?;
    wrapped.push(line);
    Ok(())
}

impl NativeCatalogTextMeasurer {
    fn new(
        request: &PrepareCatalogRequest,
        font_source: FontSource,
    ) -> Result<Self, TextLayoutError> {
        let mut faces = Vec::new();
        for metadata in request.catalog().faces() {
            let Some(asset) = request
                .catalog()
                .assets()
                .iter()
                .find(|asset| asset.id() == metadata.asset_id())
            else {
                continue;
            };
            if rustybuzz::Face::from_slice(asset.canonical_bytes(), metadata.face_index()).is_none()
            {
                continue;
            }
            faces.push(PreparedFace {
                data: asset.canonical_data(),
                face_index: metadata.face_index(),
                metadata: metadata.clone(),
                family_key: normalize_family(metadata.family_name()),
            });
        }
        if faces.is_empty() {
            return Err(TextLayoutError::NoUsableFace);
        }
        Ok(Self {
            faces,
            catalog: request.catalog().clone(),
            font_source,
            direction: TextLayoutDirection::Auto,
            script: None,
            language: None,
            features: Vec::new(),
            variations: Vec::new(),
        })
    }

    fn selector(&self, style: &TextStyle) -> FaceSelector {
        let resolved_families = style
            .font_family
            .as_deref()
            .map(|family| {
                family
                    .split(',')
                    .map(|value| self.resolve_family(&normalize_family(value.trim_matches('"'))))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let requested_weight = style
            .font_weight
            .as_deref()
            .map(parse_font_weight)
            .unwrap_or(400);
        let requested_style = style
            .font_style
            .as_deref()
            .map(normalize_family)
            .unwrap_or_else(|| "normal".to_string());
        FaceSelector {
            resolved_families,
            requested_weight,
            requested_style,
        }
    }

    fn structured_selector(
        &self,
        typography: &ThemeTextStyle,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<FaceSelector, TextLayoutError> {
        let families = typography.font_stack().families();
        budget.record_candidate_compilation(families.len())?;
        let mut resolved_families = Vec::with_capacity(families.len());
        for family in families {
            budget.record_candidate_compilation(family.len())?;
            resolved_families.push(
                self.catalog
                    .canonical_named_family_name(family)
                    .map(normalize_family)
                    .unwrap_or_else(|| normalize_family(family)),
            );
        }
        let family_face_comparisons = resolved_families
            .len()
            .checked_mul(self.faces.len())
            .ok_or(TextLayoutError::LimitExceeded("candidate_compilation"))?;
        budget.record_candidate_compilation(family_face_comparisons)?;
        let has_catalog_family = resolved_families
            .iter()
            .any(|family| self.faces.iter().any(|face| &face.family_key == family));
        if !has_catalog_family {
            return Err(TextLayoutError::FontFamilyUnavailable);
        }
        budget.record_candidate_compilation(1)?;
        Ok(FaceSelector {
            resolved_families,
            requested_weight: typography.font_weight(),
            requested_style: typography.font_style().id().to_string(),
        })
    }

    fn compile_structured_request<'a>(
        &self,
        backend_request: &'a PreparedTextBackendRequest,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<CompiledStructuredTextRequest<'a>, TextLayoutError> {
        let request = backend_request.request();
        let features = request
            .features()
            .map(|feature| {
                budget.record_candidate_compilation(1usize.saturating_add(feature.len()))?;
                Feature::from_str(feature).map_err(|_| TextLayoutError::InvalidFeature)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let variations = request
            .variations()
            .map(|variation| {
                budget.record_candidate_compilation(1usize.saturating_add(variation.len()))?;
                Variation::from_str(variation).map_err(|_| TextLayoutError::InvalidVariation)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let requested_script = request
            .script()
            .map(|value| {
                budget.record_candidate_compilation(1usize.saturating_add(value.len()))?;
                BuzzScript::from_str(value).map_err(|_| TextLayoutError::InvalidRequest("script"))
            })
            .transpose()?;
        let language = request
            .language()
            .map(|value| {
                budget.record_candidate_compilation(1usize.saturating_add(value.len()))?;
                Language::from_str(value).map_err(|_| TextLayoutError::InvalidRequest("language"))
            })
            .transpose()?;
        let wrapping = self.compile_structured_style(request.wrapping_typography(), budget)?;
        let mut metrics = if request.metrics_typography() == request.wrapping_typography() {
            wrapping.clone()
        } else {
            self.compile_structured_style(request.metrics_typography(), budget)?
        };
        if metrics.candidate_faces != wrapping.candidate_faces {
            metrics.coverage_lane = 1;
        }
        Ok(CompiledStructuredTextRequest {
            backend_request,
            features,
            variations,
            requested_script,
            language,
            wrapping,
            metrics,
        })
    }

    fn compile_structured_style(
        &self,
        typography: &ThemeTextStyle,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<CompiledStructuredTextStyle, TextLayoutError> {
        let selector = self.structured_selector(typography, budget)?;
        let candidate_faces = self.strict_candidate_face_indices(&selector, budget)?;
        if candidate_faces.is_empty() {
            return Err(TextLayoutError::FontFamilyUnavailable);
        }
        Ok(CompiledStructuredTextStyle {
            candidate_faces: candidate_faces.into(),
            coverage_lane: 0,
        })
    }

    fn candidate_face_indices(&self, selector: &FaceSelector) -> Vec<usize> {
        let score = |face: &PreparedFace| {
            let family_rank = if selector.resolved_families.is_empty() {
                0
            } else {
                selector
                    .resolved_families
                    .iter()
                    .position(|family| family == &face.family_key)
                    .unwrap_or(selector.resolved_families.len() + 1)
            };
            (
                family_rank,
                usize::from(face.metadata.style().id() != selector.requested_style),
                face.metadata.weight().abs_diff(selector.requested_weight),
                face.metadata.width(),
            )
        };

        let mut candidates = self
            .faces
            .iter()
            .enumerate()
            .map(|(index, face)| (index, score(face)))
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_, score)| *score);
        if !selector.resolved_families.is_empty()
            && candidates
                .first()
                .is_some_and(|(_, score)| score.0 > selector.resolved_families.len())
        {
            return Vec::new();
        }
        candidates.into_iter().map(|(index, _)| index).collect()
    }

    fn strict_candidate_face_indices(
        &self,
        selector: &FaceSelector,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<Vec<usize>, TextLayoutError> {
        let face_count = self.faces.len();
        let family_count = selector.resolved_families.len();
        let family_comparisons = face_count
            .checked_mul(family_count)
            .ok_or(TextLayoutError::LimitExceeded("candidate_compilation"))?;
        let sort_comparisons = face_count
            .checked_mul(usize::BITS as usize - face_count.max(1).leading_zeros() as usize)
            .ok_or(TextLayoutError::LimitExceeded("candidate_compilation"))?;
        budget.record_candidate_compilation(
            family_comparisons
                .checked_add(face_count)
                .and_then(|units| units.checked_add(sort_comparisons))
                .ok_or(TextLayoutError::LimitExceeded("candidate_compilation"))?,
        )?;
        let mut candidates = self
            .faces
            .iter()
            .enumerate()
            .filter_map(|(index, face)| {
                let family_rank = selector
                    .resolved_families
                    .iter()
                    .position(|family| family == &face.family_key)?;
                Some((
                    index,
                    (
                        family_rank,
                        usize::from(face.metadata.style().id() != selector.requested_style),
                        face.metadata.weight().abs_diff(selector.requested_weight),
                        face.metadata.width().abs_diff(5),
                    ),
                ))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_, score)| *score);
        Ok(candidates.into_iter().map(|(index, _)| index).collect())
    }

    fn choose_face_index(&self, text: &str, selector: &FaceSelector) -> Option<usize> {
        let script = self.script.or_else(|| cluster_script(text));
        // Keep legacy measurement on the same cluster-shaping coverage rule as prepared text.
        // A separate scalar CMap cache cannot prove shaped coverage and grows across labels.
        self.candidate_face_indices(selector)
            .into_iter()
            .find(|index| {
                self.face_shapes_cluster(
                    *index,
                    text,
                    self.direction,
                    script,
                    self.language.clone(),
                    &self.features,
                    &self.variations,
                )
                .unwrap_or(false)
            })
    }

    fn choose_structured_face_index(
        &self,
        text: &str,
        span_index: usize,
        coverage_lane: usize,
        style: &CompiledStructuredTextStyle,
        compiled: &CompiledStructuredTextRequest<'_>,
        script: Option<BuzzScript>,
        coverage_cache: &mut StructuredCoverageCache,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<Option<usize>, TextLayoutError> {
        if let Some(face_index) = coverage_cache.get(span_index, coverage_lane)? {
            return Ok(face_index);
        }

        let mut selected = None;
        for index in style.candidate_faces.iter().copied() {
            budget.record_face_inspection()?;
            budget.record(StructuredShapingPass::Coverage, text.len())?;
            let covers = self.face_shapes_cluster(
                index,
                text,
                compiled.backend_request.request().direction(),
                script,
                compiled.language.clone(),
                &compiled.features,
                &compiled.variations,
            )?;
            if covers {
                selected = Some(index);
                break;
            }
        }
        budget.record_coverage_cache_insertion()?;
        coverage_cache.insert(span_index, coverage_lane, selected)?;
        Ok(selected)
    }

    #[allow(clippy::too_many_arguments)]
    fn face_shapes_cluster(
        &self,
        face_index: usize,
        text: &str,
        direction: TextLayoutDirection,
        script: Option<BuzzScript>,
        language: Option<Language>,
        features: &[Feature],
        variations: &[Variation],
    ) -> Result<bool, TextLayoutError> {
        let face = &self.faces[face_index];
        let mut font = rustybuzz::Face::from_slice(&face.data, face.face_index)
            .ok_or(TextLayoutError::NoUsableFace)?;
        font.set_variations(variations);
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        if let Some(direction) = direction.to_rustybuzz() {
            buffer.set_direction(direction);
        }
        if let Some(script) = script {
            buffer.set_script(script);
        }
        if let Some(language) = language {
            buffer.set_language(language);
        }
        let glyphs = rustybuzz::shape(&font, features, buffer);
        Ok(shaped_glyph_coverage_is_complete(
            text,
            glyphs.glyph_infos().iter().map(|info| info.glyph_id),
        ))
    }

    fn resolve_family<'a>(&'a self, family: &'a str) -> String {
        self.catalog
            .canonical_family_name(family)
            .map(normalize_family)
            .unwrap_or_else(|| family.to_string())
    }

    fn shape_line(&self, text: &str, style: &TextStyle) -> ShapedLineMetrics {
        if text.is_empty() {
            return ShapedLineMetrics::default();
        }
        let selector = self.selector(style);
        let mut total = ShapedLineMetrics::default();
        let mut offset_x = 0.0;
        let mut current_face = None;
        let mut current_script = None;
        let mut run = String::new();
        for cluster in text.graphemes(true) {
            let face_index = self.choose_face_index(cluster, &selector);
            let script = self
                .script
                .or_else(|| cluster_script(cluster).or(current_script));
            if (face_index != current_face || script != current_script) && !run.is_empty() {
                if let Some(face_index) = current_face {
                    let shaped = self.shape_run(face_index, current_script, &run, style);
                    total.include_run(shaped, offset_x);
                    offset_x += shaped.advance;
                }
                run.clear();
            }
            current_face = face_index;
            current_script = script;
            run.push_str(cluster);
        }
        if let Some(face_index) = current_face {
            let shaped = self.shape_run(face_index, current_script, &run, style);
            total.include_run(shaped, offset_x);
        }
        total
    }

    fn shape_structured_line_with_evidence(
        &self,
        text: &str,
        line_range: TextByteRange,
        typography: &ThemeTextStyle,
        compiled: &CompiledStructuredTextRequest<'_>,
        style: &CompiledStructuredTextStyle,
        coverage_cache: &mut StructuredCoverageCache,
        span_cursor: &mut ProjectionSpanCursor,
        budget: &mut StructuredShapingBudget<'_>,
        response_builder: &mut PreparedTextResponseBuilder,
    ) -> Result<ShapedStructuredLine, TextLayoutError> {
        self.shape_structured_line_internal(
            text,
            line_range,
            typography,
            compiled,
            style,
            StructuredShapingPass::Metrics,
            coverage_cache,
            span_cursor,
            budget,
            Some(response_builder),
        )
    }

    fn shape_structured_line_internal(
        &self,
        text: &str,
        line_range: TextByteRange,
        typography: &ThemeTextStyle,
        compiled: &CompiledStructuredTextRequest<'_>,
        style: &CompiledStructuredTextStyle,
        pass: StructuredShapingPass,
        coverage_cache: &mut StructuredCoverageCache,
        span_cursor: &mut ProjectionSpanCursor,
        budget: &mut StructuredShapingBudget<'_>,
        mut response_builder: Option<&mut PreparedTextResponseBuilder>,
    ) -> Result<ShapedStructuredLine, TextLayoutError> {
        let emit_evidence = response_builder.is_some();
        let projection = compiled.backend_request.projection();
        let line_visible_start = line_range.start();
        let line_visible_end = line_range.end();
        if line_visible_end.saturating_sub(line_visible_start) != text.len()
            || !projection.visible().is_char_boundary(line_visible_start)
            || !projection.visible().is_char_boundary(line_visible_end)
        {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        let span_range = span_cursor.range_for_line(projection, line_range, pass, budget)?;
        if text.is_empty() {
            return Ok(ShapedStructuredLine {
                metrics: ShapedLineMetrics::default(),
                clusters: Vec::new(),
                span_range,
            });
        }

        let mut total = ShapedLineMetrics::default();
        let mut offset_x = 0.0;
        let mut current_face = None;
        let mut current_script = None;
        let mut run = String::new();
        let mut run_start = 0;
        let mut clusters = Vec::new();
        let mut atom_cursor = 0;
        for span_index in span_range.clone() {
            let span = &projection.spans()[span_index];
            let visible = span.visible();
            if visible.start() < line_visible_start || visible.end() > line_visible_end {
                return Err(TextLayoutError::InvalidPreparedText);
            }
            if visible.start() == visible.end() {
                continue;
            }
            let atom_start = visible.start() - line_visible_start;
            let atom_end = visible.end() - line_visible_start;
            if atom_start != atom_cursor || atom_end <= atom_start {
                return Err(TextLayoutError::InvalidPreparedText);
            }
            let atom = text
                .get(atom_start..atom_end)
                .ok_or(TextLayoutError::InvalidPreparedText)?;
            let atom_script = cluster_script(atom);
            let script = compiled
                .requested_script
                .or_else(|| atom_script.or(current_script));
            let context_dependent_script =
                compiled.requested_script.is_none() && atom_script.is_none();
            let coverage_lane = usize::from(
                matches!(pass, StructuredShapingPass::Metrics)
                    && (style.coverage_lane != 0 || context_dependent_script),
            );
            let face_index = self
                .choose_structured_face_index(
                    atom,
                    span_index,
                    coverage_lane,
                    style,
                    compiled,
                    script,
                    coverage_cache,
                    budget,
                )?
                .ok_or(TextLayoutError::GlyphUnavailable)?;
            if (Some(face_index) != current_face || script != current_script) && !run.is_empty() {
                let current_face = current_face.expect("non-empty run has a face");
                let shaped = self.shape_structured_run(
                    current_face,
                    current_script,
                    &run,
                    typography,
                    compiled,
                    pass,
                    budget,
                )?;
                let run_metrics = shaped.metrics;
                total.include_run(run_metrics, offset_x);
                clusters.extend(
                    shaped
                        .clusters
                        .into_iter()
                        .map(|cluster| cluster.offset(run_start, offset_x)),
                );
                offset_x += run_metrics.advance;
                if emit_evidence {
                    response_builder
                        .as_deref_mut()
                        .expect("evidence shaping requires a response builder")
                        .push_run(PreparedTextRunResponse::new(
                            TextByteRange::new(
                                line_visible_start + run_start,
                                line_visible_start + atom_start,
                            ),
                            PreparedTextFaceSlot::new(
                                u32::try_from(current_face)
                                    .expect("prepared catalog face count fits in a face slot"),
                            ),
                            self.font_source,
                        ))?;
                }
                run.clear();
                run_start = atom_start;
            } else if run.is_empty() {
                run_start = atom_start;
            }
            current_face = Some(face_index);
            current_script = script;
            run.push_str(atom);
            atom_cursor = atom_end;
        }
        if atom_cursor != text.len() {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        if let Some(face_index) = current_face {
            let shaped = self.shape_structured_run(
                face_index,
                current_script,
                &run,
                typography,
                compiled,
                pass,
                budget,
            )?;
            let run_metrics = shaped.metrics;
            total.include_run(run_metrics, offset_x);
            clusters.extend(
                shaped
                    .clusters
                    .into_iter()
                    .map(|cluster| cluster.offset(run_start, offset_x)),
            );
            if emit_evidence {
                response_builder
                    .expect("evidence shaping requires a response builder")
                    .push_run(PreparedTextRunResponse::new(
                        TextByteRange::new(
                            line_visible_start + run_start,
                            line_visible_start + text.len(),
                        ),
                        PreparedTextFaceSlot::new(
                            u32::try_from(face_index)
                                .expect("prepared catalog face count fits in a face slot"),
                        ),
                        self.font_source,
                    ))?;
            }
        }

        let grapheme_count = text.graphemes(true).count();
        let letter_spacing =
            f64::from(typography.letter_spacing_px()) * grapheme_count.saturating_sub(1) as f64;
        let word_spacing = f64::from(typography.word_spacing_px())
            * text.chars().filter(|character| *character == ' ').count() as f64;
        total.advance += letter_spacing + word_spacing;
        Ok(ShapedStructuredLine {
            metrics: total,
            clusters,
            span_range,
        })
    }

    fn shape_structured_run(
        &self,
        face_index: usize,
        script: Option<BuzzScript>,
        text: &str,
        typography: &ThemeTextStyle,
        compiled: &CompiledStructuredTextRequest<'_>,
        pass: StructuredShapingPass,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<ShapedStructuredRun, TextLayoutError> {
        budget.record(pass, text.len())?;
        let face = &self.faces[face_index];
        let mut font = rustybuzz::Face::from_slice(&face.data, face.face_index)
            .ok_or(TextLayoutError::NoUsableFace)?;
        font.set_variations(&compiled.variations);
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        if let Some(direction) = compiled
            .backend_request
            .request()
            .direction()
            .to_rustybuzz()
        {
            buffer.set_direction(direction);
        }
        if let Some(script) = script {
            buffer.set_script(script);
        }
        if let Some(language) = compiled.language.clone() {
            buffer.set_language(language);
        }
        let glyph_direction = buffer.direction();
        let glyphs = rustybuzz::shape(&font, &compiled.features, buffer);
        let glyph_infos = glyphs.glyph_infos();
        if glyph_infos.is_empty() && !text.chars().all(is_default_ignorable) {
            return Err(TextLayoutError::GlyphUnavailable);
        }
        let units_per_em = f64::from(font.units_per_em().max(1));
        let scale = f64::from(typography.font_size_px()) / units_per_em;
        let mut metrics = ShapedLineMetrics::default();
        let backward = matches!(
            glyph_direction,
            BuzzDirection::RightToLeft | BuzzDirection::BottomToTop
        );
        budget.record_glyph_visits(glyph_infos.len())?;
        let mut glyph_order_cluster_starts = Vec::with_capacity(glyph_infos.len().min(text.len()));
        let mut previous_cluster_start = None;
        for info in glyph_infos {
            if info.glyph_id == 0 {
                return Err(TextLayoutError::GlyphUnavailable);
            }
            let cluster_start =
                usize::try_from(info.cluster).map_err(|_| TextLayoutError::InvalidPreparedText)?;
            if cluster_start >= text.len() || !text.is_char_boundary(cluster_start) {
                return Err(TextLayoutError::InvalidPreparedText);
            }
            if let Some(previous) = previous_cluster_start {
                let monotonic = if backward {
                    cluster_start <= previous
                } else {
                    cluster_start >= previous
                };
                if !monotonic {
                    return Err(TextLayoutError::InvalidPreparedText);
                }
            }
            if previous_cluster_start != Some(cluster_start) {
                glyph_order_cluster_starts.push(cluster_start);
                previous_cluster_start = Some(cluster_start);
            }
        }
        if backward {
            budget.record_cluster_visits(glyph_order_cluster_starts.len())?;
            glyph_order_cluster_starts.reverse();
        }
        let mut cluster_starts = Vec::with_capacity(
            glyph_order_cluster_starts
                .len()
                .saturating_add(usize::from(glyph_order_cluster_starts.first() != Some(&0))),
        );
        if glyph_order_cluster_starts.first() != Some(&0) {
            cluster_starts.push(0);
        }
        cluster_starts.extend(glyph_order_cluster_starts);
        budget.record_cluster_visits(cluster_starts.len().saturating_sub(1))?;
        for window in cluster_starts.windows(2) {
            if window[0] >= window[1] {
                return Err(TextLayoutError::InvalidPreparedText);
            }
        }
        budget.record_cluster_visits(cluster_starts.len())?;
        let mut clusters = Vec::with_capacity(cluster_starts.len());
        for (index, start) in cluster_starts.iter().copied().enumerate() {
            clusters.push(ShapedClusterMetric {
                range: TextByteRange::new(
                    start,
                    cluster_starts.get(index + 1).copied().unwrap_or(text.len()),
                ),
                advance: 0.0,
                min_x: 0.0,
                max_x: 0.0,
                has_bounds: false,
            });
        }
        let mut pen_x = 0.0;
        let first_glyph_start = glyph_infos
            .first()
            .and_then(|info| usize::try_from(info.cluster).ok());
        let mut cluster_index = if backward {
            cluster_starts.len().saturating_sub(1)
        } else if first_glyph_start == Some(0) {
            0
        } else {
            usize::from(!cluster_starts.is_empty())
        };
        let mut active_glyph_cluster = None;
        budget.record_glyph_visits(glyph_infos.len())?;
        for (info, position) in glyph_infos.iter().zip(glyphs.glyph_positions()) {
            let x_advance = f64::from(position.x_advance) * scale;
            let glyph_id = ttf_parser::GlyphId(info.glyph_id as u16);
            let glyph_bounds = font.glyph_bounding_box(glyph_id).map(|bounds| {
                let origin_x = pen_x + f64::from(position.x_offset) * scale;
                let origin_y = f64::from(position.y_offset) * scale;
                let min_x = origin_x + f64::from(bounds.x_min) * scale;
                let max_x = origin_x + f64::from(bounds.x_max) * scale;
                let min_y = origin_y + f64::from(bounds.y_min) * scale;
                let max_y = origin_y + f64::from(bounds.y_max) * scale;
                if !metrics.has_bounds {
                    metrics.min_x = min_x;
                    metrics.max_x = max_x;
                    metrics.min_y = min_y;
                    metrics.max_y = max_y;
                    metrics.has_bounds = true;
                } else {
                    metrics.min_x = metrics.min_x.min(min_x);
                    metrics.max_x = metrics.max_x.max(max_x);
                    metrics.min_y = metrics.min_y.min(min_y);
                    metrics.max_y = metrics.max_y.max(max_y);
                }
                (min_x, max_x)
            });
            let cluster_start =
                usize::try_from(info.cluster).map_err(|_| TextLayoutError::InvalidPreparedText)?;
            if active_glyph_cluster != Some(cluster_start) {
                if active_glyph_cluster.is_some() {
                    cluster_index = if backward {
                        cluster_index
                            .checked_sub(1)
                            .ok_or(TextLayoutError::InvalidPreparedText)?
                    } else {
                        cluster_index
                            .checked_add(1)
                            .ok_or(TextLayoutError::InvalidPreparedText)?
                    };
                }
                if cluster_starts.get(cluster_index) != Some(&cluster_start) {
                    return Err(TextLayoutError::InvalidPreparedText);
                }
                active_glyph_cluster = Some(cluster_start);
            }
            clusters[cluster_index].include_glyph(x_advance, glyph_bounds);
            pen_x += x_advance;
        }
        metrics.advance = pen_x;
        Ok(ShapedStructuredRun { metrics, clusters })
    }

    fn structured_line_height(&self, typography: &ThemeTextStyle) -> f64 {
        let font_size = f64::from(typography.font_size_px()).max(0.1);
        match typography.line_height() {
            LineHeight::Normal => font_size * 1.2,
            LineHeight::Multiplier(value) => font_size * f64::from(value),
            LineHeight::Px(value) => f64::from(value),
        }
    }

    fn shape_wrap_line(
        &self,
        source: ProjectedVisibleLine,
        compiled: &CompiledStructuredTextRequest<'_>,
        coverage_cache: &mut StructuredCoverageCache,
        span_cursor: &mut ProjectionSpanCursor,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<ShapedWrapLine, TextLayoutError> {
        let request = compiled.backend_request.request();
        let projection = compiled.backend_request.projection();
        let text = source.text(projection.visible())?;
        let shaped = self.shape_structured_line_internal(
            text,
            source.visible_range(),
            request.wrapping_typography(),
            compiled,
            &compiled.wrapping,
            StructuredShapingPass::Wrapping,
            coverage_cache,
            span_cursor,
            budget,
            None,
        )?;
        let atoms = coalesce_wrap_atoms(
            text,
            source.visible_range().start(),
            projection,
            shaped.span_range,
            &shaped.clusters,
            budget,
        )?;
        let mut measure = WrapMeasure::default();
        budget.record_wrap_boundary_visits(atoms.len())?;
        for atom in &atoms {
            measure.include(atom.measure);
        }
        Ok(ShapedWrapLine { atoms, measure })
    }

    fn wrap_svg_structured_line(
        &self,
        source: ProjectedVisibleLine,
        projection: &TextProjection,
        shaped: &ShapedWrapLine,
        typography: &ThemeTextStyle,
        max_width_px: f64,
        break_long_words: bool,
        wrapped: &mut Vec<ProjectedVisibleLine>,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<(), TextLayoutError> {
        let line = source.text(projection.visible())?;
        if !max_width_px.is_finite() || max_width_px <= 0.0 || line.is_empty() {
            return push_projected_visible_line(wrapped, source, budget);
        }
        budget.record_wrap_boundary_visits(line.len())?;
        let ranges = svg_wrap_segment_ranges(line);
        let segments = align_wrap_segments(&shaped.atoms, &ranges, budget)?;
        let output_start = wrapped.len();
        let mut current = WrapLineState::default();
        let mut segment_index = 0;
        while let Some(segment) = segments.get(segment_index).copied() {
            budget.record_wrap_boundary_visits(1)?;
            let whitespace = segment_is_whitespace(segment, &shaped.atoms, budget)?;
            if current.is_empty() && whitespace {
                segment_index += 1;
                continue;
            }
            let mut candidate = current;
            budget
                .record_wrap_boundary_visits(segment.atom_end.saturating_sub(segment.atom_start))?;
            candidate.append_segment(segment, &shaped.atoms);
            if candidate.width(typography) <= max_width_px {
                current = candidate;
                segment_index += 1;
                continue;
            }
            if current.has_visible_content() {
                push_wrapped_state(wrapped, source, current, &shaped.atoms, budget)?;
                current = WrapLineState::default();
                continue;
            }
            if whitespace {
                segment_index += 1;
                continue;
            }
            if !break_long_words {
                current = candidate;
                segment_index += 1;
                continue;
            }

            for atom_index in segment.atom_start..segment.atom_end {
                budget.record_wrap_boundary_visits(1)?;
                let mut candidate = current;
                candidate.append_atom(atom_index, shaped.atoms[atom_index]);
                if current.has_visible_content() && candidate.width(typography) > max_width_px {
                    push_wrapped_state(wrapped, source, current, &shaped.atoms, budget)?;
                    current = WrapLineState::default();
                }
                current.append_atom(atom_index, shaped.atoms[atom_index]);
            }
            segment_index += 1;
        }
        if current.has_visible_content() || wrapped.len() == output_start {
            push_wrapped_state(wrapped, source, current, &shaped.atoms, budget)?;
        }
        Ok(())
    }

    fn wrap_html_structured_line(
        &self,
        source: ProjectedVisibleLine,
        projection: &TextProjection,
        shaped: &ShapedWrapLine,
        typography: &ThemeTextStyle,
        max_width_px: f64,
        wrapped: &mut Vec<ProjectedVisibleLine>,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<(), TextLayoutError> {
        let line = source.text(projection.visible())?;
        if !max_width_px.is_finite() || max_width_px <= 0.0 || line.is_empty() {
            return push_projected_visible_line(wrapped, source, budget);
        }
        budget.record_wrap_boundary_visits(line.len())?;
        let ranges = html_wrap_segment_ranges(line);
        let segments = align_wrap_segments(&shaped.atoms, &ranges, budget)?;
        let output_start = wrapped.len();
        let mut current = WrapLineState::default();
        for segment in segments {
            budget.record_wrap_boundary_visits(
                1usize.saturating_add(segment.atom_end.saturating_sub(segment.atom_start)),
            )?;
            let mut candidate = current;
            candidate.append_segment(segment, &shaped.atoms);
            if current.is_empty() || candidate.width(typography) <= max_width_px {
                current = candidate;
            } else {
                push_wrapped_state(wrapped, source, current, &shaped.atoms, budget)?;
                current = WrapLineState::default();
                current.append_segment(segment, &shaped.atoms);
            }
        }
        if !current.is_empty() || wrapped.len() == output_start {
            push_wrapped_state(wrapped, source, current, &shaped.atoms, budget)?;
        }
        Ok(())
    }

    fn prepare_structured_text(
        &self,
        backend_request: &PreparedTextBackendRequest,
    ) -> Result<PreparedTextResponse, TextLayoutError> {
        self.prepare_structured_text_with_work(backend_request)
            .map(|(response, _)| response)
    }

    fn prepare_structured_text_with_work(
        &self,
        backend_request: &PreparedTextBackendRequest,
    ) -> Result<(PreparedTextResponse, StructuredShapingWork), TextLayoutError> {
        let (response, work) = self.prepare_structured_text_attempt(backend_request);
        Ok((response?, work))
    }

    fn prepare_structured_text_attempt(
        &self,
        backend_request: &PreparedTextBackendRequest,
    ) -> (
        Result<PreparedTextResponse, TextLayoutError>,
        StructuredShapingWork,
    ) {
        self.prepare_structured_text_attempt_internal(backend_request, None)
    }

    pub(super) fn prepare_structured_text_attempt_with_work_meter(
        &self,
        backend_request: &PreparedTextBackendRequest,
        work_meter: &OperationWorkMeter,
    ) -> (
        Result<PreparedTextResponse, TextLayoutError>,
        StructuredShapingWork,
    ) {
        self.prepare_structured_text_attempt_internal(backend_request, Some(work_meter))
    }

    fn prepare_structured_text_attempt_internal(
        &self,
        backend_request: &PreparedTextBackendRequest,
        work_meter: Option<&OperationWorkMeter>,
    ) -> (
        Result<PreparedTextResponse, TextLayoutError>,
        StructuredShapingWork,
    ) {
        let mut budget = StructuredShapingBudget::new(work_meter);
        let response = (|| {
            let request = backend_request.request();
            let projection = backend_request.projection();
            let compiled = self.compile_structured_request(backend_request, &mut budget)?;
            let coverage_cache_slots = projection
                .spans()
                .len()
                .checked_mul(STRUCTURED_COVERAGE_CACHE_LANES)
                .ok_or(TextLayoutError::LimitExceeded("coverage_cache"))?;
            budget.record_coverage_cache_slots(coverage_cache_slots)?;
            let mut coverage_cache = StructuredCoverageCache::new(projection.spans().len())?;
            let mut wrapping_span_cursor = ProjectionSpanCursor::default();
            budget.record_source_line_scan(projection.visible().len())?;
            let mut wrapped_lines = Vec::new();
            let mut raw_width_px: f64 = 0.0;
            for source in projected_visible_lines(projection.visible()) {
                budget.record_source_line_visit()?;
                match request.wrap() {
                    PreparedTextWrap::SingleRun
                    | PreparedTextWrap::SvgLike {
                        max_width_px: None, ..
                    } => {
                        push_projected_visible_line(&mut wrapped_lines, source, &mut budget)?;
                    }
                    PreparedTextWrap::SvgLike {
                        max_width_px: Some(max_width_px),
                        break_long_words,
                    } => {
                        let shaped = self.shape_wrap_line(
                            source,
                            &compiled,
                            &mut coverage_cache,
                            &mut wrapping_span_cursor,
                            &mut budget,
                        )?;
                        self.wrap_svg_structured_line(
                            source,
                            projection,
                            &shaped,
                            request.wrapping_typography(),
                            max_width_px,
                            break_long_words,
                            &mut wrapped_lines,
                            &mut budget,
                        )?;
                    }
                    PreparedTextWrap::HtmlLike { max_width_px } => {
                        let shaped = self.shape_wrap_line(
                            source,
                            &compiled,
                            &mut coverage_cache,
                            &mut wrapping_span_cursor,
                            &mut budget,
                        )?;
                        raw_width_px =
                            raw_width_px.max(shaped.measure.width(request.wrapping_typography()));
                        if let Some(max_width_px) = max_width_px {
                            self.wrap_html_structured_line(
                                source,
                                projection,
                                &shaped,
                                request.wrapping_typography(),
                                max_width_px,
                                &mut wrapped_lines,
                                &mut budget,
                            )?;
                        } else {
                            push_projected_visible_line(&mut wrapped_lines, source, &mut budget)?;
                        }
                    }
                }
            }
            let line_height_px = self.structured_line_height(request.metrics_typography());
            let raw_width_px =
                matches!(request.wrap(), PreparedTextWrap::HtmlLike { .. }).then_some(raw_width_px);
            let mut response_builder = PreparedTextResponseBuilder::new(
                backend_request.binding().clone(),
                line_height_px,
                raw_width_px,
                prepared_text_request_response_budget(projection.visible().len()),
            )?;
            let mut metrics_span_cursor = ProjectionSpanCursor::default();
            for line in &wrapped_lines {
                let text = line.text(projection.visible())?;
                response_builder.reserve_line(text.len())?;
                let shaped = self.shape_structured_line_with_evidence(
                    text,
                    line.visible_range(),
                    request.metrics_typography(),
                    &compiled,
                    &compiled.metrics,
                    &mut coverage_cache,
                    &mut metrics_span_cursor,
                    &mut budget,
                    &mut response_builder,
                )?;
                response_builder.push_reserved_line(PreparedTextLineResponse::new(
                    text,
                    line.visible_range(),
                    shaped.metrics.advance.abs(),
                    shaped.metrics.bbox_x(),
                    shaped.metrics.vertical_extents()?,
                )?);
            }
            response_builder.finish()
        })();
        (response, budget.work)
    }

    fn shape_run(
        &self,
        face_index: usize,
        script: Option<BuzzScript>,
        text: &str,
        style: &TextStyle,
    ) -> ShapedLineMetrics {
        let face = &self.faces[face_index];
        let Some(mut font) = rustybuzz::Face::from_slice(&face.data, face.face_index) else {
            return ShapedLineMetrics::default();
        };
        font.set_variations(&self.variations);
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        if let Some(direction) = self.direction.to_rustybuzz() {
            buffer.set_direction(direction);
        }
        if let Some(script) = script {
            buffer.set_script(script);
        }
        if let Some(language) = self.language.clone() {
            buffer.set_language(language);
        }
        let glyphs = rustybuzz::shape(&font, &self.features, buffer);
        let units_per_em = f64::from(font.units_per_em().max(1));
        let scale = style.font_size.max(0.1) / units_per_em;
        let mut metrics = ShapedLineMetrics::default();
        let mut pen_x = 0.0;
        for (info, position) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
            let x_advance = f64::from(position.x_advance) * scale;
            let glyph_id = ttf_parser::GlyphId(info.glyph_id as u16);
            if let Some(bounds) = font.glyph_bounding_box(glyph_id) {
                let origin_x = pen_x + f64::from(position.x_offset) * scale;
                let origin_y = f64::from(position.y_offset) * scale;
                let min_x = origin_x + f64::from(bounds.x_min) * scale;
                let max_x = origin_x + f64::from(bounds.x_max) * scale;
                let min_y = origin_y + f64::from(bounds.y_min) * scale;
                let max_y = origin_y + f64::from(bounds.y_max) * scale;
                if !metrics.has_bounds {
                    metrics.min_x = min_x;
                    metrics.max_x = max_x;
                    metrics.min_y = min_y;
                    metrics.max_y = max_y;
                    metrics.has_bounds = true;
                } else {
                    metrics.min_x = metrics.min_x.min(min_x);
                    metrics.max_x = metrics.max_x.max(max_x);
                    metrics.min_y = metrics.min_y.min(min_y);
                    metrics.max_y = metrics.max_y.max(max_y);
                }
            }
            pen_x += x_advance;
        }
        metrics.advance = pen_x.abs();
        metrics
    }

    fn line_height(&self, style: &TextStyle, wrap_mode: WrapMode) -> f64 {
        let factor = match wrap_mode {
            WrapMode::HtmlLike => 1.5,
            WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => 1.1,
        };
        (style.font_size.max(1.0) * factor).max(1.0)
    }

    fn line_metrics(
        &self,
        lines: &[String],
        style: &TextStyle,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        let width = lines
            .iter()
            .map(|line| self.shape_line(line, style).layout_width())
            .fold(0.0, f64::max);
        TextMetrics {
            width,
            height: self.line_height(style, wrap_mode) * lines.len().max(1) as f64,
            line_count: lines.len().max(1),
        }
    }

    fn wrap(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        let raw_lines = split_html_br_lines(text)
            .into_iter()
            .flat_map(|line| line.split('\n'))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let raw_metrics = self.line_metrics(&raw_lines, style, wrap_mode);
        let Some(max_width) = max_width.filter(|width| width.is_finite() && *width > 0.0) else {
            return (
                raw_metrics,
                (wrap_mode == WrapMode::HtmlLike).then_some(raw_metrics.width),
            );
        };

        let lines = match wrap_mode {
            WrapMode::SvgLikeSingleRun => raw_lines,
            WrapMode::SvgLike => raw_lines
                .iter()
                .flat_map(|line| {
                    crate::text::wrap_label_like_mermaid_lines(line, self, style, max_width)
                })
                .collect::<Vec<_>>(),
            WrapMode::HtmlLike => {
                crate::text::wrap_text_lines_measurer(text, self, style, Some(max_width))
            }
        };
        let mut metrics = self.line_metrics(&lines, style, wrap_mode);
        if wrap_mode == WrapMode::HtmlLike {
            let needs_wrap = raw_metrics.width > max_width;
            metrics.width = if needs_wrap {
                metrics.width.max(max_width)
            } else {
                metrics.width.min(max_width)
            };
        }
        (
            metrics,
            (wrap_mode == WrapMode::HtmlLike).then_some(raw_metrics.width),
        )
    }
}

impl PreparedTextBackendSession for NativeCatalogTextMeasurer {
    fn prepare_text(
        &self,
        request: &PreparedTextBackendRequest,
    ) -> Result<PreparedTextResponse, TextLayoutError> {
        self.prepare_structured_text(request)
    }
}

impl TextMeasurer for NativeCatalogTextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.wrap(text, style, None, WrapMode::SvgLike).0
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        text.split('\n')
            .map(|line| self.shape_line(line, style).advance)
            .fold(0.0, f64::max)
    }

    fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        text.split('\n')
            .map(|line| self.shape_line(line, style).bbox_x())
            .fold(
                (0.0_f64, 0.0_f64),
                |(left, right), (line_left, line_right)| {
                    (left.max(line_left), right.max(line_right))
                },
            )
    }

    fn measure_svg_text_bbox_x_with_ascii_overhang(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> (f64, f64) {
        self.measure_svg_text_bbox_x(text, style)
    }

    fn measure_svg_simple_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        text.split('\n')
            .map(|line| self.shape_line(line, style).bbox_width())
            .fold(0.0, f64::max)
    }

    fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure_svg_simple_text_bbox_width_px(text, style)
    }

    fn measure_svg_raw_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        text.split('\n')
            .map(|line| self.shape_line(line, style).bbox_height())
            .fold(0.0, f64::max)
    }

    fn measure_svg_simple_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure_svg_raw_text_bbox_height_px(text, style)
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure_svg_raw_text_bbox_height_px(text, style)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.wrap(text, style, max_width, wrap_mode).0
    }

    fn measure_wrapped_with_raw_width(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        self.wrap(text, style, max_width, wrap_mode)
    }
}

fn normalize_family(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn parse_font_weight(value: &str) -> u16 {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => 400,
        "bold" | "bolder" => 700,
        "lighter" => 300,
        value => value.parse::<u16>().unwrap_or(400).clamp(1, 1000),
    }
}

fn shaped_glyph_coverage_is_complete(text: &str, glyph_ids: impl IntoIterator<Item = u32>) -> bool {
    let mut glyph_ids = glyph_ids.into_iter().peekable();
    if glyph_ids.peek().is_none() {
        return text.chars().all(is_default_ignorable);
    }
    glyph_ids.all(|glyph_id| glyph_id != 0)
}

fn cluster_script(value: &str) -> Option<BuzzScript> {
    value
        .chars()
        .map(|character| character.script())
        .find(|script| {
            !matches!(
                script,
                UnicodeScript::Common | UnicodeScript::Inherited | UnicodeScript::Unknown
            )
        })
        .and_then(|script| BuzzScript::from_str(script.short_name()).ok())
}

#[cfg(test)]
mod tests {
    use crate::text::terminal_receipt::PreparedTextTerminalReceipt;
    #[test]
    fn prepared_text_retained_bytes_count_shared_evidence_once() {
        let catalog = mixed_catalog();
        let face = &catalog.faces()[0];
        let asset = catalog
            .assets()
            .iter()
            .find(|asset| asset.id() == face.asset_id())
            .expect("the fixture face belongs to one retained asset");
        let range = TextByteRange::new(0, 2);
        let projection = TextProjection::from_parts(
            Arc::from("ab"),
            Arc::from("AB"),
            vec![SourceVisibleSpan::new(range, range)],
        )
        .expect("the fixed projection is valid");
        let line = PreparedTextLine::new(
            "AB",
            range,
            10.0,
            (0.0, 10.0),
            PreparedTextVerticalExtents::new(-6.0, 2.0).expect("fixed vertical extents are valid"),
        )
        .expect("the fixed line geometry is valid");
        let run = PreparedTextLabelEvidence::new(
            range,
            range,
            PreparedTextFaceKey::new(asset.fingerprint(), face.face_index()),
            FontSource::Embedded,
        );
        let prepared = PreparedText::new_with_evidence(
            projection,
            [line],
            [run],
            12.0,
            None,
            Arc::from([Arc::<str>::from("note")]),
            Some(PreparedTextLabelEvidenceContext {
                catalog_fingerprint: catalog.fingerprint(),
                request_digest: TextLayoutRequestDigest::from_bytes([7; 32]),
                provenance: PreparedTextLabelProvenance::Native,
            }),
        )
        .expect("the fixed prepared label is valid");

        let pending = prepared
            .label_ledger_entry()
            .expect("the prepared label retains pending export evidence");
        let pending_bytes = pending.retained_bytes();
        let final_entry = pending.bind(PreparedTextLabelId::new(
            PreparedTextLabelFamily::Flowchart,
            1,
        ));
        let final_bytes = final_entry.retained_bytes();
        let expected_ledger_bytes = PREPARED_TEXT_LEDGER_ENTRY_RECORD_BYTES
            + TEXT_PROJECTION_SPAN_RECORD_BYTES
            + TEXT_BYTE_RANGE_RECORD_BYTES
            + PREPARED_TEXT_LINE_TEXT_RECORD_BYTES
            + 2
            + PREPARED_TEXT_LABEL_EVIDENCE_RECORD_BYTES;
        let expected_prepared_bytes = PREPARED_TEXT_OWNER_RECORD_BYTES
            + 2
            + 2
            + TEXT_PROJECTION_SPAN_RECORD_BYTES
            + PREPARED_TEXT_LINE_RECORD_BYTES
            + 2
            + PREPARED_TEXT_LABEL_EVIDENCE_RECORD_BYTES
            + PREPARED_TEXT_DIAGNOSTIC_RECORD_BYTES
            + 4
            + TEXT_BYTE_RANGE_RECORD_BYTES
            + PREPARED_TEXT_LINE_TEXT_RECORD_BYTES;

        assert_eq!(pending_bytes, expected_ledger_bytes);
        assert_eq!(final_bytes, expected_ledger_bytes);
        assert_eq!(prepared.retained_bytes(), expected_prepared_bytes);
        assert!(prepared.retained_bytes() >= final_bytes);
    }

    fn terminal_receipt_entry() -> PreparedTextLabelLedgerEntry {
        let catalog = mixed_catalog();
        let evidence = catalog
            .faces()
            .iter()
            .take(2)
            .enumerate()
            .map(|(index, face)| {
                let asset = catalog
                    .assets()
                    .iter()
                    .find(|asset| asset.id() == face.asset_id())
                    .expect("the fixture face belongs to one retained asset");
                let range = TextByteRange::new(index, index + 1);
                PreparedTextLabelEvidence::new(
                    range,
                    range,
                    PreparedTextFaceKey::new(asset.fingerprint(), face.face_index()),
                    FontSource::Embedded,
                )
            })
            .collect::<Vec<_>>();
        PreparedTextLabelLedgerEntry {
            id: PreparedTextLabelId::new(PreparedTextLabelFamily::State, 4),
            catalog_fingerprint: catalog.fingerprint(),
            request_digest: TextLayoutRequestDigest::from_bytes([7; 32]),
            provenance: PreparedTextLabelProvenance::Native,
            projection_spans: Arc::from([SourceVisibleSpan::new(
                TextByteRange::new(0, 2),
                TextByteRange::new(0, 2),
            )]),
            line_ranges: Arc::from([TextByteRange::new(0, 1), TextByteRange::new(1, 2)]),
            line_texts: Arc::from([Arc::<str>::from("A"), Arc::<str>::from("B")]),
            runs: evidence.into(),
        }
    }

    #[test]
    fn terminal_receipt_binds_artifact_request_lines_and_ordered_runs() {
        let svg = r#"<svg><text id="merman-prepared-state-4">AB</text></svg>"#;
        let entry = terminal_receipt_entry();
        let receipt = PreparedTextTerminalReceipt::from_ledger(svg, &[entry.clone()])
            .expect("the valid ledger should freeze a terminal receipt");

        assert!(receipt.artifact_matches(svg));
        assert!(
            !receipt.artifact_matches(r#"<svg><text id="merman-prepared-state-4">BA</text></svg>"#)
        );
        assert!(receipt.labels()[0].native_terminal_proof_is_incomplete());

        let mut changed_request = entry.clone();
        changed_request.request_digest = TextLayoutRequestDigest::from_bytes([8; 32]);
        let request_receipt =
            PreparedTextTerminalReceipt::from_ledger(svg, &[changed_request]).unwrap();
        assert_ne!(
            receipt.labels()[0].identity_digest(),
            request_receipt.labels()[0].identity_digest()
        );

        let mut changed_lines = entry.clone();
        changed_lines.line_texts = Arc::from([Arc::<str>::from("B"), Arc::<str>::from("A")]);
        let line_receipt = PreparedTextTerminalReceipt::from_ledger(svg, &[changed_lines]).unwrap();
        assert_ne!(
            receipt.labels()[0].identity_digest(),
            line_receipt.labels()[0].identity_digest()
        );

        let mut changed_run_assignment = entry.clone();
        let mut runs = changed_run_assignment.runs.to_vec();
        runs.swap(0, 1);
        changed_run_assignment.runs = runs.into();
        assert!(
            PreparedTextTerminalReceipt::from_ledger(svg, &[changed_run_assignment]).is_none(),
            "non-monotonic ordered run evidence must fail closed"
        );

        let mut missing_runs = entry.clone();
        missing_runs.runs = Arc::from([]);
        assert!(PreparedTextTerminalReceipt::from_ledger(svg, &[missing_runs]).is_none());

        let mut native_system_face = entry;
        Arc::make_mut(&mut native_system_face.runs)[0].font_source = FontSource::System;
        assert!(PreparedTextTerminalReceipt::from_ledger(svg, &[native_system_face]).is_none());
    }

    use super::*;
    use crate::diagram_theme::{
        FontAssetSpec, FontCatalogSpec, FontSourcePolicy, FontStack, ThemeResourcePolicy,
        ThemeTextStyle,
    };
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

    #[test]
    fn shaped_coverage_requires_real_glyphs_for_visible_text() {
        assert!(shaped_glyph_coverage_is_complete("A", [7]));
        assert!(!shaped_glyph_coverage_is_complete("A", [0]));
        assert!(!shaped_glyph_coverage_is_complete("A", []));
        assert!(shaped_glyph_coverage_is_complete("\u{200d}", []));
    }

    fn mixed_catalog() -> FontCatalog {
        let latin = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let cjk = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Xiaolai-Regular-CJK-Test.woff2"
        ));
        FontCatalogSpec::new([
            FontAssetSpec::new("latin", latin),
            FontAssetSpec::new("cjk", cjk),
        ])
        .with_alias("Xiaolai", "Xiaolai SC")
        .compile(&ThemeResourcePolicy::interactive())
        .expect("fixture catalog should compile")
    }

    #[test]
    fn css_font_admission_distinguishes_generic_keywords_from_named_families() {
        let latin = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let catalog = FontCatalogSpec::new([FontAssetSpec::new("latin", latin)])
            .with_generic_family(GenericFontFamily::Cursive, "Excalifont")
            .compile(&ThemeResourcePolicy::interactive())
            .expect("fixture catalog should compile");
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                catalog,
                FontSourcePolicy::embedded_only(),
            ))
            .expect("fixture catalog should prepare");
        let unquoted = parse_css_font_stack("cursive").expect("generic CSS family");
        let quoted = parse_css_font_stack("\"cursive\"").expect("named CSS family");
        let requested = ThemeTextStyle::default().with_font_stack(unquoted.font_stack().clone());

        let raw_request =
            PrepareTextRequest::new("alpha", requested.clone()).with_family_normalized_projection();
        assert!(matches!(
            layout.prepare_text(&raw_request),
            Err(TextLayoutError::FontFamilyUnavailable)
        ));

        let admitted = layout
            .admit_typography_with_css_font_stack(&requested, Some(&unquoted))
            .expect("unquoted generic should use the catalog mapping");
        assert_eq!(
            admitted.typography().font_stack().families(),
            &["Excalifont".to_string()]
        );
        assert_eq!(
            layout.admit_typography_with_css_font_stack(&requested, Some(&quoted)),
            Err(TextLayoutError::FontFamilyUnavailable),
            "quoted generic text is a named family and must not use the generic mapping"
        );
        layout
            .prepare_text(
                &PrepareTextRequest::new("alpha", admitted.typography().clone())
                    .with_family_normalized_projection(),
            )
            .expect("admitted generic typography should shape through its canonical family");
    }

    #[test]
    fn admitted_typography_serializes_named_fonts_and_all_measured_spacing() {
        let catalog = mixed_catalog();
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                catalog,
                FontSourcePolicy::embedded_only(),
            ))
            .expect("fixture catalog should prepare");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").unwrap())
            .with_font_size_px(20.0)
            .unwrap()
            .with_line_height(LineHeight::Multiplier(1.4))
            .unwrap()
            .with_letter_spacing_px(1.5)
            .unwrap()
            .with_word_spacing_px(2.5)
            .unwrap();
        let admitted = layout
            .admit_typography(&typography)
            .expect("catalog owns the requested family");
        let style = admitted.merge_emission_font_style(Some(
            "font-family:Arial;line-height:9;letter-spacing:8px;color:#123456",
        ));

        assert!(style.contains("color:#123456"), "{style}");
        assert!(
            style.contains("font-family:\"Excalifont\" !important"),
            "{style}"
        );
        assert!(style.contains("line-height:1.4 !important"), "{style}");
        assert!(style.contains("letter-spacing:1.5px !important"), "{style}");
        assert!(style.contains("word-spacing:2.5px !important"), "{style}");
        assert!(!style.contains("font-family:Arial"), "{style}");
        assert!(!style.contains("line-height:9"), "{style}");
        assert!((admitted.line_height_em() - 1.4).abs() < 1e-6);
    }

    fn distinct_catalog_fingerprint() -> FontCatalogFingerprint {
        let latin = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        FontCatalogSpec::new([FontAssetSpec::new("different-latin", latin)])
            .compile(&ThemeResourcePolicy::interactive())
            .expect("distinct fixture catalog should compile")
            .fingerprint()
    }

    fn prepared_test_request(text: &str) -> PrepareTextRequest {
        PrepareTextRequest::new(
            text,
            ThemeTextStyle::default()
                .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid")),
        )
    }

    fn backend_request_for(
        layout: &PreparedTextLayout,
        request: &PrepareTextRequest,
    ) -> PreparedTextBackendRequest {
        let binding = PreparedTextCallBinding::new(
            layout.catalog_fingerprint(),
            layout.contract_version(),
            layout.backend().clone(),
            layout.session_token(),
            request.digest(),
        )
        .expect("admitted layout identity is a valid call binding");
        PreparedTextBackendRequest::new(binding, request)
            .expect("bounded test request should normalize")
    }

    fn metered_projection_and_digest_work(request: &PrepareTextRequest) -> usize {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        text_projection_for_request_with_meter(request, Some(&meter))
            .expect("bounded test request should build its metered projection");
        request
            .digest_with_work_meter(Some(&meter))
            .expect("bounded request digest work should be admitted");
        meter.used()
    }

    fn valid_raw_response(request: &PreparedTextBackendRequest) -> PreparedTextResponse {
        let lines = split_projected_visible_lines(request.visible_text())
            .into_iter()
            .map(|line| {
                PreparedTextLineResponse::new(
                    line.text(request.visible_text())
                        .expect("split line range belongs to the visible projection"),
                    line.visible_range,
                    10.0,
                    (0.0, 10.0),
                    PreparedTextVerticalExtents::new(-6.0, 2.0)
                        .expect("fixed vertical extents are valid"),
                )
                .expect("fixed raw line geometry is valid")
            })
            .collect::<Vec<_>>();
        let runs = lines
            .iter()
            .filter(|line| line.visible_range().start() < line.visible_range().end())
            .map(|line| {
                PreparedTextRunResponse::new(
                    line.visible_range(),
                    PreparedTextFaceSlot::new(0),
                    FontSource::Embedded,
                )
            })
            .collect::<Vec<_>>();
        PreparedTextResponse::new(request.binding().clone(), lines, runs, 12.0, None)
            .expect("fixed raw response is structurally bounded")
    }

    #[test]
    fn external_prepared_text_response_is_admitted_and_retains_actual_font_evidence() {
        #[derive(Clone)]
        struct EchoPreparedTextSession;

        impl PreparedTextBackendSession for EchoPreparedTextSession {
            fn prepare_text(
                &self,
                request: &PreparedTextBackendRequest,
            ) -> Result<PreparedTextResponse, TextLayoutError> {
                valid_raw_response(request).with_diagnostics(["host-shaped"])
            }
        }

        let catalog = mixed_catalog();
        let catalog_request =
            PrepareCatalogRequest::new(catalog.clone(), FontSourcePolicy::embedded_only());
        let backend =
            TextLayoutBackendIdentity::new("test.echo", "v1").expect("test backend identity");
        let token = TextLayoutSessionToken::from_bytes([11; 16]).expect("nonzero token");
        let response = PreparedTextLayoutResponse::new(
            catalog.fingerprint(),
            TEXT_LAYOUT_CONTRACT_VERSION,
            backend.clone(),
            TextLayoutCapabilities::native(),
            FontSource::Embedded,
            TextLayoutFaceEvidence::new(catalog.faces().iter().cloned())
                .expect("complete face evidence"),
            token,
            Arc::new(EchoPreparedTextSession),
        )
        .expect("catalog response should decode");
        let layout = PreparedTextLayout::admit_backend_response(
            &catalog_request,
            &backend,
            TextLayoutCapabilities::native(),
            response,
        )
        .expect("complete host catalog evidence should be admitted");
        let request = PrepareTextRequest::new(
            "ß<br>a",
            ThemeTextStyle::default()
                .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"))
                .with_transform(ThemeTextTransform::Uppercase),
        );

        let admitted = layout
            .prepare_text(&request)
            .expect("bound host response should be admitted");

        assert_eq!(admitted.source_text(), "ß<br>a");
        assert_eq!(admitted.visible_text(), "SS\nA");
        assert_eq!(admitted.wrapped_lines().collect::<Vec<_>>(), ["SS", "A"]);
        assert_eq!(admitted.run_evidence().len(), 2);
        assert!(
            admitted
                .run_evidence()
                .iter()
                .all(|run| run.font_source() == FontSource::Embedded)
        );
        let selected_face = &catalog.faces()[0];
        let selected_asset = catalog
            .assets()
            .iter()
            .find(|asset| asset.id() == selected_face.asset_id())
            .expect("selected face belongs to the retained catalog");
        assert!(admitted.run_evidence().iter().all(|run| {
            run.face_key()
                == PreparedTextFaceKey::new(
                    selected_asset.fingerprint(),
                    selected_face.face_index(),
                )
        }));
        let ledger_entry = admitted
            .label_ledger_entry()
            .expect("admitted backend result retains per-label evidence")
            .bind(PreparedTextLabelId::new(
                PreparedTextLabelFamily::Flowchart,
                4,
            ));
        assert_eq!(ledger_entry.id().as_svg_id(), "merman-prepared-flowchart-4");
        assert_eq!(ledger_entry.catalog_fingerprint(), catalog.fingerprint());
        assert_eq!(ledger_entry.request_digest(), request.digest());
        assert_eq!(
            ledger_entry.provenance(),
            PreparedTextLabelProvenance::HostDependent
        );
        assert_eq!(ledger_entry.projection_spans().len(), 3);
        assert_eq!(
            ledger_entry.line_ranges(),
            [TextByteRange::new(0, 2), TextByteRange::new(3, 4)]
        );
        assert_eq!(ledger_entry.evidence().len(), 2);
        assert_eq!(
            ledger_entry.evidence()[0].source_range(),
            TextByteRange::new(0, 2)
        );
        assert_eq!(
            ledger_entry.evidence()[0].visible_range(),
            TextByteRange::new(0, 2)
        );
        assert_eq!(
            ledger_entry.evidence()[1].source_range(),
            TextByteRange::new(6, 7)
        );
        assert_eq!(
            ledger_entry.evidence()[1].visible_range(),
            TextByteRange::new(3, 4)
        );
        assert!(ledger_entry.evidence().iter().all(|run| {
            run.face_key()
                == PreparedTextFaceKey::new(
                    selected_asset.fingerprint(),
                    selected_face.face_index(),
                )
                && run.font_source() == FontSource::Embedded
        }));
        assert_eq!(admitted.diagnostics().collect::<Vec<_>>(), ["host-shaped"]);
    }

    #[test]
    fn prepared_text_admission_rejects_stale_or_cross_session_bindings() {
        let catalog = mixed_catalog();
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                catalog,
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native catalog should prepare");
        let request = prepared_test_request("alpha");
        let backend_request = backend_request_for(&layout, &request);
        let response = valid_raw_response(&backend_request);

        let mut mismatched = response.clone();
        mismatched.binding.catalog_fingerprint = distinct_catalog_fingerprint();
        assert_eq!(
            layout
                .admit_text_response(&backend_request, mismatched)
                .expect_err("cross-catalog response must be rejected"),
            TextLayoutError::CatalogFingerprintMismatch
        );

        let mut mismatched = response.clone();
        mismatched.binding.request_digest = prepared_test_request("beta").digest();
        assert_eq!(
            layout
                .admit_text_response(&backend_request, mismatched)
                .expect_err("stale request digest must be rejected"),
            TextLayoutError::RequestDigestMismatch
        );

        let mut mismatched = response.clone();
        mismatched.binding.session_token =
            TextLayoutSessionToken::from_bytes([12; 16]).expect("nonzero token");
        assert_eq!(
            layout
                .admit_text_response(&backend_request, mismatched)
                .expect_err("cross-session response must be rejected"),
            TextLayoutError::SessionTokenMismatch
        );

        let mut mismatched = response.clone();
        mismatched.binding.contract_version = TEXT_LAYOUT_CONTRACT_VERSION + 1;
        assert!(matches!(
            layout
                .admit_text_response(&backend_request, mismatched)
                .expect_err("cross-contract response must be rejected"),
            TextLayoutError::UnsupportedContract { .. }
        ));

        let mut mismatched = response;
        mismatched.binding.backend =
            TextLayoutBackendIdentity::new("test.other", "v1").expect("valid identity");
        assert_eq!(
            layout
                .admit_text_response(&backend_request, mismatched)
                .expect_err("cross-backend response must be rejected"),
            TextLayoutError::BackendIdentityMismatch
        );
    }

    #[test]
    fn prepared_text_admission_rejects_invalid_geometry_ranges_and_run_evidence() {
        let catalog = mixed_catalog();
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                catalog,
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native catalog should prepare");
        let request = prepared_test_request("alpha");
        let backend_request = backend_request_for(&layout, &request);
        let response = valid_raw_response(&backend_request);

        let mut invalid = response.clone();
        Arc::make_mut(&mut invalid.lines)[0].computed_length_px = f64::NAN;
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalid)
                .expect_err("non-finite raw geometry must be rejected"),
            TextLayoutError::InvalidPreparedText
        );

        let mut invalid = response.clone();
        Arc::make_mut(&mut invalid.lines)[0].vertical_extents = PreparedTextVerticalExtents {
            top_px: 2.0,
            bottom_px: 1.0,
        };
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalid)
                .expect_err("missing a valid baseline-relative ink interval must fail closed"),
            TextLayoutError::InvalidPreparedText
        );

        let mut invalid = response.clone();
        Arc::make_mut(&mut invalid.lines)[0].visible_range = TextByteRange::new(0, 4);
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalid)
                .expect_err("line ranges must cover the exact canonical text"),
            TextLayoutError::InvalidPreparedText
        );

        let mut invalid = response.clone();
        Arc::make_mut(&mut invalid.runs)[0].face_slot = PreparedTextFaceSlot::new(u32::MAX);
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalid)
                .expect_err("unknown face slots must be rejected"),
            TextLayoutError::RunEvidenceMismatch
        );

        let mut invalid = response.clone();
        Arc::make_mut(&mut invalid.runs)[0].font_source = FontSource::System;
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalid)
                .expect_err("run source must match the admitted session source"),
            TextLayoutError::RunEvidenceMismatch
        );

        let mut invalid = response;
        invalid.runs = Arc::from([]);
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalid)
                .expect_err("non-empty lines require complete run coverage"),
            TextLayoutError::RunEvidenceMismatch
        );
    }

    #[test]
    fn prepared_text_admission_rejects_incomplete_or_mode_inconsistent_content() {
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native catalog should prepare");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));

        let wrapped_request = PrepareTextRequest::new("alpha beta", typography.clone()).with_wrap(
            PreparedTextWrap::SvgLike {
                max_width_px: Some(40.0),
                break_long_words: true,
            },
        );
        let backend_request = backend_request_for(&layout, &wrapped_request);
        let partial = PreparedTextResponse::new(
            backend_request.binding().clone(),
            [PreparedTextLineResponse::new(
                "alpha",
                TextByteRange::new(0, 5),
                10.0,
                (0.0, 10.0),
                PreparedTextVerticalExtents::new(-6.0, 2.0)
                    .expect("fixed vertical extents are valid"),
            )
            .expect("partial line geometry is structurally valid")],
            [PreparedTextRunResponse::new(
                TextByteRange::new(0, 5),
                PreparedTextFaceSlot::new(0),
                FontSource::Embedded,
            )],
            12.0,
            None,
        )
        .expect("partial response is structurally bounded");
        assert_eq!(
            layout
                .admit_text_response(&backend_request, partial)
                .expect_err("a backend cannot omit non-whitespace visible content"),
            TextLayoutError::InvalidPreparedText
        );

        let html_request = PrepareTextRequest::new("alpha beta", typography).with_wrap(
            PreparedTextWrap::HtmlLike {
                max_width_px: Some(40.0),
            },
        );
        let backend_request = backend_request_for(&layout, &html_request);
        let missing_raw_width = valid_raw_response(&backend_request);
        assert_eq!(
            layout
                .admit_text_response(&backend_request, missing_raw_width)
                .expect_err("HTML preparation must retain its unwrapped width"),
            TextLayoutError::InvalidPreparedText
        );
    }

    #[test]
    fn prepared_text_admission_rejects_backend_invalidation_and_unbounded_requests() {
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native catalog should prepare");
        let request = prepared_test_request("alpha");
        let backend_request = backend_request_for(&layout, &request);
        let invalidated = PreparedTextResponse::invalidated(
            backend_request.binding().clone(),
            ["font cache evicted"],
        )
        .expect("bounded invalidation response should decode");
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalidated)
                .expect_err("invalidated response must not enter layout"),
            TextLayoutError::BackendInvalidated
        );

        let oversized = prepared_test_request(&"x".repeat(MAX_TEXT_PROJECTION_BYTES + 1));
        assert_eq!(
            layout
                .prepare_text(&oversized)
                .expect_err("oversized text must be rejected before reaching the backend"),
            TextLayoutError::LimitExceeded("text")
        );

        let invalid_width = prepared_test_request("alpha").with_wrap(PreparedTextWrap::SvgLike {
            max_width_px: Some(f64::NAN),
            break_long_words: false,
        });
        assert_eq!(
            layout
                .prepare_text(&invalid_width)
                .expect_err("invalid widths must be rejected before reaching the backend"),
            TextLayoutError::InvalidRequest("max_width_px")
        );
    }

    #[test]
    fn prepared_text_response_enforces_hard_request_and_geometry_budgets() {
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native catalog should prepare");
        let request = prepared_test_request("a");
        let backend_request = backend_request_for(&layout, &request);
        let oversized_line = PreparedTextLineResponse::new(
            "x".repeat(MAX_TEXT_PROJECTION_BYTES),
            TextByteRange::new(0, MAX_TEXT_PROJECTION_BYTES),
            1.0,
            (0.0, 1.0),
            PreparedTextVerticalExtents::new(-0.8, 0.2).expect("fixed vertical extents are valid"),
        )
        .expect("one projection-sized line remains within the per-line hard cap");
        assert_eq!(
            PreparedTextResponse::new(
                backend_request.binding().clone(),
                vec![oversized_line; 17],
                [],
                1.0,
                None,
            )
            .expect_err("aggregate response bytes must have a hard cap"),
            TextLayoutError::LimitExceeded("response.bytes")
        );

        let mut record_amplification = valid_raw_response(&backend_request);
        record_amplification.lines = vec![record_amplification.lines[0].clone(); 3].into();
        assert_eq!(
            layout
                .admit_text_response(&backend_request, record_amplification)
                .expect_err("response records must scale with the bound projection"),
            TextLayoutError::LimitExceeded("response.records")
        );

        let multiline = prepared_test_request("a\nb");
        let backend_request = backend_request_for(&layout, &multiline);
        let mut oversized_geometry = valid_raw_response(&backend_request);
        oversized_geometry.line_height_px = 600_000_000.0;
        assert_eq!(
            layout
                .admit_text_response(&backend_request, oversized_geometry)
                .expect_err("aggregate geometry must remain within the global ceiling"),
            TextLayoutError::InvalidPreparedText
        );
    }

    #[test]
    fn prepared_text_response_stops_pulling_lines_at_the_first_retained_byte_overflow() {
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native catalog should prepare");
        let request = prepared_test_request("a");
        let backend_request = backend_request_for(&layout, &request);
        let line = PreparedTextLineResponse::new(
            "x".repeat(MAX_TEXT_PROJECTION_BYTES),
            TextByteRange::new(0, MAX_TEXT_PROJECTION_BYTES),
            1.0,
            (0.0, 1.0),
            PreparedTextVerticalExtents::new(-0.8, 0.2).expect("fixed vertical extents are valid"),
        )
        .expect("one projection-sized line remains within the per-line hard cap");
        let line_bytes = MAX_TEXT_PROJECTION_BYTES + PREPARED_TEXT_LINE_RECORD_BYTES;
        let admitted_lines = MAX_PREPARED_TEXT_RESPONSE_BYTES / line_bytes;
        let line_pulls = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let run_pulls = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let counted_lines = {
            let line_pulls = std::rc::Rc::clone(&line_pulls);
            std::iter::from_fn(move || {
                line_pulls.set(line_pulls.get().saturating_add(1));
                Some(line.clone())
            })
        };
        let counted_runs = {
            let run_pulls = std::rc::Rc::clone(&run_pulls);
            std::iter::from_fn(move || {
                run_pulls.set(run_pulls.get().saturating_add(1));
                Some(PreparedTextRunResponse::new(
                    TextByteRange::new(0, 1),
                    PreparedTextFaceSlot::new(0),
                    FontSource::Embedded,
                ))
            })
        };

        assert_eq!(
            PreparedTextResponse::new(
                backend_request.binding().clone(),
                counted_lines,
                counted_runs,
                1.0,
                None,
            )
            .expect_err("the first line beyond the retained-byte budget must stop collection"),
            TextLayoutError::LimitExceeded("response.bytes")
        );
        assert_eq!(line_pulls.get(), admitted_lines.saturating_add(1));
        assert_eq!(
            run_pulls.get(),
            0,
            "runs must never be pulled after line overflow"
        );
    }

    #[test]
    fn native_prepare_freezes_catalog_identity_and_session_token() {
        let catalog = mixed_catalog();
        let request =
            PrepareCatalogRequest::new(catalog.clone(), FontSourcePolicy::embedded_only());
        let backend = NativeTextLayoutBackend::default();
        let prepared = backend
            .prepare(&request)
            .expect("native backend should prepare fixture catalog");

        assert_eq!(prepared.catalog_fingerprint(), catalog.fingerprint());
        assert!(prepared.attests_catalog(catalog.fingerprint()));
        assert_eq!(
            prepared.face_evidence().faces().len(),
            catalog.faces().len()
        );
        assert_eq!(prepared.report().face_count(), catalog.faces().len());

        let repeated = backend
            .prepare(&request)
            .expect("repeated preparation should succeed");
        assert_eq!(prepared.session_token(), repeated.session_token());
    }

    #[test]
    fn native_prepare_shapes_latin_and_cjk_without_rediscovery() {
        let catalog = mixed_catalog();
        let request = PrepareCatalogRequest::new(catalog, FontSourcePolicy::embedded_only());
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&request)
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default().with_font_stack(
            FontStack::new(["Excalifont", "Xiaolai"]).expect("fixture stack is valid"),
        );

        let latin = prepared
            .prepare_text(&PrepareTextRequest::new("portable", typography.clone()))
            .expect("Latin label should use the first catalog family");
        let mixed = prepared
            .prepare_text(&PrepareTextRequest::new("portable 图", typography))
            .expect("CJK cluster should fall through to the second catalog family");
        assert!(latin.metrics().width.is_finite() && latin.metrics().width > 0.0);
        assert!(
            mixed.metrics().width.is_finite() && mixed.metrics().width >= latin.metrics().width
        );
        assert_eq!(latin.metrics().line_count, 1);
    }

    #[test]
    fn prepared_text_rejects_stack_external_families_and_missing_glyphs() {
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native backend should prepare fixture catalog");
        let unknown = ThemeTextStyle::default().with_font_stack(
            FontStack::single("Not In Catalog").expect("bounded unknown family is valid input"),
        );
        assert_eq!(
            prepared
                .prepare_text(&PrepareTextRequest::new("portable", unknown))
                .expect_err("an unknown family must not fall back to the first catalog face"),
            TextLayoutError::FontFamilyUnavailable
        );

        let latin_only = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        assert_eq!(
            prepared
                .prepare_text(&PrepareTextRequest::new("图", latin_only))
                .expect_err("a missing glyph must not silently become zero width"),
            TextLayoutError::GlyphUnavailable
        );
    }

    #[test]
    fn native_backend_requires_embedded_source_authority() {
        let request = PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::system_only());
        assert_eq!(
            NativeTextLayoutBackend::default()
                .prepare(&request)
                .expect_err("native catalog shaping cannot claim a system-only session"),
            TextLayoutError::FontSourceUnavailable
        );
    }

    #[test]
    fn native_measurement_separates_advance_from_outline_bounds() {
        let request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&request)
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"))
            .with_font_size_px(32.0)
            .expect("fixture font size is valid");

        let witness = ["j", "f", "Tj", "Á", "Wj"]
            .into_iter()
            .find(|text| {
                let result = prepared
                    .prepare_text(&PrepareTextRequest::new(*text, typography.clone()))
                    .expect("fixture witness should prepare");
                (result.computed_length_px() - result.bbox_width_px()).abs() > 0.01
            })
            .expect("fixture font should expose an advance/bbox distinction");
        let result = prepared
            .prepare_text(&PrepareTextRequest::new(witness, typography))
            .expect("fixture witness should prepare");
        let (left, right) = result.bbox_x();
        let bbox = result.bbox_width_px();
        let height = result.bbox_height_px();

        assert!(left.is_finite() && right.is_finite());
        assert!(bbox.is_finite() && bbox > 0.0);
        assert!(height.is_finite() && height > 0.0);
    }

    #[test]
    fn native_prepared_text_retains_baseline_relative_extents_across_lines_and_sizes() {
        let request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&request)
            .expect("native backend should prepare fixture catalog");
        let mut observed_heights = Vec::new();

        for font_size in [12.0, 32.0] {
            let typography = ThemeTextStyle::default()
                .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"))
                .with_font_size_px(font_size)
                .expect("fixture font size is valid")
                .with_line_height(LineHeight::Multiplier(1.25))
                .expect("fixture line height is valid");
            let single = prepared
                .prepare_text(&PrepareTextRequest::new("Agjp", typography.clone()))
                .expect("single-line fixture should prepare");
            let single_extents = single.vertical_extents();
            assert!(single_extents.top_px() < 0.0);
            assert!(single_extents.bottom_px() > 0.0);
            assert_eq!(single.bbox_height_px(), single_extents.height_px());

            let multiline = prepared
                .prepare_text(&PrepareTextRequest::new("Ag\njp", typography))
                .expect("multi-line fixture should prepare");
            assert_eq!(multiline.lines().len(), 2);
            let expected = multiline.lines()[0]
                .vertical_extents()
                .union(
                    multiline.lines()[1]
                        .vertical_extents()
                        .translated(multiline.line_height_px())
                        .expect("bounded line advance remains valid"),
                )
                .expect("bounded line union remains valid");
            assert_eq!(multiline.vertical_extents(), expected);
            assert_eq!(multiline.bbox_height_px(), expected.height_px());
            assert!(multiline.bbox_height_px() > single.bbox_height_px());
            observed_heights.push(single.bbox_height_px());
        }

        assert!(observed_heights[1] > observed_heights[0] * 2.0);
    }

    #[test]
    fn native_wrapping_preserves_html_svg_and_single_run_modes() {
        let request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&request)
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let max_width = prepared
            .prepare_text(&PrepareTextRequest::new("alpha", typography.clone()))
            .expect("probe should prepare")
            .computed_length_px()
            + 1.0;

        let svg = prepared
            .prepare_text(
                &PrepareTextRequest::new("alpha beta gamma", typography.clone()).with_wrap(
                    PreparedTextWrap::SvgLike {
                        max_width_px: Some(max_width),
                        break_long_words: true,
                    },
                ),
            )
            .expect("SVG wrapping should prepare");
        let single_run = prepared
            .prepare_text(
                &PrepareTextRequest::new("alpha beta gamma", typography.clone())
                    .with_wrap(PreparedTextWrap::SingleRun),
            )
            .expect("single run should prepare");
        let html = prepared
            .prepare_text(
                &PrepareTextRequest::new("alpha beta gamma", typography.clone()).with_wrap(
                    PreparedTextWrap::HtmlLike {
                        max_width_px: Some(max_width),
                    },
                ),
            )
            .expect("HTML wrapping should prepare");
        let explicit_break = prepared
            .prepare_text(
                &PrepareTextRequest::new("alpha<br/>beta", typography)
                    .with_wrap(PreparedTextWrap::SingleRun),
            )
            .expect("explicit break should prepare");

        assert!(svg.metrics().line_count > 1);
        assert_eq!(single_run.metrics().line_count, 1);
        assert!(html.metrics().line_count > 1);
        assert!(html.raw_width_px().is_some_and(|width| width > max_width));
        assert_eq!(explicit_break.metrics().line_count, 2);
    }

    #[test]
    fn native_wrapping_shapes_each_visible_byte_at_most_once_per_pass() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));

        for (text, wrap, expects_metrics_fanout) in [
            (
                "a".repeat(8_192),
                PreparedTextWrap::SvgLike {
                    max_width_px: Some(32.0),
                    break_long_words: true,
                },
                true,
            ),
            (
                "alpha ".repeat(1_365),
                PreparedTextWrap::SvgLike {
                    max_width_px: Some(1_000_000.0),
                    break_long_words: true,
                },
                false,
            ),
            (
                "alpha-beta ".repeat(744),
                PreparedTextWrap::HtmlLike {
                    max_width_px: Some(1_000_000.0),
                },
                false,
            ),
        ] {
            let request = PrepareTextRequest::new(text, typography.clone()).with_wrap(wrap);
            let backend_request = backend_request_for(&layout, &request);
            let visible_bytes = backend_request.visible_text().len();
            let projection_spans = backend_request.projection().spans().len();
            let (response, work) = measurer
                .prepare_structured_text_with_work(&backend_request)
                .expect("bounded fixture text should prepare linearly");

            assert_eq!(work.wrapping_input_bytes, visible_bytes);
            assert!(work.metrics_input_bytes <= visible_bytes);
            assert!(
                work.wrapping_input_bytes
                    .saturating_add(work.metrics_input_bytes)
                    <= visible_bytes.saturating_mul(2)
            );
            assert_eq!(work.wrapping_line_ranges, 1);
            assert_eq!(work.metrics_line_ranges, response.lines().len());
            if expects_metrics_fanout {
                assert!(work.metrics_line_ranges > work.wrapping_line_ranges);
            }
            assert!(
                work.wrapping_span_visits
                    <= projection_spans.saturating_add(work.wrapping_line_ranges.saturating_mul(2))
            );
            assert!(
                work.metrics_span_visits
                    <= projection_spans.saturating_add(work.metrics_line_ranges.saturating_mul(2))
            );
        }
    }

    #[test]
    fn native_wrapping_span_cursor_visits_multiline_projection_linearly() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let source_line_count = 512;
        let text = vec!["alpha"; source_line_count].join("\n");
        let request =
            PrepareTextRequest::new(text, typography).with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(1_000_000.0),
                break_long_words: true,
            });
        let backend_request = backend_request_for(&layout, &request);
        let projection_spans = backend_request.projection().spans().len();
        let (response, work) = measurer
            .prepare_structured_text_with_work(&backend_request)
            .expect("multiline fixture should prepare linearly");

        assert_eq!(work.wrapping_line_ranges, source_line_count);
        assert_eq!(response.lines().len(), source_line_count);
        assert!(
            work.wrapping_span_visits
                <= projection_spans.saturating_add(work.wrapping_line_ranges.saturating_mul(2))
        );
    }

    #[test]
    fn projection_span_cursor_preserves_gaps_empty_lines_and_expanded_atoms() {
        let source = "a  b\n\nß";
        let projection = TextProjection::from_parts(
            Arc::from(source),
            Arc::from("A B\n\nSS"),
            vec![
                SourceVisibleSpan::new(TextByteRange::new(0, 1), TextByteRange::new(0, 1)),
                SourceVisibleSpan::new(TextByteRange::new(1, 2), TextByteRange::new(1, 2)),
                SourceVisibleSpan::new(TextByteRange::new(2, 3), TextByteRange::new(2, 2)),
                SourceVisibleSpan::new(TextByteRange::new(3, 4), TextByteRange::new(2, 3)),
                SourceVisibleSpan::new(TextByteRange::new(4, 5), TextByteRange::new(3, 4)),
                SourceVisibleSpan::new(TextByteRange::new(5, 6), TextByteRange::new(4, 5)),
                SourceVisibleSpan::new(TextByteRange::new(6, 8), TextByteRange::new(5, 7)),
            ],
        )
        .expect("projection with folded whitespace should build");
        assert_eq!(projection.visible(), "A B\n\nSS");
        let lines = split_projected_visible_lines(projection.visible());
        assert_eq!(
            lines
                .iter()
                .map(|line| line.visible_range())
                .collect::<Vec<_>>(),
            vec![
                TextByteRange::new(0, 3),
                TextByteRange::new(4, 4),
                TextByteRange::new(5, 7),
            ]
        );

        let mut cursor = ProjectionSpanCursor::default();
        let mut budget = StructuredShapingBudget::new(None);
        let ranges = lines
            .iter()
            .map(|line| {
                cursor.range_for_line(
                    &projection,
                    line.visible_range(),
                    StructuredShapingPass::Metrics,
                    &mut budget,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .expect("ordered projected lines should admit");

        assert_eq!(ranges[0], 0..4);
        assert!(ranges[1].is_empty());
        assert_eq!(ranges[2], 6..7);
        let work = budget.work;
        assert_eq!(work.metrics_line_ranges, lines.len());
        assert!(
            work.metrics_span_visits
                <= projection
                    .spans()
                    .len()
                    .saturating_add(lines.len().saturating_mul(2))
        );

        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"))
            .with_transform(ThemeTextTransform::Uppercase);
        let request =
            PrepareTextRequest::new(source, typography).with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(1_000_000.0),
                break_long_words: true,
            });
        let binding = backend_request_for(&layout, &request).binding().clone();
        let backend_request =
            PreparedTextBackendRequest::from_projection(binding, &request, projection)
                .expect("custom test projection should match the request source");
        let projection_spans = backend_request.projection().spans().len();
        let (response, work) = measurer
            .prepare_structured_text_with_work(&backend_request)
            .expect("zero-length projection spans should not be shaped as visible atoms");

        assert_eq!(
            response
                .lines()
                .iter()
                .map(PreparedTextLineResponse::text)
                .collect::<Vec<_>>(),
            ["A B", "", "SS"]
        );
        assert_eq!(work.wrapping_line_ranges, 3);
        assert_eq!(work.metrics_line_ranges, 3);
        assert!(
            work.wrapping_span_visits
                <= projection_spans.saturating_add(work.wrapping_line_ranges.saturating_mul(2))
        );
        assert!(
            work.metrics_span_visits
                <= projection_spans.saturating_add(work.metrics_line_ranges.saturating_mul(2))
        );
    }

    #[test]
    fn native_cluster_coverage_shapes_complex_graphemes_once_across_wrap_and_metrics() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let request =
            PrepareTextRequest::new("e\u{301}", typography).with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(1_000.0),
                break_long_words: true,
            });
        let backend_request = backend_request_for(&layout, &request);
        let (_, work) = measurer
            .prepare_structured_text_with_work(&backend_request)
            .expect("combining-mark cluster should be shaped by the native backend");

        assert_eq!(work.coverage_input_bytes, "e\u{301}".len());
        assert_eq!(work.coverage_cache_insertions, 1);
        assert_eq!(work.face_inspections, 1);
    }

    #[test]
    fn native_prepared_text_charges_exact_structured_work_to_the_operation_meter() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default().with_font_stack(
            FontStack::new(["Excalifont", "Xiaolai"]).expect("fixture families are valid"),
        );
        let request = PrepareTextRequest::new("portable 图", typography).with_wrap(
            PreparedTextWrap::SvgLike {
                max_width_px: Some(64.0),
                break_long_words: true,
            },
        );
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

        layout
            .prepare_text_with_work_meter(&request, &meter)
            .expect("the operation meter should admit one complete native preparation");
        let one_label_work = meter.used();
        assert!(one_label_work > 0);
        layout
            .prepare_text_with_work_meter(&request, &meter)
            .expect("later labels should share the same operation meter");

        assert_eq!(meter.used(), one_label_work.saturating_mul(2));
    }

    #[test]
    fn request_digest_precharges_non_text_geometry_inputs() {
        let base_typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let base = PrepareTextRequest::new("", base_typography.clone());
        let metrics_typography = base_typography.with_font_stack(
            FontStack::new(["Excalifont", "Xiaolai"]).expect("fixture families are valid"),
        );
        let enriched = PrepareTextRequest::new("", metrics_typography.clone())
            .with_metrics_typography(metrics_typography)
            .with_script("Latn")
            .expect("Latn is a valid script tag")
            .with_language("en")
            .expect("en is a valid language tag")
            .with_features(["kern"])
            .expect("kern is a valid OpenType feature");
        let measure_digest = |request: &PrepareTextRequest| {
            let meter =
                OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
            request
                .digest_with_work_meter(Some(&meter))
                .expect("bounded request digest should be admitted");
            meter.used()
        };

        let base_work = measure_digest(&base);
        let enriched_work = measure_digest(&enriched);
        assert!(enriched_work > base_work);

        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, base_work)
            .expect("the measured base digest limit is valid");
        let meter = OperationWorkMeter::new(policy);
        assert_eq!(
            enriched
                .digest_with_work_meter(Some(&meter))
                .expect_err("non-text digest inputs must consume operation work"),
            TextLayoutError::LimitExceeded("operation_work")
        );
        assert!(meter.used() <= base_work);
    }

    #[test]
    fn structured_shaping_work_total_includes_every_bounded_lane() {
        let work = StructuredShapingWork {
            candidate_compilation_units: 65_536,
            coverage_input_bytes: 1,
            face_inspections: 2,
            coverage_cache_insertions: 4,
            coverage_cache_slots: 8,
            source_line_scan_bytes: 16,
            source_line_visits: 32,
            wrapped_line_emissions: 64,
            wrapping_input_bytes: 128,
            metrics_input_bytes: 256,
            wrapping_span_visits: 512,
            metrics_span_visits: 1_024,
            wrapping_line_ranges: 2_048,
            metrics_line_ranges: 4_096,
            glyph_visits: 8_192,
            cluster_visits: 16_384,
            wrap_boundary_visits: 32_768,
        };

        assert_eq!(work.total_units(), Ok(131_071));
    }

    #[test]
    fn native_prepared_text_rejects_an_operation_budget_below_exact_work() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let request =
            PrepareTextRequest::new("bounded", typography).with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(48.0),
                break_long_words: true,
            });
        let exact_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        layout
            .prepare_text_with_work_meter(&request, &exact_meter)
            .expect("the unbounded meter should measure one complete preparation");
        let exact = exact_meter.used();
        assert!(exact > 1);
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .expect("a positive narrow work limit is valid");
        let meter = OperationWorkMeter::new(policy);

        assert_eq!(
            layout
                .prepare_text_with_work_meter(&request, &meter)
                .expect_err("one work unit below the exact cost must reject"),
            TextLayoutError::LimitExceeded("operation_work")
        );
        assert!(
            meter.used() > 0 && meter.used() <= exact - 1,
            "completed shaping actions must remain charged when a later action is rejected"
        );
    }

    #[test]
    fn prepared_text_admission_work_scales_linearly_with_lines_and_runs() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measure_admission = |line_count: usize| {
            let text = (0..line_count).map(|_| "a").collect::<Vec<_>>().join("\n");
            let request = prepared_test_request(&text);
            let backend_request = backend_request_for(&layout, &request);
            let response = valid_raw_response(&backend_request);
            let meter =
                OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

            layout
                .primary_candidate()
                .admit_text_response(
                    layout.catalog(),
                    layout.contract_version(),
                    &backend_request,
                    response,
                    Some(&meter),
                )
                .expect("bounded multiline response should be admitted");
            meter.used()
        };

        let small = measure_admission(128);
        let large = measure_admission(256);
        let larger = measure_admission(384);
        let first_delta = large.saturating_sub(small);
        let second_delta = larger.saturating_sub(large);
        assert!(large > small);
        assert!(
            first_delta.abs_diff(second_delta) <= 16,
            "equal line/run increments must have linear admission cost: {small} -> {large} -> {larger}"
        );
    }

    #[test]
    fn native_right_to_left_shaping_preserves_projection_order() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let request = PrepareTextRequest::new("alpha beta", typography)
            .with_direction(TextLayoutDirection::RightToLeft)
            .with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(1_000.0),
                break_long_words: true,
            });
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

        let prepared = layout
            .prepare_text_with_work_meter(&request, &meter)
            .expect("right-to-left glyph clusters should admit in source projection order");

        assert_eq!(prepared.visible_text(), "alpha beta");
        assert_eq!(prepared.wrapped_lines().collect::<Vec<_>>(), ["alpha beta"]);
        assert_eq!(prepared.run_evidence().len(), 1);
        assert_eq!(
            prepared.run_evidence()[0].visible_range(),
            TextByteRange::new(0, "alpha beta".len())
        );
        assert!(meter.used() > 0);
    }

    #[test]
    fn native_operation_budget_stops_before_the_next_coverage_shape() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let request =
            PrepareTextRequest::new("a", typography).with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(32.0),
                break_long_words: true,
            });
        let backend_request = backend_request_for(&layout, &request);
        let (_, complete_work) = measurer
            .prepare_structured_text_with_work(&backend_request)
            .expect("fixture should expose deterministic native work");
        let admitted_before_coverage_shape = complete_work
            .candidate_compilation_units
            .saturating_add(complete_work.coverage_cache_slots)
            .saturating_add(complete_work.source_line_scan_bytes)
            .saturating_add(complete_work.source_line_visits)
            .saturating_add(complete_work.wrapping_line_ranges)
            .saturating_add(complete_work.wrapping_span_visits)
            .saturating_add(complete_work.face_inspections);
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(
                ResourceLimitId::MaxLayoutWorkUnits,
                admitted_before_coverage_shape,
            )
            .expect("the narrow shaping budget is valid");
        let meter = OperationWorkMeter::new(policy);

        let (attempt, work) =
            measurer.prepare_structured_text_attempt_with_work_meter(&backend_request, &meter);

        assert_eq!(
            attempt.expect_err("coverage shaping must not begin after its pre-charge is rejected"),
            TextLayoutError::LimitExceeded("operation_work")
        );
        assert_eq!(meter.used(), admitted_before_coverage_shape);
        assert_eq!(
            work.candidate_compilation_units,
            complete_work.candidate_compilation_units
        );
        assert_eq!(work.coverage_cache_slots, 2);
        assert_eq!(work.source_line_scan_bytes, 1);
        assert_eq!(work.source_line_visits, 1);
        assert_eq!(work.wrapping_line_ranges, 1);
        assert_eq!(work.wrapping_span_visits, 2);
        assert_eq!(work.face_inspections, 1);
        assert_eq!(work.coverage_input_bytes, 0);
        assert_eq!(work.coverage_cache_insertions, 0);
        assert_eq!(work.wrapping_input_bytes, 0);
        assert_eq!(work.metrics_input_bytes, 0);
        assert_eq!(work.metrics_line_ranges, 0);
    }

    #[test]
    fn failed_native_coverage_is_still_charged_to_the_operation_meter() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let request = PrepareTextRequest::new("图", typography);
        let backend_request = backend_request_for(&layout, &request);
        let (attempt, work) = measurer.prepare_structured_text_attempt(&backend_request);
        assert_eq!(
            attempt.expect_err("the Latin-only face does not cover the CJK witness"),
            TextLayoutError::GlyphUnavailable
        );
        let expected = work
            .total_units()
            .expect("bounded failed work should fit in usize")
            .saturating_add(metered_projection_and_digest_work(&request));
        assert!(expected > 0);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

        assert_eq!(
            layout
                .prepare_text_with_work_meter(&request, &meter)
                .expect_err("the missing glyph should remain the semantic error"),
            TextLayoutError::GlyphUnavailable
        );
        assert_eq!(meter.used(), expected);
    }

    #[test]
    fn legacy_native_prepare_entry_remains_unmetered_and_compatible() {
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));

        let prepared = layout
            .prepare_text(&PrepareTextRequest::new("compatible", typography))
            .expect("the compatibility entry should retain its existing behavior");

        assert!(prepared.metrics().width > 0.0);
    }

    #[test]
    fn resource_limit_errors_never_advance_to_another_backend_candidate() {
        assert!(!text_layout_error_allows_fallback(
            &TextLayoutError::LimitExceeded("operation_work")
        ));
    }

    #[test]
    fn native_scalar_coverage_confirms_cmap_candidates_with_shaping() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let request = PrepareTextRequest::new("A", typography);
        let backend_request = backend_request_for(&layout, &request);
        let (_, work) = measurer
            .prepare_structured_text_with_work(&backend_request)
            .expect("scalar coverage should be confirmed by shaping");

        assert_eq!(work.coverage_input_bytes, 1);
        assert_eq!(work.coverage_cache_insertions, 1);
        assert_eq!(work.face_inspections, 1);
    }

    #[test]
    fn native_coverage_cache_retains_one_face_decision_per_span() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default().with_font_stack(
            FontStack::new(["Excalifont", "Xiaolai"]).expect("fixture families are valid"),
        );
        let request = PrepareTextRequest::new("图", typography);
        let backend_request = backend_request_for(&layout, &request);
        let (_, work) = measurer
            .prepare_structured_text_with_work(&backend_request)
            .expect("CJK fallback should select the second catalog face");

        assert_eq!(work.face_inspections, 2);
        assert_eq!(work.coverage_cache_insertions, 1);
        assert_eq!(work.coverage_input_bytes, "图".len() * 2);
    }

    #[test]
    fn prepared_coverage_does_not_drop_non_breaking_or_preserved_whitespace() {
        let budget = PreparedTextAdmissionBudget::new(None);
        let projection = TextProjection::new("alpha\u{00a0}beta", ThemeTextTransform::None)
            .expect("projection should build");
        let vertical_extents =
            PreparedTextVerticalExtents::new(-0.8, 0.2).expect("fixed vertical extents are valid");
        let alpha = PreparedTextLine::new(
            "alpha",
            TextByteRange::new(0, 5),
            1.0,
            (0.0, 1.0),
            vertical_extents,
        )
        .expect("line should be valid");
        let beta_after_nbsp = PreparedTextLine::new(
            "beta",
            TextByteRange::new(7, 11),
            1.0,
            (0.0, 1.0),
            vertical_extents,
        )
        .expect("line should be valid");
        assert_eq!(
            validate_prepared_text_coverage(
                &projection,
                &[alpha.clone(), beta_after_nbsp],
                PreparedTextWrap::HtmlLike {
                    max_width_px: Some(10.0),
                },
                WhiteSpace::Normal,
                Some(10.0),
                &budget,
            ),
            Err(TextLayoutError::InvalidPreparedText)
        );

        let collapsible_projection = TextProjection::new("alpha beta", ThemeTextTransform::None)
            .expect("projection should build");
        let beta_after_space = PreparedTextLine::new(
            "beta",
            TextByteRange::new(6, 10),
            1.0,
            (0.0, 1.0),
            vertical_extents,
        )
        .expect("line should be valid");
        assert_eq!(
            validate_prepared_text_coverage(
                &collapsible_projection,
                &[alpha.clone(), beta_after_space.clone()],
                PreparedTextWrap::HtmlLike {
                    max_width_px: Some(10.0),
                },
                WhiteSpace::Normal,
                Some(10.0),
                &budget,
            ),
            Ok(())
        );
        assert_eq!(
            validate_prepared_text_coverage(
                &collapsible_projection,
                &[alpha, beta_after_space],
                PreparedTextWrap::HtmlLike {
                    max_width_px: Some(10.0),
                },
                WhiteSpace::PreWrap,
                Some(10.0),
                &budget,
            ),
            Err(TextLayoutError::InvalidPreparedText)
        );
    }

    #[test]
    fn native_wrapping_never_splits_a_transform_expansion_atom() {
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"))
            .with_transform(ThemeTextTransform::Uppercase);
        let atom_width = layout
            .prepare_text(&PrepareTextRequest::new("ß", typography.clone()))
            .expect("expanded atom probe should prepare")
            .computed_length_px();
        let prepared = layout
            .prepare_text(&PrepareTextRequest::new("ßßß", typography).with_wrap(
                PreparedTextWrap::SvgLike {
                    max_width_px: Some(atom_width + 0.01),
                    break_long_words: true,
                },
            ))
            .expect("expanded atoms should wrap at source-visible boundaries");

        assert_eq!(prepared.visible_text(), "SSSSSS");
        assert_eq!(
            prepared.wrapped_lines().collect::<Vec<_>>(),
            ["SS", "SS", "SS"]
        );
        assert!(prepared.lines().iter().all(|line| {
            let range = line.visible_range();
            range.start % 2 == 0 && range.end % 2 == 0
        }));
    }

    #[test]
    fn prepared_constructor_rejects_under_attested_capabilities_and_faces() {
        struct RejectingTextSession;

        impl PreparedTextBackendSession for RejectingTextSession {
            fn prepare_text(
                &self,
                _request: &PreparedTextBackendRequest,
            ) -> Result<PreparedTextResponse, TextLayoutError> {
                Err(TextLayoutError::BackendRejected)
            }
        }

        let catalog = mixed_catalog();
        let request =
            PrepareCatalogRequest::new(catalog.clone(), FontSourcePolicy::embedded_only());
        let backend =
            TextLayoutBackendIdentity::new("test.host", "v1").expect("test backend identity");
        let token = TextLayoutSessionToken::from_bytes([7; 16]).expect("nonzero token");
        let partial_faces = TextLayoutFaceEvidence::new([catalog.faces()[0].clone()])
            .expect("one face is valid evidence by itself");
        let session: Arc<dyn PreparedTextBackendSession> = Arc::new(RejectingTextSession);

        let partial_response = PreparedTextLayoutResponse::new(
            catalog.fingerprint(),
            TEXT_LAYOUT_CONTRACT_VERSION,
            backend.clone(),
            TextLayoutCapabilities::native(),
            FontSource::Embedded,
            partial_faces,
            token,
            session.clone(),
        )
        .expect("partial response can be decoded before request admission");
        let missing_face = PreparedTextLayout::admit_backend_response(
            &request,
            &backend,
            TextLayoutCapabilities::native(),
            partial_response,
        )
        .expect_err("partial loaded-face evidence must be rejected");
        assert_eq!(missing_face, TextLayoutError::LoadedFaceEvidenceMismatch);

        let mut capabilities = TextLayoutCapabilities::native();
        capabilities.supports_features = false;
        let all_faces = TextLayoutFaceEvidence::new(catalog.faces().iter().cloned())
            .expect("complete face evidence");
        let under_attested_response = PreparedTextLayoutResponse::new(
            catalog.fingerprint(),
            TEXT_LAYOUT_CONTRACT_VERSION,
            backend.clone(),
            capabilities,
            FontSource::Embedded,
            all_faces,
            token,
            session,
        )
        .expect("under-attested response can be decoded before request admission");
        let prepared = PreparedTextLayout::admit_backend_response(
            &request,
            &backend,
            TextLayoutCapabilities::native(),
            under_attested_response,
        )
        .expect("catalog admission does not depend on label-specific feature settings");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let text_request = PrepareTextRequest::new("portable", typography)
            .with_features(["kern"])
            .expect("kern is a valid OpenType feature");
        let missing_capability = prepared
            .prepare_text(&text_request)
            .expect_err("requested OpenType features require per-label capability evidence");
        assert_eq!(
            missing_capability,
            TextLayoutError::CapabilityNotAttested("features")
        );
    }

    #[test]
    fn preparation_rejects_unknown_contract_versions() {
        let catalog = mixed_catalog();
        let request = PrepareCatalogRequest::new(catalog, FontSourcePolicy::embedded_only())
            .with_contract_version(999);
        let error = NativeTextLayoutBackend::default()
            .prepare(&request)
            .expect_err("unknown protocol versions must not be silently accepted");
        assert_eq!(
            error,
            TextLayoutError::UnsupportedContract {
                requested: 999,
                supported: TEXT_LAYOUT_CONTRACT_VERSION,
            }
        );
    }
}
