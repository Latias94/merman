# Root Parity Residual Catalog

Baseline: Mermaid `11.16.0@7c0cafcf42e76bfaf79d0cbbd12edb986612f014`.

This document is the review evidence for
`fixtures/_verification/root-parity-residuals.json`. The catalog is verification-only: it cannot
change production rendering, and every entry is bound to the exact input and upstream SVG hashes.
It exists to keep browser-only root differences visible without restoring fixture-keyed production
overrides or broadening DOM normalization.

## Admission Method

The candidate was generated with the canonical typed render operation:

```sh
cargo run --release -p xtask -- compare-all-svgs \
  --check-dom --dom-mode parity-root --dom-decimals 3 \
  --flowchart-text-measurer vendored \
  --write-root-residual-candidate
```

The 2026-07-26 candidate contains 1,634 observations across 25 family ids. For 1,633 entries,
normalized descendants pass ordinary `parity`; the only `structure` descendant profile is the
Ishikawa hand-drawn fixture. Candidate generation rejects every other mismatch, so the catalog
cannot absorb parser, semantic, DOM-order, label-wrapper, or compared descendant-geometry failures.

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
| `browser-root-bbox` | `browser-measurement` | 1,460 | Architecture 103; Class 207; ER 51; Event Modeling 1; Flowchart 543; GitGraph 209; Ishikawa 11; Kanban 1; Mindmap 95; Requirement 42; Sankey 3; State 177; Swimlane 2; Timeline 12; Treemap 3 | Pinned Mermaid derives the final root from `setupGraphViewbox()`, `setupViewPortForSVG()`, or a family-local SVG `getBBox()`. Those paths measure the rendered DOM, including text, strokes, transforms, fallback fonts, and the Chromium float lattice. Merman uses the same padding and sizing algorithms over deterministic emitted-content bounds. |
| `browser-derived-layout` | `browser-measurement` | 122 | Block 41; Journey 4; Pie 43; Railroad 6; Railroad ABNF 2; Railroad EBNF 3; Railroad PEG 2; Sequence 5; TreeView 16 | These families compute root values explicitly, but their upstream dimensions consume `getBBox()`, `getBoundingClientRect()`, or `getComputedTextLength()` earlier in layout. The root residual is therefore the propagated browser measurement, not a second root formula. |
| `c4-headless-layout` | `source-backed-layout-approximation` | 51 | C4 51 | C4 computes its root from `screenBounds` rather than final SVG `getBBox()`. Merman ports the pinned `Bounds` and `drawInsideBoundary()` algorithms, but replaces browser/screen-dependent text and container facts with the operation's deterministic measurement and explicit container dimensions. The root-only remainder is retained as the bounded headless-layout approximation. |
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

## Production Boundary

No entry is a runtime override. The correct closure paths remain:

1. fix parser, model, layout, DOM, or root-algorithm defects when pinned source explains them;
2. improve general vendored font/DOM-shape facts when they apply to unseen text;
3. use the existing host measurement operations when exact installed-font geometry is required; or
4. retain an exact, hash-bound verification residual when the remaining fact is browser-only.

Fixture ids, complete labels, blanket float corrections, family-local hidden measurers, and broad
comparator normalization remain prohibited by ADR-0057 and ADR-0062.
