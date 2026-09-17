# Public Cyberpunk representative scenes

These three fixed inputs establish the first bounded sample for the theme product
boundary investigation. They are not a strategy for the whole preset catalog, a
claim that Cyberpunk suits every diagram family, or qualification of the other
23 Modern Mermaid themes. Suitability and terminal coverage must be assessed for
each theme/family pairing. Merely accepting an input or selecting a preset is not
visual qualification.

## Provenance and scope

The source recipe is `cyberpunk` in Modern Mermaid commit
`a021cbce37fc0b07a9f4791c28e983101ea06f2d`, `src/utils/themes.ts:209–394`.
`MERMAID_EXAMPLES.md` at that same revision supplies examples, not normative
coverage of every selector. The reference checkout is outside this crate; no
reference CSS or browser framework is shipped in these fixtures.

| Input | Origin and deliberate changes | Visible terminals |
| --- | --- | --- |
| `flowchart.mmd` | Derived from “E-commerce Purchase Flow (English)”, `MERMAID_EXAMPLES.md:38–54`. Retains browsing, stock decision, Yes/No branches and cart; shortens the failure label and adds one Checkout cluster. It is not a verbatim upstream example. | Three rectangular nodes, one decision polygon, three directed edges, two edge labels, node labels and one cluster/title. |
| `sequence.mmd` | Derived from “API Authentication (English)”, `MERMAID_EXAMPLES.md:114–146`. Reduces to Client/API, one request/response and one note; adds an activation and a single loop to expose explicitly declared recipe selectors. It is not a verbatim upstream example. | Participant boxes and labels, lifelines, solid and dashed messages, arrowheads, message labels, one note, activation and loop frame/label. |
| `xychart.mmd` | New bounded scene. The pinned `MERMAID_EXAMPLES.md` contains no XY Chart example. | Title, categorical x-axis with title/ticks, numeric y-axis with title/ticks, two bar series and two line series, interleaved in declaration order. |

The scenes contain no initialization directives, source paint overrides or
embedded fonts. Public-product checks must compile the public Cyberpunk preset
through `compile_preset`; complete-spec export/import must reproduce the same
recipe in a fresh consumer. Hand-built mechanism fixtures are separate evidence.

## Reading the source expectations

The tables below record what the pinned recipe **declares**. Selector match count,
matched SVG element, computed paint and visible output must be recorded by the
capture owner before treating a declaration as an observed upstream effect.
An unmatched selector is evidence of a recipe/DOM mismatch, not permission to
silently erase the intended effect or claim that the reference displayed it.
Mapping that intent to a typed family recipe is a separate product decision.

The base recipe declares navy `#051423` for background, primary/secondary/tertiary
fills, `mainBkg`, cluster fill and edge-label background; cyan `#00f2ff` for primary
text, primary/node/cluster borders and lines. Its nominal font is
`"Inter", "Noto Sans SC", system-ui, sans-serif` at 16px with `darkMode: true`.
Unspecified family-specific text/marker colors must be measured from the resolved
Mermaid configuration; the base variables alone do not prove their final paint.

Shadow tuples below are `(offset-x, offset-y, blur, color, alpha)` in CSS pixels.
Ordered pairs retain CSS composition order; two stages are not interchangeable
with one stronger blur. Color alpha is distinct from whole-element opacity.

### Flowchart

| Terminal / source selector | Declared visible expectation | Applicability to verify |
| --- | --- | --- |
| Node geometry, `.node rect, .node circle, .node polygon, .node path` | Navy fill, cyan 3px stroke; `rx: 10px`, `ry: 10px`; ordered cyan shadows `(0,0,8,.5)` then `(0,0,16,.3)`. | Check every actual shape. Rectangular nodes can expose corners; `rx`/`ry` do not turn a polygon or arbitrary path into a rounded shape. The decision must retain its geometry. |
| Node labels, `.label` | Cyan color, weight 600, cyan text shadow `(0,0,10,.5)`. | SVG text and HTML/foreignObject labels may receive different inherited properties. Confirm readable glyph fill, weight and visible glow. |
| Edges, `.edgePath .path` | Cyan 2px stroke and cyan shadow `(0,0,6,.6)`. | Record whether current Mermaid edges actually have this ancestry/class combination. A differently named edge is not a match. |
| Arrowheads, `.arrowheadPath` | Cyan fill and stroke. | Inspect the actual marker geometry/classes; the selector may not name the rendered marker. Visible arrowheads remain required for a usable scene. No marker-specific glow is declared here. |
| Yes/No labels, `.edgeLabel` | Navy background, cyan color, weight 600 and cyan text shadow `(0,0,10,.5)`. | Check both labels and the background terminal; a CSS background on a wrapper does not prove a painted SVG rectangle. |
| Cluster and Checkout title | `clusterBkg: #051423`, `clusterBorder: #00f2ff`; ordinary resolved label styling. | The recipe does not explicitly assign the cluster 3px stroke, 10px corners or dual shape shadows. Measure inherited/resolved text styling. |

