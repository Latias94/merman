use std::collections::HashSet;
use std::sync::Arc;

use sha2::{Digest as _, Sha256};

use crate::diagram_theme::{FontAssetFingerprint, FontCatalogFingerprint, FontSource};

use super::prepared::{
    PreparedTextLabelEvidence, PreparedTextLabelId, PreparedTextLabelLedgerEntry,
    PreparedTextLabelProvenance, SourceVisibleSpan, TextByteRange,
};

const ARTIFACT_DOMAIN: &[u8] = b"merman-prepared-text-terminal-artifact-v1";
const LABEL_DOMAIN: &[u8] = b"merman-prepared-text-terminal-label-v1";

/// Final face/source identity expected for one prepared label.
///
/// This workspace-private type is intentionally smaller than the retained shaping ledger. Native
/// exporters use it only to classify the face selected by the terminal SVG parser.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreparedTextTerminalFace {
    asset_fingerprint: FontAssetFingerprint,
    face_index: u32,
    source: FontSource,
}

impl PreparedTextTerminalFace {
    pub const fn asset_fingerprint(self) -> FontAssetFingerprint {
        self.asset_fingerprint
    }

    pub const fn face_index(self) -> u32 {
        self.face_index
    }

    pub const fn source(self) -> FontSource {
        self.source
    }
}

/// Opaque per-label terminal expectation frozen from one admitted prepared result.
///
/// The identity digest binds the request digest, source/visible projection, final line ranges,
/// renderer-owned line text, and ordered shaping runs. The native exporter compares the exposed
/// line text and face summary with the parsed tokenized SVG; exact source/visible range observation
/// remains a later multi-face proof boundary.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTextTerminalLabelReceipt {
    id: PreparedTextLabelId,
    catalog_fingerprint: FontCatalogFingerprint,
    provenance: PreparedTextLabelProvenance,
    line_texts: Arc<[Arc<str>]>,
    faces: Arc<[PreparedTextTerminalFace]>,
    identity_digest: [u8; 32],
}

impl PreparedTextTerminalLabelReceipt {
    fn from_ledger_entry(entry: &PreparedTextLabelLedgerEntry) -> Option<Self> {
        let line_texts = entry.line_texts();
        if line_texts.is_empty()
            || line_texts.len() != entry.line_ranges().len()
            || entry.evidence().is_empty()
        {
            return None;
        }
        // A Native receipt is the only path that can participate in a Portable proof. A native
        // record carrying a system face violates that invariant and must fail closed at sealing,
        // rather than being reclassified as a seemingly complete single-face receipt.
        if entry.provenance() == PreparedTextLabelProvenance::Native
            && entry
                .evidence()
                .iter()
                .any(|run| run.font_source() != FontSource::Embedded)
        {
            return None;
        }
        let (source_extent, visible_extent) = validate_projection_order(entry.projection_spans())?;
        validate_range_order(entry.line_ranges(), true, visible_extent)?;
        validate_run_order(entry.evidence(), source_extent, visible_extent)?;

        let mut faces = entry
            .evidence()
            .iter()
            .map(|run| PreparedTextTerminalFace {
                asset_fingerprint: run.face_key().asset_fingerprint(),
                face_index: run.face_key().face_index(),
                source: run.font_source(),
            })
            .collect::<Vec<_>>();
        faces.sort_unstable();
        faces.dedup();
        if faces.is_empty() {
            return None;
        }

        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, LABEL_DOMAIN);
        update_len_prefixed(&mut hasher, entry.id().family().as_bytes());
        hasher.update(entry.id().key().to_le_bytes());
        hasher.update(entry.catalog_fingerprint().as_bytes());
        hasher.update(entry.request_digest().as_bytes());
        hasher.update([provenance_id(entry.provenance())]);
        update_len(&mut hasher, entry.projection_spans().len())?;
        for span in entry.projection_spans() {
            update_range(&mut hasher, span.source())?;
            update_range(&mut hasher, span.visible())?;
        }
        update_len(&mut hasher, entry.line_ranges().len())?;
        for (range, text) in entry.line_ranges().iter().zip(line_texts) {
            update_range(&mut hasher, *range)?;
            update_len_prefixed(&mut hasher, text.as_bytes());
        }
        update_len(&mut hasher, entry.evidence().len())?;
        for run in entry.evidence() {
            update_range(&mut hasher, run.source_range())?;
            update_range(&mut hasher, run.visible_range())?;
            hasher.update(run.face_key().asset_fingerprint().as_bytes());
            hasher.update(run.face_key().face_index().to_le_bytes());
            update_len_prefixed(&mut hasher, run.font_source().id().as_bytes());
        }

