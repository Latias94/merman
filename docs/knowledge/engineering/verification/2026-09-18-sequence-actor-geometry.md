# Sequence participant rectangle geometry

Baseline: `626e0105f`. This is the Actor geometry increment of U7 in
`docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md`.
It does not close U7, promote preset qualification cells, or close C7a.

## Behavior

Sequence Actor static and Default rules now apply stroke width and corner radius to ordinary
participant rectangles. The public Cyberpunk recipe uses the U1 reference values: a 3px border
and 10px corners. Recipe export/import retains the same rendered result. Its content fingerprint
is `1304dfbf2ec85f464d2dd8c685e2023b84cd9e2d6268ab11b529d531de6e9767`;
the unpublished recipe revision and public schemas remain v1.

The rectangle writer uses an inline stroke-width declaration because the existing `.actor`
stylesheet overrides presentation attributes. It does not change the width of actor text,
lifelines, messages or arrowheads. Explicit source/site `themeVariables.strokeWidth` owns width
only; source border color does not suppress typed geometry. Clear restores the effective
source/default width and the rectangle's baseline 3px corners.

Top and enabled mirror rectangles must all be emitted before the geometry facets are counted
as applied. Candidates are recorded before model/layout lookup; a missing terminal cannot
silently disappear from the required set. Known composite glyphs, actor-man variants and custom
participant classes keep residual evidence. Ordinal requests remain unsupported when they match.
An unsupported sibling property still prevents whole-rule application. Discovery reports the
new facets as conditional, not universally portable.

A typed border adds half its width to the existing root paint margin while preserving layout
coordinates. The existing viewport resource policy still applies. This does not certify arbitrary
source CSS or add geometry consumers for unsupported glyphs.

## Validation

- Proof-first: two positive geometry regressions failed on the baseline with
  `UnverifiedFamilyTheme`; the unsupported-glyph regression already passed.
- Release nextest: 2,838 passed, three skipped across the renderer library, Sequence integration
  tests and support discovery. The first run caught a stale recipe fingerprint. A subsequent run
  caught an old test using radius as its unsupported sibling. That test now checks a consumed
  radius plus unsupported padding, retaining all original negative evidence assertions.
- After the final equivalent lint cleanup, all seven geometry integration tests passed again
  (77 unrelated tests filtered out). Freshly captured SVGs remained byte-identical to the three
  browser inputs. Release Clippy passed with 160 remaining warnings; the two warnings introduced
  at the new rectangle writer were handled without introducing another context abstraction.
  `cargo fmt --all -- --check` and `git diff --check` passed.
- The geometry regressions cover mirror on/off, Clear, independent source width/color ownership,
  mixed glyphs and custom classes, ordinal hit/miss, unsupported siblings, missing terminal
  receipts, wide-border viewport containment, and public preset exchange.
- Chromium 151.0.7922.34, 1280x960, DPR 1: four visible rectangles compute to 3px/10px for the
  typed request, 7px/10px with source-owned width, and 1px/3px after Clear. Typed rectangle width
  leaves actor text at its baseline and lifelines at 0.5px. The typed screenshot was inspected.
  These geometry checks use host fonts; they are not a typography comparison against Mermaid.

The test command is:

```text
CARGO_BUILD_JOBS=1 cargo nextest run --release -p merman-render --no-default-features \
  --features layout-cytoscape,embedded-fonts --lib --test sequence_svg_test \
  --test theme_support_discovery_test --no-fail-fast
```

Local logs are `/tmp/sequence-actor-geometry-red.log`,
`/tmp/sequence-actor-geometry-green.log`, `/tmp/sequence-actor-geometry-final.log`, and
`/tmp/sequence-actor-geometry-verified.log`. The first two full-run failures are retained rather
than replaced with passing logs. Browser inputs, observations, screenshots, capture source and
artifact hashes are under `target/bench/experiments/sequence-actor-geometry-626e0105f/`.

Final local checks are recorded in `/tmp/sequence-actor-geometry-lint-cleanup-tests.log`,
`/tmp/sequence-actor-geometry-clippy-final.log`, and `/tmp/sequence-actor-geometry-fmt-final.log`.

## Review and remaining work

The simplification pass found no useful behavior-preserving reduction. The code review covered
correctness, tests, maintainability, project standards, security, performance, API contracts and
adversarial cases without retained findings. Capacity limits required serial lenses in one
independent Codex context, not eight independent reviewers. Review run:
`20260919-064113-7bbc9094`; its date is a tool-generated run identifier.

Actor glow remains unconsumed and the public-preset regression explicitly retains its residual.
The next U7 work is the actual Actor filter attachment and paint bounds, followed by distinct
message, note, control-frame and text consumers. Full public-scene SVG/PNG/PDF qualification,
installed consumers, performance/size comparisons and same-source C7a candidate gates remain
open. This increment adds no dependency, font asset, XML scan or dynamic terminal ledger and
makes no latency, memory, or native-output improvement claim.
