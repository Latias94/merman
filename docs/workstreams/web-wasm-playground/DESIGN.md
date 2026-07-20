# Web WASM Playground

Status: Closed
Last updated: 2026-07-20

## Purpose

This lane established Merman's browser package and Playground. The current implementation is a
local-first Mermaid authoring surface: Rust/WASM owns parsing, rendering, analysis, and editor
semantics; product code owns browser lifecycle, presentation, Compare, and benchmark policy.

Current architecture is governed by:

- `docs/adr/0069-wasm-package-surface-semantics.md` for browser/Typst package separation;
- `docs/adr/0074-browser-runtime-and-benchmark-ownership.md` for document, realm, Worker, cache, and
  measurement ownership;
- `docs/adr/0073-family-owned-diagram-architecture.md` for the canonical 35-family catalog and
  semantic construction;
- `platforms/web/README.md` for public TypeScript entry points and ABI requirements.

## Current Layers

```text
merman-core / merman-analysis / merman-editor-core / merman-render
                              |
                    merman-bindings-core
                              |
                     crates/merman-wasm
                              |
       @mermanjs/web capability-specific package subpaths
                              |
          Playground document runtime and Render Coordinator
             |                 |                    |
    editor module Worker   Compare iframe     benchmark iframes
```

The package boundary is deliberately narrower than the application boundary:

- `@mermanjs/web` exposes browser binding operations, explicit disposable editor document
  sessions, and capability metadata.
- `@mermanjs/web/editor` exposes the full family catalog and parser-backed language intelligence
  through `browser-editor`; it does not contain SVG, ASCII, host, or ELK capabilities.
- The Playground owns loading/retry policy, BFCache behavior, the latest-wins render coordinator,
  Compare and benchmark realms, UI state, and local report download.
- `merman-wasm` is wasm-bindgen browser transport. `merman-typst-plugin` remains the separate
  wasm-minimal-protocol transport.

Native browser ABI remains `2`. Editor diagnostics and analysis/facts payloads remain schema `1`.

## Document Runtime

One module-level document runtime owns `idle`, staged `loading`, `ready`, and staged `error` states,
one coalesced acquisition, exact package version, and a disposable browser text-measurement
session. React components observe narrow store selectors; React mount/unmount does not own the
session.

The ESM shim import and hashed WASM fetch start concurrently. The response must be successful and
use `application/wasm`. Browser HTTP cache is the sole persistent byte cache. A retryable
compile/initialization failure may refetch once with `cache: reload`; there is no application Cache
Storage, service worker, or product warmup.

BFCache entry suspends publication and disposes replaceable realms while retaining a valid main
session. BFCache restoration resumes or reacquires it and schedules the latest request once.
Non-persisted exit and HMR dispose explicitly release owned resources. Window-realm destruction
ultimately owns the initialized wasm-bindgen module.

## Rendering And Compare

The Render Coordinator freezes source, config, theme, font, measurement mode, SVG pipeline,
viewport, and package version. It publishes only the latest coherent batch and keeps request
failures outside runtime lifecycle state.

The main document never imports Mermaid. Compare uses a generated
`sandbox="allow-scripts"` iframe with an opaque origin, authenticated MessagePort, closed protocol,
and message budgets. A CSP-hashed bootstrap manifest binds the exact engine artifact identity; the
realm verifies the engine digest before blob import. One recovering queue owns external
registration, initialization, render, and ZenUML recovery. The parent alone converts returned
markup into `SafeInlineSvg` through the shared strict validator immediately before publication.
Timeout, protocol corruption, invalid SVG, or navigation poisons and destroys the realm.

The visible interactive render timer measures the actual source. There is no hidden synthetic
render. The preview separately records when a validated artifact reaches its presentation
boundary; this feedback is not represented as a formal cross-engine benchmark.

## Benchmark

Benchmark is a separate product surface with one Window realm per engine. Merman uses a trusted
local document and Mermaid uses an opaque execution document. `realm-cold` recreates an
iframe/module realm; `warm` reuses it. It does not claim that realm-cold means network-cold.
Resource Timing entries are retained as observations without inferring unavailable HTTP-cache
provenance.

