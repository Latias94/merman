use crate::resources::{OperationWorkError, OperationWorkMeter, PreparedTextRetainedReservation};
use crate::{Error, Result};
#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;
use std::sync::Arc;

/// One renderer-owned adapter shared by the lower layout kernels.
///
/// The kernels expose different neutral control traits and consume the same render operation's
/// cumulative resource budget. Keeping the concrete adapter here
/// prevents each family from inventing a subtly different sticky-error or overflow mapping.
pub(crate) struct OperationLayoutWorkControl {
    meter: Arc<OperationWorkMeter>,
    rejection: RefCell<Option<OperationWorkError>>,
    #[cfg(test)]
    adapter_work: Cell<usize>,
}

impl OperationLayoutWorkControl {
    pub(crate) fn new(meter: Arc<OperationWorkMeter>) -> Self {
        Self {
            meter,
            rejection: RefCell::new(None),
            #[cfg(test)]
            adapter_work: Cell::new(0),
        }
    }

    pub(crate) fn charge_adapter(&mut self, units: usize) -> Result<()> {
        if let Some(error) = self.rejection.borrow().clone() {
            return Err(error.into());
        }
        #[cfg(test)]
        let next_adapter_work = self.checked_add(self.adapter_work.get(), units)?;
        self.meter.charge(units).map_err(|error| {
            *self.rejection.borrow_mut() = Some(error.clone());
            Error::from(error)
        })?;
        #[cfg(test)]
        self.adapter_work.set(next_adapter_work);
        Ok(())
    }

    /// Keeps prepared-text accounting attached to the artifact that owns the retained bytes.
    pub(crate) fn reserve_prepared_text_retained_bytes(
        &self,
        bytes: usize,
    ) -> Result<PreparedTextRetainedReservation> {
        if let Some(error) = self.rejection.borrow().clone() {
            return Err(error.into());
        }
        self.meter
            .checkpoint(merman_model::OperationPhase::Layout)?;
        self.meter
            .reserve_prepared_text_retained_bytes(bytes)
            .map_err(Error::from)
    }

    pub(crate) fn checked_add(&self, left: usize, right: usize) -> Result<usize> {
        self.meter
            .checkpoint(merman_model::OperationPhase::Layout)
            .map_err(Error::from)?;
        left.checked_add(right)
            .ok_or_else(|| self.record_arithmetic_overflow().into())
    }

    pub(crate) fn checked_mul(&self, left: usize, right: usize) -> Result<usize> {
        self.meter
            .checkpoint(merman_model::OperationPhase::Layout)
            .map_err(Error::from)?;
        left.checked_mul(right)
            .ok_or_else(|| self.record_arithmetic_overflow().into())
    }

    pub(crate) fn record_arithmetic_overflow(&self) -> OperationWorkError {
        if let Some(error) = self.rejection.borrow().clone() {
            return error;
        }
        let error = self.meter.arithmetic_overflow();
        *self.rejection.borrow_mut() = Some(error.clone());
        error
    }

    #[cfg(feature = "layout-elk")]
    pub(crate) fn arithmetic_overflow(&self) -> OperationWorkError {
        self.record_arithmetic_overflow()
    }

    pub(crate) fn map_dugong_error(&mut self, error: impl Into<dugong::LayoutError>) -> Error {
        match error.into() {
            dugong::LayoutError::Work(dugong::WorkError::Interrupted) => {
                let rejection = self.rejection.borrow().clone();
                rejection
                    .map(Error::from)
                    .unwrap_or_else(|| Error::from(self.record_arithmetic_overflow()))
            }
            dugong::LayoutError::Work(dugong::WorkError::ArithmeticOverflow) => {
                Error::from(self.record_arithmetic_overflow())
            }
            error => error.into(),
        }
    }

    #[cfg(feature = "layout-shoal")]
    pub(crate) fn map_shoal_error(&self, error: merman_layout_shoal::LayoutError) -> Error {
        use merman_layout_shoal::{LayoutError, WorkError};

        match error {
            LayoutError::Work(WorkError::Interrupted) => {
                self.rejection.borrow().clone().map_or_else(
                    || Error::InvalidModel {
                        message: "Shoal interrupted without an operation failure".into(),
                    },
                    Error::from,
                )
            }
            LayoutError::Work(WorkError::ArithmeticOverflow) => {
                self.record_arithmetic_overflow().into()
            }
            LayoutError::Unsupported(construct) => Error::UnsupportedLayoutConstruct {
                backend: "shoal",
                construct,
            },
            LayoutError::InvalidInput(detail) => Error::InvalidModel {
                message: format!("Invalid Shoal graph: {detail}"),
            },
            LayoutError::AllocationLimit => Error::InvalidModel {
                message: error.to_string(),
            },
        }
    }

