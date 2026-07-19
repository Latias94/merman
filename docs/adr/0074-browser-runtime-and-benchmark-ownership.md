# ADR 0074: Browser Runtime And Benchmark Ownership

- Status: accepted
- Date: 2026-07-18
- Baselines: Mermaid `11.16.0@7c0cafcf`, native ABI `2`, editor/analysis/facts schema `1`

## Context

The first Playground implementation mixed package loading, Cache Storage, rendering, text
measurement, warmup, and React component state in one loader. Multiple component-local hooks then
projected that module-global work into independent loading states. The Compare and benchmark paths
also shared Mermaid's Window-global mutable state, while their visible timers omitted different
parts of acquisition and initialization.

Those boundaries made correct answers impossible. A render failure could look like a runtime
failure, a bad application cache entry survived retry, and a "first render" could be measured after
an invisible synthetic render. Concurrent Compare and benchmark operations could configure the
same Mermaid object. React effect cleanup and browser page lifecycle were also conflated, which is
incorrect for back/forward cache (BFCache) restoration.

The browser surface now needs explicit owners for realm lifetime, request publication, reference
engine mutation, editor intelligence, and measurement evidence.

## Decision

### 1. One Document-Owned Merman Runtime

The top-level Playground document owns one Merman runtime. React components observe its read-only
Zustand vanilla store and invoke domain commands; component mount count does not acquire or release
the runtime.

The lifecycle states are:

| State | Meaning |
| --- | --- |
| `idle` | No acquisition is active and no session facade is published. |
| `loading(stage)` | One coalesced attempt owns import, fetch, validation, initialization, or session construction. |
| `ready(facade)` | A session owns the exact package version and disposable browser text-measurement resources. |
| `error(stage, recovery)` | Acquisition failed with an explicit stage and either retry or page-reload recovery. |

The wasm-bindgen ESM import and hashed WASM fetch start concurrently. The runtime validates HTTP
status and `application/wasm` before initialization. Browser HTTP caching is the only persistent
byte-cache authority; the Playground does not use Cache Storage or a service worker. A retryable
compile/initialization failure may perform exactly one `fetch(..., { cache: "reload" })` recovery.
An ESM import failure requires page reload because the browser module map cannot be evicted by the
application.

An initialized wasm-bindgen module is Window-realm state and cannot be unloaded. Disposing a
session releases explicitly owned measurement resources and request state; it does not claim to
make a subsequent initialization network-cold.

### 2. Browser Lifecycle Is Not React Lifecycle

The document lifecycle adapter owns browser transitions:

| Event | Runtime action | Auxiliary action |
| --- | --- | --- |
| React root unmount | None | Component resources only. |
| Vite HMR disposal | Dispose the development document session. | Dispose coordinator, realms, workers, and listeners. |
| Persisted `pagehide` or `freeze` | Suspend publication and abort an in-flight acquisition; retain a ready main session. | Invalidate benchmark work and dispose replaceable realms. |
| Persisted `pageshow` or `resume` | Resume or reacquire, then publish only the latest request. | Recreate auxiliary realms lazily. |
| Non-persisted `pagehide` | Best-effort final session disposal. | Dispose all explicitly owned resources. |
| `visibilitychange` | Keep the interactive session, but notify visibility-sensitive owners. | Invalidate an active benchmark when hidden. |

There is no `unload` owner. BFCache entry is suspension, not destruction.

### 3. Render Publication Belongs To A Coordinator

The runtime facade exposes domain operations; it does not own live render batches. A separate
Render Coordinator freezes source, config, theme, font, measurement mode, SVG pipeline, viewport,
and exact package version into a monotonic request snapshot. It debounces interactive work,
discards stale completions, and atomically publishes only the latest coherent Merman/Compare batch.
Request parse, render, or SVG-safety failures remain request artifacts and never transition the
Merman runtime to `error`.

The product path performs no synthetic SVG warmup. Interactive render time is engine execution
feedback for the actual source. `presentedAt` is recorded only after the corresponding validated SVG
has reached the preview presentation boundary. It is not a substitute for the benchmark phase
vector described below.

### 4. Mermaid Mutation Belongs To Its Realm