### Sequence

| Terminal / source selector | Declared visible expectation | Applicability to verify |
| --- | --- | --- |
| Actor/participant geometry, `.actor` | Navy fill, cyan 3px stroke, 10px `rx`/`ry`; ordered cyan shadows `(0,0,8,.5)` then `(0,0,16,.3)`. | Inspect each matched element type, including repeated participant boxes if emitted. `.actor` may also match text; that would apply geometry-oriented paint/effects to the text itself. |
| Actor labels, `.actor text` | Cyan fill, weight 600, cyan text shadow `(0,0,10,.5)`. | This is a descendant selector. A standalone `text.actor` does not match it. Do not claim label cyan/weight/glow from this rule without a match; inspect competing `.actor` styling. |
| Lifelines, `.actor-line` | Cyan 2px stroke, cyan shadow `(0,0,6,.6)`. | Check both lifelines and outward shadow extent. |
| Activation, `.activation0, .activation1, .activation2` | Cyan fill alpha .1, cyan 3px stroke. | No activation shadow or 10px radius is explicitly declared. |
| Solid/dashed messages, `.messageLine0, .messageLine1` | Cyan 2px stroke, cyan shadow `(0,0,6,.6)`. | Preserve the response dash pattern. Inspect arrowheads and message text separately; this rule does not specify their styling. |
| Note geometry, `.note` | Navy fill, magenta `#ff00ff` 2px stroke; 10px `rx`/`ry`; magenta shadow `(0,0,8,.4)`. | If the note is a polygon/path, `rx`/`ry` cannot establish rounded corners. Record the actual shape instead of asserting a visible 10px radius. |
| Note label, `.noteText` | Magenta fill, weight 600, magenta text shadow `(0,0,8,.4)`. | Check actual text and any child spans. |
| Loop label box, `.labelBox` | Navy fill, cyan 2px stroke, 10px `rx`/`ry`, cyan shadow `(0,0,6,.4)`. | Radius applies only where the actual geometry supports it. |
| Loop labels, `.labelText, .loopText` | Cyan fill, weight 600, cyan text shadow `(0,0,10,.5)`. | Keep both the loop keyword and “Each request” readable. |
| Loop frame, `.loopLine` | Cyan 2px stroke and cyan shadow `(0,0,4,.5)`. | Verify all frame segments without treating them as message lines. |

### XY Chart

| Selector suffix | Line recipe: `.line-plot-N path` | Bar recipe: `.bar-plot-N rect` |
| --- | --- | --- |
| `0` | `#6CC6CB` stroke, 3px, shadow `(0,0,6,#6CC6CB,.5)` | `#6CC6CB` fill alpha .2, same-color 2px stroke, shadow `(0,0,8,#6CC6CB,.4)` |
| `1` | `#C77DFF` stroke, 3px, shadow `(0,0,6,#C77DFF,.5)` | `#C77DFF` fill alpha .2, same-color 2px stroke, shadow `(0,0,8,#C77DFF,.4)` |
| `2` | `#7CE38B` stroke, 3px, shadow `(0,0,6,#7CE38B,.5)` | `#7CE38B` fill alpha .2, same-color 2px stroke, shadow `(0,0,8,#7CE38B,.4)` |

The source does not assign a line opacity or a whole-bar opacity in these rules.
Do not apply the bar fill alpha .2 to its opaque stroke or its shadow a second
time. It also does not define a selector for suffix `3`.

