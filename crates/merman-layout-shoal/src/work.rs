use std::{cmp::Ordering, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkError {
    Interrupted,
    ArithmeticOverflow,
}

impl fmt::Display for WorkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Interrupted => "layout work interrupted",
            Self::ArithmeticOverflow => "layout arithmetic overflow",
        })
    }
}

impl Error for WorkError {}

/// One control per operation. `check` polls cancellation without consuming units.
pub trait WorkControl {
    fn check(&mut self) -> Result<(), WorkError>;
    fn charge(&mut self, units: usize) -> Result<(), WorkError>;

    /// Attempts to charge optional work while leaving `headroom` units for required work.
    /// Declining with `Ok(false)` leaves the operation active and consumes no units.
    /// The default checks for cancellation and declines.
    fn try_charge_optional(&mut self, _units: usize, _headroom: usize) -> Result<bool, WorkError> {
        self.check()?;
        Ok(false)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopWorkControl;

impl WorkControl for NoopWorkControl {
    fn check(&mut self) -> Result<(), WorkError> {
        Ok(())
    }
    fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
        Ok(())
    }
    fn try_charge_optional(&mut self, _units: usize, _headroom: usize) -> Result<bool, WorkError> {
        Ok(true)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutError {
    InvalidInput(&'static str),
    Unsupported(&'static str),
    AllocationLimit,
    Work(WorkError),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(reason) => f.write_str(reason),
            Self::Unsupported(construct) => write!(f, "Shoal does not support {construct}"),
            Self::AllocationLimit => f.write_str("Shoal could not allocate layout storage"),
            Self::Work(error) => error.fmt(f),
        }
    }
}

impl Error for LayoutError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Work(error) => Some(error),
            _ => None,
        }
    }
}

impl From<WorkError> for LayoutError {
    fn from(error: WorkError) -> Self {
        Self::Work(error)
    }
}

pub(crate) type LayoutResult<T> = Result<T, LayoutError>;

pub(crate) fn reserved<T>(len: usize) -> LayoutResult<Vec<T>> {
    let mut vec = Vec::new();
    vec.try_reserve_exact(len)
        .map_err(|_| LayoutError::AllocationLimit)?;
    Ok(vec)
}

pub(crate) fn filled<T: Clone>(len: usize, value: T) -> LayoutResult<Vec<T>> {
    let mut vec = reserved(len)?;
    vec.resize(len, value);
    Ok(vec)
}

pub(crate) fn push<T>(vec: &mut Vec<T>, value: T) -> LayoutResult<()> {
    vec.try_reserve(1)
        .map_err(|_| LayoutError::AllocationLimit)?;
    vec.push(value);
    Ok(())
}

pub(crate) struct Work<'a> {
    control: &'a mut dyn WorkControl,
    pending: usize,
    total: usize,
    optional_limit: Option<usize>,
    optional_declined: bool,
    pub(crate) editor_choice: bool,
}

impl<'a> Work<'a> {
    pub(crate) fn new(control: &'a mut dyn WorkControl) -> Self {
        Self {
            control,
            pending: 0,
            total: 0,
            optional_limit: None,
            optional_declined: false,
            editor_choice: false,
        }
    }

    pub(crate) fn check(&mut self) -> LayoutResult<()> {
        Ok(self.control.check()?)
    }

    /// Admit a bulk operation and any preceding partial traversal batch.
    pub(crate) fn charge(&mut self, units: usize) -> LayoutResult<()> {
        let units = self
            .pending
            .checked_add(units)
            .ok_or(WorkError::ArithmeticOverflow)?;
        if units != 0 {
            let total = self
                .total
                .checked_add(units)
                .ok_or(WorkError::ArithmeticOverflow)?;
            self.check()?;
            if let Some(limit) = self.optional_limit {
                if total > limit || !self.control.try_charge_optional(units, limit)? {
                    self.optional_declined = true;
                    return Err(WorkError::Interrupted.into());
                }
            } else {
                self.control.charge(units)?;
            }
            self.total = total;
            self.pending = 0;
        }
        Ok(())
    }

    pub(crate) fn visit(&mut self, units: usize) -> LayoutResult<()> {
        self.pending = self
            .pending
            .checked_add(units)
            .ok_or(WorkError::ArithmeticOverflow)?;
        if self.pending >= 128 {
            self.charge(0)?;
        }
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> LayoutResult<()> {
        self.charge(0)?;
        self.check()
    }

    /// Bounds a comparison by the work already spent and leaves the same headroom for the caller.
    pub(crate) fn compare<T>(
        &mut self,
        draw: impl FnOnce(&mut Work<'_>) -> LayoutResult<T>,
    ) -> LayoutResult<Option<T>> {
        self.finish()?;
        let limit = self.total;
        let Some(headroom) = limit.checked_mul(2) else {
            return Ok(None);
        };
        if !self.control.try_charge_optional(0, headroom)? {
            return Ok(None);
        }
        let mut comparison = Work::new(self.control);
        comparison.optional_limit = Some(limit);
        let result = draw(&mut comparison).and_then(|drawing| {
            comparison.finish()?;
            Ok(drawing)
        });
        if comparison.optional_declined {
            self.check()?;
            return Ok(None);
        }
        result.map(Some)
    }

    pub(crate) fn charge_sort(&mut self, len: usize) -> LayoutResult<()> {
        if len > 1 {
            self.charge(
                len.checked_mul(len.ilog2() as usize + 1)
                    .ok_or(WorkError::ArithmeticOverflow)?,
            )?;
        }
        Ok(())
    }

    /// Admit the full comparison budget before entering the standard library sort.
    pub(crate) fn sort(
        &mut self,
        values: &mut [usize],
        cmp: impl Fn(&usize, &usize) -> Ordering,
    ) -> LayoutResult<()> {
        if values.len() > 1 {
            self.charge_sort(values.len())?;
            values.sort_unstable_by(cmp);
            self.check()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_comparison_distinguishes_its_bound_from_interruption() {
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        work.charge(4).unwrap();

        let declined = work.compare(|trial| {
            trial.charge(3)?;
            trial.charge(2)?;
            Ok(1)
        });
        assert_eq!(declined, Ok(None));
        assert_eq!(work.total, 4);

        let interrupted = work.compare(|_| Err::<(), _>(WorkError::Interrupted.into()));
        assert_eq!(interrupted, Err(LayoutError::Work(WorkError::Interrupted)));
        assert_eq!(work.finish(), Ok(()));
    }

    #[test]
    fn a_failed_reservation_is_an_allocation_error() {
        assert_eq!(
            reserved::<usize>(usize::MAX),
            Err(LayoutError::AllocationLimit)
        );
    }

    #[test]
    fn sort_admits_its_checked_budget_before_comparing() {
        struct Denied;
        impl WorkControl for Denied {
            fn check(&mut self) -> Result<(), WorkError> {
                Ok(())
            }
            fn charge(&mut self, units: usize) -> Result<(), WorkError> {
                assert_eq!(units, 4 * (2 + 1));
                Err(WorkError::ArithmeticOverflow)
            }
        }
        let mut ids = [1, 3, 2, 0];
        let result = Work::new(&mut Denied).sort(&mut ids, |_, _| panic!("sort was denied"));
        assert_eq!(
            result,
            Err(LayoutError::Work(WorkError::ArithmeticOverflow))
        );
        assert_eq!(ids, [1, 3, 2, 0]);
    }
}
