# Literal text and styled bounds

This follow-up to [the font audit](2026-10-27-text-audit.md) repairs the
measurement/output divergences left open in
[the preceding geometry batch](2026-10-27-font-geometry.md).
The main baseline remains Mermaid 12.2.0; the maintenance adaptation retains
Mermaid 12.1.0 and its existing configuration and transport versions.

## The shared contract

Label source, prepared HTML, and final SVG text content have different meanings.
An entity that produces the characters `<br/>` in a final text node must not
become a line break during measurement. A styled run must retain its final font
when its ink bounds contribute to the viewport.

The existing measurement interface already distinguishes label layout, Canvas
advance, direct SVG text, and child tspan probes. The deterministic and width
callback profiles now honor these distinctions for literal scalar widths,
instead of routing them through generic label normalization.

- Ordinary direct SVG text collapses SVG whitespace.
- Prepared inline fragments retain connecting spaces for computed length and
  horizontal extents. Existing incremental/full-run equivalence tests include
  spaces, tabs, line feeds, literal markup, and checkpoint restoration.
- Preserved raw text and preserved tspans keep consecutive spaces. TreeView
  uses the former because its emitted stylesheet specifies `white-space: pre`.
- Canvas receives literal text with ASCII tab/newline controls mapped to spaces.
  Its existing soft-hyphen shaping behavior is retained.

The additive preserved-raw trait method has a compatibility default. Routed
hosts use existing operation 6 (`RawBBoxWidth`) with `HtmlLike` preservation and
no wrap budget. Bindings project that request to existing `BreakSpaces`.
Prepared ComputedLength/BBoxX requests likewise preserve spaces through the existing
HtmlLike/BreakSpaces route. Native and real Chromium controls verify this route.
No operation number, wire record, or protocol version is added.

## Consumer repairs

| Consumer | Previous divergence | Repair |
| --- | --- | --- |
| XYChart | Direct title/axis text was measured as generic labels; literal break spellings could create phantom rows. | Use the client-rect width operation and a single-row raw height probe. The maintenance line also shares the stylesheet's configured family with every probe. |
| Architecture | Final service bounds flattened bold/italic runs into the base font and reused the first row's vertical facts. | Preserve run fonts, position mixed runs by advance, and union each row's ink extents and vertical offsets. Keep Cytoscape compound measurements separate. |
| Public HTML helper | The tag scanner stopped at quoted `>` characters and consumed literal comparison signs as tags. | Recognize quoted/unquoted attribute boundaries, comments, declarations, literal `<`, and unfinished tags before the existing font/entity logic. |
| C4 | Prepared, decoded rows were joined and parsed as labels a second time. | Measure final rows directly and union their indexed vertical bounds, retaining empty-row positions. |
| Treemap | Final direct title bounds used `drawSimpleText` helper semantics. | Select the raw direct-text operation. |
| Usecase | Plain labels measured escaped source while their output displayed decoded text. | For HTML, measure the emitted paragraph fragment. For SVG, follow the writer's source tokenization, then decode final tspan content once. |
| State HTML | The simple-label shortcut reinterpreted an entity-produced `<br/>` spelling. | Keep the prepared XHTML fragment on the structured helper path when the decoded candidate contains `<`. |

The HTML scanner is a private boundary recognizer, not a new HTML or CSS engine.
No dependency or feature graph changes are required by this batch.

## Regression evidence

New public-consumer regressions cover literal XYChart titles, seven Architecture
service-bound cases, seventeen HTML helper cases, Treemap raw-title bounds, four
Usecase entity projections, and State literal entity projection. A C4 row test
checks width, height, and row count after decoding.

The XYChart literal probe and HTML helper cases were run against the preceding
implementations and failed before repair. The HTML batch had thirteen failures
and four already-passing controls on each line. The maintenance XYChart font
test also exposed the missing stylesheet-family connection. Existing Usecase
font oracles now implement the actual Canvas/tspan operations; their original
font formulas and geometry assertions are retained.

Nine main layout snapshots and eight maintenance snapshots change after source/DOM review: ordinary SVG space
collapse in Sequence, Timeline and XYChart; structured HTML quantization in
State and Usecase; and actual tspan height in the ELK Usecase guard. The ELK
fixture retains its independent clearance and routing assertions. TreeView's
preserved-space snapshot is unchanged.

## Timeline receipt review

