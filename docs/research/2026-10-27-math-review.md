# Math follow-up adversarial review

Date: 2026-10-27; follow-up qualification continued on 2026-10-28. Reviewed heads: main `d14a21da5`, maintenance `c3eeba6df`.
Three independent Codex reviewers examined backend vocabulary, native evidence
and resource lifetimes, maintenance admission, and the validity of the raster
oracles. A fourth independent review checked the integrated candidate bounds
and Sequence fixes before final verification. The main agent reproduced and integrated the findings.

## Confirmed counterexamples and repairs

| Finding | Reproducer / decisive check | Repair |
| --- | --- | --- |
| Maintenance rejects a real bitmap glyph | `flowchart LR` with `A["$$\text{😀}$$"]`; parity succeeds, native-safe rejects `image` | Admit the actual bounded PNG vocabulary and canonical paint; retain cancellation, generated limits and terminal image/reference accounting |
| User class collides with internal instance marker | `A["$$x$$"]:::merman-prepared-math-native`, or that class on a neighboring plain node | Bind instances to renderer-owned occurrence evidence before checking their class; expected counts still reject removed genuine markers |
| Resolved fallback ink exceeds nominal layout | `\smash{\text{中文}}` emits nonempty paths into a zero-height viewport; emoji bitmap extends below a 0.74em viewport | Combine the logical display box with emitted geometry of missing-metrics glyph candidates; measurement and rendering share the result |
| Browser evidence can pass invisible formulas | Delete native `use`, or hide the real `foreignObject` | Require a visible pixel contribution in the complete emitted SVG for each formula |
| Shared placement errors cancel in a CSS-only oracle | Translate only native formula instances by 20px | Compare visible formula bounds from independent full parity/native outputs, retaining a 1 CSS pixel raster-edge allowance |
| Sequence size and alignment differ between pipelines | Root `svg` font rule overrides prepared role size; native projector adds centering absent from Mermaid's `drawKatex` | Formula SVG inherits its prepared font size; Sequence native content uses the original foreignObject origin |

The class collision, missing bitmap admission and fallback dimensions predate
the preceding paint fix. The browser evidence defects are flaws in its new
verification, not proof that the normal fixtures were invisible. The repaired
oracles must reject both disappearance mutations and the 20px displacement.

## Narrow dimension ownership

Only glyphs missing exact RaTeX font metrics are candidates. Mathematical
alphabet Unicode is mapped to the backend's actual KaTeX glyph slot first.
A direct, optional `ratex-font` dependency exposes the same pinned metrics; it
adds no font discovery or system API and is enabled only by `math`.

A candidate-only call to the same RaTeX SVG emitter supplies actual path or
bitmap bounds. Existing `kurbo` measures those paths. The complete formula's
emitted geometry is preserved; the viewport origin can be negative. Vertical
bounds contain candidate ink. Horizontal recovery applies only to a near-zero
original layout root, retaining intentional nested lap clipping. Ordinary
KaTeX and structural paths stay on the upstream display-list policy, including
its large-path exclusions. No general font resolver, CSS engine or replacement
SVG serializer is introduced.

If a glyph still emits `text` because the backend has no self-contained glyph,
its host-dependent font bounds remain unresolved. This repair does not invent
metrics or claim coverage for unavailable system fonts; the supplementary CJK
U+20000 counterexample remains a documented residual. Main's mixed native math
and unwired Class/Mindmap native projections retain their explicit boundaries.

## Source-backed Sequence behavior

Both pinned Mermaid 12.1.0 and 12.2.0 `drawKatex` create a `width: fit-content`
XHTML div, measure it, and position the foreignObject around the message or
note. They do not center content inside that foreignObject. `drawText`'s
vertical alignment is a different path. Flowchart and Swimlane retain their
centered shells.

The final integration pass also caught the maintenance root-style whitelist
missing the producer's new fixed inheritance declaration. Admission now accepts
only that exact root declaration and tests the real formula producer directly,
including rejection of altered declarations. An integer micro-unit bounds
accumulator prevents a second floating-point ceil from adding 0.000001em to
already-rounded normal logical dimensions (`0.125008em` regression).

## CI triage

The main reviewed head's Linux build-test job failed while downloading
`fonts-noto-cjk`, before tests, after the existing ten-minute installation
limit. The failed job was rerun successfully; the reviewed main head now has 68
successful and 7 intentionally skipped checks. No source assertion or test
gate was relaxed.
Maintenance's reviewed head has 59 successful and 4 intentionally skipped
checks. New source commits need
fresh CI; earlier heads are not qualification for the new revision.

## CI integration follow-up (2026-10-28)

