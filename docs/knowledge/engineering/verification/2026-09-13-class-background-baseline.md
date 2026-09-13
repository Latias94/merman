---
type: Verification Evidence
title: Class edge-label background pre-retirement rendering baseline
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,class,retirement,verification
---

# Boundary

This freezes the runtime comparison before retiring Class's static unqualified/Default
`EdgeLabelBackground.fill` bridge identities. Renderer source is `317acd7db`; the accompanying
changes add tests, CI selection, and this record. No production route, claim revision, or KTD23
authority changes in this slice. Block retains 36 routes and Class retains 26.

The existing independent upstream SVG witnesses in `legacy_projection_retirement.rs` record the
historical CSS and absent `data-look` attribute. The new native test exercises a diagram with two
namespaces, two classes, and a named relation. It covers five Mermaid schemes, Classic/Neo/HandDrawn,
both input HTML-label settings, unqualified/Default selectors, and solid/transparent values.

# Observations

- All 120 requested-paint comparisons preserve decoded PNG dimensions and RGBA pixels.
- Each request must reach the current bridge: the SVG's unused selector contains the requested
  solid or transparent value. This prevents a silently dropped request from passing the witness.
- The three unmatched selectors contain four projected color values: the edge-label selector's
  background, its `p` background, and its `rect` background/fill. Only those values are normalized;
  every other SVG byte must match the unthemed baseline. The values must agree with each other.
- The actual edge-label groups are present and have no `data-look` attribute. Native text survives
  conversion, and the named relation remains in the document.
- Node and NodeLabel controls change pixels in all 30 scene configurations. The baseline also
  requires nonempty dimensions and visible nonwhite pixels.

These are local macOS ARM64 observations with the actual native exporter. They do not certify
arbitrary host font environments or claim typed EdgeLabelBackground support.

# Reproduction

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked -p merman-theme-acceptance --no-default-features --features png --test class_edge_label_background_legacy_projection --test legacy_projection_retirement
python3 scripts/test_release_workflow_security.py
cargo fmt --all --check
git diff --check
```

The native tests pass 5/5, including the existing KTD23/inventory assertions; workflow tests pass
33/33. Logs: `/tmp/class-background-final.log` and `/tmp/class-background-workflows.log`.
CI and release-preflight explicitly select the new test through the internal-cfg runner with PNG.

The next slice must remove the exact legacy assignment, reconcile unsupported rules against the
Class label-background domain, compare output again, and authorize the two retired identities
through KTD23 before reducing the live route count. This record alone does not authorize deletion.