The Industrial Revolution Timeline fixture keeps the same final tspan text,
including `Internet,   Robotics,   Internet   of   Things`. Its stylesheet does
not preserve these spaces. Chromium 156.0.8078.4 reports `white-space: nowrap`;
replacing the three-space runs with single spaces leaves both bbox width and
computed length unchanged. Explicit `pre` increases the upstream width from
267.58087158203125 to 306.1125183105469 CSS pixels, and the local width from
267.734375 to 306.296875. Thus the previous deterministic expansion was incorrect.

After collapse, the local viewBox starts at x=95 with width 745, matching the
upstream horizontal viewport. Its remaining height and line-wrap differences
retain the reviewed font-measurement approximation. Independent review and the
complete main paint audit confirm no new viewport blocker.

Only this fixture's local canonical DOM signature is refreshed in each
browser-text-layout catalog. Input and upstream hashes, admitted modes,
three-decimal precision, comparator policy, and root-viewport residuals remain
unchanged. This is an exact artifact-bound refresh, not a new tolerance.

## Visible examples

[The comparison figure](assets/text-fixes-20261027/comparison.png) contains
actual native CLI exports from clean parent `cba3033d8` and the repaired source.
The adjacent HTML, diagram inputs, configurations and provenance describe how
to reproduce it. Both columns use the same macOS Arial files and drawing scale.
The screenshot adds no crop and changes no exported SVG geometry.

The XYChart title anchor moves from 238.03125 to 219.1455078125 in the same
360 by 240 viewport. Architecture's bold service-label viewport grows from
106.6875 to 117.328125 units to include the emitted bold glyphs.
These examples illustrate main; the maintenance adaptation is tested separately.

## Remaining scope

Usecase plain SVG currently follows Merman's non-wrapping writer. Both pinned
upstream `createText.ts` implementations also call `splitLineToFitWidth`, so
plain SVG automatic wrapping remains a separate parity gap. This batch does
not claim to implement it by changing measurement alone.

The shared Markdown word-line helper's height projection remains a
source-traced candidate for entity-produced literal break spellings. It needs
a consumer-level reproducer before any further implementation change.

Browser font fallback, dynamic CSS, and platform-specific paint remain bounded
host concerns. Performance optimization remains paused; correctness tests and
the comparison figure are not performance qualification.

## Final validation

The complete renderer suite passes with
`all-diagrams,layout-elk,layout-cytoscape,math`: 5,409 main tests (five existing
skips) and 2,507 maintenance tests (two existing skips). Both lines pass strict
renderer-library Clippy, nine native font primitive tests, twelve binding/WASM
projection tests, and the real Chromium DOM typography/space controls. All
three release-mode DOM comparison policies pass across all 37 families.
Native ABI, binding-contract and text-measurement protocol generated
projections are current. Main has seven Rust root contracts and twenty
Chromium oracle contracts; maintenance has ten and seventeen respectively.

The complete main paint audit checks 3,737 SVGs on darwin-arm64 with Playwright
1.64.1 / Chromium 156.0.8078.4. It has zero blocking failures, 54 browser-owned
diagnostics, 2,361 upstream-inherited results, one matched exact residual and
no unused residuals. All 3,737 final regenerated SVGs match the audited bytes.
The bounded captures retain 2,302 local indeterminate results; this is not an
all-contained claim.

The isolated maintenance paint audit includes all 3,720 live baseline pairs.
It has one blocking result: the pre-existing environment-bound Flowchart title
029 receipt still differs on darwin-arm64 while both SVG hashes match its
catalog. The XYChart extrapolation receipt matches unchanged after retaining
the existing unrelated neo stylesheet rules. No root receipt or epsilon is
modified. The Linux owner must validate its own browser environment; the full
macOS maintenance paint command is not claimed as passing.

The final native CLI rebuild produces exactly the two after-SVGs used by the
comparison figure. Independent review closes the styled bounds, prepared
spaces, HTML boundary scanner, maintenance family and Timeline receipt issues
without a remaining production blocker in this batch.

Current-head Linux workspace, assembled Web packages, full platform binding
verification and Android/Apple consumers remain owned by the selected PR CI
jobs. Earlier-head CI success is not current-head evidence. The broader SDK
matrix is not rerun locally for the unchanged ABI; this batch's changed
transport seam is covered by the routed/native/WASM/Chromium checks above.
