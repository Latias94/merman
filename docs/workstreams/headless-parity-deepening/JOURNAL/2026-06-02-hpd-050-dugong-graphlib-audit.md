# HPD-050 Dugong / Graphlib Source Audit

Date: 2026-06-02

## What Changed

- Cloned `repo-ref/dagre` and `repo-ref/graphlib`, then checked them out to the commits pinned in
  `tools/upstreams/REPOS.lock.json`.
- Added `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md` so Graphlib coverage is no longer an
  implicit assumption hidden behind Dagre tests.
- Ported the currently exposed Graphlib helper algorithm tests for:
  - `components`
  - `findCycles`
  - `preorder`
  - `postorder`
- Tightened `dugong_graphlib::alg::{preorder, postorder}` so a missing root panics instead of
  silently returning a traversal containing a non-existent node. This mirrors upstream Graphlib's
  throw behavior for the same invalid input.
- Fixed `tools/dagre-harness/run.mjs` to import `dagre-d3-es` from
  `tools/mermaid-cli/node_modules`, which is the installed Mermaid CLI toolchain used by current
  upstream SVG checks.
- Updated the Dagre harness README and stale debug comment so they no longer present 11.12-era
  Dagre package facts as current truth.

## Findings

- `dugong` already has broad direct Dagre test coverage: the pinned Dagre source has 264 upstream
  `it(...)` cases, while the Rust integration suite has more test functions because some upstream
  parameterized cases are expanded.
- `dugong-graphlib` did not have an independent coverage ledger before this slice. The pinned
  Graphlib source has 212 upstream `it(...)` cases; this pass ports only the public helper
  algorithm cases that are currently exposed and used by Dagre-style code.
- The next high-value Graphlib audit is the public `Graph` API, not shortest-path algorithms that
  current `dugong` and Mermaid-facing renderers do not consume.
- `tools/dagre-harness/run.mjs --help` previously failed before argument parsing because the root
  workspace does not install `dagre-d3-es`. The harness is now executable again.

## Verification

- `git -C repo-ref/dagre rev-parse HEAD` -> `ba986662394f8f3ed608717194e5958f3386ce01`
- `git -C repo-ref/graphlib rev-parse HEAD` -> `380d5efa1f4ab0904539f046bdba583d14ac2add`
- `cargo test -p dugong-graphlib --tests`
- `cargo test -p dugong --tests`
- `node tools/dagre-harness/run.mjs --help`
- `cargo run -p xtask -- compare-dagre-layout --fixture basic --out-dir target/compare/dagre-layout-hpd050-graphlib-audit`

The focused Dagre harness run reported `max node delta: 0.000000` and `max edge delta: 0.000000`
for `fixtures/state/basic.mmd`.
