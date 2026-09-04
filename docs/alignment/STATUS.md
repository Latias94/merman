# Alignment Status

This document is the human-readable parity dashboard. It records current support and verification
boundaries; it is not a progress log. Historical implementation notes belong in workstream,
planning, or coverage documents. See the [alignment authority map](README.md) for the machine and
prose ownership boundary.

## Current Read

| Item | Current state |
| --- | --- |
| Dashboard | Active |
| Upstream baseline | Mermaid `@11.17.2` |
| Reference graph | Generated bundle verifies Mermaid and companion source, package, lock, and provenance evidence |
| Dashboard review | Authority links and lifecycle boundaries reviewed on 2026-08-31; exact gate state belongs to the revision that ran it |
| Admission | 35 families in the primary SVG matrix; `zenuml` has a separate external-renderer comparison lane |
| Root viewport | Every primary-matrix family has covered root-viewport evidence |
| Semantic edge labels | C4, Flowchart ELK, Architecture, Requirement, State, Class, and ER use fail-closed identity/geometry/presentation admission |
| Language catalog | All 35 built-in families are available independently of optional render backends |
| Editor facts | Strict facts use schema `2`, diagnostics use schema `1`, and all 35 families share one Tree-sitter grammar/query for tolerant syntax highlighting |
| Verification boundary | Capability projections and exact artifact recipes are checked in normal CI; a strict result belongs to the exact revision that ran it |

Admission describes available capability and required evidence. It does not certify that every
gate passes after the latest uncommitted changes.

## Authority Map

| Concern | Authority |
| --- | --- |
| Pinned Mermaid source | `tools/upstreams/REPOS.lock.json` |
| Family parser and typed-render capabilities | `crates/merman-core/src/family.rs` |
| Admission and fixture coverage policy | `crates/xtask/src/cmd/admission.rs` |
| Human admission overview | `docs/alignment/ADMISSION_INVENTORY.md` |
| Executable SVG compare facts | `crates/xtask/src/cmd/compare/diagrams.rs` |
| Parity and residual policy | `docs/workstreams/PARITY_BOUNDARY.md` |
| Semantic edge-label contract | `docs/alignment/SEMANTIC_LABEL_PARITY.md` |
| Human family scope and source evidence | `docs/alignment/*_MINIMUM.md` and `*_UPSTREAM_TEST_COVERAGE.md` |

If this dashboard disagrees with executable inventory or capability facts, the executable facts
win and this file must be corrected. Family prose is review guidance, not a machine input;
`check-alignment` does not parse its wording or require document pairs.

## Support Vocabulary

| Admission state | Contract |
| --- | --- |
| Primary SVG matrix | All required evidence layers and an executable compare fact are present. |
| External comparison lane | Full local behavior plus exact companion/browser evidence, kept separate from built-in upstream SVG baselines. |

Primary admission does not mean pixel identity with Chromium or support for every upstream branch.
It means the family has source-backed local behavior and participates in the repository's
structural/parity verification contract. Browser text measurement, `foreignObject`, RoughJS, and
other explicitly documented residuals remain bounded by the parity policy.

## Primary SVG Matrix

Every row below is semantic-, layout-, and SVG-covered in the admission inventory. `N` means a
normalized fixture corpus; `N+D` means normalized fixtures plus an explicitly deferred
investigation corpus. The default DOM mode is the family command's configured comparison boundary,
not a quality ranking. Callers may select another supported mode explicitly.

