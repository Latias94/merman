# HPD-050 - FontAwesome Icon Regex Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`replace_fontawesome_icons(...)` compiled a static regex on the render text-label path:

```rust
Regex::new(r"(fa[bklrs]?):fa-([A-Za-z0-9_-]+)")
```

Pinned Mermaid 11.15.0 defines the corresponding replacement source in
`repo-ref/mermaid/packages/mermaid/src/rendering-util/createText.ts`:

```js
/(fa[bklrs]?):fa-([\w-]+)/g
```

Mermaid's registered-icon path is intentionally outside this local helper; `merman-render` keeps
the existing `<i class="...">` fallback shape used by the local SVG baselines.

## Changes

- Removed `regex::Regex` and `OnceLock` from `crates/merman-render/src/text/icons.rs`.
- Replaced global regex replacement with a direct scanner for:
  - `fa` plus optional `b`, `k`, `l`, `r`, or `s`;
  - literal `:fa-`;
  - one or more ASCII word / hyphen icon-name bytes.
- Preserved non-anchored global matching behavior, including matches that start inside a larger
  surrounding string.
- Added focused text tests using Mermaid's upstream examples for `fa`, `fab`, `fak`, and `fas`,
  plus non-match coverage for unsupported prefixes, empty icon names, and non-ASCII icon names.

## Verification

- `cargo +1.95 fmt -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render fontawesome` - passed, `7` tests run.
- `rg -n 'Regex|regex::|OnceLock|fontawesome_icon_at|replace_fontawesome_icons' crates/merman-render/src/text/icons.rs crates/merman-render/src/text/tests.rs` -
  no regex dependency matches in `text/icons.rs`; scanner and tests were the only relevant hits.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `843`
  lines parsed.

## Boundary

This is a source-backed render text-label panic-surface cleanup. It does not change registered icon
pack resolution, flowchart icon-shape nodes, HTML label measurement heuristics, SVG baselines,
root viewport formulas, core parsing, sanitizer policy, or Architecture residual classification.
