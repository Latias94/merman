# Editor artifact measurement

This is an on-demand architectural measurement, not a CI or normal test gate. It
compares two dedicated Playground builds without changing the production Vite
configuration:

- `full`: the main thread and language Worker both use `@mermanjs/web`.
- `editor`: the main thread keeps `@mermanjs/web`, while the language Worker uses
  the editor package and editor WASM artifact.

Run the complete measurement from `playground/`:

```console
npm run measure:editor-artifacts
```

The command creates fresh dedicated builds, runs balanced AB/BA cold and warm
samples in Chromium, executes a separate semantic-equivalence probe, and writes
the versioned receipt to
`target/playground/editor-artifact-measurement/receipt-v2.json`. Use
`--skip-build` only for local smoke checks because it reuses existing build
directories. A receipt records the Git revision and whether the worktree was
dirty. Only a fresh-build receipt from an unchanged clean worktree is marked
authoritative; dirty or reused-build receipts remain explicitly provisional.
The checked receipt also binds the measurement contract, startup and Worker
closures, exact full/editor runtime package provenance, and equivalence evidence
through selection-input digests. `npm run verify:editor-artifact-authority`
recomputes those digests and the derived decision without building or launching
a browser. Web source and build-script changes do not independently invalidate a
receipt because the measured, provenance-verified JS/WASM artifacts are the
runtime authority. Documentation, browser-test-only changes, emitted declarations,
and source maps do not force an R16 rerun. Runtime production closures do: Vite embeds
dynamic chunk identities in the startup entry, and the PostCSS/Tailwind build
can project lazy-workbench classes into the initial stylesheet.

The equivalence probe uses the production `WorkerClient` and the emitted
`merman-language.worker` as an explicit module Worker. Its input is the generated
`editor-language/token-equivalence-v1.json` evidence, which contains exactly one
family-baseline source for each of the 35 supported families. For both the
`full` and `editor` variants, the probe unconditionally executes all 11 query
kinds against every family. Request-local query errors are evidence too; fatal
transport or protocol errors fail the measurement.

Every successful result or request-local error is converted to a canonical JSON
form: `Uint32Array` values become arrays and object keys are recursively sorted.
The receipt stores each of the 385 per-query SHA-256 digests, source identities,
and a recomputed aggregate digest for both variants. The pure Node contract
rejects incomplete or stale matrices and derives exact equivalence by comparing
every family identity, outcome, query digest, and aggregate digest.

The transfer metric is the gzip response-body bytes served by the dedicated
same-origin server, including page and Worker requests. Peak memory is the
maximum startup sample returned by `performance.measureUserAgentSpecificMemory`
in cross-origin-isolated Chromium; no narrower heap fallback is accepted.

The decision selects the editor artifact only when all of these conditions hold:

1. Full and editor are exactly equivalent across all 35 families and 11 query
   kinds.
2. Editor cold total transfer bytes are strictly lower than full.
3. Editor peak memory does not exceed full.
4. No cold or warm primary latency regresses by both more than 5% and more than
   20 ms. The primary latencies are Worker ready, first diagnostics, and the main
   renderer's first result.

Raw AB/BA samples, build hashes, secondary compile/initialize timings, summaries,
the complete equivalence matrices, receipt authority, and the decision
explanation remain in the receipt. `contract.mjs` and the receipt's explicit
`schemaVersion` are the single executable contract authority. A separate JSON
Schema is intentionally omitted until an external consumer requires one.
