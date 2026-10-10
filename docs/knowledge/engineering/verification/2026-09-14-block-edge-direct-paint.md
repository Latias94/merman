# Block Edge direct paint — 2026-09-14

Source: `a902b5d53cf6b429b734916112ca4b6d3bd1556c`.

## Scope

Block Edge static unqualified/Default solid/transparent fill and stroke now write directly to
edge paths. Fill remains a stroke fallback only while stroke is unspecified; Clear and unsupported
stroke values block that fallback. Source lineColor and the matching source class retain ownership.
The bridge no longer assigns EdgeStroke or MarkerPaintFromEdge. The unused marker fallback helper
is removed; explicit Marker paint and the remaining Block compatibility provider remain active.

Each emitted path is checked against its semantic identity, layout-derived path geometry and
winning paint. Missing, duplicate and damaged terminals fail the receipt. Legitimate zero-length
self-loops retain exact terminal validation but do not claim applied paint. Ordinal identities
remain source-indexed even when a preceding self-loop has no visible stroke.

KTD17 v87 contains 486 routes and 754 route-profile witnesses. The Block legacy inventory falls
from 28 to 20 routes; KTD23 remains v9 with 80 historical retirements. Public support revision 87
adds Block Edge fill/stroke-paint to the shared 17-case golden as Conditional partial typed surfaces.

## Verification

- Main worktree Release owner run: 3218 passed, two existing skips. Includes renderer and bindings
  unit tests, Block SVG, support discovery, independent retirement inventory, both Block/Class
  retirement integration targets and the route runtime authorization test.
- Independent detached checkout at the exact source above: the same 3218 tests passed, with
  two existing skips. Git status was empty before and after execution. The run reused the host
  target directory while recompiling crates from the independent checkout; it was not a second
  invocation against the original worktree.
- All 754 route-profile witnesses passed actual SVG and PNG authorization. The Block Edge witness
  uses explicit empty grid cells to keep line pixels clear of nodes, labels and markers. The
  initial adjacent-node fixture failed because its short lines were covered by marker geometry;
  raster thresholds and admission requirements were not relaxed.
- Full SVG structure gate passed with `compare-all-svgs --check-dom --dom-mode structure
  --dom-decimals 3 --diagnostic-browser-text-layout`.
- Independent correctness and maintainability reviews found no remaining blocking issue after
  the self-loop regression was fixed. Added gradient fallback and ordinal self-loop negative tests.
- `cargo fmt --all -- --check` and `git diff --check` passed.

The owner command is:

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-render -p merman-theme-acceptance -p merman-bindings-core \
  --features merman-render/layout-elk,merman-render/math,merman-bindings-core/svg \
  --lib --test block_svg_test --test theme_support_discovery_test \
  --test legacy_projection_retirement --test block_title_legacy_projection \
  --test class_edge_label_background_legacy_projection --test route_cutover_runtime \
  --no-fail-fast
```

## Installed Flutter consumer

Rebuilt the package-owned macOS arm64 Native Assets library with the existing
`python3 platforms/flutter/build-native.py host` recipe and ran
`dart run tool/theme_authoring_smoke.dart`. Both one-shot and reusable consumers passed
2 materializations, 17 support queries, 5 errors and 3 budgeted operation comparisons each.
The native library was built from the implementation committed above; it remains an ignored
build artifact, not a tracked binary.

- Native recipe: flutter-desktop-native / native-distribution / aarch64-apple-darwin.
- Packaged library size: 21843920 bytes.
- SHA-256: `d65512045d3dce5f0ac1cddbb2d13c5c19ceb55681a25ea7a60be23d98ce1b31`.

## Limits

This slice does not remove the remaining Block provider or historical retirement authority.
It does not claim a performance improvement, C7a closure, public catalog promotion, full workspace
or browser-suite coverage, or rebuilt Web/Node/Typst/Python artifacts at revision 87. Their earlier
revision-86 artifact records remain historical. XY Chart cache optimization still needs measured
benefit before a broader rewrite.
