# Root Parity Residual Catalog

Baseline: Mermaid `11.16.1@7ecca0cd7f1658ef74f4e7e91f925724ef403bbf`.

This document is the review evidence for
`fixtures/_verification/root-parity-residuals.json`. The catalog is verification-only: it cannot
change production rendering, and every entry is bound to the exact input and upstream SVG hashes.
It exists to keep browser-only root differences visible without restoring fixture-keyed production
overrides or broadening DOM normalization.

This catalog owns only root `style`, `viewBox`, `width`, and `height`. It cannot accept a semantic
edge-label mismatch. Label residuals use a separate key/text/full-signature contract documented in
`SEMANTIC_LABEL_PARITY.md`, and the aggregate comparator evaluates that gate before root residual
policy.

## Admission Method

The candidate was generated with the canonical typed render operation:

```powershell
cargo run --release -p xtask -- compare-all-svgs \
  --check-dom --dom-mode parity-root --dom-decimals 3 \
  --flowchart-text-measurer vendored \
  --write-root-residual-candidate
```

The 2026-07-26 candidate contained 1,634 observations across 25 family ids. For 1,633 entries,
normalized descendants pass ordinary `parity`; the only `structure` descendant profile is the
Ishikawa hand-drawn fixture. Candidate generation rejects every other mismatch, so the catalog
cannot absorb parser, semantic, DOM-order, label-wrapper, or compared descendant-geometry failures.

The current candidate contains 1,566 observations across 25 family ids. Fixture-scoped Node KaTeX
renders are excluded from exact root approval because their local MathML viewport is measured by
the host browser. Those fixtures still require successful rendered-output and browser-measurement
evidence, ordinary `parity` for the complete normalized descendant tree, and a fail-closed root
structure contract. Their positive finite viewport dimensions remain diagnostic output.

Each stored entry includes:

- family id and fixture stem;
- descendant comparison profile;
- SHA-256 of the Mermaid input and pinned upstream SVG;
- exact upstream and local root `style`, `viewBox`, `width`, and `height`; and
- one reviewed evidence id from the table below.

Verification fails closed when an artifact hash or root value changes, a catalog entry disappears,
a new mismatch appears, or descendants no longer satisfy their declared profile.

## Evidence Classes

| Evidence id | Kind | Entries | Families | Source-backed classification |
| --- | --- | ---: | --- | --- |
| `browser-root-bbox` | `browser-measurement` | 1,392 | Architecture 33; Class 207; ER 51; Event Modeling 1; Flowchart 545; GitGraph 209; Ishikawa 11; Kanban 1; Mindmap 95; Requirement 42; Sankey 3; State 177; Swimlane 2; Timeline 12; Treemap 3 | Pinned Mermaid derives the final root from `setupGraphViewbox()`, `setupViewPortForSVG()`, or a family-local SVG `getBBox()`. Those paths measure the rendered DOM, including text, strokes, transforms, fallback fonts, and the Chromium float lattice. Merman uses the same padding and sizing algorithms over deterministic emitted-content bounds. |
| `browser-derived-layout` | `browser-measurement` | 121 | Block 41; Journey 4; Pie 43; Railroad 6; Railroad ABNF 2; Railroad EBNF 3; Railroad PEG 2; Sequence 4; TreeView 16 | These families compute root values explicitly, but their upstream dimensions consume `getBBox()`, `getBoundingClientRect()`, or `getComputedTextLength()` earlier in layout. The root residual is therefore the propagated browser measurement, not a second root formula. |
| `c4-headless-layout` | `source-backed-layout-approximation` | 51 | C4 51 | C4 computes its root from `screenBounds` rather than final SVG `getBBox()`. This evidence class covers the headless profile where no browser screen fact is supplied: Merman ports the pinned `Bounds` and `drawInsideBoundary()` algorithms, but replaces browser-dependent text and screen facts with deterministic measurement and explicit container dimensions. Browser hosts can separately project `screen.availWidth` for exact C4 wrapping. The root-only remainder is retained as the bounded headless-layout approximation. |
| `external-resource-layout` | `browser-measurement` | 1 | Flowchart 1 | The fixture intentionally retains remote `<img>` references. Mermaid leaves their intrinsic dimensions to the embedding browser, so network availability and image-load completion can change the browser-owned layout and root viewport without changing the Mermaid source or serialized resource reference. |
| `ishikawa-roughjs` | `rough-js-implementation` | 1 | Ishikawa 1 | The hand-drawn fixture uses RoughJS geometry upstream. Descendants are intentionally compared with `structure`; the exact root bbox follows RoughJS path jitter and stroke bounds that Merman does not reproduce byte-for-byte. |

