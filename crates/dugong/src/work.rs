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
    }
}
