use super::*;

pub(super) fn today_midnight_local() -> Option<DateTimeFixed> {
    let date = crate::runtime::today_naive_local();
    let naive = NaiveDateTime::new(date, chrono::NaiveTime::MIN);
    local_from_naive(naive)
}

pub(super) fn local_from_naive(naive: NaiveDateTime) -> Option<DateTimeFixed> {
    crate::runtime::datetime_from_naive_local(naive)
}

pub(super) fn add_days_local(dt: DateTimeFixed, days: i64) -> Option<DateTimeFixed> {
    let naive = crate::runtime::datetime_to_naive_local(dt);
    let date = naive.date();
    let time = naive.time();

    let magnitude = chrono::Days::new(days.unsigned_abs());
    let new_date = if days >= 0 {
        date.checked_add_days(magnitude)?
    } else {
        date.checked_sub_days(magnitude)?
    };
    local_from_naive(NaiveDateTime::new(new_date, time))
}

pub(super) fn add_months_local(dt: DateTimeFixed, months: i64) -> Option<DateTimeFixed> {
    let naive = crate::runtime::datetime_to_naive_local(dt);
    let month_index = i64::from(naive.month0()).checked_add(months)?;
    let year_delta: i32 = month_index.div_euclid(12).try_into().ok()?;
    let year = naive.year().checked_add(year_delta)?;
    let month = u32::try_from(month_index.rem_euclid(12))
        .ok()?
        .checked_add(1)?;
    let day = naive.day().min(last_day_of_month(year, month));
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    local_from_naive(NaiveDateTime::new(date, naive.time()))
}

pub(super) fn add_years_local(dt: DateTimeFixed, years: i64) -> Option<DateTimeFixed> {
    let naive = crate::runtime::datetime_to_naive_local(dt);
    let years: i32 = years.try_into().ok()?;
    let year = naive.year().checked_add(years)?;
    let month = naive.month();
    let day = naive.day().min(last_day_of_month(year, month));
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    local_from_naive(NaiveDateTime::new(date, naive.time()))
}

pub(super) fn last_day_of_month(year: i32, month: u32) -> u32 {
    let Some((next_year, next_month)) = next_month_start(year, month) else {
        return 31;
    };
    let Some(first_next) = NaiveDate::from_ymd_opt(next_year, next_month, 1) else {
        return 31;
    };
    first_next.pred_opt().map_or(1, |last| last.day())
}

fn next_month_start(year: i32, month: u32) -> Option<(i32, u32)> {
    match month {
        1..=11 => Some((year, month + 1)),
        12 => year.checked_add(1).map(|next_year| (next_year, 1)),
        _ => None,
    }
}
