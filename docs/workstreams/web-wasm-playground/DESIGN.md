# Web WASM Playground

Status: Closed

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
        @mermanjs/web capability-specific packages
                              |
          Playground document runtime and Render Coordinator
             |                 |                    |
    editor module Worker   Compare iframe     benchmark iframes
```

The package boundary is deliberately narrower than the application boundary:

- `@mermanjs/web` exposes browser binding operations, explicit disposable editor document
  sessions, and capability metadata.
- `@mermanjs/web-editor` exposes the full family catalog and parser-backed language intelligence
  through the `web-editor` artifact profile; it does not contain SVG, ASCII, host, or ELK
  capabilities.
- The Playground owns loading/retry policy, BFCache behavior, the latest-wins render coordinator,
  Compare and benchmark realms, UI state, and local report download.
- `merman-wasm` is wasm-bindgen browser transport. `merman-typst-plugin` remains the separate
  wasm-minimal-protocol transport.

The Web transport API is version `3`. Editor diagnostics and analysis/facts payloads remain schema `1`.

## Document Runtime

One module-level document runtime owns `idle`, staged `loading`, `ready`, and staged `error` states,
one coalesced acquisition, exact package version, and a disposable browser text-measurement
session. React components observe narrow store selectors; React mount/unmount does not own the
session.

The ESM shim import and hashed WASM fetch start concurrently. The response must be successful and
use `application/wasm`. Browser HTTP cache is the sole persistent byte cache. A retryable
compile/initialization failure may refetch once with `cache: reload`; there is no application Cache
Storage, service worker, or product warmup.

wasm-pack generates the raw shim without a default module path. The public package loader restores
the supported no-argument initialization contract by supplying its package-relative
`MERMAN_WASM_URL`; explicit callers retain control of the actual response or bytes. In production,
the package entry is the unique Vite-manifest owner of the hashed WASM while the raw shim owns no
asset URL.

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

One declarative artifact plan owns engine identities, source entries, generated/public outputs,
realm shape, resource policies, page inputs, and CSP placeholders. Bootstrap versus page ownership
derives the document and manifest shape instead of storing redundant profile labels. Node builders
and verifiers consume that plan directly. Checked-in generated TypeScript leaves retain literal
Vite imports and keep Compare, opaque Benchmark Mermaid, and trusted Benchmark Merman activation
payloads separate.

The visible interactive render timer measures the actual source. There is no hidden synthetic
render. The preview separately records when a validated artifact reaches its presentation
boundary; this feedback is not represented as a formal cross-engine benchmark.

## Benchmark

Benchmark is a separate product surface with one Window realm per engine. Merman uses a trusted
local document and Mermaid uses an opaque execution document. `realm-cold` recreates an
iframe/module realm; `warm` reuses it. It does not claim that realm-cold means network-cold.
Resource Timing entries are retained as observations without inferring unavailable HTTP-cache
provenance.

Protocol `3` and trace schema `1` record realm-local events for font readiness, adapter/engine
imports, resource acquisition, registration, initialization, budgeted output, isolated DOM
insertion, layout, and presentation. One closed phase contract owns applicability, event order,
failure prefixes, progress, publication boundary, and watchdog transitions. Those events do not
claim parent publication safety. Report schema `6` adds one parent-clock vector from sample dispatch
through response delivery, envelope validation, and strict SVG projection. Parent-side first/warm
publishable-SVG totals are the primary cross-engine metrics.

One immutable sample plan owns setup, warmups, measured cold/warm blocks, balanced AB/BA order,
realm reuse, exact work budgets, and aggregation eligibility; the controller interprets that plan
instead of reconstructing counters or schedules. The first request in a reused realm binds the full
payload to an `inputId`; later warm requests transmit only that identity. A typed lifecycle adapter projects
hidden/frozen/resumed/page navigation events without sharing Compare and Benchmark clocks or
realms. Failed samples are excluded, ratios fail closed, and invalid lifecycle boundaries suppress
aggregates. Corpus schema `2` runs one requested fixture per page, keeps a fresh browser process per
fixture, and linearly aggregates one success or structured failure row for every selected fixture.
Evidence can be downloaded locally as versioned JSON; it is not uploaded.

The trusted Merman engine receives the parent-acquired WASM as an explicit in-memory `Response`.
Its private bundle imports the package-owned compiled shim and measurement implementation without
executing the public package entry, so it contains neither a second WASM binary nor a fallback
asset request. The artifact plan enforces this emitted contract and an engine-local byte budget.

## Editor

The Playground configures a local Monaco instance before mounting the editor. Monaco's editor
worker and the Merman language Worker are local module workers; no CDN loader is used.

The Merman Worker imports the selected complete `@mermanjs/web` artifact and owns one disposable
`BrowserEditorSession`. The dedicated `@mermanjs/web-editor` package remains a supported public
surface, but the Playground does not load it in addition to the full renderer: same-revision
whole-site evidence showed that the split did not lower cold transfer or preserve peak memory under
the R16 rule. The full/editor comparison and its exact 35-family/11-query semantic matrix are
on-demand architecture evidence, not a normal browser gate.
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

The maintained frontend baseline is Vite 8.2, React/ReactDOM 19.2.8, ESLint 10, Tailwind 4,
Playwright/Test 1.62.1, and the grouped current Radix, Lucide, Sonner, i18n, and resizable-panel
releases recorded in the lock. Vite uses native static `new URL(..., import.meta.url)` handling;
`vite-plugin-wasm` is absent. The resizable wrapper maps the application's direction vocabulary to
v4 orientation and percentage sizes. Monaco 0.55.1 and TypeScript 5.7.3 remain deliberate holds:
Monaco 0.56 removes the contribution-only entry required by the current lazy language graph, and
TypeScript 7 is outside the admitted tooling line.

Validation is layered: hermetic TypeScript/Vite/policy tests first, prepared unit and production
artifact verification second, then mandatory Chromium desktop plus focused Firefox/WebKit smoke.
The compact Chromium mobile suite is on demand and the remaining real-device checks live in
[MOBILE_QA.md](./MOBILE_QA.md).

The closed lane is protected by:

- Web package type/export/preset/smoke/ABI and size-budget gates;
- exact generated diagram and example catalog checks;
- runtime, render coordinator, realm protocol, queue, benchmark, and Worker unit tests;
- TypeScript-resolved source/type ownership and Vite-manifest emitted ownership as separate graphs;
- plan-driven prepared artifact, CSP, public-byte, and production dist checks;
- mandatory Chromium desktop coverage for startup, render, Compare, Monaco Worker,
  BFCache/teardown, accessibility, CSP, and benchmark behavior;
- one focused mandatory startup/render/Compare/theme/focus flow with BFCache Compare-realm cleanup
  in each of Firefox and WebKit;
- an on-demand Chromium mobile-interaction lane for compact controls, dialog scrolling, workspace
  tabs, touch pan/zoom, shortened visual viewports, and overflow. Real iOS Safari and Android
  Chrome remain an explicit release residual documented in [MOBILE_QA.md](./MOBILE_QA.md).

Historical `TODO.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`, and journal entries record how the
lane was built. They are not current runtime or release contracts.

## Deferred

- A reusable public browser engine/session API requires measured construction-cost evidence and a
  separate ADR.
- Overlay/pixel-diff Compare tools remain optional inspection work.
- Offline/PWA behavior, service workers, remote benchmark storage, analytics, and a benchmark
  leaderboard are not product requirements.
- Immutable deployment headers require hosting-layer authority outside this static repository.
