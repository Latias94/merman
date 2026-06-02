# HPD-050 - Architecture Disconnected Islands Root Bounds

Date: 2026-06-02
Task: HPD-050 Architecture-first layout engine audit

## Context

`stress_architecture_disconnected_islands_046` is a useful residual because its width is already
aligned while its root height is not. That makes it a cleaner root-bounds phase probe than the
larger width-dominated Architecture rows.

The pinned Mermaid 11.15 `setupGraphViewbox(...)` path uses browser `svg.getBBox()` plus padding:
`width = svgBounds.width + padding * 2`, `height = svgBounds.height + padding * 2`, and the
`viewBox` is based on `svgBounds.x/y/width/height` plus the same padding.

## Evidence

Fresh current focused comparison:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_disconnected_islands_046 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_disconnected_islands_current_hpd050_audit.md`
- Expected failure: upstream `823.346x768.460`, local `823.346x775.647`.

Emitted-SVG bbox diagnostics after regenerating the local SVG from current code:

- Upstream SVG bbox plus padding:
  - bounds `(-329.672791,-276.729950)-(413.672791,411.729950)`
  - viewBox `823.345581x768.459900`
  - max-y contributor: group-left rect
- Local emitted SVG bbox plus padding:
  - bounds `(-329.672791,-279.229950)-(413.672791,392.229950)`
  - viewBox `823.345581x751.459900`
  - max-y contributor: top-level service D icon rect

This means the local emitted SVG scanner is too short for this row, while the final local root
viewport is too tall only after `finalize_architecture_root_viewport(...)` unions synthetic
`content_bounds` for labels that the emitted scanner cannot see.

## Rejected Experiment

A temporary experiment changed top-level service root contribution from `svg_root_bounds` to
`cytoscape_group_child_bounds`.

Result:

- Focused `stress_architecture_disconnected_islands_046` became exact:
  `823.346x768.460` vs `823.346x768.460`.
- Full Architecture `parity-root` mismatch count expanded from `26` to `84`.
- The regressions were broad, especially simple `iconText` / singleton service rows whose root
  heights shortened by about `7.25px`.

Decision: reject the global top-level-service switch. It fixes one row by weakening the SVG root
label phase for many rows.

## Conclusion

The next valid fix needs a phase-specific root label contribution model. The current seams should
remain distinct:

- `emitted_icon_bounds`: geometry visible to the SVG scanner,
- `cytoscape_group_child_bounds`: compound-child contribution for group sizing,
- `svg_root_bounds`: root-level `createText(...)` approximation for top-level services.

Do not collapse the top-level service path onto Cytoscape group-child bounds. The browser
`getBBox()` phase, Cytoscape compound sizing, and local emitted-bounds scanner are three different
surfaces here.

## Verification

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_disconnected_islands_046 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_disconnected_islands_current_hpd050_audit.md` expected failure
- `cargo run -p xtask -- debug-svg-bbox --svg fixtures\upstream-svgs\architecture\stress_architecture_disconnected_islands_046.svg --padding 40`
- `cargo run -p xtask -- debug-svg-bbox --svg target\compare\architecture\stress_architecture_disconnected_islands_046.svg --padding 40`
- `target\compare\architecture_report_parity_hpd050_resume.md`: structural Architecture parity green
- `target\compare\architecture_report_parity_root_hpd050_resume.md`: current full Architecture root residual count `26`
- `target\compare\architecture_disconnected_islands_cytoscape_top_level_experiment.md`: focused temporary experiment exact
- `target\compare\architecture_report_parity_root_cytoscape_top_level_experiment.md`: temporary experiment full root residual count `84`
