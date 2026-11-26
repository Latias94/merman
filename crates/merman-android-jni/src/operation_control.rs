use merman_bindings_core::transport_handles::OperationControlRegistry;
use merman_bindings_core::{BindingError, OperationControl};
use std::time::Duration;

pub(crate) struct JniOperationControlRegistry(OperationControlRegistry);

impl JniOperationControlRegistry {
    pub(crate) const fn new() -> Self {
        Self(OperationControlRegistry::new(i64::MAX as u64))
    }

    pub(crate) fn issue(&mut self, timeout_ms: Option<u64>) -> Result<u64, BindingError> {
        let control = OperationControl::new();
        let control = match timeout_ms {
            Some(timeout_ms) => control.with_deadline(Duration::from_millis(timeout_ms)),
            None => control,
        };
        self.0.try_insert(control).map_err(|_| {
            BindingError::internal("Android operation-control token space is exhausted")
        })
    }

    pub(crate) fn acquire(&self, token: u64) -> Result<OperationControl, BindingError> {
        self.validate_issued(token)?;
        self.0.get(token).cloned().ok_or_else(|| {
            BindingError::invalid_argument(
                "Android operation-control token is unknown or has already been released",
            )
        })
    }

    pub(crate) fn release(&mut self, token: u64) -> Result<Option<OperationControl>, BindingError> {
        self.validate_issued(token)?;
        Ok(self.0.remove(token))
    }

    fn validate_issued(&self, token: u64) -> Result<(), BindingError> {
        if self.0.was_issued(token) {
            Ok(())
        } else {
            Err(BindingError::invalid_argument(
                "Android operation-control token is zero or was never issued",
            ))
        }
    }
}
