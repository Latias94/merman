# UniFFI Bindings — Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

```bash
cargo check -p merman-bindings-core
cargo nextest run -p merman-ffi
```

## Gate Set

### Facade Gate

```bash
cargo check -p merman-bindings-core
cargo nextest run -p merman-ffi
```

This proves the shared facade compiles and the existing C ABI behavior remains covered.

### UniFFI Gate

```bash
cargo check -p merman-uniffi
cargo test -p merman-uniffi
```

This proves the generated-binding crate compiles and its Rust-side smoke tests pass.

### Binding Generation Gate

Record the exact UniFFI bindgen command after `merman-uniffi` exists. The gate should generate into
a temp directory unless generated source is intentionally committed.

### Feature Gate

```bash
cargo nextest run -p merman-ffi --features ratex-math
cargo check -p merman-uniffi --features ratex-math
```

This proves RaTeX support remains feature-gated across both binding layers.

### Formatting And Lint Gate

```bash
cargo fmt --check
cargo clippy -p merman-bindings-core --all-targets -- -D warnings
cargo clippy -p merman-ffi --all-targets -- -D warnings
cargo clippy -p merman-uniffi --all-targets -- -D warnings
```

Use narrower lint gates before all crates exist and record that limitation.

### Broader Closeout Gate

```bash
cargo nextest run --workspace
```

Use a narrower closeout gate if the workspace is too large, and explain why.

## Evidence Anchors

- `docs/workstreams/uniffi-bindings/DESIGN.md`
- `docs/workstreams/uniffi-bindings/TODO.md`
- `docs/workstreams/uniffi-bindings/MILESTONES.md`
- `docs/adr/0066-ffi-binding-strategy.md`
- `docs/bindings/FFI_PROTOCOL.md`
- future `crates/merman-bindings-core`
- future `crates/merman-uniffi`

## Evidence Log

- 2026-05-30: `cargo search uniffi --limit 3` reported `uniffi = "0.31.1"`.
- 2026-05-30: `cargo info uniffi@0.31.1` confirmed the crate metadata and features including
  `bindgen`, `build`, `cli`, and `scaffolding-ffi-buffer-fns`.
- 2026-05-30: `UBI-010` opened the lane around a shared safe facade plus minimal UniFFI crate.
- 2026-05-30: `UBI-020` added `crates/merman-bindings-core` and refactored `merman-ffi` to
  delegate render SVG, parse JSON, layout JSON, status codes, and JSON error payloads to the safe
  facade.
- 2026-05-30: `cargo check -p merman-bindings-core` passed.
- 2026-05-30: `cargo nextest run -p merman-bindings-core` passed (`10` tests).
- 2026-05-30: `cargo nextest run -p merman-bindings-core --features ratex-math` passed (`10`
  tests).
- 2026-05-30: `cargo nextest run -p merman-ffi` passed (`14` tests).
- 2026-05-30: `cargo nextest run -p merman-ffi --features ratex-math` passed (`14` tests).
- 2026-05-30: `cargo check -p merman-bindings-core --features raster,ratex-math` passed.
- 2026-05-30: `cargo nextest run -p merman-ffi --features raster,ratex-math` passed (`14` tests).
- 2026-05-30: `cargo clippy -p merman-bindings-core --all-targets -- -D warnings` passed.
- 2026-05-30: `cargo clippy -p merman-bindings-core --features ratex-math --all-targets -- -D
  warnings` passed.
- 2026-05-30: `cargo clippy -p merman-ffi --all-targets -- -D warnings` passed.
- 2026-05-30: `cargo clippy -p merman-ffi --features ratex-math --all-targets -- -D warnings`
  passed.
- 2026-05-30: `cargo fmt -p merman-bindings-core -p merman-ffi -- --check` passed.
- 2026-05-30: `git diff --check` passed.
- 2026-05-30: `UBI-030` added `crates/merman-uniffi` with a `MermanEngine` object and structured
  `MermanError` over `merman-bindings-core`.
- 2026-05-30: `cargo check -p merman-uniffi` passed.
- 2026-05-30: `cargo test -p merman-uniffi` passed (`5` unit tests plus doc-tests).
- 2026-05-30: `cargo nextest run -p merman-uniffi` passed (`5` tests).
- 2026-05-30: `cargo check -p merman-uniffi --features ratex-math` passed.
- 2026-05-30: `cargo check -p merman-uniffi --features raster,ratex-math` passed.
- 2026-05-30: `cargo clippy -p merman-uniffi --all-targets -- -D warnings` passed.
- 2026-05-30: `cargo clippy -p merman-uniffi --features ratex-math --all-targets -- -D warnings`
  passed.
- 2026-05-30: `cargo fmt -p merman-uniffi -- --check` passed.
- 2026-05-30: `git diff --check` passed after `UBI-030`.
