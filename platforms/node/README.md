# Merman Node/SSG transport evaluation

This directory is the private U14 comparison workspace. It does not declare an admitted public
Node product. A release surface may be added only after the same corpus, options, resource profile,
target, installation, semantic-model, SVG-structure, error, queue, lifecycle, timing, RSS, and
footprint gates pass. Exact geometry drift is reported independently.

The current local evaluation is inconclusive because it covers only one target and its candidate
artifacts predate the final refactor state. The macOS arm64 smoke does prove that installing only
the root package resolves and loads its target package through `optionalDependencies`; the other
declared targets still require the same evidence. See
[`docs/performance/NODE_TRANSPORT_ADMISSION.md`](../../docs/performance/NODE_TRANSPORT_ADMISSION.md)
for the reproducible evidence and the explicit non-admission decision.

The candidates are deliberately separate:

- `node-wasm` is built from `crates/merman-node` with `wasm-pack --target nodejs`. It never loads
  `@mermanjs/web` or an artifact from `platforms/web`.
- `napi` is built from the same crate and shared operation bridge with napi-rs. Each native package
  owns one target-specific `merman.node` file.

Both candidates resolve the exact `rust-static-svg` artifact recipe and append only their private
transport leaf. The build receipt records the resolved recipe digest, and the comparison rejects
artifacts built from different capability inputs. They construct
`merman_bindings_core::BindingEngine`, execute `BindingOperationRequest`, default to the
deterministic runtime policy, and preserve typed missing-capability and unknown-operation errors.

Install the pinned candidate tooling and run static contracts:

```console
npm ci
npm test
```

Build one candidate only when the repository Cargo build window is available:

```console
npm run build:candidate -- --candidate node-wasm
npm run build:candidate -- --candidate napi --target darwin-arm64
```

Run the comparison only after both artifacts exist. The harness launches isolated child processes
for cold samples, records repeated warm samples and RSS, rejects a candidate pair with semantic
JSON, typed-error, or SVG-structure differences, reports exact geometry drift without hiding it,
and refuses to select a winner when target runtime evidence is incomplete.

```console
npm run benchmark -- --native artifacts/napi/darwin-arm64/merman.node \
  --wasm artifacts/node-wasm/merman_node.js
```

`AbortSignal` removes work that is still queued. It does not interrupt Rust work that has already
started. Call `dispose()` to reject pending work and wait for executing work to finish. The
synchronous `renderSvgSync()` method is reserved for explicit static-site generation paths.
