# Merman Node/SSG private candidate

This directory contains the private Node static-SVG candidate. It is not an admitted or published
Merman product. A release surface may be added only after the same corpus, options, resource
profile, target, installation, semantic-model, SVG-structure, error, queue, lifecycle, timing, RSS,
and footprint gates pass. Exact geometry drift remains separate evidence.

For a supported Node or static-site integration today, invoke `merman-cli` as a child process and exchange Mermaid source and output through files or standard streams. Do not depend on this workspace or either candidate package in an application.

Local contract tests and actual native/WASM build probes validate the current source, but the full
multi-target admission matrix is still incomplete. See
[`docs/performance/NODE_TRANSPORT_ADMISSION.md`](https://github.com/Latias94/merman/blob/main/docs/performance/NODE_TRANSPORT_ADMISSION.md)
for the reproducible comparison evidence and explicit non-admission decision.

The candidates are deliberately separate:

- `node-wasm` is built from `crates/merman-node` with `wasm-pack --target nodejs`. It never loads `@mermanjs/web` or an artifact from `platforms/web`.
- `napi` is built from the same crate and shared operation bridge with napi-rs. Each native package owns one target-specific `merman.node` file.

Both candidates resolve the same private static-SVG recipe and append only their transport leaf. The
recipe is checked against the canonical capability descriptor; it is not a published Merman
artifact profile. Each build receipt binds the current `Cargo.lock`, resolved dependency closure,
exact target and features, complete artifact file set, verifier inputs, transport identity, runtime
catalog, metadata, synchronous and asynchronous operation probes, and post-dispose behavior. Native
and WASM package identity/version checks are compatibility checks performed after module code has
loaded; they are not origin authentication.

The public candidate facade is `MermanEngine`, created asynchronously with `createNodeEngine()`.
It accepts only deterministic runtime policy and text results. Binary operations are rejected until
a future deliberate wire break. The native and WASM transports exchange JSON strings only and
enforce the same pre-parse byte ceilings, structural depth, member/token work, field lengths,
catalog relations, response/error envelope rules, and nested metadata limits. Unknown future
metadata fields are preserved, while known operation, media type, output contract, and deterministic
runtime-policy relations are checked exactly.

The maintained engine floor is Node `>=22.0.0`. `runtimeCatalog` reports only the private static-SVG
surface. `metadataJson(id)`, `svgPlanJson()`, and generic `executeOperation()` consume the shared
bindings-core operation and metadata contracts rather than local handwritten vocabularies.

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

Run the comparison only after both artifacts exist. The harness packs and installs each candidate, then launches its workers exclusively through the installed product's `createNodeEngine()` facade. For napi it installs only the root package and verifies that npm resolves the target package through the declared exact-version optional dependency; both paths reject any installed browser fallback. The harness launches isolated child processes for cold samples, records repeated warm samples and RSS, rejects a candidate pair with semantic JSON, typed-error, or SVG-structure differences, reports exact geometry drift without hiding it, and recomputes timing summaries from the raw samples. It hashes the harness before and after measurement and aborts if an input changes. A target result binds its host mapping, build receipt, installed package manifests, loaded artifact hashes, install manifest and lockfile dependency edge, runtime probes, concurrent outcomes, and raw queue/lifecycle settlements; five unbound passing booleans cannot satisfy the admission matrix. The harness refuses to select a winner while any target evidence is missing.

```console
npm run benchmark -- --native artifacts/napi/darwin-arm64/merman.node \
  --wasm artifacts/node-wasm/merman_node.js
```

`AbortSignal` removes work that is still queued. It does not interrupt Rust work that has already started. Call `dispose()` to reject pending work and wait for executing work to finish. The synchronous `renderSvgSync()` method is reserved for explicit static-site generation paths.
