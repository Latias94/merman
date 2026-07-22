use std::sync::{LockResult, Mutex, MutexGuard, PoisonError};

pub(crate) fn recover_poison<T>(result: LockResult<T>) -> T {
    result.unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn lock_recovering_poison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    recover_poison(mutex.lock())
}
