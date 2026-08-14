#[path = "../src/operation_control.rs"]
mod operation_control;
#[path = "../src/token.rs"]
mod token;

use merman_bindings_core::{BindingStatus, OperationPhase};
use operation_control::JniOperationControlRegistry;
use std::thread;
use token::next_monotonic_jni_token;

#[test]
fn shared_jni_token_allocator_starts_positive_and_preserves_domain_errors() {
    assert_eq!(
        next_monotonic_jni_token(0, "Android engine token space is exhausted")
            .expect("first JNI token"),
        1
    );
    assert_eq!(
        next_monotonic_jni_token(41, "Android operation-control token space is exhausted")
            .expect("next JNI token"),
        42
    );

    for message in [
        "Android engine token space is exhausted",
        "Android operation-control token space is exhausted",
    ] {
        let exhausted = next_monotonic_jni_token(i64::MAX as u64, message)
            .expect_err("signed JNI token space must be bounded");
        assert_eq!(exhausted.status(), BindingStatus::InternalError);
        assert_eq!(exhausted.message(), message);
    }
}

#[test]
fn operation_control_tokens_are_monotonic_and_release_is_idempotent() {
    let mut registry = JniOperationControlRegistry::default();
    let first = registry.issue(None).expect("first operation control");

    registry.release(first).expect("first release");
    registry.release(first).expect("idempotent second release");

    let second = registry.issue(None).expect("second operation control");
    assert!(
        second > first,
        "released operation-control tokens are never reused"
    );

    let released = registry
        .acquire(first)
        .expect_err("released operation-control tokens must not be acquired");
    assert_eq!(released.status(), BindingStatus::InvalidArgument);

    let future = registry
        .release(second + 1)
        .expect_err("never-issued operation-control tokens must be rejected");
    assert_eq!(future.status(), BindingStatus::InvalidArgument);
}

#[test]
fn operation_control_clones_share_cancellation_across_threads() {
    let mut registry = JniOperationControlRegistry::default();
    let token = registry.issue(None).expect("operation control");
    let worker_control = registry.acquire(token).expect("worker control clone");

    thread::spawn(move || worker_control.cancel())
        .join()
        .expect("cancellation worker");

    assert!(
        registry
            .acquire(token)
            .expect("shared control clone")
            .is_cancelled()
    );
}

#[test]
fn zero_timeout_installs_a_relative_deadline() {
    let mut registry = JniOperationControlRegistry::default();
    let token = registry.issue(Some(0)).expect("deadline control");
    let cancelled = registry
        .acquire(token)
        .expect("deadline control clone")
        .checkpoint_at(OperationPhase::Admission)
        .expect_err("zero timeout must expire at the first checkpoint");

    assert_eq!(cancelled.reason.as_str(), "deadline_exceeded");
    assert_eq!(cancelled.phase, OperationPhase::Admission);
}
