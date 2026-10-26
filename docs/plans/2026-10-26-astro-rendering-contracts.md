---
title: Repair browser typography and scoped JavaScript icon services
type: fix
status: complete
date: 2026-10-26
---

# Browser typography and scoped JavaScript icon services

## Problem and scope

Astro-Mermaid issue 77 exposes a readable-output defect and a missing integration seam.
Class labels retain their text in SVG but can be measured with a different font from the
font used to display their bounded HTML containers. Canvas can also silently reject a
font assignment and return plausible metrics from an earlier font. Separately, the Rust
icon registry exists, but JavaScript callers cannot supply already loaded icon packs.

Implement and verify fixes on both `main` (Mermaid 12.2, Options JSON 3) and
`release/0.9.x` (Mermaid 12.1, Options JSON 2). Preserve each release line's existing
configuration and transport contracts. The earlier performance investigation remains
suspended. The research origin is
`docs/research/astro_mermaid_issue_77_2026-10-26.md`.

## Design

### Typography has two distinct measurement responsibilities

Preserve Mermaid's root-config wrapping-budget probe. It is an input to wrapping,
not an authority for the final displayed HTML label's dimensions.

Final Class title, member, and method measurement must use the effective family,
size, style, and weight of the displayed label. Reuse the existing prepared label
plan so layout and SVG serialization consume one result. Reuse existing CSS font
parsing and HTML measurement instead of introducing another font parser or a
browser-only rendering pipeline.

Browser Canvas measurement must isolate font state, detect rejected assignments,
and return the existing unhandled result when the requested font cannot be used.
Do not compare a requested font string literally with the browser's canonicalized
readback. Test quoted family names, relative weights, malformed values, and
sequential calls.

Deterministic measurement remains an explicit font-independent approximation.
This work does not promise exact arbitrary-font browser bounds without a host
measurement provider. Do not hide failures with fixture padding, global overflow,
font-specific heuristics, or ambient system-font discovery.

### Icon packs are immutable constructor services

The host acquires Iconify pack JSON. The binding admits that data through the
existing `build_icon_registry` owner and its resource limits. Add one shared JSON
adapter accepting an array of `{ json, registrationName? }` entries; bound its
encoded envelope before deserializing and preserve structured registry errors.

Node exposes `createNodeEngine({ iconPacks })`. Native and Node-WASM transports
receive a separate optional constructor argument. Binding Options JSON does not
gain an icon field, and existing callers continue to work.

Web exposes `createRenderSession({ iconPacks })` on render-capable surfaces. Each
session owns immutable services and supports SVG, SVG-result, layout, and edge
geometry operations, including existing synchronous host-measurer variants.
Operation options remain per call. The ordinary package functions keep their
existing service-free behavior. A disposed session rejects further calls; disposal
from a measurement callback defers freeing until the active call finishes.
Reuse the existing runtime-state and callback guards. Do not introduce a mutable
global icon registry, network loader, plugin framework, or registry cache.

Update actual artifact catalogs and generated facade contracts so service
discovery describes the exposed constructors. Keep render-only and full Web
surfaces aligned without leaking renderer dependencies into analysis/editor SKUs.

### Host integration

Document a small browser/build-time adapter with version-correct options, explicit
font readiness, unique diagram IDs, and scoped pack ownership. Astro owns theme
observation, pack fetching, and page lifecycle. Keep the Web server-runtime guard;
use the existing Node or Node-WASM package for SSR.

## Implementation units

1. Repair Class final typography ownership and add semantic regression coverage.
2. Repair Canvas assignment admission and add fake-context and real-browser checks.
3. Add the shared loaded-pack adapter and Node constructors, types, and tests.
4. Add scoped WASM/Web sessions, public types, package exports, and lifecycle tests.
5. Verify integrated output and update host guidance.
6. Adapt the changes to `release/0.9.x`, retaining its API and baseline semantics.
7. Review both diffs and create precise local Conventional Commits.

The renderer, Canvas adapter, and Node binding changes can be developed in parallel
with disjoint file ownership. One controller runs Cargo serially using the existing
target directory and integrates generated contracts after worker edits settle.

## Acceptance and verification

- Final Class HTML measurement requests match the emitted effective typography;
  the root wrapping probe retains its upstream behavior.
- In a real browser, host-measured Vehicle/Driver labels fit their containers with
  equivalent configured CSS. Record deterministic approximation separately.
- Invalid Canvas font assignments never report metrics from stale/default font
  state. Valid canonicalized fonts and quoted family names still work.
- Web and both Node transports render a custom icon from an already loaded pack,
  honor registration-name overrides, and isolate unrelated sessions/engines.
- Empty and omitted packs preserve existing calls. Invalid packs preserve binding
  error details. Reentrant calls and callback-time disposal remain safe.
- Focused Rust tests use nextest, with `CARGO_BUILD_JOBS=1`; no parallel Cargo jobs.
  Run affected binding/renderer/WASM/Node suites and feature checks.
- Web and Node contract generation, package tests, TypeScript checks, and actual
  browser/Node-WASM smoke checks pass against locally built artifacts.
- Run formatting, change-scoped Clippy, and representative SVG structure checks.
  Explain source-backed geometry changes instead of blindly refreshing baselines.
- Verify and commit each release line independently. Do not publish, push, open a
  PR, bump a package version, or post to the upstream issue.

## Risks and boundaries

Browser fonts must be loaded by the host; family fallback and deterministic width
estimation remain environment-dependent. HTML metrics and wrapping probes must
not be conflated. The maintained line has different theme APIs, so a textual
cherry-pick alone is insufficient evidence of compatibility.

Do not include payload slimming, native performance optimization, broad XML/theme
rewrites, a new Astro plugin, automatic icon fetching, or new generic resource
configuration. Preserve the unrelated Flutter work in the root checkout.
