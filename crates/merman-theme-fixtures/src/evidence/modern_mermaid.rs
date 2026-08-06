use crate::error::CatalogError;
use crate::io::{validate_hashed_file, verified_text};
use crate::model::{
    MODERN_MERMAID_CANVAS_LAYER_COUNTS, MODERN_MERMAID_REFERENCE_THEME_COUNT,
    MODERN_MERMAID_REFERENCE_THEMES, ReferenceBlendMode, ReferenceDiagramFamily,
    ReferenceGradientKind, ReferenceGradientRepetition, ReferenceThemeFacet,
    ReferenceThemeMechanism, SourceRecord,
};
use crate::wire::{ModernMermaidSnapshot, SnapshotEvidenceFile};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

// This digest is an intentional review gate for the manually decomposed capability corpus. It
// makes a coordinated snapshot/manifest edit visible in code review instead of silently
// accepting a new upstream interpretation.
const MODERN_MERMAID_SEMANTIC_MATRIX_SHA256: &str =
    "a8e0900c77dcf53bd6e9bb129cea465daccc4a8f2265243d8a2c30b85f99273f";

#[derive(Clone, Debug)]
pub(crate) struct ModernThemeEvidence {
    pub(crate) source_line: u32,
    pub(crate) canvas_layer_count: u8,
    pub(crate) mechanisms: BTreeSet<ReferenceThemeMechanism>,
    pub(crate) facets: BTreeSet<ReferenceThemeFacet>,
}

/// Loads a manually curated, hash-bound capability matrix.
///
/// The upstream TypeScript file is evidence for human review, not an input language that Merman
/// must execute or reimplement. `source_line` and the source file hash retain provenance while the
/// typed mechanisms and facets describe the Merman-owned model that was extracted from it.
pub(crate) fn load_modern_mermaid_evidence(
    root: &Path,
    source: &SourceRecord,
) -> Result<BTreeMap<String, ModernThemeEvidence>, CatalogError> {
    let json = verified_text(validate_hashed_file(
        root,
        &source.id,
        &source.snapshot_path,
        &source.snapshot_sha256,
    )?)?;
    let snapshot: ModernMermaidSnapshot =
        serde_json::from_str(&json).map_err(CatalogError::InvalidModernMermaidSnapshot)?;
    if snapshot.snapshot_version != 3
        || snapshot.source_id != source.id
        || snapshot.revision != source.revision
        || snapshot.notes.iter().any(|note| note.trim().is_empty())
    {
        return Err(CatalogError::ModernMermaidSnapshotMismatch);
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
        return Err(CatalogError::ModernMermaidSnapshotMismatch);
    }

    let expected_layer_counts = MODERN_MERMAID_CANVAS_LAYER_COUNTS
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let mut themes = BTreeMap::new();
    for theme in snapshot.themes {
        if theme.source_line == 0
            || expected_layer_counts.get(theme.reference_name.as_str())
                != Some(&theme.canvas_layer_count)
            || theme.mechanisms.is_empty()
            || theme
                .facets
                .iter()
                .any(|facet| !theme.mechanisms.contains(&facet.mechanism()))
            || !valid_value_facets(theme.canvas_layer_count, &theme.mechanisms, &theme.facets)
            || themes
                .insert(
                    theme.reference_name,
                    ModernThemeEvidence {
                        source_line: theme.source_line,
                        canvas_layer_count: theme.canvas_layer_count,
                        mechanisms: theme.mechanisms,
                        facets: theme.facets,
                    },
                )
                .is_some()
        {
            return Err(CatalogError::ModernMermaidSnapshotMismatch);
        }
    }

    let expected = MODERN_MERMAID_REFERENCE_THEMES
        .into_iter()
        .collect::<BTreeSet<_>>();
    let actual = themes.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if themes.len() != MODERN_MERMAID_REFERENCE_THEME_COUNT || actual != expected {
        return Err(CatalogError::ReferenceThemeSetMismatch {
            expected: expected.into_iter().map(str::to_string).collect(),
            actual: actual.into_iter().map(str::to_string).collect(),
        });
    }
    if semantic_matrix_sha256(&themes) != MODERN_MERMAID_SEMANTIC_MATRIX_SHA256 {
        return Err(CatalogError::ModernMermaidSnapshotMismatch);
    }
    Ok(themes)
}