The main document does not import or configure Mermaid. Compare owns a generated
`sandbox="allow-scripts"` iframe with an opaque origin and an authenticated `MessagePort`
capability. The generated bootstrap is a self-contained, CSP-hashed artifact whose manifest binds
the exact engine artifact identity. The realm verifies the engine source digest before importing
it from a blob URL. The channel validates the opaque origin, Window source, boot nonce, token,
protocol, message budgets, request sequence, viewport, and source/config size. The parent accepts
only a native SVG that passes the shared strict projector immediately before DOM insertion.

One failure-resilient queue serializes the complete Mermaid operation in that realm: local adapter
and engine import, external diagram/layout registration, initialization, render, ZenUML recovery,
and SVG validation. A rejection advances the queue. A timeout or protocol failure poisons and
destroys the realm before another operation can start.

Compare and benchmark never share a Mermaid object, operation queue, or iframe.

### 5. Benchmark Evidence Has Explicit Clock And Publication Boundaries

The benchmark runs Merman and Mermaid in separate Window realms with the same frozen operation
input, viewport, sample protocol, and scheduling policy. Merman uses a trusted local document;
Mermaid uses the same opaque-origin execution boundary as Compare. A `realm-cold` sample creates a
new engine iframe; this means a new Window/module realm, not a proven network-cold transfer. A
`warm` sample reuses that engine realm. Resource Timing observations are retained when available,
but they are diagnostics and do not prove HTTP cache provenance when the browser omits transfer
details.

Benchmark protocol `1` carries trace schema `1`. Each sample records one realm-local monotonic
event vector:

```text
sample_start
fonts_wait_start/end
adapter_import_start/end
engine_import_start/end
resource_acquire_start/end
register_start/end
initialize_start/end
render_start
budgeted_svg_ready
isolated_dom_inserted
isolated_layout_box_ready
isolated_presentation_ready
sample_end
```

Inapplicable or unobserved events are `null`, not zero. The controller validates ordering and
derives non-overlapping observations instead of summing spans. Realm-local `budgeted_svg_ready`
means only that the engine produced a response within its output budget;
`isolated_presentation_ready` means the isolated document completed DOM insertion, layout, and a
real animation frame. Neither event claims that the parent may publish the markup.

Every measured sample also records one parent-clock publication vector from sample dispatch,
through isolated-presentation progress receipt, response receipt, response-envelope validation,
and the shared strict SVG projector. `firstPublishableSvgMs` and `warmPublishableSvgMs` are the
primary comparable totals because both engines cross exactly that parent-side boundary. Response
delivery, envelope-validation, and strict-validation spans remain separate evidence. The UI may
summarize median, p95, min, max, mean, and coefficient of variation; report schema `4` retains both raw
realm-local events and parent publication evidence.

Runs use the same frozen source/options, `document.fonts.ready`, equal real-source warmups, a
recorded deterministic seed, and balanced interleaved AB/BA blocks. Failed samples never enter
aggregates. Corresponding ratios are absent when either engine has an invalid or failed sample.
Cancellation, timeout, iframe failure, protocol failure, hidden visibility, navigation, or freeze
produces an explicit terminal state. Visibility/lifecycle invalidation suppresses all aggregates so
no result spans an invalid environment boundary.

No report is uploaded or persisted remotely. Download is an explicit local action.

The execution iframe retains the requested client viewport but is transformed to a stable
one-CSS-pixel presentation footprint. This keeps real animation-frame evidence observable in
headless and foreground browsers without letting the hidden benchmark UI affect layout inputs.

### 6. Editor Intelligence Belongs To A Dedicated Worker

The Playground loads local Monaco code and its editor worker; it does not use Monaco's default CDN
loader. Merman language intelligence runs in a dedicated module Worker using the public
`@mermanjs/web/editor` subpath and `browser-editor` WASM preset. The preset includes the full
diagram catalog, analysis, and `merman-editor-core`, but excludes SVG rendering, ASCII, host
capabilities, and ELK.

The editor Worker owns one document URI and monotonically increasing version. `didOpen`,
`didChange`, query, cancellation, protocol validation, and disposal cross its typed channel.
Diagnostics, completion, hover, code actions, symbols, definition, references, rename, and semantic
tokens are projections of Rust `merman-editor-core`, not TypeScript syntax heuristics. Results with
stale document versions are discarded. Protocol or result-shape mismatch fails closed.

The native browser ABI remains `2`; editor diagnostics and shared analysis/facts remain schema `1`.
These numbers describe different contracts and do not advance together.

