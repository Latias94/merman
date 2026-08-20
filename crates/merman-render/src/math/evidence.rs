use std::sync::Arc;

use sha2::{Digest as _, Sha256};

use crate::DiagramFamilyId;
use crate::resources::PreparedTextRetainedReservation;

const OCCURRENCE_KEY_DOMAIN: &[u8] = b"merman-prepared-math-occurrence-v1";
const PROJECTION_DOMAIN: &[u8] = b"merman-prepared-math-native-projection-v1";
pub(crate) const PREPARED_MATH_NATIVE_CLASS: &str = "merman-prepared-math-native";
pub(crate) const PREPARED_MATH_NATIVE_CLASS_ATTRIBUTE: &str =
    r#"class="merman-prepared-math-native""#;

/// Renderer-owned identity for one semantic math-label occurrence.
///
/// The value is safe to emit as an XML attribute and is never recovered from user-authored SVG.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PreparedMathOccurrenceId(Arc<str>);

impl PreparedMathOccurrenceId {
    pub(crate) fn indexed(family_id: DiagramFamilyId, role: &'static str, index: usize) -> Self {
        Self(Arc::from(format!("{}/{role}/{index}", family_id.as_str())))
    }

    pub(crate) fn keyed(family_id: DiagramFamilyId, role: &'static str, key: &str) -> Self {
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, OCCURRENCE_KEY_DOMAIN);
        update_len_prefixed(&mut hasher, family_id.as_str().as_bytes());
        update_len_prefixed(&mut hasher, role.as_bytes());
        update_len_prefixed(&mut hasher, key.as_bytes());
        let digest = hasher.finalize();
        Self(Arc::from(format!(
            "{}/{role}/sha256:{digest:x}",
            family_id.as_str()
        )))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PreparedMathProjectionFingerprint([u8; 32]);

impl PreparedMathProjectionFingerprint {
    pub(crate) fn from_projection(projection: &str) -> Self {
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, PROJECTION_DOMAIN);
        update_len_prefixed(&mut hasher, projection.as_bytes());
        Self(hasher.finalize().into())
    }
}

/// Style facts the prepared-math backend can independently attest for one browser emission.
///
/// The compiled RaTeX backend owns both the wrapper and the default glyph paint, so it can attest
/// the requested foreground and font size. Compatibility HTML backends return opaque markup and
/// therefore remain unverified even though Merman still wraps their result in the requested CSS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedMathStyleAssurance {
    requested_foreground: Arc<str>,
    requested_font_size_bits: u64,
    default_foreground_verified: bool,
    font_size_verified: bool,
}

impl PreparedMathStyleAssurance {
    pub(crate) fn compiled_ratex(requested_foreground: &str, requested_font_size_px: f64) -> Self {
        Self {
            requested_foreground: Arc::from(requested_foreground),
            requested_font_size_bits: requested_font_size_px.to_bits(),
            default_foreground_verified: true,
            font_size_verified: true,
        }
    }

    pub(crate) fn opaque_html(requested_foreground: &str, requested_font_size_px: f64) -> Self {
        Self {
            requested_foreground: Arc::from(requested_foreground),
            requested_font_size_bits: requested_font_size_px.to_bits(),
            default_foreground_verified: false,
            font_size_verified: false,
        }
    }

    pub(crate) fn proves_default_foreground(&self, expected: &str) -> bool {
        self.default_foreground_verified && self.requested_foreground.as_ref() == expected
    }

    pub(crate) fn proves_font_size_px(&self, expected: f64) -> bool {
        self.font_size_verified && self.requested_font_size_bits == expected.to_bits()
    }

