use crate::error::WorkFailure;

/// Caller-owned control for bounded layout kernels.
///
/// Implementations must either accept a complete tranche or reject it without advancing their
/// budget. Manatee deliberately keeps the failure neutral so renderers can map it to their own
/// resource error type without creating a reverse dependency.
pub trait WorkControl {
    /// Checks whether a complete predictable tranche can fit without consuming it.
    ///
    /// The default keeps compatibility callers unbounded. Budgeted callers should override this
    /// so kernels can reject predictable work before materializing input-sized state.
    fn check(&mut self, _units: usize) -> std::result::Result<(), WorkFailure> {
        Ok(())
    }

    fn charge(&mut self, units: usize) -> std::result::Result<(), WorkFailure>;
}

pub(crate) fn admit_dynamic_work<W: WorkControl + ?Sized>(
    work_control: &mut W,
    units: usize,
) -> std::result::Result<(), WorkFailure> {
    if units == 0 {
        return Ok(());
    }
    work_control.check(units)?;
    work_control.charge(units)
}

/// Work control used by compatibility entry points that do not impose a caller budget.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopWorkControl;

impl WorkControl for NoopWorkControl {
    fn charge(&mut self, _units: usize) -> std::result::Result<(), WorkFailure> {
        Ok(())
    }
}