### 7. Examples And Detection Have Canonical Sources

The Diagram Family catalog remains the type and capability authority. A checked-in manifest
selects one or more fixture-backed Playground examples for each of the 35 full-profile logical
families. `xtask` proves exact family-set coverage, fixture/source provenance, canonical detection,
pinned Mermaid baseline, and generated TypeScript freshness.

The Playground consumes typed parser facts from Rust/WASM for logical diagram type, syntax id, and
effective layout id. Mermaid package-registration requirements are a Playground projection over
those facts. Source-prefix and regular-expression classification are not fallback authorities.

### 8. Public Package And Internal Runtime Boundaries

`@mermanjs/web` remains a stateless browser binding package. Its lifecycle functions initialize a
realm-local wasm-bindgen module, but the Playground runtime store, Render Coordinator, iframe
protocols, benchmark controller, and application retry policy are product code above the package.

`@mermanjs/web/editor` is a public capability-specific subpath backed by `browser-editor`.
`@mermanjs/web/full` remains the explicit full surface, and the default import remains
`browser-full`. A reusable public engine/session factory is deferred until benchmark evidence shows
that construction cost and consumer ownership justify a new stable API.

### 9. Deployment Cache Headers Remain A Hosting Concern

The production build guarantees content-hashed assets and validates the emitted WASM reference and
MIME in browser tests. The observed deployment currently serves hashed assets with
`Cache-Control: max-age=14400`, without `immutable`. The desired hosting policy is immutable
long-lived caching for hashed assets and revalidation for HTML. That header authority is outside
the repository's static bundle; the application must not emulate it with Cache Storage or claim it
is already deployed.

## Enforcement

Durable guards operate on contracts rather than private spelling:

- TypeScript/package import graphs and public export manifests enforce package ownership.
- WASM preset manifests, smoke tests, ABI checks, and size budgets enforce capability surfaces.
- Closed channel validators and browser tests enforce realm capability and lifecycle behavior.
- Generated example checks enforce exact 35-family set coverage, per-example provenance, and
  freshness.
- Production artifact and request inspection enforce local Monaco/Mermaid dependencies and CSP.
- Unit tests exercise state transitions, queue recovery, latest-wins publication, event derivation,
  cancellation, invalidation, and cleanup.
- Browser isolation tests prove opaque origin, denied ambient authority, bounded ephemeral storage,
  zero server receipt for CSP-blocked network APIs, and realm poisoning after navigation or failure.

Source searches may inventory obsolete files during migration, but parameter names, private
function prefixes, and arbitrary source substrings are not architecture contracts.

## Alternatives Considered

### Keep Component-Local Runtime Hooks

Rejected. Multiple views of one module-global initialization cannot provide coherent retry,
failure, BFCache, or disposal semantics.

### Keep Application Cache Storage

Rejected. There is no offline contract. It duplicates the browser HTTP cache, adds independent
freshness/eviction behavior, and can persist invalid bytes across retries.

### Run Compare And Benchmark In The Main Window

Rejected. Mermaid registration and initialization mutate realm-global state. Queuing render alone
does not isolate configuration, external registration, hangs, or cold-realm measurement.

### Call A Hidden Warmup Before Interactive Rendering

Rejected. It makes the visible first-result metric untruthful and obscures acquisition cost.
Benchmark-only warmup remains explicit, symmetric, real-source work.

### Reimplement Browser Language Features In TypeScript

Rejected. Static snippets and source scans drift from the shared parser and LSP semantics. The
dedicated Rust/WASM Worker provides the same protocol-neutral editor core while keeping the main
thread responsive.

## Consequences

- The Playground has one observable runtime truth and separate owners for lifecycle and requests.
- BFCache restoration can retain a valid main session without retaining benchmark/Compare realms.
- Interactive timing is honest but intentionally different from benchmark evidence.
- Compare and benchmark failures can be recovered by destroying their owning realms.
- The editor bundle is larger because Monaco and a Worker WASM are local, but it provides real
  parser-backed language intelligence and makes no runtime CDN dependency.
- HTTP cache quality remains measurable deployment evidence rather than hidden application policy.
- Native ABI stays `2`, editor/analysis/facts schemas stay `1`, and Mermaid remains pinned to
  `11.16.0@7c0cafcf`.
