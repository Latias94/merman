# Production theme provider retirement — 2026-09-14

Source: `1c37f8096b608009f8ee2c876efa21e29f23a8bb`.

## Result and boundary

Compiled themes no longer create or install the family Legacy overlay provider. The
renderer bridge/cache module is excluded from ordinary production compilation. Root Mermaid
compatibility configuration, parse identity, and family-owned post-detection default eligibility
remain installed. Family theme programs compile on demand for typed consumers, rather than
being prepared during parsing just to return an empty overlay.

The core plan can now contain no family provider. Installing that plan replaces any previous
fallback provider with None; a regression first exercises the old provider and then proves that
replacement stops its calls while preserving the new recipe and default-eligibility decisions.
A separate renderer test parses Block, Flowchart, Sequence and State without populating the
family program cache, then proves that resolving Block creates the selected typed program.
The generic core overlay mechanism remains available for its existing consumers and historical
tests; the retired renderer-specific provider does not run in production.

The independent KTD23 history/probes and unit-test provider harness are intentionally retained.
This change does not replace historical retirement authority with a hard-coded zero claim.
The matrix inventory remains empty and KTD17 v89 still authorizes 506 scalar routes with
814 route-profile witnesses; KTD23 v9 retains 80 historical routes and 160 value probes.

## Reconciled cutover and discovery

The broad owner run exposed omissions left by the preceding Block tranches:

- The renderer's expected route ledger omitted generic Text, used the old shared marker
  projection identity, and still expected four executable Block legacy routes.
- The Default selector counts were stale. Block has 11 admitted target/facet pairs, each with
  Solid and Transparent witnesses. Acceptance now checks each pair and its exact projection.
- The TreeView marker assertion accidentally used Block's separated projection constant.
- Public Block Text fill discovery still claimed legacy support after its typed cutover.
- The matrix classified Text stroke together with shape stroke despite having no text stroke
  writer. Text stroke is now Unsupported; stroke-only and mixed fill/stroke tests require a
  residual under BestEffort and rejection under RequirePortable.

Support revision 90 reports partial typed Block Text fill, with no broader portability claim.
The shared authoring fixture now includes its query and carries 22 support vectors. The public
support/matrix reconciliation passes across every current family, target and facet. No route
manifest version, authorization digest or qualification scope was expanded by these repairs.

## Verification

Main-worktree Release owner run: **4899/4899 passed, two skipped**. An independent detached
checkout at the exact source above repeated the same command: **4899/4899 passed, two skipped**,
with empty Git status before and after. Cargo used two build jobs and nextest two test threads.
The clean run reused the host target cache but recompiled repository crates from the independent
checkout path. The skipped tests explicitly require manual performance benchmarking (unsupported
palette discovery and static provenance workload); no functional retirement gate was skipped.

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-core -p merman-render -p merman-theme-acceptance -p merman-bindings-core \
  --features merman-render/layout-elk,merman-render/math,merman-bindings-core/svg \
  --lib --test block_svg_test --test theme_support_discovery_test \
  --test legacy_projection_retirement --test block_title_legacy_projection \
  --test class_edge_label_background_legacy_projection --test route_cutover_runtime \
  --test preset_qualification --no-fail-fast --test-threads 2
```

The production-configured full SVG structure gate passed in the main worktree:

```text
CARGO_BUILD_JOBS=2 cargo run --locked --release -p xtask -- compare-all-svgs \
  --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
```

Existing exact browser-text-layout residual allowances were used unchanged. No golden SVG,
normalizer or diagnostic allowance was modified. `cargo fmt --all -- --check` and
`git diff --check` passed. The initial narrowed run was insufficient: wider runs found the
failures listed above. An early default-concurrency failed run also reported three nextest leak
observations; neither final two-thread run reported a leak. Raw logs and checkout status are
retained under `target/bench/experiments/theme-provider-retirement/`.

## Remaining delivery work

This closes the production family provider removal slice. It does not freeze C7a, promote public
preset qualification cells, or attest rebuilt revision-90 Web/Node/Typst/Flutter/Python packages.
The earlier artifact budget measurements belong to their recorded source. Full workspace,
complete browser matrices and all platform builds were not run here. Continue with actual
revision-90 consumer artifacts and C7a public discovery/catalog rollout; preserve independent
historical authority while reducing any remaining migration-only facilities separately.
