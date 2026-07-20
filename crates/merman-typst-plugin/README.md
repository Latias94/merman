# merman-typst-plugin

`merman-typst-plugin` is the Typst WebAssembly transport for `merman`. It uses
`wasm-minimal-protocol` and delegates rendering and analysis to
`merman-bindings-core`; it does not own a second Mermaid parser or renderer.

## ABI 2

The current Typst plugin ABI version is `2`. The WebAssembly module has a closed
host surface that distinguishes callable ABI operations from linker metadata.

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

`wasm-profiles.json` owns the ABI number. The crate build generates both the
numeric `TYPST_PLUGIN_ABI_VERSION` constant and the ASCII bytes returned by
`abi_version` from that field. `package_version` returns the Rust workspace
package version.

`capabilities_json` must exactly match the selected entry in
`wasm-profiles.json`. `render_svg_json` returns render payload schema 1 with only
`version`, `ok`, `code`, `code_name`, `message`, and `svg`. `analyze_json`
returns the canonical analysis schema 1 used by the Rust analysis and editor
surfaces; ABI 1's legacy `validate_json` projection is not exported.

Changing an imported or exported function, its WebAssembly signature, or one of
these byte payload contracts requires a Typst plugin ABI change. Changes to the
Typst wrapper API under `packages/typst/merman/src/` do not require an ABI bump
when this transport remains unchanged.

## Profiles

`wasm-profiles.json` is the source of truth for features and capabilities. The
package tooling defaults to the `publish` profile and accepts only these public
profile aliases:

| Alias | Capabilities |
| --- | --- |
| `publish` | Render, canonical analysis, full Mermaid config, and ELK layout |
| `full-no-elk` | Render, canonical analysis, and full Mermaid config without ELK |
| `minimal` | Render and canonical analysis without full config or ELK |

Bridge-only and render-only entries are internal size-measurement profiles, not
package publication choices.

RaTeX math is not supported by the Typst plugin. Its current dependency closure
uses browser system-font discovery, which violates the zero-browser-import
Typst boundary. A future math profile must pass a separate import and behavior
admission before it can be exposed.

The Typst wrapper version in `packages/typst/merman/typst.toml` is independent
from the Rust workspace version returned by `package_version`.

## Verification

Build and install the provenance-bound publish artifact with:

```bash
cargo run --locked -p xtask -- build-typst-package --profile publish
```

Check its closed import, export, and function-signature surface with the shared
Wasmi module validator:

```bash
cargo run --locked -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm target/typst-wasm-artifacts/typst-full-elk/merman_typst_plugin.wasm
```

Invoke and validate all five ABI operations through a Typst-compatible `wasmi`
host:

```bash
cargo run --locked -p xtask -- typst-plugin-smoke --profile publish --wasm target/typst-wasm-artifacts/typst-full-elk/merman_typst_plugin.wasm
```

The smoke command defaults to `--profile publish`. Use `minimal` or
`full-no-elk` only when the artifact was built from that exact descriptor
profile. Raw Cargo output under `target/wasm-build/` is private build input and
must not be packaged or used as release evidence.

Compile the wrapper examples and tests against the exact staged bundle with:

```bash
cargo run --locked -p xtask -- typst-package-smoke --profile publish --skip-wasm-build
```

The installed bundle carries two schema-1 manifests. `merman_typst_plugin.manifest.json` proves the
canonical WASM profile, production Cargo input closure, toolchain, flags, versions, and artifact
digest. `merman_package.manifest.json` additionally binds the frozen Typst wrapper tree and licenses
to that artifact. The package transaction rejects source drift and any extra, missing, or changed
staged file before replacing an existing version.