        Some(Self {
            id: entry.id(),
            catalog_fingerprint: entry.catalog_fingerprint(),
            provenance: entry.provenance(),
            line_texts: line_texts.to_vec().into(),
            faces: faces.into(),
            identity_digest: hasher.finalize().into(),
        })
    }

    pub const fn id(&self) -> PreparedTextLabelId {
        self.id
    }

    pub const fn catalog_fingerprint(&self) -> FontCatalogFingerprint {
        self.catalog_fingerprint
    }

    pub const fn provenance(&self) -> PreparedTextLabelProvenance {
        self.provenance
    }

    pub fn line_texts(&self) -> impl ExactSizeIterator<Item = &str> {
        self.line_texts.iter().map(AsRef::as_ref)
    }

    pub fn faces(&self) -> &[PreparedTextTerminalFace] {
        &self.faces
    }

    /// Returns whether the current native exporter can prove this label without inferring
    /// fallback ranges from glyph callback order.
    pub fn native_terminal_proof_is_complete(&self) -> bool {
        self.provenance == PreparedTextLabelProvenance::Native
            && self.faces.len() == 1
            && self.faces[0].source() == FontSource::Embedded
    }

    pub fn native_terminal_proof_is_incomplete(&self) -> bool {
        self.provenance == PreparedTextLabelProvenance::Native && self.faces.len() > 1
    }

    #[cfg(test)]
    pub(crate) const fn identity_digest(&self) -> &[u8; 32] {
        &self.identity_digest
    }
}

/// Renderer-owned receipt binding a tokenized SVG artifact to its prepared-text ledger.
///
/// The exporter independently hashes the exact SVG it parses and reconstructs terminal line and
/// face observations from the resolved tree. The retained request and ordered range details remain
/// hidden behind each label's identity digest.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTextTerminalReceipt {
    artifact_digest: [u8; 32],
    labels: Arc<[PreparedTextTerminalLabelReceipt]>,
}

impl PreparedTextTerminalReceipt {
    pub(crate) fn from_ledger(
        tokenized_svg: &str,
        ledger: &[PreparedTextLabelLedgerEntry],
    ) -> Option<Self> {
        if ledger.is_empty() {
            return None;
        }
        let mut ids = HashSet::with_capacity(ledger.len());
        let mut labels = Vec::with_capacity(ledger.len());
        for entry in ledger {
            if !ids.insert(entry.id()) {
                return None;
            }
            labels.push(PreparedTextTerminalLabelReceipt::from_ledger_entry(entry)?);
        }

        Some(Self {
            artifact_digest: artifact_digest(tokenized_svg),
            labels: labels.into(),
        })
    }

    pub fn artifact_matches(&self, tokenized_svg: &str) -> bool {
        self.artifact_digest == artifact_digest(tokenized_svg)
    }

    pub fn labels(&self) -> &[PreparedTextTerminalLabelReceipt] {
        &self.labels
    }
}

fn artifact_digest(svg: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, ARTIFACT_DOMAIN);
    update_len_prefixed(&mut hasher, svg.as_bytes());
    hasher.finalize().into()
}

fn validate_projection_order(spans: &[SourceVisibleSpan]) -> Option<(usize, usize)> {
    let mut source_end = 0usize;
    let mut visible_end = 0usize;
    for span in spans {
        let source = span.source();
        let visible = span.visible();
        if source.start() != source_end
            || visible.start() != visible_end
            || source.start() > source.end()
            || visible.start() > visible.end()
            || (source.start() == source.end() && visible.start() == visible.end())
        {
            return None;
        }
        source_end = source.end();
        visible_end = visible.end();
    }
    Some((source_end, visible_end))
}

fn validate_range_order(ranges: &[TextByteRange], allow_empty: bool, extent: usize) -> Option<()> {
    let mut previous_end = 0usize;
    for range in ranges {
        if range.start() < previous_end
            || range.start() > range.end()
            || range.end() > extent
            || (!allow_empty && range.start() == range.end())
        {
            return None;
        }
        previous_end = range.end();
    }
    Some(())
}

fn validate_run_order(
    runs: &[PreparedTextLabelEvidence],
    source_extent: usize,
    visible_extent: usize,
) -> Option<()> {
    let source_ranges = runs.iter().map(PreparedTextLabelEvidence::source_range);
    let visible_ranges = runs.iter().map(PreparedTextLabelEvidence::visible_range);
    validate_range_order(&source_ranges.collect::<Vec<_>>(), false, source_extent)?;
    validate_range_order(&visible_ranges.collect::<Vec<_>>(), false, visible_extent)
}

fn provenance_id(provenance: PreparedTextLabelProvenance) -> u8 {
    match provenance {
        PreparedTextLabelProvenance::Native => 0,
        PreparedTextLabelProvenance::HostDependent => 1,
    }
}

fn update_len(hasher: &mut Sha256, len: usize) -> Option<()> {
    hasher.update(u64::try_from(len).ok()?.to_le_bytes());
    Some(())
}

fn update_range(hasher: &mut Sha256, range: TextByteRange) -> Option<()> {
    hasher.update(u64::try_from(range.start()).ok()?.to_le_bytes());
    hasher.update(u64::try_from(range.end()).ok()?.to_le_bytes());
    Some(())
}

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}
