# Benchmark Matrix

Use one row per implementation, transport, target, and workflow. Store raw samples and the command that produced them; publish summaries only after parity and input-digest checks pass.

## Required provenance

Record the base/target commit, host OS and CPU, logical CPU count, Rust/Cargo versions, Node/npm versions, optimization profile, target triple, lockfile digest, benchmark harness digest, corpus path and digest, input options, warmups, measured iterations, concurrency, and date in UTC.

## Native render/parse comparison

Compare Merman, the pinned `mermaid-rs-renderer` reference, and Mermaid.js only when all three consume an equivalent corpus and produce the same requested operation. Prefer the repository's existing corpus and `docs/performance/BENCHMARKING.md`; inspect the command source before running it.

For a release-to-current Merman comparison, run two explicitly labeled lanes:

- Product default: each revision's documented default features. This answers what a default user
  pays for, including deliberately added capabilities.
- Same capability: `--no-default-features` plus the smallest equivalent SVG leaves. Exclude rows
  unavailable in either revision and verify the fixture source hashes before calculating ratios.

Do not collapse those lanes into one regression number.

Collect separate metrics:

| Metric | Meaning |
| --- | --- |
| `parse_semantic_ms` | Parse and semantic-model construction only. |
| `layout_ms` | Typed layout only when the implementation exposes a stable operation. |
| `render_svg_ms` | Parse + layout + SVG serialization for the same output contract. |
| `cold_start_ms` | Process/module initialization plus one operation. |
| `warm_p50/p95_ms` | Repeated in-process operation latency. |
| `rss_peak_bytes` | Peak process memory under the declared sampling method. |
| `success/parity` | Successful outcomes and structural/semantic parity status. |

Do not compare Mermaid.js browser `getBBox()` timing with a deterministic Rust renderer as though it were the same workload. Report browser/text-measurement residuals separately.

## Rust size and dependency closure

For each named workflow, run `cargo tree --locked --edges normal --prefix none --format '{p}'` and count unique `name@version@source` identities. Record direct manifest dependencies separately. Build with the same `--release`/`--profile`, target, lockfile, and feature selection, then record:

```console
stat -f '%z %N' target/release/<binary>
file target/release/<binary>
```

On non-macOS hosts use the platform equivalent of `stat`. If a stripped artifact is the shipped unit, measure the stripped copy and the packaged archive, not only the unstripped linker output.

## Web and Typst WASM

Use the exact artifact profile rather than a legacy Cargo preset:

```console
cargo run -p xtask -- wasm-size-matrix \
  --surface web --artifact-profile web-full \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run -p xtask -- wasm-size-matrix \
  --surface web --artifact-profile web-render \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

Store `raw_bytes`, `stripped_bytes`, `gzip_bytes`, and `brotli_bytes`, plus the profile's capabilities and outputs. Compare `web-render` with an older artifact only if the older artifact also includes the same SVG/layout/math contract. A full package that gained analysis, editor, ASCII, a layout engine, or math is not a fair size baseline for a slim renderer.

## Node transport comparison

The private harness under `platforms/node` is the authority for candidate packaging and transport parity:

```console
npm test --prefix platforms/node
npm run build:candidate --prefix platforms/node -- --candidate node-wasm
npm run build:candidate --prefix platforms/node -- --candidate napi --target darwin-arm64
npm run benchmark --prefix platforms/node -- \
  --native artifacts/napi/darwin-arm64/merman.node \
  --wasm artifacts/node-wasm/merman_node.js
```

The harness measures cold process, warm end-to-end SVG, concurrency, RSS, queue lifecycle, typed errors, installed footprint, and semantic/SVG parity. Add a separate semantic-only probe for parse speed by invoking the same installed product facade with `operationId: "semantic-json"` and the exact corpus/options. Never label SVG latency as parse latency.

Node transports can serialize map-like JSON in a different object-key order. Parse
`semantic-json`, recursively sort object keys, preserve array order, and compare the canonical
form. Keep both the raw-byte difference count and the canonical semantic mismatch count in the
evidence so a serializer ordering difference cannot be misreported as a semantic failure.

Treat `@mermanjs/node` as private/inconclusive until every declared target has runtime evidence and the harness decision admits it. If `--locked` fails because `crates/merman-node/Cargo.lock` is stale, preserve that failure and classify the current branch as not reproducibly buildable.

## Mermaid.js and mermaid-rs-renderer

Pin the Mermaid.js package/repository revision and the `mermaid-rs-renderer` revision. Record whether each run uses a browser, Node DOM shim, or native path, and keep DOM measurement, SVG canonicalization, and font setup explicit. If a reference cannot run on the host, report the missing prerequisite and keep Merman's result independently useful; do not replace it with a different implementation or a guessed value.

## Result schema

Keep raw data in machine-readable form:

```json
{
  "implementation": "merman-node-wasm",
  "operation": "semantic-json",
  "corpus_digest": "sha256:...",
  "samples_ms": [],
  "summary": {"p50_ms": 0, "p95_ms": 0},
  "parity": {"matched": 0, "mismatched": 0},
  "provenance": {"commit": "...", "target": "..."}
}
```

Do not publish a winner from a single outlier, a mismatched corpus, or a candidate with failed parity.
