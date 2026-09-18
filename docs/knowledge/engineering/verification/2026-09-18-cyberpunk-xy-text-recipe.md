# Public Cyberpunk XY text recipe

Date: 2026-09-18. Base: `699933336`.
Plan: [theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), U8.
Status: scoped verification complete; no qualification promotion.

The public Cyberpunk complete recipe now supplies family-scoped XY Title (18px, weight 700),
AxisTitle (13px) and Legend (12px), all filled `#00f2ff`. AxisTitle and Legend leave weight
unspecified; they do not clear another applicable rule. Tick-label sizing remains at its existing
baseline. The values come from the XY rules in local modern_mermaid `src/utils/themes.ts` at
verified commit `a021cbce37fc0b07a9f4791c28e983101ea06f2d`.

The first new public-entry test failed with Title 20px versus expected 18px. It calls actual
`compile_preset` and `export_preset`, then imports the saved preset into a fresh compiler.
Final checks cover both chart orientations, title/axis-title/legend attributes, unchanged tick
size, actual PNG/PDF export/admission and identical PNG pixels after exchange. The scoped recipe
test ensures the three font-size rules do not leak into other families. Both recipe tests reuse
one private native-export helper while retaining their distinct role-color assertions.

The first preset suite passed 15/17; both failures identified the expected recipe fingerprint
change. The exact compiled identity is now
`200fc3981ca90b06294d9a3ec6b35e7c7071d64ad765007f1c12f4c088cb75a8`.
The catalog constant was updated to that value. All unpublished protocol revisions remain 1;
resource identity, dependencies, effect graph count and qualification cells remain unchanged.

Verification:

- Renderer Release preset and XY regression: 91 passed, 2,481 filtered out.
- Public facade Release tests: 14 passed, including actual PNG/PDF exports and both orientations.
- Bindings metadata Release tests: 22 passed, 264 filtered out.
- Formatting and diff checks: passed.
- Renderer Clippy: completed successfully with 169 warnings in existing code; no diagnostic
  targets the new recipe helper or changed implementation lines. This is not a warning-free claim.

Independent source review completed with no findings. The six review lenses were applied in one
independent Codex context because the agent thread limit prevented separate reviewers; they are
not six independent reviews. Receipt:
`/tmp/compound-engineering-501/ce-code-review/20260919-014924-4035278d/review.json`.
The source review did not run Cargo or certify full scene qualification.

Local logs: `/tmp/merman-xy-preset-fonts-red.log`,
`/tmp/merman-xy-preset-fonts-identity.log`, `/tmp/merman-xy-preset-fonts-renderer.log`,
`/tmp/merman-xy-preset-fonts-facade.log`, `/tmp/merman-xy-preset-fonts-bindings.log`, and
`/tmp/merman-xy-preset-fonts-clippy.log`.

This increment does not add text glow, claim a complete visual reproduction, close U8/U9/U10,
or freeze C7a. The [text glow boundary investigation](2026-09-18-xychart-text-glow-design.md)
records the required shared measurement/ink/baseline path and the CSS-to-SVG blur conversion.
No full workspace, browser/installed-platform matrix or cost comparison is claimed here.
