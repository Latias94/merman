# Architecture Indexed FCoSE - Evidence And Gates

Status: Complete
Last updated: 2026-05-28

## Baseline Snapshot

Architecture is the highest-value first target because the current performance docs show the largest
standard-canary gap in the layout stage:

- `architecture_medium` layout ratio: about `9.44x` slower than mmdr.
- `architecture_medium` end-to-end ratio: about `4.23x` slower than mmdr.
- Current override inventory from `cargo run -p xtask -- report-overrides`:
  - Root viewport overrides: `286`
  - Text metric lookup overrides: `490`
  - Hand-curated helpers: `0`
  - Manual raw bridges: `0`

## Smallest Current Repro

The smallest architectural repro is not one fixture; it is the hot path shape:

```bash
cargo nextest run -p manatee
cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

## Gate Set

### Targeted Iteration Gate

```bash
cargo nextest run -p manatee
```

This proves the indexed FCoSE API preserves compatibility behavior at the graph-layout crate
boundary.

### Architecture Integration Gate

```bash
cargo nextest run -p merman-render architecture
cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3
cargo run -p xtask -- report-overrides --check-no-growth
```

This proves Architecture can use indexed FCoSE without semantic, DOM, root viewport, or override
budget regressions.

### Performance Gate

```bash
cargo bench -p merman --features render --bench architecture_layout_stress
cargo bench -p merman --features render --bench pipeline -- architecture_medium
```

This proves whether the refactor moves the actual slow canary instead of only cleaning up code.

### Broader Closeout Gate

```bash
cargo fmt -p manatee -p merman-render -- --check
cargo clippy -p manatee --all-targets -- -D warnings
cargo clippy -p merman-render --all-targets -- -D warnings
cargo nextest run -p manatee
cargo nextest run -p merman-render
```

Use narrower closeout only if workspace-wide gates are too slow, and record the reason here.

### Review Gate

Run `review-workstream` before accepting task or lane completion. Record blocking findings, missing
gates, and residual risks here or link to the review note.

## Evidence Anchors

- `docs/workstreams/architecture-indexed-fcose/DESIGN.md`
- `docs/workstreams/architecture-indexed-fcose/TODO.md`
- `docs/workstreams/architecture-indexed-fcose/MILESTONES.md`
- `crates/manatee/src/algo/fcose/mod.rs`
- `crates/merman-render/src/architecture.rs`

## Fresh Evidence - 2026-05-28

### Formatting

```bash
cargo fmt -p manatee -p merman-render -- --check
```

Result: passed. Proves the modified Rust packages are rustfmt-clean.

### Manatee Indexed FCoSE API

```bash
cargo nextest run -p manatee
```

Result: passed, `11` tests. Proves the new indexed FCoSE API preserves existing compatibility
behavior and does not regress existing manatee tests.

Key regression test:

- `algo::fcose::tests::indexed_layout_matches_string_graph_layout_for_compound_constraints`

### Architecture Integration

```bash
cargo nextest run -p merman-render architecture
cargo nextest run -p merman-render
cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3
cargo run -p xtask -- report-overrides --check-no-growth
```

Results:

- Targeted Architecture filter passed, `6` tests.
- Full `merman-render` package passed, `203` tests.
- Architecture parity-root command exited `0`.
- Override growth check passed; root viewport overrides stayed at `286`, text metric lookup
  overrides stayed at `490`, helper/raw bridge counts stayed at `0`.

This proves Architecture can use indexed FCoSE without test, DOM parity-root, or override budget
regressions.

### Clippy

```bash
cargo clippy -p manatee --all-targets -- -D warnings
cargo clippy -p merman-render --all-targets -- -D warnings
```

Result: both passed. Proves the new public indexed API and Architecture integration satisfy the
current lint gate.

### Performance

Current benchmark machine and toolchain:

- OS: Microsoft Windows 11 Pro, version `10.0.26200`, build `26200`.
- CPU: 13th Gen Intel(R) Core(TM) i9-13900KF, `24` cores / `32` logical processors, max clock
  reported as `3000 MHz`.
- RAM: `68400455680` bytes, about `63.7 GiB`.
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`, LLVM `20.1.1`.
- Cargo: `cargo 1.87.0 (99624be96 2025-05-06)`.
- Nextest: `cargo-nextest 0.9.116`.

```bash
cargo bench -p merman --features render --bench architecture_layout_stress
cargo bench -p merman --features render --bench pipeline -- architecture_medium
```

Results:

- `layout_stress/architecture_reasonable_height_layout_x50`: `[24.026 ms 24.148 ms 24.281 ms]`.
- `parse/architecture_medium`: `[2.5550 us 2.5624 us 2.5702 us]`.
- `parse_known_type/architecture_medium`: `[84.916 us 85.209 us 85.508 us]`.
- `layout/architecture_medium`: `[62.729 us 62.962 us 63.190 us]`.
- `render/architecture_medium`: `[28.984 us 29.095 us 29.217 us]`.
- `end_to_end/architecture_medium`: `[96.225 us 96.962 us 97.785 us]`.

Criterion reported a same-machine improvement versus the previous local benchmark sample for these
bench IDs. Keep that separate from historical docs, which may have used another machine.

Important benchmark caveat: older performance documents may have been produced on a different
machine and toolchain state than the current benchmark machine listed above. Treat the comparison to
`docs/performance/spotcheck_2026-05-14_flowchart_override_inventory_full_bench_gate.md`
(`47.063..49.017 ms` for the same Architecture layout stress benchmark) as directional historical
context, not as a strict same-machine before/after ratio. The authoritative evidence for this lane
is the fresh command output above from the current workspace.

## Notes

Fresh verification is required before marking a task, Codex goal, or lane complete.

Do not claim a performance win without fresh benchmark output from this lane. Existing performance
docs only justify the target selection.

## Review And Verification Closeout - 2026-05-28

Review-workstream result:

- Workstream compliance: no blocking findings. The lane stayed within indexed FCoSE/Architecture
  scope, preserved the string-keyed compatibility API, updated evidence, and split unrelated
  dispatch/text-cache refactors as follow-ons.
- Code quality: no blocking findings. The main residual risk is that the new indexed FCoSE types
  are public under `manatee::algo::fcose`; this is intentional for Architecture and mirrors the
  existing `cose_bilkent::layout_indexed` boundary.
- Missing gates: none for this lane. Workspace-wide nextest was not run because the changed code is
  scoped to `manatee` and `merman-render`; package, parity, clippy, format, and override gates were
  run fresh.

Verify-rust-workstream claim:

- Verified claim: Architecture now uses indexed FCoSE directly, existing FCoSE compatibility remains
  covered, and the lane can close with fresh evidence.
- Fresh evidence above was collected after the final parent-validation guard was added.
