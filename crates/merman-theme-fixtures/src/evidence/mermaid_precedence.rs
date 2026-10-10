use crate::error::CatalogError;
use crate::io::{validate_hashed_file, validate_id, verified_text};
use crate::model::{
    MERMAID_STYLE_PRECEDENCE_ENTRY_IDS, MermaidStyleAdmission, MermaidStyleFamily,
    MermaidStylePlane, MermaidStyleProperty, SourceRecord, StylePrecedenceEvidence,
};
use crate::wire::{
    MermaidStylePrecedenceSnapshot, SnapshotEvidenceFile, StylePrecedenceEvidenceWire,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) fn load_mermaid_style_precedence(
    root: &Path,
    source: &SourceRecord,
) -> Result<Vec<StylePrecedenceEvidence>, CatalogError> {
    let json = verified_text(validate_hashed_file(
        root,
        &source.id,
        &source.snapshot_path,
        &source.snapshot_sha256,
    )?)?;
    let snapshot: MermaidStylePrecedenceSnapshot =
        serde_json::from_str(&json).map_err(CatalogError::InvalidMermaidStylePrecedenceSnapshot)?;
    let expected_evidence = source
        .evidence
        .iter()
        .map(|record| SnapshotEvidenceFile {
            path: record.source_path.clone(),
            sha256: record.sha256.clone(),
        })
        .collect::<BTreeSet<_>>();
    let actual_evidence = snapshot.evidence_files.into_iter().collect::<BTreeSet<_>>();
    if snapshot.snapshot_version != 1
        || snapshot.source_id != source.id
        || snapshot.revision != source.revision
        || actual_evidence != expected_evidence
    {
        return Err(CatalogError::MermaidStylePrecedenceSnapshotMismatch);
    }

    let evidence_paths = source
        .evidence
        .iter()
        .map(|record| record.source_path.clone())
        .collect::<BTreeSet<_>>();
    let mut referenced_evidence_paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut entries = Vec::with_capacity(snapshot.entries.len());
    for entry in snapshot.entries {
        validate_id(&entry.id)?;
        if !ids.insert(entry.id.clone())
            || entry.note.trim().is_empty()
            || entry.evidence.is_empty()
            || (entry.rank.is_some() == entry.encounter_order)
            || entry
                .evidence
                .iter()
                .any(|reference| !valid_evidence_reference(reference, &evidence_paths))
            || !valid_style_precedence_entry(&entry)
        {
            return Err(CatalogError::MermaidStylePrecedenceSnapshotMismatch);
        }
        for reference in &entry.evidence {
            let Some((path, _)) = parse_evidence_reference(reference) else {
                return Err(CatalogError::MermaidStylePrecedenceSnapshotMismatch);
            };
            referenced_evidence_paths.insert(path.to_string());
        }
        entries.push(convert_entry(entry));
    }
    let expected_ids = MERMAID_STYLE_PRECEDENCE_ENTRY_IDS
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if ids != expected_ids || referenced_evidence_paths != evidence_paths {
        return Err(CatalogError::MermaidStylePrecedenceSnapshotMismatch);
    }
    validate_rank_groups(&entries)?;
    Ok(entries)
}

pub(crate) fn parse_evidence_reference(reference: &str) -> Option<(&str, Vec<(usize, usize)>)> {
    let (path, ranges) = reference.split_once(':')?;
    if ranges.is_empty() {
        return None;
    }
    let ranges = ranges
        .split(',')
        .map(|range| {
            let (start, end) = range.split_once('-')?;
            let Ok(start) = start.parse::<usize>() else {
                return None;
            };
            let Ok(end) = end.parse::<usize>() else {
                return None;
            };
            (start > 0 && start <= end).then_some((start, end))
        })
        .collect::<Option<Vec<_>>>()?;
    Some((path, ranges))
}

fn valid_evidence_reference(reference: &str, evidence_paths: &BTreeSet<String>) -> bool {
    parse_evidence_reference(reference).is_some_and(|(path, _)| evidence_paths.contains(path))
}

