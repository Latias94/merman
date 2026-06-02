# 2026-06-02 - HPD-050 Architecture Prelayout Adapter

Status: in progress

Extracted Architecture's pre-layout Cytoscape bbox adapter from the main layout function into
`architecture_fcose_prelayout_bounds(...)`.

Why this matters:

- The helper owns the Architecture-specific approximation that computes the FCoSE initial center
  and node `BoundsExtras`.
- `manatee` remains a reusable FCoSE port; Mermaid/Cytoscape-specific bbox policy stays in
  `merman-render`.
- Group title state was removed from the layout view because current source/evidence says group
  titles do not affect the pre-layout relocation center.

Verification:

- `cargo fmt --all`
- `cargo test -p merman-render architecture_prelayout_bounds_feed_label_extras_without_group_title_state --lib`
- `cargo test -p merman-render architecture_relative_constraints_preserve_mermaid_duplicate_bfs_pops --lib`
- `cargo test -p merman-render --test architecture_layout_test`
- `cargo run -p xtask -- report-overrides --check-no-growth`
- `git diff --check`

Residual check:

- Focused `stress_architecture_batch5_long_titles_and_punct_076` parity-root report remains the
  known `+5.000px` max-width tail (`542.926px` upstream vs `547.926px` local), so this refactor did
  not hide a root-width tune.
