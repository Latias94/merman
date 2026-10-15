# ADR-0094: Mermaid 12.2 Upstream Transition

## Status

Accepted for the development baseline. The reference refresh and strict verification completed
on 2026-10-13. This records local verification of the selected graph, not a publication decision.
This decision does not authorize publication or modify the published 0.9.0 release history.

## Date

2026-10-13

## Decision

Select Mermaid `12.2.0`, tag `mermaid@12.2.0`, commit
`1fb7869866dd619168d840291c9c54b4f129758b`, over the previous `12.1.0` graph at
`21f72f07ea22c0af48a3149c550654e80d8e40cb`. Preserve parser `2.0.1`, reference CLI `12.0.0`,
DOMPurify `3.4.16`, Tidy Tree `1.0.1`, ZenUML plugin `1.0.1`, ZenUML Core `3.50.1`, and the
reference compiler/browser-driver selections. Their independently published source identities remain
unchanged. The ordered built-in diagram and layout registration inventories are unchanged.

Add no Cargo feature: the changes fit the existing diagram, SVG, layout and math capabilities and
introduce no Rust dependency or platform boundary. Public transport schemas and FFI versions remain
unchanged. The independent Tree-sitter package retains its own selected baseline; `colorBy` is
configuration, not a new grammar production.

## Source-backed behavior

All paths below refer to the selected source commit under `repo-ref/mermaid`.

| Change | Source | Local evidence |
| --- | --- | --- |
| Structural depth colors, eleven-color cycle and child-edge color | `packages/mermaid/src/diagrams/mindmap/mindmapDb.ts`, `mindmapColors.spec.ts` | Core Mindmap regressions and `upstream_mermaid_12_2_depth_colors` |
| Center SVG labels and color Neo HTML labels | `mindmapDb.getData.test.ts`, `mindmap/styles.ts` | `mindmap_mermaid12_2_test` and `upstream_mermaid_12_2_svg_label_centering` |
| Circle label diagonal, minimum-width opt-out and one Neo padding step | `rendering-elements/shapes/circle.ts`, `doubleCircle.ts`, `util.spec.ts` | `flowchart_circle_mermaid_12_2_test`, shared shape tests and Mindmap circle regressions |
| Reject terminal straightening that increases edge/label coverage | `layout-algorithms/elk/render.ts`, `elk/__tests__/render.spec.ts` | Shared ELK and parent-relative paint-route regressions across graph families |
| Preserve disconnected source order and split terminal lanes after crossing cleanup | `swimlanes/phase0.helpers.ts`, `postProcessing.ts` | Source-order and fixed-size detour regressions |
| Account for inherited/case-insensitive direction and title-band boundary rails | `swimlanes/direction/materializedGeometry.ts` | Title-band geometry regressions |
| Scope explicit lane DOM IDs and clip using the actual engine | `clusters/swimlane.js`, `edges.js`, `layout-algorithms/common/index.ts` | Swimlane SVG and clipping regressions, including synthetic-lane fallback |

Source-backed snapshots add the four inputs under
`e2e/platform/dev-diagrams/layout-tests/swimlanes/regressions/` for source order, title rails and
terminal detours. The large upstream E2E directory migration does not change the semantics of the
historical retained Cypress manifests. Existing source-corpus identities remain historical.

## Reference execution and admission

Node `24.21.0` and npm `12.0.2` installed the candidate reference graph with lifecycle scripts
disabled. The exact official npm signatures/attestations command passed with 55 verified packages,
zero invalid and zero missing records. The raw output remains temporary admission evidence, bound
by the compact selected decision receipt. Eight Chromium probes passed for depth colors, SVG
labels, Neo labels, Circle minimum widths, ELK, actual Swimlane engine selection and HTML cleaning.
The probes executed the existing shared reference-runtime builder and selected sanitizer.

The source manifest declares Chevrotain `~11.1.2`, while the verified published Mermaid archive
declares `~13.2.0`. Executable reference behavior follows the published package. Parser `2.0.1`
is unchanged. Source changes in this release do not add a parser production.

KaTeX's source and published range moves to `^0.18.0`; the reference CLI lock resolves its
compatible `0.18.10` package. The Playground selects the same `0.18.10` for Mermaid while retaining
its existing `0.19.0` security override for external dependencies. The compatible core version has
no published KaTeX advisory in the official npm audit used during this transition.
An outside-range stable package is not substituted into the reference CLI implicitly. Rust math
remains the existing independently implemented rendering capability. Theme runtime values are
unchanged; default configuration adds only `mindmap.colorBy: "branch"`.

The optional Node math comparison backend resolves both KaTeX and its stylesheet from Mermaid's
module location. The reference workspace also contains an older KaTeX copy for ZenUML; resolving
from the workspace root would execute that plugin dependency instead of Mermaid's selected one.
Hosts without Mermaid retain the documented local-KaTeX backend behavior.

## Reviewed artifact boundary

The 37-family refresh executes the selected runtime: 3,517 existing SVGs stay byte-identical,
203 change and eleven primary source-backed inputs are added. Two additional measured
inputs remain family-local controlled evidence. The changes belong to Mindmap, circle-driven
Flowchart layout, Swimlane routing, the State ELK guard, and version text in Info/error diagrams.
The 28 semantic-label receipts retain exactly the same input/upstream geometry and SVG hashes.
Ten parser diagnostic receipts change only the rendered version footer.

