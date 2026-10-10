use std::fmt;
use std::ops::Range;

use crate::resources::OperationWorkMeter;

/// Minimal SVG output surface shared by unbounded helpers and operation-bounded renderers.
///
/// Family writers intentionally depend on this narrow interface instead of `String`, allowing the
/// operation boundary to reject an append before the backing allocation grows.
pub(super) trait SvgOutput: fmt::Write {
    fn push_str(&mut self, value: &str);
    fn push(&mut self, value: char);
    fn len(&self) -> usize;
    fn as_str(&self) -> &str;
    fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()>;
    fn checkpoint(&mut self) -> crate::Result<()>;
}

impl SvgOutput for String {
    fn push_str(&mut self, value: &str) {
        String::push_str(self, value);
    }

    fn push(&mut self, value: char) {
        String::push(self, value);
    }

    fn len(&self) -> usize {
        String::len(self)
    }

    fn as_str(&self) -> &str {
        String::as_str(self)
    }

    fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
        String::replace_range(self, range, replacement);
        Ok(())
    }

    fn checkpoint(&mut self) -> crate::Result<()> {
        Ok(())
    }
}

/// Operation-owned SVG buffer that checks every retained append before allocation.
///
/// Producer-owned projected-byte accounting remains separate: some emitters reserve or charge
/// their own contributions before serialization, so this document sink must not charge them a
/// second time.
pub(super) struct BoundedSvgOutput<'a> {
    out: String,
    work_meter: &'a OperationWorkMeter,
    state: BoundedSvgOutputState,
}

enum BoundedSvgOutputState {
    Active,
    Failed(BoundedSvgOutputFailure),
}

#[derive(Clone)]
enum BoundedSvgOutputFailure {
    Operation(crate::resources::OperationWorkError),
    InvalidModel(&'static str),
}

impl BoundedSvgOutputState {
    fn record_failure(&mut self, failure: BoundedSvgOutputFailure) {
        if matches!(self, Self::Active) {
            *self = Self::Failed(failure);
        }
    }

    fn error(&self) -> Option<crate::Error> {
        match self {
            Self::Active => None,
            Self::Failed(failure) => Some(failure.to_error()),
        }
    }

    fn into_error(self) -> Option<crate::Error> {
        match self {
            Self::Active => None,
            Self::Failed(failure) => Some(failure.into_error()),
        }
    }
}

impl BoundedSvgOutputFailure {
    fn to_error(&self) -> crate::Error {
        self.clone().into_error()
    }

    fn into_error(self) -> crate::Error {
        match self {
            Self::Operation(error) => error.into(),
            Self::InvalidModel(message) => crate::Error::InvalidModel {
                message: message.to_string(),
            },
        }
    }
}

impl<'a> BoundedSvgOutput<'a> {
    pub(super) fn new(work_meter: &'a OperationWorkMeter) -> Self {
        Self {
            out: String::new(),
            work_meter,
            state: BoundedSvgOutputState::Active,
        }
    }

    pub(super) fn finish(self) -> crate::Result<String> {
        match self.state.into_error() {
            Some(error) => Err(error),
            None => Ok(self.out),
        }
    }

    fn replace_retained_range(
        &mut self,
        range: Range<usize>,
        replacement: &str,
    ) -> crate::Result<()> {
        if let Some(error) = self.state.error() {
            return Err(error);
        }
        let removed = range.len();
        let Some(retained_without_range) = self.out.len().checked_sub(removed) else {
            return Err(self.fail(BoundedSvgOutputFailure::InvalidModel(
                "bounded SVG replacement range exceeds the current output",
            )));
        };
        let Some(final_len) = retained_without_range.checked_add(replacement.len()) else {
            return Err(self.fail(BoundedSvgOutputFailure::InvalidModel(
                "bounded SVG replacement length overflowed",
            )));
        };
        if final_len > self.out.len() {
            let growth = final_len - self.out.len();
            if let Err(failure) = self.admit_growth(growth) {
                return Err(self.fail(failure));
            }
        }
        self.out.replace_range(range, replacement);
        Ok(())
    }