    #[cfg(feature = "layout-elk")]
    pub(crate) fn map_elk_error(&mut self, error: merman_layout_elk::Error) -> Error {
        self.map_elk_error_with_context(error, "ELK")
    }

    #[cfg(feature = "layout-elk")]
    pub(crate) fn map_elk_error_with_context(
        &mut self,
        error: merman_layout_elk::Error,
        context: &str,
    ) -> Error {
        match error.work_error() {
            Some(merman_layout_elk::WorkError::Interrupted) => {
                let rejection = self.rejection.borrow_mut().take();
                rejection
                    .map(Error::from)
                    .unwrap_or_else(|| Error::from(self.record_arithmetic_overflow()))
            }
            Some(merman_layout_elk::WorkError::ArithmeticOverflow) => {
                Error::from(self.record_arithmetic_overflow())
            }
            None => Error::InvalidModel {
                message: format!("{context} layout failed: {error}"),
            },
        }
    }

    #[cfg(feature = "layout-cytoscape")]
    pub(crate) fn map_manatee_error(&mut self, error: manatee::Error) -> Error {
        match error {
            manatee::Error::WorkFailure(manatee::WorkFailure::Interrupted) => self
                .rejection
                .borrow_mut()
                .take()
                .map(Error::from)
                .unwrap_or_else(|| Error::InvalidModel {
                    message: "manatee work control interrupted without a resource error"
                        .to_string(),
                }),
            manatee::Error::WorkFailure(manatee::WorkFailure::ArithmeticOverflow) => {
                Error::from(self.record_arithmetic_overflow())
            }
            error => Error::InvalidModel {
                message: format!("manatee layout failed: {error}"),
            },
        }
    }

    #[cfg(all(test, feature = "layout-elk"))]
    pub(crate) fn adapter_work(&self) -> usize {
        self.adapter_work.get()
    }
}

pub(crate) type DugongOperationWorkControl = OperationLayoutWorkControl;

#[cfg(feature = "layout-elk")]
pub(crate) type ElkOperationWorkControl = OperationLayoutWorkControl;

impl dugong::WorkControl for OperationLayoutWorkControl {
    fn charge(&mut self, units: usize) -> std::result::Result<(), dugong::WorkError> {
        if self.rejection.borrow().is_some() {
            return Err(dugong::WorkError::Interrupted);
        }
        self.meter.charge(units).map_err(|error| {
            *self.rejection.borrow_mut() = Some(error);
            dugong::WorkError::Interrupted
        })
    }
}

#[cfg(feature = "layout-shoal")]
impl merman_layout_shoal::WorkControl for OperationLayoutWorkControl {
    fn check(&mut self) -> std::result::Result<(), merman_layout_shoal::WorkError> {
        self.charge(0)
    }

    fn charge(&mut self, units: usize) -> std::result::Result<(), merman_layout_shoal::WorkError> {
        if self.rejection.borrow().is_some() {
            return Err(merman_layout_shoal::WorkError::Interrupted);
        }
        self.meter.charge(units).map_err(|error| {
            *self.rejection.borrow_mut() = Some(error);
            merman_layout_shoal::WorkError::Interrupted
        })
    }
}

#[cfg(feature = "layout-cytoscape")]
impl manatee::WorkControl for OperationLayoutWorkControl {
    fn check(&mut self, units: usize) -> std::result::Result<(), manatee::WorkFailure> {
        if self.rejection.borrow().is_some() {
            return Err(manatee::WorkFailure::Interrupted);
        }
        self.meter.preflight(units).map_err(|error| {
            *self.rejection.borrow_mut() = Some(error);
            manatee::WorkFailure::Interrupted
        })
    }

    fn charge(&mut self, units: usize) -> std::result::Result<(), manatee::WorkFailure> {
        if self.rejection.borrow().is_some() {
            return Err(manatee::WorkFailure::Interrupted);
        }
        self.meter.charge(units).map_err(|error| {
            *self.rejection.borrow_mut() = Some(error);
            manatee::WorkFailure::Interrupted
        })
    }
}

