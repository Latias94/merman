# Python UniFFI Package - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

```bash
cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
```

Before this lane, that command proved Python source generation but did not import the generated
module or call the Rust engine from Python.

## Gate Set

### Generator Compile Gate

```bash
cargo check -p merman-uniffi --features bindgen-smoke --examples
```

### Python Import/Call Gate

```bash
cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
```

This must generate Python bindings, place the native library beside the generated module, import the
package with Python, and execute at least one SVG render and semantic JSON parse.

### Formatting And Diff Gate

```bash
cargo fmt -p merman-uniffi -- --check
git diff --check
```

Use a narrow formatting gate because ASCII files are explicitly out of scope and currently have
parallel user edits.

## Evidence Anchors

- `crates/merman-uniffi/Cargo.toml`
- `crates/merman-uniffi/tests/bindgen_smoke.rs`
- `bindings/python/merman-uniffi`
- `docs/bindings/UNIFFI.md`
- `docs/bindings/PYTHON_UNIFFI.md`

## Evidence Log

- 2026-05-30: `python --version` reports Python 3.13.11.
- 2026-05-30: PUP-010 opened this lane and selected staged package import/call smoke as the
  release-quality gate.
- 2026-05-30: PUP-020 added `bindings/python/merman-uniffi` with package metadata, shim,
  generated-artifact ignore rules, and `crates/merman-uniffi/examples/generate_python_package.rs`.
- 2026-05-30: `cargo check -p merman-uniffi --features bindgen-smoke --examples` passed.
- 2026-05-30: `cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke`
  passed (`2` tests): source generation and staged Python import/call smoke.
- 2026-05-30: `cargo run -p merman-uniffi --features bindgen-smoke --example generate_python_package
  -- --package-dir <temp>` passed and generated `merman_uniffi.py` plus copied
  `merman_uniffi.dll` into a temporary package directory.
- 2026-05-30: `cargo fmt -p merman-uniffi -- --check` passed.
- 2026-05-30: `cargo clippy -p merman-uniffi --features bindgen-smoke --all-targets --
  -D warnings` passed after removing a needless test borrow.
- 2026-05-30: Final `cargo nextest run -p merman-uniffi --features bindgen-smoke --test
  bindgen_smoke` passed (`2` tests, `0` skipped).
- 2026-05-30: `git diff --check` passed.
- 2026-05-30: `cargo package -p merman-uniffi --allow-dirty --list` passed and includes
  `examples/generate_python_package.rs`.
