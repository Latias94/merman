use merman_bindings_core::transport_handles::{
    OperationControlHandleError, OperationControlRegistry,
};
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
        self.0.acquire(token).map_err(control_handle_error)
    }

    pub(crate) fn release(&mut self, token: u64) -> Result<Option<OperationControl>, BindingError> {
        match self.0.retire(token) {
            Ok(control) => Ok(Some(control)),
            Err(OperationControlHandleError::Retired) => Ok(None),
            Err(error) => Err(control_handle_error(error)),
        }
    }
}

fn control_handle_error(error: OperationControlHandleError) -> BindingError {
    BindingError::invalid_argument(match error {
        OperationControlHandleError::NeverIssued => {
            "Android operation-control token is zero or was never issued"
        }
        OperationControlHandleError::Retired => {
            "Android operation-control token is unknown or has already been released"
        }
    })
}