## Upstream Source Audit

The classification above was checked against the pinned Mermaid sources, not inferred from root
deltas:

- `packages/mermaid/src/setupGraphViewbox.js` reads `svgElem.node().getBBox()` and then applies
  padding and `useMaxWidth`.
- `packages/mermaid/src/rendering-util/setupViewPortForSVG.ts` reads `svg.node().getBBox()` before
  applying the same sizing contract.
- Architecture, Class, ER, Event Modeling, Flowchart, GitGraph, Kanban, Mindmap, Requirement,
  Sankey, State, Swimlane, Timeline, and Treemap call one of those helpers. Ishikawa performs the
  equivalent root `getBBox()` locally.
- Block `calculateBlockSizes()` reads node bboxes; Journey wraps with
  `getBoundingClientRect()`; Pie measures legend/title DOM width; Railroad measures temporary text
  with `getBBox()`; Sequence uses DOM bounds for actors, notes, messages, and activations; TreeView
  measures every row label with raw text `getBBox()`.
- C4 `c4Renderer.js` derives root dimensions from `screenBounds`; its separate headless container
  contract is documented in `docs/alignment/C4_LAYOUT_MINIMUM.md`.

## 2026-07-26 Candidate Review

The authoritative full candidate was regenerated after the attested Cypress/package-test import
refresh and the Class ELK adapter-DOM convergence. It contained no descendant mismatch outside the
existing evidence profiles. Review results:

- previous reviewed entries: `1,627`;
- fresh observed entries: `1,634`;
- shared fixture identities: `1,623`, comprising `1,532` unchanged observations, `51` upstream-SVG
  digest-only refreshes, and `40` reviewed-signature changes;
- entries removed because the Class root became exact: `4`;
- newly observed root-only entries: `11`;
- entries returned to candidate-only `unreviewed` status before source classification: `51`.

The `51` candidate-only entries all retained `parity` descendants. Source review classified `27`
as `browser-root-bbox`: Class `18`, Flowchart `2`, GitGraph `4`, Mindmap `1`, and State `2`. It
classified the remaining `24` as `browser-derived-layout`: Railroad `5`, Railroad ABNF `1`,
Railroad EBNF `2`, Railroad PEG `1`, and TreeView `15`. These are the same family-level source
paths already audited above; no new evidence kind or comparator normalization was introduced.

The changed inputs restore test-scoped Mermaid configuration and repair mojibake captured by the
corpus importer. The Class changes also replace the historical Dagre-shaped ELK SVG wrapper with
the pinned layout-adapter DOM. In every case the ordinary `parity` comparison still covers the
complete normalized descendant tree, so the catalog records only the remaining browser-owned root
measurement. Contract revision, schema version, and source-backed evidence rationales are
unchanged.

## 2026-07-31 Browser Math Portability Review

The exact CI recipe was repeated on macOS and Linux after Node KaTeX became a required fixture-level
renderer. Native MathML produced different root viewports across the two host browser and font
stacks even though the normalized descendants passed `parity`. Recording either platform's value
as the single approved local signature would make the catalog host-specific.

The comparison harness now treats only the root viewport of a fixture that actually invoked Node
KaTeX as diagnostic when `parity-root` is requested. It still requires successful browser evidence,
compares all normalized descendants with ordinary `parity`, requires the root `viewBox`, `max-width`,
width/height contract, origin, and non-numeric style to remain valid, and leaves every non-math
fixture on the exact fail-closed root catalog. Reports include exact and diagnostic-only fixture
counts plus every browser-math root delta. No upstream/local dimension-matching tolerance or
fixture-keyed production behavior was added.

This review removed the browser-probed Sequence math observation
`upstream_docs_math_sequence_002`, reducing `browser-derived-layout` from 122 to 121 and the catalog
from 1,634 to 1,633 entries. Five Mindmap signatures were also regenerated after their local root
values changed; their input and upstream SVG hashes were unchanged, their descendants retained
`parity`, and their existing `browser-root-bbox` classification still applies.

## 2026-08-04 Architecture Bounds Review

The full candidate was regenerated after the Architecture FCoSE adapter separated Cytoscape body,
label, compound-child, and final-element bounding-box phases. The source-backed correction removed
70 Architecture root residuals outright: those fixtures now match the pinned browser root at three
decimals without catalog assistance.

