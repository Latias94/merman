use crate::sync::lock_recovering_poison;
use std::sync::Mutex;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

type TerminationHook = Box<dyn FnOnce() + Send + 'static>;

#[derive(Default)]
struct LifecycleGate {
    termination_hook: Option<TerminationHook>,
    termination_hook_registered: bool,
}

impl std::fmt::Debug for LifecycleGate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LifecycleGate")
            .field("has_termination_hook", &self.termination_hook.is_some())
            .field(
                "termination_hook_registered",
                &self.termination_hook_registered,
            )
            .finish()
    }
}

#[derive(Debug)]
pub(super) struct SessionLifecycle {
    terminated: AtomicBool,
    commit_gate: Mutex<LifecycleGate>,
    changed: Notify,
    #[cfg(test)]
    termination_count: AtomicUsize,
}

impl Default for SessionLifecycle {
    fn default() -> Self {
        Self {
            terminated: AtomicBool::new(false),
            commit_gate: Mutex::new(LifecycleGate::default()),
            changed: Notify::new(),
            #[cfg(test)]
            termination_count: AtomicUsize::new(0),
        }
    }
}

impl SessionLifecycle {
    pub(super) fn terminate(&self) -> bool {
        let termination_hook = {
            let mut gate = lock_recovering_poison(&self.commit_gate);
            if self.terminated.swap(true, Ordering::AcqRel) {
                return false;
            }

            #[cfg(test)]
            self.termination_count.fetch_add(1, Ordering::Relaxed);
            gate.termination_hook.take()
        };
        if let Some(termination_hook) = termination_hook {
            termination_hook();
        }
        self.changed.notify_waiters();
        true
    }

    pub(super) fn register_termination_hook(
        &self,
        termination_hook: impl FnOnce() + Send + 'static,
    ) {
        let mut termination_hook = Some(Box::new(termination_hook) as TerminationHook);
        {
            let mut gate = lock_recovering_poison(&self.commit_gate);
            assert!(
                !gate.termination_hook_registered,
                "session protocol termination hook registered more than once"
            );
            gate.termination_hook_registered = true;
            if !self.terminated.load(Ordering::Acquire) {
                gate.termination_hook = termination_hook.take();
            }
        }
        if let Some(termination_hook) = termination_hook {
            termination_hook();
        }
    }

    /// Runs one short, synchronous state mutation unless termination linearized first.
    ///
    /// The caller must never hold this gate across an await. Session state is locked before this
    /// method is entered, while termination only takes this gate, so the lock order cannot cycle.
    pub(super) fn commit_if_active<T>(&self, mutation: impl FnOnce() -> T) -> Option<T> {
        let _gate = lock_recovering_poison(&self.commit_gate);
        if self.terminated.load(Ordering::Acquire) {
            return None;
        }
        Some(mutation())
    }

    pub(super) fn is_terminated(&self) -> bool {
        self.terminated.load(Ordering::Acquire)
    }

    pub(super) async fn terminated(&self) {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.is_terminated() {
                return;
            }
            changed.as_mut().await;
        }
    }

    #[cfg(test)]
    pub(super) fn termination_count(&self) -> usize {
        self.termination_count.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::SessionLifecycle;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn termination_hook_runs_exactly_once() {
        let lifecycle = SessionLifecycle::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let hook_calls = Arc::clone(&calls);
        lifecycle.register_termination_hook(move || {
            hook_calls.fetch_add(1, Ordering::Relaxed);
        });

        assert!(lifecycle.terminate());
        assert!(!lifecycle.terminate());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn hook_registered_after_termination_runs_immediately() {
        let lifecycle = SessionLifecycle::default();
        assert!(lifecycle.terminate());
        let calls = Arc::new(AtomicUsize::new(0));
        let hook_calls = Arc::clone(&calls);

        lifecycle.register_termination_hook(move || {
            hook_calls.fetch_add(1, Ordering::Relaxed);
        });

        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}
