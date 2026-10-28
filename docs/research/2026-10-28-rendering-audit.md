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

Maintenance's integrated renderer run passes 2,544 tests and exposes two remaining
controls: the Class escape-token defect and an actor-label entity decode repeated
by its active text writer. Both are repaired. The resulting full library plus
Class, Block HTML, and Sequence integration run passes all 1,820 tests (one
intentional skip). CLI math passes all seven tests, including actual formula ink,
fraction bars, radicals, and CSS paint controls. The full SVG structure/parity/
root comparison completes successfully across every registered family, using
the existing exact browser-text-layout residual receipts and unchanged rules.

The [direct CLI comparison](assets/rendering-audit-20261028/comparison.html) and
its [screenshot](assets/rendering-audit-20261028/comparison.png) expose three
maintenance before/after controls. They are line-specific correctness evidence.

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

## Maintenance final adaptation

All 1,833 final library, Class SVG, Block HTML, Sequence SVG and layout snapshot
tests pass on `84f971b23`. Strict renderer Clippy with all targets and diagram,
ELK, Cytoscape and math features passes under `-D warnings`; cargo fmt passes.
The existing maintenance font declaration resolver is extended with distinct
inherited/element defaults and a root-font context for Block's retained terminals.
It is not replaced by main's theme cascade architecture. The two new literal
HTML-height routes use an existing measurement operation and preserve defaulted
Rust trait compatibility; no transport operation or wire version changes.

Maintenance's final SVG matrix identifies the same single reviewed Class local
signature change as main. The pinned 12.1 title likewise uses `translate(0,-14)`;
the repaired local artifact has the same canonical signature as main. Refresh
only this existing receipt's local signature, keeping input/upstream hashes,
modes, precision, comparison policies and tolerances unchanged.

The final maintenance SVG comparison passes all 37 registered families in
structure, parity and parity-root modes. The final CLI rebuild supplies the
adjacent after exports; those files bind the exact committed renderer source and
binary rather than relying on the earlier staged working copy.
