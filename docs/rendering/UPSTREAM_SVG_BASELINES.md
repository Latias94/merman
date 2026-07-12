# Upstream SVG Baselines

This document describes how to generate **upstream Mermaid SVG outputs** that act as baselines for
1:1 parity work.

Baseline version: Mermaid `@11.16.0`.

Historical fixture notes may still mention the baseline version that introduced a fixture or
normalization rule. The current authoritative baseline is ADR-0001 plus
`tools/upstreams/REPOS.lock.json`.

## Why This Exists

Without upstream SVG baselines, it is easy to "visually align by feel" and regress output
in subtle ways (marker ids, viewBox sizing, CSS selectors, etc). Baselines make changes auditable.

## Golden Layers

To make 1:1 parity work tractable, `merman` keeps multiple kinds of goldens:

- Upstream SVG baselines (this doc): the authoritative end-to-end output from Mermaid (via CLI).
- Semantic snapshots: parser output snapshots for `fixtures/**/*.mmd` (generated via
  `cargo run -p xtask -- update-snapshots`).
- Layout golden snapshots: geometry-level snapshots (`*.layout.golden.json`) that validate the
  headless layout model and help localize diffs to layout vs. SVG rendering (see
  `docs/adr/0047-layout-golden-snapshots.md`).

## Recommended Additional Goldens

If we need tighter 1:1 parity coverage beyond ER, extend the golden strategy in these directions:

- **Diagram-by-diagram SVG compare reports** (like `compare-er-svgs`), including:
  - viewBox + width/height deltas
  - marker and defs checks (arrowheads, gradients, filters)
  - optional geometry probes (e.g. parse `<path d>` and compare command sequences at a high level)
- **Error/diagnostics snapshots** for known-invalid inputs (parse errors and runtime render errors),
  including line/column ranges and message text.
- **Security-level snapshots** for sanitization behavior (e.g. `securityLevel` differences, HTML
  label allowlists), to prevent accidental loosening.
- **Theme/style snapshots** that lock the generated CSS blocks for a small set of themes and config
  overrides (prevents silent selector drift).

Config/frontmatter fixtures should also update
`docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md` with the field's accepted/merged/consumed/rendered
evidence. This keeps parser-entry support, layout consumption, and SVG golden coverage from being
collapsed into a single "supported" claim.

## Tooling

We use `@mermaid-js/mermaid-cli` pinned under `tools/mermaid-cli/`.
The CLI version and Mermaid version do not always match 1:1, so we use `npm overrides`
to force Mermaid `11.16.0`.

Install:

- `cd tools/mermaid-cli && npm install`

### Render-environment attestation

Every schema-v2 `_baseline-manifest.json` distinguishes between two provenance modes:

- `generated`: the complete family was rendered in one measured environment. The manifest records
  the CDP browser product/version/revision, Puppeteer version, OS identity, the versions reported by
  both Mermaid's ESM and IIFE runtimes, SHA-256 tree fingerprints for the installed Mermaid and
  Mermaid CLI packages, and a browser-font fingerprint.
- `adopted-existing`: the corpus and hashes were validated, but its historical browser environment
  cannot be proved. Do not add a render environment to an adopted corpus after the fact.

`gen-upstream-svgs` probes the browser once before rendering or writing provenance. It then passes
the exact executable reported by the launched Puppeteer process to both mmdc and the seeded IIFE
renderer. To select a browser, set `PUPPETEER_EXECUTABLE_PATH` before invoking xtask;
`CHROME_EXECUTABLE` is not a Puppeteer configuration input. The absolute executable path is used
only for the current command and is never stored in the manifest. Timed renderer processes run in
a managed process tree; Puppeteer detachment is disabled so timeout cleanup terminates and reaps
both Node and its browser descendants. Generated probe and seeded-renderer scripts use immutable,
content-addressed paths installed by atomic rename, so concurrent xtask processes cannot observe a
partially written script.

