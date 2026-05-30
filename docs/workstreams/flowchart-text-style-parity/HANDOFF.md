# Flowchart Text Style Parity Handoff

Current phase: measurement completeness in progress.

Active tasks:

- None.

Next action:

Pick up TSP-040 for spacing and line-height measurement semantics. Use fixture evidence before
modeling CSS effects beyond the current flowchart-specific font-style path.

Known constraints:

- Do not introduce a browser or CSS engine.
- Protect unrelated dirty worktree changes.
- Keep this lane focused on flowchart until spacing semantics are either implemented or explicitly
  rejected with evidence.
- `cargo nextest run -p merman-render --lib` currently has one unrelated local
  Node/KaTeX browser-shell measurement failure:
  `math::tests::node_katex_math_renderer_measures_sanitized_flowchart_browser_shell` reports
  height `27.265625` against its expected environment range.