#[cfg(feature = "layout-elk")]
impl merman_layout_elk::WorkControl for OperationLayoutWorkControl {
    fn check(&mut self, units: usize) -> std::result::Result<(), merman_layout_elk::WorkError> {
        if self.rejection.borrow().is_some() {
            return Err(merman_layout_elk::WorkError::Interrupted);
        }
        self.meter.preflight(units).map_err(|error| {
            *self.rejection.borrow_mut() = Some(error);
            merman_layout_elk::WorkError::Interrupted
        })
    }

    fn charge(&mut self, units: usize) -> std::result::Result<(), merman_layout_elk::WorkError> {
        if self.rejection.borrow().is_some() {
            return Err(merman_layout_elk::WorkError::Interrupted);
        }
        self.meter.charge(units).map_err(|error| {
            *self.rejection.borrow_mut() = Some(error);
            merman_layout_elk::WorkError::Interrupted
        })
    }
}

#[cfg(all(test, feature = "layout-elk"))]
mod tests {
    use super::ElkOperationWorkControl;
    use crate::Error;
    use crate::resources::{
        OperationWorkMeter, RenderResourcePolicy, RenderResourceProfile, ResourceLimitCause,
        ResourceLimitExceeded, ResourceLimitId, ResourceLimitOverride, ResourceLimitPhase,
    };
    use merman_layout_elk::{
        Error as ElkError, WorkControl as ElkWorkControl, WorkError as ElkWorkError,
    };
    use merman_model::{
        CancelReason, OperationCancelled, OperationControl, OperationLedgerError, OperationPhase,
        OperationResourceDomain, OperationResourceOverride, OperationResourceProvenance,
    };
    use std::sync::Arc;

    #[test]
    fn elk_adapter_cancellation_boundaries_preserve_usage_and_first_terminal() {
        const PREFIX: usize = 3;
        const NEXT: usize = 2;
        let policy = RenderResourcePolicy::constrained()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, PREFIX + NEXT)
            .unwrap();

