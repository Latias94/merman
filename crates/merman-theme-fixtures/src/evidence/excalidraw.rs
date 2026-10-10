use crate::error::CatalogError;
use crate::io::{
    parse_unicode_codepoint, unicode_range_contains, validate_hashed_file, verified_text,
};
use crate::model::{AssetRecord, FixtureRecord, SourceRecord};
use crate::wire::{ExcalidrawFontProvenanceSnapshot, SnapshotEvidenceFile};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) fn validate_excalidraw_font_snapshot(
    root: &Path,
    source: &SourceRecord,
    assets: &[AssetRecord],
    fixtures: &[FixtureRecord],
) -> Result<(), CatalogError> {
    let json = verified_text(validate_hashed_file(
        root,
        &source.id,
        &source.snapshot_path,
        &source.snapshot_sha256,
    )?)?;
    let snapshot: ExcalidrawFontProvenanceSnapshot =
        serde_json::from_str(&json).map_err(CatalogError::InvalidExcalidrawFontSnapshot)?;
    if snapshot.snapshot_version != 1
        || snapshot.source_id != source.id
        || snapshot.revision != source.revision
        || snapshot.license != "MIT"
    {
        return Err(CatalogError::ExcalidrawFontSnapshotMismatch);
    }
    let expected_evidence = source
        .evidence
        .iter()
        .map(|record| SnapshotEvidenceFile {
            path: record.source_path.clone(),
            sha256: record.sha256.clone(),
        })
        .collect::<BTreeSet<_>>();
    if snapshot.evidence_files.into_iter().collect::<BTreeSet<_>>() != expected_evidence {
        return Err(CatalogError::ExcalidrawFontSnapshotMismatch);
    }

    let mut snapshot_fonts = BTreeMap::new();
    for font in snapshot.fonts {
        if snapshot_fonts.insert(font.asset_id.clone(), font).is_some() {
            return Err(CatalogError::ExcalidrawFontSnapshotMismatch);
        }
    }
    let source_assets = assets
        .iter()
        .filter(|asset| asset.source_id == source.id)
        .collect::<Vec<_>>();
    if snapshot_fonts.len() != source_assets.len() {
        return Err(CatalogError::ExcalidrawFontSnapshotMismatch);
    }
    for asset in source_assets {
        let font = snapshot_fonts
            .get(&asset.id)
            .ok_or(CatalogError::ExcalidrawFontSnapshotMismatch)?;
        if font.source_path != asset.source_path
            || font.sha256 != asset.source_sha256
            || font.family != asset.canonical_family
            || font.runtime_family != asset.family
            || font.unicode_range != asset.unicode_range
            || font.license != "OFL-1.1"
            || font.fixture_codepoints.iter().any(|codepoint| {
                parse_unicode_codepoint(codepoint).is_none_or(|value| {
                    !unicode_range_contains(&asset.unicode_range, value)
                        .expect("asset unicode ranges are validated before snapshots")
                        || !fixtures.iter().any(|fixture| {
                            fixture.asset_ids.contains(&asset.id)
                                && fixture.expectation.visible_text.iter().any(|sample| {
                                    sample
                                        .chars()
                                        .any(|character| u32::from(character) == value)
                                })
                        })
                })
            })
        {
            return Err(CatalogError::ExcalidrawFontSnapshotMismatch);
        }
    }
    Ok(())
}