    pub(crate) fn retained_heap_bytes(&self) -> usize {
        self.requested_foreground.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedMathExpectation {
    occurrence_id: PreparedMathOccurrenceId,
    projection_fingerprint: Option<PreparedMathProjectionFingerprint>,
    expected_emissions: usize,
}

impl PreparedMathExpectation {
    pub(crate) fn available(
        occurrence_id: PreparedMathOccurrenceId,
        projection_fingerprint: PreparedMathProjectionFingerprint,
        expected_emissions: usize,
    ) -> Self {
        Self {
            occurrence_id,
            projection_fingerprint: Some(projection_fingerprint),
            expected_emissions,
        }
    }

    pub(crate) fn unavailable(
        occurrence_id: PreparedMathOccurrenceId,
        expected_emissions: usize,
    ) -> Self {
        Self {
            occurrence_id,
            projection_fingerprint: None,
            expected_emissions,
        }
    }

    pub(crate) fn occurrence_id(&self) -> &PreparedMathOccurrenceId {
        &self.occurrence_id
    }

    pub(crate) const fn projection_fingerprint(&self) -> Option<PreparedMathProjectionFingerprint> {
        self.projection_fingerprint
    }

    pub(crate) const fn expected_emissions(&self) -> usize {
        self.expected_emissions
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.occurrence_id.as_str().len())
    }
}

/// Immutable renderer-owned expectations for one rendered family artifact.
#[derive(Debug, Clone, Default)]
pub(crate) struct PreparedMathEvidenceLease {
    entries: Arc<[PreparedMathExpectation]>,
    duplicate_occurrence_id: Option<PreparedMathOccurrenceId>,
    _retained_reservations: Arc<[PreparedTextRetainedReservation]>,
}

impl PartialEq for PreparedMathEvidenceLease {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Eq for PreparedMathEvidenceLease {}

impl PreparedMathEvidenceLease {
    pub(crate) fn new(
        mut entries: Vec<PreparedMathExpectation>,
        retained_reservations: Vec<PreparedTextRetainedReservation>,
    ) -> Self {
        entries.sort_unstable_by(|left, right| left.occurrence_id().cmp(right.occurrence_id()));
        let duplicate_occurrence_id = entries
            .windows(2)
            .find(|entries| entries[0].occurrence_id() == entries[1].occurrence_id())
            .map(|entries| entries[0].occurrence_id().clone());
        Self {
            entries: entries.into(),
            duplicate_occurrence_id,
            _retained_reservations: retained_reservations.into(),
        }
    }

