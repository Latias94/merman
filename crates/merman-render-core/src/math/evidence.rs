use std::sync::Arc;

use merman_model::family::DiagramFamilyId;
use sha2::{Digest as _, Sha256};

use crate::resources::PreparedTextRetainedReservation;

const OCCURRENCE_KEY_DOMAIN: &[u8] = b"merman-prepared-math-occurrence-v1";
const PROJECTION_DOMAIN: &[u8] = b"merman-prepared-math-native-projection-v1";
pub const PREPARED_MATH_NATIVE_CLASS: &str = "merman-prepared-math-native";

/// Renderer-owned identity for one semantic math-label occurrence.
///
/// The value is safe to emit as an XML attribute and is never recovered from user-authored SVG.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreparedMathOccurrenceId(Arc<str>);

impl PreparedMathOccurrenceId {
    pub fn indexed(family_id: DiagramFamilyId, role: &'static str, index: usize) -> Self {
        Self(Arc::from(format!("{}/{role}/{index}", family_id.as_str())))
    }

    pub fn keyed(family_id: DiagramFamilyId, role: &'static str, key: &str) -> Self {
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, OCCURRENCE_KEY_DOMAIN);
        update_len_prefixed(&mut hasher, family_id.as_str().as_bytes());
        update_len_prefixed(&mut hasher, role.as_bytes());
        update_len_prefixed(&mut hasher, key.as_bytes());
        let digest = hasher.finalize();
        let family = family_id.as_str();
        let suffix = "/sha256:";
        let mut id =
            String::with_capacity(family.len() + 1 + role.len() + suffix.len() + 2 * digest.len());
        id.push_str(family);
        id.push('/');
        id.push_str(role);
        id.push_str(suffix);
        merman_model::push_hex_lower(&mut id, &digest);
        Self(Arc::from(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreparedMathProjectionFingerprint([u8; 32]);

impl PreparedMathProjectionFingerprint {
    pub fn from_projection(projection: &str) -> Self {
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, PROJECTION_DOMAIN);
        update_len_prefixed(&mut hasher, projection.as_bytes());
        Self(hasher.finalize().into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedMathExpectation {
    occurrence_id: PreparedMathOccurrenceId,
    projection_fingerprint: Option<PreparedMathProjectionFingerprint>,
    expected_emissions: usize,
}

impl PreparedMathExpectation {
    pub fn available(
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

    pub fn unavailable(occurrence_id: PreparedMathOccurrenceId, expected_emissions: usize) -> Self {
        Self {
            occurrence_id,
            projection_fingerprint: None,
            expected_emissions,
        }
    }

    pub fn occurrence_id(&self) -> &PreparedMathOccurrenceId {
        &self.occurrence_id
    }

    pub const fn projection_fingerprint(&self) -> Option<PreparedMathProjectionFingerprint> {
        self.projection_fingerprint
    }

    pub const fn expected_emissions(&self) -> usize {
        self.expected_emissions
    }

    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.occurrence_id.as_str().len())
    }
}

/// Immutable renderer-owned expectations and native payloads for one rendered family artifact.
#[derive(Debug, Clone, Default)]
pub struct PreparedMathEvidenceLease {
    entries: Arc<[PreparedMathExpectation]>,
    native_projections: Arc<[Option<Arc<super::PreparedMathNativeProjection>>]>,
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
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        entries: Vec<PreparedMathExpectation>,
        retained_reservations: Vec<PreparedTextRetainedReservation>,
    ) -> Self {
        Self::with_native_projections(
            entries.into_iter().map(|entry| (entry, None)).collect(),
            retained_reservations,
        )
    }

    pub fn with_native_projections(
        mut entries: Vec<(
            PreparedMathExpectation,
            Option<Arc<super::PreparedMathNativeProjection>>,
        )>,
        retained_reservations: Vec<PreparedTextRetainedReservation>,
    ) -> Self {
        entries.sort_unstable_by(|left, right| left.0.occurrence_id().cmp(right.0.occurrence_id()));
        let duplicate_occurrence_id = entries
            .windows(2)
            .find(|entries| entries[0].0.occurrence_id() == entries[1].0.occurrence_id())
            .map(|entries| entries[0].0.occurrence_id().clone());
        let (entries, native_projections): (Vec<_>, Vec<_>) = entries.into_iter().unzip();
        Self {
            entries: entries.into(),
            native_projections: native_projections.into(),
            duplicate_occurrence_id,
            _retained_reservations: retained_reservations.into(),
        }
    }

    pub fn entries(&self) -> &[PreparedMathExpectation] {
        &self.entries
    }

    pub fn native_projection(&self, index: usize) -> Option<&super::PreparedMathNativeProjection> {
        self.native_projections.get(index)?.as_deref()
    }

    pub fn retained_entry_bytes(
        expectation: &PreparedMathExpectation,
        projection: Option<&super::PreparedMathNativeProjection>,
    ) -> usize {
        expectation
            .retained_bytes()
            .saturating_add(std::mem::size_of::<
                Option<Arc<super::PreparedMathNativeProjection>>,
            >())
            .saturating_add(
                projection.map_or(0, super::PreparedMathNativeProjection::retained_bytes),
            )
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn validate_unique_occurrences(&self) -> Result<(), String> {
        let Some(duplicate) = &self.duplicate_occurrence_id else {
            return Ok(());
        };
        Err(format!(
            "prepared-math evidence contains duplicate occurrence identity `{}`",
            duplicate.as_str()
        ))
    }

    pub fn expectation(
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

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
