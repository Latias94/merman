//! Owner-local work control for the Dagre layout kernel.

/// Neutral failure returned by caller-provided layout work controls.
///
/// Dugong deliberately does not depend on renderer resource-policy types. Callers can map an
/// interruption to their own cancellation or resource error while arithmetic overflow remains a
/// deterministic kernel failure even when the caller otherwise has no ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkError {
    Interrupted,
    ArithmeticOverflow,
}

impl std::fmt::Display for WorkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Interrupted => "layout work was interrupted by the caller",
            Self::ArithmeticOverflow => "layout work arithmetic overflowed",
        })
    }
}

impl std::error::Error for WorkError {}

/// Caller-owned work control for one Dugong layout invocation.
///
/// Implementations must accept a complete tranche or reject it without advancing their budget.
pub trait WorkControl {
    fn charge(&mut self, units: usize) -> Result<(), WorkError>;
}

/// Checked no-op control used by the compatibility [`crate::layout`] entry point.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopWorkControl;

impl WorkControl for NoopWorkControl {
    fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
        Ok(())
    }
}

pub(crate) fn checked_add(left: usize, right: usize) -> Result<usize, WorkError> {
    left.checked_add(right).ok_or(WorkError::ArithmeticOverflow)
}

pub(crate) fn checked_mul(left: usize, right: usize) -> Result<usize, WorkError> {
    left.checked_mul(right).ok_or(WorkError::ArithmeticOverflow)
}

pub(crate) fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

pub(crate) fn checked_n_log_n(value: usize) -> Result<usize, WorkError> {
    checked_mul(value, ceil_log2(value).max(1))
}

/// Conservative comparison-height work for ordered object-key map mutations.
///
/// Graphlib uses ordered maps for JavaScript array-index keys. Each insertion or removal is
/// logarithmic in the maximum live numeric-key population; the extra level covers the root/leaf
/// visit for singleton and empty maps instead of treating those mutations as free.
pub(crate) fn checked_ordered_key_updates(
    entry_bound: usize,
    update_count: usize,
) -> Result<usize, WorkError> {
    let height = checked_add(ceil_log2(entry_bound), 1)?;
    checked_mul(update_count, height)
}

/// Conservative work for Graphlib's construction-only unparented assignment batch.
///
/// The batch builds and validates a union-by-rank forest from `existing_parent_count` live links,
/// then replays every first assignment. Ordinary child buckets are covered by the linear tranche
/// and amortized compaction; array-index children additionally remove from one ordered root map and
/// insert into one ordered parent map. Keeping the existing-link count separate from allocated node
/// slots avoids charging an empty freshly copied forest as though every slot already had a parent.
pub(crate) fn checked_unparented_parent_batch_work(
    node_slots: usize,
    existing_parent_count: usize,
    assignment_count: usize,
    numeric_assignment_count: usize,
) -> Result<usize, WorkError> {
    if assignment_count == 0 {
        return Ok(0);
    }
    let find_work = checked_mul(checked_add(ceil_log2(node_slots), 1)?, 4)?;
    let forest_items = checked_add(existing_parent_count, assignment_count)?;
    let union_work = checked_mul(forest_items, find_work)?;
    let linear_work = checked_add(
        checked_mul(node_slots, 6)?,
        checked_mul(assignment_count, 2)?,
    )?;
    let ordered_updates = checked_mul(numeric_assignment_count, 2)?;
    let ordered_work = checked_ordered_key_updates(node_slots, ordered_updates)?;
    checked_add(checked_add(union_work, linear_work)?, ordered_work)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_work_arithmetic_fails_closed() {
        assert_eq!(
            checked_add(usize::MAX, 1),
            Err(WorkError::ArithmeticOverflow)
        );
        assert_eq!(
            checked_mul(usize::MAX, 2),
            Err(WorkError::ArithmeticOverflow)
        );
    }

    #[test]
    fn logarithmic_work_is_monotonic_and_checked() {
        assert_eq!(ceil_log2(0), 0);
        assert_eq!(ceil_log2(1), 0);
        assert_eq!(ceil_log2(2), 1);
        assert_eq!(ceil_log2(3), 2);
        assert_eq!(ceil_log2(4), 2);
        assert_eq!(checked_n_log_n(4), Ok(8));
        assert_eq!(checked_ordered_key_updates(0, 3), Ok(3));
        assert_eq!(checked_ordered_key_updates(1, 3), Ok(3));
        assert_eq!(checked_ordered_key_updates(4, 3), Ok(9));
        assert_eq!(checked_unparented_parent_batch_work(0, 0, 0, 0), Ok(0));
        assert_eq!(checked_unparented_parent_batch_work(4, 3, 0, 0), Ok(0));
        assert_eq!(checked_unparented_parent_batch_work(4, 0, 3, 0), Ok(66));
        assert_eq!(checked_unparented_parent_batch_work(4, 0, 3, 2), Ok(78));
        assert_eq!(checked_unparented_parent_batch_work(4, 3, 3, 0), Ok(102));
        assert_eq!(
            checked_ordered_key_updates(usize::MAX, usize::MAX),
            Err(WorkError::ArithmeticOverflow)
        );
        assert_eq!(
            checked_unparented_parent_batch_work(usize::MAX, usize::MAX, usize::MAX, usize::MAX,),
            Err(WorkError::ArithmeticOverflow)
        );
    }
}
