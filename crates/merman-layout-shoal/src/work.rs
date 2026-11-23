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
}

impl<'a> Work<'a> {
    pub(crate) fn new(control: &'a mut dyn WorkControl) -> Self {
        Self {
            control,
            pending: 0,
            total: 0,
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
            self.total = self
                .total
                .checked_add(units)
                .ok_or(WorkError::ArithmeticOverflow)?;
            self.check()?;
            self.control.charge(units)?;
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

    /// Admit the full comparison budget before entering the standard library sort.
    pub(crate) fn sort(
        &mut self,
        ids: &mut [usize],
        cmp: impl Fn(&usize, &usize) -> Ordering,
    ) -> LayoutResult<()> {
        if ids.len() > 1 {
            let levels = ids.len().ilog2() as usize + 1;
            self.charge(
                ids.len()
                    .checked_mul(levels)
                    .ok_or(WorkError::ArithmeticOverflow)?,
            )?;
            ids.sort_unstable_by(cmp);
            self.check()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
