# Sequence LoopLabel effects and the public Cyberpunk recipe

Date: 2026-09-19. Base: `c515ca879`. Status: bounded increment verified; U7 remains open.
This is a U7 increment, not full Sequence or C7a qualification.

## Scope and reference

The reference observation in [the Sequence text record](2026-09-19-sequence-text-reference.md)
identifies cyan alpha .5, sigma 5 (CSS text-shadow blur 10px) on the fixed scene's actual
`labelText` and `loopText`, with weight 400. The public recipe now requests that effect through
Sequence LoopLabel. The semantic role also covers alternative/critical section titles; this
uniform extension is not a claim that the reference CSS explicitly styles `.sectionTitle`.
No new public target, dependency, bundled font or unpublished version is introduced.
The exact compiled recipe fingerprint is
`8884217861d6a00d29192323e0011675e413175df9a9e620809fe0776750b9f7`.
No qualified catalog cells were promoted.

All three existing LoopLabel surfaces participate: control keyword, primary title and section
title. Their real writers materialize shadows from final text and actual positions. The shared
Sequence text-shadow helper retains the distinct middle/Note-dy/alphabetic baselines and role
IDs; wrapping is not repeated. The existing deferred root combines Note and Loop paint bounds.
Clear retains ordinary bytes. Empty title rows retain layout but no filter, while the control
keyword still paints. Math branches, missing layout and unsupported siblings cannot establish
complete effect consumption. Source color and typography ownership remain independent.

The first integration test failed before production edits with `IncompleteFamilyTheme`.
The first wider Renderer run passed 2,847/2,849 (three skips); both failures were the
expected catalog fingerprint drift. The catalog was updated from the compiler result, not
from a hand-authored recipe hash. Final checks are recorded below.

## Verification

- Native Release composed effects: **15/15 passed**, serially, including both color spaces and
  SourceGraphic versus Previous inputs on all three LoopLabel surfaces. Public Cyberpunk now
  produces 10 actual filters/references and 14 shadow stages. Earlier Note/actor/message/marker
  and nonempty zero-width measurement tests remain passing.
- Five fresh native facade cases (public recipe, root font sizes 16/48 with and without Clear)
  exported SVG/PNG/PDF. All exports retained HostDependent/SystemOrHostFontDependency; none
  reported incomplete theme, mismatched native filter receipts or unlocalized PDF filters.
- Chromium 151.0.7922.34 observed actual text size, weight, leaf color and filter references.
  Expanded transparent captures (300px margin, checked for paint touching the probe boundary)
  found no paint outside the production viewport in all three
  glow scenes (alpha >=32, one-pixel allowance). Removing only LoopLabel filter references
  changes visible pixels; Clear changes none. See local browser observations for exact counts.
- The 48px Clear stress scene retains existing no-effect clipping (1,653 pixels). Separately
  rendered fill-only baselines are byte-identical. This proves no effect/Clear regression;
  it does not establish that ordinary layout is correct. The first 100px-margin probe observed
  only 1,573 pixels because its capture also clipped the far right of the line; the corrected
  300px capture measures 1,653 and asserts that paint does not reach the probe boundary.
  The affected primary title is `[中文长标题 Long title`. It extends beyond both horizontal
  edges. Source inspection finds that explicit `<br>` short-circuits wrapping, block layout
  uses the measured title height, and ordinary root bounds include frame/keyword geometry
  rather than this long line's actual horizontal extent. This is not established to be merely
  a host-font error. A same-input browser check with installed reference Mermaid **11.12.1**
  also clips the primary line: root `[0,0,350,1015]`, text bbox x=-39.5078125..439.5078125.
  Its anchor is 200 versus Merman's 217.5, so this is not pixel equivalence. It establishes
  a shared boundary in that reference version, not a branch regression or proof about the
  project's current pinned baseline. Check the pinned upstream policy before changing ordinary
  root/layout behavior; do not add arbitrary padding or claim this residual is fixed.
- Actual public Chromium, PNG and PDFium-rasterized PDF views were inspected. PDFium uses
  the existing local library at 96 dpi; its path/hash are captured with the observations.
  No Web/WASM bundle or installed package was rebuilt by this experiment.

- Final Renderer library, Sequence SVG and shared theme-resolution Release regression:
  **2,849/2,849 passed**, three skips, after fingerprint refresh and simplification.

- No-embedded-fonts Release SVG check: **6/6 passed**, including existing Loop fill ownership
  and Note effect regressions; 92 cases outside the filter, math intentionally disabled.

- Full default SVG corpus: **35 groups passed**, structure/parity/parity-root at three decimals
  with exact browser-text-layout diagnostics. Sequence rendered 320/322 (two external-math
  skips), with 960 comparisons and the same 12 exact residual comparisons. No oracle changed.

- Scoped Release Clippy completed with existing warnings, not warning-free.
  `cargo fmt --all -- --check` and `git diff --check` passed.

## Remaining ActorLabel work

Read-only source inspection found four obligations, not just ordinary participant labels:

- `actor_shapes.rs`: decoded/wrapped text at `cy + tspan.dy`, central baseline.
- `actor_man_glyphs.rs`: boundary has parent y translation 21; entity top/bottom use 6/22.
  Filter geometry stays local, while root paint bounds must include the parent translation.
- `frames.rs`: BoxTitle already belongs to the Actor typography role.
- `actor_popup.rs`: left-aligned link text may have a hidden ancestor. It is not paintless text;
  native removal of hidden groups must not be counted as a surviving effect application.

Math switch fallback must not certify the actual foreignObject branch. Clear, mirror-disabled
bottoms and missing terminals need their own correct consumption accounting. Do not move DOM
or add a generic drawable/proof engine merely to avoid those placement and visibility rules.

Full ActorLabel/Loop frame/lifeline/activation work, public scene qualification, other-theme
coverage, final installed candidate and impact measurements remain open.

## Local evidence

Artifacts: `target/bench/experiments/sequence-loop-text-c515ca879/` (capture source,
rlib SHA-256 and command, five SVG/PNG/PDF cases, separately rendered Clear baselines,
browser probe/observations and PDFium probe/observations).
The bounded clipping investigation is `baseline-clipping-diagnosis.json` in that directory.
`verification-source.json` records the tested source and artifact SHA-256 inventory;
`sequence_report_parity_root.md` preserves the default Sequence corpus result.
Logs: `/tmp/sequence-loop-text-red.log`, `/tmp/sequence-loop-text-renderer.log`,
`/tmp/sequence-loop-text-native.log`, `/tmp/sequence-loop-text-renderer-final.log`,
`/tmp/sequence-loop-text-no-font.log`, `/tmp/sequence-loop-text-parity.log`,
`/tmp/sequence-loop-text-clippy.log`.

Simplify used three full rubrics serially in one independent Codex context. The sole finding
removed the trivial eight-argument LoopTextRenderContext constructor in favor of named fields
at its sole caller. Receipt: `/tmp/sequence-loop-text-simplify-c515ca879.json`.

Formal `ce-code-review` completed seven serial lenses in one independent Codex context,
with no retained source finding. Receipt:
`/tmp/compound-engineering-501/ce-code-review/20260919-124435-03c4e64d/`.
Its initial Not ready verdict explicitly awaited parent runtime/lint verification; the checks
above provide that evidence. No external model or seven-reviewer
independence is claimed. Existing CI and release-preflight already select `sequence_svg_test`
and `theme_composed_effects`; no additional CI job or duplicate script was added.
