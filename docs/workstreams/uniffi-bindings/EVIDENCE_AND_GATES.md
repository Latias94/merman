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
