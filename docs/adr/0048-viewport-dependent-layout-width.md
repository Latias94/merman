# 0048: Host-Container-Dependent Layout Size (Headless)

Date: 2026-01-19

Amended: 2026-08-02

## Status

Accepted

## Context

Some Mermaid diagrams derive layout geometry from browser/DOM state rather than purely from the
diagram definition and Mermaid config. Gantt reads its SVG parent element's `offsetWidth`; C4
uses `screen.availWidth` as the root `widthLimit` for row wrapping in `c4Renderer.js`.

In `merman`, the core goal is Mermaid parity while remaining **headless** and usable by multiple
UI frameworks. This requires a deterministic, explicit replacement for DOM-derived available
space without confusing the browser page viewport with the element that owns layout.

## Decision

- `merman-render` exposes the host's **available layout-container size** through
  `LayoutOptions.container_width` / `LayoutOptions.container_height` and the browser's distinct
  `screen.availWidth` value through optional `LayoutOptions.screen_available_width`.
- The headless defaults are `container_width = 800` and `container_height = 600` CSS pixels.
- Gantt uses the available container width unless `gantt.useWidth` is explicitly configured; the
  explicit Mermaid config remains authoritative.
- C4 uses `screen_available_width` when a browser host supplies it, matching Mermaid's actual
  `screen.availWidth` dependency. Headless hosts fall back to the available container width.
- A browser page viewport is not a `LayoutOptions` value. Verification adapters that render in a
  browser must resolve their renderer-specific page geometry into a container size before invoking
  the production operation. For example, mmdc's 1200px page and the default 8px body margins yield
  a Gantt parent `offsetWidth` of 1184px.
- Production defaults must not encode a verification runner's page viewport or body margins.

## Consequences

- Layout snapshots become deterministic and reproducible across environments.
- Upstream SVG baselines generated via Mermaid CLI can be compared meaningfully by projecting the
  baseline renderer's page geometry into an explicit container profile.
- Consumers embedding `merman` can pass the actual target container size without reproducing a
  browser viewport model. Browser hosts can additionally project the screen availability that C4
  reads upstream instead of conflating it with the owning element's width.
- The serialized request contract is intentionally breaking: the old `viewport_width` and
  `viewport_height` keys are rejected rather than retained as compatibility aliases.
