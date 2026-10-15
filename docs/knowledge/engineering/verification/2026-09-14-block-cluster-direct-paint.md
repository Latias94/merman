# Block Cluster direct paint — 2026-09-14

Source: `f2adde25ea39cb646ae51a9d14714b4d5cb0ccb0`.

## Behavior and ownership

Block Composite rectangles now consume static unqualified/Default solid/transparent Cluster fill
and stroke directly. Node and Cluster share one final shell plan and one actual-emission receipt.
A valid Node paint winner keeps the existing property-local priority on a Composite; Cluster supplies
only the remaining properties. Unsupported Node requests retain their residual even if Cluster
paints a fallback. Ordinary nodes remain outside the Cluster domain; Cluster ordinals count only
Composite terminals.

Source inline/class paint and explicit Cluster configuration retain ownership. The source class
set now matches the writer: fixed node/flowchart-label classes, explicit assigned classes, and
default only when no class is assigned or default is explicitly present. This fixes both false
NotApplicable results for a nonmatching default class and false Applied results when the fixed node
class overrides inline paint through important CSS.

Explicit typed Cluster transparency no longer passes through legacy CSS alpha replacement. The
unmodified default renderer retains Mermaid's faded clusterBkg/clusterBorder styles. Removed
Cluster bridge assignments include the secondaryColor fanout, which had no independent Block
terminal. Marker and generic Text compatibility remain active.

## Evidence and removed code

The receipt reads the actual emitted node group and shell fragment before label emission. It binds
node identity, transform, shell kinds, Composite rectangle dimensions and final fill/stroke. Missing,
duplicate, wrong-color, wrong-position and wrong-size mutations are rejected. Double-circle and
RoughJS output remain supported by the existing shell domain.

The old style-self-report receipt API and its parallel tests are removed. The writer no longer
rebuilds ownership or filters the already-final paint. The test-only source_owned_fill_mask helper
is removed; nested-source, empty-append and replacement scenarios now exercise the real plan.

KTD17 v88 authorizes 494 routes and 778 route-profile witnesses. Block's executable legacy inventory
falls from 20 to 12 routes: Marker 8 and Text fill 4. KTD23 remains v9 with 80 historical routes.
Public support revision 88 adds two Cluster entries to the 19-case support golden; claims remain
Conditional partial typed support, not unrestricted portability.

## Validation

- Before implementation, four new Cluster public regressions failed for missing terminal/evidence
  behavior. Eight focused public cases now cover scalar/transparent paint, source/config ownership,
  property-local Node competition, unsupported siblings, absent domains, class matching and Node
  gradient residuals. Actual XML receipt tests replace style-only self-comparison.
- Final main-worktree owner run: 3227 Release tests passed, two existing skips. It includes renderer
  and bindings unit tests, Block SVG and support discovery, independent legacy retirement,
  Block/Class retirement integration targets, and complete route-runtime authorization.
- Independent detached checkout at the exact source above: 3227 tests passed, two skips, with
  empty Git status before and after. Cargo reused the host target directory while recompiling
  source from the independent checkout. One Gantt native-evidence test was reported as passed but
  leaky by nextest; its immediate isolated rerun with the identical feature combination passed
  without a leak report. This is recorded rather than presented as an entirely warning-free run.
- All 778 route-profile witnesses passed SVG and native PNG controls.
- Full SVG structure gate passed with `compare-all-svgs --check-dom --dom-mode structure
  --dom-decimals 3 --diagnostic-browser-text-layout`.
- Independent correctness review found and rechecked the two class ownership fixes. Maintainability
  review led to the ownership/receipt cleanup above. No remaining blocking finding was reported.
- `cargo fmt --all -- --check` and `git diff --check` passed.

The owner command is the same scoped command recorded in the
[Block Edge verification](2026-09-14-block-edge-direct-paint.md), applied to this source.

## Flutter artifact

The canonical `python3 platforms/flutter/build-native.py host` rebuilt flutter-desktop-native for
native-distribution/aarch64-apple-darwin. Actual Native Assets smoke passed, for each one-shot and
reusable consumer: 2 materializations, 19 support queries, 5 errors and 3 budgeted operation
comparisons. The ABI 3 Dart contract suite also passed, including the shared unknown qualification
identifier vectors introduced at `2e06b0101`.

- Packaged library size: 21843952 bytes.
- SHA-256: `94074337377baf554ed0e054e273390c38262a96b51933a830d2f729bfcb3056`.

## Remaining scope

This does not close C7a, retire the remaining Block provider/probes, qualify public preset cells,
or establish current Web/Node/Typst/Python artifact builds. Full workspace, full browser matrices
and all native hosts were not run. Artifact budgets will be assessed against justified product
capabilities and one-source canonical builds, under the
[budget reassessment decision](../../../performance/theme_artifact_budget_reassessment_2026-09-14.md).
