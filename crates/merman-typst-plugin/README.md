# merman-typst-plugin

`merman-typst-plugin` is the Typst WebAssembly transport for `merman`. It uses
`wasm-minimal-protocol` and delegates rendering and analysis to
`merman-bindings-core`; it does not own a second Mermaid parser or renderer.

## Typst Plugin ABI 2

The current Typst plugin ABI version is `2`. It is independent from native ABI 3 and from the
shared binding options schema. The WebAssembly module has a closed host surface that distinguishes
callable ABI operations from linker metadata.

It imports exactly these two protocol functions:

- `typst_env::wasm_minimal_protocol_write_args_to_buffer`
- `typst_env::wasm_minimal_protocol_send_result_to_host`

Its callable ABI consists of exactly these five protocol functions:

- `abi_version() -> bytes`
- `package_version() -> bytes`
- `capabilities_json() -> bytes`
- `render_svg_json(source: bytes, options_json: bytes) -> bytes`
- `analyze_json(source: bytes, options_json: bytes) -> bytes`

The module also exports exactly three non-callable support values:

- `memory`
- `__data_end`, an immutable `i32` global emitted by Rust's WebAssembly linker
- `__heap_base`, an immutable `i32` global emitted by Rust's WebAssembly linker

The linker globals are transport metadata, not plugin operations. No other
function, memory, table, or global export is allowed.

`wasm-profiles.json` owns the ABI number. Run
`cargo run -p xtask -- gen-typst-profile-constants` after changing it; the generated Rust
projection contains both the numeric `TYPST_PLUGIN_ABI_VERSION` constant and the ASCII bytes
returned by `abi_version`. `verify-typst-profile-constants` and the aggregate `verify-generated`
gate reject drift. `package_version` returns the Rust workspace package version.

`capabilities_json` reports the flat runtime catalog schema `1` compiled from the canonical
`typst-wasm` artifact recipe. It contains only current artifact facts: transport and package
identity, capability/output/operation IDs, registry size, text measurement, and resource
descriptors. It does not copy the global capability vocabulary or independently versioned options
and result schemas. `render_svg_json` and `analyze_json` return result
envelope schema 1 with `version`, `operation`, `ok`, `code`, `code_name`, `kind`,
`capability_id`, `message`, and `data`. A successful render stores the SVG in
`data.svg`; a failed operation keeps its machine-readable error kind and optional
capability ID. Successful analysis stores canonical analysis schema 1 in
`data.analysis`. ABI 1's legacy `validate_json` projection is not exported.

Changing an imported or exported function, its WebAssembly signature, or one of
these byte payload contracts requires a Typst plugin ABI change. Changes to the
Typst wrapper API under `packages/typst/merman/src/` do not require an ABI bump
when this transport remains unchanged.

## Profiles

`wasm-profiles.json` owns the plugin ABI number and points package tooling to the
sole public `publish` profile and canonical `typst-wasm` artifact. Cargo features,
compiled capabilities, target, and expected outputs are owned by the exact recipe
in `capabilities/artifact-profiles-v1.json`. Cargo defaults stay empty.

| Package profile | Capabilities |
| --- | --- |
| `publish` | Render, canonical analysis, and the Cytoscape and ELK layout backends |

There are no bridge-only or render-only package profiles. Maintainers can build
direct Cargo feature leaves for local closure experiments, but those combinations
are not named product identities, publication choices, or release evidence.
Mermaid configuration, sanitization, detection, and semantic parsing are invariant
core behavior; a missing layout backend produces a typed capability error only
when a diagram requires it.

RaTeX math is not supported by the Typst plugin. Its current dependency closure
uses browser system-font discovery, which violates the zero-browser-import
Typst boundary. A future math admission must pass separate import and behavior
gates before it updates the one canonical artifact recipe.

The exact Typst dependency gate admits `json5`, `lol_html`, and `url` as measured
pure-Rust parts of invariant Mermaid language, configuration, and sanitization
semantics. It continues to reject browser bindings and system adapters. The
artifact size budget and final module import check bound this admitted closure.

The Typst wrapper version in `packages/typst/merman/typst.toml` is independent
from the Rust workspace version returned by `package_version`.

## Verification

Build and install the provenance-bound publish artifact with:

```bash
cargo run --locked -p xtask -- build-typst-package --profile publish
```

Check the dependency closure using the package, target, default-feature policy,
and features owned by the exact `typst-wasm` artifact recipe:

```bash
cargo run --locked -p xtask -- profile-budget check-deps --profile typst-wasm --artifact-profile typst-wasm
```

Check the canonical artifact's compressed size budget without referring to its
private output directory:

```bash
cargo run --locked -p xtask -- wasm-size-matrix --surface typst --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

`build-typst-package` verifies the artifact manifest, closed import/export and
function-signature surface, runtime capability catalog, and all five ABI
operations through a Typst-compatible `wasmi` host before staging the package.
Raw Cargo output and `target/typst-wasm-artifacts/` are private implementation
details and must not be referenced by CI, release automation, or package users.

Compile the wrapper examples and tests against the exact staged bundle with:

```bash
cargo run --locked -p xtask -- typst-package-smoke --profile publish --skip-wasm-build
```

The installed bundle carries a schema-2 `merman_typst_plugin.manifest.json`,
which proves the canonical artifact recipe, `default_features` policy, exact Cargo
feature set, exact `typst-wasm` artifact profile, production input closure,
toolchain, flags, versions, and artifact digest. The schema-1
`merman_package.manifest.json` additionally binds the
frozen Typst wrapper tree and licenses to that artifact. The package transaction
rejects source drift and any extra, missing, or changed staged file before
replacing an existing version.