fn validate_rank_groups(entries: &[StylePrecedenceEvidence]) -> Result<(), CatalogError> {
    let mut groups = BTreeMap::<_, Vec<u16>>::new();
    for entry in entries {
        if let Some(rank) = entry.rank {
            groups
                .entry((entry.family, entry.property, entry.plane))
                .or_default()
                .push(rank);
        }
    }
    for mut ranks in groups.into_values() {
        ranks.sort_unstable();
        if ranks
            .iter()
            .copied()
            .ne((0..ranks.len()).map(|rank| rank as u16))
        {
            return Err(CatalogError::MermaidStylePrecedenceSnapshotMismatch);
        }
    }
    Ok(())
}

fn convert_entry(entry: StylePrecedenceEvidenceWire) -> StylePrecedenceEvidence {
    StylePrecedenceEvidence {
        id: entry.id,
        family: entry.family,
        origin: entry.origin,
        property: entry.property,
        plane: entry.plane,
        admission: entry.admission,
        rank: entry.rank,
        encounter_order: entry.encounter_order,
        evidence: entry.evidence,
        note: entry.note,
    }
}

fn valid_style_precedence_entry(entry: &StylePrecedenceEvidenceWire) -> bool {
    let Some((family, origin, property, plane, admission, rank, encounter_order)) =
        expected_style_precedence_tuple(&entry.id)
    else {
        return false;
    };
    if (
        entry.family,
        entry.origin,
        entry.property,
        entry.plane,
        entry.admission,
        entry.rank,
        entry.encounter_order,
    ) != (
        family,
        origin,
        property,
        plane,
        admission,
        rank,
        encounter_order,
    ) {
        return false;
    }
    match entry.plane {
        MermaidStylePlane::Config => {
            entry.family == MermaidStyleFamily::Global
                && entry.property == MermaidStyleProperty::EffectiveConfig
                && entry.admission == MermaidStyleAdmission::Typed
                && entry.rank.is_some()
        }
        MermaidStylePlane::CssCascade => {
            entry.property == MermaidStyleProperty::StylesheetPaint
                && matches!(
                    entry.admission,
                    MermaidStyleAdmission::PaintOnly | MermaidStyleAdmission::SourceCompatibility
                )
                && entry.rank.is_some()
        }
        MermaidStylePlane::PaintOnly => {
            entry.family == MermaidStyleFamily::ErDiagram
                && entry.property == MermaidStyleProperty::NodePaint
                && entry.admission == MermaidStyleAdmission::PaintOnly
                && entry.rank.is_some()
        }
        MermaidStylePlane::PreLayout => {
            entry.property != MermaidStyleProperty::StylesheetPaint
                && entry.property != MermaidStyleProperty::EffectiveConfig
                && matches!(
                    entry.admission,
                    MermaidStyleAdmission::Typed | MermaidStyleAdmission::UnverifiedResidual
                )
        }
    }
}

type StylePrecedenceTuple = (
    MermaidStyleFamily,
    crate::model::MermaidStyleOrigin,
    MermaidStyleProperty,
    MermaidStylePlane,
    MermaidStyleAdmission,
    Option<u16>,
    bool,
);

