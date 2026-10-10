use std::sync::Arc;

use crate::resources::PreparedTextRetainedReservation;

use super::prepared::PreparedTextLabelLedgerEntry;

/// Shared ownership for prepared-text evidence and the bytes that keep it alive.
#[derive(Debug, Clone, Default)]
pub(crate) struct PreparedTextEvidenceLease {
    entries: Arc<[PreparedTextLabelLedgerEntry]>,
    _retained_reservations: Arc<[PreparedTextRetainedReservation]>,
}

impl PreparedTextEvidenceLease {
    pub(crate) fn new(
        entries: Vec<PreparedTextLabelLedgerEntry>,
        retained_reservations: Vec<PreparedTextRetainedReservation>,
    ) -> Self {
        Self {
            entries: entries.into(),
            _retained_reservations: retained_reservations.into(),
        }
    }

    pub(crate) fn entries(&self) -> &[PreparedTextLabelLedgerEntry] {
        &self.entries
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl PartialEq for PreparedTextEvidenceLease {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Eq for PreparedTextEvidenceLease {}
