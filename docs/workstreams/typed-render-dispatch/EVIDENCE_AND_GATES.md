# Typed Render Dispatch - Evidence And Gates

Status: Complete
Last updated: 2026-05-28

## Gate Set

### Core Metadata Gate

```bash
cargo nextest run -p merman-core render_semantic_model
```

Proves model-owned kind and alias metadata behavior.

### Renderer Dispatch Gate

```bash
cargo nextest run -p merman-render render_model
cargo nextest run -p merman-render
```

Proves renderer typed layout dispatch still works and package behavior is stable.

### Broader Gate

```bash
cargo fmt -p merman-core -p merman-render -- --check
cargo clippy -p merman-core --all-targets -- -D warnings
cargo clippy -p merman-render --all-targets -- -D warnings
```

## Evidence Anchors

- `crates/merman-core/src/diagram/mod.rs`
- `crates/merman-core/src/lib.rs`
- `crates/merman-render/src/lib.rs`
- `crates/merman-core/src/tests/misc.rs`

## Fresh Evidence

Recorded on 2026-05-28 in the local development workspace.

### Focused Core Gate

```text
cargo nextest run -p merman-core render_semantic_model
Result: pass, 2 tests run, 2 passed, 521 skipped
```

### Focused Renderer Gate

The first run of `cargo nextest run -p merman-render render_model` returned no tests. This lane
added `merman-render tests::render_model_dispatch_*` so the gate now covers alias dispatch and
mismatched typed-model rejection.

```text
cargo nextest run -p merman-render render_model
Result: pass, 2 tests run, 2 passed, 203 skipped
```

### Package Gates

```text
cargo fmt -p merman-core -p merman-render -- --check
Result: pass

cargo nextest run -p merman-core
Result: pass, 523 tests run, 523 passed, 0 skipped

cargo nextest run -p merman-render
Result: pass, 205 tests run, 205 passed, 0 skipped

cargo clippy -p merman-core --all-targets -- -D warnings
Result: pass

cargo clippy -p merman-render --all-targets -- -D warnings
Result: pass
```

## Notes

- `RenderSemanticModel::kind()` now owns canonical kind names used by parse timing logs.
- `RenderSemanticModel::supports_diagram_type()` now owns alias compatibility.
- `layout_parsed_render_layout_only` validates typed model compatibility once, then matches on
  variants only.
- JSON fallback remains diagram-type based for unsupported/plugin-style diagrams.
