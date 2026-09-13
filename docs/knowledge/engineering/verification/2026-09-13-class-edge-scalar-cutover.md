# Class Edge scalar cutover

Date: 2026-09-13

Baseline: `8535c3597`. The existing relation-path and referenced-marker integration test
was expanded to cover unqualified and Default Fill/Stroke before changing production code.
The old implementation failed the Default Stroke case with
`LegacyFamilyThemeCompatibility { family_id: class, residual_count: 2 }`.
The local red log is `/tmp/class-edge-scalar-red.log`.

## Runtime and retirement boundary

The existing Class relation plan now resolves scalar fill as a stroke fallback only when
stroke is Unspecified. Clear, gradient, and other unsupported stroke values block that
fallback. Explicit Mermaid `lineColor` owns both semantic paths. Shared direct-static
paint resolution performs selector, origin, and value checks; Class retains its own
fallback rule and terminal ledger. The terminal receipt includes the source property,
so a stroke emission cannot certify the sibling fill merely because both share a rule
index. Matching unsupported ordinal winners remain residuals; non-intersecting or
stroke-masked ordinal fills are NotApplicable. Existing best-effort static paint behavior
under unsupported ordinal stroke is preserved.

A follow-up writer audit found that the old lineColor also styled attached-note
connectors through `.relation`. The first typed migration omitted those paths; the new
note-only regression failed before the repair (`/tmp/class-note-scalar-red.log`).
The final paint domain includes valid attached-note connectors, with expectations derived
from source note declaration indices rather than emitted layout paths. Missing, duplicate,
or mismatched note checkpoints fail completion. Width, markers, labels, and relation
ordinals keep their existing relation-only domain: inserting a note does not move a relation
ordinal. Static note-only paint requests must be applied or rejected, rather than treated as
an empty domain. The native Class edge fixture now includes an attached note.

Class no longer executes EdgeStroke or MarkerPaintFromEdge bridge assignments. Both
Class Title and generic Text compatibility remain: Title's `titleColor` does not style
the diagram-title node, but it does reach visible namespace labels. Removing that route
as if it had no writer would lose real behavior.

KTD17 v82 adds six routes and twelve Classic/HandDrawn native witnesses, giving 458 routes
and 678 route-profile witnesses. The new routes retain the existing EdgeStroke replacement
and MarkerPaintFromEdge fallback-retirement obligations. Raster proof observes Class
Edge.fill in the emitted stroke channel. KTD23 remains v9 with 80 historical identities
and 160 value probes. The live bridge inventory is 48 routes: Block 32 and Class 16.
Both the production inventory digest and the acceptance manifest digest were updated
from observed inventories after reviewing the exact six route additions and obligations.

Public support revision 84 moves Class Edge.fill from partial legacy to partial typed.
The shared authoring support fixture now has twelve vectors, including Class Edge.fill.
This does not qualify native output through the discovery API or certify installed
packages built against earlier revisions.

## Verification

The final private-cfg Release gate passed **445/445**, including the complete **678-witness**
native route-cutover runner with the attached-note fixture. The log is
`/tmp/class-edge-note-verified.log`. Earlier runs exposed stale inventory snapshots and,
after adding note coverage, a missing CSS declaration separator and an incorrect assumption
that namespace writer order equals source note order. Both runtime issues were independently
reviewed and fixed before this passing gate. The namespace reproducer failed with the old
ordered ledger while the ordinary note color case passed
(`/tmp/class-note-namespace-red.log`); the repaired ledger matches source identities and still
rejects duplicate, missing, or incorrect terminals.

The unchanged revision-84 support fixture also passed the three compiled Native C ABI,
UniFFI, and Typst shared-golden tests (`/tmp/class-edge-scalar-transports.log`). This is not
an installed-package or complete cross-platform qualification claim.

The gate includes Class SVG/terminal tests, public discovery, mechanism classification,
bridge inventory, authorization digests, the full cutover runner, KTD23 retirement,
Block's title/composite-label retirement, and Class's retired background pixel control.
Tests cover solid/transparent paint, source/site ownership, absent relations, mixed and
split rule grouping, Clear/gradient stroke barriers, ordinal masks, and missing/wrong
terminal receipt identities. Note-specific coverage also includes declaration-index gaps,
namespace emission order, standalone notes, static paint with relation-only width, and
relation ordinal stability.

```text
python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-render -p merman-theme-acceptance --lib \
  --test class_svg_test --test theme_support_discovery_test \
  --test route_cutover_runtime --test legacy_projection_retirement \
  --test block_title_legacy_projection \
  --test class_edge_label_background_legacy_projection \
  -E 'test(class) | test(family_mechanism_matrix) | test(legacy_family_theme_bridge) | test(edge_fill_is_observed) | test(route_inventory_retains) | test(route_manifest_digest) | binary(theme_support_discovery_test) | binary(route_cutover_runtime) | binary(legacy_projection_retirement) | binary(block_title_legacy_projection)' \
  --no-fail-fast
```

The scoped private-cfg Release Clippy command completed successfully
(`/tmp/class-edge-note-clippy.log`); it retained the repository's warning-level diagnostics
and did not use `-D warnings`. The full blocking SVG structure comparison also exited 0
(`/tmp/class-edge-note-structure.log`):

```text
cargo run --locked --release -p xtask -- compare-all-svgs \
  --check-dom --dom-mode structure --dom-decimals 3 \
  --diagnostic-browser-text-layout
```

The six-role review and fresh finding validator completed after fixing the two note issues.
The superseded review receipt was explicitly withdrawn. One non-blocking coverage suggestion
remains: no separate integration test mutates the prepared layout to remove an attached-note
edge or its points. Source-count and receipt unit negatives cover missing, duplicate, and
incorrect identities/paints; the production SVG/native paths are exercised above.

All Cargo commands use `CARGO_BUILD_JOBS=2` and run serially. This is a C5/C7b route
migration, not closure of C7a, global bridge retirement, or the outstanding WASM size gate.
