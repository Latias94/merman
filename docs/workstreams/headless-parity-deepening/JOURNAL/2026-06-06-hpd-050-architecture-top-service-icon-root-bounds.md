# HPD-050 Architecture Top-Service Icon Root-Bounds Audit

Date: 2026-06-06
Task: HPD-050 Architecture-first layout engine audit

## Context

After the direct service tail revalidation, the post-strict Architecture `parity-root` queue still
contained several small icon/service rows. This pass checked whether those rows expose a production
formula seam or should remain diagnostic browser/root-bounds lattice residuals.

## Evidence

- `target/compare/architecture-report-parity-root-top-service-icon-audit-hpd050.md`
- `target/compare/architecture-render-path-top-service-icon-hpd050`
- `target/compare/architecture-delta-top-service-icon-render-path-hpd050`

Commands:

- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target\compare\architecture-report-parity-root-top-service-icon-audit-hpd050.md`
- `cargo run -p xtask -- debug-architecture-render-path-probe --fixture stress_architecture_external_icons_005 --fixture upstream_architecture_cypress_fallback_icon --fixture upstream_cypress_architecture_spec_should_render_an_architecture_diagram_with_the_fallback_icon_004 --fixture upstream_html_demos_architecture_default_icon_from_unknown_icon_name_003 --fixture upstream_html_demos_architecture_external_icons_demo_012 --browser-exe "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --out target\compare\architecture-render-path-top-service-icon-hpd050`
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_external_icons_005 --fixture upstream_architecture_cypress_fallback_icon --fixture upstream_cypress_architecture_spec_should_render_an_architecture_diagram_with_the_fallback_icon_004 --fixture upstream_html_demos_architecture_default_icon_from_unknown_icon_name_003 --fixture upstream_html_demos_architecture_external_icons_demo_012 --render-probe-dir target\compare\architecture-render-path-top-service-icon-hpd050 --out target\compare\architecture-delta-top-service-icon-render-path-hpd050`

## Findings

The Architecture `parity-root` refresh expected-failed with the current `20` mismatch rows. The
five selected top-level service/icon fixtures remain in that diagnostic queue, with root residual
scores from `0.273438px` to `0.523438px`.

All five render-path probes reported `facts match: true`, so the probe facts match the stored
upstream SVGs.

The three fallback/default single-service icon fixtures share the same root attribution:

- root X owners are `service-unknown@0` and `service-unknown@80` on both sides;
- owner X deltas are `0`;
- the `-0.273438px` width residual comes from asymmetric root padding / text-bbox lattice
  differences, not from layout width.

`upstream_html_demos_architecture_external_icons_demo_012` is a no-group service-position lattice
row:

- all four service positions are shifted by `dx=-0.5`, `dy=-1.0`;
- root X owners are `service-fa` and `service-s3`;
- viewBox height is exact, and the remaining `+0.523438px` width residual is padding lattice on
  top of that uniform service shift.

`stress_architecture_external_icons_005` is group-owned:

- root X edges are owned by `group-cloud`;
- root padding stays stable at `40px`;
- the `+0.5px` width residual is exactly the emitted `group-cloud` SVG rect width delta.

## Outcome

No production behavior changed.

These rows are now classified as bounded root-bounds lattice diagnostics, not release-blocking
production formula candidates. Do not add root-padding, service-body, icon-size, or group-rect
constants for them. Future production work should return to the service child-label / final-bbox
model for the larger direct rows, or use a fresh source-backed Architecture/Dagre/Graphlib seam.

## Verification

- `compare-architecture-svgs --dom-mode parity-root` expected-failed with the active `20`
  Architecture root/style mismatch rows.
- `debug-architecture-render-path-probe` passed; all five fixtures reported `facts match: true`.
- `debug-architecture-delta --render-probe-dir` passed and wrote the joined reports.
- `cargo run -p xtask -- report-overrides --check-no-growth` passed; Architecture root overrides
  remain at `0`.
- `git diff --check` passed.
