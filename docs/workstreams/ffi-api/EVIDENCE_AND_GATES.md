# FFI API — Evidence And Gates

Status: Draft
Last updated: 2026-05-30

## Gate Set

### Initial Documentation Gate

```bash
git diff --check -- docs/adr/0066-ffi-binding-strategy.md docs/workstreams/ffi-api
```

This proves the ADR and workstream docs do not contain whitespace errors.

### Targeted Iteration Gate

```bash
cargo fmt -p merman-ffi -- --check
cargo nextest run -p merman-ffi
```

Use after `crates/merman-ffi` exists. This proves the FFI crate builds, formats, and passes its
focused memory/error/API tests.

### Header Smoke Gate

```bash
cargo nextest run -p merman-ffi header_smoke
```

Use after the public C header exists. This proves a C consumer can compile and link against the
exported API.

### Feature Matrix Gate

```bash
cargo nextest run -p merman-ffi --features raster
cargo nextest run -p merman-ffi --features ratex-math
cargo nextest run -p merman-ffi --features raster,ratex-math
```

Use only after those features are exposed through FFI. This prevents optional output modes from
silently breaking.

### Broader Closeout Gate

```bash
cargo fmt --check
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Use before declaring the lane complete, unless workspace runtime is too high. If narrowed, record
the reason and replacement coverage here.

## Evidence Anchors

- `docs/adr/0066-ffi-binding-strategy.md`
- `docs/workstreams/ffi-api/DESIGN.md`
- `docs/workstreams/ffi-api/TODO.md`
- `docs/workstreams/ffi-api/MILESTONES.md`
- future `docs/bindings/FFI_PROTOCOL.md`
- future `crates/merman-ffi` tests

## Notes

Fresh verification is required before marking a task, Codex goal, or lane complete.
