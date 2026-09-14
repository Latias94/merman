# Class Text cutover correctness repair

Repair source: `9be480c86e533d626c4694a211f67261fbe0d24f`.

Baseline: `523ce8881` (`feat(theme): route Class text through typed terminals`).

The baseline promoted Class Text before closing its renderer and acceptance obligations.
A private Release owner run on that baseline returned 3124 passes, 38 failures and two
skips. Those results do not establish Class retirement or C7a eligibility. Most failures
concern matrix, support, bridge dispatch and cutover authorization reconciliation; they
must not be repaired by copying observed counts into assertions.

## Correctness repair

Node paint expectations now use the existing `ThemeRuleOrigin::target()` instead of
scanning every mechanism route for each node and property. Namespace labels use the
same actual winner target. Rule indices remain globally unique within the full theme;
this is not a change to rule indexing or author order.

Class Text marks an earlier static rule NotApplicable only when every authored facet
has a later winner with the same selector. This covers unqualified and explicit Default
Clear followed by Solid. A winning unsupported sibling facet remains incomplete, even
when another facet of the same rule was replaced. The existing static resolved style
supplies property origins once; no node-by-rule provenance scan is added.

The missing-geometry edge-label fallback again emits literal text. With no typed paint,
its output matches the old unstyled writer exactly; adding paint preserves literal
Markdown characters and entity escaping. The geometry-present Markdown writer retains
its existing behavior.

The previously removed namespace Unsupported rejection assertion is restored. The
all-channel SVG test checks both color and fill on every nonempty text node through its
local inheritance chain. Mutation cases remove or corrupt each Account title, member
and method paint owner while leaving the other terminals intact. These are rendered
output assertions, not qualification of an arbitrarily modified final SVG.

## Verification

The final main-worktree private Release owner run passed **183/183** selected tests
(108 Class SVG and 75 internal Class tests); 2545 unrelated tests were filtered out.
Log: `/tmp/class-text-repair-owner.log`.

The full SVG structure command passed in the main worktree. Class selected 251 fixtures,
rendered 249 and retained its two existing exclusions, reviewed font-row segmentation policy,
and exact browser-text-layout residual. No comparator policy was changed.
Logs: `/tmp/class-text-repair-structure.log` and
`/tmp/class-text-repair-structure-report.md`. Workspace formatting, diff checks, and the
Block/Class workflow selection regression also passed. Structure mode does not check
root viewports, browser visibility or native export paint.

```text
CARGO_BUILD_JOBS=2 cargo run --locked --release -p xtask -- compare-all-svgs \
  --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout

CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-render --features layout-elk,math --lib --test class_svg_test \
  -E 'test(class::) | binary(class_svg_test)' --no-fail-fast
```

The Clear/Solid regression first failed with `IncompleteFamilyTheme` (required=2,
accounted=1), then passed with the repair. The sibling-facet cases remain strict-mode
failures as intended. Standards and Spec reviews found no new blocking issue in this
repair; they did not certify the incomplete cutover.

## Clean-checkout confirmation

The exact repair source was checked out at `/tmp/merman-class-text-repair-9be480c86`.
The same private Release owner command passed **183/183**, with 2545 unrelated tests
filtered out. Git status was empty before and after the run. It reused the main
worktree's target directory, and compilation logs identify the clean source path.

Log: `/tmp/class-text-repair-clean-owner.log`.
Log SHA-256: `2bdc4a444b09c656e26116ff5dce716a470828898d973ab18a425bf23e5a1067`.
Source/status observations and main/clean/structure log hashes are retained in
`/tmp/class-text-repair-verification.json`.

The clean run covers the selected owner tests. Full structure comparison ran in the main
worktree against the same renderer source. This does not claim full workspace, complete
browser-suite, installed-package or final artifact-profile verification.

The writer-emission obligation was subsequently addressed by
[the Class writer-emission repair](2026-09-14-class-text-writer-emission.md), including
notes. That source has its own scoped verification and does not inherit this clean result.

## Remaining cutover obligations

- Reconcile partially shadowed mixed rules by their surviving properties. The bounded
  full-shadowing repair does not claim complete ordinal or cross-selector coverage.
- Close support/matrix/bridge dispatch and cutover authorization with native terminal
  witnesses before retiring the Class provider. Keep typed NodeLabel and Title APIs.
- Rerun the wider private owner suite on the completed source and verify it in a clean
  checkout. This repair does not turn the baseline's 38 failures into a passing gate.

The earlier Block CI gap is closed by `2fa9063d5`; ordinary CI and release-preflight
explicitly select its PNG/internal-cfg witness, guarded by the workflow test. XY Chart's
unused receipt payload repair is already recorded at `477c09505`; it is a structural
storage improvement, not a measured latency claim. C7a artifact qualification, public
rollout, package size gates and contract freeze remain separate open work.