fn valid_value_facets(
    canvas_layer_count: u8,
    mechanisms: &BTreeSet<ReferenceThemeMechanism>,
    facets: &BTreeSet<ReferenceThemeFacet>,
) -> bool {
    if facets.iter().any(|facet| {
        matches!(
            facet,
            ReferenceThemeFacet::SemanticSelector { mechanism, .. }
                if !matches!(
                    mechanism,
                    ReferenceThemeMechanism::HasSelector
                        | ReferenceThemeMechanism::NotSelector
                )
        )
    }) {
        return false;
    }

    let blend_facets = facets
        .iter()
        .filter(|facet| matches!(facet, ReferenceThemeFacet::CanvasBlend { .. }))
        .count();
    if mechanisms.contains(&ReferenceThemeMechanism::CanvasBlend) != (blend_facets == 1)
        || blend_facets == 1 && canvas_layer_count == 0
    {
        return false;
    }

    let gradient_facets = facets
        .iter()
        .filter(|facet| matches!(facet, ReferenceThemeFacet::CanvasGradient { .. }))
        .collect::<Vec<_>>();
    if mechanisms.contains(&ReferenceThemeMechanism::CanvasGradient) != (canvas_layer_count > 0)
        || mechanisms.contains(&ReferenceThemeMechanism::CanvasGradient)
            == gradient_facets.is_empty()
        || mechanisms.contains(&ReferenceThemeMechanism::CanvasLayering) != (canvas_layer_count > 1)
    {
        return false;
    }

    let has_pattern_facet = gradient_facets.iter().any(|facet| {
        matches!(
            facet,
            ReferenceThemeFacet::CanvasGradient {
                repetition: ReferenceGradientRepetition::Repeating
                    | ReferenceGradientRepetition::Tiled,
                ..
            }
        )
    });
    if mechanisms.contains(&ReferenceThemeMechanism::CanvasPattern) != has_pattern_facet {
        return false;
    }

    for mechanism in [
        ReferenceThemeMechanism::HasSelector,
        ReferenceThemeMechanism::NotSelector,
    ] {
        let has_facet = facets.iter().any(|facet| {
            matches!(
                facet,
                ReferenceThemeFacet::SemanticSelector {
                    mechanism: facet_mechanism,
                    ..
                } if *facet_mechanism == mechanism
            )
        });
        if mechanisms.contains(&mechanism) != has_facet {
            return false;
        }
    }
    true
}

fn semantic_matrix_sha256(themes: &BTreeMap<String, ModernThemeEvidence>) -> String {
    let mut digest = Sha256::new();
    digest.update(b"merman-modern-mermaid-semantic-matrix-v1\0");
    digest.update(
        u32::try_from(themes.len())
            .expect("closed theme count fits u32")
            .to_be_bytes(),
    );
    for (name, theme) in themes {
        update_length_prefixed(&mut digest, name.as_bytes());
        digest.update(theme.source_line.to_be_bytes());
        digest.update([theme.canvas_layer_count]);
        digest.update(
            u16::try_from(theme.mechanisms.len())
                .expect("closed mechanism count fits u16")
                .to_be_bytes(),
        );
        for mechanism in &theme.mechanisms {
            digest.update([mechanism_code(*mechanism)]);
        }
        digest.update(
            u16::try_from(theme.facets.len())
                .expect("closed facet count fits u16")
                .to_be_bytes(),
        );
        for facet in &theme.facets {
            match facet {
                ReferenceThemeFacet::CanvasBlend { mode } => {
                    digest.update([0, blend_mode_code(*mode)]);
                }
                ReferenceThemeFacet::CanvasGradient {
                    gradient,
                    repetition,
                } => {
                    digest.update([
                        1,
                        gradient_kind_code(*gradient),
                        gradient_repetition_code(*repetition),
                    ]);
                }
                ReferenceThemeFacet::SemanticSelector { mechanism, family } => {
                    digest.update([2, mechanism_code(*mechanism), diagram_family_code(*family)]);
                }
            }
        }
    }
    format!("{:x}", digest.finalize())
}