The subsequent complete-SVG verification heads (main `f1edead9d`, maintenance
`d80499a70`) exposed stale dependency records. All seven fuzz jobs on each
branch stopped at the locked workspace check, before replaying any corpus:
`fuzz/Cargo.lock` lacked the optional direct `merman-render -> ratex-font` edge.
Refreshing that single local-package relation leaves every registry package
version and checksum unchanged. Both branches now pass the workflow's pinned
`nightly-2026-07-01` workspace check with `--locked --offline`.

Main also refreshes the three current native-memory owner contracts' `Cargo.lock` input hashes.
Only the current source-input hashes change; thresholds, semantic digests,
candidate-admission rules and historical evidence remain intact. The complete
performance-owner Python checks pass locally on both branches (157 focused
contracts on main, 121 on maintenance).

Local verification also exposed macOS `/var` versus `/private/var` aliases in
test fixtures. Canonicalizing fixture paths fixes command-provenance assertions
without relaxing production receipt validation. New remote CI results belong
to the follow-up commits, not the previously reviewed heads.

## Verification record

Source repairs: main `19518166e`; maintenance `5fa43519a`.
`review-provenance.json` binds the twelve branch exports to source inputs,
argument lists, binary identity and SHA-256 values. These are source-checkout
results; no package release or tag was published.

The implementation runs Cargo sequentially with two build jobs and the shared
existing target. Old compiled caches were removed after a disk-full linker
failure; no source or working-tree changes were discarded.

- Main full renderer: 5,420 passing tests, five existing skips. Maintenance:
  2,520 passing tests, two existing skips. Layout goldens are unchanged.
- Both CLI math suites: seven passing tests. The HTML-label paint branch is
  enabled explicitly; a separate top-level-plus-Flowchart false control checks
  the existing literal SVG-text branch. The formerly redundant false round is
  removed rather than claimed as a second math rendering path.
- Both renderer libraries pass strict Clippy under all diagrams and both layouts.
- Each regenerated branch manifest contains twelve real CLI SVGs. Chromium
  156.0.8078.4 passes nineteen checks, twelve visible formula contributions and
  six independent parity/native placement pairs, including Sequence and the
  resolved CJK/bitmap regressions.
- Both oracles reject deletion of native instances, hidden foreignObjects and a
  20px native displacement. They use exact RGBA differences for visible
  contribution; no intensity threshold masks missing paint. The placement
  allowance is one CSS pixel for raster-edge quantization, not a DOM/comparator
  tolerance. `browser-mutations.json` records the counterexamples.

The resolved-glyph Chromium controls confirm actual containment on both lines:
CJK smash changes from a zero-height viewport to `viewBox="0 -0.826172 2 0.89502"`;
the emoji bitmap changes from a clipped 0.74em viewport to
`viewBox="0 -0.01 1 1"`. These figures describe the generated self-contained
font output in this environment, not arbitrary missing host glyphs.

The existing PNG overview remains bound to the previous paint source commit.
The regenerated SVG/browser evidence is separate; no old picture is repainted
to imply that it depicts this follow-up.

Main's refreshed full 37-family structure/parity/parity-root gate passes. All
3,737 generated SVGs are byte-identical to the prior complete Chromium root
paint report, verified fixture-for-fixture by SHA-256 with no additions or
missing files. Root paint evidence is retained for those exact bytes; the new
compiled-math exports have their own fresh browser visibility/placement checks.
No comparator, layout golden, tracked upstream SVG or root residual is changed.

Main also passes the complete feature-matrix owner (isolated consumers and all
49 package/feature builds), representative artifact dependency closures,
strict all-targets/all-features CLI Clippy, the no-math Flowchart build, seven
root contracts, generated-license checks, legal projection checks and third-party
contract checks. Legal report changes contain only the new Cargo.lock digest;
there are no additional runtime packages or license entries.

Maintenance's refreshed 37-family, three-mode SVG gate also passes. All 3,720
SVGs are byte-identical to the previous full Chromium root report. The report's
single pre-existing macOS Flowchart-title receipt limitation remains recorded;
this identity check is not a new all-green macOS root-paint claim. No baseline,
comparator or receipt was changed to pass either branch.

Both lines pass `cargo deny check advisories bans licenses sources`, their
representative artifact closure checks, generated licenses/legal/third-party
checks and the corresponding Python legal test suites (20 main, 22 maintenance).
The direct optional metrics dependency does not add a runtime package to either
existing math closure. Its lock-file edge and generated fingerprints are kept
in the same source repair commit.

Maintenance also completes its full feature-matrix owner: isolated consumers
and all 42 package/feature builds pass. Both source repair lines have complete
local qualification; new-head CI remains responsible for its Linux workspace,
assembled Web and platform/binding jobs. Prior-head CI success is not reused
as proof for these source commits.
