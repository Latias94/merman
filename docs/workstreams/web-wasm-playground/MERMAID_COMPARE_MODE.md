# Mermaid Compare Mode

Status: Implemented
Last updated: 2026-08-16

## Purpose

Compare renders one frozen Mermaid source through Merman WASM and pinned Mermaid JS
`11.16.1@7ecca0cd` for interactive side-by-side inspection. It is a compatibility aid, not a
pixel-diff oracle and not the formal benchmark.

## User Contract

- Normal SVG and ASCII views remain Merman-owned.
- Compare is selected explicitly and loads the reference engine lazily.
- Both panes show the exact engine version, render status, and an interactive render duration.
- Desktop uses side-by-side panes; narrow viewports stack them without changing artifact identity.
- Copy consumes the validated displayed artifact. Export opens one shared workbench frozen to the
  chosen engine and publication, with exact SVG plus planned PNG/JPEG output.
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
  +-- Compare realm controller ---> opaque-origin Mermaid iframe
                                      |
                                      +-- local Mermaid/ZenUML/ELK imports
                                      +-- operation queue
                                      +-- SVG safety validation
```

The main document does not import, register, initialize, or render Mermaid. A generated
`sandbox="allow-scripts"` iframe with an opaque origin owns one Mermaid realm and receives a
transferred `MessagePort` after the parent authenticates the exact child Window. The channel
validates an unpredictable token, protocol version, realm id, sequence,
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
- one canonical `800x600` viewport and controlled `screenAvailableWidth=800`;
- diagnostics/Compare flags;
- exact Merman package version.

It invokes Merman and, when enabled, the Compare realm. Monotonic request ids make publication
latest-wins even when non-abortable work completes out of order. Parse/layout diagnostics and
engine render failures remain request artifacts; they do not change the Merman runtime lifecycle.
Benchmark pauses coordinator scheduling and resumes exactly the latest input after cleanup.

Every interactive operation sends `800x600` and `screenAvailableWidth=800` to both engines. The
Mermaid realm installs the controlled screen width before Mermaid loads. Pane allocation, canvas
resize, zoom, pan, pinch, fit, and SVG Bounds visibility remain presentation-only and cannot enqueue
a render. The Benchmark owns its canonical input independently.

The Share menu separates portable workspace links from issue-reproduction links. Workspace links
carry render intent but never device pixels. Issue links additionally restore the workspace pane,
editor tab, Preview mode, and SVG Bounds preference. New links contain no host dimensions or camera
coordinates; legacy Host-bearing links restore supported fields and degrade to canonical rendering.
Neither copy action mutates the current URL or active operation.

Preview presentation preserves each engine's valid SVG `viewBox` byte-for-byte while the diagram
floats directly on a full-surface grid canvas. The responsive clone suppresses Merman's known
default white root background without changing the frozen artifact, exports, or non-default root
backgrounds. A missing-`viewBox` artifact may retain preview-local
intrinsic dimensions, but browser bounds are never promoted into renderer geometry. The optional
SVG Bounds outline follows the mounted root and affects neither fit nor export. Browser-dependent
title or root-viewBox clipping visible in pinned Mermaid therefore remains visible in both panes;
the Playground does not hide it with a Merman-only bounds workaround.

Each Compare pane has one export launcher. The workbench keeps the engine identity visible and
does not retarget when a newer render completes. Mermaid raster output uses the pane's validated
artifact; Merman raster output rerenders the frozen operation once through `resvg-safe`. Preview
and download use the same encoded Blob.

## Security And Resource Lifecycle

Mermaid, ZenUML, ELK, and all adapters are lockfile-pinned and production-bundled. There is no
runtime CDN import. Their digest-bound engine artifact is transferred into the opaque realm, which
is attached and sized for real layout measurement but capability-isolated from application state.

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
