# State Node Renderer Extraction - Evidence And Gates

Status: Complete
Last updated: 2026-05-28

## Gate Set

```bash
cargo nextest run -p merman-render state
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo fmt -p merman-render -- --check
cargo nextest run -p merman-render
cargo clippy -p merman-render --all-targets -- -D warnings
```

## Evidence Anchors

- `crates/merman-render/src/svg/parity/state/node.rs`
- `crates/merman-render/src/svg/parity/state/render.rs`
- `crates/merman-render/src/svg/parity/state/mod.rs`
- `docs/rendering/REFACTOR_TODO.md`

## Fresh Evidence

Recorded on 2026-05-28 in the local development workspace.

```text
cargo nextest run -p merman-render state
Result: pass, 13 tests run, 13 passed, 194 skipped

cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3
Result: pass

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

- This lane is not a performance benchmark. Historical performance data may have been produced on
  a different machine; compare any benchmark-style data only after rerunning on the same machine.
- `state/render.rs` now owns root traversal and cluster emission; `state/node.rs` owns leaf-node
  SVG rendering.