    pub(crate) fn entries(&self) -> &[PreparedMathExpectation] {
        &self.entries
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn validate_unique_occurrences(&self) -> Result<(), String> {
        let Some(duplicate) = &self.duplicate_occurrence_id else {
            return Ok(());
        };
        Err(format!(
            "prepared-math evidence contains duplicate occurrence identity `{}`",
            duplicate.as_str()
        ))
    }

    pub(crate) fn expectation(
        &self,
        occurrence_id: &str,
    ) -> Result<Option<(usize, &PreparedMathExpectation)>, String> {
        self.validate_unique_occurrences()?;
        Ok(self
            .entries
            .binary_search_by(|entry| entry.occurrence_id().as_str().cmp(occurrence_id))
            .ok()
            .map(|index| (index, &self.entries[index])))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedMathTerminalOccurrenceReceipt {
    expectation: PreparedMathExpectation,
    emitted: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedMathTerminalReceipt {
    artifact_digest: [u8; 32],
    occurrences: Arc<[PreparedMathTerminalOccurrenceReceipt]>,
}

impl PreparedMathTerminalReceipt {
    pub(crate) fn from_terminal_svg(
        svg: &str,
        evidence: &PreparedMathEvidenceLease,
    ) -> Result<Option<Self>, String> {
        if evidence.is_empty() {
            return Ok(None);
        }
        evidence.validate_unique_occurrences()?;
        if evidence
            .entries()
            .iter()
            .any(|entry| entry.projection_fingerprint().is_none())
        {
            return Err("prepared-math occurrence is marked native-unavailable".to_owned());
        }

        let document = roxmltree::Document::parse(svg)
            .map_err(|error| format!("prepared-math terminal SVG is malformed: {error}"))?;
        let mut emitted = vec![0usize; evidence.entries().len()];
        for node in document.descendants().filter(|node| node.is_element()) {
            let Some(class) = node.attribute("class") else {
                continue;
            };
            if !class
                .split_ascii_whitespace()
                .any(|token| token == PREPARED_MATH_NATIVE_CLASS)
            {
                continue;
            }
            let occurrence_id = node
                .attribute(super::PREPARED_MATH_OCCURRENCE_ATTRIBUTE)
                .ok_or_else(|| {
                    "prepared-math native terminal is missing its occurrence identity".to_owned()
                })?;
            let (expected_index, expected) =
                evidence.expectation(occurrence_id)?.ok_or_else(|| {
                    format!("prepared-math terminal emitted unknown occurrence `{occurrence_id}`")
                })?;
            emitted[expected_index] = emitted[expected_index].saturating_add(1);
            if emitted[expected_index] > expected.expected_emissions() {
                return Err(format!(
                    "prepared-math terminal emitted duplicate occurrence `{occurrence_id}`"
                ));
            }
            let mut children = node.children().filter(|child| child.is_element());
            let projection = children.next().ok_or_else(|| {
                format!("prepared-math terminal occurrence `{occurrence_id}` has no projection")
            })?;
            if children.next().is_some() {
                return Err(format!(
                    "prepared-math terminal occurrence `{occurrence_id}` has duplicate projections"
                ));
            }
            let actual =
                PreparedMathProjectionFingerprint::from_projection(&svg[projection.range()]);
            if Some(actual) != expected.projection_fingerprint() {
                return Err(format!(
                    "prepared-math terminal occurrence `{occurrence_id}` has a mismatched projection"
                ));
            }
        }

        if let Some((missing, _)) = evidence
            .entries()
            .iter()
            .zip(emitted.iter())
            .find(|(entry, emitted)| **emitted < entry.expected_emissions())
        {
            return Err(format!(
                "prepared-math terminal is missing occurrence `{}`",
                missing.occurrence_id().as_str()
            ));
        }

        let occurrences = evidence
            .entries()
            .iter()
            .cloned()
            .zip(emitted)
            .map(
                |(expectation, emitted)| PreparedMathTerminalOccurrenceReceipt {
                    expectation,
                    emitted,
                },
            )
            .collect::<Vec<_>>();
        Ok(Some(Self {
            artifact_digest: artifact_digest(svg),
            occurrences: occurrences.into(),
        }))
    }

    pub(crate) fn occurrence_count(&self) -> usize {
        self.occurrences.len()
    }

    pub(crate) fn artifact_digest(&self) -> [u8; 32] {
        self.artifact_digest
    }
}

fn artifact_digest(svg: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, b"merman-prepared-math-terminal-artifact-v1");
    update_len_prefixed(&mut hasher, svg.as_bytes());
    hasher.finalize().into()
}

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available_evidence(
        occurrence_id: PreparedMathOccurrenceId,
        projection: &str,
        expected_emissions: usize,
    ) -> PreparedMathEvidenceLease {
        PreparedMathEvidenceLease::new(
            vec![PreparedMathExpectation::available(
                occurrence_id,
                PreparedMathProjectionFingerprint::from_projection(projection),
                expected_emissions,
            )],
            Vec::new(),
        )
    }

    fn terminal_occurrence(occurrence_id: &PreparedMathOccurrenceId, projection: &str) -> String {
        format!(
            r#"<g class="merman-prepared-math-native" data-merman-prepared-math-occurrence="{}">{projection}</g>"#,
            occurrence_id.as_str(),
        )
    }

    #[test]
    fn semantic_occurrence_ids_are_stable_and_attribute_safe() {
        let indexed =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "edge-label", 17);
        let keyed = PreparedMathOccurrenceId::keyed(
            DiagramFamilyId::SEQUENCE,
            "block-label",
            "critical \0 λ",
        );

        assert_eq!(indexed.as_str(), "flowchart/edge-label/17");
        assert!(keyed.as_str().starts_with("sequence/block-label/sha256:"));
        assert!(
            keyed
                .as_str()
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"/-:".contains(&byte))
        );
    }

