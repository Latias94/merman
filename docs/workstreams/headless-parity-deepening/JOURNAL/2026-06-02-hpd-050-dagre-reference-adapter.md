# HPD-050 - Dagre Reference Adapter

Date: 2026-06-02

## Context

`docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md` identifies ARCH-022: the Dagre JS reference
runner and Rust `compare-dagre-layout` command duplicated input schema, compound-edge endpoint
normalization, harness invocation, and Rust/JS delta extraction. That made the tool useful for
State debugging but awkward to reuse for other Dagre-backed diagram audits.

## Outcome

- Extracted `crates/xtask/src/cmd/debug/dagre_reference.rs` as the Rust-side Dagre reference
  adapter.
- The new module owns:
  - `tools/dagre-harness/run.mjs` input JSON serialization,
  - Rust layout output snapshots,
  - JS harness invocation,
  - JS reference output parsing and max node/edge delta calculation,
  - the Mermaid-style compound-edge endpoint normalization mirrored from the JS harness.
- Left `compare-dagre-layout` State-only for this slice. It now mainly parses command args, builds
  the State Dagre graph, optionally extracts a State cluster subgraph, and delegates reference work
  to the adapter.
- Added a focused unit test for compound-edge normalization so the extracted adapter is not only
  covered by command smoke tests.

## Verification

- `cargo fmt --all`
- `cargo check -p xtask`
- `cargo test -p xtask compound_edge_normalization_moves_edges_to_non_cluster_child`
- `cargo test -p xtask`
- `node tools/dagre-harness/run.mjs --help`
- `cargo run -p xtask -- compare-dagre-layout --fixture basic --out-dir target\compare\dagre-layout-hpd050-reference-adapter`
- `cargo run -p xtask -- compare-dagre-layout --fixture stress_state_composite_with_external_edges_028 --out-dir target\compare\dagre-layout-hpd050-reference-adapter-composite`
- `cargo run -p xtask -- compare-dagre-layout --fixture stress_state_composite_with_external_edges_028 --cluster state-Big-7 --out-dir target\compare\dagre-layout-hpd050-reference-adapter-cluster`

All three layout comparisons reported zero max node delta and zero max edge delta.

## Notes

- `--cluster` takes the debug graph's internal cluster id. For
  `stress_state_composite_with_external_edges_028`, the valid id was `state-Big-7`, not the source
  alias `Big`.
- This does not broaden the command to non-State diagrams yet. The next useful step is to add
  diagram-specific graph producers only when a real Mermaid/Dagre residual audit needs them.
