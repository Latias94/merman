# Terminal text repair evidence

Date: 2026-10-26. Plan: `docs/plans/2026-10-26-text-terminal.md`.

## Scope and authority

Main starts at `a6be76743`; maintenance starts at `96fc586d5`. The actual pinned
reference descriptors select Mermaid 12.2.0 on main and 12.1.0 on maintenance.
Historical filenames mentioning 11.16 are not release authority.

This work fixes text correctness, not a new performance qualification. The root
worktree's unrelated Flutter changes are excluded. Native glyph-fallback ranking
is not changed because its backend contract remains an open question.

## Main implementation evidence

- Before implementation, four shared/Class/Flowchart regression tests fail, and
  four Sequence/Mindmap/Block/Usecase tests fail. Three additional State run,
  asymmetric bbox, and inline-override tests also fail.
- Shared projection and compiled-winner changes pass a 291-test focused run.
  After formatting, the overlapping 249-test shared run also passes.
- The integrated renderer library, affected families, Class typography,
  root-dependent typography, and layout snapshots pass 3,911 tests, with two
  existing skips. Two layout snapshots are refreshed with the existing xtask:
  Mindmap configured font size and the local Usecase ELK boundary guard.
- Independent review finds HTML code spans incorrectly sharing the SVG token
  allowlist, Class note entity-depth and baseline-probe mismatches, actor/business
  relative-size inheritance, and State node ink-origin omissions. These are
  addressed rather than hidden behind geometry tolerances.
- Current-source browser verification catches a further Sequence title anchor
  mismatch: a 40px title still extends to x=556.55 while its repaired root ends at
  x=415. The title writer uses start anchoring but the measured horizontal extents
  are center-relative. A new start-anchor test fails before its geometry fix.
- After the anchor fix, all 143 Sequence tests pass (one existing skip). After
  separating HTML projection and sharing Class note rows/entity provenance,
  264 shared/Class tests pass. These counts overlap; they are not additive totals.

## Design safeguards

SVG words retain their explicit normal resets and Markdown strong/em attributes.
HTML measurement retains literal code tokens through a separate projection of
the same parser. It keeps its established measurement primitives, paragraph gaps,
and host-provided heights. A trial routing all HTML through XHTML measurement
changed unrelated Requirement primitive selection and exact residual signatures;
that trial is not retained and no residual is broadened to accommodate it.
Class SVG notes retain one parsed label plan for measurement,
baseline probes, and emission, with already-decoded model entity provenance.

State retains plain rows on the ordinary path and styled rows only where needed.
Explicit inner inline font declarations are replayed after presentation-attribute
resolution. Source fonts repeated across real DOM ancestors resolve at each
ancestor; same-element role defaults are not an inherited parent font size.

No upstream SVG, comparator tolerance, overflow policy, public ABI, or automatic
font discovery is changed. Final full-structure, browser, lint, maintenance, and
remote-CI results are recorded only after those checks actually finish.

## Completed terminal-text verification

- Main `c337bb65e`: the integrated renderer library, affected diagram families,
  typography contracts and layout snapshots pass 3,920 tests, with two existing
  skips. Strict renderer-library Clippy and the complete 37-family structure
  owner pass. The Class-only feature leaf builds with existing partial-feature
  warnings; it is not a strict leaf-Clippy claim.
- Current-source Edge verification confirms the 40px Sequence title is contained
  by its root. The root is `-50 -62.777777776 805 287.777777776`; title ink is
  approximately `(75, -61.965, 481.571, 46.206)`. Mindmap computes to 32px and
  Block to 48px. These are font/geometry checks, not pixel-perfect parity claims.
- Formal independent review of `a6be76743..c337bb65e` completes with one P2:
  HTML code spans trigger the SVG parser allowlist, omitting visible link/strike
  syntax and inserting spaces around adjacent code. Run:
  `20261026-173825-8771979a`. Review artifacts are local, not published build inputs.
- The Usecase integration regression fails before its fix on both release lines:
  the rendered paragraph contains ``x `c` [visible label](u)`` while measurement
  receives only ``x `c` ``. The fix preserves HTML source prefixes/suffixes and
  character adjacency without changing SVG filtering or HTML measurement
  primitives. Six integration cases independently compare rendered text, host
  requests and foreignObject dimensions; a unit case covers child styles and
  styled code adjacency. A focused independent inspection finds no blocker.
- Main `222d7a765`: the final library plus Usecase run passes 3,249 tests with one
  existing skip. Strict renderer-library Clippy and the exact complete 37-family
  structure owner pass again. These runs overlap the larger integration run and
  must not be added as distinct-test totals.
