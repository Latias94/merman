# Mermaid Compare Mode

Status: Implemented
Last updated: 2026-07-18

## Purpose

Compare renders one frozen Mermaid source through Merman WASM and pinned Mermaid JS
`11.16.1@7ecca0cd` for interactive side-by-side inspection. It is a compatibility aid, not a
pixel-diff oracle and not the formal benchmark.

## User Contract

- Normal SVG and ASCII views remain Merman-owned.
- Compare is selected explicitly and loads the reference engine lazily.
- Both panes show the exact engine version, render status, and an interactive render duration.
- Desktop uses side-by-side panes; narrow viewports stack them without changing artifact identity.
- SVG/PNG export and copy actions consume the validated displayed artifact.
- A Merman failure and Mermaid failure remain independent evidence; partial success is visible.
- Source/config/theme/font changes produce one latest coherent batch. Actions are disabled while a
  replacement batch is pending.

Interactive durations describe engine execution for the actual source. They exclude no work by
performing a hidden synthetic warmup, and they are not presented as cross-engine benchmark phases.
`presentedAt` is recorded separately when a validated SVG reaches its preview presentation
boundary.

## Ownership

```text
top document
  |
  +-- Merman document runtime ----> Merman request artifact
  |
  +-- Render Coordinator ---------> latest coherent batch
  |
  +-- Compare realm controller ---> same-origin Mermaid iframe
                                      |
                                      +-- local Mermaid/ZenUML/ELK imports
                                      +-- operation queue
                                      +-- SVG safety validation
```

The main document does not import, register, initialize, or render Mermaid. A same-origin iframe
owns one Mermaid realm and receives a transferred `MessagePort` after exact origin/Window
handshake. The channel validates an unpredictable token, protocol version, realm id, sequence,
message/source/config/result budgets, and request identity. The parent validates returned SVG again
before DOM insertion.

One recovering queue owns the complete reference operation:

1. import the local adapter and pinned Mermaid module;
2. register only the external diagram/layout requirements derived from canonical parser facts;
3. initialize stable site and request configuration;
4. render under a unique request id;
5. recover and retry the bounded ZenUML registration case when applicable;
6. validate the SVG before returning it.

A rejected operation does not poison the queue. A timeout, malformed protocol, or stuck global
mutation poisons and destroys the iframe; the next request must create a fresh realm.

## Canonical Requirements

Merman's typed detection facts provide the logical family, syntax id, and effective layout id.
Playground code maps those neutral facts to Mermaid-only requirements such as ZenUML or ELK
registration. Compare does not scan source prefixes or regular expressions as a fallback. This
keeps frontmatter, directives, aliases, incomplete input, ELK selection, and Mermaid 11.16 families
aligned with the shared Rust parser.

## Render Coordinator

The coordinator freezes:

- source;
- config JSON;
- theme and diagram font;
- text-measurement mode and SVG pipeline;
- Compare viewport;
- diagnostics/Compare flags;
- exact Merman package version.

It invokes Merman and, when enabled, the Compare realm. Monotonic request ids make publication
latest-wins even when non-abortable work completes out of order. Parse/layout diagnostics and
engine render failures remain request artifacts; they do not change the Merman runtime lifecycle.
Benchmark pauses coordinator scheduling and resumes exactly the latest input after cleanup.

## Security And Resource Lifecycle

Mermaid, ZenUML, ELK, and all adapters are lockfile-pinned, production-bundled, and same-origin.
There is no runtime CDN import. The realm is attached and sized for real layout measurement, but it
is capability-isolated from application state.

Compare owns its iframe, port, handshake listeners, pending request, timeout, and operation queue.
It disposes them on replacement, HMR, non-persisted exit, BFCache suspension, or protocol poison.
BFCache restoration creates the realm lazily when Compare is needed again.

## Relationship To Benchmark

The benchmark never reuses the Compare iframe or Mermaid object. It creates equivalent dedicated
Window realms for both engines and records versioned realm-local phase events. It uses equal
real-source warmups, deterministic balanced AB/BA ordering, raw failure retention, visibility
invalidation, and fail-closed ratios. See
`docs/adr/0074-browser-runtime-and-benchmark-ownership.md`.

Compare's render duration answers \"how long did this interactive engine call take?\" The benchmark
answers separate acquisition, initialization, valid-SVG, and presentation questions. Neither value
is derived from the other.

## Non-Goals

- Native CLI versus Mermaid JS benchmarking.
- A claim of pixel-perfect equivalence.
- Overlay, swipe, source-DOM diff, or raster pixel diff in the core Compare flow.
- A user-selectable unpinned Mermaid version.
- Sharing Mermaid global state with the main document or benchmark.
- Loading reference-engine dependencies before Compare is requested.

Overlay or structured DOM inspection may be added later as an explicitly separate inspector, but
browser text, `getBBox()`, font, `foreignObject`, RoughJS, and wrapper differences must remain
visible residuals rather than being normalized away in production.
