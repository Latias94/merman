# Workspace Release Versioning — Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

```bash
cargo package -p merman-bindings-core --allow-dirty
cargo package -p merman-ffi --allow-dirty
```

## Gate Set

### Version Gate

```bash
cargo check -p merman-ffi
cargo check -p merman-uniffi
```

This proves binding crates still resolve after any version alignment.

### Package File-List Gate

```bash
cargo package -p merman-render --allow-dirty --list
cargo package -p merman-bindings-core --allow-dirty --list
cargo package -p merman-ffi --allow-dirty --list
cargo package -p merman-uniffi --allow-dirty --list
```

This proves publish packages contain the expected files even when full verification is blocked by
unpublished workspace dependencies.

### Full Package Gate

Run full `cargo package` in publish order where crates.io dependency availability allows it.

### Formatting And Diff Gate

```bash
cargo fmt --check
git diff --check
```

Use narrower formatting checks if this lane only edits release docs and manifests.

## Evidence Anchors

- `Cargo.toml`
- `crates/merman-render/Cargo.toml`
- `crates/merman-bindings-core/Cargo.toml`
- `crates/merman-ffi/Cargo.toml`
- `crates/merman-uniffi/Cargo.toml`
- `docs/workstreams/ffi-release-hardening/HANDOFF.md`
- `docs/workstreams/uniffi-bindings/HANDOFF.md`

## Evidence Log

- 2026-05-30: `cargo nextest run -p merman-ffi` passed (`14` tests).
- 2026-05-30: `cargo package -p merman-ffi --allow-dirty --list` passed and listed
  `README.md`, `include/merman.h`, `src/lib.rs`, and FFI smoke tests.
- 2026-05-30: `cargo package -p merman-ffi --allow-dirty` failed because
  `merman-bindings-core` is not available from crates.io.
- 2026-05-30: `cargo package -p merman-bindings-core --allow-dirty` failed because published
  `merman-render 0.6.0` lacks the local `ratex-math` feature required by
  `merman-bindings-core`.
- 2026-05-30: `cargo package -p merman-render --allow-dirty --list` passed.
- 2026-05-30: `cargo package -p merman-render --allow-dirty` passed.
- 2026-05-30: `cargo package -p merman-bindings-core --allow-dirty --list` passed.
