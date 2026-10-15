# Tail family theme regression — Block, Class, and XY Chart

This record captures the current regression boundary for the three families that still have
executable legacy compatibility routes. It is evidence for the current candidate source, not a
bridge-retirement authorization.

## Source and inventory

- Source revision: `28cb9e916` (`docs(theme): refresh support manifest snapshot`)
- Legacy route inventory: 78 executable routes
- Family split: Block 36, Class 26, XY Chart 16
- The global legacy provider remains required by these three families.

## Verification

The following command ran with the pinned Rust toolchain, `CARGO_BUILD_JOBS=2`, and nextest:

```text
cargo nextest run --locked -p merman-render \
  -E 'test(block) | test(class) | test(xychart)' --test-threads 2
```

Result: **411 tests passed, 3,440 tests skipped**.

The selected tests cover Block and Class theme evidence, source/config ownership, typography and
paint precedence, strict residual handling, XY Chart palette and typography terminal receipts,
layout propagation, data-label behavior, and the family API projections. Release inventory also
passes independently and continues to report the same 36/26/16 split.

## Boundary

The green regression suite proves current typed and compatibility behavior for the exercised
scenarios. It does not prove that every legacy route has a typed terminal writer. Bridge removal
still requires route-specific terminal evidence, provider removal, and reconciliation of KTD17,
KTD18, KTD19, KTD23, and the renderer-owned support claims.
