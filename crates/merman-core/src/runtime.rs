//! Ambient operation context for source orchestration.

use merman_model::runtime::OperationContext;
#[cfg(any(
    test,
    feature = "diagram-gantt",
    feature = "diagram-block",
    feature = "diagram-git-graph",
    feature = "diagram-mindmap"
))]
use merman_model::runtime::RuntimePolicy;
#[cfg(test)]
use merman_model::runtime::{RuntimeCapability, RuntimePolicyError, compiled_system_adapter_ids};
#[cfg(test)]
use merman_model::time::CivilDate;
use std::cell::RefCell;

thread_local! {
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "Clippy 1.100 flags the standard library TLS fallback even though this initializer is const"
    )]
    static OPERATION_CONTEXT: RefCell<Option<OperationContext>> = const { RefCell::new(None) };
}

pub(crate) fn with_operation_context<R>(context: &OperationContext, f: impl FnOnce() -> R) -> R {
    OPERATION_CONTEXT.with(|cell| {
        let previous = cell.replace(Some(context.clone()));
        struct Restore<'a> {
            cell: &'a RefCell<Option<OperationContext>>,
            previous: Option<OperationContext>,
        }
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.cell.replace(self.previous.take());
            }
        }
        let _restore = Restore { cell, previous };
        f()
    })
}

#[cfg(test)]
pub(crate) fn today_local() -> CivilDate {
    active_operation_context().today_local()
}

#[cfg(any(test, feature = "diagram-git-graph", feature = "diagram-mindmap"))]
pub(crate) fn generated_id_hex(domain: &str, counter: u64, len: usize) -> String {
    let context = active_operation_context();
    context.derive_hex(domain, counter, len)
}

#[cfg(any(
    test,
    feature = "diagram-gantt",
    feature = "diagram-block",
    feature = "diagram-git-graph",
    feature = "diagram-mindmap"
))]
pub(crate) fn active_operation_context() -> OperationContext {
    OPERATION_CONTEXT
        .with(|cell| cell.borrow().clone())
        .unwrap_or_else(|| {
            RuntimePolicy::deterministic()
                .begin_operation()
                .expect("the deterministic runtime policy is infallible")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_pre_cancel_precedes_runtime_error_and_runtime_precedes_detection() {
        let parser = crate::SemanticParser::new()
            .with_runtime_policy(RuntimePolicy::deterministic().with_test_system_clock());
        for known_type in [false, true] {
            let parse = |control: &crate::OperationControl| {
                if known_type {
                    parser.parse_diagram_with_type_controlled_sync("unknown", "unknown", control)
                } else {
                    parser.parse_diagram_controlled_sync("unknown", control)
                }
            };
            let cancelled = crate::OperationControl::new();
            cancelled.cancel();
            assert!(parse(&cancelled).is_err());
            if !compiled_system_adapter_ids().contains(&RuntimeCapability::SystemClock.id()) {
                assert!(matches!(
                    parse(&crate::OperationControl::new()).unwrap(),
                    Err(crate::Error::RuntimePolicy(
                        RuntimePolicyError::MissingCapability(RuntimeCapability::SystemClock)
                    ))
                ));
            }
        }
    }

    fn date(year: i32, month: u32, day: u32) -> CivilDate {
        CivilDate::new(year, month, day).expect("valid test date")
    }

    #[test]
    fn operation_context_restores_after_panic() {
        let outer = RuntimePolicy::deterministic()
            .with_fixed_today(Some(date(2026, 7, 18)))
            .begin_operation()
            .unwrap();
        let inner = RuntimePolicy::deterministic()
            .with_fixed_today(Some(date(2030, 1, 2)))
            .begin_operation()
            .unwrap();

        with_operation_context(&outer, || {
            let panic = std::panic::catch_unwind(|| {
                with_operation_context(&inner, || panic!("test panic"));
            });
            assert!(panic.is_err());
            assert_eq!(today_local(), outer.today_local());
        });
    }
}