fn update_length_prefixed(digest: &mut Sha256, value: &[u8]) {
    digest.update(
        u32::try_from(value.len())
            .expect("closed evidence string length fits u32")
            .to_be_bytes(),
    );
    digest.update(value);
}

const fn mechanism_code(mechanism: ReferenceThemeMechanism) -> u8 {
    match mechanism {
        ReferenceThemeMechanism::BackdropFilter => 0,
        ReferenceThemeMechanism::CanvasBlend => 1,
        ReferenceThemeMechanism::CanvasGradient => 2,
        ReferenceThemeMechanism::CanvasLayering => 3,
        ReferenceThemeMechanism::CanvasPattern => 4,
        ReferenceThemeMechanism::CanvasSolid => 5,
        ReferenceThemeMechanism::CssFilter => 6,
        ReferenceThemeMechanism::CssLetterSpacing => 7,
        ReferenceThemeMechanism::CssTextTransform => 8,
        ReferenceThemeMechanism::DashArray => 9,
        ReferenceThemeMechanism::ExternalSvgFilterReference => 10,
        ReferenceThemeMechanism::FontStack => 11,
        ReferenceThemeMechanism::HasSelector => 12,
        ReferenceThemeMechanism::NotSelector => 13,
        ReferenceThemeMechanism::NthChildSelector => 14,
        ReferenceThemeMechanism::RoundedCorners => 15,
        ReferenceThemeMechanism::StrokeStyling => 16,
        ReferenceThemeMechanism::ThemeVariables => 17,
    }
}

const fn blend_mode_code(mode: ReferenceBlendMode) -> u8 {
    match mode {
        ReferenceBlendMode::Multiply => 0,
        ReferenceBlendMode::Overlay => 1,
        ReferenceBlendMode::Screen => 2,
    }
}

const fn gradient_kind_code(kind: ReferenceGradientKind) -> u8 {
    match kind {
        ReferenceGradientKind::Linear => 0,
        ReferenceGradientKind::Radial => 1,
    }
}

const fn gradient_repetition_code(repetition: ReferenceGradientRepetition) -> u8 {
    match repetition {
        ReferenceGradientRepetition::None => 0,
        ReferenceGradientRepetition::Repeating => 1,
        ReferenceGradientRepetition::Tiled => 2,
    }
}

const fn diagram_family_code(family: ReferenceDiagramFamily) -> u8 {
    match family {
        ReferenceDiagramFamily::ClassDiagram => 0,
        ReferenceDiagramFamily::ErDiagram => 1,
        ReferenceDiagramFamily::Flowchart => 2,
        ReferenceDiagramFamily::StateDiagram => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_facet_closure_rejects_independent_canvas_mismatches() {
        assert!(valid_value_facets(0, &BTreeSet::new(), &BTreeSet::new()));

        let gradient = ReferenceThemeFacet::CanvasGradient {
            gradient: ReferenceGradientKind::Linear,
            repetition: ReferenceGradientRepetition::None,
        };
        assert!(!valid_value_facets(
            2,
            &BTreeSet::from([ReferenceThemeMechanism::CanvasGradient]),
            &BTreeSet::from([gradient]),
        ));
        assert!(!valid_value_facets(
            1,
            &BTreeSet::from([
                ReferenceThemeMechanism::CanvasGradient,
                ReferenceThemeMechanism::CanvasPattern,
            ]),
            &BTreeSet::from([gradient]),
        ));
        assert!(!valid_value_facets(
            0,
            &BTreeSet::from([ReferenceThemeMechanism::CanvasBlend]),
            &BTreeSet::new(),
        ));
    }
}
