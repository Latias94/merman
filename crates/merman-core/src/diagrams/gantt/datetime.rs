use super::*;

pub(super) fn today_midnight_local() -> DateTimeFixed {
    let date = crate::runtime::today_naive_local();
    let naive = date.and_hms_opt(0, 0, 0).unwrap_or_else(|| {
        NaiveDate::from_ymd_opt(1970, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    });
    local_from_naive(naive)
}

pub(super) fn local_from_naive(naive: NaiveDateTime) -> DateTimeFixed {
    match Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => dt.fixed_offset(),
        chrono::LocalResult::Ambiguous(a, _b) => a.fixed_offset(),
        chrono::LocalResult::None => chrono::DateTime::<FixedOffset>::from_naive_utc_and_offset(
            naive,
            FixedOffset::east_opt(0).unwrap(),
        ),
    }
}

pub(super) fn add_days_local(dt: DateTimeFixed, days: i64) -> Option<DateTimeFixed> {
    let local = dt.with_timezone(&Local);
    let naive = local.naive_local();
    let date = naive.date();
    let time = naive.time();

    let new_date = if days >= 0 {
        date.checked_add_days(chrono::Days::new(days as u64))?
    } else {
        date.checked_sub_days(chrono::Days::new((-days) as u64))?
    };
    Some(local_from_naive(NaiveDateTime::new(new_date, time)))
}

pub(super) fn add_months_local(dt: DateTimeFixed, months: i64) -> Option<DateTimeFixed> {
    let local = dt.with_timezone(&Local);
    let naive = local.naive_local();
    let mut year = naive.year();
    let mut month0 = naive.month0() as i64; // 0..=11

    month0 += months;
    year += month0.div_euclid(12) as i32;
    month0 = month0.rem_euclid(12);

    let month = (month0 as u32) + 1;
    let day = naive.day().min(last_day_of_month(year, month));
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    Some(local_from_naive(NaiveDateTime::new(date, naive.time())))
}

pub(super) fn add_years_local(dt: DateTimeFixed, years: i64) -> Option<DateTimeFixed> {
    let local = dt.with_timezone(&Local);
    let naive = local.naive_local();
    let year = naive.year().checked_add(years as i32)?;
    let month = naive.month();
    let day = naive.day().min(last_day_of_month(year, month));
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    Some(local_from_naive(NaiveDateTime::new(date, naive.time())))
}

pub(super) fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
    let last = first_next
        .pred_opt()
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
    last.day()
}
