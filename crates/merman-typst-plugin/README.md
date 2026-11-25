# merman-typst-plugin

`merman-typst-plugin` is the Typst WebAssembly transport for `merman`. It uses `wasm-minimal-protocol` and delegates rendering, analysis, and Theme v2 operations to `merman-bindings-core`.

> **Implementation transport:** Typst authors should import the [`@preview/merman`](https://typst.app/universe/package/merman) package. Do not load this raw WebAssembly artifact or depend on the Rust crate directly; the Typst wrapper owns package versioning, options, error presentation, and artifact loading.

The current source tree builds the unreleased wrapper `0.7.0` with Merman `0.11.0`. The published wrapper `0.6.0` bundled Merman `0.10.0` and plugin ABI 3. See the [Typst package guide](../../distribution/typst/merman/README.md) for imports, rendering options, and upgrade instructions.

## Typst Plugin ABI 4

The Typst plugin ABI is versioned independently of the native ABI and shared binding schemas. ABI 4 retains ABI 3's import and export signatures and adopts Binding Options JSON 4 and Theme v2 payloads. Use the wrapper and plugin from the same package.

It imports exactly these two protocol functions:

- `typst_env::wasm_minimal_protocol_write_args_to_buffer`
- `typst_env::wasm_minimal_protocol_send_result_to_host`

Its callable ABI consists of exactly these seven protocol functions:

- `abi_version() -> bytes`
- `package_version() -> bytes`
- `capabilities_json() -> bytes`
- `theme_catalog_json() -> bytes`
- `render_svg_json(source: bytes, options_json: bytes) -> bytes`
- `analyze_json(source: bytes, options_json: bytes) -> bytes`
- `theme_operation_json(operation_id: bytes, input: bytes, options_json: bytes) -> bytes`

The module also exports exactly three non-callable support values:

- `memory`
- `__data_end`, an immutable `i32` global emitted by Rust's WebAssembly linker
- `__heap_base`, an immutable `i32` global emitted by Rust's WebAssembly linker

The linker globals are transport metadata, not plugin operations. The crate's build script
exports them explicitly for the WebAssembly cdylib so the surface does not depend on the
compiler's default linker exports. No other function, memory, table, or global export is allowed.

`wasm-profiles.json` owns the ABI number. Run `cargo run -p xtask -- gen-typst-profile-constants` after changing it; the generated Rust projection contains both the numeric `TYPST_PLUGIN_ABI_VERSION` constant and the ASCII bytes returned by `abi_version`. `verify-typst-profile-constants` and the aggregate `verify-generated` gate reject drift. `package_version` returns the Rust workspace package version.

`capabilities_json` reports the flat runtime catalog schema 1 for the `typst-wasm` artifact. `render_svg_json`, `analyze_json`, `theme_operation_json`, and `theme_catalog_json` use result envelope schema 1. A successful render returns `data.svg` and `metadata.theme_outcome`. Analysis returns `data.analysis`, and a theme call returns `data.result`. Transport errors include a machine-readable error kind and an optional capability ID.

`theme_catalog_json` returns shared catalog schema 2 and appears as `theme-catalog` in `metadata_ids`. `theme_operation_json` accepts `export-theme-preset` with raw UTF-8 preset ID bytes and `resolve-theme` with a JSON object containing `document`, optional `appearance`, and optional `auto_fix_contrast`. Pass empty `options_json` bytes for both operations.

For SVG rendering, Typst's effective default combines the `constrained` base profile with the
shared `interactive` layout-work allowance, currently 14,100,000 units. An omitted or null
`resources.profile` inherits this host default; explicit `"constrained"` without a numeric layout
override retains the shared 125,000 units. An explicit finite positive integer in
`resources.limits.max_layout_work_units` overrides either choice. Other ceilings, options byte
admission, and analysis retain the constrained policy. The runtime catalog reports shared profile
capacities. Theme resolution uses the shared Theme v2 compiler budget.

The closed Typst import surface has no synchronous font-measurement service. Its runtime catalog
therefore advertises only the built-in `deterministic` text-measurement provider; Typst font assets
and browser host callbacks are not measured by this transport.

Changing an imported or exported function, its WebAssembly signature, or one of these byte payload contracts requires a Typst plugin ABI change. Changes to the Typst wrapper API under `distribution/typst/merman/src/` do not require an ABI bump when this transport remains unchanged.

## Profiles

`wasm-profiles.json` owns the plugin ABI number and points package tooling to the sole public `publish` profile and canonical `typst-wasm` artifact. Cargo features, compiled capabilities, target, and expected outputs are owned by the exact recipe in `capabilities/artifact-profiles-v2.json`. Cargo defaults stay empty.

| Package profile | Capabilities |
| --- | --- |
| `publish` | Render, canonical analysis, and the Cytoscape and ELK layout backends |

There are no bridge-only or render-only package profiles. Maintainers can build direct Cargo feature leaves for local closure experiments, but those combinations are not named product identities, publication choices, or release evidence. The distributed recipe explicitly selects `all-diagrams`; source builds can select `diagram-*` families separately. Selected families preserve Mermaid configuration, sanitization, and semantic parsing; a missing layout backend produces a typed capability error only when a diagram requires it.

The publish artifact compiles SVG rendering. The closed Typst operation set omits ASCII, PNG,
JPEG, and PDF.

RaTeX math is not supported by the Typst plugin. Its current dependency closure uses browser system-font discovery, which violates the zero-browser-import Typst boundary. A future math admission must pass separate import and behavior gates before it updates the one canonical artifact recipe.

The exact Typst dependency gate admits `json5`, `html5ever`, and `url` as measured pure-Rust parts of invariant Mermaid language, configuration, and sanitization semantics. It continues to reject browser bindings and system adapters. The artifact size budget and final module import check bound this admitted closure.

The Typst wrapper version in `distribution/typst/merman/typst.toml` is independent from the Rust workspace version returned by `package_version`.

## Verification

Build, budget, and install the provenance-bound publish artifact in one production pass:

```bash
cargo run --locked -p xtask -- build-typst-package --profile publish \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

Check the dependency closure using the package, target, default-feature policy, and features owned by the exact `typst-wasm` artifact recipe:

```bash
python3 scripts/verify_artifact_dependency_closures.py --profile typst-wasm
```

The producer builds raw WASM, runs Binaryen 133 `wasm-opt -Oz`, and strips the optimized result
once. Its private schema-4 manifest binds raw, optimized, and final stripped bytes by SHA-256 and
length. The budget consumer verifies those files, measures raw bytes before optimization, and
compresses the final stripped WASM with the unchanged gzip and Brotli recipes. Omitting
`--budget-file` skips only the optional size gate, not fresh production or identity checks.
The retired `wasm-size-matrix --surface typst` and `--artifact-profile typst-wasm` selections
report the replacement package-build command instead of producing another artifact.

`build-typst-package` verifies the artifact manifest, closed import/export and function-signature surface, runtime capability catalog, and all seven ABI operations through a Typst-compatible `wasmi` host before staging the package. Raw Cargo output and `target/typst-wasm-artifacts/` are private implementation details and must not be referenced by CI, release automation, or package users.

Compile the wrapper examples and tests against the exact staged bundle with:

```bash
cargo run --locked -p xtask -- typst-package-smoke --profile publish --skip-wasm-build
```

`--skip-wasm-build` is smoke-only. It still validates current source, features, tools, flags,
versions, and all three artifact digests before packaging the frozen final WASM. A schema-3
artifact must be rebuilt once; its manifest has no raw/optimized evidence for the size gate.
The private raw and optimized files are never installed into the runtime package.

The build consumes a private artifact provenance manifest to prove the canonical artifact recipe,
`default_features` policy, exact Cargo feature set, exact `typst-wasm` artifact profile, production
input closure, toolchain, flags, versions, and artifact digest. It also freezes the Typst wrapper
tree and licenses in memory while staging, then rejects source drift and any extra, missing, or
changed staged file before replacing an existing version. Neither build receipt is copied into the
installed Typst package.
