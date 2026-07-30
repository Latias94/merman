#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

#[derive(Debug, Default)]
pub(super) struct SessionLifecycle {
    terminated: AtomicBool,
    changed: Notify,
    #[cfg(test)]
    termination_count: AtomicUsize,
}

impl SessionLifecycle {
    pub(super) fn terminate(&self) -> bool {
        if self.terminated.swap(true, Ordering::AcqRel) {
            return false;
        }

        #[cfg(test)]
        self.termination_count.fetch_add(1, Ordering::Relaxed);
        self.changed.notify_waiters();
        true
    }

    pub(super) fn is_terminated(&self) -> bool {
        self.terminated.load(Ordering::Acquire)
    }

    pub(super) async fn terminated(&self) {
        loop {
            let changed = self.changed.notified();
            if self.is_terminated() {
                return;
            }
            changed.await;
        }
    }

    #[cfg(test)]
    pub(super) fn termination_count(&self) -> usize {
        self.termination_count.load(Ordering::Relaxed)
    }
}
