# Alignment Milestones

This document tracks high-level alignment milestones for the pinned Mermaid baseline.

It defines release-oriented exit criteria, not the current gate result. Use
[`STATUS.md`](STATUS.md) for current admission, [`GAP_BACKLOG.md`](GAP_BACKLOG.md) for active work,
and verification output for the exact revision under review. The superseded
[`PARITY_HARDENING_PLAN.md`](PARITY_HARDENING_PLAN.md) is retained as historical evidence.

## Current transition

The active unreleased target is Mermaid `@12.1.0` at `21f72f07ea22c0af48a3149c550654e80d8e40cb`; milestone evidence below remains historical `12.0.0` admission evidence until refresh.

## Previous baseline evidence

- Mermaid baseline: `repo-ref/mermaid` at `mermaid@12.0.0`, commit `98a0945418c76238f15df2afaddbba4272656c3b` (see `tools/upstreams/REPOS.lock.json`).
- DOM gate: `parity-root` (root `<svg>` viewport + DOM structure, decimals = 3).

## Milestones

### M0: Baseline parity for the selected corpus

Exit criteria:

- `cargo nextest run` is green.
- `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --diagnostic-browser-text-layout` is green.
- Upstream SVG baselines are stored under `fixtures/upstream-svgs/**` and are traceable to the
  pinned Mermaid CLI toolchain.

### M1: Fixture expansion with zero-regression parity gates

Goal:

- Increase confidence by importing more upstream tests/docs fixtures, while keeping M0 gates green
  after each batch.

Batch policy:

- Prefer small batches (10–30 fixtures) that share a single upstream source file.
- Every imported fixture must be traceable to an upstream path (and pinned commit via
  `tools/upstreams/REPOS.lock.json`).

Exit criteria:

- `parity-root` remains green for the expanded corpus.
- No “silent drift”: new fixtures must include semantic + layout snapshots, and (when applicable)
  upstream SVG baselines.

### M2: Preserve fixture-free renderer behavior

The family-owned architecture migration removed the former fixture-scoped production overrides.
ADR-0062 and ADR-0073 define the maintained boundary; historical import notes do not authorize
reintroducing those overrides.

Goal:

- Derive renderer behavior from source semantics, geometry, configuration, and operation-owned
  measurement rather than fixture identity.

Exit criteria:

- No fixture-id keyed branches remain in Stage B SVG parity renderers.
- Global gates remain green for the current corpus.
- Each removed special-case is replaced by either:
  - an algorithmic/layout/measurement improvement, or
  - an ADR that documents an unavoidable upstream ambiguity (rare).

### M3: Preserve computed root viewport ownership

Goal:

- Compute every viewport through deterministic family or emitted-content bounds and shared root
  algorithms.

Exit criteria:

- No runtime root table or fixture-id lookup remains while M0 gates stay green.
- Browser-dependent residuals are explicit verification evidence under ADR-0062, never production
  output policy.

### M4: “Beyond parity-root” strict SVG XML parity (selective)

Goal:

- Where feasible, make `strict` mode XML compares match upstream, beyond structure-only parity.

Notes:

- This is intentionally diagram-by-diagram and not a gate for all diagrams on day one.

Exit criteria:

- At least one high-volume diagram (Flowchart) is `strict`-green at `--dom-decimals 3`.
- Any remaining strict diffs are documented in diagram-specific strict-gap notes (e.g.
  `docs/alignment/FLOWCHART_SVG_STRICT_XML_GAPS.md`).

### M5: ZenUML external-lane admission (complete for the pinned source)

Goal:

- Keep the family-owned ZenUML grammar, semantic/editor facts, typed layout, native SVG, and
  external browser comparison lane aligned to the admitted source while keeping Mermaid parity
  gates green.

Constraints:

- ZenUML is an external diagram upstream. The exact plugin graph is tested in an opaque browser
  realm, while the local headless path emits strict-validated native SVG.
- `merman` does not claim Mermaid upstream SVG baselines for ZenUML; the external lane owns its
  source-backed evidence separately.

Pinned-source evidence:

1. Family-owned parsing and semantic/editor facts live under
   `crates/merman-core/src/diagrams/zenuml/`.
2. Typed layout and SVG are covered by `crates/merman/tests/zenuml_typed_render.rs`.
3. `tools/upstreams/ZENUML_BROWSER_ADMISSION_PROBES.json` records the browser probe contract.
   Observations are produced only by the manual Mermaid upgrade admission workflow; a reviewed
   selection is retained by `MERMAID_SELECTION_DECISION.json`, outside the built-in Mermaid SVG
   matrix.

Exit criteria:

- The pinned ZenUML source passes parser, semantic/editor, typed-layout, native-SVG, and external
  browser admission evidence. Future Core upgrades must repeat that workflow.

## Gap backlog

For the prioritized gap list and execution plan, see:

- `docs/alignment/GAP_BACKLOG.md`

## Release notes

- The cross-channel workflow is [`../release/RELEASING.md`](../release/RELEASING.md).
- The Cargo-specific gate summary is [`../releasing/PUBLISHING.md`](../releasing/PUBLISHING.md).
