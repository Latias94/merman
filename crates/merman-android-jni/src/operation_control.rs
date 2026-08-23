use crate::token::next_monotonic_jni_token;
use merman_bindings_core::{BindingError, OperationControl};
use std::{collections::BTreeMap, time::Duration};

/// Owns Android operation controls behind monotonically increasing opaque tokens.
#[derive(Default)]
pub(crate) struct JniOperationControlRegistry {
    last_token: u64,
    controls: BTreeMap<u64, OperationControl>,
}

impl JniOperationControlRegistry {
    pub(crate) fn issue(&mut self, timeout_ms: Option<u64>) -> Result<u64, BindingError> {
        let token = next_monotonic_jni_token(
            self.last_token,
            "Android operation-control token space is exhausted",
        )?;
        let control = timeout_ms.map_or_else(OperationControl::new, |timeout_ms| {
            OperationControl::new().with_deadline(Duration::from_millis(timeout_ms))
        });

        self.last_token = token;
        let previous = self.controls.insert(token, control);
        debug_assert!(
            previous.is_none(),
            "Android operation-control tokens are never reused"
        );
        Ok(token)
    }

    pub(crate) fn acquire(&self, token: u64) -> Result<OperationControl, BindingError> {
        self.validate_issued_token(token)?;
        self.controls.get(&token).cloned().ok_or_else(|| {
            BindingError::invalid_argument(
                "Android operation-control token is unknown or has already been released",
            )
        })
    }

    pub(crate) fn release(&mut self, token: u64) -> Result<(), BindingError> {
        self.validate_issued_token(token)?;
        self.controls.remove(&token);
        Ok(())
    }

    fn validate_issued_token(&self, token: u64) -> Result<(), BindingError> {
        if token == 0 || token > self.last_token {
            return Err(BindingError::invalid_argument(
                "Android operation-control token is zero or was never issued",
            ));
        }
        Ok(())
    }
}
