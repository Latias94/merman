use chrono::NaiveDate;
use std::cell::Cell;

thread_local! {
    static FIXED_TODAY_LOCAL: Cell<Option<NaiveDate>> = const { Cell::new(None) };
}

pub(crate) fn with_fixed_today_local<R>(today: Option<NaiveDate>, f: impl FnOnce() -> R) -> R {
    FIXED_TODAY_LOCAL.with(|cell| {
        let prev = cell.replace(today);
        let out = f();
        cell.set(prev);
        out
    })
}

pub(crate) fn today_naive_local() -> NaiveDate {
    FIXED_TODAY_LOCAL
        .with(|cell| cell.get())
        .unwrap_or_else(|| chrono::Local::now().date_naive())
}
