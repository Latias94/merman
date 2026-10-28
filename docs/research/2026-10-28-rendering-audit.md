# Rendering contract audit follow-up

Date: 2026-10-28. Branches: main (Mermaid 12.2.0) and v0.9.x (Mermaid 12.1.0).

This follow-up separates actual rendering counterexamples from verification gaps.
The earlier performance objective remains paused. Updating source-provenance hashes
is not a new measurement, a threshold change, or a performance pass.

## Reproducers and ownership

| Area | Minimal counterexample | Required contract |
| --- | --- | --- |
| Mathematical Unicode | `\smash{𝐀}`, `\mathrlap{𝐀}`, `\mathclap{𝐀}` | The actual emitted glyph and its bounds must use the same KaTeX glyph identity, including Unicode mapping. |
| Native formula viewport | `x\rlap{yyyyyyyy}` and `\llap{yyyyyyyy}x` | Native projection preserves the producer's viewport clipping, including negative origins and bitmap glyphs. |
| Sequence formula rows | `$$x$$<br/>$$x+x+x+x$$` | Keeping the foreignObject origin does not change the producer's centering of shorter formula rows. |
| Block HTML labels | `AAAAAA<br/>BBBBBB` on nodes and edges | The measured HTML and emitted prepared fragment agree about actual breaks and visible content. |
| Class SVG labels | `**MMMMMMMM**` as a class title or relationship label | Parsing, role weight, final visible runs and measurement agree; only upstream's conditional literal fallback is preserved. |
| Class fallback boundary | `&g **t**` | Preserve spaces between emitted words before testing the upstream literal fallback condition. |
| Literal SVG row geometry | `A&lt;br&gt;B`, `**A&lt;br&gt;B**` | Decoded terminal words stay on their retained SVG row; bbox measurement does not reparse visible angle brackets as markup. |
| Block inline typography | `<span style="font-size:64px">MMMM</span>` | Retained font declarations reach measurement as well as emission, with the same inheritance context. |
| Sequence entity labels | `#128512;<br/>W` versus `😀<br/>W` | Final drawn bbox measurement splits actual breaks and then decodes each text node, exactly as the writer does. |
| Reference budgets | `<use xml:href="#p"/>` versus ordinary `href` | Budget planning chooses the same effective reference as the pinned renderer, retaining XLink precedence. |
| Math marker detection | User HTML whose id contains `data-merman-prepared-math-native=` | Marker text inside an attribute value does not establish renderer ownership. |
| Math receipts | Namespace changes on owned definitions; references to an occurrence's ancestor | A receipt protects expanded SVG names, projection context and the actual number of instances, not just local names and direct references. |
| Projection execution | Contiguous user ids colliding with generated math ids | Id allocation advances monotonically and projection observes existing cancellation checkpoints. |
| Maintenance report freezing | Current native-memory driver report | The freezer accepts the actual current report, verifies build/reset provenance and timeouts, and retains historical report compatibility. |
| Maintenance central CI | Manually dispatch central CI without fuzz inputs | The reusable fuzz workflow selects fixed regressions for the central caller; direct scheduled/manual discovery retains its own inputs. |
| Browser oracle | Remove or hide only a fraction bar | Whole-formula visibility and bbox equality cannot substitute for the bar's independent visible contribution. |

## Verification record

The first CI repair synchronizes only the missing local `merman-render -> ratex-font`
fuzz lock edge on each branch. Registry versions and checksums are unchanged. Both
pinned nightly locked checks pass. Main's stale current root-lock hashes and
maintenance's nine stale current bench/manifest/lock hashes are refreshed without
changing metric thresholds, semantic output digests, or admission rules. The
maintenance current-input assertion rejects all nine stale records before refresh.

The fraction oracle now checks four actual bars per branch in the complete emitted
SVG. Both deletion and opacity-zero mutations fail while numerator and denominator
remain present. The existing committed overview remains bound to its published
provenance; it is not a substitute for current-source regression tests.

