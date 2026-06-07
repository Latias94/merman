# HPD-050 - Gantt Datetime Fallback Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`crates/merman-core/src/diagrams/gantt/datetime.rs` still had production `unwrap()` calls while
constructing fixed fallback date/time values:

```rust
NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
```

These values were constants, but the code sat in ordinary Gantt date helpers used by public parse
paths. `last_day_of_month(...)` also used unchecked December year rollover before asking chrono for
the next month start.

## Changes

- Replaced `today_midnight_local()`'s `and_hms_opt(...).unwrap_or_else(...)` chain with direct
  `NaiveDateTime::new(date, chrono::NaiveTime::MIN)` construction.
- Added `next_month_start(...)` so `last_day_of_month(...)` handles invalid internal months and
  overflowing December years through explicit `Option` branches instead of fallback unwraps.
- Preserved ordinary leap/non-leap month behavior.
- Added a focused Gantt regression for February leap/non-leap values, invalid internal month, and
  `i32::MAX` December rollover.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core gantt` - passed, `46` tests run.
- `rg -n 'NaiveDate::from_ymd_opt\(1970, 1, 1\)\.unwrap\(\)|and_hms_opt\(0, 0, 0\)\.unwrap\(' crates/merman-core/src/diagrams/gantt/datetime.rs` -
  no matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `878`
  lines parsed.
- `git diff --check` - passed.

## Boundary

This is a local Gantt datetime panic-surface cleanup. It does not change date parsing, duration
parsing, missing-year today behavior, task ordering, weekend/exclude handling, retained config
projection, rendered layout, SVG baselines, root viewport formulas, or Mermaid parity residual
classification.
