# Remaining Theme Cleanup Execution — October 10, 2026

This is the implementation record for the reopened
[plan](../plans/2026-10-10-1535-refactor-theme-addition-necessity-plan.md), following the
[call-chain review](theme_remaining_code_review_2026-10-10.md) at `a987774ff`.
The goal is active. Completed batches below do not establish overall completion.
No speed, memory or binary-size gain is inferred from source deletions.

## Exact Sequence declaration proof (U7/U9, F3)

Commit: `c2eb20866`. The writer proof now counts every matching declaration, including multiple
properties inside one rule. It retains the existing bounded observation grammar and does not
interpret browser CSS cascade. Native raster proofs remain separate.

The existing duplicate-rule test now also covers identical same-rule values and an earlier
`!important` value followed by the expected value. Before the fix, the important-predecessor
case failed its rejection assertion. After the fix, all 13 `c6_sequence_proof` tests passed:

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --locked \
  -p merman-theme-acceptance --lib -E 'test(c6_sequence_proof)' \
  --test-threads 2 --cargo-quiet
```

The whole library was initially invoked without diagram features; two preset tests rejected
Flowchart capability before reaching their assertions. That invocation is not a passing receipt.
The final owner run must use its actual diagram feature recipe. Focused Sequence proof tests
operate on observed SVG fixtures and passed under the stated recipe.

## Core overlay and family tests (U2/U6/U9, F2/F7/F9/F10)

Core commit `3b907235e` deletes the private host/fallback contribution executor, injectors,
application/provenance and empty production state. The live GitGraph/ER raw paint authority
freeze moves to `config/paint_inputs.rs`, consuming detector changes directly. Same-value
assignments, nested replacement, scoped appearance, secure filtering and keyword ELK tests
remain. This batch adds 352 and deletes 2,247 source/test lines: net 1,895 fewer lines.

Family test commit `ec39d2169` retains the 12 geometry executions and both palette/Sequence
consumers in local case tables. It removes three redundant renders and 166 net test lines.
Math assertions now inspect each native math subtree, and a negative control rejects empty
groups even when unrelated paths remain. The Swimlane test now counts backend render/measure
calls before and after SVG emission, retaining its sidecar and HTML checks. Production math
did not change; these are stronger test oracles, not evidence of a production math failure.

Bindings commit `f38bafabc` retains raw complete-spec reconstruction. Carrying its typed value
would require a context-aware Serde seed through the options/base/request-overlay/authoring
owners, a duplicate envelope schema or a placeholder payload. No measured benefit justifies
that complexity. A six-case test through `parse_options` confirms that raw payload errors
precede schema/layout/CSS while envelope errors retain their later position.

The integrated Core/family/theme owners passed 2,117 tests:

```text
CARGO_BUILD_JOBS=2 cargo nextest run --locked \
  -p merman-core -p merman-render -p merman-bindings-core \
  --features merman-render/all-diagrams,merman-render/math,merman-render/layout-elk,merman-bindings-core/all-diagrams,merman-bindings-core/svg,merman-bindings-core/layout-elk \
  --lib -E 'package(merman-core) | test(family::tests) | test(theme::tests)' \
  --test-threads 2 --cargo-quiet
```

An earlier invocation enabled ELK only on the renderer and failed the binding's advertised
capability check. The corrected recipe above enables both owners; no source gate was weakened.
Workspace formatting and `git diff --check` passed.

Playground commit `02ecec568` keeps one external-theme limitation in the export menu. Existing
mobile recipe/share interaction now requires exactly one explanation. TypeScript checking,
`build:prepared` and that Chromium browser case passed. The built Web runtime was reused for
this UI-only verification; it is not a fresh-WASM receipt for later Rust changes.

## Remaining gates

Font-chain retirement, local work reduction and final boundary dispositions remain open.
Authoritative Cargo runs are serialized by the main agent. Independent workers do not run
builds or modify the index. Final format/clippy,
affected owner checks, actual post-implementation review and accounting are still required.