        for cancel_after_check in [true, false] {
            let control = OperationControl::new();
            let meter = Arc::new(OperationWorkMeter::new_with_control(
                policy,
                control.clone(),
            ));
            let mut work_control = ElkOperationWorkControl::new(Arc::clone(&meter));
            ElkWorkControl::check(&mut work_control, PREFIX).unwrap();
            ElkWorkControl::charge(&mut work_control, PREFIX).unwrap();
            assert_eq!(meter.used(), PREFIX);

            let interrupted = if cancel_after_check {
                ElkWorkControl::check(&mut work_control, NEXT).unwrap();
                assert_eq!(meter.used(), PREFIX);
                control.cancel();
                ElkWorkControl::charge(&mut work_control, NEXT).unwrap_err()
            } else {
                control.cancel();
                ElkWorkControl::check(&mut work_control, NEXT).unwrap_err()
            };
            assert_eq!(interrupted, ElkWorkError::Interrupted);
            assert_eq!(meter.used(), PREFIX);
            let Error::Cancelled(first) = work_control.map_elk_error(ElkError::Work(interrupted))
            else {
                panic!("expected structured operation cancellation");
            };
            assert_eq!(
                first,
                OperationCancelled {
                    phase: OperationPhase::Layout,
                    reason: CancelReason::Requested,
                }
            );
            assert!(work_control.rejection.borrow().is_none());

            for replay_with_check in [true, false] {
                let interrupted = if replay_with_check {
                    ElkWorkControl::check(&mut work_control, NEXT).unwrap_err()
                } else {
                    ElkWorkControl::charge(&mut work_control, NEXT).unwrap_err()
                };
                assert_eq!(interrupted, ElkWorkError::Interrupted);
                let Error::Cancelled(replayed) =
                    work_control.map_elk_error(ElkError::Work(interrupted))
                else {
                    panic!("expected cancellation after consuming the adapter rejection");
                };
                assert_eq!(replayed, first);
                assert!(work_control.rejection.borrow().is_none());
                assert_eq!(meter.used(), PREFIX);
            }

            let Error::Cancelled(replayed) =
                work_control.map_elk_error(ElkError::Work(ElkWorkError::ArithmeticOverflow))
            else {
                panic!("expected the latched cancellation to precede overflow");
            };
            assert_eq!(replayed, first);
            assert_eq!(meter.used(), PREFIX);
            assert_eq!(
                control.terminal_checkpoint_at(OperationPhase::Emit),
                Err(OperationLedgerError::Cancelled(first))
            );

            let fresh_control = OperationControl::new();
            let fresh_meter = Arc::new(OperationWorkMeter::new_with_control(
                policy,
                fresh_control.clone(),
            ));
            let mut fresh = ElkOperationWorkControl::new(Arc::clone(&fresh_meter));
            for units in [PREFIX, NEXT] {
                ElkWorkControl::check(&mut fresh, units).unwrap();
                ElkWorkControl::charge(&mut fresh, units).unwrap();
            }
            assert_eq!(fresh_meter.used(), PREFIX + NEXT);
            assert_eq!(
                fresh_control.terminal_checkpoint_at(OperationPhase::Layout),
                Ok(())
            );
        }
    }

    #[test]
    fn elk_adapter_overflow_mapping_preserves_full_first_terminal() {
        const PREFIX: usize = 3;
        let policy = RenderResourcePolicy::constrained()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 5)
            .unwrap()
            .with_limit(ResourceLimitId::MaxSvgBytes, 17)
            .unwrap();
        let control = OperationControl::new();
        let meter = Arc::new(OperationWorkMeter::new_with_control(
            policy,
            control.clone(),
        ));
        let mut work_control = ElkOperationWorkControl::new(Arc::clone(&meter));
        ElkWorkControl::check(&mut work_control, PREFIX).unwrap();
        ElkWorkControl::charge(&mut work_control, PREFIX).unwrap();

        let Error::ResourceLimitExceeded(first) =
            work_control.map_elk_error(ElkError::Work(ElkWorkError::ArithmeticOverflow))
        else {
            panic!("expected structured layout-work overflow");
        };
        assert_eq!(
            first,
            ResourceLimitExceeded {
                cause: ResourceLimitCause::ArithmeticOverflow,
                phase: ResourceLimitPhase::LayoutModel,
                limit: "max_layout_work_units",
                actual: usize::MAX,
                max: 5,
                profile: RenderResourceProfile::Constrained,
                explicit_overrides: vec![
                    ResourceLimitOverride {
                        id: ResourceLimitId::MaxLayoutWorkUnits,
                        value: 5,
                    },
                    ResourceLimitOverride {
                        id: ResourceLimitId::MaxSvgBytes,
                        value: 17,
                    },
                ],
            }
        );
        let terminal = OperationLedgerError::ArithmeticOverflow {
            id: "max_layout_work_units",
            phase: OperationPhase::Layout,
            resource_phase: "layout_model",
            actual: usize::MAX as u64,
            maximum: 5,
            provenance: OperationResourceProvenance::new(
                OperationResourceDomain::Render,
                Some(RenderResourceProfile::Constrained),
                [
                    OperationResourceOverride {
                        id: "max_layout_work_units",
                        value: 5,
                    },
                    OperationResourceOverride {
                        id: "max_svg_bytes",
                        value: 17,
                    },
                ],
            ),
        };
        assert_eq!(meter.used(), PREFIX);
        assert_eq!(
            control.terminal_checkpoint_at(OperationPhase::Layout),
            Err(terminal.clone())
        );

        control.cancel();
        for replay_with_check in [true, false] {
            let interrupted = if replay_with_check {
                ElkWorkControl::check(&mut work_control, 2).unwrap_err()
            } else {
                ElkWorkControl::charge(&mut work_control, 2).unwrap_err()
            };
            assert_eq!(interrupted, ElkWorkError::Interrupted);
            let Error::ResourceLimitExceeded(replayed) =
                work_control.map_elk_error(ElkError::Work(interrupted))
            else {
                panic!("expected overflow to remain the first terminal after cancellation");
            };
            assert_eq!(replayed, first);
            assert!(work_control.rejection.borrow().is_none());
            assert_eq!(meter.used(), PREFIX);
        }
        let Error::ResourceLimitExceeded(replayed) =
            work_control.map_elk_error(ElkError::Work(ElkWorkError::ArithmeticOverflow))
        else {
            panic!("expected the exact overflow after consuming the adapter rejection");
        };
        assert_eq!(replayed, first);
        assert_eq!(meter.used(), PREFIX);
        assert_eq!(
            control.terminal_checkpoint_at(OperationPhase::Emit),
            Err(terminal)
        );
    }
}