    #[test]
    fn projection_fingerprint_is_domain_separated_and_content_sensitive() {
        assert_ne!(
            PreparedMathProjectionFingerprint::from_projection("<g><path/></g>"),
            PreparedMathProjectionFingerprint::from_projection("<g><rect/></g>")
        );
    }

    #[test]
    fn terminal_receipt_binds_exact_artifact_and_expected_emission_count() {
        let occurrence_id =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::SEQUENCE, "actor-label", 0);
        let projection = r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9"><path d="M0 0h1"/></g>"#;
        let evidence = available_evidence(occurrence_id.clone(), projection, 2);
        let occurrence = terminal_occurrence(&occurrence_id, projection);
        let svg = format!(r#"<svg>{occurrence}{occurrence}</svg>"#);

        let receipt = PreparedMathTerminalReceipt::from_terminal_svg(&svg, &evidence)
            .expect("valid terminal evidence")
            .expect("non-empty terminal receipt");

        assert_eq!(receipt.occurrence_count(), 1);
        assert_ne!(receipt.artifact_digest(), [0; 32]);
        let changed_svg = format!(r#"<svg data-version="2">{occurrence}{occurrence}</svg>"#);
        let changed = PreparedMathTerminalReceipt::from_terminal_svg(&changed_svg, &evidence)
            .expect("same occurrence evidence in a different exact artifact")
            .expect("non-empty terminal receipt");
        assert_ne!(receipt.artifact_digest(), changed.artifact_digest());
    }

    #[test]
    fn terminal_receipt_rejects_missing_duplicate_unavailable_and_mismatched_occurrences() {
        let occurrence_id =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", 0);
        let projection = r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9"><path d="M0 0h1"/></g>"#;
        let evidence = available_evidence(occurrence_id.clone(), projection, 1);

        let error = PreparedMathTerminalReceipt::from_terminal_svg("<svg/>", &evidence)
            .expect_err("missing occurrence must fail closed");
        assert!(error.contains("missing occurrence"), "{error}");

        let occurrence = terminal_occurrence(&occurrence_id, projection);
        let duplicated = format!(r#"<svg>{occurrence}{occurrence}</svg>"#);
        let error = PreparedMathTerminalReceipt::from_terminal_svg(&duplicated, &evidence)
            .expect_err("unexpected duplicate occurrence must fail closed");
        assert!(error.contains("duplicate occurrence"), "{error}");

        let unavailable = PreparedMathEvidenceLease::new(
            vec![PreparedMathExpectation::unavailable(
                occurrence_id.clone(),
                1,
            )],
            Vec::new(),
        );
        let error = PreparedMathTerminalReceipt::from_terminal_svg("<svg/>", &unavailable)
            .expect_err("native-unavailable occurrence must fail closed");
        assert!(error.contains("native-unavailable"), "{error}");

        let mismatched = terminal_occurrence(
            &occurrence_id,
            r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9"><rect/></g>"#,
        );
        let error = PreparedMathTerminalReceipt::from_terminal_svg(
            &format!("<svg>{mismatched}</svg>"),
            &evidence,
        )
        .expect_err("mismatched projection must fail closed");
        assert!(error.contains("mismatched projection"), "{error}");
    }

    #[test]
    fn terminal_receipt_rejects_duplicate_renderer_occurrence_identities() {
        let occurrence_id =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", 0);
        let projection = r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9"><path d="M0 0h1"/></g>"#;
        let fingerprint = PreparedMathProjectionFingerprint::from_projection(projection);
        let evidence = PreparedMathEvidenceLease::new(
            vec![
                PreparedMathExpectation::available(occurrence_id.clone(), fingerprint, 1),
                PreparedMathExpectation::available(occurrence_id, fingerprint, 1),
            ],
            Vec::new(),
        );

        let error = PreparedMathTerminalReceipt::from_terminal_svg("<svg/>", &evidence)
            .expect_err("duplicate renderer occurrence identities must fail closed");

        assert!(error.contains("duplicate occurrence identity"), "{error}");
    }
}
