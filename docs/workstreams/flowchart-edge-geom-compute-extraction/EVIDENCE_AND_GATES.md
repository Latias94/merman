# Flowchart Edge Geometry Compute Extraction - Evidence And Gates

Status: Complete
Last updated: 2026-05-28

## Gate Set

```bash
cargo nextest run -p merman-render flowchart
cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --text-measurer vendored
cargo fmt -p merman-render -- --check
cargo nextest run -p merman-render
cargo clippy -p merman-render --all-targets -- -D warnings
git diff --check
```

## Evidence Anchors

- `crates/merman-render/src/svg/parity/flowchart/edge_geom/compute.rs`
- `crates/merman-render/src/svg/parity/flowchart/edge_geom.rs`
- `crates/merman-render/src/svg/parity/flowchart/mod.rs`
- `docs/rendering/REFACTOR_TODO.md`

## Fresh Evidence

2026-05-28 on the local machine listed below:

- `cargo nextest run -p merman-render flowchart` - PASS, 69 tests run, 69 passed, 139 skipped.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --text-measurer vendored` - PASS.
- `cargo fmt -p merman-render -- --check` - PASS.
- `cargo nextest run -p merman-render` - PASS, 208 tests run, 208 passed, 0 skipped.
- `cargo clippy -p merman-render --all-targets -- -D warnings` - PASS.
- `git diff --check` - PASS.

## Local Machine

- OS: Microsoft Windows 11 Pro, version 10.0.26200, build 26200.
- CPU: 13th Gen Intel(R) Core(TM) i9-13900KF, 24 cores, 32 logical processors, max clock 3000 MHz.
- Memory: 66797320 KiB total visible memory.
- Rust: `rustc 1.87.0`, `cargo 1.87.0`.
- nextest: `cargo-nextest 0.9.116`, host `x86_64-pc-windows-msvc`.

## Notes

- This lane is not a performance benchmark. Historical performance data may have been produced on
  a different machine; compare benchmark-style data only after rerunning on the same machine.
- Concurrent user changes in `CHANGELOG.md`, `crates/merman-render/src/svg/parity/fallback.rs`,
  `docs/adr/0063-extensible-svg-output-pipeline.md`, and
  `docs/workstreams/resvg-safe-svg-output/` are intentionally excluded from this lane.