The remaining Architecture set contains 33 observations. Twenty-two signatures are unchanged and
11 local signatures moved closer to the pinned result. There are no new fixture identities and no
changes in any other family. Every remaining Architecture observation passes complete descendant
`parity`; only root `style` or `viewBox` differs. The 11 changed signatures therefore retain the
existing `browser-root-bbox` classification backed by the pinned Architecture
`setupGraphViewbox()` call and its SVG `getBBox()` dependency. The catalog decreases from 1,633 to
1,563 entries and Architecture decreases from 103 to 33 without changing schema, comparison
revision, normalization, or production rendering.

## 2026-08-07 Mermaid 11.16.1 Baseline Review

The full candidate was regenerated after moving the pinned source baseline from Mermaid 11.16.0
to 11.16.1. The candidate still contains 1,563 observations across 25 family ids and no descendant
profile changed. Twenty upstream SVG digest changes retained their existing root signatures and
evidence classes.

One Flowchart fixture,
`upstream_cypress_flowchart_v2_spec_4023_should_render_html_labels_with_images_and_or_text_correctly_042`,
changed its upstream root from `671.344 × 630` to `413.734 × 198`. The Mermaid release diff does
not change Flowchart image-label rendering. The fixture itself references the remote
`https://mermaid.js.org/mermaid-logo.svg`, and the regenerated SVG records the browser's unloaded
intrinsic image dimensions while retaining the same external references. This observation is now
classified separately as `external-resource-layout`; it is not generalized into a tolerance or a
production override. The catalog remains exact and hash-bound, so a future remote-load outcome
change requires another explicit review.

## 2026-08-08 Text Semantics and Metric Identity Review

The full candidate was regenerated after the vendored text-measurement identity was aligned with
Mermaid 11.16.1 and Flowchart sanitization stopped deleting literal comparison text. The candidate
contains no removed fixture identities, three new Flowchart root-only observations, and 12 changed
local root signatures: Architecture 2, Flowchart 5, Mindmap 4, and State 1.

For all 12 changed signatures, the Mermaid input and pinned upstream SVG hashes are unchanged. The
three new observations are likewise bound to existing pinned inputs and upstream SVGs. Every one
of the 15 reviewed observations passes complete descendant `parity`; only root `style` or
`viewBox` differs. The pinned renderers for Architecture, Flowchart, Mindmap, and State all derive
the final viewport through `setupGraphViewbox()`, `setupViewPortForSVG()`, or the equivalent SVG
`getBBox()` path already audited above, so all 15 observations retain the
`browser-root-bbox` classification.

The catalog therefore increases from 1,563 to 1,566 observations, with Flowchart increasing from
543 to 546 total root observations. No schema, comparison revision, descendant normalization, or
production rendering override changed. The reviewed candidate was accepted with SHA-256
`242f60ef17145489803de85f63f1fac72927d226fc620de388f068bd16c8fc90`.

## 2026-08-08 Reference CLI Provenance Review

The full candidate was regenerated after the reference CLI workspace lock changed and the required
fresh-render provenance workflow refreshed every upstream SVG family. It still contains 1,566
observations across 25 family ids. Twenty Block and State entries changed only their upstream SVG
digests: their inputs, root signatures, descendant profiles, and reviewed evidence classes are
unchanged.

The external-image Flowchart fixture changed its upstream root from `413.734 × 198` back to
`671.344 × 630` because the remote Mermaid logo now loaded with square intrinsic dimensions. Its
input is unchanged, its local root remains `671.359 × 630`, and its complete descendants still pass
`parity`. The observation therefore retains the existing `external-resource-layout` classification;
no tolerance, normalization, or production override was added. The reviewed candidate was accepted
with SHA-256 `46e8c8652393462f8fc7974998ac2a1bbd7bb474e6f51fda3efaa59d64fcc045`.

## Production Boundary

No entry is a runtime override. The correct closure paths remain:

1. fix parser, model, layout, DOM, or root-algorithm defects when pinned source explains them;
2. improve general vendored font/DOM-shape facts when they apply to unseen text;
3. use the existing host measurement operations when exact installed-font geometry is required; or
4. retain an exact, hash-bound verification residual when the remaining fact is browser-only.

Fixture ids, complete labels, blanket float corrections, family-local hidden measurers, and broad
comparator normalization remain prohibited by ADR-0057 and ADR-0062.