Declaration order is **bar, line, bar, line**. If the renderer numbers all series
globally, classes would select bar-0 teal, line-1 purple and bar-2 green, while
line-3 receives no explicit Cyberpunk series rule. If it numbers independently by
kind, both first series select suffix 0 and both second series select suffix 1.
These are diagnostic alternatives, not an assertion about the captured DOM.
The capture must record all four actual classes and computed paints. Neither a
series-local semantic mapping nor a global ordinal mapping can be inferred solely
from these CSS selectors. The chosen public behavior must be stated explicitly
before its visual expectation is promoted.

| Other terminal / selector | Declared expectation | Applicability to verify |
| --- | --- | --- |
| Tick paths, `.ticks path` | Cyan stroke, whole-element opacity .3. | This does not declare all axis lines or tick-label text cyan. |
| Chart title, `.chart-title text` | Cyan fill, weight 700, 18px, cyan text shadow `(0,0,15,.8)`. | Confirm title text matches and remains visible. |
| Axis titles, `.left-axis .title text, .bottom-axis .title text` | Cyan fill, 13px, cyan text shadow `(0,0,10,.6)`. | Verify both Window and Requests titles. |
| Legend text, `.legend text` | Cyan fill, 12px, cyan text shadow `(0,0,8,.5)`. | This scene does not request a legend. Zero matches do not create a missing-terminal requirement. |

### Complete preview background

`themes.ts:384–394` declares a navy root background and these layers, in CSS list
order:

1. A horizontal 1px cyan grid line at alpha .03, transparent elsewhere, repeated
   every 40px in a 40px by 40px tile.
2. A 90-degree linear gradient with the same line/alpha/tile, producing the
   perpendicular grid direction.
3. A full-canvas radial gradient centered at 50%/50%, cyan alpha .05 at its center
   and transparent at 70%.

`backgroundBlendMode: screen` applies to the composition. `Preview.tsx:75–76`
selects the recipe's background class/style unless a background override is
chosen. The background belongs to the preview/container composition; Mermaid's
plain SVG is not proof that these layers were exported. The bounded public
product expects this complete composition at the root canvas. The grid is anchored
to the canvas's top-left corner, independent of a nonzero diagram viewBox origin.
At natural output dimensions (native scale 1, no fit or size reduction), the tile
is 40 output pixels and the line is 1 pixel. Resizing the entire exported SVG or
raster scales its canvas and diagram together. The exported recipe does not promise
a separate screen-fixed CSS background when a host scales the diagram; that would
be a different, host-owned composition from a single self-contained image.

## Capture and target contract

Use a fixed 1280 by 960 CSS-pixel browser viewport, device pixel ratio 1 and 100%
zoom. Capture the composed canvas with enough padding for outward glow; record
the actual canvas bounds and diagram scale. Use a controlled caller/test font,
wait for it to finish loading and record its family, file/version and content
identity in the observation report. Keep the same font bytes available to the
native target. The source's fallback list is not proof that Inter loaded. These
fixtures neither bundle fonts nor require production font embedding.

Record exact Mermaid version, reference commit, Merman revision/build profile,
browser version, native rasterizer/PDF renderer versions, output dimensions and
font provenance with the captures. The historical Mermaid 11.12.1 reference audit
and Merman's 11.17.2 baseline must not be described as same-version parity.

| Target | Required observations before qualification |
| --- | --- |
| SVG in a browser | Actual selector matches and computed styles; visible labels, paths and markers; ordered glow without clipping; applicable radii; composed root background. Save both raw SVG and composed browser capture, since container paint is not part of raw SVG by default. |
| Native PNG | Actual exported pixels retain the applicable paints, widths, text, alpha, ordered glow and full background. Record scale and examine stroke cores plus outward blur bands; a valid PNG with missing effects is incomplete. |
| Native PDF | Inspect a rasterization from the actual exported PDF for the same terminals and composition. Record PDF page/scale and rasterizer. Successful PDF creation or filter admission does not establish visible output. |

Compare named terminal regions, not whole-image equality across layout engines.
Exact recipe colors, stage order, declared widths/alphas and matched text weights
are structural/computed-style expectations. Font metrics, antialiasing, blur color
space and layout differences require target-specific measured tolerances recorded
before acceptance changes. A tolerance must not admit a blank label, absent
marker, missing shadow stage, omitted background layer or clipped glow. Until
those measurements and DOM matches are recorded, the corresponding target or
facet is **unverified**. This README fixes source facts and inputs; it does not
certify any current preset or invent pixel-tolerance thresholds.
