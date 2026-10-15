# Theme admission corrections and bounded Flowchart edge effects

Status: scoped validation complete. This increment does not close U6 or C7a.

## Scope and baseline

The reported audit spans `54f8398f4..6d9ef7c80`. The edge-effect implementation was
uncommitted development outside that audit baseline. The September 18 fetch returned
`origin/main` at `f6b041131`, already included through merge `f63dda087`; no additional
mainline commits were available to merge.

## Reported defects

- HTML font-weight: generated class selectors can change descendants without appearing in the
  sanitized fragment. The bounded check covers the actual label shell and ancestor classes.
  Nested spans with an unmeasured class winner retain residual evidence; plain labels with
  measured source-owned weights remain accepted. Source/config precedence cannot suppress an
  unverified terminal. Node, EdgeLabel and Swimlane paths share the check.
- WASM options: extracting `timeout_ms` uses borrowed raw JSON values and preserves duplicate
  fields for shared admission. Tests compare current-source errors with and without a timeout,
  including escaped duplicate keys and nested definition fields.
- Flutter one-shot options: constructor policy and request options retain raw JSON member
  values. Only the existing constructor-owned policy fields are projected. Theme resource
  budgets remain in authoring admission, and removing policy fields preserves the original
  request byte charge. Tests cover all three authoring operations and ordinary SVG requests.
- Web effects: `ThemeEffectGraph.color_space` accepts `linear-rgb` and `srgb`; omission retains
  the Rust default. Public construction and exported-recipe reading are typechecked, with
  invalid-value and null negative cases.

The Web correction is commit `755ba9444`; the HTML admission correction is `6728e7cdd`.
The raw transport admission correction is `9bfb49e19`.
The separately developed edge-effect increment is `ddb1c1d67`.

## Edge-effect increment

The initial strict-mode regression failed with `IncompleteFamilyTheme`: Edge effect bindings
had no terminal consumer. The implementation now prepares a filter from the same cached path
geometry consumed by the writer, after Swimlane line-hop processing. Regions use local
`userSpaceOnUse` coordinates; only viewport union adds the enclosing root offsets. Horizontal
and vertical paths therefore need no fabricated nonzero object bounding box.

Shared shadow lowering retains existing object-bounding-box behavior for node effects. Native
receipts include coordinate units, and the exporter verifies the exact region against usvg.
The source envelope includes bounded stroke joins and the selected markers' rotated viewport
bounds. Edge labels remain outside the filtered path. Unknown source geometry/filter ownership
and non-classic looks retain residuals. Static/default rules and bindings are supported;
ordinal edge effects remain unsupported, and Clear suppresses an effect binding.

The public Cyberpunk recipe is unchanged by this edge increment. Connecting it still requires
its complete visible scene checks, including text glow. This record does not certify a complete
modern_mermaid reproduction or all diagram families.
At this increment, the suggested Web public-entry background checks (colors, positions, radii
and layer order) remained an explicit U9 test requirement. They were subsequently implemented
in `9d8438ca9`; see [the Web background and glow record](2026-09-18-cyberpunk-web-background-and-glow.md)
for its freshly built full-profile smoke and Chromium result. That evidence belongs to its stated
source baseline, not automatically to later HEADs or complete U9 qualification.

## Verification

- Web contracts: passed; 40 WASM exports, 50 runtime bindings and 5 package entries checked.
- Renderer Release regression: 2,804 passed, 2 skipped across the library, Flowchart effect,
  theme resolution, support discovery and label measurement contract targets. This final run
  includes assigned/default/structural classes, nested spans and mixed-facet effect failures.
- Native filter receipt tests (`merman-export`, `png,pdf`): 17 passed, including actual cyan
  pixels outside horizontal/vertical paths and nested-coordinate/unit mismatch cases.
- WASM transport admission (`svg,layout-cytoscape`, Release): 5 passed.
- Flutter Dart analysis: passed for the wrapper and authoring smoke.
- Flutter execution against a newly built macOS alpha.7 host-owned FFI library: passed for
  one-shot and reusable consumers (2 materializations, 22 support queries, 5 error vectors
  and 3 budgeted authoring operations per consumer), plus the new duplicate-field and
  runtime-policy probes. The library uses the desktop `analysis,ascii,svg,layout-cytoscape,layout-elk`
  features in the dev profile; this is not an installed release-package claim.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Full-document native PNG/PDF export: 1 test passed across horizontal, vertical, nested-root
  and circle/cross-marker scenes. PNG contains cyan shadow pixels and both exports retain
  complete theme evidence. The initial arbitrary 40-level channel-contrast assertion failed
  on a real `[218, 253, 255, 255]` glow pixel; the final achromatic fixture instead checks
  for cyan channel separation. This is bounded edge verification, not full-preset acceptance.

Local logs are in `/tmp/merman-theme-audit-fixes-focused.log`,
`/tmp/merman-edge-glow-native-receipt.log`, `/tmp/merman-theme-raw-options-wasm.log`,
`/tmp/merman-web-color-space-green.log`, `/tmp/merman-theme-audit-renderer-final.log`,
`/tmp/merman-edge-glow-document.log`, `/tmp/merman-theme-audit-flutter-native-build.log`,
and `/tmp/merman-flutter-raw-options-smoke.log`. They are session artifacts, not release receipts.

No full workspace run, all-platform rebuild, installed-package matrix or performance comparison
is claimed by this increment. The existing Flutter bundled library reports alpha.6; this run
explicitly loaded the fresh alpha.7 library without replacing that bundled artifact.

## Follow-up review disposition (2026-09-18)

A second review of the same `54f8398f4..6d9ef7c80` range reported three findings. They are
already covered by later commits and are not new open defects on the current branch:

- The Flowchart HTML weight ownership correction is `6728e7cdd`. Unmeasured nested HTML weight
  keeps residual evidence; the renderer no longer treats fragment inspection alone as proof.
- Raw WASM and Flutter option projection is `9bfb49e19`. Timeout/policy projection preserves the
  original JSON members, duplicate keys and recipe byte accounting before shared admission.
- The Web `ThemeEffectGraph.color_space` public type is `755ba9444`, including construction and
  export/import type coverage.

The public Cyberpunk recipe now also has an independent Flowchart Edge graph in `da7b94532`.
It is a one-stage sRGB source-graphic shadow with a six-pixel blur, scoped only to Flowchart Edge;
Node keeps its separate two-stage graph. The recipe exchange and native Flowchart regression now
verify that scope. This remains a bounded consumer increment: label glow, Sequence and XY Chart
scene qualification, catalog cells and C7a delivery evidence are still open.
