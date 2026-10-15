# Sequence Note geometry and shadow verification

Date: 2026-09-19. Implementation base: `0cd9836a3`.
Scope: the Note rectangle increment of U7 in
`docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md`.

## Behavior

Static unqualified/Default Note rules now consume stroke width, radius and bounded composed
shadows on the actual rectangle. Global Note effect bindings use the same consumer. Clear
removes the corresponding typed geometry or effect; an effect rule supersedes its binding.
Note fill/stroke configuration keeps its existing source precedence. Actor `strokeWidth`
configuration does not change Note's SVG-default 1px stroke.

The Note paint plan takes geometry from the same layout nodes as its writer, reuses shared
shadow lowering, and unions the outward-rounded materialized filter region with root bounds.
Width-only requests also include their stroke outset. Without a requested width or effect the
plan does not scan messages or create filter entries. A radius-only request needs no larger
bounds. No layout placement, dependency, font, budget or schema version changes.

The writer records each actual rectangle and effect reference. Missing terminal emissions
cannot satisfy the semantic Note count. Unsupported sibling facets, ordinal/other-variant
requests and unsupported effect primitives remain explicit. Activation keeps its existing
paint-only support; Note text is not part of the rectangle filter.

The public Cyberpunk recipe adds a Sequence-only Note rule: 2px stroke, 10px radius and an
sRGB magenta drop shadow with sigma 8 and alpha 0.4. These values follow `.note` in the pinned
`repo-ref/modern_mermaid/src/utils/themes.ts`. Public compile and complete-spec export/import
share the recipe; no preset-specific writer branch is introduced. The compiled fingerprint is
`7e3e00beb39ef862907d19fd94b261e16354dfd314854de48abcf5557e7e272c`.
Catalog qualification cells remain unchanged.

## Verification history

The first behavior test failed before implementation with `IncompleteFamilyTheme` for a real
three-note diagram. It then passed with left, right and spanning notes; a wide stroke; binding,
rule and Clear cases; filter-to-rectangle attachment; and containment in the production viewBox.
Further cases cover source paint ownership, geometry Clear, absent Notes, unsupported sibling
facets/selectors/primitives and materialization resource limits.

The first broad renderer run had 2,802 passes, four failures and three existing skips. Two
failures identified the changed recipe fingerprint, one the added graph count, and one a new
oracle that incorrectly expected split flood color/opacity. The catalog now uses the actual
compiled fingerprint, graph count is 13, and the oracle checks the existing RGBA serialization.
The shared production shadow writer was not changed for that assertion.

Final scoped runtime validation:

- Renderer Release nextest: 2,806 passed, three existing skips. Features:
  `layout-cytoscape,embedded-fonts`; targets: `--lib`, `sequence_svg_test`,
  `theme_resolution_svg_test`. Log: `/tmp/sequence-note-renderer-final.log`.
- Native Release nextest: 11 passed with `svg,png,pdf,layout-cytoscape,embedded-fonts`,
  target `theme_composed_effects`. The first parallel run reported one nextest `leaky` marker
  on the existing `composed_state_shadows_reach_png_and_localized_pdf` test. Repeating this
  target with `--test-threads 1` passed all 11 without that marker; the cause of the first
  observation is not established. Logs: `/tmp/sequence-note-native.log` and
  `/tmp/sequence-note-native-serial.log`.
- Actual public-preset PNG and PDF exports report seven filters/references and eleven shadow
  stages. Both retain `HostDependent` / `SystemOrHostFontDependency`, without filter-receipt
  or incomplete-theme reasons. PNG and PDFium's 96-dpi PDF raster were visually inspected.
- Chromium 151.0.7922.34: seven real SVG cases passed, including the public scene and
  2px/12px/200px widths with and without effect Clear. Computed widths/radii and filter
  presence match the requests. A larger transparent viewport contains no painted Note pixels
  outside the production viewport at alpha >=32 with a 1px antialias allowance. Removing
  the actual Note filter changes 4,698 public-scene pixels; Clear cases change zero pixels.
  These checks use SVG emitted by the fresh native library, not a rebuilt Web/WASM package.
- Full 35-group default SVG comparison passed with `structure,parity,parity-root`, decimals 3,
  `--diagnostic-browser-text-layout --report-root`. Sequence rendered 320 of 322 fixtures;
  two existing external-math skips, 960 DOM comparisons and the same twelve exact reviewed
  browser-text residual comparisons. No parity receipts changed.
  Log: `/tmp/sequence-note-parity.log`.
- Scoped Release Clippy passed for `merman-render` and `merman` with the native export feature
  set. Renderer/facade warning counts remain 160/31, matching the prior increment; these
  existing warnings are not claimed resolved. Log: `/tmp/sequence-note-clippy.log`.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

Local captures and runnable probes are under
`target/bench/experiments/sequence-note-shadow-0cd9836a3/`, including source and native rlib
hashes, public SVG/PNG/PDF, browser observations and PDFium library provenance. They are local
verification artifacts, not release receipts. No full workspace, installed-package matrix,
all-browser/platform rebuild or new performance measurements are claimed.

The existing CI and release-preflight jobs already run `sequence_svg_test` and
`theme_composed_effects`; this increment adds no test target or workflow.

## Review and limits

The actual seven-lens code review completed with no retained findings:
`/tmp/compound-engineering-501/ce-code-review/20260919-111131-dbc52bd4/review.json`.
Its source-set hash is `e00a271a587336b42450d51fa3c763ff95f06f0ff807816ce8f8438b1f21ef3e`.
Its initial verdict was Not ready because final runtime checks were still pending; the
subsequent parent-owned observations are recorded above. The review used seven serial lenses
in one independent Codex context under the thread-capacity limit.

The actual three-lens simplification pass is recorded in
`/tmp/sequence-note-simplify-0cd9836a3.json`. It found no behavior-preserving simplification and
identified the flood-color test oracle above. Thread capacity required three serial lenses in
one independent Codex context, rather than three independent reviewers.

This increment does not complete the Sequence reference scene: required text weights/glows
and frame/activation styling remain separate work. It does not qualify other families or all
preset/profile/host combinations. Full U7, C7a release-candidate closure and the cost audit stay
open. Historical Web/Flutter/package evidence is not relabeled as current-source validation.
