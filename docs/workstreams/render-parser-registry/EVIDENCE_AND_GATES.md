# Render Parser Registry - Evidence And Gates

Status: Complete
Last updated: 2026-05-28

## Gate Set

```bash
cargo nextest run -p merman-core render_parser_registry
cargo fmt -p merman-core -p merman-render -- --check
cargo nextest run -p merman-core
cargo nextest run -p merman-render
cargo clippy -p merman-core --all-targets -- -D warnings
cargo clippy -p merman-render --all-targets -- -D warnings
```

## Evidence Anchors

- `crates/merman-core/src/diagram/mod.rs`
- `crates/merman-core/src/lib.rs`
- `crates/merman-core/src/tests/misc.rs`

## Fresh Evidence

Recorded on 2026-05-28 in the local development workspace.

```text
cargo nextest run -p merman-core render_parser_registry
Result: pass, 2 tests run, 2 passed, 523 skipped

cargo fmt -p merman-core -p merman-render -- --check
Result: pass

cargo nextest run -p merman-core
Result: pass, 525 tests run, 525 passed, 0 skipped

cargo nextest run -p merman-render
Result: pass, 205 tests run, 205 passed, 0 skipped

cargo clippy -p merman-core --all-targets -- -D warnings
Result: pass

cargo clippy -p merman-render --all-targets -- -D warnings
Result: pass
```

## Notes

- `RenderDiagramRegistry` now owns typed render parser lookup.
- `Engine::parse_render_semantic_model` falls back to `DiagramRegistry` JSON parsing when a typed
  parser is not registered.
- Runtime date/time behavior is deliberately unchanged in this lane.
- Macro/table generation is deferred until registry boilerplate becomes the next clear bottleneck.
