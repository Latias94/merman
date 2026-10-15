# Flowchart node shadows — 2026-09-18

Source baseline: `f71e70017737c4ef5a74d2a136103fb56020cdd1`; the changes and tests described
here are committed together with this record. This is a bounded part of
[U6](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), not completion
of the full Flowchart scene, Cyberpunk preset, C7a, or the overall theme plan.

## Behavior and boundaries

Classic Flowchart and Swimlane nodes consume ordered zero-spread drop-shadow chains on
rectangles, rounded rectangles, diamonds, circles and double circles. Filter references attach
to the existing shape primitives, excluding labels. Unsupported shapes and nonclassic looks
keep explicit residuals. Clear and ordinal rules operate on actual semantic node occurrences;
source filters retain ownership without suppressing unrelated typed paint.

The State shadow recorder and native receipt reconciliation are now shared by these family
writers. Shared lowering preserves Previous/SourceGraphic inputs, linearRGB/sRGB evaluation,
resource ceilings and outward-rounded filter bounds. Shape preparation and viewport expansion
use the same materialized bounds. Layout node positions are unchanged. Source styles and resolved
node themes are reused by emission; when no Node effect route exists, the new preparation is empty
and the viewport performs no additional node scan.

The public support manifest describes partial typed effect support. The bounded Flowchart
rectangle preset fixture now consumes the actual public Cyberpunk recipe. Sequence still reports
its missing actor effect consumer. No public qualified catalog cells were promoted, no schema
version was incremented, and no dependency, font asset or resource budget was added.

## Review corrections

- An initial filter wrapper changed class selectors and multiplied HTML-label-mode opacity twice.
  Filters now attach to the original primitives. A Chromium probe confirmed that a class opacity
  of 0.5 stayed 0.5 on the shape but also became 0.5 on the added wrapper; the wrapper was removed.
  The integration regression checks the original direct-parent shape boundary.
- Diamond miter joins need more than half the stroke width at their tips. The shared materializer
  now accepts the actual stroke envelope, using conservative miter outsets for diamonds.
- Relative CSS source stroke widths such as `20%` and `2em` cannot be bounded using the renderer's
  default width. Those nodes retain source geometry and an unconsumed effect residual; strict
  portability rejects them. No CSS unit evaluator was introduced.

The existing Diamond numeric source-width certification residual remains unchanged. Its best-effort
geometry test verifies filter bounds without claiming strict source-style admission.

The code-review run `20260918-102758-5d0151b6` reported no remaining actionable findings after these
fixes. Correctness and adversarial reviews completed in independent subagents. An upstream model
routing failure and a thread-capacity limit prevented the remaining independent reviewers from
starting; the review coordinator completed testing, standards, maintainability, performance and
API-contract passes in its own context. That coverage is not independent cross-validation.
The local review artifact is `/tmp/compound-engineering-501/ce-code-review/20260918-102758-5d0151b6`.

## Validation

- The first public Flowchart native test failed with `ThemeEvidenceIncomplete` before implementation.
- Renderer Release unit tests: 2664 passed, 2 skipped. This run preceded the final source-width guard;
  the guard and Swimlane adaptation were exercised by the subsequent integration run.
- Flowchart SVG, marker, cluster, cluster-label and the first nine node-effect regressions: 168 passed.
- Final node-effect and Swimlane cluster regressions: 19 passed, including relative source widths,
  actual lane/edge-label boundaries, source ownership, mixed-shape residuals, ordinal Clear,
  nested-root bounds, recipe round-trip, opacity structure, miter bounds and host resource ceilings.
- Public authoring and actual native State/Flowchart shadow export: 15 passed. Both linearRGB and
  sRGB retain red/blue PNG pixels; SourceGraphic reset removes the first shadow. PNG/PDF receipts
  agree, and an explicitly lower filter primitive budget still rejects the export.
- Workspace-only acceptance: 10 passed across C6, actual catalog recipes, and Block/Class/Flowchart
  legacy-projection regressions. The internal cfg and PNG feature were explicitly enabled.
- Clippy completed successfully with 160 renderer warnings under this feature selection, including
  dead code and existing API-shape lints. This is not a warning-free build or a completed repository
  warning cleanup. Formatting and `git diff --check` passed.

Commands were run serially with `CARGO_BUILD_JOBS=1`:

```console
cargo nextest run --locked -p merman-render --release --no-default-features \
  --features layout-cytoscape --lib --no-fail-fast
cargo nextest run --locked -p merman-render --release --no-default-features \
  --features layout-cytoscape --test flowchart_node_effects --test flowchart_svg_test \
  --test flowchart_marker_theme --test flowchart_cluster_theme_test \
  --test flowchart_cluster_label_theme --no-fail-fast
cargo nextest run --locked -p merman-render --release --no-default-features \
  --features layout-cytoscape --test flowchart_node_effects \
  --test swimlane_cluster_theme_test --no-fail-fast
cargo nextest run --locked -p merman --release --no-default-features \
  --features svg,png,pdf,layout-cytoscape --test theme_authoring \
  --test theme_composed_effects --no-fail-fast
python3 scripts/run_theme_acceptance.py nextest run --locked \
  -p merman-theme-acceptance --no-default-features --features png,layout-cytoscape \
  --test c6_runtime --test preset_qualification --test block_title_legacy_projection \
  --test class_edge_label_background_legacy_projection \
  --test flowchart_marker_legacy_projection --cargo-quiet
cargo clippy --locked -p merman-render --release --no-default-features \
  --features layout-cytoscape --lib
```

Local logs: `/tmp/merman-flowchart-effects-lib.log`, `/tmp/merman-flowchart-effects-final.log`,
`/tmp/merman-flowchart-glow-source-width.log`, `/tmp/merman-flowchart-glow-native-final.log`,
`/tmp/merman-flowchart-glow-acceptance-final.log`, `/tmp/merman-flowchart-glow-clippy.log`.
These temporary files supplement the durable test names and commands above.

## Remaining scope

Node geometry coverage is deliberately bounded. Flowchart text/edge glow, the complete U1 visual
scene, Sequence/XYChart effect consumers, public catalog promotion and final artifact qualification
remain open. PNG pixel assertions establish visible shadow colors and chain-reset behavior; PDF
checks establish export and localized filter receipts, not page-level visual equivalence. This run
does not establish full workspace, installed transport, all-host or full browser-matrix validation,
nor a measured performance/size improvement.

## Remote main

A fresh remote check found `origin/main` at `9edd6d86a`, already included by merge `620a35ac9`.
There were zero remote-main commits outside this branch, so no second merge was necessary.
