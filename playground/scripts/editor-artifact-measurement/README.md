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
The checked receipt also binds the measurement contract, every production page
runtime closure, the Worker/equivalence closure, the portable full/editor
runtime package contract, and equivalence evidence through selection-input
digests. The package contract binds exact runtime JavaScript, package exports,
artifact profile and capabilities, and the WASM source digest. Before computing
that contract during measurement, every JavaScript and WASM runtime artifact
must match its own provenance record. The recorded contract deliberately
excludes platform-specific WASM binary identity, build-input digest, and
host-form tool descriptions: the same pinned sources and toolchain release can
emit different bytes on supported builders.

`npm run verify:editor-artifact-authority` is a fast selection-topology check,
not a measurement-freshness gate. It validates that the checked receipt records
an authoritative capture, then proves that the current package dependencies,
exact local lockfile resolutions, and the single runtime `?worker` edge reachable
from `src/editor/worker-browser.ts` still implement the artifact selected by that
receipt. The verifier follows the resolved production Worker target rather than a
historical filename. It does not compare the receipt's historical content
digests with the current build. Ordinary source, package, or WASM changes
therefore use their normal unit, contract, provenance, and build checks instead
of forcing a new Chromium run.

Rerun the on-demand measurement only when maintainers deliberately:

- reconsider switching between the `full` and `editor` Worker artifacts;
- change the 5%/20 ms, memory, cold-byte, or semantic-equivalence decision rules;
- change the two-artifact topology, realm ownership, or measurement method; or
- request new economic evidence after a major architecture change.

Vite may split shared chunks across any production HTML entry, and the
PostCSS/Tailwind build can project lazy-workbench classes into the initial
stylesheet. Tailwind automatic source detection is disabled; its explicit
source roots are all members of the production TypeScript runtime closure, so
test and tooling text cannot mutate the measured CSS.

The equivalence probe uses the production `WorkerClient` and the emitted
`merman-language.worker` as an explicit module Worker. Its input is the generated
`contracts/editor-language/token-equivalence-v1.json` evidence, which contains exactly one
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
