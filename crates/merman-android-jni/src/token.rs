use merman_bindings_core::BindingError;

/// Allocates the next positive token representable by JNI's signed `jlong` transport.
pub(crate) fn next_monotonic_jni_token(
    last_token: u64,
    exhausted_message: &str,
) -> Result<u64, BindingError> {
    last_token
        .checked_add(1)
        .filter(|token| *token <= i64::MAX as u64)
        .ok_or_else(|| BindingError::internal(exhausted_message))
}
