# HPD-050 - Gantt Date/Duration Regex Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

Gantt still compiled five fixed regex helpers on public parse/date paths:

```rust
Regex::new(r"^\d+$")
Regex::new(r"(?i)^after\s+(?<ids>[\d\w -]+)")
Regex::new(r"(?i)^until\s+(?<ids>[\d\w -]+)")
Regex::new(r"^(\d+(?:\.\d+)?)([Mdhmswy]|ms)$")
Regex::new(r"^\d{4}-\d{2}-\d{2}$")
```

Pinned Mermaid 11.15.0 defines the corresponding boundaries in
`repo-ref/mermaid/packages/mermaid/src/diagrams/gantt/ganttDb.js`:

```js
/^\d+$/
/^after\s+(?<ids>[\d\w- ]+)/
/^until\s+(?<ids>[\d\w- ]+)/
/^(\d+(?:\.\d+)?)([Mdhmswy]|ms)$/
```

The upstream `after` / `until` patterns are case-sensitive in the pinned source, so this slice
intentionally removes the previous local `(?i)` behavior.

## Changes

- Removed `regex::Regex`, `OnceLock`, and the Gantt-local `*_RE` statics from
  `crates/merman-core/src/diagrams/gantt/mod.rs`.
- Replaced pure digit checks with an ASCII byte scanner, matching JavaScript `\d`.
- Replaced `after` / `until` regex matching with a scanner for source-shaped keyword whitespace
  and ASCII word / hyphen / space ID capture.
- Preserved Mermaid's non-anchored trailing behavior for relative refs: the scanner captures the
  maximal source-allowed ID span and ignores any later disallowed suffix just as `RegExp.exec`
  would.
- Replaced duration parsing with a scanner for required integer digits, optional fractional digits,
  and the source-supported units `M`, `d`, `h`, `m`, `s`, `w`, `y`, and `ms`.
- Replaced strict `YYYY-MM-DD` regex matching with a byte-shape check followed by the existing
  `NaiveDate::parse_from_str(...)` calendar validation.
- Added focused Gantt tests for duration invalid forms, `_` / `-` relative IDs, source-regex
  whitespace backtracking, and case-sensitive `after` / `until` keyword behavior.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core gantt` - passed, `45` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'regex::Regex|Regex::new|OnceLock<Regex>|OnceLock\s*<\s*Regex' crates/merman-core/src -g '*.rs'` -
  no production core regex compile/cache matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

This is a source-backed Gantt parser/date panic-surface cleanup and completes the currently known
production `merman-core/src` regex compilation cleanup. It does not change Gantt layout, renderer
SVG output, task ordering, weekend/exclude semantics, sanitizer policy, retained config
projection, SVG baselines, root viewport formulas, or Architecture residual classification.
