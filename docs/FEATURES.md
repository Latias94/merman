# Feature Surfaces

Cargo features select compiled products and explicit system adapters. They do not select a Mermaid
language subset. Every `merman-core` build has the complete pinned Mermaid language catalog:
configuration and frontmatter handling, sanitization, detection, canonical semantic parsing,
source spans, family headers, and editor vocabulary. There is no `full`, `tiny`,
`core-full-*`, or runtime registry-profile feature.

Use `diagram_family_capabilities()` when an integration needs the complete catalog and
`binding_capabilities()` when it needs to know which optional product APIs are present in the
loaded artifact. A diagram may parse in every build while a render backend for its chosen layout is
not compiled.

The capability vocabulary and target constraints are recorded in
[`capabilities/feature-surface-v1.json`](../capabilities/feature-surface-v1.json). Cargo manifests
remain the source of exact per-crate forwarding edges. During this migration, the vocabulary is not
release or artifact-build authority: U13 introduces the observed artifact-profile and transport
contracts before any distributable surface can claim an exact closure.

## System Adapters

| Crate | Feature | Default | Meaning |
| --- | --- | ---: | --- |
| `merman-core` | `system-clock` | no | Allows a native operation policy to capture the wall clock. |
| `merman-core` | `system-timezone` | no | Allows fallible discovery of the complete system time zone and its transition rules. |
| `merman-core` | `system-random` | no | Allows a native operation policy to obtain an operating-system seed. |
| `merman-core` | `system-timing` | no | Allows explicitly requested operation timing diagnostics. |
| Facade and native transport crates | matching `system-*` feature | no at low-level surfaces | Forwards only the named adapter. |

`merman-core` has no default features. Disabling system adapters does **not** remove any diagram
family, YAML/JSON5 configuration, or sanitizer behavior. Adapters are captured into one operation
context before work begins; parsers, analyzers, layout engines, and renderers do not read ambient
process state themselves.

## Product Features

| Product family | Features | Meaning |
| --- | --- | --- |
| `merman` | `render`, `ascii` | Typed layout/SVG and terminal-oriented output. |
| `merman` | `cytoscape-layout`, `elk-layout` | Optional named layout engines. They imply `render`; use them when the selected diagram/layout requires that engine. |
| `merman` | `ratex-math`, `raster` | Optional math rendering and PNG/JPEG/PDF conversion. `raster` is an umbrella for all current bitmap/PDF backends. |
| `merman-analysis` | `system-clock`, `system-timezone`, `system-random`, `system-timing` | Optional system adapters only; analysis semantics are always complete when this crate is present. |
| `merman-editor-core` | matching `system-*` features | Optional system adapters only; it consumes the complete parser-backed facts catalog. |
| `merman-lsp` | `stdio`, matching `system-*` features | `stdio` builds the bundled binary; the library remains protocol-neutral without it. |
| `merman-bindings-core` | `analysis`, `render`, `ascii` | Shared native binding entry points. `editor-language` implies `analysis`. |
| `merman-bindings-core` | `cytoscape-layout`, `elk-layout`, `ratex-math`, `raster` | Forwards optional render backends to native bindings. |
| `merman-ffi` / `merman-uniffi` | `analysis`, `render`, `ascii`, `cytoscape-layout`, `elk-layout`, `ratex-math`, `raster` | Native transport products. Public symbols remain shape-stable and report an unsupported capability when the relevant product is absent. |
| `merman-wasm` | `analysis`, `render`, `ascii`, `editor-language` | Browser wasm-bindgen products. `editor-language` implies `analysis`. |
| `merman-wasm` | `cytoscape-layout`, `elk-layout`, `ratex-math` | Browser render backend choices. |
| `merman-typst-plugin` | `render`, `analysis`, `cytoscape-layout`, `elk-layout` | Typst product leaves. The crate has no defaults and exposes no system adapters. Its publish recipe is defined by the artifact profile. |

The `cytoscape-layout` and `elk-layout` features name user-observable Mermaid layout behavior, not
implementation crates. Do not infer family availability from them: semantic recognition is always
available, while a missing optional backend currently fails through the render boundary. U4 makes
that failure a stable typed missing-capability result.

## Recommended Selections

```toml
# Typical native SVG application.
merman = { version = "0.8.0-alpha.3", features = ["render"] }

# Deterministic parser/analysis host without system clock, time-zone, random, or timing adapters.
merman-analysis = { version = "0.8.0-alpha.3", default-features = false }

# Protocol-neutral LSP library; add `stdio` only for the bundled executable.
merman-lsp = { version = "0.8.0-alpha.3", default-features = false }

# Render with an optional layout engine.
merman = { version = "0.8.0-alpha.3", features = ["render", "elk-layout"] }
```

WASM and Typst packaging should use their checked-in surface descriptors rather than hand-assembling
an accidental feature closure:

- [`platforms/web/web-surface-descriptor.json`](../platforms/web/web-surface-descriptor.json)
- [`crates/merman-typst-plugin/wasm-profiles.json`](../crates/merman-typst-plugin/wasm-profiles.json)

## Wire Contracts

Cargo features, runtime resource profiles, and wire versions are separate contracts. Diagnostics
remain schema `1`; parser facts are schema `2` and reject facts v1 before decoding nested fields.
Runtime-contract schema `4` is independently versioned from the native ABI, which remains `2`
until U6 replaces it atomically. None of those versions selects a Mermaid language catalog.

Runtime resource profiles bound parsing, layout, SVG construction, and output allocation inside
the compiled product. General bindings default to `interactive`, the CLI defaults to
`trusted-native`, and the Typst adapter enforces `constrained`. Query the versioned runtime contract
from the loaded binding instead of copying limits into a host application.

## Rules For New Features

- Add a public feature only for a user-observable API, output, engine, adapter, or compiled tool.
- Do not create one Cargo feature per diagram family. Family grammar and semantic admission are one
  language contract; expensive optional engines are valid boundaries when users can choose them.
- Keep features positive and composable. Do not introduce negative variants such as `no-elk`.
- Add typed unsupported behavior, additive preset coverage, an observed omission profile, and
  measured closure evidence before exposing a new optional product.
- Do not hide system access behind parser or renderer names. Express it through the independent
  adapter features above and capture it at the operation boundary.
