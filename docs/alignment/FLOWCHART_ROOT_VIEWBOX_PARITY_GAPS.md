# Flowchart Root SVG Parity Gaps (Mermaid@11.12.2)

This note tracks current gaps when comparing Flowchart Stage-B output against upstream Mermaid
SVG baselines **including** root `<svg>` viewport attributes (`viewBox`, `style="max-width: ..."`).

## Why This Exists

`merman`'s default Flowchart SVG parity checks focus on DOM structure (`--dom-mode parity`) and
intentionally ignore the root `<svg>` `viewBox` and `style` attributes while the layout and text
measurement subsystems are still converging.

For "full SVG DOM" parity work (closer to SVG XML parity), use `parity-root` mode.
In `xtask` DOM comparison, `parity-root` behaves like `parity` for geometry/noise masking, but it
also compares the root `<svg>` viewport attributes (`viewBox`, `style`).

## How To Run

- Generate a report and include root viewport deltas (does not fail unless `--check-dom` is set):
  - `cargo run -p xtask -- compare-flowchart-svgs --dom-mode parity-root --dom-decimals 3 --report-root`

- Note: if you omit `--check-dom`, `xtask` does **not** assert DOM parity; it only generates local
  SVGs and (in `parity-root`) reports the root viewport deltas.

- Generate a report **and** fail on full DOM parity-root mismatches:
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root`

When iterating specifically on viewport deltas, prefer the vendored Flowchart font metrics:

- `cargo run -p xtask -- compare-flowchart-svgs --dom-mode parity-root --dom-decimals 3 --report-root --text-measurer vendored`

The report is written to:

- `target/compare/flowchart_report.md`

## Debugging Root `.root` Transforms (Recursive Clusters)

When `parity-root` fails, it can be useful to inspect Mermaid's nested `<g class="root" transform="translate(...)">`
placements for extracted clusters (recursive flowchart subgraphs). These transforms strongly influence cluster and node
positions, and mismatches can manifest as large `max-width`/`viewBox` deltas.

After generating a local SVG under `target/compare/flowchart/<fixture>.svg`, run:

- `cargo run -p xtask -- debug-flowchart-svg-roots --fixture upstream_flowchart_v2_self_loops_spec`
- `cargo run -p xtask -- debug-flowchart-svg-diff --fixture upstream_flowchart_v2_self_loops_spec --min-abs-delta 0.5 --max 80`

This prints:

- Root `<svg>` `viewBox` and `max-width` (from `style`)
- All `<g class="root" transform="translate(...)">` payloads
- Per-cluster rect geometry (`x/y/width/height`) and which `.root` group contains it

## Current Status

As of the current implementation:

- `merman-render` now computes Flowchart root `viewBox`/`max-width` using a headless approximation of
  Mermaid's `setupViewPortForSVG` behavior (including the diagram title bounding box).
- Flowchart-v2 self-loop helper nodes (`*-*-*-1/2`) are now sized at `0.1×0.1` for layout parity,
  matching Mermaid's `insertNode(...)` + `updateNodeBounds(...)` behavior for empty `labelRect` nodes.
- Flowchart title X-extents are measured via `TextMeasurer::measure_svg_text_bbox_x`, allowing
  `VendoredFontMetricsTextMeasurer` to use pinned `svg_overrides` where available.
- `VendoredFontMetricsTextMeasurer` quantizes override-derived SVG bbox extents to a 1/1024px grid
  to reduce FP drift in root viewport strings (especially for wide titles).
- Flowchart-v2 stadium nodes (and other rough-path-based shapes) can have `node.width/height` used
  for Dagre layout derived from the rendered rough path bbox (`updateNodeBounds(getBBox)`), which
  can be narrower than the theoretical `(text bbox + padding)` sizing formula.
- With `--text-measurer vendored`, `xtask compare-flowchart-svgs --check-dom --dom-mode parity-root`
  is expected to pass for the current upstream fixture set (at `--dom-decimals 3`).

## Next Steps (Expected)

- Keep `parity-root` in CI to prevent regressions in root viewport sizing (`viewBox` / `max-width`).
- When moving toward stricter SVG parity, prefer adding fixture-driven tests for any remaining
  non-determinism in layout and text measurement before tightening DOM masking.

## Biggest Current Deltas

When `parity-root` is passing, this section should be empty. If it regresses, regenerate a report:

- `cargo run -p xtask -- compare-flowchart-svgs --dom-mode parity-root --dom-decimals 3 --report-root --text-measurer vendored`

and use the largest deltas in `target/compare/flowchart_report.md` to prioritize investigation.

## Investigation Notes

- Mermaid flowchart-v2 initializes Dagre graphs with `marginx=8, marginy=8` (both in the top-level
  graph and in extracted cluster graphs). Ensuring `merman-render` uses those margins is a
  prerequisite for meaningful root viewport comparisons.
- Mermaid uses dagrejs defaults for graph spacing (notably `edgesep=20`). Keeping `dugong`'s
  `GraphLabel` defaults aligned with dagrejs is important for multiedge routing and therefore
  root `viewBox/max-width` parity.
- Empty subgraphs (`subgraph B ... end` with no members in the semantic model) are rendered as
  regular nodes in Mermaid. Treating them as clusters can distort recursive root transforms and
  root viewport sizes; `merman-render` now sizes/layouts empty subgraphs as leaf nodes, which
  restores the expected extracted root translate-y (e.g. `0, 90` in `outgoing_links_4`).
- The previously large delta for `upstream_flowchart_v2_arrows_graph_direction_lt_spec` was caused
  by Mermaid's DOMPurify-style sanitization turning a stray `<` in the title into the literal text
  `&lt;` (which then affects title width and therefore `viewBox/max-width`). `merman-core` now
  matches this behavior more closely by escaping "stray" `<` tokens before running the
  DOMPurify-like HTML rewrite.
