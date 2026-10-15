# Sequence control frame and keyword background

Date: 2026-09-19. Parent: `dde65b008`. Status: bounded increment verified; full U7 and C7a remain open.
This record does not close U7, preset qualification or C7a.

## Ownership correction

The previous unpublished `Loop` paint target wrote the keyword polygon. Current authors now
use `LoopLabelBackground` for that surface, while `Loop` owns the four control frame lines and
any section separators. `LoopLabel` continues to own text. There is no compatibility alias,
new wire shape, dependency, font or protocol version. Discovery adds the Sequence-only known
target; generic string-based transports use the shared semantic inventory.

Frame and polygon keep independent receipts and scoped filters. The real writers supply
coordinates and observe successful emission after their output checkpoints. Unsupported frame
fill, polygon radius and other unconsumed siblings remain residual. Source-owned colors do not
suppress independent width/effect facets. Effect Clear restores the existing baseline rather
than imposing a new source-style override.

Keyword bounds include all emitted polygon vertices, including the leftward corner at a
configured width below 8.4px. Their stroke envelope accounts for acute miter joins; frame lines
retain the half-stroke envelope. Default/no-effect separator attributes avoid a new allocation.

## Public recipe and resources

Cyberpunk adds a 2px cyan control frame with sigma-4/alpha-.5 glow and a 2px keyword border
with sigma-6/alpha-.4 glow. Frame and XY Legend use identical sRGB shadows, so they share the
`cyberpunk-cyan-soft-glow` graph while retaining independent terminal applications. The initial
literal implementation had 17 graphs and was correctly rejected by the 16-graph Interactive
limit. Reusing the identical graph preserves both visuals and the existing budget.

All ten shared palette recipes move their old box rule to the background target. Exact catalog
fingerprints are refreshed from compiled output, not copied from old receipts. Current KTD17
moves eight box authorization rows one-for-one, preserving projection bits, contribution IDs
and witness obligations. Historical KTD23 files remain unchanged. The internal authorization
version advances from 89 to 90; the public theme schema remains v1.

## Verification

- The initial frame-effect regression failed with `IncompleteFamilyTheme` before implementation:
  `/tmp/sequence-control-red.log`.
- First renderer run: 2,907/2,909 passed, three skips. Only catalog identity assertions failed
  after the target rename; the new real frame/separator regression passed.
- Second run exposed the 17-graph resource rejection; control isolation/Clear and unsupported
  sibling regressions passed. These results do not certify the later resource correction.
- A read-only authority check confirmed exactly eight current route relocations and unchanged
  historical owners: `/tmp/sequence-control-authority-check.md`.
- A separate draft check identified the narrow polygon bounds issue. Its source-backed
  counterexample is covered by a new regression; no pre-fix runtime failure is claimed for it.
  `/tmp/sequence-control-draft-check.md`.
- Simplification ran all three full rubrics serially in one independent Codex context:
  `/tmp/sequence-control-simplify.json`. Its short-circuit route scan recommendation is applied.
  This is not a three-reviewer consensus or a measured performance improvement.

- Third renderer run passed 2,911/2,913 tests with three skips; only the exact Cyberpunk
  fingerprint checks remained. Updating from the compiler produced the final Release result:
  **2,913/2,913 passed**, three skips (`/tmp/sequence-control-renderer-final.log`).
- Review identified a weak narrow-polygon assertion: it used three sigma while shared lowering
  reserves four, allowing the old missing vertex to hide inside the spare margin. The test now
  requires a genuinely outlying vertex and includes the four-sigma/miter bounds. The subsequent
  focused Release run passed **6/6** (`/tmp/sequence-control-focused.log`). No executed mutation
  failure is claimed.
- Formal review ran eight lenses serially in one independent Codex context and retained no
  findings: `/tmp/compound-engineering-501/ce-code-review/20260919-151123-1fd30175/`. Its
  initial Not ready verdict reflected the then-pending parent-owned verification below, not
  a completed U7 or C7a gate.

- Native first run passed 18/19. The new control fixture's `Loop` sRGB red sample count
  was exactly 30 against the unchanged `>30` oracle, while its default blue surfaces polluted
  the blue count. Neutral source colors removed that pollution but retained the same red count.
  A diagnostic PNG showed the actual thin dashed shadow. The fixture now includes both an
  `alt` and a simple `loop` (nine frame terminals, two keyword polygons), preserving the
  thresholds, SourceGraphic reset-red-zero negative check, both color spaces and native
  receipts. The final native Release suite passed **19/19** (`/tmp/sequence-control-native.log`).
  The diagnostic alone is not a passing test or a general visibility guarantee for every thin line.