Protocol `1` and trace schema `1` record realm-local events for font readiness, adapter/engine
imports, resource acquisition, registration, initialization, budgeted output, isolated DOM
insertion, layout, and presentation. Those events do not claim parent publication safety. Report
schema `4` adds one parent-clock vector from sample dispatch through response delivery, envelope
validation, and strict SVG projection. Parent-side first/warm publishable-SVG totals are the primary
cross-engine metrics. The controller alone derives intervals and aggregate statistics. Equal
real-source warmups, a recorded seed, and balanced AB/BA blocks reduce order bias. Failed samples
are excluded, ratios fail closed, and hidden/frozen/navigation boundaries invalidate the run and
suppress aggregates. Evidence can be downloaded locally as versioned JSON; it is not uploaded.

## Editor

The Playground configures a local Monaco instance before mounting the editor. Monaco's editor
worker and the Merman language Worker are local module workers; no CDN loader is used.

The Merman Worker imports `@mermanjs/web/editor` and owns one disposable `BrowserEditorSession`.
`didOpen` constructs its analyzed document, `didChange` atomically replaces the snapshot with a
newer source/version, and queries do not resend or compare source text. Diagnostics, detection,
completions, hover, code actions, symbols, definition, references, rename, and semantic tokens all
read that same snapshot. One generated descriptor supplies Monaco's legend; WASM returns the
already validated packed token sequence.
Stale results are discarded and protocol, version, descriptor-digest, or result-shape mismatch
fails closed. Cancelling a client wait does not claim to interrupt a synchronous WASM call: the
Worker finishes that call and its versioned result is ignored. TypeScript syntax heuristics are not
a fallback language service.

This is VS Code-like language analysis over the shared Rust editor core, not a browser-hosted LSP
process. LSP transport concerns remain in `merman-lsp`; editor behavior is shared below both
adapters.

## Examples And Detection

`playground/examples/manifest.json` selects one or more fixture-backed examples for every full
profile logical family. The current teaching catalog contains 78 examples across the exact
35-family set. `xtask` proves source provenance, canonical detection, Mermaid `11.16.0` baseline,
and generated output freshness. The gallery searches generated titles, categories, aliases, syntax
ids, and family types.

Live diagram identity comes from typed Rust/WASM parser facts, including logical family, syntax id,
and effective layout. Playground-only Mermaid registration requirements project those facts.
First-line prefix and regex classification are not canonical paths.

## Browser Trust And Cache Boundary

All runtime modules, Monaco workers, Mermaid, ZenUML, ELK, and WASM assets are production-bundled
and acquired from owned application artifacts. Mermaid and its external modules execute inside an
opaque-origin document that cannot directly fetch them; the parent transfers a digest-bound engine
artifact over the authenticated channel. CSP denies connection, worker, frame, media, object, and
non-data image capabilities. Only bounded frozen ephemeral storage facades are installed after
engine verification. A self-navigation can issue its first document request before observation;
the next load poisons the session and removes the frame, so absolute zero egress remains a host
interception capability rather than a browser-iframe claim. SVG must pass the shared parent-side
DOM-safety policy before insertion.

Vite content hashes allow HTTP caching, but deployment headers are externally owned. The currently
observed hashed-asset policy is `Cache-Control: max-age=14400`, without `immutable`. Long-lived
immutable caching for hashed assets and HTML revalidation remain the desired host configuration;
the app does not simulate that policy with Cache Storage.

## Verification

The closed lane is protected by:

- Web package type/export/preset/smoke/ABI and size-budget gates;
- exact generated diagram and example catalog checks;
- runtime, render coordinator, realm protocol, queue, benchmark, and Worker unit tests;
- production build graph and artifact checks;
- real-browser startup, render, Compare, Monaco Worker, BFCache/teardown, accessibility, responsive,
  CSP, and benchmark tests.

Historical `TODO.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`, and journal entries record how the
lane was built. They are not current runtime or release contracts.

## Deferred

- A reusable public browser engine/session API requires measured construction-cost evidence and a
  separate ADR.
- Overlay/pixel-diff Compare tools remain optional inspection work.
- Offline/PWA behavior, service workers, remote benchmark storage, analytics, and a benchmark
  leaderboard are not product requirements.
- Immutable deployment headers require hosting-layer authority outside this static repository.
