# Theme Capability Deepening - Closeout

Status: Closed
Date: 2026-06-04

## Result

This lane is closed.

Completed tasks:

- `TCD-010`: created the durable follow-on lane and ADR.
- `TCD-020`: introduced render-side `PresentationTheme` and migrated Flowchart, Class, State,
  Sequence, and Block CSS/theme fallback consumers.
- `TCD-030`: introduced renderer-owned XyChart chart palette resolution.
- `TCD-040`: revalidated public `HeadlessRenderer` theme renderability coverage.
- `TCD-050`: reviewed, verified, and closed the lane.

## Shipped Behavior

- `merman-core` remains authoritative for Mermaid-compatible `themeVariables` expansion.
- `merman-render` now has a deeper render-side presentation theme view for the first
  high-duplication SVG/CSS consumers.
- XyChart plot palette selection now has one renderer-owned helper:
  - explicit resolved `themeVariables.xyChart.plotColorPalette` wins;
  - default theme keeps the legacy Mermaid default palette;
  - missing non-default chart palettes can derive series colors from `xyChart.accentColor` or
    `primaryColor`.
- Public renderability smoke still proves visible theme signals through `HeadlessRenderer`.

## Review

Review result:

- Workstream compliance: no blocking findings.
- Code quality: no blocking findings.
- Missing gates: none after stale filter commands were corrected.
- Residual risk: this lane does not claim full theme-system completion.

## Verification

Fresh closeout gate passed:

- `cargo fmt --check --all`
- `cargo nextest run -p merman-render flowchart_svg`
- `cargo nextest run -p merman-render class_svg`
- `cargo nextest run -p merman-render state_svg`
- `cargo nextest run -p merman-render sequence_svg`
- `cargo nextest run -p merman-render block_svg`
- `cargo nextest run -p merman-render presentation_theme`
- `cargo nextest run -p merman-render chart_palette`
- `cargo nextest run -p merman-render xychart`
- `cargo nextest run -p merman-render quadrantchart`
- `cargo test -p merman --features render --test theme_renderability_smoke`
- `git diff --check`

## Follow-Ons

Do not reopen this lane for broad theme-system work.

Open narrower follow-ons for:

- remaining raw theme access in specific diagram families;
- Mindmap/GitGraph/Radar/Pie or other palette-heavy contracts after source review;
- public bindings, playground UX, or schema surfaces;
- host-specific styling policy, which remains an SVG postprocessor boundary.

## Commit State

Not committed yet. Repo policy requires user confirmation before committing.

Suggested Conventional Commit message:

```text
refactor(merman-render): add render-side theme capability seams
```