The strict comparator retires eight Flowchart browser-text receipts after their DOM now matches.
Nine changed Flowchart signatures retain existing measurement boundaries after individual identity,
label, shape and source-geometry review. State 040 retains its existing exact receipt; State 039
adds one exact `parity`/`parity-root` receipt because browser label width selects a different legal
terminal-straightening branch. Controlled browser dimensions reproduce all 16 upstream edge-path
topologies and verify that the guarded edge retains the original non-overlapping route. The
catalog therefore shrinks from 139 to 132 entries. No family-wide normalization or tolerance is
added. The comparator emits the existing canonical local signature for an unadmitted diagnostic
so a reviewed receipt can be bound without an independent XML canonicalizer.

The filtered-title root-paint receipt remains tied to its original browser/platform audit. Its
SVG inputs are unchanged; a Windows replay cannot substitute for that exact CI environment.
This platform-bound paint residual remains separate from the cross-platform structural root gate.

## Fixture sufficiency

[The coverage review](../alignment/FIXTURE_COVERAGE_12_2.md) maps source deltas to the semantic,
layout, rendering and browser layers. Four added primary inputs cover Neo depth/palette cycling,
the complete ELK #8368 issue, and mixed-order/non-default-width Swimlane cases. Two measured
family-local inputs preserve narrow SVG Markdown and HandDrawn circle evidence. They found
source-backed rect-padding and Rough circle branch omissions; neither fix broadens comparators.

Six follow-up primary inputs cover ABNF/EBNF/PEG grammar combinations, nested Agentflow collapse
and self-loops, a single-boundary Usecase ELK guard rejection, and nested/explicit/synthetic Swimlane
lanes with strict link filtering. Their actual selected-runtime SVGs expose further source omissions:
Agentflow is absent from Dagre's self-loop merge allowlist; Usecase SVG label backgrounds contribute
to provider and guard dimensions; Swimlane synthetic colors follow declaration slots, and label
movement follows the actual updated-path signal. These repairs preserve the source graph, root
bounds and existing comparator policies. The primary matrix now has 3,761 inputs and 3,737 SVGs.

HandDrawn Circle and DoubleCircle now use the existing Rough generator with the source fill,
ring order, classes and per-ring stroke widths. Each ring charges the existing work meter before
sampling or hachure allocation and checks cancellation before generation. Source-shared curve
defaults and checked arithmetic bound the scan estimate; there is no new coordinate cap or resource
framework. `stroke:none` preserves PRNG generation order while omitting the outline from the SVG.
Exact-budget, oversized finite geometry, overflow and between-ring cancellation regressions cover
the new path. Other Rough shape helpers keep their existing behavior.

## Browser host integration

Fresh locked browser tooling exposed two host issues during the full Playground gate. The source
geometry test server now pre-optimizes the late syntax-worker dependency before its page-level
module evaluations. The test retains its original raster assertions and does not retry navigation.
The opaque Compare and Benchmark shell inherits the host document language through an escaped
HTML attribute. Identical C4 text CSS otherwise chose different Windows fallback fonts in the host
and a language-less iframe. This changes host projection only: verified bootstrap bytes, CSP hashes,
protocol fields, Rust geometry and numeric comparison tolerances stay unchanged. Language is
captured when the realm is created; live UI-language synchronization is outside this change.

## Verification boundary

The initial aligned revision passes workspace nextest (9,427 passed, seven existing ignored tests),
formatting, all-target/all-feature Clippy, 652 feature combinations and 36 artifact recipes.
`xtask verify --strict` completes successfully, including documentation, three-mode DOM comparison,
five Web/WASM recipes, 129 Web tests, 37-family Web consumer/DOM-safety smoke, 449 Playground tests,
lint/build, three non-Chromium browser cases, 110 Chromium desktop cases and 255 VS Code tests.
Two Web symlink tests are skipped by the Windows platform policy. Legal checks verify thirteen
Rust license reports and the governed package projections.

After the final Rough resource fix, the verifier is rebuilt from the frozen source. Materialized
and trusted-base reference verification, generated projections, and all 37 families in
`structure`, `parity`, and `parity-root` pass again. Exact command evidence is retained in the
2026-10-13 engineering verification shards; temporary raw logs remain under `target/mermaid-12.2/`.

The coverage follow-up passes workspace nextest with all features (9,716 tests, eighteen existing
feature/diagnostic skips). After the last source review fixes, 1,813 affected renderer tests,
all-target/all-feature Clippy and the release-mode 37-family three-mode DOM gate pass. Materialized
and trusted-base reference verification and generated projections also pass, as do the final
five-package Web/WASM build, Web tests, consumer and DOM-safety smoke. Chromium replay
confirms that both formerly blocking Swimlane inputs have no geometry or structural paint outside
their roots. Windows retains the existing platform-bound filtered-title diagnostic; Linux CI owns
its exact residual replay against the pushed revision.

The selected bundle, base-bound receipt, regenerated runtime projections and actual SVG generation
own the reference identity. Exact gate results and residual review belong to the verified revision
and engineering evidence shards. A selection receipt is not a claim that every local/browser output
is pixel-identical. No comparator normalization is broadened by this transition.
