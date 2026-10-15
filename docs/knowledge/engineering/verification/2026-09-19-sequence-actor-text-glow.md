# Sequence ActorLabel effects and the public Cyberpunk recipe

Date: 2026-09-19. Base: `195d783d8`. Status: bounded implementation and verification complete.
This is a U7 increment; it does not close Sequence scene qualification or C7a.

## Contract and drawing ownership

The [reference observation](2026-09-19-sequence-text-reference.md) identifies two ordered
cyan sRGB shadows (sigma 8/16, alpha .5/.3) on actual participant text, at weight 400.
The public recipe reuses its existing shape graph for ActorLabel; no graph, font asset,
dependency, public target, unpublished version, or qualified cell is added.
ActorLabel also includes box titles and participant popup links. Those semantic surfaces
receive the same rule; this is not a claim that every such surface occurs in the fixed reference scene.

The existing private ActorLabelContext carries the role's actual typography, measurement,
shadow and receipt to the writers. Ordinary participants, collections, queues and databases
use decoded final lines and the existing `cy + dy` placement. Actor/boundary/control/entity
labels retain their existing text and placement. Their filter regions remain local; aggregate
root bounds also account for the actual parent group's y translation. Box titles keep their
existing middle anchor, and visible popup links use their actual start anchor. Writers with
historical 16px terminal fallback measure that same terminal size when typed typography does
not require resolved emission. The default path creates no label cache or additional wrapping.

Hidden interactive popup links cannot establish a surviving native effect. They retain an
incomplete effect obligation and do not emit filters; a forced-visible menu can consume the
request. Explicit Clear remains consumable for hidden menus and prepared math. Math fallback
text does not certify a filter on the actual foreignObject. Missing placements retain an
unconsumed candidate, and ordinal selectors and unsupported sibling facets retain residuals.

## Verification

The new all-shape/box/menu strict regression failed before implementation with
`IncompleteFamilyTheme`; log: `/tmp/sequence-actor-text-red.log`.
The first runtime pass caught malformed filter-attribute serialization; it was corrected
using the existing escaped `filter="url(...)"` emission convention.

First Renderer Release run: **2,849/2,852 passed**, three skips. The three failures were
exact recipe identity drift (catalog fingerprint and complete-spec round trip) and the previous
shape-graph target list, which now also includes ActorLabel. All newly added effect behavior
tests passed. Catalog identity was updated from the actual compiler result:
`cebe312092bd0070e2915139e80a3b77211b21143a343f5767416c6cc5e20550`.

Simplification applied three full rubrics in one independent Codex context. Its single quality
finding removed a duplicate translation match: the glyph writers now pass their actual group
translation into the text receipt. Receipt: `/tmp/sequence-actor-text-simplify-195d783d8.json`.

Native Release composed-effect tests: **16/16 passed**, serially, including both color spaces,
SourceGraphic versus Previous, every actor shape, translated glyphs, box titles and forced-visible
links. Public Cyberpunk produced **14 filters / 14 references / 22 shadow stages** in both PNG
and PDF under the existing default budget. The broad actor test is split into three bounded
scenes (11, 8 and 3 terminals); no conversion ceiling changed. Its first pass used pale default
actor surfaces and failed the helper's absolute red-pixel threshold. Explicit black actor fill
and white text make the intended red/blue composition visible; all existing thresholds and
SourceGraphic negative assertions were retained.

Five fresh facade cases (public preset plus 16/48px actor text with glow and Clear) exported
SVG/PNG/PDF from the native run's rebuilt rlib. All retained
HostDependent/SystemOrHostFontDependency and none reported ThemeEvidenceIncomplete,
NativeFilterReceiptMismatch or PdfNativeFilterNotLocalized. Clear matched separately rendered
fill-and-font-size-only baselines byte for byte.

Chromium **151.0.7922.34** observed actual leaf fill, size, weight and filter bindings. The
transparent expanded-viewport probe retains parent transforms; alpha >=32 and a one-pixel
allowance found no viewport escape in the public and two stress glow cases. Captures also
assert no paint touches the diagnostic viewport boundary. Removing only ActorLabel filter
references changed 6,830 / 10,737 / 51,057 visible alpha samples respectively; Clear changed
none. The 16px Clear case has no overflow. The 48px Clear case has 154 overflow pixels;
it is byte-identical to the ordinary baseline. The isolated overflow is the bottom mirrored
participant A's third line, `中文`: actual center y=272.5, ink to y=294.5, viewport ends at
287. There is no parent transform; the other 20 text terminals do not escape. The ordinary
non-wrapped actor keeps fixed height while three 48px rows use +/-48 offsets, and the root
tracks footer shapes rather than the final line's ink. This remains a layout boundary,
not a claimed fix or a demonstrated new effect regression. See
`baseline-clipping-diagnosis.json` in the artifact directory; no upstream equivalence was
asserted for this separate stress case.

Actual public browser, PNG and PDFium views were inspected. PDFium rasterization uses the
existing local library at 96dpi, with library path/hash captured in the artifact record.
This does not establish full reference equivalence, packaged Web/WASM behavior or qualification.

Final Renderer Release library, Sequence SVG and shared theme-resolution regression:
**2,852/2,852 passed**, three skips, after the exact fingerprint refresh and simplification.
Log: `/tmp/sequence-actor-text-renderer-final.log`.

No-embedded-fonts Release SVG check: **8/8 passed**, including ActorLabel, NoteLabel and
LoopLabel effects; 92 tests outside the filter, math intentionally disabled.
Log: `/tmp/sequence-actor-text-no-font.log`.

Sequence default corpus comparison passed: **322 selected, 320 rendered, two external-math
skips, 960 DOM comparisons**, retaining the 12 exact browser-text-layout residual comparisons.
No baseline or normalization policy changed. This increment checked Sequence, not the full
family matrix. Report: `sequence_report_parity_root.md` in the artifact directory.
Log: `/tmp/sequence-actor-text-parity.log`.

Scoped Release Clippy passed with existing warnings; this is not a warning-free claim.
`cargo fmt --all -- --check` and `git diff --check` passed. Commands, exit codes and logs:
`/tmp/sequence-actor-remaining-checks.json`. Existing installed candidate/platform evidence
is historical.

Artifacts are scoped to `target/bench/experiments/sequence-actor-text-195d783d8/`.
The native capture contract identifies its exact rlib SHA-256 and rustc command.
Logs: `/tmp/sequence-actor-text-red.log`, `/tmp/sequence-actor-text-renderer.log`,
`/tmp/sequence-actor-text-native.log`, `/tmp/sequence-actor-text-native-final.log`.

Formal code review completed eight lenses serially in one independent Codex context, without
retained source findings. Receipt:
`/tmp/compound-engineering-501/ce-code-review/20260919-132947-647dc79b/`.
Its initial Not ready verdict was conditional on the parent's final runtime/parity/lint checks;
those checks subsequently passed as recorded above. No external model or eight-reviewer
independence is claimed. `verification-source.json` in the artifact directory records the
parent commit, exact source-file hashes, artifact hashes and verification counts.

## Remaining scope

Full Sequence lifeline/frame/activation consumers, final scene comparison, other presets,
installed consumers and impact gates remain open. The prior LoopLabel record's long-title
clipping boundary is unchanged and is not claimed fixed by this increment.