| Family | Corpus | Compare command | Default DOM mode |
| --- | ---: | --- | --- |
| `er` | N+D | `compare-er-svgs` | `parity` |
| `flowchart` | N+D | `compare-flowchart-svgs` | `parity` |
| `state` | N+D | `compare-state-svgs` | `structure` |
| `class` | N+D | `compare-class-svgs` | `parity` |
| `sequence` | N+D | `compare-sequence-svgs` | `structure` |
| `info` | N+D | `compare-info-svgs` | `parity` |
| `pie` | N+D | `compare-pie-svgs` | `structure` |
| `sankey` | N+D | `compare-sankey-svgs` | `parity-root` |
| `packet` | N+D | `compare-packet-svgs` | `structure` |
| `timeline` | N | `compare-timeline-svgs` | `structure` |
| `journey` | N | `compare-journey-svgs` | `parity` |
| `kanban` | N | `compare-kanban-svgs` | `structure` |
| `gitgraph` | N+D | `compare-gitgraph-svgs` | `parity` |
| `gantt` | N | `compare-gantt-svgs` | `structure` |
| `c4` | N+D | `compare-c4-svgs` | `parity` |
| `block` | N+D | `compare-block-svgs` | `structure` |
| `radar` | N+D | `compare-radar-svgs` | `parity` |
| `requirement` | N+D | `compare-requirement-svgs` | `parity` |
| `mindmap` | N | `compare-mindmap-svgs` | `parity` |
| `architecture` | N+D | `compare-architecture-svgs` | `parity` |
| `quadrantchart` | N | `compare-quadrantchart-svgs` | `parity` |
| `treemap` | N+D | `compare-treemap-svgs` | `parity` |
| `xychart` | N+D | `compare-xychart-svgs` | `parity` |
| `treeView` | N | `compare-tree-view-svgs` | `parity` |
| `ishikawa` | N+D | `compare-ishikawa-svgs` | `parity` |
| `eventmodeling` | N | `compare-eventmodeling-svgs` | `parity` |
| `error` | N | `compare-error-svgs` | `parity` |
| `venn` | N | `compare-venn-svgs` | `parity` |
| `swimlane` | N | `compare-swimlane-svgs` | `parity` |
| `railroad` | N | `compare-railroad-svgs` | `parity` |
| `railroadEbnf` | N | `compare-railroad-ebnf-svgs` | `parity` |
| `railroadAbnf` | N | `compare-railroad-abnf-svgs` | `parity` |
| `railroadPeg` | N | `compare-railroad-peg-svgs` | `parity` |
| `wardley` | N | `compare-wardley-svgs` | `parity` |
| `cynefin` | N | `compare-cynefin-svgs` | `parity` |

## Non-Primary Families

| Family | State | Current boundary |
| --- | --- | --- |
| `zenuml` | External comparison lane | Full grammar, semantic/editor model, typed layout, and headless SVG are implemented against the admitted ZenUML Core behavior source. The exact external plugin graph is tested in an opaque browser realm and publishes only strict-validated native SVG; it remains outside the built-in upstream-SVG matrix. |

LSP/editor support is tracked independently in the family capability registry. SVG admission must
not be used to infer completions, navigation, diagnostics, or source-span support for a family.

## Retained Mermaid 11.16 Source Scope

The pinned baseline includes families that were absent from older dashboards. All families below
are in the primary matrix, with these boundaries:

- `treeView-beta`: browser text metrics remain a family-documented residual.
- `ishikawa` / `ishikawa-beta`: classic SVG passes parity for 12 fixtures. Hand-drawn output is
  implemented and all 13 fixtures pass structure; JavaScript RoughJS versus Rust `roughr` path
  geometry remains a documented residual.
- `eventmodeling`: `entity`, `note`, and `gwt` remain in the single semantic source and editor
  facts, then the render projection omits them because Mermaid's DB/renderer does not consume them.
  Strict browser HTML behavior remains a documented residual.
- `venn-beta`: classic SVG retains parity comparison. Hand-drawn output is deterministic and three
  pinned Cypress fixtures cover the exact structure-only boundary; JavaScript RoughJS versus Rust
  `roughr` path geometry remains a documented residual.
- `swimlane-beta`: reuses Flowchart semantics but owns source-backed swimlane layout and routing.
- `railroad-*-beta`: four dialects share a typed model and renderer; browser font-height
  differences remain measurable residuals.
- `cynefin-beta`: deterministic headless text and path formatting replace browser-specific
  measurements.
- `wardley-beta`: ten source-backed Cypress fixtures cover typed semantics, editor facts, layout,
  SVG, theme, and root-viewport behavior.

The raw Mermaid `11.15.0..11.16.0` added-file corpus is preserved under
`fixtures/_upstream/mermaid-11.16.0/`:

- 122 upstream source paths;
- 121 unique contents;
- 122 verbatim managed `.mmd` files.

This corpus is evidence, not automatic fixture admission. Its leading underscore keeps raw inputs
out of ordinary snapshot sweeps. Family-specific fixtures and upstream SVG baselines remain the
promotion mechanism.

Five exact parser-only fixtures remain: one Flowchart parser case, two Sankey circular-link cases,
and two XYChart inputs without plot data. The selected Mermaid 11.17.2 runtime fails to render all five, so
`cargo run -p xtask -- audit-gaps --check-upstream-render` reports zero actionable parser-only
gaps. Exact family-scoped capability facts own these exclusions; filename patterns do not.

The 30 Mermaid 11.16 Swimlane DDLT inputs follow the same evidence boundary under
`fixtures/swimlane/_upstream_ddlt/`. A dedicated layout test checks their finite geometry,
orthogonal routing, and valid SVG output; they are intentionally excluded from ordinary snapshot
admission.

## Evidence Layers

| Layer | Scope | Current worktree count |
| --- | --- | ---: |
| Semantic goldens | Parse and semantic JSON | 3,747 |
| Layout goldens | Typed geometry and bounds | 3,744 |
| Upstream SVG baselines | Pinned Mermaid CLI output | 3,696 |

Counts are an audit snapshot, not an API contract. `check-alignment` validates required evidence
for each admission record rather than relying on these totals.

Refresh semantic and layout goldens with `cargo run -p xtask -- update-snapshots` and
`cargo run -p xtask -- update-layout-snapshots`. The upstream baseline procedure lives in
`docs/rendering/UPSTREAM_SVG_BASELINES.md`.

Raster PNG/JPG/PDF output is best-effort integration output, not a pixel-parity layer. Pure-Rust
rasterizers do not reproduce every browser `foreignObject` behavior; see
`docs/rendering/RASTER_OUTPUT.md`.

## Current Boundaries

- Parser, semantic model, config precedence, layout, routing, sanitizer, and DOM-order differences
  require source-backed fixes. Comparator normalization must not hide them.
- Runtime fixture IDs and complete fixture-label lookup tables are not accepted measurement or
  viewport mechanisms. Root bounds must come from family geometry or emitted content.
- General font/DOM-shape facts may be generated from repeatable browser or font evidence, but they
  must generalize beyond fixture strings.
- Browser `getBBox()` floats, font fallback, `foreignObject`, HTML serialization, and RoughJS may
  remain visible residuals when a robust headless derivation is unavailable.
- Architecture compound layout remains a source-backed approximation of upstream
  Cytoscape/FCoSE behavior; family coverage documents own its geometry residuals.
- Mermaid 11.16 has a known upstream Flowchart regression for arrows between subgraphs
  ([mermaid-js/mermaid#7954](https://github.com/mermaid-js/mermaid/issues/7954)). Do not restore
  11.15 behavior locally to make affected fixtures look different from the pinned upstream.

CLI compatibility details live in `docs/alignment/CLI_COMPATIBILITY.md`. The CLI supports the
documented `mmdc@11.16.0` render/export flags through `merman-cli mmdc`; it does not install an
`mmdc` binary alias. Native rendering uses the separate `render` and `batch` commands.

## Verification

Run the smallest gate that proves the change, then widen according to blast radius:

```sh
# Family capabilities, fixtures, upstream manifests, and compare facts.
cargo run -p xtask -- check-alignment

# One admitted family.
cargo run -p xtask -- compare-<diagram>-svgs --check-dom --dom-decimals 3

# Standard repository contract: fmt, nextest, structure, and parity.
cargo run -p xtask -- verify

# Release-strength contract, including all features, clippy, feature matrix, and root parity.
cargo run -p xtask -- verify --strict

# Mermaid 11.16 added-MMD corpus integrity.
cargo run -p xtask -- sync-upstream-mmd-corpus \
  --from mermaid@11.15.0 \
  --to mermaid@11.16.0 \
  --check
```

For a single SVG bounds investigation:

```sh
cargo run -p xtask -- debug-svg-bbox --svg <path> --padding 8
```