- Maintenance `0fd54d886`: the corresponding final run passes 1,678 tests, with
  no skips; strict Clippy and the complete 37-family structure owner pass. Earlier
  affected-family integration passes 1,880 tests with one existing skip. Current
  Edge verification confirms the same Sequence title containment and a 32px
  Mindmap root font. The first browser probe required adapting to maintenance's
  consuming `Engine` API; the successful check uses the current maintenance rlibs.

## CI follow-up identified before publication

The old remote PR heads still fail independently of the terminal-text tests.
Read-only CI analysis identifies main's early native portability enforcement and
typed Flowchart relative-weight ordering, plus maintenance's accessible-name
mismatch and a Playwright-patch-bound exact viewport receipt. These are separate
repair units; terminal-text green results do not prove those failures resolved.
No new remote CI success is claimed here.

## Main CI repair verification

The old Linux workspace failures are reproduced locally: all five selected
tests fail before the repair. The native facade now completes a document while
retaining strict requirements and invalidated evidence, then lets PNG/JPEG/PDF
admission own rejection. A private workspace bridge mirrors standalone SVG's
existing bridge. The public `finalize_resvg` remains strict, with a regression
test proving it still rejects a mutating postprocessor. Budget, cancellation,
prepared-math and evidence-invalidating checks are unchanged.

Flowchart installs the typed/config-selected parent label weight before resolving
the winning source declaration. Relative `bolder` therefore resolves against
600 to 900, not against the pre-theme default to 700. Layout and writer fallback
paths share this order; the global weight algorithm is unchanged. Invalid later
declarations still win compilation and fall back to the inherited parent.

At `4a835c9e1`, all 74 tests across native-export residuals, render operations,
typed-family API, Flowchart measurement and theme-resolution contracts pass.
This includes the five original failures and the public strict-finalization
guard. Native tests select `system-fonts` and JPEG/PNG/PDF to match the workspace
capability set; earlier runs without a usable font-source feature fail with
`FontSourceUnavailable` and are not reported as successful validation.

The broader renderer library, Flowchart, typed-theme and layout-snapshot run
passes 3,413 tests with one existing skip. Strict facade/renderer library Clippy
passes, as does the complete 37-family exact structure gate. These test counts
overlap earlier runs. Main-only typed label and target-evidence architecture is
not imported into maintenance, which does not have those failing paths.

Independent review then identifies a remaining sibling path: collapsed
subgraphs draw as nodes but are measured through `SubgraphTitle` ownership. Their
Dagre/ELK measurement still resolves `bolder` before installing the typed parent,
while writer fallback resolves it afterward. A new weight-sensitive regression
fails with `dagre/true: ("GroupTitle", Some("700"))`, expecting 900.

`7213f0e43` applies the same parent-first resolution to collapsed Dagre and ELK
labels. The regression checks empty/collapsed node-shaped labels, their final
200px foreignObject width, and terminal 900 weight; the existing ordinary-node
host measurement contract additionally covers ELK. All 24 focused contracts pass.
The final broader suite passes 3,414 tests with one existing skip, strict renderer
Clippy over all targets passes, and the complete 37-family structure owner passes
again. No additional layout snapshot changes are required.

Formal review `20261026-194839-a50cbfd2` completes on `a0cb01fed..4a835c9e1`
with the one collapsed-subgraph P2 confirmed by its independent validator.
The original five reviewer results are retained; an interrupted sixth CLI read
is recovered once through the parent's native Codex agent tools. Merge,
validation and report complete with all child agents collected and closed.
`7213f0e43` addresses #1 and the related ELK/empty-subgraph test gap. A separate
independent inspection of that final patch finds no blocker; it notes that
non-empty collapsed and pure-SVG subgraph paths are statically covered rather
than each having a dedicated new fixture.

The maintenance replacement CI run `40323444725` passes its Linux build-test and
exact viewport receipt owner, but a remaining Web test assumes Sequence titles
must naturally overflow their root. The repaired title now fits, invalidating
that test precondition. Both branches explicitly move a mounted title across
the root boundary before testing presentation modes and fit behavior. They
retain strict overflow/containment checks, identical root and title markup,
visible/hidden clipping, and the extreme-overflow fit check. No product overflow
policy is changed to keep a historical bug alive.

Final main production verification rebuilds and assembles all five WASM packages
and Playground (42 exports, 56 runtime bindings, five TypeScript package entries).
All 16 presentation browser tests pass on that production output. The changed
test waits for Fit's scheduled animation-frame transform using the same strict
containment assertion, rather than reading the old screen transform immediately
after the click. Browser-test typechecking and targeted ESLint pass. Maintenance's
corresponding presentation suite passes all 15 tests, with the final asynchronous
fit adjustment separately rechecked in its focused case.
