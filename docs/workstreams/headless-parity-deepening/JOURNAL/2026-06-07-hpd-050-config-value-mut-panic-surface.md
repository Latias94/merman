# HPD-050 - Config Value Mutation Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

The shared config hardening slice replaced recursive `serde_json::Value` clone/drop/merge behavior
with explicit heap-backed helpers. `MermaidConfig::value_mut(...)` still ended with an internal
`unreachable!` after manually making the `Arc<Value>` unique.

That invariant should hold in normal control flow, but this is the shared public config boundary
used by host `site_config`, frontmatter config, and init directives. It should not expose a panic
if the uniqueness assumption is ever invalidated by future maintenance.

## Changes

- Removed the `unreachable!("MermaidConfig Arc was made unique before mutable access")` branch.
- Kept the existing non-recursive clone-on-write behavior when the config has shared strong or
  weak references.
- Returned the mutable value through `Arc::make_mut(...)` only after the explicit non-recursive
  clone-on-write guard has made recursive cloning unnecessary.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core clone_on_write_handles_deep_config_with_small_stack site_config_deep_merge_handles_deep_public_config_with_small_stack init_directive_config_sanitizes_deep_values_with_small_stack frontmatter_config_deep_merge_handles_deep_values_with_small_stack` -
  passed, `4` tests run.
- `rg -n 'MermaidConfig Arc was made unique|unreachable!|panic!|expect\(|unwrap\(' crates/merman-core/src/config/mod.rs` -
  reports only `#[cfg(test)]` small-stack thread spawn/join `expect(...)` calls.
- `git diff --check` - passed.

## Boundary

This is a shared config panic-surface cleanup only. It does not change config merge semantics,
frontmatter or init directive parsing, legacy font-family mirroring, retained config projection,
theme derivation, parser behavior, SVG output, root viewport formulas, or Mermaid parity residual
classification.
