# Class Cluster scalar cutover verification

Date: 2026-09-13

Class namespace rectangles consume typed `Cluster` fill and stroke rules for static unqualified and
`Default` selectors. The terminal receipt binds cluster IDs, independent winning rule indices, and
final CSS. The implementation covers ordinary namespace groups, extracted roots, and the shared ELK
writer; executable evidence for each configuration is listed below.

Inventory remains 40 live legacy routes (32 Block, 8 Class), KTD17 v83 with 466 routes and 694 native
route-profile witnesses, and support claim revision 85 with 14 shared fixture vectors. The remaining
Class legacy routes are Text.fill and Title.fill; Cluster fill/stroke have entered the typed matrix.
This record does not close C7a or C7b.

## Corrected build provenance and ownership

The initial 452/454 acceptance result was incorrectly described as test isolation. The failing
combined Class test executable linked a core library from the old baseline checkout, while the
single-package executable linked the current core. Both used the same serde_json fingerprint.

Evidence captured before rebuilding:

- combined binary: `class_svg_test-e6a55fa9dc0c9c7f`;
- SHA-256: `fe75faa08c0fbcc6372c5a51115f486f81f26093cf709dc9274cb87d1f2357db`;
- embedded core paths included
  `/private/tmp/merman-class-cluster-red-37c286778/crates/merman-core/src/generated/lalrpop/class_grammar.rs`;
- the baseline and current checkout had reused the same target directory. Relative dep-info and
  source timestamps permitted reuse of the older core artifact.

Rebuilding the current core, without changing its source, made the original combined ownership
cases pass. The rebuilt executable contains current core paths and no old-baseline core paths.
`/tmp/class-cluster-stale-core-linkage.txt` records the captured linkage evidence;
`/tmp/class-cluster-current-core-rebuild.log` records the passing pair of integration tests.

Commit `3882012f6` attempted to compensate by checking source variables across multiple themes.
That was too broad: `base/mainBkg` and `base/primaryBorderColor` do not own Cluster paint, even though
other theme programs use similarly named variables. The renderer now checks only the consumed
`clusterBkg` and `clusterBorder` paths. Theme-specific derivation and provenance stay in core.
The added negative controls coexist with the existing base/dark derived-owner positive controls.
The pre-fix failure is recorded in `/tmp/class-cluster-ownership-nonconsumer-red.log`.

Do not compare different revisions through the same target directory. A clean checkout of the
same tested revision may reuse dependencies only when its workspace sources are rebuilt and the
resulting executable's source provenance is checked.

## Extracted namespace relation receipts

A separate valid fixture exposed an ordering bug: an internal `A --> B` relation is declared before
an external `X --> Y`, but the writer emits the external root first. Zipping terminal events against
the declaration vector rejected the complete SVG. Replacing the external relation with an isolated
node had hidden the failure; the full fixture is now restored.

Relation and label-background receipts now match unique source relation indices, allowing SVG
traversal order to differ. Missing, duplicate, foreign, wrong-marker, wrong-rule, wrong-paint, and
duplicate-expected cases remain rejected. Marker identity, hand-drawn paint, width completeness,
and ordinal source semantics are preserved. Integration tests cover both roots, distinct markers,
and visible-label background residuals. The pre-fix failure is recorded in
`/tmp/class-cluster-extracted-relations-red.log`.

## Verification scope

- The valid migration-before fixture returned `LegacyFamilyThemeCompatibility` against old
  production code: `/tmp/class-cluster-baseline-valid-red.log`. An earlier malformed namespace
  fixture failed parsing and is not migration evidence.
- Current-core ownership checks, including the new unrelated-variable controls: 2/2 passed.
- Intermediate terminal and Cluster checks after restoring the full extracted fixture: 20/20 passed.
- Independent correctness review found no remaining actionable issues in the ownership and
  source-indexed receipt changes. It was a read-only review, separate from executable checks.

The final private Release combination passed 469/469 tests, including ELK, source-owner controls,
relation/background mutations, restored cross-root scenes, support discovery, Block/Class historical
retirement checks, and all 694 native route-profile witnesses:

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-render -p merman-theme-acceptance --features merman-render/layout-elk --lib \
  --test class_svg_test --test theme_support_discovery_test --test route_cutover_runtime \
  --test legacy_projection_retirement --test block_title_legacy_projection \
  --test class_edge_label_background_legacy_projection \
  -E 'test(class) | test(family_mechanism_matrix) | test(legacy_family_theme_bridge) | test(route_inventory_retains) | test(route_manifest_digest) | binary(theme_support_discovery_test) | binary(route_cutover_runtime) | binary(legacy_projection_retirement) | binary(block_title_legacy_projection)' \
  --no-fail-fast
```

Log: `/tmp/class-terminal-final-acceptance.log`. This is a focused acceptance combination, not a full
workspace, browser, installed-package, or cross-platform run.

The complete blocking SVG structure gate passed (exit 0):

```text
CARGO_BUILD_JOBS=2 cargo run --locked --release -p xtask -- compare-all-svgs \
  --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
```

Log: `/tmp/class-terminal-full-structure.log`. Formatting and `git diff --check` also passed.
Clean-checkout results are recorded separately after execution.
