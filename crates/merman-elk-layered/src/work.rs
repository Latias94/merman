//! Owner-neutral work control for the source-backed ELK layered kernel.

/// Neutral failure returned by caller-provided layout work controls.
///
/// The source port deliberately does not depend on renderer resource-policy types. Callers map an
/// interruption to their own cancellation or resource error, while arithmetic overflow remains a
/// deterministic kernel failure even when the caller otherwise has no ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkError {
    Interrupted,
    ArithmeticOverflow,
}

impl std::fmt::Display for WorkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Interrupted => "ELK layout work was interrupted by the caller",
            Self::ArithmeticOverflow => "ELK layout work arithmetic overflowed",
        })
    }
}

impl std::error::Error for WorkError {}

/// Caller-owned work control for one ELK import/layout invocation.
///
/// `check` is a non-consuming admission probe. Importers and processors may check one or more
/// monotonic prefixes before allocating input-sized planning state, then check and charge the
/// complete tranche immediately before mutation. Implementations must not assume that every
/// checked amount is later charged or that a processor calls `check` exactly once.
pub trait WorkControl {
    /// Checks whether an admitted prefix or complete tranche would be accepted without consuming
    /// it.
    fn check(&mut self, units: usize) -> Result<(), WorkError>;

    /// Consumes a complete tranche immediately before its owned work executes.
    fn charge(&mut self, units: usize) -> Result<(), WorkError>;
}

/// Checked no-op control used by compatibility entry points.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopWorkControl;

impl WorkControl for NoopWorkControl {
    fn check(&mut self, _units: usize) -> Result<(), WorkError> {
        Ok(())
    }

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
    checked_mul(value, ceil_log2(value))
}

pub(crate) fn checked_sum(values: impl IntoIterator<Item = usize>) -> Result<usize, WorkError> {
    values.into_iter().try_fold(0usize, checked_add)
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
        assert_eq!(
            checked_sum([usize::MAX, 1]),
            Err(WorkError::ArithmeticOverflow)
        );
        assert_eq!(ceil_log2(0), 0);
        assert_eq!(ceil_log2(1), 0);
        assert_eq!(ceil_log2(3), 2);
        assert_eq!(checked_n_log_n(8), Ok(24));
        assert_eq!(
            checked_n_log_n(usize::MAX),
            Err(WorkError::ArithmeticOverflow)
        );
    }
}
