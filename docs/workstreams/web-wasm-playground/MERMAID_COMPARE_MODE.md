# Mermaid Compare Mode

Status: First side-by-side slice implemented
Last updated: 2026-06-01

## Purpose

The playground should eventually support an optional comparison mode that renders the same Mermaid
source through both Merman WASM and Mermaid JS. This is more useful than comparing the native CLI
against Mermaid JS because the playground question is browser-specific: users need to know whether
the web renderer is compatible, visually close enough, and fast enough in the same runtime.

## Design Goals

- Keep the default editor fast: do not load Mermaid JS on the initial page load.
- Make visual parity easy to inspect without turning the main editor into a debugging tool.
- Compare SVG first; generate PNG from either SVG through the same browser export path.
- Record render time and errors for both engines, but avoid presenting this as a rigorous benchmark
  unless a dedicated measurement loop is running.
- Keep the comparison mode useful on both desktop and narrow screens.

## Non-Goals

- Native CLI vs Mermaid JS benchmarking in the playground.
- Pixel-perfect diffing in the first slice.
- Loading every Mermaid JS dependency before the user asks for comparison.
- Replacing the existing Merman-first live editor flow.

## UI Options Considered

### Option A: Engine Selector

Add a segmented control in the preview tab: `Merman | Mermaid | ASCII`.

This is the smallest UI, but it is weak for comparison because users must remember what changed
between engine switches. It is still useful as a fallback on very small screens.

### Option B: Side-by-Side Compare

Add a `Compare` tab next to `SVG` and `ASCII`. The compare tab renders two panes:

- left: Merman
- right: Mermaid JS

Each pane has a compact header with engine name, version, render time, status, and export/copy
actions. Pan and zoom should be linked by default, with a toggle to unlink them when inspecting
large layout differences.

This should be the first implementation because it directly answers the parity question and fits
the current resizable editor/preview layout.

### Option C: Overlay and Difference Inspector

Render both SVGs into a shared viewport with an opacity slider, swipe handle, or generated PNG
pixel diff.

This is powerful for deep parity work, but it adds more complexity: SVG sizes need normalization,
text rendering differences can create noisy diffs, and PNG conversion can fail on SVG features such
as `foreignObject`. This should be a later inspector mode, not the first comparison surface.

## Recommended UX

The preview tab bar should become:

```text
SVG | ASCII | Compare
```

`SVG` remains the default Merman preview. `ASCII` stays available only for supported diagrams.
`Compare` is optional and lazy-loads Mermaid JS the first time it is opened.

Inside `Compare`, use:

```text
[Side by side] [Overlay] [Source]

┌ Merman 0.7.0  12.4ms  OK ───────────────┐ ┌ Mermaid 11.15.0  38.1ms  OK ──────────┐
│                                          │ │                                          │
│                 SVG viewport             │ │                 SVG viewport             │
│                                          │ │                                          │
└ Export SVG  Export PNG  Copy SVG ───────┘ └ Export SVG  Export PNG  Copy SVG ───────┘
```

The first slice only needs the `Side by side` view. `Overlay` and `Source` can be disabled or
hidden until implemented.

For narrow screens, stack the panes vertically and keep the same linked zoom state.

## Loading Model

Mermaid JS should be a dynamic import:

```ts
const mermaid = await import("mermaid");
```

Initialization should happen once per page session. Use the same effective theme and security
configuration as the upstream parity tools where possible. The package version should be pinned to
the same Mermaid baseline used by the repository, currently `mermaid@11.16.0`.

The first time the user opens `Compare`, show a loading state:

```text
Loading Mermaid JS for comparison...
```

After loading, re-render automatically when source code or theme changes.

## Data Model

Use one artifact shape for both engines:

```ts
type RenderEngine = "merman" | "mermaid";

interface RenderArtifact {
  engine: RenderEngine;
  engineVersion: string;
  svg: string | null;
  error: string | null;
  renderTimeMs: number | null;
}
```

The Merman artifact is produced by the existing `useMerman()` path. The Mermaid artifact should
come from a new `playground/src/lib/mermaid-renderer.ts` module that hides dynamic import,
initialization, IDs, Mermaid config, and error normalization.

## Export Behavior

The existing SVG and PNG export helpers can be reused for either engine:

- `Export Merman SVG`
- `Export Merman PNG`
- `Export Mermaid SVG`
- `Export Mermaid PNG`

PNG should be generated from the displayed SVG, not from a separate renderer. This keeps export
behavior consistent and makes visual comparison easier to reason about.

## Benchmark Relationship

The compare UI should show render times as interactive feedback only. A real browser benchmark
should still use a separate warmup/measurement loop in one Chromium session, measuring repeated
Merman `renderSvg()` calls against repeated Mermaid `mermaid.render()` calls on the same fixtures.

The compare UI can later expose a `Run sample` action that runs a small in-browser benchmark for
the current diagram, but that should be clearly labeled as local and approximate.

## Implementation Slices

1. Done: Add `mermaid@11.15.0` to the playground and implement a lazy `mermaid-renderer.ts` wrapper.
2. Done: Extract the current pan/zoom SVG preview into a reusable `SvgViewport` component.
3. Done: Add the `Compare` tab with side-by-side Merman and Mermaid artifacts.
4. Done: Add per-pane SVG/PNG export and copy actions.
5. Deferred: Add optional overlay/source/diff tools after the side-by-side path is proven useful.

## Open Questions

- Should the URL share state include the selected preview mode, or should shared links always open
  in the normal Merman SVG preview?
- Should Mermaid render errors appear beside Merman render errors, or should a Merman error keep the
  current full-panel error treatment in normal SVG mode?
- Should we expose a user-visible Mermaid version selector later, or keep the baseline fixed for
  parity with repository tests?