The probe resolves Mermaid from the actual `@mermaid-js/mermaid-cli` package context, so the ESM and
IIFE attestations describe the same dependency tree that mmdc uses rather than an unrelated root
`node_modules/mermaid` installation. The package fingerprints must also match the pinned 11.16.0
artifacts, so a same-version locally modified runtime is rejected before rendering.

The font probe hashes fixed SVG `getBBox`/`getComputedTextLength` and canvas `measureText` samples.
It is an environment fingerprint only: the values must not be copied into `merman-render`, used to
tune text coefficients, or turned into fixture-specific wrapping overrides.

A filtered generation against the baseline corpus may extend only an existing `generated` manifest
with the exact same measured environment. The isolated `--fresh-output` mode used by
`check-upstream-svgs` is the exception: when the family output directory is empty,
`--filter --fresh-output` may create a new `generated` manifest with `complete: false`. That
manifest covers only the selected check output and cannot establish or replace a complete baseline
corpus. Use a complete family generation to establish a new environment or replace an adopted
corpus. A filtered merge into an already complete generated corpus keeps `complete: true` only after
the merged manifest is revalidated against the entire live family; a previously incomplete corpus
remains incomplete. `check-upstream-svgs` compares render environments before comparing fresh SVG
output; an `adopted-existing` baseline must be fully regenerated before that command can claim
reproducibility.

Generation is transactional at the family batch boundary. Every SVG is rendered and validated in a
temporary location before any baseline is promoted. The manifest is staged only after all outputs and
provenance checks succeed. Promotion uses backups for both replacements and deletions, including the
removal of a stale SVG when its fixture becomes excluded, and restores the previous SVGs and manifest
if any file or metadata commit fails. A rejected partial environment therefore cannot leave new SVGs
paired with stale provenance. Generation and provenance adoption share the same per-family
cross-process lock for final preflight, SVG promotion, and manifest commit. Baseline checks and
pinned compare commands hold that lock while reading the manifest and SVG corpus, so they cannot
observe a writer's promotion window. Provenance adoption with `--diagram all` acquires all family
locks in stable output-path order, validates every family, then stages, backs up, and installs all
manifests as one rollback-capable batch. A failed later install therefore restores every earlier
family instead of leaving a partial adoption.

All generator invocations also hold one cross-process Mermaid CLI toolchain lock from the
`node_modules` installation check through rendering and the final runtime-package fingerprint
verification. This serializes `npm ci`/`npm install` against every renderer that reads the shared
toolchain, including imports that already hold a family transaction lock.

To validate and honestly adopt historical schema-v1 baselines without inventing environment proof:

- `cargo run -p xtask -- adopt-upstream-svg-provenance --diagram all --allow-downgrade`

## Import Fixtures From Mermaid Syntax Docs

To keep fixture expansion repeatable and traceable, `xtask` can import Mermaid code fences from
the upstream syntax docs under `repo-ref/mermaid/docs/syntax/*.md`.

- Import `sequenceDiagram` doc fences as `fixtures/sequence/upstream_docs_*.mmd` (deduped by content):
  - `cargo run -p xtask -- import-upstream-docs --diagram sequence`
- Import a small batch while iterating:
  - `cargo run -p xtask -- import-upstream-docs --diagram flowchart --limit 10`
- Prefer more complex examples (largest/feature-dense blocks first):
  - `cargo run -p xtask -- import-upstream-docs --diagram flowchart --complex --min-lines 10 --limit 10`
- Optional: also generate upstream SVG baselines + refresh semantic/layout goldens:
  - `cargo run -p xtask -- import-upstream-docs --diagram sequence --with-baselines`

Notes:

- Import uses Mermaid-like type detection for ` ```mermaid` fences, so mixed-diagram pages can be
  imported into the correct `fixtures/<diagram>/` folder.
- External plugin docs like `zenuml.md` are currently ignored (out of scope for pinned SVG parity).
- When using `--with-baselines`, the import will **skip** candidates that:
  - fail upstream CLI rendering, or
  - produce a suspicious "blank" upstream SVG (commonly a 16×16 `viewBox` for empty diagrams).
  Skips are logged to `target/import-upstream-docs.report.txt` for later triage.

## Generate (ER only)

- `cargo run -p xtask -- gen-upstream-svgs --diagram er`

Outputs to:

- `fixtures/upstream-svgs/er/*.svg`

## Generate (Sequence)

- `cargo run -p xtask -- gen-upstream-svgs --diagram sequence`

Outputs to:

- `fixtures/upstream-svgs/sequence/*.svg`

## Generate (Info)

- `cargo run -p xtask -- gen-upstream-svgs --diagram info`

Outputs to:

- `fixtures/upstream-svgs/info/*.svg`

## Generate (Pie)

- `cargo run -p xtask -- gen-upstream-svgs --diagram pie`

Outputs to:

- `fixtures/upstream-svgs/pie/*.svg`

## Generate (Sankey)

- `cargo run -p xtask -- gen-upstream-svgs --diagram sankey`

Outputs to:

- `fixtures/upstream-svgs/sankey/*.svg`

## Generate (Packet)

- `cargo run -p xtask -- gen-upstream-svgs --diagram packet`

Outputs to:

- `fixtures/upstream-svgs/packet/*.svg`

## Generate (Timeline)

- `cargo run -p xtask -- gen-upstream-svgs --diagram timeline`

Outputs to:

- `fixtures/upstream-svgs/timeline/*.svg`

## Generate (Journey)

- `cargo run -p xtask -- gen-upstream-svgs --diagram journey`

Outputs to:

- `fixtures/upstream-svgs/journey/*.svg`

## Generate (Kanban)

- `cargo run -p xtask -- gen-upstream-svgs --diagram kanban`

Outputs to:

- `fixtures/upstream-svgs/kanban/*.svg`

## Generate (Gantt)

- `cargo run -p xtask -- gen-upstream-svgs --diagram gantt`

Outputs to:

- `fixtures/upstream-svgs/gantt/*.svg`

## Generate (GitGraph)

- `cargo run -p xtask -- gen-upstream-svgs --diagram gitgraph`

Outputs to:

- `fixtures/upstream-svgs/gitgraph/*.svg`

## Generate (C4)

- `cargo run -p xtask -- gen-upstream-svgs --diagram c4`

Outputs to:

- `fixtures/upstream-svgs/c4/*.svg`

Notes:

- Mermaid C4 has known render-time type assumptions that make some valid parser fixtures
  non-renderable (e.g. kv-objects stored in `label.text`, or `UpdateElementStyle(..., techn="Rust")`
  storing `techn` as a raw string).
- `xtask gen-upstream-svgs --diagram c4` skips such fixtures when generating baselines.

## Generate (Block)

- `cargo run -p xtask -- gen-upstream-svgs --diagram block`

Outputs to:

- `fixtures/upstream-svgs/block/*.svg`

## Generate (Radar)

- `cargo run -p xtask -- gen-upstream-svgs --diagram radar`

Outputs to:

- `fixtures/upstream-svgs/radar/*.svg`

## Generate (Treemap)

- `cargo run -p xtask -- gen-upstream-svgs --diagram treemap`

Outputs to:

- `fixtures/upstream-svgs/treemap/*.svg`

## Parser-Only Fixtures

Some fixtures are intentionally **parser-only** (they validate semantic parsing but are not
renderable in upstream Mermaid at the pinned version).

Convention:

- Any fixture whose filename contains `_parser_only_` (or `_parser_only_spec`) is skipped by:
  - `xtask gen-upstream-svgs`
  - `xtask check-upstream-svgs`
  - diagram compare tasks like `xtask compare-flowchart-svgs`
- Flowchart KaTeX HTML-demo fixtures are an explicit non-example: the active files use `*_katex`
  stems and participate in upstream SVG generation/check/compare through the Node/Puppeteer KaTeX
  measurement backend.

## Normalized Fixtures (CLI-Compatible)

Some upstream suites (notably Cypress) include inputs that are accepted by the browser bundle but
rejected by the pinned Mermaid CLI (currently `@11.16.0`), often due to shorthand syntax.

To preserve the upstream strings *and* still get authoritative CLI SVG baselines + DOM parity
comparisons, we add `*_normalized` variants that rewrite the input into the pinned Mermaid grammar.

Rule of thumb:

- keep the upstream string as a `*_parser_only_` fixture (semantic-only), and
- add a `*_normalized` fixture that is eligible for:
  - `xtask gen-upstream-svgs`
  - layout snapshots (`*.layout.golden.json`)
  - DOM parity compares

## Generate (C4 Stage B)

Generate local Stage-B C4 SVG outputs (not upstream baselines):

- `cargo run -p xtask -- gen-c4-svgs`

Outputs to:

- `target/svgs/c4/*.svg`

## Compare (C4)

Generate a report comparing upstream C4 SVGs and the current Rust Stage-B C4 output:

- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity --dom-decimals 3`

Notes:

- Mermaid derives C4 type-line `textLength` values from browser font metrics
  (`calculateTextWidth` + `getBBox`). To make DOM parity reproducible in a headless Rust context,
  `merman-render` now owns the observed `textLength` values for built-in C4 shape types directly
  in `crates/merman-render/src/svg/parity/c4.rs`. The current source baseline is Mermaid `11.16.0`;
  existing measured constants should be refreshed when the C4 SVG corpus is regenerated.

## Generate (All supported diagrams)

- `cargo run -p xtask -- gen-upstream-svgs --diagram all`

## Verify Baselines (All supported diagrams)

Regenerate upstream SVGs into `target/upstream-svgs-check/` and verify they match the pinned
baselines under `fixtures/upstream-svgs/`:

- `cargo run -p xtask -- check-upstream-svgs --diagram all`

Notes:

- Most diagrams are compared as **raw SVG bytes** (exact string match).
- Some diagrams are compared using a **structure-level DOM signature** by default (instead of raw
  bytes) because their upstream output is not reliably byte-stable across environments:
  - `state`: rough/stochastic geometry output (the DOM check ignores `<path d>` / `data-points` and
    normalizes generated ids).
  - `gitGraph`: auto-generated commit ids with random suffixes (not byte-stable).
  - `gantt`: output depends on the rendering environment (page width via
    `parentElement.offsetWidth`) and may include a `today` marker whose x-position depends on the
    current date.
  - `er`, `class`, `requirement`, `block`, `mindmap`, `architecture`: upstream SVG often differs
    only by numeric formatting / viewport rounding / minor browser-layout drift, which makes raw
    byte comparisons too strict for CI.
- To force DOM comparison for all diagrams (useful when iterating on tooling):
  - `cargo run -p xtask -- check-upstream-svgs --diagram all --check-dom --dom-mode structure --dom-decimals 3`

Determinism note:

- Architecture diagrams use Cytoscape `fcose`, whose spectral initialization relies on
  `Math.random()`. To keep baselines reproducible, `xtask gen-upstream-svgs --diagram architecture`
  renders via a small Puppeteer wrapper that seeds browser-side randomness deterministically (while
  still using the official Mermaid CLI bundle).

## Compare (ER)

Generate a small report comparing upstream SVGs and the current Rust Stage-B ER SVG output:

- `cargo run -p xtask -- compare-er-svgs`
- Fail the command if marker definitions diverge:
  - `cargo run -p xtask -- compare-er-svgs --check-markers`
- Fail the command if the **SVG DOM** diverges (ignores attribute order/whitespace and rounds
  numeric tokens for comparison):
  - `cargo run -p xtask -- compare-er-svgs --check-dom --dom-decimals 3` (default `--dom-mode parity`)
  - Use a looser, structure-only mode while iterating on DOM shape (replaces numeric tokens with
    `<n>`, ignores `data-points`, and ignores `<style>` text):
    - `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode structure --dom-decimals 3`
  - Use a parity-focused mode to ignore geometry noise (replaces numeric tokens in geometry attrs
    with `<n>`, ignores `data-points` and `<style>` text, and ignores `max-width` heuristics inside
    HTML label `<div>` style attributes):
    - `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity --dom-decimals 3`
  - For size/viewBox parity work, use `parity-root` which is identical to `parity` but also compares
    the root `<svg>` `viewBox` and `style` attributes:
    - `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

## Generate (Flowchart Stage B)

Generate local Stage-B flowchart SVG outputs (not upstream baselines):

- `cargo run -p xtask -- gen-flowchart-svgs`

Outputs to:

- `target/svgs/flowchart/*.svg`

## Compare (Flowchart)

Generate a report comparing upstream flowchart SVGs and the current Rust Stage-B flowchart output:

- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- Use the looser, structure-only mode while iterating on large layout/routing refactors:
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode structure --dom-decimals 3`
- For root `<svg>` viewport parity (`viewBox` / `style="max-width: ..."`), use `parity-root` and the root-delta report:
  - `cargo run -p xtask -- compare-flowchart-svgs --dom-mode parity-root --dom-decimals 3 --report-root`
  - See `docs/alignment/FLOWCHART_ROOT_VIEWBOX_PARITY_GAPS.md` for current status.

## Compare (Block)

Generate a report comparing upstream block SVGs and the current Rust Stage-B block output:

- `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (Radar)

Generate a report comparing upstream radar SVGs and the current Rust Stage-B radar output:

- `cargo run -p xtask -- compare-radar-svgs --check-dom --dom-mode parity --dom-decimals 3`

Notes:

- Flowchart `domId` suffixes depend on FlowDB `vertexCounter` (Jison `addVertex(...)` call order, including `@{...}` shapeData passes).
  The flowchart semantic model includes `vertexCalls` to make this deterministic and reproducible in Rust.

## Generate (StateDiagram Stage B)

Generate local Stage-B stateDiagram SVG outputs (not upstream baselines):

- `cargo run -p xtask -- gen-state-svgs`

Outputs to:

- `target/svgs/state/*.svg`

## Generate (ClassDiagram Stage B)

Generate local Stage-B classDiagram SVG outputs (not upstream baselines):

- `cargo run -p xtask -- gen-class-svgs`

Outputs to:

- `target/svgs/class/*.svg`

Notes:

- Stage-B class layout sizes nodes using the global Mermaid `fontSize` and a fixed `line-height: 1.5`
  (matching upstream HTML label structure) to keep layout and SVG rendering consistent and avoid
  label overlap.

## Compare (StateDiagram)

Generate a report comparing upstream stateDiagram SVGs and the current Rust Stage-B stateDiagram
output (DOM signature comparison; upstream is not byte-stable):

- `cargo run -p xtask -- compare-state-svgs --dom-mode structure --dom-decimals 3`

## Compare (ClassDiagram)

Generate a report comparing upstream classDiagram SVGs and the current Rust Stage-B classDiagram
output (DOM signature comparison):

- `cargo run -p xtask -- compare-class-svgs --dom-mode parity --dom-decimals 3`
- Use the looser, structure-only mode while iterating on DOM shape:
  - `cargo run -p xtask -- compare-class-svgs --dom-mode structure --dom-decimals 3`

Notes:

- `fixtures/class/upstream_text_label_variants_spec.mmd` is excluded (Mermaid CLI failure first
  recorded at 11.15.0; re-check before admitting under newer baselines).
- `fixtures/class/upstream_parser_class_spec.mmd` is excluded from Class DOM and canonical-XML
  compares because the upstream SVG contains prototype-key rendering artifacts (nested `g.root` /
  `translate(NaN, ...)` and missing prototype-key nodes), while `merman` renders deterministically.

Notes:

- The flowchart DOM compare is intentionally looser than ER while Stage-B rendering is still being
  brought up. It ignores `<path d>` and `data-points` geometry payloads and normalizes child order
  for container groups like `g.root` by using the first descendant cluster id as a sort hint.

## Compare (Info)

Generate a report comparing upstream info SVGs and the current Rust Stage-B info output:

- `cargo run -p xtask -- compare-info-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (Pie)

Generate a report comparing upstream pie SVGs and the current Rust Stage-B pie output:

- `cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (Packet)

Generate a report comparing upstream packet SVGs and the current Rust Stage-B packet output:

- `cargo run -p xtask -- compare-packet-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (Timeline)

Generate a report comparing upstream timeline SVGs and the current Rust Stage-B timeline output:

- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (Journey)

Generate a report comparing upstream journey SVGs and the current Rust Stage-B journey output:

- `cargo run -p xtask -- compare-journey-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (Kanban)

Generate a report comparing upstream kanban SVGs and the current Rust Stage-B kanban output:

- `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Compare (GitGraph)

Generate a report comparing upstream gitGraph SVGs and the current Rust Stage-B gitGraph output:

- `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Notes

- The generator passes `--svgId <fixture_stem>` to make the root SVG id deterministic.
- If any fixture fails, the tool preserves that family's existing SVGs and manifest, removes the
  temporary outputs, records a unique report under
  `<output-root>/.xtask-upstream-svg-staging/<diagram>/`, and exits non-zero. SVGs and provenance are
  promoted only after the complete batch validates; failure reports never contaminate a fresh family
  output directory.
- We currently store raw upstream SVG outputs. For `state` diagrams, upstream output is not
  byte-stable, so baseline verification uses a structure-level DOM signature instead of a raw byte
  compare.
- `gitgraph` output is not byte-stable because commit ids can be randomly generated by upstream
  Mermaid when not explicitly specified. Baseline verification uses a structure-level DOM signature
  by default.

## Known Upstream Rendering Failures / Anomalies

- `fixtures/state/upstream_state_parser_spec.mmd`: includes `__proto__`/`constructor` states; Mermaid CLI currently crashes (excluded from `gen-upstream-svgs` / `check-upstream-svgs`).
- `fixtures/class/upstream_text_label_variants_spec.mmd`: includes a whitespace-only label (`" "`); Mermaid CLI currently fails (NaN transforms / missing SVG in render tree; excluded from `gen-upstream-svgs` / `check-upstream-svgs`).
- `fixtures/class/upstream_parser_class_spec.mmd`: includes `__proto__`/`constructor` classes; Mermaid CLI renders but produces invalid transforms (NaN), duplicated root groups, and missing prototype-key nodes (excluded from `compare-class-svgs` and `compare-svg-xml`).
- `fixtures/gantt/today_marker_and_axis.mmd`: Mermaid CLI crashes while parsing `topAxis` (`yy.TopAxis is not a function`) (excluded from `gen-upstream-svgs` / `check-upstream-svgs`).
- `fixtures/gantt/click_loose.mmd` / `fixtures/gantt/click_strict.mmd`: contain non-canonical `click ... href "<url>" "<extra>"` syntax that Mermaid CLI rejects (excluded from `gen-upstream-svgs` / `check-upstream-svgs`).
- `fixtures/gantt/dateformat_hash_comment_truncates.mmd` / `fixtures/gantt/excludes_hash_comment_truncates.mmd`: rely on `#` inline comment truncation that Mermaid CLI rejects (excluded from `gen-upstream-svgs` / `check-upstream-svgs`).

These exclusions keep baseline verification and compare reports actionable for the rest of the suite.