## Local export and browser observations

Fresh Cargo artifact JSON supplied the facade rlib used by `native-capture.rs`; its command,
rlib hash and successful execution are recorded in `capture-provenance.json`. Experiments:
`target/bench/experiments/sequence-controls-dde65b008/`. Current Cyberpunk recipe fingerprint:
`ebcd233e9d77256cbb490f3ed107f3fc441dad13638b420006d8e2cd23eb565b`.
The fixed public scene has **21 filters / 21 references / 29 shadow stages**.

The public preset, zero-margin 12px/narrow-polygon stress scene and its Clear counterpart all
exported SVG/PNG/PDF. PNG/PDF admission retained only `HostDependent/SystemOrHostFontDependency`;
there was no incomplete theme evidence, native filter mismatch or unlocalized PDF filter.
Chromium **151.0.7922.34** confirmed public 2px cyan frame/keyword borders, dark keyword fill,
frame sigma-4/alpha-.5 and keyword sigma-6/alpha-.4 shadows. Clear removes the requested
shadow without removing the independent authored stroke width.

Expanded transparent captures retain actual terminal geometry/styles. At alpha >=32 with a
one-pixel allowance, the public/stress/Clear samples had **2,583 / 20,283 / 6,477** painted
pixels and **zero viewport escapes**. Removing only the actual filter references changed
**7,635 / 28,238 / 0** alpha samples. No sample touched the diagnostic capture boundary.
Browser, native PNG and PDFium public outputs, plus the native stress output, were visually
inspected. PDFium provenance and 96dpi raster dimensions are in `pdfium-observations.json`.
These observations do not assert reference-identical fonts/layout or all-host portability.

The native test fixture received a narrow independent follow-up in
`control-native-fixture-addendum.json` under the formal review directory. It confirms nine/two
unique filter definitions and valid references for the two fixture targets; the parent owns
the later successful runtime. The review's earlier verdict is preserved, not rewritten.

## Authorization reconciliation

The initial focused authority run passed the inventory and bounded projection checks; only
its expected old digest failed (two/three passed). After the independent eight-row inventory
review, manifest 90 was refreshed to the runtime-derived digest
`73ed3fec108884181557db1b560040fcf2401e4c5c0107b23bc78bbaddf7190c`.
Route/witness counts remain **506/814**, with **57** historical projection channels. Logs:
`/tmp/sequence-control-authority.log`; the full relevant run passed **130/131**. Its sole failure
was a stale test expectation
that the simple Sequence admission fixture still lacked an actor effect consumer. The current
fixture is only Alice/Bob messages (no control frame/note); its valid HostDependent result
cannot qualify the complete Cyberpunk scene. After removing that stale residual expectation,
both preset qualification tests passed **2/2**. Public cells remain empty, and the existing
Cyberpunk palette-only qualification still fails as required. See
`/tmp/sequence-control-acceptance-final.log` and `/tmp/sequence-control-qualification-final.log`.
This last test adjustment received a narrow independent review; no production acceptance
policy was changed. The addendum is `control-qualification-addendum.json` under the formal
review directory.

## Final regression checks

No-embedded-font Release tests passed **7/7**, including public recipe exchange, real control
filters, source/Clear independence, unsupported siblings and the narrow polygon. Log:
`/tmp/sequence-control-no-font.log`.

Shared binding discovery passed **1/1**, explicitly including `loop-label-background` in
the generated semantic catalog (`/tmp/sequence-control-metadata.log`).

Default Sequence parity completed **322 selected / 320 rendered / two math skips**, with
**960** DOM comparisons and the same **12 exact** accepted browser text-layout residuals.
The report is copied into the experiment directory. Scoped Release Clippy, workspace
formatting and diff whitespace checks passed; Clippy is not warning-free. Commands, exit
codes and timings are in `/tmp/sequence-control-remaining.json`.

The shared shadow graph also passed **4/4** existing public XY tests: English/Chinese role
paint and glow, both orientations, recipe export/import, kind/ordinal selection, native
receipts and the default full-scene conversion budget (`/tmp/sequence-control-xy-recipe.log`).

The original review receipt remains unchanged. All parent-owned local gates listed in that
receipt are now addressed in this record; later fixture and qualification changes have narrow
review addenda. No unresolved source finding remains for this increment. The small additional
frame CSS string copy has not been measured as a performance defect; total cost remains part
of U10. No dependency, feature, font or public qualification expansion was needed. Installed consumers and the full
platform delivery matrix remain outside this increment's completed evidence.