fn expected_style_precedence_tuple(id: &str) -> Option<StylePrecedenceTuple> {
    use crate::model::MermaidStyleOrigin::*;
    use MermaidStyleAdmission::*;
    use MermaidStyleFamily::*;
    use MermaidStylePlane::*;
    use MermaidStyleProperty::*;

    Some(match id {
        "class-assignment-before-definition-copy" => (
            ClassDiagram,
            ClassDefNamed,
            NodePaint,
            PreLayout,
            Typed,
            None,
            true,
        ),
        "class-definition-before-assignment-no-backfill" => (
            ClassDiagram,
            AssignedClass,
            NodePaint,
            PreLayout,
            Typed,
            None,
            true,
        ),
        "class-node-inline" => (
            ClassDiagram,
            InlineStyle,
            NodePaint,
            PreLayout,
            Typed,
            None,
            true,
        ),
        "class-typography-classdef-residual" => (
            ClassDiagram,
            ClassDefNamed,
            NodeTypography,
            PreLayout,
            UnverifiedResidual,
            None,
            true,
        ),
        "flow-edge-default" => (
            Flowchart,
            LinkStyleDefault,
            EdgePaint,
            PreLayout,
            Typed,
            Some(0),
            false,
        ),
        "flow-edge-specific" => (
            Flowchart,
            LinkStyleSpecific,
            EdgePaint,
            PreLayout,
            Typed,
            Some(1),
            false,
        ),
        "flow-node-assigned" => (
            Flowchart,
            AssignedClass,
            NodePaint,
            PreLayout,
            Typed,
            Some(3),
            false,
        ),
        "flow-node-default" => (
            Flowchart,
            ClassDefDefault,
            NodePaint,
            PreLayout,
            Typed,
            Some(1),
            false,
        ),
        "flow-node-inline" => (
            Flowchart,
            InlineStyle,
            NodePaint,
            PreLayout,
            Typed,
            Some(4),
            false,
        ),
        "flow-node-named" => (
            Flowchart,
            ClassDefNamed,
            NodePaint,
            PreLayout,
            Typed,
            Some(2),
            false,
        ),
        "flow-node-theme" => (
            Flowchart,
            ThemeVariables,
            NodePaint,
            PreLayout,
            Typed,
            Some(0),
            false,
        ),
        "global-config-frontmatter" => (
            Global,
            Frontmatter,
            EffectiveConfig,
            Config,
            Typed,
            Some(0),
            false,
        ),
        "global-config-init" => (
            Global,
            InitDirective,
            EffectiveConfig,
            Config,
            Typed,
            Some(1),
            false,
        ),
        "global-css-classdef" => (
            Global,
            GeneratedClassCss,
            StylesheetPaint,
            CssCascade,
            MermaidStyleAdmission::PaintOnly,
            Some(2),
            false,
        ),
        "global-css-family" => (
            Global,
            FamilyThemeCss,
            StylesheetPaint,
            CssCascade,
            MermaidStyleAdmission::PaintOnly,
            Some(0),
            false,
        ),
        "global-css-family-typography-residual" => (
            Global,
            FamilyThemeCss,
            StylesheetTypography,
            PreLayout,
            UnverifiedResidual,
            Some(0),
            false,
        ),
        "global-css-theme" => (
            Global,
            ThemeCss,
            StylesheetPaint,
            CssCascade,
            SourceCompatibility,
            Some(1),
            false,
        ),
        "state-node-assigned" => (
            StateDiagram,
            AssignedClass,
            NodePaint,
            PreLayout,
            Typed,
            Some(1),
            false,
        ),
        "state-node-inline" => (
            StateDiagram,
            InlineStyle,
            NodePaint,
            PreLayout,
            Typed,
            Some(2),
            false,
        ),
        "state-node-theme" => (
            StateDiagram,
            ThemeVariables,
            NodePaint,
            PreLayout,
            Typed,
            Some(0),
            false,
        ),
        "er-node-default" => (
            ErDiagram,
            ClassDefDefault,
            NodePaint,
            MermaidStylePlane::PaintOnly,
            MermaidStyleAdmission::PaintOnly,
            Some(0),
            false,
        ),
        "er-node-assigned" => (
            ErDiagram,
            AssignedClass,
            NodePaint,
            MermaidStylePlane::PaintOnly,
            MermaidStyleAdmission::PaintOnly,
            Some(1),
            false,
        ),
        "er-node-inline" => (
            ErDiagram,
            InlineStyle,
            NodePaint,
            MermaidStylePlane::PaintOnly,
            MermaidStyleAdmission::PaintOnly,
            Some(2),
            false,
        ),
        "er-typography-default-residual" => (
            ErDiagram,
            ClassDefDefault,
            NodeTypography,
            PreLayout,
            UnverifiedResidual,
            Some(0),
            false,
        ),
        "er-typography-assigned-residual" => (
            ErDiagram,
            AssignedClass,
            NodeTypography,
            PreLayout,
            UnverifiedResidual,
            Some(1),
            false,
        ),
        "er-typography-inline-residual" => (
            ErDiagram,
            InlineStyle,
            NodeTypography,
            PreLayout,
            UnverifiedResidual,
            Some(2),
            false,
        ),
        _ => return None,
    })
}
