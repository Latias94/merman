# SVG Debug Point-List Consolidation - Evidence And Gates

Status: Complete
Last updated: 2026-05-28

## Gate Set

```bash
cargo nextest run -p merman-render fmt_points
cargo nextest run -p merman-render debug_svg
cargo fmt -p merman-render -- --check
cargo nextest run -p merman-render
cargo clippy -p merman-render --all-targets -- -D warnings
```

## Evidence Anchors

- `crates/merman-render/src/svg/parity.rs`
- `crates/merman-render/src/svg/parity/er.rs`
- `crates/merman-render/src/svg/parity/flowchart/debug_svg.rs`
- `crates/merman-render/src/svg/parity/class/debug_svg.rs`
- `crates/merman-render/src/svg/parity/state/debug_svg.rs`
- `crates/merman-render/src/svg/parity/sequence/debug.rs`

## Fresh Evidence

Recorded on 2026-05-28 in the local development workspace.

```text
cargo nextest run -p merman-render fmt_points
Result: pass, 1 test run, 1 passed, 206 skipped

cargo nextest run -p merman-render debug_svg
Result: pass, 5 tests run, 5 passed, 202 skipped

cargo fmt -p merman-render -- --check
Result: pass

cargo nextest run -p merman-render
Result: pass, 207 tests run, 207 passed, 0 skipped

cargo clippy -p merman-render --all-targets -- -D warnings
Result: pass
```

## Local Machine

- OS: Microsoft Windows 11 Pro 10.0.26200 build 26200
- CPU: 13th Gen Intel(R) Core(TM) i9-13900KF, 24 cores / 32 logical processors, 3000 MHz max clock
- Memory: 66797320 KiB total visible memory, about 63.7 GiB
- Rust: rustc 1.87.0, cargo 1.87.0
- nextest: cargo-nextest 0.9.116, host x86_64-pc-windows-msvc

## Notes

- No performance benchmark was taken in this lane. Historical performance data in repository docs
  may have been produced on a different machine; compare it only directionally unless rerun on the
  machine above.
