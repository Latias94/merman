---
type: Verification Report
title: Brutalist and Spotless recipe expansion
timestamp: 2026-09-29
source_commit: working-tree based on 39c979e608165a3b30a0fcec7342e4f196285ced
reference_commit: a021cbce37fc0b07a9f4791c28e983101ea06f2d
---

# Result

Brutalist and Spotless now have dedicated recipe builders for Flowchart, Sequence and XY Chart.
They reuse existing paint, rule and export mechanisms. No Cargo dependency, feature, embedded font
or external asset was added. Class retains its `base_only` catalog entry; `qualified_cells` remains
empty. Dedicated design metadata describes rule scope, not completed visual qualification.

## Source-backed visual mechanisms

The reference is the pinned `modern_mermaid/src/utils/themes.ts`, specifically its `brutalist`
and `spotless` definitions. The implementation adapts semantic targets instead of CSS DOM selectors.

| Preset | Canvas | Shapes | Text |
| --- | --- | --- | --- |
| Brutalist | Warm solid background | Bold borders; 6 px hard shadow on nodes, actors, notes, activations and bars; Flowchart 2n/3n/5n accent colors | Bold weights; no text shadow |
| Spotless | Two perpendicular 40 px tiled gradients, 1 px ink lines at 2% alpha | Cream surfaces, dark brown outlines; no filters | Family-scoped weight and tracking; no text shadow |

Brutalist line series and connectors remain unfiltered. Spotless requires no filter rasterization
for its recipe. The first local prototype added stripe backgrounds and text shadows to both
presets; source review rejected those additions before delivery. Its earlier 18-output capture
is superseded by the final recipe checks below.

This is a scoped adaptation, not exact screenshot reproduction. Font selection remains host-owned.
Reference corner radii, uppercase transformation, per-series XY colors and CSS-specific shape
exceptions are not all reproduced. Semantic ordinal order can differ from DOM `nth-child` order.
These differences must remain explicit during visual review; they are not grounds to claim parity.

## Verification

- All 18 renderer preset unit tests pass, including exact fingerprints, export/import round trips,
  resource limits and the new distinct-mechanism check.
- The new check rejects text effects, requires the hard shadow for Brutalist and two canvas layers
  with no effect graphs for Spotless.
- The public catalog fixture records dedicated Flowchart, Sequence and XY designs for both presets,
  preserves Class base-only scope and leaves qualification cells empty.

### Ordinal fill correction during recovery

Native export success alone missed a visible defect: Brutalist's ordinal accent rules matched,
but the Flowchart route classifier rejected ordinal node fills. Best-effort output omitted those
fills and exposed Mermaid's default purple; `RequirePortable` reported three unresolved rules.

Flowchart and Swimlane now admit solid and transparent ordinal node fills through the existing
per-node paint consumer. Static paint behavior is unchanged. Ordinal strokes, clear, gradients,
patterns and non-default variants retain their existing unsupported status. No alternate
renderer, CSS rewrite or broad capability relaxation was introduced.

A real SVG regression test checks a complete 30-node 2n/3n/5n cycle, overlapping-rule precedence,
and inline source-fill precedence. A second test checks exact and cyclic transparent fills in
both Flowchart and Swimlane. The initial native capture from this recovery session predates this
fix and is superseded by the final results below.

### Recovery revalidation (2026-10-02 UTC)

The current worktree was rechecked before committing the recovered change:

- `cargo fmt --all -- --check`: passed.
- Renderer preset tests: 18 passed, included in 176 passing route, support-manifest, preset and
  Flowchart evidence unit tests after the ordinal-fill fix.
- Full Flowchart SVG integration target: 132 passed, including both new regressions.
- Binding catalog metadata tests with the `svg` feature: 3 passed.
- Binding authoring, preset export and support-query contract tests: 12 passed.
- `merman-theme-fixtures`: 67 passed, 2 skipped.

Cargo checks ran serially with `CARGO_BUILD_JOBS=1`, and nextest used one test thread.
Reproduce the renderer checks with:

```text
cargo nextest run --locked -p merman-render --lib -E 'test(diagram_theme::family_mechanism_matrix::tests) | test(flowchart::theme_evidence::tests) | test(diagram_theme::presets::tests) | test(diagram_theme::support_manifest::tests)' --test-threads=1
cargo nextest run --locked -p merman-render --test flowchart_svg_test --test-threads=1
```

This is change-scoped verification; it does not establish a full-workspace or cross-platform pass.

Native captures use the existing `public-cyberpunk` source fixtures to keep input geometry
constant across presets.

The release CLI rebuilt successfully. All 18 native exports succeeded (two presets, three
families, three formats); SVG XML and PNG/PDF signatures were checked. In these SVG captures,
Spotless has no applied filters, and Brutalist applies filters only to shape elements.

| Preset | Family | SVG bytes | PNG bytes | PDF bytes |
| --- | --- | ---: | ---: | ---: |
| brutalist | flowchart | 19,672 | 12,529 | 41,975 |
| brutalist | sequence | 29,403 | 12,334 | 33,633 |
| brutalist | xychart | 15,377 | 16,711 | 46,368 |
| spotless | flowchart | 18,133 | 13,944 | 31,940 |
| spotless | sequence | 26,600 | 14,492 | 31,131 |
| spotless | xychart | 10,768 | 18,785 | 33,896 |

Local artifacts and SHA-256 values are in
`target/bench/experiments/theme-preset-recovery-final-20261002T160818Z/matrix.json`.
The final SVG checks also assert the Flowchart fixture's white, yellow and teal node fills.
These bounded smoke checks do not qualify arbitrary inputs or prove pixel parity.

## Cost and acceptance boundary

These changes add recipe construction code and static values, and admit ordinal node fills through
the existing Flowchart-family paint consumer. They do not resolve or measure the branch-wide
binary-size and default-render regression. No zero-overhead claim follows from the
unchanged dependency list. The existing alpha.6 comparison and default-path profiling remain open.

For bounded native output, retain hard shadows when the backend supports them. Larger diagrams
must keep the existing explicit filter-budget errors; do not drop effects or relax budgets to make
an export succeed. Appearance qualification still requires dense-label and reference-image review.