Initial CI repairs: main `3ae46db9e` completes with 68 successful and seven
intentionally skipped checks. Maintenance `a627e8dd4` passes all seven fuzz jobs
and the performance contracts; its separate Intel macOS extension packaging job fails only while uploading
the artifact (`CreateArtifact: ENOTFOUND`), after tests. That job is rerun
without changing source checks and finishes successfully. These results do not
qualify later source edits.

The integrated main glyph/Sequence probes pass 11 tests. Maintenance passes 34
probes including its native projection controls. Maintenance current-driver
freezing passes 19 baseline tests and 29 driver contracts; the central fuzz
caller repair passes 54 relevant Python checks and actionlint. Full renderer,
current-source exports and final remote CI are recorded separately for the final revision.

## Architectural boundary

The repeated counterexamples demonstrate a renderer-owned preparation lifecycle
problem: authored text is reinterpreted during measurement and emission. This is
separate from headless font and browser geometry residuals. The main follow-up
retains family-local Class SVG labels and Block sanitized XHTML, and moves native
math geometry out of browser templates into the existing prepared evidence
transport. It does not introduce a universal text IR, scene graph, or CSS engine.
Terminal SVG parsing still owns actual placement and final validation.

Maintenance receives the independently reproduced correctness repairs. It does
not receive main's prepared math evidence transport or family artifact redesign.
A Class backslash control also exposed a missing SVG Markdown escape-token rule
in maintenance; the upstream browser controls retain the literal fallback and
require a direct outer tspan without an inner styled word.

The Block control uses ``prefix `literal` suffix`` inside a plain label. An entire
backtick-delimited label enters a different lexer state and is rejected by both
pinned Mermaid versions; it cannot establish a plain-text preservation contract.

## Main ownership verification

The initial integrated ownership run passes 5,461 of 5,463 renderer tests. Its two
failures are an invalid zero-limit cancellation fixture and the eight Sequence
layout goldens affected by entity decoding; both are corrected. A later complete
run passes 5,471 of 5,472 tests, with only six Class layout goldens changing after
title geometry switches to actual terminal bbox operations. Those goldens are
refreshed through the existing generator.

On the final renderer source, all 3,422 library, Block HTML, Class SVG, and layout
snapshot controls pass. The seven CLI math tests also pass, including actual ink,
separate rule and stroke pixels, source CSS, and raw native SVG reimport. The
Class-only literal-row entry preserves other families' existing host contracts.
The Block inline-font path preserves literal decoded angle brackets, collapsible
line tails, NBSP, inherited faces, relative sizes, and host vertical metrics.

The native math template producer and consumer are removed. Missing private
payloads fail projection. Reimported native vector SVG is admitted as ordinary
untrusted SVG and receives neither private evidence nor a prepared-math receipt.
Independent reviews confirm occurrence/fingerprint pairing, cancellation,
retained reservation lifetime, and extension invalidation on the production path.
The final source remains subject to the SVG matrix and remote CI recorded below.

The main SVG matrix identifies one exact local-signature change, Class
`stress_class_svg_font_size_px_string_precedence_026`. XML inspection preserves
all 123 elements and every text/paint/style attribute; only numeric title and
node geometry changes. Chromium measures the 24px `Foo` title at approximately
28px before and after. The pinned upstream title uses `translate(0,-14)`, as does
the repaired output; the old writer used `translate(0,-13.2)` from a 26.4px generic
line estimate. Refresh only that existing receipt's local signature from
`87ce258418433c0af094ecd8f2a751d47801cc33a4b94ac7eb6cbd2a64b14795`
to `1b14866d58e4f5e5324f392af9f3f6f59e6ddca7bbf41bb83f8b1138fb43bc87`.
Its fixture hash, upstream hash, modes, precision and admission rules remain
unchanged. This is a reviewed geometry change, not broader DOM normalization.

Final main renderer Clippy passes with all targets and the all-diagram, ELK,
Cytoscape, and math features under `-D warnings`. The complete registered-family
SVG comparison passes structure, parity, and parity-root after the single reviewed
Class receipt refresh. No comparator implementation or threshold changes are made.
