# HPD-050 - Architecture Probe Group Expansion Summary

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

The active Architecture root residuals still need source-phase evidence before any production
formula change. Existing `debug-architecture-fcose-probe` Markdown already exposed final
`node.boundingBox()`, `bodyBounds`, `labelBounds.all`,
`childrenBoundingBoxIncludeLabels`, and `childrenBoundingBoxBodyOnly`, but reviewers still had to
manually subtract child bboxes from final group bboxes to reason about final compound expansion.

That manual subtraction is exactly where the focused `+5px` group/service rows can become
misleading: a row may be child-contribution drift, final group expansion drift, or a combination.

## Outcome

- Added `format_probe_rect_expansion(...)` to `crates/xtask/src/cmd/debug/architecture.rs`.
- Extended the `Final Node Bounds` Markdown table with `bb over children labels`.
- The new column reports final `node.boundingBox()` expansion over
  `childrenBoundingBoxIncludeLabels` as left, right, top, bottom, `dw`, and `dh`.
- Verified a real `stress_architecture_batch5_long_titles_and_punct_076` probe writes the new
  column. The `pipeline` group reports:
  `l=41.500 r=41.500 t=41.500 b=41.500 dw=83.000 dh=83.000`.
- No Architecture layout, renderer, root-bounds, SVG, probe JSON, fixture, or baseline behavior
  changed.

## Verification

- `cargo nextest run -p xtask fcose_probe_markdown_summarizes_stage_and_node_bounds` - passed,
  `1` test run.
- `cargo nextest run -p xtask` - passed, `95` tests run.
- `cargo fmt --check -p xtask` - passed.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --out target\compare\architecture-fcose-probe-expansion-hpd050 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"` -
  passed and wrote a Markdown summary containing the new `bb over children labels` column.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- JSON parse gates passed for `CONTEXT.jsonl` (`565` records) and `WORKSTREAM.json`.

## Residual Boundary

This is source-evidence infrastructure only. It makes browser final group expansion explicit for
Architecture residual triage, but it does not change local group formulas or claim root residual
closure. Future formula work should cite this column together with local delta reports before
touching root-bounds code.