    fn admit_growth(&mut self, additional: usize) -> Result<(), BoundedSvgOutputFailure> {
        self.work_meter
            .check_svg_append(self.out.len(), additional)
            .map_err(BoundedSvgOutputFailure::Operation)?;

        let required_len =
            self.out
                .len()
                .checked_add(additional)
                .ok_or(BoundedSvgOutputFailure::InvalidModel(
                    "bounded SVG output length overflowed",
                ))?;
        let current_capacity = self.out.capacity();
        if required_len <= current_capacity {
            return Ok(());
        }

        let geometric_capacity = current_capacity.saturating_mul(2);
        let mut target_capacity = required_len.max(geometric_capacity);
        if let Some(max_svg_bytes) = self.work_meter.max_svg_bytes() {
            target_capacity = target_capacity.min(max_svg_bytes);
        }
        let reserve = target_capacity.checked_sub(self.out.len()).ok_or(
            BoundedSvgOutputFailure::InvalidModel(
                "bounded SVG output capacity fell below its retained length",
            ),
        )?;
        self.out.try_reserve_exact(reserve).map_err(|_| {
            BoundedSvgOutputFailure::InvalidModel("failed to reserve bounded SVG output")
        })
    }

    fn record_write(&mut self, value: &str) -> fmt::Result {
        if matches!(&self.state, BoundedSvgOutputState::Failed(_)) {
            return Err(fmt::Error);
        }
        if let Err(failure) = self.admit_growth(value.len()) {
            self.state.record_failure(failure);
            return Err(fmt::Error);
        }
        self.out.push_str(value);
        Ok(())
    }

    fn fail(&mut self, failure: BoundedSvgOutputFailure) -> crate::Error {
        self.state.record_failure(failure);
        self.state
            .error()
            .expect("recording a bounded SVG failure must make the sink terminal")
    }

    #[cfg(test)]
    fn retained_capacity(&self) -> usize {
        self.out.capacity()
    }
}

impl fmt::Write for BoundedSvgOutput<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.record_write(value)
    }
}

impl SvgOutput for BoundedSvgOutput<'_> {
    fn push_str(&mut self, value: &str) {
        let _ = self.record_write(value);
    }

    fn push(&mut self, value: char) {
        let mut encoded = [0u8; 4];
        let _ = self.record_write(value.encode_utf8(&mut encoded));
    }

    fn len(&self) -> usize {
        self.out.len()
    }

    fn as_str(&self) -> &str {
        self.out.as_str()
    }

    fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
        self.replace_retained_range(range, replacement)
    }

    fn checkpoint(&mut self) -> crate::Result<()> {
        match self.state.error() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

    #[test]
    fn bounded_svg_output_rejects_before_retaining_an_over_limit_append() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 10)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);

        out.push_str("<svg>");
        out.push_str("12345");
        assert_eq!(out.as_str(), "<svg>12345");
        assert_eq!(meter.projected_svg_bytes(), 0);
        let capacity_before_rejection = out.retained_capacity();

        out.push_str("x");
        assert_eq!(out.as_str(), "<svg>12345");
        assert_eq!(out.retained_capacity(), capacity_before_rejection);
        let error = out.finish().unwrap_err();
        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
    }

    #[test]
    fn bounded_svg_output_keeps_the_first_failure_after_every_checkpoint() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 10)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);

        out.push_str("<svg>12345");
        out.push('x');

        let first = resource_limit(out.checkpoint().unwrap_err());
        let repeated = resource_limit(out.checkpoint().unwrap_err());
        assert_eq!(repeated, first);

        let replacement = out.replace_range(0..5, "").unwrap_err();
        assert_eq!(resource_limit(replacement), first);
        assert_eq!(out.as_str(), "<svg>12345");
        assert_eq!(meter.projected_svg_bytes(), 0);

        let finished = resource_limit(out.finish().unwrap_err());
        assert_eq!(finished, first);
        assert_eq!(meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn bounded_svg_output_geometrically_reserves_repeated_short_writes() {
        const MAX_SVG_BYTES: usize = 64;

        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, MAX_SVG_BYTES)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);
        let mut capacity_changes = 0;
        let mut previous_capacity = out.retained_capacity();

        for _ in 0..MAX_SVG_BYTES {
            out.push('x');
            let capacity = out.retained_capacity();
            assert!(capacity <= MAX_SVG_BYTES);
            if capacity != previous_capacity {
                capacity_changes += 1;
                previous_capacity = capacity;
            }
        }

        assert!(capacity_changes <= 8);
        assert_eq!(meter.projected_svg_bytes(), 0);
        assert_eq!(out.finish().unwrap(), "x".repeat(MAX_SVG_BYTES));
        assert_eq!(meter.projected_svg_bytes(), 0);
    }

    fn resource_limit(error: crate::Error) -> crate::ResourceLimitExceeded {
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected bounded SVG output to preserve its resource-limit failure");
        };
        error
    }
}
