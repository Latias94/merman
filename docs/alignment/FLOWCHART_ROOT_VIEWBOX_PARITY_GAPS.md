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

At the time of writing:

- `merman-render` now computes Flowchart root `viewBox`/`max-width` using a headless approximation of
  Mermaid's `setupViewPortForSVG` behavior (including the diagram title bounding box).
- Flowchart-v2 self-loop helper nodes (`*-*-*-1/2`) are now sized at `0.1×0.1` for layout parity,
  matching Mermaid's `insertNode(...)` + `updateNodeBounds(...)` behavior for empty `labelRect` nodes.
- Flowchart title X-extents are measured via `TextMeasurer::measure_svg_text_bbox_x`, allowing
  `VendoredFontMetricsTextMeasurer` to use pinned `svg_overrides` where available.
- `VendoredFontMetricsTextMeasurer` quantizes override-derived SVG bbox extents to a 1/1024px grid
  to reduce FP drift in root viewport strings (especially for wide titles).
- `--dom-mode parity-root` is still expected to fail for many Flowchart fixtures, primarily because
  Flowchart root viewport values are font-metric-derived in Mermaid (DOM `getBBox()`), and our
  headless text measurement currently does not reproduce browser font fallback and sub-pixel
  rounding.
- The `--report-root` output helps quantify which fixtures have the largest viewport deltas so we
  can iteratively close the gap.

## Next Steps (Expected)

- Improve `TextMeasurer` fidelity for Flowchart title and label text (font-family aware metrics), or
  introduce additional pinned, upstream-derived font metric vendoring where it blocks `parity-root`
  checks.
- Prefer deriving Flowchart title `svg_overrides` from upstream SVG fixtures (when the title is the
  limiting bbox contributor) so the generated metric table does not depend on local font/rendering
  differences.

## Biggest Current Deltas

From `target/compare/flowchart_report.md` (generated via `--report-root --dom-mode parity-root --dom-decimals 3`):

| Fixture | upstream max-width(px) | local max-width(px) | Δ |
|---|---:|---:|---:|
| `upstream_singlenode_shapes_spec` | 1557.230 | 1567.690 | +10.460 |
| `upstream_flow_text_special_chars_spec` | 1394.730 | 1404.190 | +9.460 |
| `upstream_flow_style_style_preserves_labels_spec` | 364.234 | 365.234 | +1.000 |
| `upstream_flowchart_v2_escaped_without_html_labels_spec` | 353.422 | 354.010 | +0.588 |

These drive most of the remaining `--check-dom --dom-mode parity-root` failures.

## Investigation Notes

- Mermaid flowchart-v2 initializes Dagre graphs with `marginx=8, marginy=8` (both in the top-level
  graph and in extracted cluster graphs). Ensuring `merman-render` uses those margins is a
  prerequisite for meaningful root viewport comparisons.
- Mermaid uses dagrejs defaults for graph spacing (notably `edgesep=20`). Keeping `dugong`'s
  `GraphLabel` defaults aligned with dagrejs is important for multiedge routing and therefore
  root `viewBox/max-width` parity.
- Mermaid's extracted cluster roots (`<g class="root" transform="translate(...)"`) can include an
  additional deterministic y-offset when an empty sibling subgraph is present (e.g.
  `outgoing_links_4` in `flowchart-v2.spec.js`). `merman-render` accounts for this by adjusting the
  root translate-y and expanding the root viewport bbox accordingly.
- The previously large delta for `upstream_flowchart_v2_arrows_graph_direction_lt_spec` was caused
  by Mermaid's DOMPurify-style sanitization turning a stray `<` in the title into the literal text
  `&lt;` (which then affects title width and therefore `viewBox/max-width`). `merman-core` now
  matches this behavior more closely by escaping "stray" `<` tokens before running the
  DOMPurify-like HTML rewrite.
