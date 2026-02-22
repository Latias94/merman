use chrono::{DateTime, FixedOffset, NaiveDateTime};

/// 在当前线程内覆盖“本地时区偏移”（分钟）。
///
/// Mermaid（尤其是 Gantt）在若干地方遵循 JavaScript 的本地时间语义，这会导致同一份 fixture
/// 在不同时区的 CI runner 上产生不同的快照输出。
///
/// 该函数提供一个最小侵入的方式：在闭包执行期间把“本地时区”固定为一个 `FixedOffset`，
/// 以获得可复现的结果。`None` 表示使用系统 `chrono::Local`。
pub fn with_fixed_local_offset_minutes<R>(offset_minutes: Option<i32>, f: impl FnOnce() -> R) -> R {
    crate::runtime::with_fixed_local_offset_minutes(offset_minutes, f)
}

/// 将一个“本地时间”的 `NaiveDateTime` 解释为某个本地时区下的绝对时间点。
///
/// 当 `with_fixed_local_offset_minutes(Some(x))` 生效时，使用固定偏移；否则使用系统本地时区。
pub fn datetime_from_naive_local(naive: NaiveDateTime) -> DateTime<FixedOffset> {
    crate::runtime::datetime_from_naive_local(naive)
}

/// 把一个绝对时间点映射到“本地时间”（以 `FixedOffset` 表示）。
///
/// 当 `with_fixed_local_offset_minutes(Some(x))` 生效时，使用固定偏移；否则使用系统本地时区。
pub fn datetime_to_local_fixed(dt: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    crate::runtime::datetime_to_local_fixed(dt)
}

/// 获取一个绝对时间点在“本地时间”语义下对应的 `NaiveDateTime`。
pub fn datetime_to_naive_local(dt: DateTime<FixedOffset>) -> NaiveDateTime {
    crate::runtime::datetime_to_naive_local(dt)
}
