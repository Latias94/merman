# FFI Release Hardening — Evidence And Gates

Status: Closed with packaging concern
Last updated: 2026-05-30

## Smallest Current Repro

```bash
cargo nextest run -p merman-ffi c_consumer_smoke
```

## Gate Set

### C Consumer Gate

```bash
cargo nextest run -p merman-ffi c_consumer_smoke
```

This proves a C source can include the public header and exercise exported calls through a compiled
test harness.

### Package Gate

```bash
cargo package -p merman-ffi --allow-dirty
```

This proves the FFI crate has publishable metadata and package contents for the current worktree.

### Package Test Gate

```bash
cargo nextest run -p merman-ffi
```

This proves the full FFI crate test suite still passes after hardening.

### Formatting And Lint Gate

```bash
cargo fmt -p merman-ffi -- --check
cargo clippy -p merman-ffi --all-targets -- -D warnings
```

This proves Rust formatting and lint cleanliness for the package touched by this lane.

### Broader Closeout Gate

```bash
cargo nextest run --workspace
```

Use a narrower closeout gate if the workspace is too large, and explain why.

## Evidence Anchors

- `docs/workstreams/ffi-release-hardening/DESIGN.md`
- `docs/workstreams/ffi-release-hardening/TODO.md`
- `docs/workstreams/ffi-release-hardening/MILESTONES.md`
- `crates/merman-ffi/include/merman.h`
- `crates/merman-ffi/tests`
- `docs/bindings/FFI_PROTOCOL.md`
- `README.md`

## Evidence Log

- 2026-05-30: `FRH-010` opened the lane and froze scope around package validation, real C smoke
  coverage, and documentation entry points.
- 2026-05-30: `cargo nextest run -p merman-ffi c_consumer_smoke` passed (`1` test). This proves a
  compiled C consumer can include `merman.h`, call `render_svg`, `parse_json`, `layout_json`, check
  success/error payloads, and release buffers through `merman_buffer_free`.
- 2026-05-30: `cargo package -p merman-ffi --allow-dirty` failed during dependency resolution
  because crates.io `merman-render 0.6.0` does not expose the local `ratex-math` feature. This is a
  workspace release-order blocker, not a C ABI implementation failure.
- 2026-05-30: `cargo package -p merman-ffi --allow-dirty --list` passed and listed:
  `.cargo_vcs_info.json`, `Cargo.lock`, `Cargo.toml`, `Cargo.toml.orig`, `README.md`,
  `include/merman.h`, `src/lib.rs`, `tests/c_consumer_smoke.c`, `tests/c_consumer_smoke.rs`, and
  `tests/header_smoke.rs`.
- 2026-05-30: `cargo nextest run -p merman-ffi` passed (`14` tests).
- 2026-05-30: `cargo nextest run -p merman-ffi --features ratex-math` passed (`14` tests).
- 2026-05-30: `cargo fmt -p merman-ffi -- --check` passed.
- 2026-05-30: `cargo clippy -p merman-ffi --all-targets -- -D warnings` passed.
- 2026-05-30: `cargo clippy -p merman-ffi --features ratex-math --all-targets -- -D warnings`
  passed.
- 2026-05-30: `git diff --check` passed.
- 2026-05-30: Full workspace `cargo nextest run --workspace` was not run for this narrow closeout;
  the lane touched only `merman-ffi`, binding docs, README entry points, and workstream docs. Re-run
  the full workspace release gate before a repository release tag.
